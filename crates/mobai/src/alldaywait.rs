//! (tickexact 2026-11-12, ironspec 13:27 / 15:01 / 15:17, G1 on 02e012c7f: a pick at the
//! stop-9-down queue) `mctas mobai all-day-wait`: on the official 26.3 server, the module
//! (`--module`, its entities summoned, force-loaded; `--set` cells laid over it, e.g. the shaft's
//! hiding glass), `--trials` villagers one after another, each in a cart with HOME at `--home` (the
//! bed's head), from day time `--from` with time running. The cart either sits held at `--seat`
//! (NoGravity) or, with `--start`, rides from there (at each of `--speeds` in turn, west-east motion
//! along x, or `--motion`) down the leg and the shaft to the queue, its arrival the first tick it is
//! at rest in the seat's cell. With `--release-after N` the queue releases it N gt after the arrival
//! (a redstone block at `--release-cell`, removed when the departure detector `--departure` powers):
//! the drop-and-roll to A6's entry (`--entry`), the activator's eject, and the rider's first calm
//! REST tick after it (on A6's ledge) end the trial; else the trial runs `--ticks` gt.
//! Logged every tick (the brain agent): the rider's activity switches, its HOME and NEAREST_HOSTILE
//! writes and its CANT_REACH_WALK_TARGET_SINCE stamp (set, erased). Printed per trial: the stamp at
//! the arrival (its age: the game time less the stamp), the calm (not PANIC) ticks after it with
//! the stamp's age at each (the maximum, against MoveToTargetSink-free REST's 1200-gt HOME limit),
//! the stamp's clear tick against the arrival, HOME's erasures; with the release: the stamp at the
//! entry, the eject, the first calm REST tick after it and the stamp then, and every stamp set from
//! the release to that tick.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const DY: i32 = 80;
const MEMS: &str = "minecraft:home,minecraft:nearest_hostile,minecraft:cant_reach_walk_target_since,minecraft:path";

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

