//! (tickexact 2026-11-12, ironspec: A6's napper ride, the citable number for the hold release's
//! settle lock) `mctas mobai napper-ride`: on the official 26.3 server, onechunk's stop-9-down module
//! (`--module`, its entities summoned), the napper's cart summoned at R (as 7b90c3f1's return-bench
//! `--napper-lift --lift-depth`: (4.5, -18.9375 - depth, 4.5), heading east at `--speeds`), every
//! tick its position and speed: the tick the approach detector (`--detector`, (7,-3,4); the module
//! lays a plain east-west rail there, made a detector rail of that shape for the row) rises, the
//! speed it enters the T (`--t`, (9,0,7)) at (the tick its cell becomes the T's, from the north),
//! and the tick it comes to rest on the hold (`--hold`, (9,0,8): its cell, horizontal speed under
//! 0.005). `--restart-at` names climb points (cart y thresholds) where a row saves, stops and
//! starts the server once (the cart and its rider reloaded) before it goes on.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() }
}

#[allow(clippy::too_many_arguments)]
pub fn napper_ride(version: &str, module: &std::path::Path, depth: i32, speeds: &[f64], detector: [i32; 3], t_cell: [i32; 3], hold: [i32; 3], restarts: &[f64], max_ticks: u32) -> Result<()> {
    println!(
        "== napper-ride on the official {version} server: {} (its entities summoned); the napper from R ({},{},4) east at {speeds:?}; the detector ({},{},{}), the T ({},{},{}), the hold ({},{},{}); restarts at cart y {restarts:?}",
        module.display(),
        4,
        -19 - depth,
        detector[0],
        detector[1],
        detector[2],
        t_cell[0],
        t_cell[1],
        t_cell[2],
        hold[0],
        hold[1],
        hold[2]
    );
    let mut rows: Vec<(f64, Option<f64>)> = Vec::new();
    for &v in speeds {
        rows.push((v, None));
        for &r in restarts {
            rows.push((v, Some(r)));
        }
    }
    for (v, restart) in rows {
        let mut srv = VanillaServer::start(&opts(version, None))?;
        srv.cmd_ok("gamerule advance_time false")?;
        srv.cmd_ok("gamerule spawn_mobs false")?;
        srv.cmd_ok("time set 1000")?;
        srv.cmd_ok("forceload add -32 -32 31 31")?;
        srv.step(40)?;
        crate::seatclear::place_part(&mut srv, module)?;
        // the approach detector as the hold release reads it (the module lays a plain rail there):
        // a detector rail of the same shape
        let shape = srv.cmd_long(&format!("execute if block {} {} {} minecraft:rail[shape=east_west]", detector[0], detector[1] + DY, detector[2]))?.contains("passed");
        if shape {
            srv.cmd_ok(&format!("setblock {} {} {} minecraft:detector_rail[shape=east_west,powered=false]", detector[0], detector[1] + DY, detector[2]))?;
        }
        srv.step(100)?;
        let ry = -18.9375 - depth as f64;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart 4.5 {} 4.5 {{Tags:[\"napcart\"],Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"naprider\"]}}]}}",
            ry + DY as f64
        ))?;
        let (mut det_rise, mut t_entry, mut rest, mut restarted) = (None::<u32>, None::<(u32, f64, f64)>, None::<u32>, false);
        let mut last = [0.0f64; 3];
        let mut path: Vec<String> = Vec::new();
        for t in 1..=max_ticks {
            srv.step(1)?;
            let p = nums(&srv.cmd_long("data get entity @e[tag=napcart,limit=1] Pos")?);
            let m = nums(&srv.cmd_long("data get entity @e[tag=napcart,limit=1] Motion")?);
            if p.len() != 3 {
                path.push(format!("t{t}: no cart"));
                break;
            }
            let pos = [p[0], p[1] - DY as f64, p[2]];
            let speed = if m.len() == 3 { (m[0] * m[0] + m[2] * m[2]).sqrt() } else { -1.0 };
            let cell = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
            if det_rise.is_none() && srv.cmd_long(&format!("execute if block {} {} {} minecraft:detector_rail[powered=true]", detector[0], detector[1] + DY, detector[2]))?.contains("passed") {
                det_rise = Some(t);
            }
            if t_entry.is_none() && cell == t_cell {
                // the step that brought it in: its horizontal travel this tick
                let step = ((pos[0] - last[0]).powi(2) + (pos[2] - last[2]).powi(2)).sqrt();
                t_entry = Some((t, speed, step));
            }
            if rest.is_none() && cell == hold && speed >= 0.0 && speed < 0.005 {
                rest = Some(t);
            }
            if t % 5 == 0 || t <= 3 {
                path.push(format!("t{t} ({:.3},{:.3},{:.3}) v {speed:.3}", pos[0], pos[1], pos[2]));
            }
            last = pos;
            if let Some(r) = restart
                && !restarted
                && pos[1] >= r
            {
                // a save, stop and start with the cart where it is
                srv.cmd_ok("save-on")?;
                srv.cmd_ok("save-all flush")?;
                let old = srv.dir.clone();
                let world = old.join(&srv.level_name);
                drop(srv);
                srv = VanillaServer::start(&opts(version, Some(world)))?;
                let _ = std::fs::remove_dir_all(&old);
                srv.cmd_ok("forceload add -32 -32 31 31")?;
                restarted = true;
                path.push(format!("t{t}: RESTART at y {:.3}", pos[1]));
            }
            if rest.is_some() && t > rest.unwrap_or(0) + 5 {
                break;
            }
        }
        println!(
            "  start {v} b/gt{}: detector ({},{},{}) rises at t{}; T entry at {}; rest on the hold at {}; detector -> rest {}",
            restart.map(|r| format!(", restart at y >= {r}")).unwrap_or_default(),
            detector[0],
            detector[1],
            detector[2],
            det_rise.map(|t| t.to_string()).unwrap_or_else(|| "-".into()),
            t_entry.map(|(t, s, st)| format!("t{t} (speed {s:.4}, the entering step {st:.4})")).unwrap_or_else(|| "-".into()),
            rest.map(|t| format!("t{t}")).unwrap_or_else(|| "-".into()),
            match (det_rise, rest) {
                (Some(a), Some(b)) => format!("{} gt", b - a),
                _ => "-".into(),
            }
        );
        if std::env::var_os("MCTAS_NAPPER_RIDE_PATH").is_some() {
            println!("    {}", path.join("; "));
        }
        let _ = std::fs::remove_dir_all(&srv.dir);
    }
    Ok(())
}
