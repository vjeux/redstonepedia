//! (tickexact 2026-11-12, ironspec: A6's napper ride, the citable number for the hold release's
//! settle lock) `mctas mobai napper-ride`: on the official 26.3 server, onechunk's stop-9-down module
//! (`--module`, its entities summoned, its chunks force-loaded), the napper's cart summoned at R (as
//! 7b90c3f1's return-bench `--napper-lift --lift-depth`: (4.5, -18.9375 - depth, 4.5), heading east
//! at `--speeds`), every tick its position and speed: the tick the approach detector (`--detector`,
//! (7,-3,4); the module lays a plain east-west rail there, made a detector rail of that shape for
//! the row) rises, the speed it enters the T (`--t`, (9,0,7)) at (the tick its cell becomes the
//! T's), and the tick it comes to rest on the hold (`--hold`, (9,0,8): its cell, horizontal speed
//! under 0.005). Restart rows (a save, stop and start, the cart and its rider reloaded):
//! `--restart-at` at the first tick the cart reaches a climb height; `--every-tick` at every tick
//! from the speed's own detector rise to its rest (each a row), printing each row's detector ->
//! rest and the maximum with its restart tick; `--join FROM,TO` the player joining FROM..=TO gt after
//! the start at the worst restart tick (the single-player case: the restart and the login
//! coincide). Ticks after a start at which the cart, not yet at rest, did not move are printed as
//! entity freezes.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() }
}

#[derive(Clone, Copy)]
enum Restart {
    None,
    AtY(f64),
    AtTick(u32),
}

pub struct Cells {
    pub detector: [i32; 3],
    pub t_cell: [i32; 3],
    pub hold: [i32; 3],
}

#[derive(Default)]
struct Ride {
    rise: Option<u32>,
    t_entry: Option<(u32, f64, f64)>,
    rest: Option<u32>,
    restart_tick: Option<u32>,
    freezes: Vec<u32>,
    path: Vec<String>,
}

