//! (tickexact 2026-11-12, ironspec) `mctas mobai seat-offset`: on the official 26.3 server, a
//! seated villager's position minus its cart's, both read in the same tick (the server paused
//! between ticks), on a flat rail and on an ascending one, at rest and moving. The lanes model the
//! seat at cart + 0.5 in y; this is the official number.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn pos(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1], v[2]]))
}

fn row(srv: &mut VanillaServer, label: &str, cart: &str) -> Result<String> {
    let c = pos(srv, &format!("@e[tag={cart},limit=1]"))?;
    let v = pos(srv, &format!("@e[tag={cart}v,limit=1]"))?;
    Ok(match (c, v) {
        (Some(c), Some(v)) => format!(
            "{label}: cart ({:.4},{:.4},{:.4}) villager ({:.4},{:.4},{:.4}) villager - cart ({:+.4},{:+.4},{:+.4})",
            c[0],
            c[1] - DY as f64,
            c[2],
            v[0],
            v[1] - DY as f64,
            v[2],
            v[0] - c[0],
            v[1] - c[1],
            v[2] - c[2]
        ),
        _ => format!("{label}: not found ({c:?}, {v:?})"),
    })
}

pub fn seat_offset(version: &str) -> Result<()> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("forceload add -16 -16 47 47")?;
    srv.cmd_ok(&format!("fill -4 {} -4 40 {} 12 minecraft:air", DY - 2, DY + 12))?;
    srv.step(20)?;
    // flat: a straight rail on stone, x 0..30 at z 0
    srv.cmd_ok(&format!("fill 0 {} 0 30 {} 0 minecraft:stone", DY - 1, DY - 1))?;
    srv.cmd_ok(&format!("fill 0 {DY} 0 30 {DY} 0 minecraft:rail[shape=east_west] strict"))?;
    // ascending: rails stepping up east at z 6 from x 0 (y DY) to x 6 (y DY+6), each on stone; the
    // lowest an unpowered powered rail (a brake that holds a cart at rest on the slope)
    for i in 0..=6 {
        srv.cmd_ok(&format!("setblock {i} {} 6 minecraft:stone", DY + i - 1))?;
        let s = if i == 0 { "minecraft:powered_rail[shape=ascending_east,powered=false]".to_string() } else { "minecraft:rail[shape=ascending_east]".to_string() };
        srv.cmd_ok(&format!("setblock {i} {} 6 {s} strict", DY + i))?;
    }
    srv.cmd_ok(&format!("setblock 7 {} 6 minecraft:stone", DY + 6))?;
    // off-rail: a closed bottom-half oak trapdoor (as H), the cart resting on it
    srv.cmd_ok(&format!("setblock 12 {} 10 minecraft:stone", DY - 1))?;
    srv.cmd_ok(&format!("setblock 12 {DY} 10 minecraft:oak_trapdoor[half=bottom,open=false,facing=north]"))?;
    for (tag, x, y, z) in [("flat", 2.5, DY as f64, 0.5), ("slope", 0.5, DY as f64 + 0.5, 6.5), ("trap", 12.5, DY as f64 + 0.5, 10.5)] {
        srv.cmd_ok(&format!("summon minecraft:minecart {x} {y} {z} {{Tags:[\"{tag}\"],Passengers:[{{id:\"minecraft:villager\",Tags:[\"{tag}v\"],NoAI:1b}}]}}"))?;
    }
    srv.step(40)?;
    println!("== seat-offset on the official {version} server (y less {DY})");
    println!("  {}", row(&mut srv, "flat rail, at rest", "flat")?);
    println!("  {}", row(&mut srv, "ascending rail (the brake), at rest", "slope")?);
    println!("  {}", row(&mut srv, "off-rail on a closed bottom-half trapdoor, at rest", "trap")?);
    // moving: the flat cart pushed east, the slope cart released (the brake powered) to roll down…
    // it sits on the lowest cell, so instead it is pushed up the slope and rolls back down
    srv.cmd_ok("data merge entity @e[tag=flat,limit=1] {Motion:[0.4d,0.0d,0.0d]}")?;
    srv.cmd_ok(&format!("setblock 0 {DY} 6 minecraft:rail[shape=ascending_east] strict"))?;
    srv.cmd_ok("data merge entity @e[tag=slope,limit=1] {Motion:[0.6d,0.0d,0.0d]}")?;
    for t in 1..=14 {
        srv.step(1)?;
        println!("  t{t} {}", row(&mut srv, "flat, moving", "flat")?);
        println!("  t{t} {}", row(&mut srv, "ascending, moving", "slope")?);
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
