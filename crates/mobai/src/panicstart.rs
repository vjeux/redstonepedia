//! (tickexact 2026-11-12, ironspec: PANIC's start erases an IDLE rider's entity walk) `mctas mobai
//! panic-start`: on the official 26.3 server, an IDLE villager seated in a cart on a glass floor
//! (day time 1000, frozen) with a second villager free on the walled floor; once the rider's brain
//! has an entity walk target or an interaction target (InteractWith, SetWalkTargetFromLookTarget),
//! a zombie in a boat comes into sight 5 blocks away; every tick around PANIC's first tick the
//! WALK_TARGET, PATH, LOOK_TARGET, INTERACTION_TARGET and NEAREST_HOSTILE lines logged during the
//! rider's own Brain.tick (set and erase, by which behaviour) and its activity switches. NOT the
//! writes the other villager's behaviours make to the rider's brain during ITS tick
//! (BehaviorUtils.setWalkAndLookTargetMemories writes both brains): the recorded scene
//! `panic-entity-walk` has the rider's memories at every tick's end (the neighbour sets its entity
//! walk again within PANIC). `--trials` repeats it (the zombie killed, the rider calm again, the
//! next one summoned when the next entity walk is set).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 64;
const MEMS: &str = "minecraft:walk_target,minecraft:path,minecraft:look_target,minecraft:interaction_target,minecraft:nearest_hostile,minecraft:cant_reach_walk_target_since";

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
        Ok(s[..whole].lines().filter_map(|l| l.split("mctas brain agent: ").nth(1).map(|r| r.trim().to_string())).filter(|l| !l.starts_with("Brain reports")).collect())
    }
}

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

pub fn panic_start(version: &str, trials: u32, wait_max: u32) -> Result<()> {
    let jar = mctas::bridge::brain_agent(version)?;
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 1000")?;
    srv.cmd_ok("forceload add -16 -16 15 15")?;
    srv.step(20)?;
    println!("== panic-start on the official {version} server: an IDLE rider (day time 1000) in a cart at (0.5,{Y},0.5), a second villager free on the walled glass floor; a zombie in a boat at (5.5,{Y},0.5) once the rider has an entity walk or interaction target");
    srv.cmd_ok(&format!("fill -6 {} -6 9 {} 6 minecraft:glass", Y - 1, Y - 1))?;
    srv.cmd_ok(&format!("fill -6 {Y} -6 9 {} 6 minecraft:air", Y + 4))?;
    // the floor walled round at its edge (2 high; the inside open: glass blocks sight, so no pen)
    srv.cmd_ok(&format!("fill -7 {} -7 10 {} 7 minecraft:glass hollow", Y - 1, Y + 1))?;
    srv.cmd_ok(&format!("fill -6 {Y} -6 9 {} 6 minecraft:air", Y + 1))?;
    srv.cmd_ok(&format!("setblock 0 {Y} 0 minecraft:rail"))?;
    srv.cmd_ok(&format!("summon minecraft:minecart 0.5 {}.0625 0.5 {{Tags:[\"pc\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"pr\"]}}]}}", Y))?;
    srv.cmd_ok(&format!("summon minecraft:villager 2.5 {Y} 0.5 {{PersistenceRequired:1b,Tags:[\"pb\"]}}"))?;
    let rider = uuid_of(&mut srv, "@e[tag=pr,limit=1]")?;
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    let mut t = 0u32;
    for trial in 0..trials {
        // wait for an entity walk or an interaction target
        let mut armed = None;
        let mut history: Vec<(u32, String)> = Vec::new();
        while t < 100_000 && armed.is_none() {
            srv.step(1)?;
            t += 1;
            for l in log.drain(&srv)? {
                if !l.ends_with(&format!("@{rider}")) && !l.contains(&format!(" @{rider}")) {
                    continue;
                }
                let text = l.split(" @").next().unwrap_or("").to_string();
                history.push((t, text.clone()));
                let entity_walk = text.starts_with("set minecraft:walk_target") && text.contains("EntityTracker");
                if entity_walk || text.starts_with("set minecraft:interaction_target") {
                    armed = Some((t, text));
                }
            }
            if t > wait_max * (trial + 1) + 2000 {
                break;
            }
        }
        let Some((ta, why)) = armed else {
            println!("  trial {trial}: no entity walk or interaction target in time; the rider's lines by kind:");
            let mut kinds: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
            for (_, text) in &history {
                let memory = text.split(" = ").next().unwrap_or(text).split(" (absent)").next().unwrap_or(text).to_string();
                let by = text.rsplit(" by ").next().unwrap_or("").split(".lambda").next().unwrap_or("").split(':').next().unwrap_or("").to_string();
                *kinds.entry(format!("{memory} by {by}")).or_default() += 1;
            }
            for (k, n) in &kinds {
                println!("    {n:6} {k}");
            }
            let sets: Vec<&(u32, String)> = history.iter().filter(|(_, t)| t.starts_with("set ")).collect();
            for (h, text) in sets.iter().skip(sets.len().saturating_sub(20)).map(|p| (&p.0, &p.1)) {
                println!("    t{h}: {text}");
            }
            break;
        };
        println!("  trial {trial}: armed at t{ta}: {why}");
        for (h, text) in history.iter().filter(|(h, _)| *h + 5 >= ta) {
            println!("    t{h}: {text}");
        }
        srv.cmd_ok(&format!("summon minecraft:oak_boat 5.5 {Y} 0.5 {{Tags:[\"pz\"],Passengers:[{{id:\"minecraft:zombie\",PersistenceRequired:1b,Tags:[\"pzz\"]}}]}}"))?;
        println!("    t{t}: the zombie summoned in its boat at (5.5,{Y},0.5)");
        let mut panic_at = None;
        let mut after = 0;
        while after < 60 {
            srv.step(1)?;
            t += 1;
            for l in log.drain(&srv)? {
                if !l.contains(&format!("@{rider}")) {
                    continue;
                }
                let text = l.split(" @").next().unwrap_or("").to_string();
                if text.starts_with("activity panic") && panic_at.is_none() {
                    panic_at = Some(t);
                }
                println!("    t{t}{}: {text}", panic_at.map(|p| format!(" (panic +{})", t - p)).unwrap_or_default());
            }
            if panic_at.is_some() {
                after += 1;
            } else if t > ta + 200 {
                println!("    no panic within 200 gt");
                break;
            }
        }
        srv.cmd("kill @e[tag=pzz]")?;
        srv.cmd("kill @e[tag=pz]")?;
        // calm again before the next trial
        srv.step(600)?;
        t += 600;
        log.drain(&srv)?;
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
