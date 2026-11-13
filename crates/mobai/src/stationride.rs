//! (tickexact 2026-11-12, ironspec 20:16 (4): the station's calibration arbiter) `mctas mobai
//! station-ride`: on the official 26.3 server, the lift (`--lift`, 7b90c3f1's path-(a) record file)
//! and the station (`--station`, dfb89499's part) laid in the loop frame, their entities summoned
//! (the batch's three carts in the pockets (x,0,0), x -1/3/7, the next three in the holds), held at
//! day time 1000. Pocket k's input lever (x-2,0,0) is switched on at REL + offset k - 1 and off 16
//! gt later (day-1's repeater (x-1,0,0) into the pocket's rail; the re-brake), offsets `--offsets`
//! (0,12,24). Printed per lane, relative to REL + offset k: the cart's first motion, its leaving the
//! pocket's cell, its entering the merge row (block z 7) with its speed, its entering the shared
//! lift's column (block x -5) and the first tick its y reaches 12 there (the y-12 pass), and for
//! lane 7 (which enters the column from its side slope at y 12) the column entry; `--restarts`
//! rows repeat it with one save/stop/start at REL + that many ticks.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;
const LANES: [i32; 3] = [-1, 3, 7];

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() }
}

#[derive(Default, Clone)]
struct Lane {
    first_motion: Option<u32>,
    moved: Option<u32>,
    /// (dfb89499's "leaves its rest", ironspec 20:35 PT (1)) the first tick the cart is more than
    /// 0.05 off its pocket's centre horizontally
    off05: Option<u32>,
    left: Option<u32>,
    merge: Option<(u32, f64, [f64; 3])>,
    z7_any: Option<(u32, f64, [f64; 3])>,
    slope_rest: Option<(u32, f64)>,
    column: Option<u32>,
    y12: Option<u32>,
    last: Option<[f64; 3]>,
}

