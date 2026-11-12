//! (tickexact 2026-11-12, ironspec: dfb89499's lane 7 hold, 7b90c3f1's lift part) `mctas mobai
//! side-slope`: on the official 26.3 server, the module (`--module`, its shared lift's bubble column
//! (-5,0..22,7) with its glass walls; entities summoned, force-loaded) with a side entry laid into
//! the column's east wall: the braked slope powered_rail[ascending_east] at (-4,12,7) on glass
//! (-4,11,7), the flat crest rail (-3,13,7) on glass, the climb's last slope rail[ascending_west]
//! (-2,12,7) and a flat (-1,12,7), each on glass, and an oak wall sign facing south at (-4,13,7) on
//! glass (-4,13,6), dry or (`--waterlogged`) waterlogged. A villager-ridden cart from the flat at
//! each `--speeds` west over the crest comes to rest on the slope; at `--release-at` a redstone
//! block under the slope (-4,11,7) for 10 gt powers it, and the cart enters the column and rises.
//! Printed: every change of the states of (-4,13,7) and its six neighbours (the water's level and
//! source, waterlogging), the slope's and the crest's states, the rest tick and feet y, the rider's
//! air while it rests, the entry into the column and the exit at its top; `--restarts` rows with one
//! save/stop/start at these ticks (at rest, right after the release).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() }
}

/// The first of the candidate states the cell holds (`execute if block`), else "other".
fn state_of(srv: &mut VanillaServer, p: [i32; 3]) -> Result<String> {
    let mut cands: Vec<String> = vec![
        "minecraft:oak_wall_sign[waterlogged=true]".into(),
        "minecraft:oak_wall_sign[waterlogged=false]".into(),
        "minecraft:bubble_column[drag=false]".into(),
        "minecraft:bubble_column[drag=true]".into(),
        "minecraft:powered_rail[waterlogged=true]".into(),
        "minecraft:powered_rail[waterlogged=false]".into(),
        "minecraft:rail[waterlogged=true]".into(),
        "minecraft:rail[waterlogged=false]".into(),
        "minecraft:glass".into(),
        "minecraft:redstone_block".into(),
        "minecraft:air".into(),
    ];
    for l in 0..16 {
        cands.push(format!("minecraft:water[level={l}]"));
    }
    for c in cands {
        if srv.cmd_long(&format!("execute if block {} {} {} {c}", p[0], p[1] + DY, p[2]))?.contains("passed") {
            return Ok(c.trim_start_matches("minecraft:").to_string());
        }
    }
    Ok("other".into())
}

