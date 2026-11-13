//! (tickexact 2026-11-12, ironspec 16:04: path (a)'s window calibration) `mctas mobai hold-pulse`:
//! on the official 26.3 server, 7b90c3f1's path-(a) lift part (`--module`, its entities summoned,
//! force-loaded), the napper's cart (a villager riding) at rest on the hold (9,0,8); the hold's pulse
//! fired into `--pulse-cell` ((7,-1,8): the repeater (8,-1,8) into the slime under the hold) as a
//! redstone block from `--pulse-at` (each of them a start phase) for `--pulse-len` gt; every tick the
//! cart's position and motion: the tick the hold's rail reads powered (the rise), the cart's first
//! motion, its leaving the hold's cell, the T, the ramp, the column's cell (x < -4) and the first
//! tick its y reaches `--pass-y` (12) in the column. Printed per start: rise -> pass in game ticks
//! (and from the tick after the rise, the benches' other convention); `--restarts` rows with one
//! save/stop/start at these ticks after the rise.

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
pub fn hold_pulse(version: &str, module: &std::path::Path, rest: [f64; 3], pulse_cell: [i32; 3], pulse_at: &[u32], pulse_len: u32, pass_y: f64, restarts: &[u32], max_ticks: u32) -> Result<()> {
    println!(
        "== hold-pulse on the official {version} server: {} (its entities summoned, force-loaded); the napper's cart at rest at ({},{},{}); the pulse a redstone block at ({},{},{}) from t{pulse_at:?} for {pulse_len} gt; the pass at y {pass_y} in the column; restarts at rise + {restarts:?}",
        module.display(),
        rest[0],
        rest[1],
        rest[2],
        pulse_cell[0],
        pulse_cell[1],
        pulse_cell[2]
    );
    let hold = [rest[0].floor() as i32, rest[1].floor() as i32, rest[2].floor() as i32];
    for &p in pulse_at {
        let mut rows: Vec<Option<u32>> = vec![None];
        rows.extend(restarts.iter().map(|&r| Some(r)));
        for restart in rows {
            let mut srv = VanillaServer::start(&opts(version, None))?;
            srv.cmd_ok("gamerule advance_time false")?;
            srv.cmd_ok("gamerule spawn_mobs false")?;
            srv.cmd_ok("time set 1000")?;
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            srv.step(40)?;
            crate::seatclear::place_part(&mut srv, module)?;
            srv.step(20)?;
            srv.cmd_ok(&format!(
                "summon minecraft:minecart {} {} {} {{Tags:[\"hpc\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"hpv\"]}}]}}",
                rest[0],
                rest[1] + DY as f64,
                rest[2]
            ))?;
            let air_before = srv.cmd_long(&format!("execute if block {} {} {} minecraft:air", pulse_cell[0], pulse_cell[1] + DY, pulse_cell[2]))?.contains("passed");
            let (mut rise, mut first_motion, mut left, mut t_cell, mut ramp, mut column, mut pass, mut restarted) = (None::<u32>, None::<u32>, None::<u32>, None::<u32>, None::<u32>, None::<u32>, None::<(u32, [f64; 3])>, None::<u32>);
            let mut path: Vec<String> = Vec::new();
            for t in 1..=max_ticks {
                if t == p {
                    srv.cmd_ok(&format!("setblock {} {} {} minecraft:redstone_block", pulse_cell[0], pulse_cell[1] + DY, pulse_cell[2]))?;
                }
                if t == p + pulse_len {
                    srv.cmd_ok(&format!("setblock {} {} {} minecraft:air", pulse_cell[0], pulse_cell[1] + DY, pulse_cell[2]))?;
                }
                srv.step(1)?;
                if rise.is_none() && srv.cmd_long(&format!("execute if block {} {} {} minecraft:powered_rail[powered=true]", hold[0], hold[1] + DY, hold[2]))?.contains("passed") {
                    rise = Some(t);
                }
                let pp = nums(&srv.cmd_long("data get entity @e[tag=hpc,limit=1] Pos")?);
                let m = nums(&srv.cmd_long("data get entity @e[tag=hpc,limit=1] Motion")?);
                if pp.len() != 3 || m.len() != 3 {
                    path.push(format!("t{t}: no cart"));
                    break;
                }
                let pos = [pp[0], pp[1] - DY as f64, pp[2]];
                if rise.is_some() && first_motion.is_none() && (m[0].abs() > 1e-9 || m[2].abs() > 1e-9) {
                    first_motion = Some(t);
                }
                if left.is_none() && rise.is_some() && pos[2] < hold[2] as f64 {
                    left = Some(t);
                }
                if t_cell.is_none() && pos[0].floor() as i32 == 9 && pos[2].floor() as i32 == 7 && rise.is_some() {
                    t_cell = Some(t);
                }
                if ramp.is_none() && pos[0] < -1.0 && pos[1] > 0.1 {
                    ramp = Some(t);
                }
                if column.is_none() && pos[0] < -4.0 {
                    column = Some(t);
                }
                if column.is_some() && pass.is_none() && pos[1] >= pass_y {
                    pass = Some((t, pos));
                }
                if rise.is_some() && (t % 5 == 0 || pass.is_some_and(|(pt, _)| pt == t)) {
                    path.push(format!("t{t} ({:.3},{:.3},{:.3}) v ({:+.3},{:+.3},{:+.3})", pos[0], pos[1], pos[2], m[0], m[1], m[2]));
                }
                if let (Some(r), Some(rs)) = (restart, rise)
                    && restarted.is_none()
                    && t == rs + r
                {
                    srv.cmd_ok("save-on")?;
                    srv.cmd_ok("save-all flush")?;
                    let old = srv.dir.clone();
                    let world = old.join(&srv.level_name);
                    drop(srv);
                    srv = VanillaServer::start(&opts(version, Some(world)))?;
                    let _ = std::fs::remove_dir_all(&old);
                    srv.cmd_ok("forceload add -32 -32 31 31")?;
                    restarted = Some(t);
                    path.push(format!("t{t}: RESTART at ({:.3},{:.3},{:.3})", pos[0], pos[1], pos[2]));
                }
                if pass.is_some() {
                    break;
                }
            }
            let rel = |x: Option<u32>| match (x, rise) {
                (Some(a), Some(r)) => format!("rise + {}", a as i64 - r as i64),
                _ => "-".into(),
            };
            println!(
                "  pulse at t{p}, {}: the pulse cell {}; the rise at {}; first motion {}; out of the hold's cell {}; the T {}; the ramp {}; the column {}; y {pass_y} at {}{}",
                restart.map(|r| format!("restart at rise + {r}")).unwrap_or_else(|| "no restart".into()),
                if air_before { "air before" } else { "NOT air before" },
                rise.map(|t| format!("t{t} (pulse + {})", t as i64 - p as i64)).unwrap_or_else(|| "never".into()),
                rel(first_motion),
                rel(left),
                rel(t_cell),
                rel(ramp),
                rel(column),
                rel(pass.map(|(t, _)| t)),
                match (pass, rise) {
                    (Some((pt, pos)), Some(r)) => format!(" = {} gt from the rise ({} from the tick after it), at ({:.3},{:.3},{:.3})", pt - r, pt - r - 1, pos[0], pos[1], pos[2]),
                    _ => String::new(),
                }
            );
            if std::env::var_os("MCTAS_HOLD_PULSE_PATH").is_some() {
                println!("    {}", path.join("; "));
            }
            let _ = std::fs::remove_dir_all(&srv.dir);
        }
    }
    Ok(())
}
