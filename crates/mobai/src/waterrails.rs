//! (tickexact 2026-11-12, ironspec 18:46: waterlogged rails as (C)'s descent's drag) `mctas mobai
//! water-rails`: on the official 26.3 server, two copies 8 apart in a void, each a straight descent
//! north of `--slopes` rails (ascending_south, the cart rolling down them north) and then `--flat`
//! flat north_south rails ending at a glass stop, every rail on glass between glass walls (the water
//! kept in); one copy dry, the other with every rail waterlogged. A cart with a villager riding is
//! summoned at rest on each copy's top slope (`--start` its motion north). Printed per copy, every
//! tick until it stops or `--ticks`: the cart's position and speed; the speed at the foot of the
//! slopes, the ticks to it, the speed over the flat and where it stopped.
//! From 26.3's OldMinecartBehavior / AbstractMinecart: in water a slope's slide is 0.0078125 x 0.2
//! a tick, the max speed 0.2 (else 0.4), the natural slowdown (0.997 ridden, 0.96 empty) times 0.95,
//! the off-track gravity 0.005 (else 0.04).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

/// The two copies (x 0 dry, x 8 wet): `wet_slopes` false keeps the wet copy's slopes (and its top
/// rail) dry, only its flats waterlogged.
fn build_rig(srv: &mut VanillaServer, slopes: i32, flat: i32, wet_slopes: bool) -> Result<()> {
    for (k, wet) in [false, true].into_iter().enumerate() {
        let x = k as i32 * 8;
        let ws = wet && wet_slopes;
        // slope i at (x, Y - i, -i), its high end south; the flat after it at y Y - slopes
        for i in 0..slopes {
            let (y, z) = (Y - i, -i);
            srv.cmd_ok(&format!("setblock {x} {} {z} minecraft:glass", y - 1))?;
            for wx in [x - 1, x + 1] {
                srv.cmd_ok(&format!("fill {wx} {} {z} {wx} {} {z} minecraft:glass", y - 1, y + 1))?;
            }
            srv.cmd_ok(&format!("setblock {x} {y} {z} minecraft:rail[shape=ascending_south,waterlogged={ws}]"))?;
        }
        let fy = Y - slopes;
        for j in 0..flat {
            let z = -slopes - j;
            srv.cmd_ok(&format!("setblock {x} {} {z} minecraft:glass", fy - 1))?;
            for wx in [x - 1, x + 1] {
                srv.cmd_ok(&format!("fill {wx} {} {z} {wx} {} {z} minecraft:glass", fy - 1, fy + 1))?;
            }
            srv.cmd_ok(&format!("setblock {x} {fy} {z} minecraft:rail[shape=north_south,waterlogged={wet}]"))?;
        }
        // the stop, and a flat rail at the top slope's high end (so it keeps its slope) on glass,
        // closed south by glass (the water kept in)
        srv.cmd_ok(&format!("fill {x} {fy} {} {x} {} {} minecraft:glass", -slopes - flat, fy + 1, -slopes - flat))?;
        srv.cmd_ok(&format!("setblock {x} {Y} 1 minecraft:glass"))?;
        for wx in [x - 1, x + 1] {
            srv.cmd_ok(&format!("fill {wx} {Y} 1 {wx} {} 1 minecraft:glass", Y + 2))?;
        }
        srv.cmd_ok(&format!("setblock {x} {} 1 minecraft:rail[shape=north_south,waterlogged={ws}]", Y + 1))?;
        srv.cmd_ok(&format!("fill {x} {} 2 {x} {} 2 minecraft:glass", Y + 1, Y + 2))?;
        // the slopes again, their shapes as stated (a slope placed before its neighbours reshapes)
        for i in 0..slopes {
            srv.cmd(&format!("setblock {x} {} {} minecraft:rail[shape=ascending_south,waterlogged={ws}]", Y - i, -i))?;
        }
    }
    Ok(())
}

