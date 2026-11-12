//! (tickexact 2026-11-12, ironspec: the bed stop's merge) `mctas mobai day-jump`: on the official
//! 26.3 server with the brain agent, a villager with HOME on a bed walking to it at night along a
//! flat 1-wide glass corridor 14 blocks long, no hostiles; when it is `--at` blocks from the bed's
//! head, `/time set 0` (the day-time jump a night skip makes). Every tick from the jump to day time
//! `--until`: its activity, WALK_TARGET (cell or absent), PATH (MoveToTargetSink running: its path
//! is set while it runs), its position, and whether it sleeps. The question: does REST's walk
//! target survive the switch to IDLE at day time 10, and is it walked to the end?

use crate::seatclear::{Agent, MEMS};
use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn pos(srv: &mut VanillaServer) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long("data get entity @e[tag=dj,limit=1] Pos")?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1] - DY as f64, v[2]]))
}

pub fn day_jump(version: &str, at: f64, until: i64) -> Result<()> {
    let jar = mctas::bridge::brain_agent(version)?;
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        tick_rate: 1000,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 31 31")?;
    srv.step(20)?;
    // the path: a 1-wide glass corridor x 1, z 2..15, into a 3 x 3 room x -1..1, z -1..1 holding the
    // bed (its head at (0,0,0), its foot at (0,0,1), facing north): the corridor enters the room at
    // (1,0,1) beside the foot, and the head's east cell (1,0,0) is free, so the bed can be reached;
    // a stone floor, glass walls two high, a glass roof
    srv.cmd_ok(&format!("fill -3 {} -3 3 {} 17 minecraft:air", DY - 1, DY + 4))?;
    srv.cmd_ok(&format!("fill -2 {} -2 2 {} 16 minecraft:glass", DY - 1, DY + 2))?;
    srv.cmd_ok(&format!("fill -1 {} -1 1 {} 1 minecraft:stone", DY - 1, DY - 1))?;
    srv.cmd_ok(&format!("fill 1 {} 2 1 {} 15 minecraft:stone", DY - 1, DY - 1))?;
    srv.cmd_ok(&format!("fill -1 {DY} -1 1 {} 1 minecraft:air", DY + 1))?;
    srv.cmd_ok(&format!("fill 1 {DY} 2 1 {} 15 minecraft:air", DY + 1))?;
    srv.cmd_ok(&format!("setblock 0 {DY} 1 minecraft:red_bed[facing=north,part=foot]"))?;
    srv.cmd_ok(&format!("setblock 0 {DY} 0 minecraft:red_bed[facing=north,part=head]"))?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("time set 13000")?;
    srv.cmd_ok(&format!(
        "summon minecraft:villager 1.5 {DY} 15.5 {{Tags:[\"dj\"],PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;0,{DY},0]}}}}}}}}}}"
    ))?;
    let mut agent = Agent { at: 0 };
    agent.drain(&srv)?;
    println!("== day-jump on the official {version} server: the bed (head (0,0,0), foot (0,0,1)) in a 3 x 3 room, the villager from (1.5,0,15.5) down a 1-wide corridor (x 1) into the room beside the bed; /time set 0 when it is {at} from the bed (the nearer cell's centre); every tick to day time {until}");
    // walk until `at` from the head (its feet's distance to the head cell's centre, horizontally)
    let mut walked = 0;
    loop {
        srv.step(1)?;
        walked += 1;
        let lines = agent.drain(&srv)?;
        for l in &lines {
            if walked <= 3 || l.contains("walk_target") || l.starts_with("activity") {
                println!("  walk t{walked}: {}", l.split(" @").next().unwrap_or(l));
            }
        }
        let p = pos(&mut srv)?.unwrap_or([0.0, 0.0, 99.0]);
        // the distance to the bed: to the nearer of its two cells' centres (the foot (0.5, 1.5))
        let d = ((p[0] - 0.5).powi(2) + (p[2] - 1.5).powi(2)).sqrt().min(((p[0] - 0.5).powi(2) + (p[2] - 0.5).powi(2)).sqrt());
        if d <= at {
            println!("  t{walked}: at ({:.3},{:.3},{:.3}), {d:.2} from the bed: /time set 0", p[0], p[1], p[2]);
            break;
        }
        if walked > 1200 {
            println!("  it never came within {at} of the bed in 1,200 gt (at ({:.3},{:.3},{:.3}))", p[0], p[1], p[2]);
            let _ = std::fs::remove_dir_all(&srv.dir);
            return Ok(());
        }
    }
    srv.cmd_ok("time set 0")?;
    let gt_jump = srv.game_time()?;
    let mut walk_target = String::from("(as before the jump)");
    let mut path = String::from("(as before the jump)");
    let mut activity = String::from("rest");
    loop {
        srv.step(1)?;
        // the day time: game time since the jump (the jump set it to 0; the clock runs)
        let day = srv.game_time()? - gt_jump;
        let mut notes = Vec::new();
        for l in agent.drain(&srv)? {
            let l = l.split(" @").next().unwrap_or(&l).to_string();
            if let Some(a) = l.strip_prefix("activity ") {
                activity = a.split(' ').next().unwrap_or("").to_string();
                notes.push(l.clone());
            } else if l.starts_with("set minecraft:walk_target") {
                walk_target = l.split(" = ").nth(1).unwrap_or("").split(" by ").next().unwrap_or("").to_string();
                notes.push(l.clone());
            } else if l.starts_with("erase minecraft:walk_target") && !l.contains("(absent)") {
                walk_target = "absent".into();
                notes.push(l.clone());
            } else if l.starts_with("set minecraft:path") {
                path = "set (MoveToTargetSink running)".into();
                notes.push(l.clone());
            } else if l.starts_with("erase minecraft:path") && !l.contains("(absent)") {
                path = "absent".into();
                notes.push(l.clone());
            }
        }
        let p = pos(&mut srv)?.unwrap_or([0.0, 0.0, 0.0]);
        let sleeping = srv.cmd_long("data get entity @e[tag=dj,limit=1] SleepingX")?.contains("entity data");
        println!(
            "  day {day}: activity {activity}; WALK_TARGET {walk_target}; PATH {path}; at ({:.3},{:.3},{:.3}), {:.2} from the bed's head; {}{}",
            p[0],
            p[1],
            p[2],
            ((p[0] - 0.5).powi(2) + (p[2] - 0.5).powi(2)).sqrt(),
            if sleeping { "ASLEEP" } else { "awake" },
            if notes.is_empty() { String::new() } else { format!(" | {}", notes.join(" | ")) }
        );
        if day >= until || day < 0 {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
