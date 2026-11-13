//! (tickexact 2026-11-12, G1 on b8949c96's way-in: the drop-and-roll's official entry) `mctas mobai
//! queue-roll`: on the official 26.3 server, the module with the stop-9-down way-in (`--module`, its
//! entities summoned, force-loaded), a cart with a villager at rest on the queue rail (`--start`),
//! released by a redstone block at `--release-cell` from tick `--release-at` for `--release-len` gt;
//! every tick its position and speed and the rider's health: the tick it enters `--entry` (A6's entry
//! (3,-25,-5)) with its speed there, the rider's least health and its hurt ticks; `--restarts` rows
//! with one save/stop/start at a tick counted from the release (mid-fall, mid-roll).

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
pub fn queue_roll(version: &str, module: &std::path::Path, start: [f64; 3], release_cell: [i32; 3], release_at: u32, release_len: u32, departure: Option<[i32; 3]>, entry: [i32; 3], restarts: &[u32], max_ticks: u32) -> Result<()> {
    println!(
        "== queue-roll on the official {version} server: {} (its entities summoned, force-loaded); a ridden cart at rest at ({},{},{}), a redstone block at ({},{},{}) from t{release_at} {}; the entry ({},{},{}); restarts at release + {restarts:?}",
        module.display(),
        start[0],
        start[1],
        start[2],
        release_cell[0],
        release_cell[1],
        release_cell[2],
        departure.map(|d| format!("until the departure detector ({},{},{}) powers (onechunk's emulation of the gate)", d[0], d[1], d[2])).unwrap_or_else(|| format!("for {release_len} gt")),
        entry[0],
        entry[1],
        entry[2]
    );
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
        srv.step(100)?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {} {} {} {{Tags:[\"qrc\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"qrv\"]}}]}}",
            start[0],
            start[1] + DY as f64,
            start[2]
        ))?;
        let before = srv.cmd_long(&format!("execute if block {} {} {} minecraft:air", release_cell[0], release_cell[1] + DY, release_cell[2]))?.contains("passed");
        let (mut ent, mut least, mut hurts, mut restarted) = (None::<(u32, f64)>, f64::MAX, Vec::new(), None::<u32>);
        let mut reload_note: Option<crate::restartcheck::Reload> = None;
        let mut path = Vec::new();
        let mut removed: Option<u32> = None;
        for t in 1..=max_ticks {
            if t == release_at {
                srv.cmd_ok(&format!("setblock {} {} {} minecraft:redstone_block", release_cell[0], release_cell[1] + DY, release_cell[2]))?;
            }
            if departure.is_none() && t == release_at + release_len {
                srv.cmd_ok(&format!("setblock {} {} {} minecraft:air", release_cell[0], release_cell[1] + DY, release_cell[2]))?;
                removed = Some(t);
            }
            srv.step(1)?;
            if let Some(d) = departure
                && t >= release_at
                && removed.is_none()
                && srv.cmd_long(&format!("execute if block {} {} {} minecraft:detector_rail[powered=true]", d[0], d[1] + DY, d[2]))?.contains("passed")
            {
                srv.cmd_ok(&format!("setblock {} {} {} minecraft:air", release_cell[0], release_cell[1] + DY, release_cell[2]))?;
                removed = Some(t);
            }
            let p = nums(&srv.cmd_long("data get entity @e[tag=qrc,limit=1] Pos")?);
            let m = nums(&srv.cmd_long("data get entity @e[tag=qrc,limit=1] Motion")?);
            let h = nums(&srv.cmd_long("data get entity @e[tag=qrv,limit=1] Health")?).first().copied().unwrap_or(f64::NAN);
            let ht = nums(&srv.cmd_long("data get entity @e[tag=qrv,limit=1] HurtTime")?).first().copied().unwrap_or(0.0);
            if p.len() != 3 {
                path.push(format!("t{t}: no cart"));
                break;
            }
            let pos = [p[0], p[1] - DY as f64, p[2]];
            let speed = if m.len() == 3 { (m[0] * m[0] + m[2] * m[2]).sqrt() } else { -1.0 };
            least = least.min(h);
            if ht > 0.0 {
                hurts.push(t);
            }
            let cell = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
            if ent.is_none() && cell == entry {
                ent = Some((t, speed));
            }
            if t % 5 == 0 || (t >= release_at && t <= release_at + 3) {
                path.push(format!("t{t} ({:.3},{:.3},{:.3}) v {speed:.3} h {h}", pos[0], pos[1], pos[2]));
            }
            if let Some(r) = restart
                && restarted.is_none()
                && t == release_at + r
            {
                let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, Some(w)), Some("@e[tag=qrc,limit=1]"))?;
                srv = s;
                srv.cmd_ok("forceload add -32 -32 31 31")?;
                restarted = Some(t);
                reload_note = Some(rl);
                path.push(format!("t{t}: RESTART at ({:.3},{:.3},{:.3}); {}", pos[0], pos[1], pos[2], rl.describe()));
            }
            if ent.is_some_and(|(e, _)| t > e + 3) {
                break;
            }
        }
        println!(
            "  {}: the release cell {}; the release block removed at {}; the entry ({},{},{}) at {}; the rider's least health {least}; hurt at {}{}",
            restart.map(|r| format!("restart at release + {r}")).unwrap_or_else(|| "no restart".into()),
            if before { "air before" } else { "NOT air before" },
            removed.map(|t| format!("release + {}", t as i64 - release_at as i64)).unwrap_or_else(|| "never".into()),
            entry[0],
            entry[1],
            entry[2],
            ent.map(|(t, s)| format!("t{t} = release + {} ({s:.4} b/gt)", t as i64 - release_at as i64)).unwrap_or_else(|| "never".into()),
            if hurts.is_empty() { "no tick".to_string() } else { format!("{:?}", hurts) },
            reload_note.map(|x| format!("; {}", x.describe())).unwrap_or_default()
        );
        if std::env::var_os("MCTAS_QUEUE_ROLL_PATH").is_some() {
            println!("    {}", path.join("; "));
        }
        let _ = std::fs::remove_dir_all(&srv.dir);
    }
    Ok(())
}
