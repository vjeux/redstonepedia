//! (tickexact 2026-11-12, ironspec's design-space verify list: the T+600 lock) `mctas mobai t600`:
//! on the official 26.3 server with the golem agent (each golem tagged with its summoner), a batch
//! of 3 villagers seated in minecarts, kept panicking by a zombie in a boat 5–7 blocks off, LAST_SLEPT
//! set: does the batch that summoned at T summon again at exactly T + 600? Each golem lives until
//! T + 2 (killed then, as a portal pad sends it off at T + 2). Each member's GOLEM_DETECTED_RECENTLY
//! ttl is printed at T + 1, T + 598, T + 599 and T + 600. Cases:
//! - (i) every member can summon (the first in tick order marks the others at T); 36 watchers (no
//!   LAST_SLEPT, outside the 10.6 box, within reach of the golems) count the re-marks the golems'
//!   2-tick dwell gives through the 20-gt living scan and the 200-gt golem sensor;
//! - (ii) the first member in tick order can't summon (no spawnable floor within its ±8), so a later
//!   member marks it after its own tick at T;
//! - (iii) a mixed batch: one member's lock preset as if marked at T, the two others' as if marked
//!   at T − 100.

use anyhow::{Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};

fn uuid_str(k: u32) -> String {
    format!("00000000-0000-0000-0000-{k:012x}")
}

/// The member's GOLEM_DETECTED_RECENTLY ttl (None: absent).
fn ttl(srv: &mut VanillaServer, tag: &str) -> Result<Option<i64>> {
    let out = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Brain.memories.\"minecraft:golem_detected_recently\""))?;
    Ok(out.split("ttl: ").nth(1).and_then(|s| s.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect::<String>().parse().ok()))
}

/// The summoner (member number k) of the golem that exists now, if any; the golem stays.
fn golem(srv: &mut VanillaServer) -> Result<Option<String>> {
    let out = srv.cmd_long("data get entity @e[type=minecraft:iron_golem,limit=1] Tags")?;
    if !out.contains("has the following entity data") {
        return Ok(None);
    }
    Ok(Some(match out.split("mctas.by.").nth(1) {
        Some(s) => {
            let u: String = s.chars().take(36).collect();
            match (1..=99).find(|k| uuid_str(*k) == u) {
                Some(k) => format!("m{k}"),
                None => format!("an untracked villager {u}"),
            }
        }
        None => "an untagged golem".into(),
    }))
}

fn fmt_ttl(v: Option<i64>) -> String {
    v.map(|t| format!("ttl {t}")).unwrap_or_else(|| "free".into())
}

/// A villager seated in a minecart on a rail at (x, 64, z) over a stone at y 63.
#[allow(clippy::too_many_arguments)]
fn seat(srv: &mut VanillaServer, x: i32, z: i32, tag: &str, k: u32, last_slept: Option<i64>, golem_ttl: Option<i64>) -> Result<()> {
    // (the stone may already be there: a no-change setblock reports failure)
    let _ = srv.cmd(&format!("setblock {x} 63 {z} minecraft:stone"))?;
    srv.cmd_ok(&format!("setblock {x} 64 {z} minecraft:rail"))?;
    let mut mem = Vec::new();
    if let Some(t) = last_slept {
        mem.push(format!("\"minecraft:last_slept\":{{value:{t}L}}"));
    }
    if let Some(t) = golem_ttl {
        mem.push(format!("\"minecraft:golem_detected_recently\":{{value:1b,ttl:{t}L}}"));
    }
    let u = [0, 0, 0, k as i32];
    // (the cell's centre as a number: "{x}.5" read -13.5 for x = -13, the centre of the next cell)
    let (cx, cz) = (x as f64 + 0.5, z as f64 + 0.5);
    srv.cmd_ok(&format!(
        "summon minecraft:minecart {cx} 64.0625 {cz} {{Passengers:[{{id:\"minecraft:villager\",Tags:[\"{tag}\"],UUID:[I;{},{},{},{}],PersistenceRequired:1b,Brain:{{memories:{{{}}}}}}}]}}",
        u[0],
        u[1],
        u[2],
        u[3],
        mem.join(",")
    ))?;
    Ok(())
}

