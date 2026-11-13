//! (tickexact 2026-11-13, ironspec 00:00 / 00:30 PT: TTL memories across a world open) `mctas mobai
//! golem-ttl`: on the official 26.3 server, one golem stand as golem-pad lays it (a glass floor, three
//! villagers in carts, their last sleep recent, panicking at a husk in a boat 5 away; a 5 x 3
//! cobblestone pad, the only cells a golem can spawn on; every golem killed on its 2nd tick). The
//! first golem's tick is T (a panic summon: game time % 100 == 0). With a restart, the world is
//! saved, stopped and started at T + `--restart-after` (restartcheck::reload); each villager's
//! GOLEM_DETECTED_RECENTLY (value and remaining ttl) is printed before the save and after the load,
//! and the next golem's tick relative to T. From T + 500 every slot is printed: the trio's memories
//! at the tick before it, and whether a golem came or the summon found no spot (the brain agent's
//! `no spot by` line from Villager.spawnGolemIfNeeded's trySpawnMob). At T the agent's
//! GOLEM_DETECTED_RECENTLY sets are printed in their order, each with its villager (v0..v2) and its
//! caller. `--boundary m`: at T + 100 the trio's memories are set (data merge) so that each clears
//! m ticks before the slot S = T + 400, and the restart (if any) comes at T + 200, inside the window.
//! (26.3: MemorySlot.tick counts a ttl down once per brain tick; Brain.pack saves the remaining ttl;
//! a world open's idle ticks run no brain.)

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 64;
const MEMS: &str = "minecraft:golem_detected_recently";

fn opts(version: &str, jar: &std::path::Path, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        tick_rate: 1000,
        keep_dir: true,
        world,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    }
}

struct Log {
    at: u64,
}

impl Log {
    fn drain(&mut self, srv: &VanillaServer) -> Result<Vec<String>> {
        let mut f = std::fs::File::open(&srv.log)?;
        f.seek(SeekFrom::Start(self.at))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let whole = s.rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.at += whole as u64;
        Ok(s[..whole].lines().filter_map(|l| l.split("mctas brain agent: ").nth(1).map(|r| r.trim().to_string())).collect())
    }
}

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

fn golems(srv: &mut VanillaServer) -> Result<usize> {
    Ok(srv.cmd_long("execute as @e[type=minecraft:iron_golem,tag=!gt_old] run data get entity @s Pos")?.matches("entity data: ").count())
}

fn memories(srv: &mut VanillaServer) -> Result<String> {
    let mut out = Vec::new();
    for i in 0..3 {
        let s = srv.cmd_long(&format!("data get entity @e[tag=gtv{i},limit=1] Brain.memories.\"minecraft:golem_detected_recently\""))?;
        out.push(format!("v{i} {}", s.split("entity data: ").nth(1).map(|r| r.trim().to_string()).unwrap_or_else(|| "none".into())));
    }
    Ok(out.join("; "))
}

/// A brain-agent line with its villager named (v0..v2) instead of its UUID.
fn named(l: &str, uuids: &[String]) -> String {
    let mut s = l.to_string();
    for (i, u) in uuids.iter().enumerate() {
        s = s.replace(&format!("@{u}"), &format!("[v{i}]")).replace(u.as_str(), &format!("v{i}"));
    }
    s
}

