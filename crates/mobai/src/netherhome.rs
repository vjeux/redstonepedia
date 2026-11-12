//! (tickexact 2026-11-12, ironspec's design-space verify list, item 3) `mctas mobai nether-home`:
//! on the official 26.3 server, does a villager in the Nether whose HOME is the overworld's bed
//! lose that HOME? The code says a calm REST tick there releases it (`SetWalkTargetFromBlockMemory`
//! erases a memory whose dimension isn't the villager's, and the villager schedule runs on the
//! overworld clock everywhere), and a panicking villager keeps it (REST behaviours don't run in
//! PANIC). Every villager holds the overworld bed as HOME.
//! - Residents, placed by day (time 6000): N1 calm in the Nether, N2 in the Nether panicking at a
//!   zombie 5 blocks off (the zombie in a boat), O1 calm in the overworld 6 blocks from the bed (the
//!   control). Read after 40 ticks of day, then the clock is set to night (14000) and read again.
//! - Crossings, each riding a minecart placed in an overworld portal: N3 at night, N4 by day (then
//!   kept in the Nether into the night).

use anyhow::{Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const BED: [i32; 3] = [0, 64, 0];

fn home_of(srv: &mut VanillaServer, tag: &str) -> Result<String> {
    let out = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Brain.memories"))?;
    if !out.contains("has the following entity data") {
        return Ok("(not found)".into());
    }
    Ok(match out.find("\"minecraft:home\"") {
        Some(i) => {
            let h = &out[i..];
            let dim = h.split("dimension: \"").nth(1).and_then(|s| s.split('"').next()).unwrap_or("?").to_string();
            format!("HOME {dim}")
        }
        None => "no HOME".into(),
    })
}

fn in_nether(srv: &mut VanillaServer, tag: &str) -> Result<bool> {
    let out = srv.cmd_long(&format!("execute in minecraft:the_nether if entity @e[tag={tag},distance=0..]"))?;
    Ok(out.contains("passed"))
}

fn brain(dim: &str) -> String {
    format!("Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"{dim}\",pos:[I;{},{},{}]}}}}}}}}", BED[0], BED[1], BED[2])
}

fn read(srv: &mut VanillaServer, label: &str, tags: &[&str]) -> Result<()> {
    let mut line = format!("{label}:");
    for tag in tags {
        let h = home_of(srv, tag)?;
        line.push_str(&format!("  {tag} {h}"));
    }
    println!("{line}");
    Ok(())
}

/// A minecart with a villager (tag, HOME the overworld bed) placed in the overworld portal; steps
/// until the villager is in the Nether, then reads its HOME at +0, +1, +5, +20 and +100.
fn cross(srv: &mut VanillaServer, tag: &str) -> Result<()> {
    srv.cmd_ok(&format!(
        "summon minecraft:minecart 20.5 64 0.5 {{Passengers:[{{id:\"minecraft:villager\",Tags:[\"{tag}\"],PersistenceRequired:1b,{}}}]}}",
        brain("minecraft:overworld")
    ))?;
    println!("{tag} before the crossing: {}", home_of(srv, tag)?);
    for k in 1..=200u32 {
        srv.step(1)?;
        if in_nether(srv, tag)? {
            println!("{tag} in the Nether at +{k}: {}", home_of(srv, tag)?);
            let mut t = 0;
            for at in [1u32, 5, 20, 100] {
                srv.step(at - t)?;
                t = at;
                println!("{tag} +{at} after the crossing: {}", home_of(srv, tag)?);
            }
            return Ok(());
        }
    }
    bail!("{tag} never crossed into the Nether in 200 ticks")
}

pub fn nether_home(version: &str) -> Result<()> {
    println!("== nether-home on the official {version} server: villagers holding the overworld bed {BED:?} as HOME");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        keep_dir: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 6000")?;
    // the overworld: a floor, the bed, and a portal (x 20..21, z 0, y 64..66)
    srv.cmd_ok("forceload add -16 -16 31 15")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -16 -16 31 15")?;
    srv.step(40)?;
    srv.cmd_ok("fill -16 63 -16 31 63 15 minecraft:stone")?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=north,part=foot]", BED[0], BED[1], BED[2] + 1))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=north,part=head]", BED[0], BED[1], BED[2]))?;
    srv.cmd_ok("fill 19 63 0 22 63 0 minecraft:obsidian")?;
    srv.cmd_ok("fill 19 67 0 22 67 0 minecraft:obsidian")?;
    srv.cmd_ok("fill 19 64 0 19 66 0 minecraft:obsidian")?;
    srv.cmd_ok("fill 22 64 0 22 66 0 minecraft:obsidian")?;
    srv.cmd_ok("fill 20 64 0 21 66 0 minecraft:nether_portal[axis=x]")?;
    // the Nether: a cleared box with a floor; the zombie first, then N2 5 blocks from it and N1
    // 13.9 from it
    let out = srv.cmd_long("execute in minecraft:the_nether run fill -8 69 -10 10 75 10 minecraft:air")?;
    println!("nether clear: {}", out.trim());
    srv.cmd_ok("execute in minecraft:the_nether run fill -8 68 -10 10 68 10 minecraft:obsidian")?;
    srv.cmd_ok("execute in minecraft:the_nether run summon minecraft:oak_boat 5.5 69 6.5 {Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}")?;
    srv.step(5)?;
    let out = srv.cmd_long(&format!("execute in minecraft:the_nether run summon minecraft:villager 0.5 69 6.5 {{Tags:[\"n2\"],PersistenceRequired:1b,{}}}", brain("minecraft:overworld")))?;
    println!("n2 summon: {}", out.trim());
    srv.cmd_ok(&format!("execute in minecraft:the_nether run summon minecraft:villager 0.5 69 -6.5 {{Tags:[\"n1\"],PersistenceRequired:1b,{}}}", brain("minecraft:overworld")))?;
    srv.cmd_ok(&format!("summon minecraft:villager 0.5 64 -6.5 {{Tags:[\"o1\"],PersistenceRequired:1b,{}}}", brain("minecraft:overworld")))?;
    let tags = ["n1", "n2", "o1"];
    for tag in ["n1", "n2"] {
        let a = srv.cmd_long(&format!("execute in minecraft:the_nether if entity @e[tag={tag},distance=0..]"))?;
        let b = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?;
        println!("{tag} probe: {} | {}", a.trim(), b.trim());
    }
    read(&mut srv, "placed by day, t 0", &tags)?;
    srv.step(40)?;
    read(&mut srv, "day, t 40", &tags)?;
    srv.cmd_ok("time set 14000")?;
    let mut t = 0;
    for at in [1u32, 5, 20, 100] {
        srv.step(at - t)?;
        t = at;
        read(&mut srv, &format!("night (time set 14000) +{at}"), &tags)?;
    }
    println!("-- crossings");
    cross(&mut srv, "n3")?;
    // where the crossings arrive: a zombie in a boat 4 blocks from that spot, then N5 crosses at
    // night with the zombie in sight of its arrival
    let pos = srv.cmd_long("data get entity @e[tag=n3,limit=1] Pos")?;
    let v: Vec<f64> = pos.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|s| s.parse().ok()).collect();
    if v.len() >= 3 {
        let (x, y, z) = (v[v.len() - 3], v[v.len() - 2], v[v.len() - 1]);
        println!("the crossings arrive at {x:.2} {y:.2} {z:.2} in the Nether");
        let out = srv.cmd_long(&format!(
            "execute in minecraft:the_nether run summon minecraft:oak_boat {:.1} {:.1} {:.1} {{Passengers:[{{id:\"minecraft:zombie\",PersistenceRequired:1b,Tags:[\"z5\"]}}]}}",
            x,
            y,
            z + 4.0
        ))?;
        println!("zombie for n5: {}", out.trim());
        srv.step(5)?;
        cross(&mut srv, "n5")?;
    }
    srv.cmd_ok("time set 6000")?;
    srv.step(2)?;
    cross(&mut srv, "n4")?;
    srv.cmd_ok("time set 14000")?;
    let mut t = 0;
    for at in [1u32, 5, 20, 100] {
        srv.step(at - t)?;
        t = at;
        read(&mut srv, &format!("n4 kept in the Nether, night (time set 14000) +{at}"), &["n4"])?;
    }
    Ok(())
}
