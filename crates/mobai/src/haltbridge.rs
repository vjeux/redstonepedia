//! (tickexact 2026-11-12, ironspec and G1: the halt bridge) `mctas mobai halt-bridge`: on the
//! official 26.3 server, whether an unpowered powered rail holds a cart against another pressing
//! it. Each row is its own copy of the lane's foot (H: a closed bottom-half oak trapdoor on glass,
//! glass walls round it; R_1: an unpowered powered rail ascending south on glass; plain slopes up
//! behind it, all placed as stated), or a flat brake for the flat rows. Rows:
//! - `hold-empty`, `hold-ridden`: one cart at rest on R_1 for 400 gt;
//! - `pair-empty`: a cart at rest on R_1, a second empty one rolled down the slope onto it (must slide);
//! - `pair-ridden-V`: the same, both ridden, the upper one arriving at about V b/gt (the lane's queue
//!   contact speeds 0.106 .. 0.459; must hold at 0.79);
//! - `flat-ridden`: a held cart on a flat unpowered powered rail, a ridden cart rolled onto it
//!   (G1's discriminating row: at the rider-box pitch the cart boxes overlap at the same height).
//! Every tick: both carts' position, motion and yaw; the boxes' vertical gap; whether each cart's
//! push search (its box inflated 0.2 sideways, 0 vertically: `OldMinecartBehavior.pushAndPickupEntities`)
//! meets the other's box; the |cos| `AbstractMinecart.pushOtherMinecart` tests (the line between the
//! centres against the pushed cart's yaw, at least 0.8 to push).