#[allow(clippy::too_many_arguments)]
pub fn station_ride(version: &str, lift: &std::path::Path, station: &std::path::Path, rel: u32, offsets: &[u32], restarts: &[u32], ticks: u32, lane7_release: Option<(u32, u32)>) -> Result<()> {
    println!(
        "== station-ride on the official {version} server: {} + {} (loop frame, entities summoned), day time 1000 held; pocket k's lever (x-2,0,0) on at REL + offset k - 1 for 16 gt, REL t{rel}, offsets {offsets:?}{}",
        lift.display(),
        station.display(),
        lane7_release.map(|(at, len)| format!("; lane 7's side slope released by a redstone block at (-4,12,6) at REL + {at} for {len} gt")).unwrap_or_default()
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
        crate::seatclear::place_part(&mut srv, lift)?;
        crate::seatclear::place_part(&mut srv, station)?;
        srv.step(5)?;
        // the pocket carts: the minecart nearest each pocket's centre, tagged; exactly one each
        // (G1's note: an untagged lane errors instead of printing nothing)
        for (k, x) in LANES.iter().enumerate() {
            srv.cmd_ok(&format!("tag @e[type=minecraft:minecart,x={},y={},z=0.5,distance=..0.6,limit=1] add sr{k}", *x as f64 + 0.5, DY))?;
            let n = srv.cmd_long(&format!("execute if entity @e[tag=sr{k}]"))?;
            let count = pass_count(&n);
            println!("  lane x {x}: tagged ({})", n.trim());
            anyhow::ensure!(count == 1, "lane x {x}: {count} tagged carts at its pocket ({})", n.trim());
        }
        let mut lanes = vec![Lane::default(); 3];
        let mut restarted = None;
        for t in 1..=ticks {
            for (k, &x) in LANES.iter().enumerate() {
                let on = rel + offsets[k] - 1;
                if t == on || t == on + 16 {
                    srv.cmd_ok(&format!("setblock {} {} 0 minecraft:lever[face=floor,facing=north,powered={}]", x - 2, DY, t == on))?;
                }
            }
            if let Some((at, len)) = lane7_release {
                if t == rel + at {
                    srv.cmd_ok(&format!("setblock -4 {} 6 minecraft:redstone_block", 12 + DY))?;
                }
                if t == rel + at + len {
                    srv.cmd_ok(&format!("setblock -4 {} 6 minecraft:air", 12 + DY))?;
                }
            }
            if let Some(r) = restart
                && restarted.is_none()
                && t == rel + r
            {
                // the probe: the latest lane already released (a moving cart; lane -1 if none is)
                let k = (0..3).rev().find(|&k| rel + offsets[k] < t).unwrap_or(0);
                let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, Some(w)), Some(&format!("@e[tag=sr{k},limit=1]")))?;
                srv = s;
                println!("  t{t}: restart, probe lane x {} ({})", LANES[k], rl.describe());
                srv.cmd_ok("forceload add -32 -32 31 31")?;
                restarted = Some(t);
            }
            srv.step(1)?;
            for (k, &x) in LANES.iter().enumerate() {
                let p = nums(&srv.cmd_long(&format!("data get entity @e[tag=sr{k},limit=1] Pos"))?);
                let m = nums(&srv.cmd_long(&format!("data get entity @e[tag=sr{k},limit=1] Motion"))?);
                if p.len() != 3 || m.len() != 3 {
                    continue;
                }
                let pos = [p[0], p[1] - DY as f64, p[2]];
                let l = &mut lanes[k];
                let base = rel + offsets[k];
                if t >= base.saturating_sub(1) {
                    if l.first_motion.is_none() && (m[0].abs() > 1e-6 || m[2].abs() > 1e-6) {
                        l.first_motion = Some(t);
                    }
                    if l.moved.is_none()
                        && let Some(q) = l.last
                        && ((q[0] - pos[0]).abs() > 1e-6 || (q[1] - pos[1]).abs() > 1e-6 || (q[2] - pos[2]).abs() > 1e-6)
                    {
                        l.moved = Some(t);
                    }
                    if l.off05.is_none() && (pos[0] - (x as f64 + 0.5)).hypot(pos[2] - 0.5) > 0.05 {
                        l.off05 = Some(t);
                    }
                    if l.z7_any.is_none() && pos[2].floor() as i32 >= 7 {
                        l.z7_any = Some((t, m[0].hypot(m[2]), pos));
                    }
                    // at rest on the side slope (-4,12,7) (lane 7's intake)
                    if l.slope_rest.is_none() && pos[0].floor() as i32 == -4 && pos[1].floor() as i32 == 12 && pos[2].floor() as i32 == 7 && m[0].hypot(m[2]) < 0.002 {
                        l.slope_rest = Some((t, pos[1]));
                    }
                    if l.left.is_none() && (pos[0].floor() as i32 != x || pos[2].floor() as i32 != 0 || pos[1].floor() as i32 != 0) {
                        l.left = Some(t);
                    }
                    if l.merge.is_none() && pos[2].floor() as i32 == 7 && pos[1].floor() as i32 == 0 {
                        l.merge = Some((t, m[0].hypot(m[2]), pos));
                    }
                    if l.column.is_none() && pos[0].floor() as i32 == -5 {
                        l.column = Some(t);
                    }
                    if l.y12.is_none() && pos[0].floor() as i32 == -5 && pos[1] >= 12.0 {
                        l.y12 = Some(t);
                    }
                }
                l.last = Some(pos);
            }
        }
        let rel_s = |t: Option<u32>, base: u32| t.map(|t| format!("+{}", t as i64 - base as i64)).unwrap_or("-".into());
        println!("  {}:", restart.map(|r| format!("a restart at REL + {r}")).unwrap_or_else(|| "no restart".into()));
        for (k, &x) in LANES.iter().enumerate() {
            let base = rel + offsets[k];
            let l = &lanes[k];
            println!(
                "    lane x {x} (REL + {}): first motion {}, first move {}, > 0.05 off the pocket's centre {}, out of the pocket {}, the merge row (z 7, y 0) {}{}, first at z >= 7 (any y) {}{}, the column (x -5) {}, y >= 12 there {}{}; last at {}",
                offsets[k],
                rel_s(l.first_motion, base),
                rel_s(l.moved, base),
                rel_s(l.off05, base),
                rel_s(l.left, base),
                rel_s(l.merge.map(|m| m.0), base),
                l.merge.map(|(_, v, p)| format!(" at {v:.4} b/gt ({:.3},{:.3},{:.3})", p[0], p[1], p[2])).unwrap_or_default(),
                rel_s(l.z7_any.map(|m| m.0), base),
                l.z7_any.map(|(_, v, p)| format!(" at {v:.4} b/gt ({:.3},{:.3},{:.3})", p[0], p[1], p[2])).unwrap_or_default(),
                rel_s(l.column, base),
                rel_s(l.y12, base),
                match (l.slope_rest, lane7_release) {
                    (Some((t, feet)), rel7) => format!(
                        "; at rest on the side slope from {} (feet {feet:.4}){}",
                        rel_s(Some(t), base),
                        match (rel7, l.column) {
                            (Some((at, _)), Some(c)) => format!(", the column entry at the slope's release + {}", c as i64 - (rel + at) as i64),
                            _ => String::new(),
                        }
                    ),
                    _ => String::new(),
                },
                l.last.map(|p| format!("({:.2},{:.2},{:.2})", p[0], p[1], p[2])).unwrap_or("-".into())
            );
        }
        let _ = std::fs::remove_dir_all(&srv.dir);
    }
    Ok(())
}

/// The count in an `execute if entity` reply (26.3's `commands.execute.conditional.pass_count`,
/// "Test passed. Count: %s"; a failed test, "Test failed", is 0).
fn pass_count(reply: &str) -> u32 {
    reply.to_ascii_lowercase().split("count:").nth(1).and_then(|s| s.trim().split(|c: char| !c.is_ascii_digit()).next().and_then(|d| d.parse::<u32>().ok())).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::pass_count;

    /// Replies recorded from the official 26.3 server over RCON (station-ride's own lines).
    #[test]
    fn pass_count_reads_the_servers_reply() {
        assert_eq!(pass_count("Test passed. Count: 1"), 1);
        assert_eq!(pass_count("Test passed. Count: 12"), 12);
        assert_eq!(pass_count("Test failed"), 0);
    }
}
