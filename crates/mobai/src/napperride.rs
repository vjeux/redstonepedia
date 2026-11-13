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
//! from the summon at R (the whole climb; `--before N` narrows it to N gt before the speed's own
//! detector rise, `--only-before` to the ticks before the rise) to its rest (each a row), printing
//! each row's detector -> rest and the maximum with its restart tick; `--join FROM,TO` the player
//! joining FROM..=TO gt after the start at the worst restart tick (the single-player case: the
//! restart and the login coincide). Ticks after a start at which the cart, not yet at rest, did not
//! move are printed as entity freezes. With MCTAS_NAPPER_RIDE_PATH the nominal ride's path, and
//! every tick from 8 before the detector's rise to 2 after it.

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
    /// cells laid over the module (`--set`: e.g. the hold's slime stop (9,0,9) as built)
    pub set: Vec<([i32; 3], String)>,
}

#[derive(Default)]
struct Ride {
    rise: Option<u32>,
    t_entry: Option<(u32, f64, f64)>,
    rest: Option<u32>,
    restart_tick: Option<u32>,
    freezes: Vec<u32>,
    path: Vec<String>,
    /// every tick: (t, the cart's position, its motion)
    trace: Vec<(u32, [f64; 3], [f64; 3])>,
    /// (ironspec 20:35 PT (2a)) the restart's check: the cart's position across the load
    reload: Option<crate::restartcheck::Reload>,
}

/// A ride whose restart (if any) left the cart where it was across the load: re-run up to 4 times
/// otherwise (the loaded world can move its entities before the freeze: uncounted ticks).
/// (ironspec 23:56 PT: no vacuous passes) restart rows whose every attempt had uncounted ticks
static UNCLEAN_ROWS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[allow(clippy::too_many_arguments)]
fn ride_checked(version: &str, module: &std::path::Path, depth: i32, v: f64, c: &Cells, restart: Restart, join: Option<u32>, max_ticks: u32) -> Result<Ride> {
    let mut last = None;
    for _ in 0..4 {
        let r = ride(version, module, depth, v, c, restart, join, max_ticks)?;
        if r.reload.map(|x| x.clean()).unwrap_or(true) {
            return Ok(r);
        }
        last = Some(r);
    }
    UNCLEAN_ROWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(last.expect("four attempts"))
}