/// The run's settings.
pub struct Wait {
    pub seat: [f64; 3],
    pub home: [i32; 3],
    pub from: i64,
    pub ticks: u32,
    /// (the start, the motion per speed: None = `speeds` along +x)
    pub start: Option<[f64; 3]>,
    pub motion: Option<[f64; 3]>,
    pub speeds: Vec<f64>,
    pub trials: u32,
    pub set: Vec<([i32; 3], String)>,
    /// (N gt after the arrival, the release cell, the departure detector, A6's entry)
    pub release: Option<(u32, [i32; 3], [i32; 3], [i32; 3])>,
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

pub fn all_day_wait(version: &str, module: &std::path::Path, w: &Wait) -> Result<()> {
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
    for (p, s) in &w.set {
        srv.cmd(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
    }
    srv.step(100)?;
    println!(
        "== all-day-wait on the official {version} server: {} (its entities summoned){}; {} trial(s), the rider's cart {} with HOME ({},{},{}), from day time {} with time running{}",
        module.display(),
        if w.set.is_empty() { String::new() } else { format!(", {} cells set over it", w.set.len()) },
        w.trials,
        match w.start {
            None => format!("held at ({},{},{})", w.seat[0], w.seat[1], w.seat[2]),
            Some(p) => format!("from ({},{},{}) at {} to the seat ({},{},{})", p[0], p[1], p[2], w.motion.map(|m| format!("({},{},{})", m[0], m[1], m[2])).unwrap_or_else(|| format!("x speeds {:?}", w.speeds)), w.seat[0], w.seat[1], w.seat[2]),
        },
        w.home[0],
        w.home[1],
        w.home[2],
        w.from,
        match w.release {
            Some((n, c, d, e)) => format!("; released {n} gt after the arrival (a redstone block at ({},{},{}) until the departure detector ({},{},{}) powers) to A6's entry ({},{},{}), the eject and the first calm REST tick after it", c[0], c[1], c[2], d[0], d[1], d[2], e[0], e[1], e[2]),
            None => format!("; {} gt per trial", w.ticks),
        }
    );
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    let seat_cell = [w.seat[0].floor() as i32, w.seat[1].floor() as i32, w.seat[2].floor() as i32];
    let mut summary: Vec<String> = Vec::new();
    for trial in 0..w.trials {
        srv.cmd_ok("gamerule advance_time false")?;
        srv.cmd_ok(&format!("time set {}", w.from))?;
        let (pos, motion, hold) = match w.start {
            Some(p) => {
                let v = if w.speeds.is_empty() { 0.0 } else { w.speeds[trial as usize % w.speeds.len()] };
                (p, w.motion.unwrap_or([v, 0.0, 0.0]), false)
            }
            None => (w.seat, [0.0; 3], true),
        };
        let gt0 = srv.game_time()?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {} {} {} {{{}Motion:[{}d,{}d,{}d],Tags:[\"adwc{trial}\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"adw{trial}\"],Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
            pos[0],
            pos[1] + DY as f64,
            pos[2],
            if hold { "NoGravity:1b," } else { "" },
            motion[0],
            motion[1],
            motion[2],
            w.home[0],
            w.home[1] + DY,
            w.home[2]
        ))?;
        let rsel = format!("@e[tag=adw{trial},limit=1]");
        let csel = format!("@e[tag=adwc{trial},limit=1]");
        let who = uuid_of(&mut srv, &rsel)?;
        srv.cmd_ok("gamerule advance_time true")?;
        log.drain(&srv)?;
        let (mut activity, mut stamp): (String, Option<i64>) = (String::from("(the summon's)"), None);
        let mut arrival: Option<u32> = if hold { Some(0) } else { None };
        let mut stamp_at_arrival: Option<Option<i64>> = if hold { Some(None) } else { None };
        let (mut calm, mut calm_max_age, mut first_panic) = (0u32, None::<i64>, None::<u32>);
        let (mut home_erased, mut stamp_events, mut switches) = (Vec::new(), Vec::<String>::new(), Vec::<String>::new());
        let mut cleared_after_arrival: Option<u32> = None;
        let (mut released, mut unreleased, mut entry, mut eject, mut ledge_calm) = (None::<u32>, false, None::<(u32, Option<i64>)>, None::<u32>, None::<(u32, Option<i64>)>);
        let mut arms_after_release: Vec<u32> = Vec::new();
        let limit = if w.release.is_some() { w.ticks.max(3000) } else { w.ticks };
        // (farmopt 15:42) after the release: the last NEAREST_HOSTILE set (the post's last sight),
        // the first tick not in PANIC; after the eject: the stamp's clear (tick, by what)
        let (mut last_sight, mut calm_after_release, mut ledge_clear) = (None::<u32>, None::<u32>, None::<(u32, String)>);
        let mut departure_tick: Option<u32> = None;
        // (ironspec 15:49) at the first calm tick after the release: the cart's position, the
        // activity before it, and the first PATH set from that tick (the HOME walk's path)
        let (mut calm_cart, mut calm_before, mut calm_path): (Option<[f64; 3]>, String, Option<(u32, String)>) = (None, String::new(), None);
        for t in 1..=limit {
            srv.step(1)?;
            let now = gt0 + t as i64;
            let tick_start_activity = activity.clone();
            for l in log.drain(&srv)? {
                if !l.ends_with(&format!("@{who}")) {
                    continue;
                }
                if let Some(a) = l.strip_prefix("activity ") {
                    activity = a.split(" by ").next().unwrap_or(a).to_string();
                    switches.push(format!("t{t} {activity}"));
                }
                if let Some(p) = l.strip_prefix("set minecraft:path = ")
                    && calm_after_release.is_some_and(|c| t >= c)
                    && calm_path.is_none()
                {
                    calm_path = Some((t, p.split(" by ").next().unwrap_or(p).to_string()));
                }
                if l.starts_with("set minecraft:nearest_hostile") {
                    last_sight = Some(t);
                }
                if l.starts_with("erase minecraft:home") && !l.contains("(absent)") {
                    home_erased.push(t);
                }
                if let Some(v) = l.strip_prefix("set minecraft:cant_reach_walk_target_since = ") {
                    let v = v.split(" by ").next().unwrap_or(v).trim().parse::<i64>().ok();
                    stamp_events.push(format!("t{t} set {}", v.map(|x| x.to_string()).unwrap_or_else(|| "?".into())));
                    if released.is_some() && ledge_calm.is_none() {
                        arms_after_release.push(t);
                    }
                    stamp = v;
                } else if l.starts_with("erase minecraft:cant_reach_walk_target_since") && !l.contains("(absent)") {
                    let by = l.rsplit(" by ").next().unwrap_or("?").split(" @").next().unwrap_or("?").to_string();
                    stamp_events.push(format!("t{t} erased by {by}"));
                    stamp = None;
                    if arrival.is_some_and(|a| t >= a) && cleared_after_arrival.is_none() {
                        cleared_after_arrival = Some(t);
                    }
                    if eject.is_some() && ledge_clear.is_none() {
                        ledge_clear = Some((t, by));
                    }
                }
            }
            if arrival.is_none() {
                let p = nums(&srv.cmd_long(&format!("data get entity {csel} Pos"))?);
                let m = nums(&srv.cmd_long(&format!("data get entity {csel} Motion"))?);
                if p.len() == 3 && m.len() == 3 {
                    let cell = [p[0].floor() as i32, (p[1] - DY as f64).floor() as i32, p[2].floor() as i32];
                    if cell == seat_cell && (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt() < 0.005 {
                        arrival = Some(t);
                        stamp_at_arrival = Some(stamp);
                    }
                }
                continue;
            }
            let arr = arrival.unwrap_or(0);
            if t <= arr {
                continue;
            }
            let panicking = activity.contains("panic");
            if released.is_none() {
                if !panicking {
                    calm += 1;
                    if let Some(s) = stamp {
                        let age = now - s;
                        calm_max_age = Some(calm_max_age.map_or(age, |m: i64| m.max(age)));
                    }
                } else if first_panic.is_none() {
                    first_panic = Some(t);
                }
            }
            if let Some((n, cell, det, ent)) = w.release {
                if released.is_none() && t == arr + n {
                    srv.cmd_ok(&format!("setblock {} {} {} minecraft:redstone_block", cell[0], cell[1] + DY, cell[2]))?;
                    released = Some(t);
                }
                if released.is_some() && !unreleased && srv.cmd_long(&format!("execute if block {} {} {} minecraft:detector_rail[powered=true]", det[0], det[1] + DY, det[2]))?.contains("passed") {
                    srv.cmd_ok(&format!("setblock {} {} {} minecraft:air", cell[0], cell[1] + DY, cell[2]))?;
                    unreleased = true;
                    departure_tick = Some(t);
                }
                if released.is_some() && entry.is_none() {
                    let p = nums(&srv.cmd_long(&format!("data get entity {csel} Pos"))?);
                    if p.len() == 3 && [p[0].floor() as i32, (p[1] - DY as f64).floor() as i32, p[2].floor() as i32] == ent {
                        entry = Some((t, stamp));
                    }
                }
                if released.is_some() && calm_after_release.is_none() && !panicking {
                    calm_after_release = Some(t);
                    calm_before = tick_start_activity.clone();
                    let p = nums(&srv.cmd_long(&format!("data get entity {csel} Pos"))?);
                    if p.len() == 3 {
                        calm_cart = Some([p[0], p[1] - DY as f64, p[2]]);
                    }
                }
                if entry.is_some() && eject.is_none() && !srv.cmd_long(&format!("execute as {rsel} on vehicle if entity @s"))?.contains("passed") {
                    eject = Some(t);
                }
                if eject.is_some() && ledge_calm.is_none() && activity.contains("rest") {
                    ledge_calm = Some((t, stamp));
                }
                // the trial ends once the ledge's stamp is settled: no stamp at the first calm REST
                // tick, its clear after the eject, or 600 gt after the eject
                if let Some((lt, ls)) = ledge_calm
                    && (ls.is_none() || ledge_clear.is_some() || eject.is_some_and(|e| t >= e + 600))
                    && t >= lt
                {
                    break;
                }
            }
        }
        if released.is_some() && !unreleased {
            let (_, cell, _, _) = w.release.unwrap_or_default();
            srv.cmd(&format!("setblock {} {} {} minecraft:air", cell[0], cell[1] + DY, cell[2]))?;
        }
        let riding = srv.cmd_long(&format!("execute as {rsel} on vehicle if entity @s"))?.contains("passed");
        let home_now = srv.cmd_long(&format!("data get entity {rsel} Brain.memories.\"minecraft:home\""))?;
        let home_kept = home_now.contains("pos:");
        let age_at = |s: Option<i64>, t: u32| s.map(|x| format!("{x} (age {})", gt0 + t as i64 - x)).unwrap_or_else(|| "none".into());
        println!(
            "  trial {trial}{}: arrival {}; the stamp then {}; calm ticks after it {calm}{} (the stamp's largest age at a calm tick {}); the stamp cleared after the arrival at {}",
            if hold { String::new() } else { format!(" ({:.2} b/gt)", motion[0].hypot(motion[2])) },
            arrival.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
            stamp_at_arrival.map(|s| age_at(s, arrival.unwrap_or(0))).unwrap_or_else(|| "-".into()),
            first_panic.map(|t| format!(", the first PANIC at t{t}")).unwrap_or_default(),
            calm_max_age.map(|a| a.to_string()).unwrap_or_else(|| "-".into()),
            cleared_after_arrival.map(|t| format!("t{t} (arrival + {})", t - arrival.unwrap_or(0))).unwrap_or_else(|| "never".into())
        );
        if w.release.is_some() {
            println!(
                "    released at {}; the departure detector at {}; the post's last sight (NEAREST_HOSTILE set) at {}; out of PANIC at {} (the activity before: {}; the cart at {}); the first path from then: {}; A6's entry at {} (the stamp then {}); the eject at {}; the first calm REST tick after it {}; stamp sets from the release to it: {}; the stamp's clear after the eject: {}",
                released.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
                departure_tick.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
                last_sight.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
                calm_after_release.map(|t| format!("t{t} (release + {})", t - released.unwrap_or(0))).unwrap_or_else(|| "never".into()),
                if calm_before.is_empty() { "-" } else { calm_before.as_str() },
                calm_cart.map(|p| format!("({:.3},{:.3},{:.3}) cell ({},{},{})", p[0], p[1], p[2], p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32)).unwrap_or_else(|| "-".into()),
                calm_path.as_ref().map(|(t, p)| format!("t{t} {p}")).unwrap_or_else(|| "none".into()),
                entry.map(|(t, _)| format!("t{t}")).unwrap_or_else(|| "never".into()),
                entry.map(|(t, s)| age_at(s, t)).unwrap_or_else(|| "-".into()),
                eject.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
                ledge_calm.map(|(t, s)| format!("t{t} (the stamp {})", age_at(s, t))).unwrap_or_else(|| "never".into()),
                if arms_after_release.is_empty() { "none".to_string() } else { format!("{arms_after_release:?}") },
                match (ledge_calm, &ledge_clear) {
                    (Some((_, None)), _) => "none armed".to_string(),
                    (_, Some((t, by))) => format!("t{t} (eject + {}) by {by}", t - eject.unwrap_or(0)),
                    _ => "NOT within 600 gt".to_string(),
                }
            );
        }
        println!(
            "    activity switches: {}; the stamp's writes: {}; HOME erased at {home_erased:?}, kept at the end: {home_kept}; still seated: {riding}",
            if switches.is_empty() { "none".to_string() } else { switches.iter().take(8).cloned().collect::<Vec<_>>().join(", ") },
            if stamp_events.is_empty() { "none".to_string() } else { stamp_events.iter().take(10).cloned().collect::<Vec<_>>().join("; ") }
        );
        summary.push(format!(
            "{}|{}|{}|{}",
            stamp_at_arrival.flatten().is_some(),
            cleared_after_arrival.map(|t| (t - arrival.unwrap_or(0)) as i64).unwrap_or(-1),
            home_kept,
            arms_after_release.len()
        ));
        srv.cmd(&format!("kill {rsel}"))?;
        srv.cmd(&format!("kill {csel}"))?;
        srv.step(20)?;
        log.drain(&srv)?;
    }
    if w.trials > 1 {
        let armed = summary.iter().filter(|s| s.starts_with("true")).count();
        let kept = summary.iter().filter(|s| s.split('|').nth(2) == Some("true")).count();
        let worst = summary.iter().filter(|s| s.starts_with("true")).filter_map(|s| s.split('|').nth(1).and_then(|x| x.parse::<i64>().ok())).max();
        let arms: usize = summary.iter().filter_map(|s| s.split('|').nth(3).and_then(|x| x.parse::<usize>().ok())).sum();
        println!(
            "  over {} trials: the stamp armed at the arrival in {armed}; the latest clear of an armed stamp after the arrival: {}; HOME kept in {kept}; stamp sets from the release to the ledge's first calm REST tick: {arms}",
            w.trials,
            worst.map(|x| if x < 0 { "NEVER cleared".to_string() } else { format!("arrival + {x}") }).unwrap_or_else(|| "-".into())
        );
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
