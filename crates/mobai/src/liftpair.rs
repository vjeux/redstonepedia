//! (tickexact 2026-11-12, ironspec 20:05 (1b): the column bar) `mctas mobai lift-pair`: on the
//! official 26.3 server, 7b90c3f1's path-(a) lift (`--module`, its entities summoned, force-loaded):
//! two carts, each with a villager riding, enter the shared lift's column through its braked side
//! slope (-4,12,7) (ascending_east, off): the leader summoned at rest on the slope and released by
//! a redstone block at `--release-cell` ((-4,12,6), day-1's repeater's cell) for 10 gt at
//! `--release-at`; the follower summoned on the slope once the leader has left it and released
//! `--gaps` gt after the leader. Every tick both carts' positions: the vertical gap between their
//! boxes (0.98 x 0.7 x 0.98) where they overlap horizontally, any box contact (the boxes, or the
//! 0.2-inflated push box, `pushAndPickupEntities`, meeting the other's), and whether either cart's
//! path differs from its solo ride (the leader alone; the follower's solo is the leader's shifted
//! by the gap). Printed per gap: the least vertical box gap and where, contacts, disturbed ticks.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

/// One run: the leader released at `rel`, the follower (when `gap` is set) at `rel + gap`; each
/// cart's position every tick (design frame), None once gone.
fn run(version: &str, module: &std::path::Path, slope: [f64; 3], release_cell: [i32; 3], rel: u32, gap: Option<u32>, ticks: u32) -> Result<[Vec<Option<[f64; 3]>>; 2]> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 1000")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, module)?;
    srv.step(20)?;
    let (rx, ry, rz) = (release_cell[0], release_cell[1] + DY, release_cell[2]);
    anyhow::ensure!(srv.cmd_long(&format!("execute if block {rx} {ry} {rz} minecraft:air"))?.contains("passed"), "the release cell ({},{},{}) is not air", release_cell[0], release_cell[1], release_cell[2]);
    let summon = |srv: &mut VanillaServer, tag: &str| -> Result<()> {
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {} {} {} {{Tags:[\"{tag}\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b,Tags:[\"{tag}v\"]}}]}}",
            slope[0],
            slope[1] + DY as f64,
            slope[2]
        ))?;
        Ok(())
    };
    // (7b90c3f1's question) MCTAS_LIFTPAIR_RIDERS=a,b prints both carts' and riders' positions
    // (the rider's feet; its box 0.6 x 1.95) every tick from a to b
    let window: Option<(u32, u32)> = std::env::var("MCTAS_LIFTPAIR_RIDERS").ok().and_then(|s| {
        let v: Vec<u32> = s.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        (v.len() == 2).then(|| (v[0], v[1]))
    });
    summon(&mut srv, "lp0")?;
    let slope_cell = [slope[0].floor() as i32, slope[1].floor() as i32, slope[2].floor() as i32];
    let mut out: [Vec<Option<[f64; 3]>>; 2] = [Vec::new(), Vec::new()];
    let mut follower_in = false;
    let rel_b = gap.map(|g| rel + g);
    for t in 1..=ticks {
        // the release pulses (10 gt each), the follower summoned once the leader is off the slope
        for r in [Some(rel), rel_b].into_iter().flatten() {
            if t == r {
                srv.cmd_ok(&format!("setblock {rx} {ry} {rz} minecraft:redstone_block"))?;
            }
            if t == r + 10 {
                srv.cmd_ok(&format!("setblock {rx} {ry} {rz} minecraft:air"))?;
            }
        }
        // the follower summoned at rest on the slope once the leader's 10-gt pulse is over (the
        // slope unpowered again) and the leader off the slope: released at rel + gap (gap >= 11)
        if rel_b.is_some() && !follower_in && t >= rel + 10 {
            let off = out[0].last().copied().flatten().map(|p| [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32] != slope_cell).unwrap_or(true);
            if off {
                summon(&mut srv, "lp1")?;
                follower_in = true;
            }
        }
        srv.step(1)?;
        for (k, row) in out.iter_mut().enumerate() {
            let p = nums(&srv.cmd_long(&format!("data get entity @e[tag=lp{k},limit=1] Pos"))?);
            row.push(if p.len() == 3 { Some([p[0], p[1] - DY as f64, p[2]]) } else { None });
        }
        if let Some((a, b)) = window
            && t >= a
            && t <= b
        {
            let mut line = format!("    gap {} t{t}:", gap.map(|g| g.to_string()).unwrap_or("-".into()));
            for k in 0..2 {
                let c = nums(&srv.cmd_long(&format!("data get entity @e[tag=lp{k},limit=1] Pos"))?);
                let v = nums(&srv.cmd_long(&format!("data get entity @e[tag=lp{k}v,limit=1] Pos"))?);
                let who = if k == 0 { "leader" } else { "follower" };
                if c.len() == 3 && v.len() == 3 {
                    line += &format!(
                        " {who} cart ({:.4},{:.4},{:.4}) rider feet ({:.4},{:.4},{:.4}) (rider - cart y {:+.4}; box y {:.4}..{:.4});",
                        c[0],
                        c[1] - DY as f64,
                        c[2],
                        v[0],
                        v[1] - DY as f64,
                        v[2],
                        v[1] - c[1],
                        v[1] - DY as f64,
                        v[1] - DY as f64 + 1.95
                    );
                } else {
                    line += &format!(" {who} -;");
                }
            }
            println!("{line}");
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub fn lift_pair(version: &str, module: &std::path::Path, slope: [f64; 3], release_cell: [i32; 3], rel: u32, gaps: &[u32], ticks: u32) -> Result<()> {
    println!(
        "== lift-pair on the official {version} server: {} (its entities summoned, force-loaded); two ridden carts through the braked side slope at ({},{},{}), released by a redstone block at ({},{},{}) for 10 gt, the leader at t{rel}, the follower {gaps:?} gt later",
        module.display(),
        slope[0],
        slope[1],
        slope[2],
        release_cell[0],
        release_cell[1],
        release_cell[2]
    );
    let solo = run(version, module, slope, release_cell, rel, None, ticks)?;
    let solo = &solo[0];
    // the leader alone: where it rises, tops out, leaves
    let rise: Vec<String> = solo.iter().enumerate().filter(|(t, _)| (t + 1) % 5 == 0).map(|(t, p)| p.map(|p| format!("t{} ({:.2},{:.2},{:.2})", t + 1, p[0], p[1], p[2])).unwrap_or_else(|| format!("t{} gone", t + 1))).collect();
    println!("  the leader alone, every 5 gt: {}", rise.join("; "));
    for &gap in gaps {
        let [a, b] = run(version, module, slope, release_cell, rel, Some(gap), ticks)?;
        let (mut least, mut least_at) = (f64::INFINITY, String::new());
        let (mut contacts, mut push_box) = (Vec::<u32>::new(), Vec::<u32>::new());
        let (mut dis_a, mut dis_b) = (None::<u32>, None::<u32>);
        for t in 0..ticks as usize {
            let (Some(pa), Some(pb)) = (a[t], b[t]) else { continue };
            // horizontal overlap of the boxes (half width 0.49) and of the push box (+0.2)
            let (dx, dz) = ((pa[0] - pb[0]).abs(), (pa[2] - pb[2]).abs());
            let over = dx < 0.98 && dz < 0.98;
            let over_push = dx < 0.98 + 0.2 && dz < 0.98 + 0.2;
            // the vertical gap between the boxes (each 0.7 tall), signed: < 0 overlapping
            let vgap = if pa[1] >= pb[1] { pa[1] - (pb[1] + 0.7) } else { pb[1] - (pa[1] + 0.7) };
            if over && vgap < least {
                least = vgap;
                least_at = format!("t{} leader ({:.3},{:.3},{:.3}) follower ({:.3},{:.3},{:.3})", t + 1, pa[0], pa[1], pa[2], pb[0], pb[1], pb[2]);
            }
            if over && vgap < 0.0 {
                contacts.push(t as u32 + 1);
            } else if over_push && vgap < 0.0 {
                push_box.push(t as u32 + 1);
            }
        }
        for t in 0..ticks as usize {
            let same = |p: Option<[f64; 3]>, q: Option<[f64; 3]>| match (p, q) {
                (Some(p), Some(q)) => (p[0] - q[0]).abs() < 1e-6 && (p[1] - q[1]).abs() < 1e-6 && (p[2] - q[2]).abs() < 1e-6,
                (None, None) => true,
                _ => false,
            };
            if dis_a.is_none() && !same(a[t], solo[t]) {
                dis_a = Some(t as u32 + 1);
            }
            // the follower's solo: the leader's shifted by the gap
            if t >= gap as usize && dis_b.is_none() && b[t].is_some() && !same(b[t], solo[t - gap as usize]) {
                dis_b = Some(t as u32 + 1);
            }
        }
        println!(
            "  gap {gap} gt: least vertical box gap {} ({least_at}); box contacts {}; push-box only {}; the leader {}, the follower {}",
            if least.is_finite() { format!("{least:+.4}") } else { "- (never stacked)".into() },
            if contacts.is_empty() { "0".to_string() } else { format!("{} at t{:?}", contacts.len(), &contacts[..contacts.len().min(8)]) },
            if push_box.is_empty() { "0".to_string() } else { format!("{} at t{:?}", push_box.len(), &push_box[..push_box.len().min(8)]) },
            dis_a.map(|t| format!("DISTURBED from t{t}")).unwrap_or_else(|| "undisturbed".into()),
            dis_b.map(|t| format!("DISTURBED from t{t}")).unwrap_or_else(|| "undisturbed".into()),
        );
    }
    Ok(())
}