/// A zombie in a boat at (x + 0.5, 64, z + 0.5) over a stone, under a stone roof 3 above (no sky:
/// a zombie burns in daylight).
fn zombie(srv: &mut VanillaServer, x: i32, z: i32) -> Result<()> {
    let _ = srv.cmd(&format!("setblock {x} 63 {z} minecraft:stone"))?;
    let _ = srv.cmd(&format!("setblock {x} 67 {z} minecraft:stone"))?;
    srv.cmd_ok(&format!("summon minecraft:oak_boat {} 64 {} {{Passengers:[{{id:\"minecraft:zombie\",PersistenceRequired:1b}}]}}", x as f64 + 0.5, z as f64 + 0.5))?;
    Ok(())
}

/// Steps to the next game time that is a multiple of 100 and returns it.
fn to_next_summon_tick(srv: &mut VanillaServer) -> Result<i64> {
    let now = srv.game_time()?;
    let t = (now / 100 + 1) * 100;
    srv.step((t - now) as u32)?;
    Ok(t)
}

/// The case's summons: from now, every summon tick until two golems or `limit` summon ticks;
/// prints each golem's summoner (killed at T + 2) and the members' ttl around T + 600.
fn run_case(srv: &mut VanillaServer, members: &[&str], watchers: &[String], limit: u32) -> Result<()> {
    let mut first: Option<i64> = None;
    for _ in 0..limit {
        let t = to_next_summon_tick(srv)?;
        let g = golem(srv)?;
        if g.is_none() && first.is_some() {
            let golems = srv.cmd_long("execute if entity @e[type=minecraft:iron_golem]")?;
            let zombies = srv.cmd_long("execute if entity @e[type=minecraft:zombie]")?;
            let p = srv.cmd_long(&format!("data get entity @e[tag={},limit=1] Pos", members[0]))?;
            let v = srv.cmd_long(&format!("execute if entity @e[tag={},predicate=!minecraft:none]", members[0]))?;
            let _ = v;
            println!("    probe: golems {} | zombies {} | {} {}", golems.trim(), zombies.trim(), members[0], p.trim());
        }
        let mut line = format!("  t {t}{}: ", first.map(|t0| format!(" (T+{})", t - t0)).unwrap_or_default());
        line.push_str(&match &g {
            Some(s) => format!("golem summoned by {s}"),
            None => "no golem".into(),
        });
        for m in members {
            line.push_str(&format!("  {m} {}", fmt_ttl(ttl(srv, m)?)));
        }
        println!("{line}");
        if g.is_some() {
            srv.step(2)?;
            srv.cmd_ok("kill @e[type=minecraft:iron_golem]")?;
            if first.is_none() {
                first = Some(t);
                let mut l1 = format!("  T+2 (golem killed):");
                for m in members {
                    l1.push_str(&format!("  {m} {}", fmt_ttl(ttl(srv, m)?)));
                }
                println!("{l1}");
                if !watchers.is_empty() {
                    // the watchers' re-marks over the next 25 gt (a lock set after T)
                    srv.step(23)?;
                    let mut marked = Vec::new();
                    for w in watchers {
                        if let Some(v) = ttl(srv, w)? {
                            marked.push(format!("{w} ttl {v}"));
                        }
                    }
                    println!("  T+25: {} of {} watchers marked by the golem's dwell: {}", marked.len(), watchers.len(), marked.join(", "));
                }
                // the members at T + 598 and T + 599
                let now = srv.game_time()?;
                srv.step((t + 598 - now) as u32)?;
                for at in [598, 599] {
                    if at == 599 {
                        srv.step(1)?;
                    }
                    let mut l = format!("  T+{at}:");
                    for m in members {
                        l.push_str(&format!("  {m} {}", fmt_ttl(ttl(srv, m)?)));
                    }
                    println!("{l}");
                }
            } else {
                return Ok(());
            }
        }
    }
    if first.is_none() {
        bail!("no golem in {limit} summon ticks");
    }
    Ok(())
}

