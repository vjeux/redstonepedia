//! (tickexact 2026-11-12, ironspec 20:53 PT (1); 58f770cc's configuration) `mctas mobai bed-trap`:
//! A6's restart trap on the official 26.3 server. The bed stop alone with its run-out (58f770cc's
//! `bed-tier-fold-bench.txt`, local frame, pasted 80 up with its entities), night held (day time
//! 13800); a ridden cart summoned at the entry cell's centre (3.5, 2.0625, -3.5) heading west at v,
//! the napper riding it with HOME the bed's head (-3,0,4). A restart (save, stop, start) between
//! ticks T-1 and T (T the first tick after the load) goes through `restartcheck::reload`, the cart
//! the probe (re-run up to 4 times until it held). Every tick: the rider seated or not, its feet,
//! asleep or not. Printed per row: the eject (the first unseated tick and the feet there), the
//! longest stall after it (awake, unseated, the feet moving < 0.003 a tick) with where, and the
//! lie-down; `MCTAS_BED_TRAP_TRACE=1` prints the rider every tick to the eject + 12.

use crate::restartcheck::{Reload, reload};
use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const DY: i32 = 80;

/// The overrides' before/after are printed once per run (the first trial).
static OVERRIDES_PRINTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One cell's state as the saved region files hold it (the frame's y).
fn cell_state(srv: &mut VanillaServer, p: [i32; 3]) -> Result<String> {
    let q = [p[0], p[1] + DY, p[2]];
    let snap = srv.snapshot(q, q)?;
    Ok(snap.region.get(q).unwrap_or("?").to_string())
}
/// The brain agent's watch list: the villager's PATH and WALK_TARGET sets, and (+path) every Path's
/// node-index moves (ironspec 22:11 PT: the mechanism, not only the outcome)
const MEMS: &str = "minecraft:path,minecraft:walk_target,minecraft:nearest_hostile,minecraft:cant_reach_walk_target_since,minecraft:home,+path";

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

fn opts(version: &str, jar: &std::path::Path, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        keep_dir: true,
        world,
        // (cc767b78 04:2x) MCTAS_BED_TRAP_NAV: the agent's navigation hooks too (+nav)
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}{}", jar.display(), if std::env::var_os("MCTAS_BED_TRAP_NAV").is_some() { ",+nav" } else { "" }))),
        ..Default::default()
    }
}

/// The brain agent's lines since the last drain (a restart starts a new log: `at` back to 0).
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

#[derive(Default)]
struct Row {
    eject: Option<(u32, [f64; 3])>,
    /// (first tick, last tick, feet) of the longest stall after the eject
    stall: Option<(u32, u32, [f64; 3])>,
    sleep: Option<u32>,
    reload: Option<Reload>,
    trace: Vec<String>,
    /// the mechanism: the villager's path computations and their node-index moves, by tick
    paths: Vec<String>,
}

/// (ironspec 23:19 PT) the real approach instead of a summon at the entry: the ridden cart at rest at
/// `start` (the file's frame), released by a redstone block at `release` from tick `release_at` for
/// 10 gt, or (`lever`, ironspec 00:24 PT: record7's built R1) by turning the lever the `--set` list
/// lays at `release` (powered) off at `release_at`; the napper's HOME at `home`.
#[derive(Clone, Copy)]
pub struct Approach {
    pub start: [f64; 3],
    pub release: [i32; 3],
    pub release_at: u32,
    pub home: [i32; 3],
    /// A cell whose block (a trapdoor's open state, air) is printed at the napper's first panic
    pub watch: Option<[i32; 3]>,
    /// The release is the `--set` lever at `release` turned off (no block appears or disappears)
    pub lever: bool,
    /// The file's own ridden cart at `start` is the napper's (no summon)
    pub file_cart: bool,
    /// Day (day time 1000) instead of night (13800)
    pub day: bool,
    /// With `file_cart`: the cart moved here (at rest) once tagged
    pub cart_pos: Option<[f64; 3]>,
    /// The summoned cart's starting Motion (x only: along the leg's first cell)
    pub start_vx: f64,
    /// A file cart (and its rider) at this position removed before the run (the seat emptied for an
    /// arriving napper)
    pub clear: Option<[f64; 3]>,
}

