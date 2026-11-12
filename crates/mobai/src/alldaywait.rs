//! (tickexact 2026-11-12, ironspec 13:27 / 15:01: a pick waiting at the stop-9-down queue all day)
//! `mctas mobai all-day-wait`: on the official 26.3 server, the module (`--module`, its entities
//! summoned, force-loaded; `--set` cells laid over it, e.g. the shaft's hiding glass), a villager in
//! a cart with HOME at `--home` (the bed's head), from day time `--from` with time running, for
//! `--ticks` gt. The cart either sits held at `--seat` (NoGravity) or, with `--start`, rides from
//! there (`--motion`) down the leg and the shaft to the queue, its arrival the first tick it is at
//! rest in the seat's cell. Logged every tick (the brain agent): the rider's activity switches, its
//! HOME and NEAREST_HOSTILE writes and its CANT_REACH_WALK_TARGET_SINCE stamp (set, erased);
//! printed: the stamp at the arrival, the calm (not PANIC) ticks after it with the activity, the
//! stamp at every calm tick (its runs), the stamp's erasures after the arrival, HOME's erasures,
//! whether it still rides at the end.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const DY: i32 = 80;
const MEMS: &str = "minecraft:home,minecraft:nearest_hostile,minecraft:cant_reach_walk_target_since";

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