pub fn t600(version: &str) -> Result<()> {
    println!("== t600 on the official {version} server (golem agent): a batch of 3 seated villagers, panicking, LAST_SLEPT set");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        keep_dir: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        java_agent: Some(mctas::bridge::golem_agent(version)?),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 6000")?;
    // (i) at x 0: the members at x 0, 2, 4 (z 0) over a floor x -6..12, z -8..8; the zombie at
    // (2.5, 5.5); 36 watchers on the lines z -13 and z 14 (x -6..12 by 2: 10 each) and x -11 and
    // x 17 (z -6..10 by 2: 8 each), each over its own stone
    srv.cmd_ok("forceload add -32 -32 47 31")?;
    srv.step(20)?;
    srv.cmd_ok("fill -6 63 -8 12 63 8 minecraft:stone")?;
    zombie(&mut srv, 2, 5)?;
    let now = srv.game_time()?;
    for (k, x) in [(1u32, 0), (2, 2), (3, 4)] {
        seat(&mut srv, x, 0, &format!("m{k}"), k, Some(now), None)?;
    }
    let mut watchers = Vec::new();
    let mut k = 10u32;
    for z in [-13, 14] {
        for x in (-6..=12).step_by(2) {
            seat(&mut srv, x, z, &format!("w{k}"), k, None, None)?;
            watchers.push(format!("w{k}"));
            k += 1;
        }
    }
    for x in [-11, 17] {
        for z in (-6..=10).step_by(2) {
            seat(&mut srv, x, z, &format!("w{k}"), k, None, None)?;
            watchers.push(format!("w{k}"));
            k += 1;
        }
    }
    println!("(i) every member can summon; m1 ticks first (summoned first)");
    run_case(&mut srv, &["m1", "m2", "m3"], &watchers, 12)?;
    srv.cmd_ok("kill @e[type=!minecraft:player]")?;
    srv.step(2)?;
    // (ii) at x 200: m4 at x 189 with no floor within its ±8 (its own stone is under its cart), m5
    // and m6 at x 199 and 201 over a floor x 198..208; the zombie at (195.5, 4.5) on its own stone
    srv.cmd_ok("forceload add 170 -32 239 31")?;
    srv.step(20)?;
    srv.cmd_ok("fill 198 63 -8 208 63 8 minecraft:stone")?;
    zombie(&mut srv, 195, 4)?;
    let now = srv.game_time()?;
    seat(&mut srv, 189, 0, "m4", 4, Some(now), None)?;
    seat(&mut srv, 199, 0, "m5", 5, Some(now), None)?;
    seat(&mut srv, 201, 0, "m6", 6, Some(now), None)?;
    println!("(ii) m4 ticks first but can't summon (no floor within its ±8); m5 marks");
    run_case(&mut srv, &["m4", "m5", "m6"], &[], 12)?;
    srv.cmd_ok("kill @e[type=!minecraft:player]")?;
    srv.step(2)?;
    // (iii) at x 400: summoned 1 tick before a summon tick T0 with m7's lock preset to ttl 600 (as
    // if marked at T0) and m8's and m9's to ttl 500 (as if marked at T0 - 100)
    srv.cmd_ok("forceload add 370 -32 439 31")?;
    srv.step(20)?;
    srv.cmd_ok("fill 394 63 -8 412 63 8 minecraft:stone")?;
    zombie(&mut srv, 402, 5)?;
    let now = srv.game_time()?;
    let t0 = (now / 100 + 2) * 100;
    srv.step((t0 - 1 - now) as u32)?;
    let now = srv.game_time()?;
    seat(&mut srv, 400, 0, "m7", 7, Some(now), Some(600))?;
    seat(&mut srv, 402, 0, "m8", 8, Some(now), Some(500))?;
    seat(&mut srv, 404, 0, "m9", 9, Some(now), Some(500))?;
    println!("(iii) at T0 = {t0}: m7 locked as if marked at T0, m8 and m9 as if marked at T0 - 100");
    for at in [0i64, 499, 500, 598, 599, 600] {
        let now = srv.game_time()?;
        if t0 + at > now {
            srv.step((t0 + at - now) as u32)?;
        }
        let g = golem(&mut srv)?;
        println!(
            "  T0+{at}: {}  m7 {}  m8 {}  m9 {}",
            g.map(|s| format!("golem by {s}")).unwrap_or_else(|| "no golem".into()),
            fmt_ttl(ttl(&mut srv, "m7")?),
            fmt_ttl(ttl(&mut srv, "m8")?),
            fmt_ttl(ttl(&mut srv, "m9")?)
        );
    }
    Ok(())
}