pub fn water_rails(version: &str, slopes: i32, flat: i32, start: f64, ticks: u32, every_tick: bool) -> Result<()> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -64 31 16")?;
    srv.step(20)?;
    println!(
        "== water-rails on the official {version} server: a descent north of {slopes} slopes then {flat} flat rails, on glass between glass walls; a ridden cart from rest on the top slope (motion {start} north); dry vs waterlogged"
    );
    build_rig(&mut srv, slopes, flat, true)?;
    srv.step(10)?;
    for (k, wet) in [false, true].into_iter().enumerate() {
        let x = k as i32 * 8;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {x}.5 {}.5625 0.5 {{Tags:[\"wr{k}\"],Motion:[0.0d,0.0d,{}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b}}]}}",
            Y,
            -start
        ))?;
        let _ = wet;
    }
    // (b8949c96's unit test: every tick's position and full motion, `--every-tick`)
    let mut rows: [Vec<(u32, [f64; 3], f64)>; 2] = [Vec::new(), Vec::new()];
    let mut full: [Vec<(u32, [f64; 3], [f64; 3])>; 2] = [Vec::new(), Vec::new()];
    for t in 1..=ticks {
        srv.step(1)?;
        for (k, row) in rows.iter_mut().enumerate() {
            let p = nums(&srv.cmd_long(&format!("data get entity @e[tag=wr{k},limit=1] Pos"))?);
            let m = nums(&srv.cmd_long(&format!("data get entity @e[tag=wr{k},limit=1] Motion"))?);
            if p.len() == 3 && m.len() == 3 {
                let rel = [p[0] - (k as f64 * 8.0), p[1] - Y as f64, p[2]];
                row.push((t, rel, m[0].hypot(m[2])));
                full[k].push((t, rel, [m[0], m[1], m[2]]));
            }
        }
    }
    for (k, row) in rows.iter().enumerate() {
        let label = if k == 0 { "dry" } else { "WATERLOGGED" };
        let foot = row.iter().find(|(_, p, _)| p[2] < -(slopes as f64) + 0.5 + 0.0001 - 1.0 + 1.0 && p[1] <= -(slopes as f64) + 0.07);
        let stop = row.windows(2).find(|w| w[0].1 == w[1].1 && w[1].2 < 0.001).map(|w| w[1]);
        println!(
            "  {label}: at the foot of the slopes (on the flat) {}; stopped {}",
            foot.map(|(t, p, s)| format!("t{t} at z {:.3}, speed {s:.4}", p[2])).unwrap_or_else(|| "never".into()),
            stop.map(|(t, p, _)| format!("at t{t}, z {:.3}", p[2])).unwrap_or_else(|| format!("not within {ticks} gt")),
        );
        if every_tick {
            // the copy's own frame: x relative to its rail line (x 0.5 the rail's centre), y relative to Y
            println!("    t, x, y, z, vx, vy, vz, cell (x relative to the copy's rail column, y to {Y})");
            for (t, p, m) in &full[k] {
                println!(
                    "    {t}, {:.6}, {:.6}, {:.6}, {:+.6}, {:+.6}, {:+.6}, ({},{},{})",
                    p[0],
                    p[1],
                    p[2],
                    m[0],
                    m[1],
                    m[2],
                    p[0].floor() as i32,
                    p[1].floor() as i32,
                    p[2].floor() as i32
                );
            }
        } else {
            let path: Vec<String> = row.iter().filter(|(t, _, _)| *t <= 12 || t % 5 == 0).map(|(t, p, s)| format!("t{t} y {:.3} z {:.3} v {s:.4}", p[1], p[2])).collect();
            println!("    {}", path.join("; "));
        }
    }
    // every water cell around the wet copy that is not a source (the flowing water the slopes'
    // sources spill), by cell, relative to the copy
    let mut flowing = Vec::new();
    for y in (Y - slopes - 3)..=(Y + 3) {
        for z in (-slopes - flat - 1)..=3 {
            for dx in -3..=3 {
                let x = 8 + dx;
                let out = srv.cmd_long(&format!("execute if block {x} {y} {z} minecraft:water"))?;
                if out.contains("passed") {
                    let src = srv.cmd_long(&format!("execute if block {x} {y} {z} minecraft:water[level=0]"))?.contains("passed");
                    if !src {
                        flowing.push(format!("({dx},{},{z})", y - Y));
                    }
                }
            }
        }
    }
    println!("  the wet copy's flowing (non-source) water cells within 3 of its rail column, relative (x to the column, y to {Y}): {} {flowing:?}", flowing.len());
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
