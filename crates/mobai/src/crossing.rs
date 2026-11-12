//! (tickexact 2026-11-12, ironspec and G1: the S-bend crossing margin) `mctas mobai crossing`: on
//! the official 26.3 server, the composition (the module and its parts, rails as stated) with a
//! ridden napper cart at rest on the siding's R at `--napper` (its range's top, from the slowest
//! arrival), and ridden through-carts on the split row started at `--through` moving east at each
//! of `--speeds` (one pass each), entering K from the west. Every tick while a through-cart is
//! within 3 blocks: both carts' boxes, the box gap in z and in y, whether each push search (the box
//! inflated 0.2 sideways, 0 vertically: `OldMinecartBehavior.pushAndPickupEntities`) meets the
//! other cart, both speeds, and any change of the napper's position.

use crate::seatclear::{module_mismatches, place_part, start_world_part};
use anyhow::Result;
use mctas::vanilla::VanillaServer;

const DY: i32 = 80;

#[derive(Clone, Copy)]
struct Cart {
    pos: [f64; 3],
    motion: [f64; 3],
}

fn read(srv: &mut VanillaServer, tag: &str) -> Result<Option<Cart>> {
    let nums = |s: &str| -> Vec<f64> { s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f']).parse().ok()).collect() };
    let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
    if p.len() != 3 {
        return Ok(None);
    }
    let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Motion"))?);
    Ok(Some(Cart { pos: [p[0], p[1] - DY as f64, p[2]], motion: [m.first().copied().unwrap_or(0.0), m.get(1).copied().unwrap_or(0.0), m.get(2).copied().unwrap_or(0.0)] }))
}

/// The gap between two cart boxes (0.98 wide, 0.7 tall) along an axis: positive apart.
fn gap(a: &Cart, b: &Cart, k: usize) -> f64 {
    let (half_a, half_b) = if k == 1 { (0.0, 0.0) } else { (0.49, 0.49) };
    let (alo, ahi) = if k == 1 { (a.pos[1], a.pos[1] + 0.7) } else { (a.pos[k] - half_a, a.pos[k] + half_a) };
    let (blo, bhi) = if k == 1 { (b.pos[1], b.pos[1] + 0.7) } else { (b.pos[k] - half_b, b.pos[k] + half_b) };
    (blo - ahi).max(alo - bhi)
}

fn search_meets(a: &Cart, b: &Cart) -> bool {
    gap(a, b, 0) < 0.2 && gap(a, b, 2) < 0.2 && gap(a, b, 1) < 0.0
}

#[allow(clippy::too_many_arguments)]
pub fn crossing(version: &str, module: &std::path::Path, parts: &[std::path::PathBuf], air: &[[i32; 3]], napper: [f64; 3], through: [f64; 3], speeds: &[f64], ticks: u32) -> Result<()> {
    println!(
        "== crossing on the official {version} server: {} with {}; the napper at rest at ({},{},{}), through-carts from ({},{},{}) east at {speeds:?}",
        module.display(),
        parts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "),
        napper[0],
        napper[1],
        napper[2],
        through[0],
        through[1],
        through[2]
    );
    let (mut srv, _agent, _bed) = start_world_part(version, module, false, &[], None)?;
    for p in parts {
        place_part(&mut srv, p)?;
    }
    for a in air {
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:air", a[0], a[1] + DY, a[2]))?;
    }
    srv.step(10)?;
    for f in std::iter::once(module).chain(parts.iter().map(|p| p.as_path())) {
        let m = module_mismatches(&mut srv, f)?;
        println!("  module-check {}: {} cells differ{}", f.display(), m.len(), if m.is_empty() { String::new() } else { format!(" (first: {})", m.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) });
    }
    srv.cmd_ok(&format!("summon minecraft:minecart {} {} {} {{Tags:[\"napper\"],Passengers:[{{id:\"minecraft:villager\",NoAI:1b}}]}}", napper[0], napper[1] + DY as f64, napper[2]))?;
    srv.step(20)?;
    let n0 = read(&mut srv, "napper")?.ok_or_else(|| anyhow::anyhow!("the napper cart is gone"))?;
    println!("  the napper settled at ({:.4},{:.4},{:.4})", n0.pos[0], n0.pos[1], n0.pos[2]);
    for (i, v) in speeds.iter().enumerate() {
        let tag = format!("thru{i}");
        srv.cmd_ok(&format!("summon minecraft:minecart {} {} {} {{Tags:[\"{tag}\"],Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",NoAI:1b,Tags:[\"{tag}v\"]}}]}}", through[0], through[1] + DY as f64, through[2]))?;
        let (mut least_z, mut meets, mut moved) = (f64::MAX, 0u32, 0.0f64);
        for t in 1..=ticks {
            srv.step(1)?;
            let (Some(n), Some(c)) = (read(&mut srv, "napper")?, read(&mut srv, &tag)?) else {
                println!("  pass {i} at {v}: t{t} a cart is gone");
                break;
            };
            let d = ((n.pos[0] - c.pos[0]).powi(2) + (n.pos[2] - c.pos[2]).powi(2)).sqrt();
            let shift = (n.pos[0] - n0.pos[0]).abs() + (n.pos[1] - n0.pos[1]).abs() + (n.pos[2] - n0.pos[2]).abs();
            moved = moved.max(shift);
            if d < 3.0 {
                let (gz, gy) = (gap(&n, &c, 2), gap(&n, &c, 1));
                least_z = least_z.min(gz);
                let (m1, m2) = (search_meets(&c, &n), search_meets(&n, &c));
                if m1 || m2 {
                    meets += 1;
                }
                println!(
                    "  pass {i} at {v} t{t}: through ({:.4},{:.4},{:.4}) v({:+.4},{:+.4},{:+.4}) | napper ({:.4},{:.4},{:.4}) v({:+.4},{:+.4},{:+.4}) moved {:.4} | box gap z {gz:+.4} y {gy:+.4} | through's search meets the napper {m1}, the napper's meets the through {m2}",
                    c.pos[0], c.pos[1], c.pos[2], c.motion[0], c.motion[1], c.motion[2], n.pos[0], n.pos[1], n.pos[2], n.motion[0], n.motion[1], n.motion[2], shift
                );
            }
        }
        println!("  pass {i} at {v}: the least box gap in z {least_z:+.4}; ticks a push search met the other cart {meets}; the napper's largest position change {moved:.4}");
        srv.cmd_ok(&format!("kill @e[tag={tag}]"))?;
        srv.cmd_ok(&format!("kill @e[tag={tag}v]"))?;
        srv.step(5)?;
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