/// One run: the first golem's game time T, the next golem's offset from T (None if none came), the
/// notes.
fn run(version: &str, jar: &std::path::Path, restart_after: Option<u32>, boundary: Option<u32>, ticks: u32) -> Result<(Option<i64>, Option<i64>, Vec<String>)> {
    let mut srv = VanillaServer::start(&opts(version, jar, None))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 1000")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 16 24")?;
    srv.step(200)?;
    srv.cmd_ok(&format!("fill -10 {} -10 10 {} 14 minecraft:glass", Y - 1, Y - 1))?;
    // the pad (golem-pad's): 5 x 3 cobblestone on the glass, the only cells a golem can spawn on
    // (LEGACY_IRON_GOLEM never spawns on glass)
    srv.cmd_ok(&format!("fill 2 {Y} 0 4 {Y} 4 minecraft:cobblestone"))?;
    let mut uuids = Vec::new();
    for (i, dz) in [0, 2, 4].iter().enumerate() {
        srv.cmd(&format!("setblock 0 {Y} {dz} minecraft:rail"))?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart 0.5 {}.0625 {dz}.5 {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"gtv{i}\"],Brain:{{memories:{{\"minecraft:last_slept\":{{value:900L}}}}}}}}]}}",
            Y
        ))?;
        uuids.push(uuid_of(&mut srv, &format!("@e[tag=gtv{i},limit=1]"))?);
    }
    srv.cmd_ok(&format!("summon minecraft:oak_boat -4.5 {Y} 2.5 {{Passengers:[{{id:\"minecraft:husk\",PersistenceRequired:1b}}]}}"))?;
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    let mut notes = Vec::new();
    let (mut first, mut next) = (None::<i64>, None::<i64>);
    let mut restarted = false;
    let restart_at = if boundary.is_some() { restart_after.map(|_| 200) } else { restart_after };
    for _ in 0..ticks {
        let gt = srv.game_time()?;
        if let Some(t0) = first {
            // the boundary row's memories, set at T + 100 so each clears m ticks before T + 400
            if let Some(m) = boundary
                && gt == t0 + 100
            {
                let l = 400 - m as i64 - 100 - 1;
                for i in 0..3 {
                    srv.cmd_ok(&format!("data merge entity @e[tag=gtv{i},limit=1] {{Brain:{{memories:{{\"minecraft:golem_detected_recently\":{{value:1b,ttl:{l}L}}}}}}}}"))?;
                }
                notes.push(format!("T + 100: the trio's memories set to ttl {l} (each clears at its brain tick T + {}, {m} before the slot T + 400): {}", 100 + l + 1, memories(&mut srv)?));
            }
            if let Some(r) = restart_at
                && !restarted
                && gt == t0 + r as i64
            {
                notes.push(format!("before the save (T + {r}): {}", memories(&mut srv)?));
                let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, jar, Some(w)), None)?;
                srv = s;
                log = Log { at: 0 };
                log.drain(&srv)?;
                srv.cmd_ok("forceload add -16 -16 16 24")?;
                notes.push(format!("the load: {}", rl.describe()));
                notes.push(format!("after the load (game time {}): {}", srv.game_time()?, memories(&mut srv)?));
                restarted = true;
            }
            // from T + 500 (or the boundary's slot), the trio's memories the tick before each slot
            let watch_from = if boundary.is_some() { t0 + 300 } else { t0 + 500 };
            if gt >= watch_from && (gt + 1).rem_euclid(100) == 0 && next.is_none() {
                notes.push(format!("slot T + {}: the tick before it: {}", gt + 1 - t0, memories(&mut srv)?));
            }
        }
        srv.step(1)?;
        let t = srv.game_time()?;
        let lines = log.drain(&srv)?;
        srv.cmd("kill @e[type=minecraft:iron_golem,tag=gt_old]")?;
        let born = golems(&mut srv)? > 0;
        if let Some(t0) = first
            && next.is_none()
            && t.rem_euclid(100) == 0
            && t > t0
        {
            let spot: Vec<String> = lines.iter().filter(|l| l.starts_with("no spot")).map(|l| named(l, &uuids)).collect();
            notes.push(format!("slot T + {}: {}", t - t0, if born { "a golem".to_string() } else if spot.is_empty() { "no golem, no summon call failed for a spot (no call)".to_string() } else { format!("no golem: {}", spot.join(", ")) }));
        }
        if born {
            match first {
                None => {
                    first = Some(t);
                    notes.push(format!("the first golem at game time {t} (% 100 = {})", t.rem_euclid(100)));
                    let sets: Vec<String> = lines.iter().filter(|l| l.contains("golem_detected_recently")).map(|l| named(l, &uuids)).collect();
                    notes.push(format!("T: the agent's lines in order: {}", sets.join(" | ")));
                    notes.push(format!("T: memories after the tick: {}", memories(&mut srv)?));
                }
                Some(t0) if next.is_none() => {
                    next = Some(t - t0);
                    srv.cmd("tag @e[type=minecraft:iron_golem,tag=!gt_old] add gt_old")?;
                    break;
                }
                _ => {}
            }
            srv.cmd("tag @e[type=minecraft:iron_golem,tag=!gt_old] add gt_old")?;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok((first, next, notes))
}

pub fn golem_ttl(version: &str, restart_after: u32, runs: u32, ticks: u32, boundary: Option<u32>) -> Result<()> {
    println!(
        "== golem-ttl on the official {version} server: one golem stand (a glass floor, three panicking villagers in carts, a husk in a boat 5 away, a 5 x 3 cobblestone pad; every golem killed on its 2nd tick); T the first golem's game time; {}; the next golem's tick relative to T; {runs} run(s) each",
        match boundary {
            Some(m) => format!("BOUNDARY rows: the trio's memories set at T + 100 to clear {m} tick(s) before the slot T + 400; with a restart at T + 200 (inside the window) and without"),
            None => format!("with a restart at T + {restart_after} (save, stop, start) and without"),
        }
    );
    let jar = mctas::bridge::brain_agent(version)?;
    let mut summary = Vec::new();
    let mut not_judged = 0u32;
    for (label, restart) in [("control (no restart)", None), ("restart", Some(restart_after))] {
        for k in 1..=runs {
            let (first, next, notes) = run(version, &jar, restart, boundary, ticks)?;
            println!(
                "  {label}, run {k}: the first golem at {}; the next golem at T + {}",
                first.map(|t| format!("game time {t}")).unwrap_or_else(|| "none".into()),
                next.map(|d| d.to_string()).unwrap_or_else(|| format!("- (none within {ticks} gt): NOT JUDGED (0 samples)"))
            );
            for n in notes {
                println!("    {n}");
            }
            summary.push(format!("{label} {}", next.map(|d| format!("T+{d}")).unwrap_or("-".into())));
            if next.is_none() {
                not_judged += 1;
            }
        }
    }
    println!("  next golem: {}", summary.join(", "));
    // (ironspec 23:56 PT: no vacuous passes)
    anyhow::ensure!(not_judged == 0, "{not_judged} row(s) NOT JUDGED (0 samples): the run fails");
    Ok(())
}
