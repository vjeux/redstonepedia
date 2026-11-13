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
/// The brain agent's watch list: the villager's PATH and WALK_TARGET sets, and (+path) every Path's
/// node-index moves (ironspec 22:11 PT: the mechanism, not only the outcome)
const MEMS: &str = "minecraft:path,minecraft:walk_target,+path";

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
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
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

fn trial(version: &str, jar: &std::path::Path, file: &std::path::Path, set: &[([i32; 3], String)], v: f64, restart: Option<u32>, ticks: u32) -> Result<Row> {
    let mut srv = VanillaServer::start(&opts(version, jar, None))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, file)?;
    for (p, s) in set {
        srv.cmd_ok(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
    }
    srv.cmd_ok("time set 13800")?;
    srv.step(20)?;
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    srv.cmd_ok(&format!(
        "summon minecraft:minecart 3.5 {} -3.5 {{Tags:[\"btc\"],Motion:[{}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"btv\"],Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;-3,{DY},4]}}}}}}}}}}]}}",
        2.0625 + DY as f64,
        -v
    ))?;
    let mut r = Row::default();
    let (mut run_from, mut last): (Option<(u32, [f64; 3])>, Option<[f64; 3]>) = (None, None);
    let trace = std::env::var_os("MCTAS_BED_TRAP_TRACE").is_some();
    // the villager's paths (identity ids from its PATH sets): only their index moves are kept
    let mut ids: Vec<String> = Vec::new();
    for t in 1..=ticks {
        if restart == Some(t) {
            let (s, rl) = reload(srv, |w| opts(version, jar, Some(w)), Some("@e[tag=btc,limit=1]"))?;
            srv = s;
            log.at = 0;
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            r.reload = Some(rl);
            r.paths.push(format!("t{t}: RESTART ({})", rl.describe()));
        }
        srv.step(1)?;
        let window = r.eject.map(|(e, _)| t <= e + 40).unwrap_or(true);
        for l in log.drain(&srv)? {
            if l.starts_with("set minecraft:path") {
                if let Some(id) = l.split(" id ").nth(1).and_then(|s| s.split_whitespace().next()) {
                    ids.push(id.to_string());
                }
                if window {
                    r.paths.push(format!("t{t}: {l}"));
                }
            } else if l.starts_with("erase minecraft:path") && window {
                r.paths.push(format!("t{t}: {l}"));
            } else if let Some(rest) = l.strip_prefix("path ") {
                let id = rest.split_whitespace().next().unwrap_or("");
                if window && ids.iter().any(|i| i == id) {
                    r.paths.push(format!("t{t}: {l}"));
                }
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
        if trace && r.eject.map(|(e, _)| t <= e + 12).unwrap_or(true) {
            r.trace.push(format!("t{t} {} ({:.3},{:.3},{:.3}){}", if seated { "seated" } else { "on foot" }, feet[0], feet[1], feet[2], if asleep { " asleep" } else { "" }));
        }
        if r.eject.is_none() && !seated {
            r.eject = Some((t, feet));
        }
        if asleep && r.sleep.is_none() {
            r.sleep = Some(t);
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

pub fn bed_trap(version: &str, file: &std::path::Path, set: &[([i32; 3], String)], speeds: &[f64], restarts: &[u32], repeats: u32, ticks: u32) -> Result<()> {
    println!(
        "== bed-trap on the official {version} server: {} (local frame, +{DY} y, its entities summoned), night (day time 13800, held); a ridden cart at the entry's centre (3.5, 2.0625, -3.5) heading west, the napper's HOME the bed's head (-3,0,4); speeds {speeds:?}; a restart between t(T-1) and tT for T in {restarts:?} (and none), {repeats} run(s) each; each restart row prints the attempt whose cart held across the load and, when one is caught in up to 6 attempts, one whose cart moved (uncounted harness ticks: a harness artefact); the brain agent prints the villager's PATH sets (id, next index, every node) and its paths' node-index moves",
        file.display()
    );
    let jar = mctas::bridge::brain_agent(version)?;
    let mut rows: Vec<Option<u32>> = vec![None];
    rows.extend(restarts.iter().map(|&r| Some(r)));
    for &v in speeds {
        for &restart in &rows {
            for k in 0..repeats {
                let (mut held, mut moved): (Option<Row>, Option<Row>) = (None, None);
                for _ in 0..if restart.is_some() { 6 } else { 1 } {
                    let row = trial(version, &jar, file, set, v, restart, ticks)?;
                    if row.reload.is_some_and(|x| !x.clean()) {
                        moved.get_or_insert(row);
                    } else {
                        held.get_or_insert(row);
                    }
                    if held.is_some() && (moved.is_some() || restart.is_none()) {
                        break;
                    }
                }
                for (label, row) in [("the cart held across the load", held), ("the cart MOVED across the load: uncounted harness ticks, a harness artefact", moved)] {
                    let Some(row) = row else {
                        if restart.is_some() {
                            println!("  v {v}, restart before t{} (run {}): no attempt where {label}", restart.unwrap_or(0), k + 1);
                        }
                        continue;
                    };
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
    Ok(())
}