fn ride(version: &str, module: &std::path::Path, depth: i32, v: f64, c: &Cells, restart: Restart, join: Option<u32>, max_ticks: u32) -> Result<Ride> {
    let mut srv = VanillaServer::start(&opts(version, None))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 1000")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    // the joining player's spawn: a glass platform 30 blocks off the module (x -30, z -30), the
    // spawn radius 0 (a survival player spawns anywhere within 10 of the world spawn otherwise:
    // on the napper's track, where it stopped the cart in two join rows)
    srv.cmd_ok(&format!("fill -32 {} -32 -28 {} -28 minecraft:glass", 29 + DY, 29 + DY))?;
    srv.cmd_ok(&format!("setworldspawn -30 {} -30", 30 + DY))?;
    // (the rule's name: 26.3's snake case; whichever the server knows)
    // (G1's note on fde817ca3: which rule took, or an error when neither exists)
    let mut radius_rule = None;
    for rule in ["respawn_radius", "spawn_radius"] {
        let out = srv.cmd(&format!("gamerule {rule} 0"))?;
        if !out.contains("Incorrect") && !out.contains("Unknown") {
            radius_rule = Some(rule);
            break;
        }
    }
    let Some(radius_rule) = radius_rule else {
        anyhow::bail!("neither gamerule respawn_radius nor spawn_radius exists: the joining player could spawn on the track");
    };
    println!("  (the joining player's spawn: (-30, 30, -30) on glass, gamerule {radius_rule} 0)");
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, module)?;
    for (p, s) in &c.set {
        srv.cmd(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
    }
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
        if m.len() == 3 {
            r.trace.push((t, pos, [m[0], m[1], m[2]]));
        }
        last = pos;
        let now = match restart {
            Restart::None => false,
            Restart::AtY(y) => pos[1] >= y,
            Restart::AtTick(k) => t == k,
        };
        if now && restarted_at.is_none() {
            let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, Some(w)), Some("@e[tag=napcart,limit=1]"))?;
            srv = s;
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            restarted_at = Some(t);
            r.restart_tick = Some(t);
            r.reload = Some(rl);
            r.path.push(format!("t{t}: RESTART at ({:.3},{:.3},{:.3}); {}", pos[0], pos[1], pos[2], rl.describe()));
        }
        if let (Some(j), Some(s)) = (join, restarted_at)
            && !joined
            && t >= s + j
        {
            // the player logs in j gt after the start (at the world spawn, off the module)
            let port = srv.game_port();
            srv.bot = Some(mctas::vanilla::bot::Bot::connect(port, "mctas_bot")?);
            joined = true;
            let bp = nums(&srv.cmd_long("data get entity mctas_bot Pos")?);
            r.path.push(format!("t{t}: the player joined at {bp:?}"));
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
        if r.freezes.is_empty() {
            String::new()
        } else {
            // the cart where the freezes start (a stuck cart is still in place there)
            let at = r.freezes.first().and_then(|f| r.trace.iter().find(|(t, _, _)| t == f)).map(|(_, p, m)| format!(" (the cart at ({:.4},{:.4},{:.4}), motion ({:+.4},{:+.4},{:+.4}))", p[0], p[1], p[2], m[0], m[1], m[2])).unwrap_or_default();
            let runs = if r.freezes.len() > 8 { format!("{} ticks from t{} to t{}", r.freezes.len(), r.freezes[0], r.freezes[r.freezes.len() - 1]) } else { format!("{:?}", r.freezes) };
            format!("; entity freezes after the start: {runs}{at}")
        }
    ) + &r.reload.map(|x| format!("; {}", x.describe())).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub fn napper_ride(version: &str, module: &std::path::Path, depth: i32, speeds: &[f64], c: &Cells, restarts: &[f64], every_tick: bool, before: Option<u32>, only_before: bool, join: Option<(u32, u32)>, max_ticks: u32) -> Result<()> {
    println!(
        "== napper-ride on the official {version} server: {} (its entities summoned, force-loaded{}); the napper from R ({},{},4) east at {speeds:?}; the detector ({},{},{}), the T ({},{},{}), the hold ({},{},{}); restarts at cart y {restarts:?}{}{}",
        module.display(),
        if c.set.is_empty() { String::new() } else { format!("; laid over it: {}", c.set.iter().map(|(p, s)| format!("({},{},{}) {s}", p[0], p[1], p[2])).collect::<Vec<_>>().join(", ")) },
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
        if every_tick {
            match before {
                None => "; a restart at every tick from the summon at R (the whole climb) to the rest".to_string(),
                Some(n) => format!("; a restart at every tick from {n} gt before the detector's rise to {}", if only_before { "the rise" } else { "the rest" }),
            }
        } else {
            String::new()
        },
        join.map(|(a, b)| format!("; the player joining {a}..={b} gt after the start at the worst restart tick")).unwrap_or_default()
    );
    let mut overall: Option<(u32, f64, u32)> = None;
    for &v in speeds {
        let nominal = ride(version, module, depth, v, c, Restart::None, None, max_ticks)?;
        println!("{}", line(v, "", c, &nominal));
        if std::env::var_os("MCTAS_NAPPER_RIDE_PATH").is_some() {
            println!("    {}", nominal.path.join("; "));
            if let Some(a0) = nominal.rise {
                // every tick around the rise: the lift's top exit, the exit rail, the detector
                // (MCTAS_NAPPER_RIDE_TRACE: every tick of the ride)
                let all = std::env::var_os("MCTAS_NAPPER_RIDE_TRACE").is_some();
                for (t, p, m) in nominal.trace.iter().filter(|(t, _, _)| all || (*t + 8 >= a0 && *t <= a0 + 2)) {
                    println!(
                        "    t{t}{}: ({:.4},{:.4},{:.4}) motion ({:+.4},{:+.4},{:+.4}) horizontal {:.4}",
                        if *t == a0 { " (the rise)" } else { "" },
                        p[0],
                        p[1],
                        p[2],
                        m[0],
                        m[1],
                        m[2],
                        m[0].hypot(m[2])
                    );
                }
            }
        }
        for &y in restarts {
            let r = ride_checked(version, module, depth, v, c, Restart::AtY(y), None, max_ticks)?;
            println!("{}", line(v, &format!(", restart at y >= {y} (t{})", r.restart_tick.unwrap_or(0)), c, &r));
        }
        if every_tick && let (Some(a0), Some(b)) = (nominal.rise, nominal.rest) {
            // (G1 on 02e012c7f: R_max is over a restart at every tick of the approach, the whole
            // climb from R; a restart at the lift's top, before the rise, moved the rest 4 gt later
            // on f52f712d7) `--before N` narrows the window to N gt before the rise
            let a = before.map(|n| a0.saturating_sub(n)).unwrap_or(1).max(1);
            let b = if only_before { a0.saturating_sub(1).max(a) } else { b };
            let mut worst: Option<(u32, u32)> = None;
            for k in a..=b {
                let r = ride_checked(version, module, depth, v, c, Restart::AtTick(k), None, max_ticks)?;
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
                        let r = ride_checked(version, module, depth, v, c, Restart::AtTick(k), Some(j), max_ticks)?;
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
    } else if !restarts.is_empty() || before.is_some() {
        println!("  the maximum detector -> rest over every row: NOT JUDGED (0 samples)");
        anyhow::bail!("the detector -> rest bar has 0 samples: NOT JUDGED, the run fails");
    }
    // (ironspec 23:56 PT: no vacuous passes) a row whose four attempts all had uncounted ticks
    let unclean = UNCLEAN_ROWS.load(std::sync::atomic::Ordering::Relaxed);
    anyhow::ensure!(unclean == 0, "{unclean} restart row(s) NOT JUDGED (uncounted ticks in all 4 attempts): the run fails");
    Ok(())
}