use anyhow::{bail, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

#[derive(Clone, Copy, Debug)]
struct Cart {
    pos: [f64; 3],
    motion: [f64; 3],
    yaw: f64,
}

fn read(srv: &mut VanillaServer, tag: &str) -> Result<Option<Cart>> {
    let nums = |s: &str| -> Vec<f64> { s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f']).parse().ok()).collect() };
    let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
    if p.len() != 3 {
        return Ok(None);
    }
    let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Motion"))?);
    let r = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Rotation"))?);
    Ok(Some(Cart { pos: [p[0], p[1] - DY as f64, p[2]], motion: [m.first().copied().unwrap_or(0.0), m.get(1).copied().unwrap_or(0.0), m.get(2).copied().unwrap_or(0.0)], yaw: r.first().copied().unwrap_or(0.0) }))
}

/// A cart's box: 0.98 wide, 0.7 tall, from its feet.
fn bbox(c: &Cart) -> ([f64; 3], [f64; 3]) {
    ([c.pos[0] - 0.49, c.pos[1], c.pos[2] - 0.49], [c.pos[0] + 0.49, c.pos[1] + 0.7, c.pos[2] + 0.49])
}

/// Whether `a`'s push search (its box inflated 0.2 in x and z) meets `b`'s box.
fn search_meets(a: &Cart, b: &Cart) -> bool {
    let (alo, ahi) = bbox(a);
    let (blo, bhi) = bbox(b);
    let (alo, ahi) = ([alo[0] - 0.2, alo[1], alo[2] - 0.2], [ahi[0] + 0.2, ahi[1], ahi[2] + 0.2]);
    (0..3).all(|k| alo[k] < bhi[k] && blo[k] < ahi[k])
}

/// `pushOtherMinecart`'s |cos|: the horizontal line from `pushed` to `other` against `pushed`'s yaw.
fn cos(pushed: &Cart, other: &Cart) -> f64 {
    let (dx, dz) = (other.pos[0] - pushed.pos[0], other.pos[2] - pushed.pos[2]);
    let l = (dx * dx + dz * dz).sqrt();
    if l < 1e-9 {
        return 0.0;
    }
    let y = (pushed.yaw as f32).to_radians();
    ((dx / l) * y.cos() as f64 + (dz / l) * y.sin() as f64).abs()
}

/// The lane's foot at x `ox`: H (oz), R_1 (oz+1, y+1), plain slopes up to (oz+6, y+6), a stone
/// high end; or with `flat`, a flat brake at (ox, 0, oz+1) and plain flat rails behind it.
fn build(srv: &mut VanillaServer, ox: i32, flat: bool) -> Result<()> {
    let (y, oz) = (DY, 0);
    srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:air", ox - 2, y - 2, oz - 2, ox + 2, y + 10, oz + 10))?;
    if flat {
        srv.cmd_ok(&format!("fill {ox} {} {oz} {ox} {} {} minecraft:glass", y - 1, y - 1, oz + 8))?;
        srv.cmd_ok(&format!("setblock {ox} {y} {oz} minecraft:glass"))?;
        srv.cmd_ok(&format!("setblock {ox} {y} {} minecraft:powered_rail[shape=north_south,powered=false] strict", oz + 1))?;
        for k in 2..=8 {
            srv.cmd_ok(&format!("setblock {ox} {y} {} minecraft:rail[shape=north_south] strict", oz + k))?;
        }
        return Ok(());
    }
    // H: a closed bottom trapdoor on glass, glass walls north, east and west
    srv.cmd_ok(&format!("setblock {ox} {} {oz} minecraft:glass", y - 1))?;
    srv.cmd_ok(&format!("setblock {ox} {y} {oz} minecraft:oak_trapdoor[facing=north,half=bottom,open=false]"))?;
    srv.cmd_ok(&format!("fill {} {y} {} {} {} {} minecraft:glass", ox - 1, oz - 1, ox + 1, y + 1, oz - 1))?;
    srv.cmd_ok(&format!("fill {} {y} {oz} {} {} {oz} minecraft:glass", ox - 1, ox - 1, y + 1))?;
    srv.cmd_ok(&format!("fill {} {y} {oz} {} {} {oz} minecraft:glass", ox + 1, ox + 1, y + 1))?;
    for k in 1..=6 {
        srv.cmd_ok(&format!("setblock {ox} {} {} minecraft:glass", y + k - 1, oz + k))?;
        let s = if k == 1 { "minecraft:powered_rail[shape=ascending_south,powered=false]" } else { "minecraft:rail[shape=ascending_south]" };
        srv.cmd_ok(&format!("setblock {ox} {} {} {s} strict", y + k, oz + k))?;
    }
    srv.cmd_ok(&format!("setblock {ox} {} {} minecraft:stone", y + 6, oz + 7))?;
    Ok(())
}

fn summon(srv: &mut VanillaServer, tag: &str, p: [f64; 3], vz: f64, ridden: bool) -> Result<()> {
    let pass = if ridden { format!(",Passengers:[{{id:\"minecraft:villager\",NoAI:1b,Tags:[\"{tag}v\"]}}]") } else { String::new() };
    srv.cmd_ok(&format!("summon minecraft:minecart {} {} {} {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,{vz}d]{pass}}}", p[0], p[1] + DY as f64, p[2]))?;
    Ok(())
}

pub fn halt_bridge(version: &str, rows: &str, ticks: u32) -> Result<()> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("forceload add -16 -16 160 31")?;
    srv.step(20)?;
    println!("== halt-bridge on the official {version} server (y less {DY}; R_1 at (x,1,1), H at (x,0,0); a slope cell k at (x,k,k))");
    for (i, row) in rows.split(',').enumerate() {
        let ox = 8 * i as i32;
        let flat = row.starts_with("flat");
        build(&mut srv, ox, flat)?;
        srv.step(2)?;
        let x = ox as f64 + 0.5;
        let r1 = if flat { [x, 0.0625, 1.5] } else { [x, 1.5625, 1.5] };
        let (ridden, second, speed) = match row {
            "hold-empty" => (false, false, 0.0),
            "hold-ridden" => (true, false, 0.0),
            "pair-empty" => (false, true, 0.0),
            "flat-ridden" => (true, true, 0.3),
            r if r.starts_with("pair-ridden-near-") => (true, true, r["pair-ridden-near-".len()..].parse::<f64>().map_err(|_| anyhow::anyhow!("pair-ridden-near-V: V a speed, got {r}"))?),
            r if r.starts_with("pair-ridden-") => (true, true, r["pair-ridden-".len()..].parse::<f64>().map_err(|_| anyhow::anyhow!("pair-ridden-V: V a speed, got {r}"))?),
            r => bail!("unknown row {r} (hold-empty, hold-ridden, pair-empty, pair-ridden-V, flat-ridden)"),
        };
        summon(&mut srv, &format!("a{i}"), r1, 0.0, ridden)?;
        srv.step(20)?;
        if second {
            // the second cart: 3 cells up the slope (or 4 back on the flat), at rest or already moving
            // down at `speed` (b/gt along z); `pair-ridden-near-V`: 1 cell up, so it meets the held
            // cart within a quarter block and its contact speed stays near V (G1: from 3 cells up
            // the roll adds about 0.05)
            let p = if flat {
                [x, 0.0625, 5.5]
            } else if row.starts_with("pair-ridden-near-") {
                [x, 2.5625, 2.5]
            } else {
                [x, 4.5625, 4.5]
            };
            summon(&mut srv, &format!("b{i}"), p, -speed, ridden)?;
        }
        println!("== row {row} (x {ox}{}):", if flat { ", flat brake" } else { "" });
        let mut held_moved: Option<u32> = None;
        // the arriving cart's speed the tick before it met the held one (it stops dead, or a push
        // search first meets): the row's real contact speed (the summon speed is 3 cells up)
        let (mut last_speed, mut contact_speed): (f64, Option<f64>) = (0.0, None);
        let a0 = read(&mut srv, &format!("a{i}"))?;
        for t in 1..=ticks {
            srv.step(1)?;
            let a = read(&mut srv, &format!("a{i}"))?;
            let b = if second { read(&mut srv, &format!("b{i}"))? } else { None };
            if let (Some(a), Some(a0)) = (a, a0) {
                if held_moved.is_none() && ((a.pos[2] - a0.pos[2]).abs() > 1e-4 || (a.pos[1] - a0.pos[1]).abs() > 1e-4) {
                    held_moved = Some(t);
                }
            }
            if let (Some(a), Some(b)) = (a, b) {
                let speed = (b.motion[0].powi(2) + b.motion[2].powi(2)).sqrt();
                if contact_speed.is_none() && last_speed > 0.01 && (speed < 0.001 || search_meets(&b, &a) || search_meets(&a, &b)) {
                    contact_speed = Some(last_speed);
                }
                last_speed = speed;
            }
            let fmt = |c: &Cart| format!("({:.4},{:.4},{:.4}) v({:+.4},{:+.4},{:+.4}) yaw {:.1}", c.pos[0], c.pos[1], c.pos[2], c.motion[0], c.motion[1], c.motion[2], c.yaw);
            let line = match (a, b) {
                (Some(a), Some(b)) => {
                    let gap = if b.pos[1] >= a.pos[1] { b.pos[1] - (a.pos[1] + 0.7) } else { a.pos[1] - (b.pos[1] + 0.7) };
                    format!(
                        "held {} | other {} | vertical gap {:+.4} | b's search meets a {} (|cos| at a {:.3}) | a's search meets b {} (|cos| at b {:.3})",
                        fmt(&a),
                        fmt(&b),
                        gap,
                        search_meets(&b, &a),
                        cos(&a, &b),
                        search_meets(&a, &b),
                        cos(&b, &a)
                    )
                }
                (Some(a), None) => format!("held {}", fmt(&a)),
                _ => "the held cart is gone".to_string(),
            };
            if t <= 120 || t % 20 == 0 {
                println!("  t{t} {line}");
            }
        }
        let end = read(&mut srv, &format!("a{i}"))?;
        println!(
            "  row {row}: the held cart first moved at {}; at the end {:?} (it started at {:?}); the arriving cart's contact speed {}",
            held_moved.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
            end.map(|c| c.pos),
            a0.map(|c| c.pos),
            contact_speed.map(|v| format!("{v:.4} b/gt")).unwrap_or_else(|| "-".into())
        );
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