#[allow(clippy::too_many_arguments)]
fn trial(version: &str, jar: &std::path::Path, file: &std::path::Path, set: &[([i32; 3], String)], v: f64, approach: Option<Approach>, restart: Option<u32>, ticks: u32) -> Result<Row> {
    let mut srv = VanillaServer::start(&opts(version, jar, None))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, file)?;
    // (ironspec 04:07 PT) a bench override that doesn't change its cell fails the run: each --set cell
    // is laid and the server must report it changed (overlay cells equal to the file's own are left
    // out by the CLI, so they never reach here); the first trial prints each override's before/after
    let first = !OVERRIDES_PRINTED.swap(true, std::sync::atomic::Ordering::Relaxed);
    for (p, s) in set {
        let before = if first { cell_state(&mut srv, *p)? } else { String::new() };
        let out = srv.cmd(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
        anyhow::ensure!(out.contains("Changed the block"), "the override ({},{},{}) = {s} changed nothing ({}): a bench override must change its cell (ironspec 04:07): the run fails", p[0], p[1], p[2], out.trim());
        if first {
            println!("  override ({},{},{}): {before} -> {s}", p[0], p[1], p[2]);
        }
    }
    srv.cmd_ok(if approach.is_some_and(|a| a.day) { "time set 1000" } else { "time set 13800" })?;
    srv.step(20)?;
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    let home = approach.map(|a| a.home).unwrap_or([-3, 0, 4]);
    let (start, motion) = match approach {
        Some(a) => (a.start, a.start_vx),
        None => ([3.5, 2.0625, -3.5], -v),
    };
    if let Some(c) = approach.and_then(|a| a.clear) {
        let near = format!("type=minecraft:minecart,x={},y={},z={},distance=..1", c[0], c[1] + DY as f64, c[2]);
        srv.cmd(&format!("execute as @e[{near}] on passengers run kill @s"))?;
        srv.cmd(&format!("kill @e[{near}]"))?;
    }
    if approach.is_some_and(|a| a.file_cart) {
        // (ironspec 00:31 PT: record7 itself) the file's own cart at `start` is the napper's (its
        // rider given HOME and the tags): the bench's entities stay the record's
        let near = format!("x={},y={},z={},distance=..1", start[0], start[1] + DY as f64, start[2]);
        srv.cmd_ok(&format!("tag @e[type=minecraft:minecart,{near},sort=nearest,limit=1] add btc"))?;
        srv.cmd_ok("execute as @e[tag=btc,limit=1] on passengers run tag @s add btv")?;
        anyhow::ensure!(srv.cmd_long("execute if entity @e[tag=btv]")?.contains("passed"), "--file-cart: no ridden minecart within 1 of the approach start");
        srv.cmd_ok(&format!(
            "data merge entity @e[tag=btv,limit=1] {{Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}",
            home[0],
            home[1] + DY,
            home[2]
        ))?;
        // (farmopt 01:04: the arriving cart's approach) the same cart moved elsewhere, at rest
        if let Some(c) = approach.and_then(|a| a.cart_pos) {
            srv.cmd_ok(&format!("data merge entity @e[tag=btc,limit=1] {{Pos:[{}d,{}d,{}d],Motion:[0.0d,0.0d,0.0d]}}", c[0], c[1] + DY as f64, c[2]))?;
        }
    } else {
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {} {} {} {{Tags:[\"btc\"],Motion:[{motion}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"btv\"],Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
            start[0],
            start[1] + DY as f64,
            start[2],
            home[0],
            home[1] + DY,
            home[2]
        ))?;
    }
    let mut r = Row::default();
    let (mut run_from, mut last): (Option<(u32, [f64; 3])>, Option<[f64; 3]>) = (None, None);
    let trace = std::env::var_os("MCTAS_BED_TRAP_TRACE").is_some();
    // the villager's paths (identity ids from its PATH sets): only their index moves are kept
    let mut ids: Vec<String> = Vec::new();
    // only the napper's own PATH sets / erases (the file's other villagers have brains too)
    let me = uuid_of(&mut srv, "@e[tag=btv,limit=1]")?;
    // the first NEAREST_HOSTILE the napper's sensor sets (what it panics at), printed once
    let mut hostile_seen = false;
    let mut hostile_lines = 0u32;
    // CANT_REACH_WALK_TARGET_SINCE as the agent's set / erase lines leave it (the memory is
    // codec-less: never saved, invisible to `data get`)
    let mut stamp: Option<i64> = None;
    let mut panic_seen = false;
    for t in 1..=ticks {
        if let Some(a) = approach {
            let cell = format!("{} {} {}", a.release[0], a.release[1] + DY, a.release[2]);
            if a.lever {
                if t == a.release_at {
                    let laid = set.iter().find(|(p, _)| *p == a.release).map(|(_, s)| s.clone()).ok_or_else(|| anyhow::anyhow!("--release-lever: no --set lever at the release cell"))?;
                    anyhow::ensure!(laid.contains("lever") && laid.contains("powered=true"), "--release-lever: the --set state at the release cell is not a powered lever: {laid}");
                    srv.cmd_ok(&format!("setblock {cell} {}", laid.replace("powered=true", "powered=false")))?;
                }
            } else {
                if t == a.release_at {
                    srv.cmd_ok(&format!("setblock {cell} minecraft:redstone_block"))?;
                }
                if t == a.release_at + 10 {
                    srv.cmd_ok(&format!("setblock {cell} minecraft:air"))?;
                }
            }
        }
        if restart == Some(t) {
            let (s, rl) = reload(srv, |w| opts(version, jar, Some(w)), Some("@e[tag=btc,limit=1]"))?;
            srv = s;
            log.at = 0;
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            r.reload = Some(rl);
            // (G1 on 6ec22ffd8 #7) a world open drops every codec-less memory, the stamp included
            stamp = None;
            r.paths.push(format!("t{t}: RESTART ({}); CANT_REACH dropped with the load (codec-less)", rl.describe()));
        }
        // (ironspec 04:50 (3)) MCTAS_BED_TRAP_SKIP_AT=T: the night skip's effect at tick T (the clock
        // set to the next morning, 24000) and the day let run (advance_time on), the stamp printed
        if std::env::var("MCTAS_BED_TRAP_SKIP_AT").ok().and_then(|v| v.parse::<u32>().ok()) == Some(t) {
            srv.cmd_ok("time set 24000")?;
            srv.cmd_ok("gamerule advance_time true")?;
            r.paths.push(format!("t{t}: NIGHT SKIP (the clock set to 24000, the day running); CANT_REACH {}", stamp.map(|s| s.to_string()).unwrap_or_else(|| "none".into())));
        }
        srv.step(1)?;
        // the mechanism's window: until the lie-down (or 200 gt after the eject)
        let skip_run = std::env::var_os("MCTAS_BED_TRAP_SKIP_AT").is_some();
        let window = r.sleep.is_none() && (skip_run || r.eject.map(|(e, _)| t <= e + 200).unwrap_or(true));
        for l in log.drain(&srv)? {
            if l.contains("minecraft:cant_reach_walk_target_since") && l.contains(&me) {
                if l.starts_with("set ") {
                    stamp = l.split(" = ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok());
                } else if l.starts_with("erase ") {
                    stamp = None;
                }
            }
            if (l.starts_with("set minecraft:nearest_hostile") || l.starts_with("erase minecraft:nearest_hostile")) && l.contains(&me) {
                // every change of the napper's NEAREST_HOSTILE (the first 40), with the tick
                if hostile_lines < 40 && (!hostile_seen || l.starts_with("erase") || std::env::var_os("MCTAS_BED_TRAP_HOSTILE").is_some()) {
                    hostile_lines += 1;
                    r.paths.push(format!("t{t}: {l}"));
                }
                if l.starts_with("set") {
                    hostile_seen = true;
                }
                continue;
            }
            if l.contains("VillagerPanicTrigger.start") && l.contains(&me) && !panic_seen {
                panic_seen = true;
                if let Some(w) = approach.and_then(|a| a.watch) {
                    let at = format!("{} {} {}", w[0], w[1] + DY, w[2]);
                    let state = if srv.cmd(&format!("execute if block {at} #minecraft:trapdoors[open=true]"))?.contains("passed") {
                        "a trapdoor, OPEN"
                    } else if srv.cmd(&format!("execute if block {at} #minecraft:trapdoors[open=false]"))?.contains("passed") {
                        "a trapdoor, CLOSED"
                    } else if srv.cmd(&format!("execute if block {at} minecraft:air"))?.contains("passed") {
                        "air (no block)"
                    } else {
                        "another block"
                    };
                    r.paths.push(format!("t{t}: the panic; the watched cell ({},{},{}): {state}", w[0], w[1], w[2]));
                }
            }
            if l.starts_with("set minecraft:path") && l.contains(&me) {
                if let Some(id) = l.split(" id ").nth(1).and_then(|s| s.split_whitespace().next()) {
                    ids.push(id.to_string());
                }
                if window {
                    r.paths.push(format!("t{t}: {l}"));
                }
            } else if (l.starts_with("erase minecraft:path") || l.starts_with("set minecraft:walk_target") || l.starts_with("erase minecraft:walk_target") || l.contains("minecraft:cant_reach_walk_target_since")) && l.contains(&me) && window {
                r.paths.push(format!("t{t}: {l}"));
            } else if let Some(rest) = l.strip_prefix("path ") {
                let id = rest.split_whitespace().next().unwrap_or("");
                if window && ids.iter().any(|i| i == id) {
                    r.paths.push(format!("t{t}: {l}"));
                }
            } else if l.starts_with("nav ") && l.contains(&me) && window {
                // (+nav, MCTAS_BED_TRAP_NAV) the navigation's per-tick state, timeouts, stops and the
                // sink's stop causes
                r.paths.push(format!("t{t}: {l}"));
            }
        }
        let p = nums(&srv.cmd_long("data get entity @e[tag=btv,limit=1] Pos")?);
        if p.len() != 3 {
            r.trace.push(format!("t{t} no rider"));
            continue;
        }
        let feet = [p[0], p[1] - DY as f64, p[2]];
        let seated = srv.cmd_long("execute as @e[tag=btv,limit=1] on vehicle if entity @s[tag=btc]")?.contains("passed");
        let asleep = srv.cmd_long("data get entity @e[tag=btv,limit=1] sleeping_pos")?.contains("entity data");
        // (ironspec 00:39 / 00:45 PT) the drop row: the cart's position and velocity and the rider's
        // (farmopt 01:04 / ironspec 01:00: the sight anchor) the seated rider's eye, its feet and the
        // cart's y, from `execute anchored eyes` (Entity.getEyePosition), on the first 3 ticks and
        // every 20th
        if std::env::var_os("MCTAS_BED_TRAP_EYE").is_some() && (t <= 3 || t % 20 == 0) {
            srv.cmd("kill @e[type=minecraft:marker,tag=eyem]")?;
            srv.cmd("execute as @e[tag=btv,limit=1] at @s anchored eyes positioned ^ ^ ^ run summon minecraft:marker ~ ~ ~ {Tags:[\"eyem\"]}")?;
            let eye = nums(&srv.cmd_long("data get entity @e[type=minecraft:marker,tag=eyem,limit=1] Pos")?);
            let cart = nums(&srv.cmd_long("data get entity @e[tag=btc,limit=1] Pos")?);
            srv.cmd("kill @e[type=minecraft:marker,tag=eyem]")?;
            if eye.len() == 3 && cart.len() == 3 {
                r.paths.push(format!(
                    "t{t}: eye ({:.4},{:.4},{:.4}) feet y {:.4} cart y {:.4} (eye - feet {:.4})",
                    eye[0],
                    eye[1] - DY as f64,
                    eye[2],
                    p[1] - DY as f64,
                    cart[1] - DY as f64,
                    eye[1] - p[1]
                ));
            }
        }
        // (ironspec 00:39 / 00:45 PT) the drop row: the cart's position and velocity and the rider's
        // health from the release to 130 gt after it
        if trace && approach.is_some_and(|a| t >= a.release_at && t <= a.release_at + 130) {
            let cp = nums(&srv.cmd_long("data get entity @e[tag=btc,limit=1] Pos")?);
            let cm = nums(&srv.cmd_long("data get entity @e[tag=btc,limit=1] Motion")?);
            let hp = srv.cmd_long("data get entity @e[tag=btv,limit=1] Health")?;
            let hp = hp.split("entity data: ").nth(1).map(|s| s.trim().to_string()).unwrap_or_else(|| "?".into());
            if cp.len() == 3 && cm.len() == 3 {
                let rel = t - approach.map(|a| a.release_at).unwrap_or(0);
                r.trace.push(format!("t{t} (release + {rel}) cart ({:.3},{:.3},{:.3}) v ({:.4},{:.4},{:.4}) rider health {hp}", cp[0], cp[1] - DY as f64, cp[2], cm[0], cm[1], cm[2]));
            }
        }
        if trace && r.eject.map(|(e, _)| t <= e + 12).unwrap_or(true) {
            r.trace.push(format!("t{t} {} ({:.3},{:.3},{:.3}){}", if seated { "seated" } else { "on foot" }, feet[0], feet[1], feet[2], if asleep { " asleep" } else { "" }));
        }
        if r.eject.is_none() && !seated {
            r.eject = Some((t, feet));
            // (ironspec 01:17 PT) the stamp age at the eject: CANT_REACH_WALK_TARGET_SINCE (codec-less:
            // never saved, so `data get` can't see it; tracked from the agent's set / erase lines)
            let gt = srv.game_time()?;
            let since = stamp;
            r.paths.push(format!(
                "t{t}: eject: CANT_REACH_WALK_TARGET_SINCE {} (game time {gt}{})",
                since.map(|s| s.to_string()).unwrap_or_else(|| "none".into()),
                since.map(|s| format!("; the stamp's age {}", gt - s)).unwrap_or_default()
            ));
        }
        if asleep && r.sleep.is_none() {
            r.sleep = Some(t);
            // (ironspec 01:00 PT (b)) the visit's cost: HOME kept, and CANT_REACH_WALK_TARGET_SINCE
            let home = srv.cmd_long("data get entity @e[tag=btv,limit=1] Brain.memories.\"minecraft:home\"")?.contains("entity data");
            let cant = stamp.map(|s| s.to_string()).unwrap_or_else(|| "none".into());
            r.paths.push(format!("t{t}: lie-down: HOME {}; CANT_REACH_WALK_TARGET_SINCE {cant} (game time now {})", if home { "kept" } else { "LOST" }, srv.game_time()?));
        }
        // the stall: awake and unseated, the feet moving < 0.003 a tick
        if r.eject.is_some() && !asleep && !seated {
            let still = last.map(|q| (q[0] - feet[0]).hypot(q[2] - feet[2]) < 0.003 && (q[1] - feet[1]).abs() < 0.003).unwrap_or(false);
            if still {
                let (from, at) = *run_from.get_or_insert((t - 1, feet));
                if r.stall.map(|(a, b, _)| t - from > b - a).unwrap_or(true) {
                    r.stall = Some((from, t, at));
                }
            } else {
                run_from = None;
            }
        } else {
            run_from = None;
        }
        last = Some(feet);
        if r.sleep.is_some_and(|s| t > s + 5) {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(r)
}

#[allow(clippy::too_many_arguments)]
pub fn bed_trap(version: &str, file: &std::path::Path, set: &[([i32; 3], String)], speeds: &[f64], approach: Option<Approach>, restarts: &[u32], repeats: u32, ticks: u32, held_only: bool) -> Result<()> {
    println!(
        "== bed-trap on the official {version} server: {} (+{DY} y, its entities summoned), night (day time 13800, held); {}; speeds {speeds:?}; a restart between t(T-1) and tT for T in {restarts:?} (and none), {repeats} run(s) each; each restart row prints the attempt whose cart held across the load and{}, when one is caught in up to 6 attempts, one whose cart moved (uncounted harness ticks: a harness artefact); the brain agent prints the villager's PATH sets (id, next index, every node) and its paths' node-index moves",
        file.display(),
        match approach {
            Some(a) => format!(
                "the REAL APPROACH: a ridden cart at rest at ({},{},{}) released {} ({},{},{}) at t{}, the napper's HOME ({},{},{}) (the speeds unused)",
                a.start[0],
                a.start[1],
                a.start[2],
                if a.lever { "by turning off the --set lever at" } else { "by a redstone block for 10 gt at" },
                a.release[0],
                a.release[1],
                a.release[2],
                a.release_at,
                a.home[0],
                a.home[1],
                a.home[2]
            ),
            None => "a ridden cart at the entry's centre (3.5, 2.0625, -3.5) heading west, the napper's HOME the bed's head (-3,0,4) (local frame)".to_string(),
        },
        if held_only { " (--held-only: not looking for a moved one)" } else { "" }
    );
    let jar = mctas::bridge::brain_agent(version)?;
    let mut not_judged = 0u32;
    let mut rows: Vec<Option<u32>> = vec![None];
    rows.extend(restarts.iter().map(|&r| Some(r)));
    for &v in speeds {
        for &restart in &rows {
            for k in 0..repeats {
                let (mut held, mut moved): (Option<Row>, Option<Row>) = (None, None);
                for _ in 0..if restart.is_some() { 6 } else { 1 } {
                    let row = trial(version, &jar, file, set, v, approach, restart, ticks)?;
                    if row.reload.is_some_and(|x| !x.clean()) {
                        moved.get_or_insert(row);
                    } else {
                        held.get_or_insert(row);
                    }
                    if held.is_some() && (held_only || moved.is_some() || restart.is_none()) {
                        break;
                    }
                }
                for (label, row) in [("the cart held across the load", held), ("the cart MOVED across the load: uncounted harness ticks, a harness artefact", moved)] {
                    let Some(row) = row else {
                        if restart.is_some() {
                            let held_label = label.starts_with("the cart held");
                            if held_label {
                                not_judged += 1;
                            }
                            println!(
                                "  v {v}, restart before t{} (run {}): {}no attempt where {label}",
                                restart.unwrap_or(0),
                                k + 1,
                                if held_label { "NOT JUDGED (0 samples): " } else { "" }
                            );
                        }
                        continue;
                    };
                    if row.eject.is_none() {
                        not_judged += 1;
                        print!("  NOT JUDGED (0 samples: the napper never left the cart): ");
                    }
                    println!(
                        "  v {v}, {} (run {}{}): eject {}; the longest stall after it {}; lie-down {}{}",
                        restart.map(|t| format!("restart before t{t}")).unwrap_or_else(|| "no restart".into()),
                        k + 1,
                        if restart.is_some() { format!(", {label}") } else { String::new() },
                        row.eject.map(|(t, f)| format!("t{t} at ({:.3},{:.3},{:.3})", f[0], f[1], f[2])).unwrap_or_else(|| "never".into()),
                        row.stall.map(|(a, b, f)| format!("{} gt (t{a}..t{b}) at ({:.3},{:.3},{:.3})", b - a, f[0], f[1], f[2])).unwrap_or_else(|| "none".into()),
                        row.sleep.map(|t| format!("t{t}")).unwrap_or_else(|| format!("not within {ticks} gt")),
                        row.reload.map(|x| format!("; {}", x.describe())).unwrap_or_default()
                    );
                    if !row.paths.is_empty() {
                        println!("    paths: {}", row.paths.join(" | "));
                    }
                    if !row.trace.is_empty() {
                        println!("    {}", row.trace.join("; "));
                    }
                }
            }
        }
    }
    // (ironspec 23:56 PT: no vacuous passes) a restart row with no held attempt, or a row whose
    // napper was never ejected, measured nothing
    anyhow::ensure!(not_judged == 0, "{not_judged} row(s) NOT JUDGED (0 samples): the run fails");
    Ok(())
}