fn ride(version: &str, module: &std::path::Path, depth: i32, v: f64, c: &Cells, restart: Restart, join: Option<u32>, max_ticks: u32) -> Result<Ride> {
    let mut srv = VanillaServer::start(&opts(version, None))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 1000")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.cmd_ok(&format!("setworldspawn 0 {} 0", 30 + DY))?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, module)?;
    let d = c.detector;
    if srv.cmd_long(&format!("execute if block {} {} {} minecraft:rail[shape=east_west]", d[0], d[1] + DY, d[2]))?.contains("passed") {
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:detector_rail[shape=east_west,powered=false]", d[0], d[1] + DY, d[2]))?;
    }
    srv.step(100)?;
    let ry = -18.9375 - depth as f64;
    srv.cmd_ok(&format!(
        "summon minecraft:minecart 4.5 {} 4.5 {{Tags:[\"napcart\"],Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"naprider\"]}}]}}",
        ry + DY as f64
    ))?;
    let mut r = Ride::default();
    let mut last = [f64::NAN; 3];
    let mut restarted_at: Option<u32> = None;
    let mut joined = false;
    for t in 1..=max_ticks {
        srv.step(1)?;
        let p = nums(&srv.cmd_long("data get entity @e[tag=napcart,limit=1] Pos")?);
        let m = nums(&srv.cmd_long("data get entity @e[tag=napcart,limit=1] Motion")?);
        if p.len() != 3 {
            r.path.push(format!("t{t}: no cart"));
            break;
        }
        let pos = [p[0], p[1] - DY as f64, p[2]];
        let speed = if m.len() == 3 { (m[0] * m[0] + m[2] * m[2]).sqrt() } else { -1.0 };
        let cell = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
        if r.rise.is_none() && srv.cmd_long(&format!("execute if block {} {} {} minecraft:detector_rail[powered=true]", d[0], d[1] + DY, d[2]))?.contains("passed") {
            r.rise = Some(t);
        }
        if r.t_entry.is_none() && cell == c.t_cell {
            let step = ((pos[0] - last[0]).powi(2) + (pos[2] - last[2]).powi(2)).sqrt();
            r.t_entry = Some((t, speed, step));
        }
        if r.rest.is_none() && cell == c.hold && (0.0..0.005).contains(&speed) {
            r.rest = Some(t);
        }
        if restarted_at.is_some() && r.rest.is_none() && pos == last {
            r.freezes.push(t);
        }
        if t % 5 == 0 || t <= 3 {
            r.path.push(format!("t{t} ({:.3},{:.3},{:.3}) v {speed:.3}", pos[0], pos[1], pos[2]));
        }
        last = pos;
        let now = match restart {
            Restart::None => false,
            Restart::AtY(y) => pos[1] >= y,
            Restart::AtTick(k) => t == k,
        };
        if now && restarted_at.is_none() {
            srv.cmd_ok("save-on")?;
            srv.cmd_ok("save-all flush")?;
            let old = srv.dir.clone();
            let world = old.join(&srv.level_name);
            drop(srv);
            srv = VanillaServer::start(&opts(version, Some(world)))?;
            let _ = std::fs::remove_dir_all(&old);
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            restarted_at = Some(t);
            r.restart_tick = Some(t);
            r.path.push(format!("t{t}: RESTART at ({:.3},{:.3},{:.3})", pos[0], pos[1], pos[2]));
        }
        if let (Some(j), Some(s)) = (join, restarted_at)
            && !joined
            && t >= s + j
        {
            // the player logs in j gt after the start (the world spawn over the station)
            let port = srv.game_port();
            srv.bot = Some(mctas::vanilla::bot::Bot::connect(port, "mctas_bot")?);
            joined = true;
            r.path.push(format!("t{t}: the player joined"));
        }
        if r.rest.is_some_and(|x| t > x + 5) {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(r)
}

fn line(v: f64, label: &str, c: &Cells, r: &Ride) -> String {
    format!(
        "  start {v} b/gt{label}: detector ({},{},{}) rises at t{}; T entry at {}; rest on the hold at {}; detector -> rest {}{}",
        c.detector[0],
        c.detector[1],
        c.detector[2],
        r.rise.map(|t| t.to_string()).unwrap_or_else(|| "-".into()),
        r.t_entry.map(|(t, s, st)| format!("t{t} (speed {s:.4}, the entering step {st:.4})")).unwrap_or_else(|| "-".into()),
        r.rest.map(|t| format!("t{t}")).unwrap_or_else(|| "-".into()),
        match (r.rise, r.rest) {
            (Some(a), Some(b)) => format!("{} gt", b - a),
            _ => "-".into(),
        },
        if r.freezes.is_empty() { String::new() } else { format!("; entity freezes after the start at {:?}", r.freezes) }
    )
}

#[allow(clippy::too_many_arguments)]
pub fn napper_ride(version: &str, module: &std::path::Path, depth: i32, speeds: &[f64], c: &Cells, restarts: &[f64], every_tick: bool, before: u32, only_before: bool, join: Option<(u32, u32)>, max_ticks: u32) -> Result<()> {
    println!(
        "== napper-ride on the official {version} server: {} (its entities summoned, force-loaded); the napper from R ({},{},4) east at {speeds:?}; the detector ({},{},{}), the T ({},{},{}), the hold ({},{},{}); restarts at cart y {restarts:?}{}{}",
        module.display(),
        4,
        -19 - depth,
        c.detector[0],
        c.detector[1],
        c.detector[2],
        c.t_cell[0],
        c.t_cell[1],
        c.t_cell[2],
        c.hold[0],
        c.hold[1],
        c.hold[2],
        if every_tick { format!("; a restart at every tick from {before} gt before the detector's rise to the rest") } else { String::new() },
        join.map(|(a, b)| format!("; the player joining {a}..={b} gt after the start at the worst restart tick")).unwrap_or_default()
    );
    let mut overall: Option<(u32, f64, u32)> = None;
    for &v in speeds {
        let nominal = ride(version, module, depth, v, c, Restart::None, None, max_ticks)?;
        println!("{}", line(v, "", c, &nominal));
        if std::env::var_os("MCTAS_NAPPER_RIDE_PATH").is_some() {
            println!("    {}", nominal.path.join("; "));
        }
        for &y in restarts {
            let r = ride(version, module, depth, v, c, Restart::AtY(y), None, max_ticks)?;
            println!("{}", line(v, &format!(", restart at y >= {y} (t{})", r.restart_tick.unwrap_or(0)), c, &r));
        }
        if every_tick && let (Some(a0), Some(b)) = (nominal.rise, nominal.rest) {
            // (the approach: from `before` gt ahead of the detector's rise; a restart at the lift's
            // top, before the rise, moved the rest 4 gt later on f52f712d7)
            let a = a0.saturating_sub(before).max(1);
            let b = if only_before { a0.saturating_sub(1).max(a) } else { b };
            let mut worst: Option<(u32, u32)> = None;
            for k in a..=b {
                let r = ride(version, module, depth, v, c, Restart::AtTick(k), None, max_ticks)?;
                println!("{}", line(v, &format!(", restart at t{k} (detector {}{})", if k >= a0 { "+" } else { "-" }, k.abs_diff(a0)), c, &r));
                if let (Some(x), Some(y)) = (r.rise, r.rest) {
                    let dr = y - x;
                    if worst.is_none_or(|w| dr > w.0) {
                        worst = Some((dr, k));
                    }
                }
            }
            if let Some((dr, k)) = worst {
                println!("  start {v} b/gt: the maximum detector -> rest over the restart ticks t{a}..t{b}: {dr} gt (the restart at t{k})");
                if overall.is_none_or(|o| dr > o.0) {
                    overall = Some((dr, v, k));
                }
                if let Some((j0, j1)) = join {
                    let mut jworst: Option<(u32, u32)> = None;
                    for j in j0..=j1 {
                        let r = ride(version, module, depth, v, c, Restart::AtTick(k), Some(j), max_ticks)?;
                        println!("{}", line(v, &format!(", restart at t{k}, the player joining {j} gt after the start"), c, &r));
                        if let (Some(x), Some(y)) = (r.rise, r.rest)
                            && jworst.is_none_or(|w| y - x > w.0)
                        {
                            jworst = Some((y - x, j));
                        }
                    }
                    if let Some((dr, j)) = jworst {
                        println!("  start {v} b/gt, restart at t{k}: the maximum with a join: {dr} gt (joining {j} gt after the start)");
                        if overall.is_none_or(|o| dr > o.0) {
                            overall = Some((dr, v, k));
                        }
                    }
                }
            }
        }
    }
    if let Some((dr, v, k)) = overall {
        println!("  the maximum detector -> rest over every row: {dr} gt (start {v} b/gt, the restart at t{k})");
    }
    Ok(())
}
