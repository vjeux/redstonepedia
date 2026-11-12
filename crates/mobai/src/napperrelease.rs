//! (tickexact 2026-11-12, ironspec: the row fix's napper release) `mctas mobai napper-release`: on
//! the official 26.3 server, the composition (the module and its parts, rails as stated); a ridden
//! napper arrives into a siding from K's cell at each of `--arrivals` (so it rests where that arrival
//! leaves it on R), waits `--wait` gt, then the siding's switch is pulsed (a redstone block at the
//! repeater's input `--switch` for `--pulse` gt: R powered through its support); every tick its
//! position and motion until it has left R northward (down into the lane) and come to rest, or
//! `--ticks` run out. Per arrival: the rest z, the tick R powered (the repeater), the tick the cart
//! moved, the tick it left R and its speed then, where it came to rest; `--phases` repeats each
//! release at that many different game-tick phases (the wait stepped by 1 gt).

use crate::seatclear::{module_mismatches, place_part, start_world_part};
use anyhow::Result;
use mctas::vanilla::VanillaServer;

const DY: i32 = 80;

fn read(srv: &mut VanillaServer, tag: &str) -> Result<Option<([f64; 3], [f64; 3])>> {
    let nums = |s: &str| -> Vec<f64> { s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f']).parse().ok()).collect() };
    let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
    if p.len() != 3 {
        return Ok(None);
    }
    let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Motion"))?);
    Ok(Some(([p[0], p[1] - DY as f64, p[2]], [m.first().copied().unwrap_or(0.0), m.get(1).copied().unwrap_or(0.0), m.get(2).copied().unwrap_or(0.0)])))
}

#[allow(clippy::too_many_arguments)]
pub fn napper_release(version: &str, module: &std::path::Path, parts: &[std::path::PathBuf], k: [i32; 3], switch: [i32; 3], arrivals: &[f64], wait: u32, pulse: u32, phases: u32, ticks: u32) -> Result<()> {
    println!(
        "== napper-release on the official {version} server: {} with {}; K ({},{},{}), the switch's input ({},{},{}) pulsed {pulse} gt; arrivals {arrivals:?}, {phases} phases each",
        module.display(),
        parts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "),
        k[0],
        k[1],
        k[2],
        switch[0],
        switch[1],
        switch[2]
    );
    let (mut srv, _agent, _bed) = start_world_part(version, module, false, &[], None)?;
    for p in parts {
        place_part(&mut srv, p)?;
    }
    srv.step(10)?;
    for f in std::iter::once(module).chain(parts.iter().map(|p| p.as_path())) {
        let m = module_mismatches(&mut srv, f)?;
        println!("  module-check {}: {} cells differ{}", f.display(), m.len(), if m.is_empty() { String::new() } else { format!(" (first: {})", m.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) });
    }
    let below = srv.cmd_long(&format!("execute if block {} {} {} minecraft:slime_block", k[0], k[1] - 2 + DY, k[2] - 2))?.contains("passed");
    println!("  R's support ({},{},{}) is slime: {below}", k[0], k[1] - 2, k[2] - 2);
    let mut n = 0u32;
    for v in arrivals {
        for ph in 0..phases {
            n += 1;
            let tag = format!("nap{n}");
            srv.cmd_ok(&format!(
                "summon minecraft:minecart {} {} {} {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,{}d],Passengers:[{{id:\"minecraft:villager\",NoAI:1b,Tags:[\"{tag}v\"]}}]}}",
                k[0] as f64 + 0.5,
                k[1] as f64 + 0.0625 + DY as f64,
                k[2] as f64 + 0.5,
                -v
            ))?;
            srv.step(wait + ph)?;
            let Some((rest, _)) = read(&mut srv, &tag)? else {
                println!("  arrival {v} phase {ph}: the cart is gone before the release");
                continue;
            };
            srv.cmd_ok(&format!("setblock {} {} {} minecraft:redstone_block", switch[0], switch[1] + DY, switch[2]))?;
            let (mut moved, mut left_r, mut speed_out, mut powered_at) = (None, None, 0.0f64, None);
            let mut last = rest;
            let mut still = 0;
            for t in 1..=ticks {
                srv.step(1)?;
                if t == pulse {
                    srv.cmd_ok(&format!("setblock {} {} {} minecraft:air", switch[0], switch[1] + DY, switch[2]))?;
                }
                if powered_at.is_none() && srv.cmd_long(&format!("execute if block {} {} {} minecraft:powered_rail[powered=true]", k[0], k[1] - 1 + DY, k[2] - 2))?.contains("passed") {
                    powered_at = Some(t);
                }
                let Some((p, m)) = read(&mut srv, &tag)? else { break };
                if moved.is_none() && (p[2] - last[2]).abs() > 1e-4 {
                    moved = Some(t);
                }
                // out of the siding: past R's low (north) end, z < R's z
                if left_r.is_none() && p[2] < (k[2] - 2) as f64 {
                    left_r = Some(t);
                    speed_out = (m[0].powi(2) + m[2].powi(2)).sqrt();
                }
                still = if (p[0] - last[0]).abs() + (p[1] - last[1]).abs() + (p[2] - last[2]).abs() < 1e-4 { still + 1 } else { 0 };
                last = p;
                if left_r.is_some() && still >= 10 {
                    break;
                }
            }
            println!(
                "  arrival {v} phase {ph}: rested at z {:.4} (y {:.4}); R powered at +{}; the cart moved at +{}; left R (north of z {}) at +{} at {:.4} b/gt; it came to rest at ({:.3},{:.3},{:.3})",
                rest[2],
                rest[1],
                powered_at.map(|t| t.to_string()).unwrap_or_else(|| "never".into()),
                moved.map(|t| t.to_string()).unwrap_or_else(|| "NEVER".into()),
                k[2] - 2,
                left_r.map(|t| t.to_string()).unwrap_or_else(|| "NEVER".into()),
                speed_out,
                last[0],
                last[1],
                last[2]
            );
            srv.cmd_ok(&format!("kill @e[tag={tag}]"))?;
            srv.cmd_ok(&format!("kill @e[tag={tag}v]"))?;
            srv.cmd(&format!("setblock {} {} {} minecraft:air", switch[0], switch[1] + DY, switch[2]))?;
            srv.step(40)?;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