#[allow(clippy::too_many_arguments)]
pub fn all_day_wait(version: &str, module: &std::path::Path, seat: [f64; 3], home: [i32; 3], from: i64, ticks: u32, start: Option<([f64; 3], [f64; 3])>, set: &[([i32; 3], String)]) -> Result<()> {
    let jar = mctas::bridge::brain_agent(version)?;
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        tick_rate: 1000,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule advance_weather false")?;
    srv.cmd_ok("weather clear 1000000")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, module)?;
    for (p, s) in set {
        srv.cmd(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
    }
    srv.step(100)?;
    srv.cmd_ok(&format!("time set {from}"))?;
    let (pos, motion, hold) = match start {
        Some((p, m)) => (p, m, false),
        None => (seat, [0.0; 3], true),
    };
    srv.cmd_ok(&format!(
        "summon minecraft:minecart {} {} {} {{{}Motion:[{}d,{}d,{}d],Tags:[\"adwc\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"adw\"],Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
        pos[0],
        pos[1] + DY as f64,
        pos[2],
        if hold { "NoGravity:1b," } else { "" },
        motion[0],
        motion[1],
        motion[2],
        home[0],
        home[1] + DY,
        home[2]
    ))?;
    let who = uuid_of(&mut srv, "@e[tag=adw,limit=1]")?;
    srv.cmd_ok("gamerule advance_time true")?;
    println!(
        "== all-day-wait on the official {version} server: {} (its entities summoned){}; the rider's cart {} with HOME ({},{},{}), from day time {from} for {ticks} gt, time running",
        module.display(),
        if set.is_empty() { String::new() } else { format!(", {} cells set over it", set.len()) },
        if hold { format!("held at ({},{},{})", seat[0], seat[1], seat[2]) } else { format!("from ({},{},{}) at ({},{},{}) to the seat ({},{},{})", pos[0], pos[1], pos[2], motion[0], motion[1], motion[2], seat[0], seat[1], seat[2]) },
        home[0],
        home[1],
        home[2]
    );
    let mut log_at = 0u64;
    let mut drain = |srv: &VanillaServer| -> Result<Vec<String>> {
        let mut f = std::fs::File::open(&srv.log)?;
        f.seek(SeekFrom::Start(log_at))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let whole = s.rfind('\n').map(|i| i + 1).unwrap_or(0);
        log_at += whole as u64;
        Ok(s[..whole].lines().filter_map(|l| l.split("mctas brain agent: ").nth(1).map(|r| r.trim().to_string())).collect())
    };
    let seat_cell = [seat[0].floor() as i32, seat[1].floor() as i32, seat[2].floor() as i32];
    let (mut activity, mut stamp): (String, Option<String>) = (String::from("?"), None);
    let mut arrival: Option<u32> = if hold { Some(0) } else { None };
    let mut stamp_at_arrival: Option<Option<String>> = if hold { Some(None) } else { None };
    let (mut calm, mut calm_runs) = (0u32, Vec::<(u32, u32, String, Option<String>)>::new());
    let mut run_start: Option<(u32, String, Option<String>)> = None;
    let (mut home_erased, mut stamp_events) = (Vec::new(), Vec::<String>::new());
    let mut switches: Vec<String> = Vec::new();
    for t in 1..=ticks {
        srv.step(1)?;
        for l in drain(&srv)? {
            if !l.ends_with(&format!("@{who}")) {
                continue;
            }
            if let Some(a) = l.strip_prefix("activity ") {
                activity = a.split(" by ").next().unwrap_or(a).to_string();
                switches.push(format!("t{t} {activity}"));
            }
            if l.starts_with("erase minecraft:home") && !l.contains("(absent)") {
                home_erased.push(t);
            }
            if let Some(v) = l.strip_prefix("set minecraft:cant_reach_walk_target_since = ") {
                let v = v.split(" by ").next().unwrap_or(v).to_string();
                stamp_events.push(format!("t{t} set {v}"));
                stamp = Some(v);
            } else if l.starts_with("erase minecraft:cant_reach_walk_target_since") && !l.contains("(absent)") {
                stamp_events.push(format!("t{t} erased by {}", l.rsplit(" by ").next().unwrap_or("?").split(" @").next().unwrap_or("?")));
                stamp = None;
            }
        }
        if arrival.is_none() {
            let p = nums(&srv.cmd_long("data get entity @e[tag=adwc,limit=1] Pos")?);
            let m = nums(&srv.cmd_long("data get entity @e[tag=adwc,limit=1] Motion")?);
            if p.len() == 3 && m.len() == 3 {
                let cell = [p[0].floor() as i32, (p[1] - DY as f64).floor() as i32, p[2].floor() as i32];
                if cell == seat_cell && (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt() < 0.005 {
                    arrival = Some(t);
                    stamp_at_arrival = Some(stamp.clone());
                }
            }
        }
        let Some(arr) = arrival else { continue };
        if t <= arr {
            continue;
        }
        let panicking = activity.contains("panic");
        if !panicking {
            calm += 1;
            if run_start.is_none() {
                run_start = Some((t, activity.clone(), stamp.clone()));
            }
        } else if let Some((s, a, st)) = run_start.take() {
            calm_runs.push((s, t - 1, a, st));
        }
    }
    if let Some((s, a, st)) = run_start.take() {
        calm_runs.push((s, ticks, a, st));
    }
    let riding = srv.cmd_long("execute as @e[tag=adw,limit=1] on vehicle if entity @s")?.contains("passed");
    let home_now = srv.cmd_long("data get entity @e[tag=adw,limit=1] Brain.memories.\"minecraft:home\"")?;
    println!(
        "  arrival at the seat's cell: {}; the stamp then: {}",
        arrival.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
        stamp_at_arrival.map(|s| s.unwrap_or_else(|| "none".into())).unwrap_or_else(|| "-".into())
    );
    println!(
        "  calm (not PANIC) ticks after the arrival: {calm}{}",
        if calm_runs.is_empty() { String::new() } else { format!(" (runs: activity, the stamp at the run's first tick: {})", calm_runs.iter().take(12).map(|(a, b, c, st)| format!("t{a}..t{b} {c} stamp {}", st.clone().unwrap_or_else(|| "none".into()))).collect::<Vec<_>>().join("; ")) }
    );
    println!("  the activity's switches (the first 16): {}", if switches.is_empty() { "none".to_string() } else { switches.iter().take(16).cloned().collect::<Vec<_>>().join("; ") });
    println!("  the stamp's writes: {}", if stamp_events.is_empty() { "none".to_string() } else { stamp_events.iter().take(20).cloned().collect::<Vec<_>>().join("; ") });
    println!("  HOME erased at {home_erased:?}; HOME at the end: {}; still seated: {riding}", home_now.split("entity data: ").nth(1).unwrap_or(&home_now).trim());
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