#[allow(clippy::too_many_arguments)]
pub fn side_slope(version: &str, module: &std::path::Path, speeds: &[f64], waterlogged: bool, release_at: u32, restarts: &[u32], max_ticks: u32) -> Result<()> {
    println!(
        "== side-slope on the official {version} server: {} (its shared lift), the side entry laid (slope (-4,12,7), crest (-3,13,7), climb (-2,12,7), flat (-1,12,7)), the wall sign at (-4,13,7) {}; a ridden cart west at {speeds:?}; the slope powered from t{release_at} for 10 gt; restarts at t{restarts:?}",
        module.display(),
        if waterlogged { "WATERLOGGED" } else { "dry" }
    );
    let watch: [[i32; 3]; 7] = [[-4, 13, 7], [-5, 13, 7], [-3, 13, 7], [-4, 14, 7], [-4, 12, 7], [-4, 13, 6], [-4, 13, 8]];
    for &v in speeds {
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
            let mut before = Vec::new();
            for p in [[-3, 12, 7], [-2, 11, 7], [-1, 11, 7], [-3, 13, 7], [-2, 12, 7], [-1, 12, 7], [-4, 13, 6]] {
                before.push(format!("({},{},{}) {}", p[0], p[1], p[2], state_of(&mut srv, p)?));
            }
            for (p, s) in [
                ([-4, 11, 7], "minecraft:glass".to_string()),
                ([-3, 12, 7], "minecraft:glass".into()),
                ([-2, 11, 7], "minecraft:glass".into()),
                ([-1, 11, 7], "minecraft:glass".into()),
                ([-4, 13, 6], "minecraft:glass".into()),
                ([-4, 12, 7], "minecraft:powered_rail[shape=ascending_east,powered=false]".into()),
                ([-3, 13, 7], "minecraft:rail[shape=east_west]".into()),
                ([-2, 12, 7], "minecraft:rail[shape=ascending_west]".into()),
                ([-1, 12, 7], "minecraft:rail[shape=east_west]".into()),
                ([-4, 13, 7], format!("minecraft:oak_wall_sign[facing=south,waterlogged={waterlogged}]")),
            ] {
                srv.cmd(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
            }
            if restart.is_none() && v == speeds[0] {
                println!("  the module's cells before the entry was laid: {}", before.join(", "));
            }
            srv.step(5)?;
            srv.cmd_ok(&format!(
                "summon minecraft:minecart -0.5 {} 7.5 {{Tags:[\"ssc\"],Motion:[{}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"ssv\"]}}]}}",
                12.0625 + DY as f64,
                -v
            ))?;
            let mut states: Vec<String> = Vec::new();
            for p in &watch {
                states.push(state_of(&mut srv, *p)?);
            }
            let mut changes: Vec<String> = Vec::new();
            let (mut rest, mut rest_y, mut airs, mut entry, mut exit, mut restarted) = (None::<u32>, f64::NAN, Vec::<(u32, i64)>::new(), None::<u32>, None::<(u32, [f64; 3])>, None::<u32>);
            let (mut on_slope, mut creep) = (None::<u32>, Vec::<String>::new());
            for t in 1..=max_ticks {
                if t == release_at {
                    srv.cmd_ok(&format!("setblock -4 {} 7 minecraft:redstone_block", 11 + DY))?;
                }
                if t == release_at + 10 {
                    srv.cmd_ok(&format!("setblock -4 {} 7 minecraft:glass", 11 + DY))?;
                }
                srv.step(1)?;
                for (i, p) in watch.iter().enumerate() {
                    let s = state_of(&mut srv, *p)?;
                    if s != states[i] {
                        changes.push(format!("t{t} ({},{},{}) {} -> {s}", p[0], p[1], p[2], states[i]));
                        states[i] = s;
                    }
                }
                let pp = nums(&srv.cmd_long("data get entity @e[tag=ssc,limit=1] Pos")?);
                let m = nums(&srv.cmd_long("data get entity @e[tag=ssc,limit=1] Motion")?);
                if pp.len() != 3 || m.len() != 3 {
                    changes.push(format!("t{t}: no cart"));
                    break;
                }
                let pos = [pp[0], pp[1] - DY as f64, pp[2]];
                let speed = (m[0] * m[0] + m[2] * m[2]).sqrt();
                let cell = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
                if rest.is_none() && t < release_at && cell == [-4, 12, 7] && speed < 0.005 && m[1].abs() < 0.005 {
                    rest = Some(t);
                    rest_y = pos[1];
                }
                // the rider's air from the cart's first tick on the slope to the release (a cart that
                // keeps creeping never "rests"); the cart's position and motion just before it
                if cell == [-4, 12, 7] && t < release_at {
                    on_slope.get_or_insert(t);
                    let a = nums(&srv.cmd_long("data get entity @e[tag=ssv,limit=1] Air")?).first().copied().unwrap_or(f64::NAN) as i64;
                    if airs.last().is_none_or(|l| l.1 != a) {
                        airs.push((t, a));
                    }
                    if t + 1 == release_at || t + 40 == release_at {
                        creep.push(format!("t{t} ({:.4},{:.4}) motion ({:+.5},{:+.5},{:+.5})", pos[0], pos[1], m[0], m[1], m[2]));
                    }
                }
                if entry.is_none() && t >= release_at && cell[0] == -5 {
                    entry = Some(t);
                }
                if entry.is_some() && exit.is_none() && pos[1] >= 23.0 {
                    exit = Some((t, pos));
                }
                if let Some(r) = restart
                    && restarted.is_none()
                    && t == r
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
                    changes.push(format!("t{t}: RESTART (the cart at ({:.3},{:.4},{:.3}))", pos[0], pos[1], pos[2]));
                }
                if exit.is_some_and(|(e, _)| t > e + 2) {
                    break;
                }
            }
            println!(
                "  {v} b/gt, {}: on the slope from {}; rest on it at {} (feet y {rest_y:.4}); the rider's air on the slope {}; the cart before the release {}; the entry into the column at {}; the exit at the top at {}",
                restart.map(|r| format!("a restart at t{r}")).unwrap_or_else(|| "no restart".into()),
                on_slope.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
                rest.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
                if airs.is_empty() { "-".to_string() } else { airs.iter().take(10).map(|(t, a)| format!("t{t} {a}")).collect::<Vec<_>>().join(", ") },
                if creep.is_empty() { "-".to_string() } else { creep.join(", ") },
                entry.map(|t| format!("t{t} (release + {})", t as i64 - release_at as i64)).unwrap_or_else(|| "never".into()),
                exit.map(|(t, p)| format!("t{t} ({:.3},{:.3},{:.3})", p[0], p[1], p[2])).unwrap_or_else(|| "never".into())
            );
            println!("    states at the start: {}", watch.iter().zip(&states).map(|(p, _)| format!("({},{},{})", p[0], p[1], p[2])).collect::<Vec<_>>().join(" "));
            println!("    changes: {}", if changes.is_empty() { "none".to_string() } else { changes.iter().take(30).cloned().collect::<Vec<_>>().join("; ") });
            println!("    the states at the end: {}", watch.iter().zip(&states).map(|(p, s)| format!("({},{},{}) {s}", p[0], p[1], p[2])).collect::<Vec<_>>().join(", "));
            let _ = std::fs::remove_dir_all(&srv.dir);
        }
    }
    Ok(())
}
