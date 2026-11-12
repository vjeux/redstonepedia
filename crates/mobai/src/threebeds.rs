//! (tickexact 2026-11-12, ironspec's 3-bed check: vjeux allows up to 3 beds) `mctas mobai three-beds`:
//! on the official 26.3 server, three beds side by side (heads at x 0, 3 and 6, z 0, facing north),
//! one napper per bed on foot beside it, and three calm riders seated in minecarts at x 18 (z -4, 0,
//! 4): VA holds bed A as HOME, 18.4 from A and 15.5 from bed B; VB holds B, 15.0 from it; VC holds C,
//! 12.6 from it. At night all three nappers sleep at once. `ValidateNearbyPoi` reads only a
//! villager's own HOME bed, so VA keeps its HOME although it is within 16 of occupied B, while VB
//! and VC (the positive controls) lose theirs. Then a real save, stop and start at night (the
//! calm window after a reload) and the same reads.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

fn brain_home(x: i32) -> String {
    format!("Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{x},64,0]}}}}}}}}")
}

fn state(srv: &mut VanillaServer, tag: &str) -> Result<String> {
    let mem = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Brain.memories"))?;
    if !mem.contains("has the following entity data") {
        return Ok("(not found)".into());
    }
    let home = match mem.find("\"minecraft:home\"") {
        Some(i) => {
            let h = &mem[i..];
            let p = h.split("pos: [I; ").nth(1).and_then(|s| s.split(']').next()).unwrap_or("?").to_string();
            format!("HOME [{p}]")
        }
        None => "no HOME".into(),
    };
    let sleeping = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] sleeping_pos"))?;
    let asleep = sleeping.contains("has the following entity data");
    Ok(format!("{home}{}", if asleep { " asleep" } else { "" }))
}

fn read(srv: &mut VanillaServer, label: &str) -> Result<()> {
    let mut line = format!("{label}:");
    for tag in ["nA", "nB", "nC", "VA", "VB", "VC"] {
        line.push_str(&format!("  {tag} {}", state(srv, tag)?));
    }
    println!("{line}");
    Ok(())
}

pub fn three_beds(version: &str) -> Result<()> {
    println!("== three-beds on the official {version} server: beds A, B, C (heads x 0, 3, 6), a napper each, calm riders VA (HOME A), VB (HOME B), VC (HOME C) at x 18");
    let start = |world: Option<std::path::PathBuf>| {
        VanillaServer::start(&VanillaOptions {
            version: version.to_string(),
            difficulty: "normal".into(),
            with_bot: false,
            keep_dir: true,
            biome: "minecraft:plains".into(),
            tick_rate: 1000,
            world,
            ..Default::default()
        })
    };
    let mut srv = start(None)?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 31 31")?;
    srv.step(20)?;
    srv.cmd_ok("time set 6000")?;
    srv.cmd_ok("fill -8 63 -8 24 63 8 minecraft:stone")?;
    for (bed, x) in [("A", 0), ("B", 3), ("C", 6)] {
        srv.cmd_ok(&format!("setblock {x} 64 1 minecraft:red_bed[facing=north,part=foot]"))?;
        srv.cmd_ok(&format!("setblock {x} 64 0 minecraft:red_bed[facing=north,part=head]"))?;
        srv.cmd_ok(&format!("summon minecraft:villager {x}.5 64 -1.5 {{Tags:[\"n{bed}\"],PersistenceRequired:1b,{}}}", brain_home(x)))?;
    }
    for (tag, z, x) in [("VA", -4, 0), ("VB", 0, 3), ("VC", 4, 6)] {
        srv.cmd_ok(&format!("setblock 18 64 {z} minecraft:rail"))?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart 18.5 64.0625 {z}.5 {{Passengers:[{{id:\"minecraft:villager\",Tags:[\"{tag}\"],PersistenceRequired:1b,{}}}]}}",
            brain_home(x)
        ))?;
    }
    read(&mut srv, "day, placed")?;
    srv.cmd_ok("time set 12500")?;
    for at in [100u32, 300, 600] {
        srv.step(if at == 100 { 100 } else { at - [100u32, 300, 600][[100u32, 300, 600].iter().position(|v| *v == at).unwrap() - 1] })?;
        read(&mut srv, &format!("night (time set 12500) +{at}"))?;
    }
    // a real save, stop and start while the nappers sleep
    srv.cmd_ok("save-on")?;
    srv.cmd_ok("save-all flush")?;
    let old = srv.dir.clone();
    let world = old.join(&srv.level_name);
    drop(srv);
    let mut srv = start(Some(world))?;
    let _ = std::fs::remove_dir_all(&old);
    srv.cmd_ok("gamerule advance_time true")?;
    read(&mut srv, "after a save, stop and start at night")?;
    for at in [20u32, 60, 200] {
        let prev = match at {
            20 => 0,
            60 => 20,
            _ => 60,
        };
        srv.step(at - prev)?;
        read(&mut srv, &format!("after the restart +{at}"))?;
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
