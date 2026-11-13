//! (tickexact 2026-11-12, ironspec 19:52 (2) / 19:29 PT: the S-bend curve bar) `mctas mobai
//! sbend-boundary`: on the official 26.3 server, the module of record (`--module`) with the lanes
//! file laid over it (`--lanes`, the same frame), lane x 3: ironfarm's siding drive (a delay-3
//! repeater (0,22,9) facing west -> slime (1,22,9) -> dust (1,23,9) -> a delay-1 repeater (1,23,8)
//! facing north -> R's slime support (1,23,7) -> R (1,24,7)) set where absent; the napper's cart (a
//! villager riding) at rest on R, the leader's cart (a villager riding) summoned on the branch at
//! (3.5, 24.0625, 9.5) moving -z at `--leader-v` (0.36). A first pass without the switch finds D
//! (the tick detector (3,24,8) first reads powered); then a pass per offset in `--offsets` with a
//! redstone block set at (-1,22,9) (the drive's input, between ticks) at D + offset. Printed per
//! pass: both carts every tick from D - 2 (position, speed, cell), the ticks both are over C (each
//! footprint, centre +- 0.49, overlapping C's column (3..4, 7..8) with centre y in (23.5, 25.0):
//! ironfarm's touch test), box contacts (0.98 x 0.7 boxes overlapping) and their riders' boxes,
//! the napper's stop (speed < 0.002 for 20 gt) and where.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

const DRIVE: [([i32; 3], &str); 4] = [
    ([0, 22, 9], "minecraft:repeater[facing=west,delay=3]"),
    ([1, 22, 9], "minecraft:slime_block"),
    ([1, 23, 9], "minecraft:redstone_wire[north=side,south=side]"),
    ([1, 23, 8], "minecraft:repeater[facing=south,delay=1]"),
];

struct Pass {
    carts: [Vec<Option<([f64; 3], f64)>>; 2],
    riders: [Vec<Option<[f64; 3]>>; 2],
    d_edge: Option<u32>,
}

#[allow(clippy::too_many_arguments)]
fn run(version: &str, module: &std::path::Path, lanes: &std::path::Path, napper: [f64; 3], leader_v: f64, switch_at: Option<u32>, ticks: u32, show_cells: bool, leader_first: bool, napper_roll: Option<[f64; 6]>) -> Result<Pass> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 1000")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, module)?;
    crate::seatclear::place_part(&mut srv, lanes)?;
    // the drive, where the files leave it air (a repeater's `facing` is its input side)
    let mut laid = Vec::new();
    for (c, b) in DRIVE {
        if srv.cmd_long(&format!("execute if block {} {} {} minecraft:air", c[0], c[1] + DY, c[2]))?.contains("passed") {
            srv.cmd_ok(&format!("setblock {} {} {} {b}", c[0], c[1] + DY, c[2]))?;
            laid.push(format!("({},{},{})", c[0], c[1], c[2]));
        }
    }
    srv.step(20)?;
    // only the two carts of the row: the files' own carts and riders (the lanes' returns, the
    // pockets') removed, so the detector D reads the leader alone
    srv.cmd_ok("kill @e[type=minecraft:minecart]")?;
    srv.cmd_ok("kill @e[type=minecraft:villager]")?;
    srv.step(2)?;
    // D released (a file cart pressed it at the paste; its 20-gt re-check frees it): the leader's
    // own press is then its rising edge
    for _ in 0..60 {
        if srv.cmd_long(&format!("execute if block 3 {} 8 minecraft:detector_rail[powered=false]", 24 + DY))?.contains("passed") {
            break;
        }
        srv.step(1)?;
    }
    anyhow::ensure!(srv.cmd_long(&format!("execute if block 3 {} 8 minecraft:detector_rail[powered=false]", 24 + DY))?.contains("passed"), "the detector (3,24,8) stays powered with no cart");
    if show_cells {
        let mut states = Vec::new();
        for c in [[-1, 22, 9], [0, 22, 9], [1, 22, 9], [1, 23, 9], [1, 23, 8], [1, 23, 7], [1, 24, 7], [3, 24, 8], [3, 24, 7], [3, 24, 6], [3, 23, 6], [3, 22, 6]] {
            let mut name = "other".to_string();
            for id in ["air", "slime_block", "repeater", "redstone_wire", "powered_rail", "detector_rail", "rail", "glass", "stone", "redstone_block", "redstone_torch", "stone_slab", "smooth_stone_slab"] {
                if srv.cmd_long(&format!("execute if block {} {} {} minecraft:{id}", c[0], c[1] + DY, c[2]))?.contains("passed") {
                    name = id.to_string();
                    break;
                }
            }
            states.push(format!("({},{},{}) {name}", c[0], c[1], c[2]));
        }
        println!("  the drive laid at {laid:?}; cells: {}", states.join("; "));
    }
    // (ironspec 20:28 PT) the leader first in the entity list: summoned before the napper, parked
    // 40 above on its own rail, teleported into place with its motion at LEAD
    if leader_first {
        srv.cmd_ok(&format!("setblock 3 {} 9 minecraft:glass", 24 + DY + 39))?;
        srv.cmd_ok(&format!("setblock 3 {} 9 minecraft:rail[shape=north_south]", 24 + DY + 40))?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart 3.5 {} 9.5 {{Tags:[\"sb0\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b,Tags:[\"sbv0\"]}}]}}",
            24.0625 + DY as f64 + 40.0
        ))?;
    }
    if napper_roll.is_none() {
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {} {} {} {{Tags:[\"sb1\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b,Tags:[\"sbv1\"]}}]}}",
            napper[0],
            napper[1] + DY as f64,
            napper[2]
        ))?;
    }
    srv.step(30)?;
    // the leader summoned at LEAD (t10) = P, its press in its first tick (D reads powered after
    // t11): a switch at D - 6.. can still be laid before it
    const LEAD: u32 = 10;
    let mut p = Pass { carts: [Vec::new(), Vec::new()], riders: [Vec::new(), Vec::new()], d_edge: None };
    for t in 1..=ticks {
        // (ironfarm's qualifying phase) the napper rolling: summoned at P with its state after
        // P - 1, before the leader (napper first in the entity list)
        if t == LEAD
            && let Some(s) = napper_roll
        {
            srv.cmd_ok(&format!(
                "summon minecraft:minecart {} {} {} {{Tags:[\"sb1\"],Motion:[{}d,{}d,{}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b,Tags:[\"sbv1\"]}}]}}",
                s[0],
                s[1] + DY as f64,
                s[2],
                s[3],
                s[4],
                s[5]
            ))?;
        }
        if t == LEAD && leader_first {
            srv.cmd_ok(&format!("tp @e[tag=sb0,limit=1] 3.5 {} 9.5", 24.0625 + DY as f64))?;
            srv.cmd_ok(&format!("data merge entity @e[tag=sb0,limit=1] {{Motion:[0.0d,0.0d,{}d]}}", -leader_v))?;
        }
        if t == LEAD && !leader_first {
            srv.cmd_ok(&format!(
                "summon minecraft:minecart 3.5 {} 9.5 {{Tags:[\"sb0\"],Motion:[0.0d,0.0d,{}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b,Tags:[\"sbv0\"]}}]}}",
                24.0625 + DY as f64,
                -leader_v
            ))?;
        }
        if switch_at == Some(t) {
            srv.cmd_ok(&format!("setblock -1 {} 9 minecraft:redstone_block", 22 + DY))?;
        }
        srv.step(1)?;
        if std::env::var_os("MCTAS_SBEND_DEBUG").is_some() && t <= 12 {
            let d = srv.cmd_long(&format!("execute if block 3 {} 8 minecraft:detector_rail[powered=true]", 24 + DY))?.contains("passed");
            let lp = srv.cmd_long("data get entity @e[tag=sb0,limit=1] Pos")?;
            let carts = srv.cmd_long(&format!("execute if entity @e[type=minecraft:minecart,x=3,y={},z=8,dx=1,dy=1,dz=1]", 24 + DY))?;
            eprintln!("  dbg t{t}: D powered {d}; leader {}; a cart in D's cell: {}", lp.trim(), carts.trim());
        }
        if p.d_edge.is_none() && srv.cmd_long(&format!("execute if block 3 {} 8 minecraft:detector_rail[powered=true]", 24 + DY))?.contains("passed") {
            p.d_edge = Some(t);
        }
        for k in 0..2 {
            let pos = nums(&srv.cmd_long(&format!("data get entity @e[tag=sb{k},limit=1] Pos"))?);
            let m = nums(&srv.cmd_long(&format!("data get entity @e[tag=sb{k},limit=1] Motion"))?);
            p.carts[k].push(if pos.len() == 3 && m.len() == 3 { Some(([pos[0], pos[1] - DY as f64, pos[2]], m[0].hypot(m[2]))) } else { None });
            let r = nums(&srv.cmd_long(&format!("data get entity @e[tag=sbv{k},limit=1] Pos"))?);
            p.riders[k].push(if r.len() == 3 { Some([r[0], r[1] - DY as f64, r[2]]) } else { None });
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(p)
}

fn over_c(p: [f64; 3]) -> bool {
    p[0] + 0.49 > 3.0 && p[0] - 0.49 < 4.0 && p[2] + 0.49 > 7.0 && p[2] - 0.49 < 8.0 && p[1] > 23.5 && p[1] < 25.0
}

/// Two axis-aligned boxes (centre x/z, foot y; half width w, height h) overlapping or touching
/// (within 1e-4: a move cut by Entity.move's collision leaves the boxes touching, not overlapping;
/// a minecart collides with pushable entities, its leader's rider included).
fn boxes(a: [f64; 3], wa: f64, ha: f64, b: [f64; 3], wb: f64, hb: f64) -> bool {
    const E: f64 = 1e-4;
    (a[0] - b[0]).abs() < wa + wb + E && (a[2] - b[2]).abs() < wa + wb + E && a[1] < b[1] + hb + E && b[1] < a[1] + ha + E
}

#[allow(clippy::too_many_arguments)]
pub fn sbend_boundary(version: &str, module: &std::path::Path, lanes: &std::path::Path, napper: [f64; 3], leader_v: f64, offsets: &[i32], ticks: u32, leader_first: bool, napper_roll: Option<[f64; 6]>) -> Result<()> {
    println!(
        "== sbend-boundary on the official {version} server: {} + {} (lane 3); the napper {}, the leader from (3.5, 24.0625, 9.5) at {leader_v} b/gt -z summoned at P = t10 (its press in its first tick; D reads powered after t11); the switch (a redstone block at (-1,22,9)) at D + {offsets:?}",
        module.display(),
        lanes.display(),
        match napper_roll {
            Some(s) => format!("rolling: summoned at P before the leader at ({},{},{}) with motion ({},{},{})", s[0], s[1], s[2], s[3], s[4], s[5]),
            None => format!("at rest at ({},{},{})", napper[0], napper[1], napper[2]),
        }
    );
    let nominal = run(version, module, lanes, napper, leader_v, None, ticks, true, leader_first, napper_roll)?;
    let Some(d) = nominal.d_edge else {
        println!("  the detector (3,24,8) never rose: the leader {:?}", nominal.carts[0].iter().rev().flatten().next());
        return Ok(());
    };
    println!("  no switch: D (detector (3,24,8) powered) at t{d}");
    for &off in offsets {
        let Some(at) = u32::try_from(d as i64 + off as i64).ok().filter(|&t| t >= 1) else { continue };
        let p = run(version, module, lanes, napper, leader_v, Some(at), ticks, false, leader_first, napper_roll)?;
        let mut touch = Vec::new();
        let (mut cart_contact, mut rider_contact) = (Vec::new(), Vec::new());
        let mut first_motion = None;
        for t in 0..ticks as usize {
            let (Some((a, _)), Some((b, vb))) = (p.carts[0][t], p.carts[1][t]) else { continue };
            if first_motion.is_none() && vb > 1e-6 {
                first_motion = Some(t as u32 + 1);
            }
            if over_c(a) && over_c(b) {
                touch.push(t as u32 + 1);
            }
            if boxes(a, 0.49, 0.7, b, 0.49, 0.7) {
                cart_contact.push(t as u32 + 1);
            }
            // a cart's box against the other cart's rider (villager 0.6 x 1.95: half width 0.3)
            if let (Some(ra), Some(rb)) = (p.riders[0][t], p.riders[1][t])
                && (boxes(b, 0.49, 0.7, ra, 0.3, 1.95) || boxes(a, 0.49, 0.7, rb, 0.3, 1.95))
            {
                rider_contact.push(t as u32 + 1);
            }
        }
        // the napper's stop: at rest (< 0.002) for 20 gt after it moved
        let stop = first_motion.and_then(|f| {
            (f as usize..ticks as usize).find(|&t| (t..(t + 20).min(ticks as usize)).all(|u| p.carts[1][u].map(|(_, v)| v < 0.002).unwrap_or(false)) && t + 20 <= ticks as usize).map(|t| (t as u32 + 1, p.carts[1][t].map(|(q, _)| q)))
        });
        // (ironfarm's lip margin, sidingbench::lip_margin) N: the first tick whose napper move brings
        // its box front (z - 0.49) to or past the leader's rider's back face (its z + 0.3) where the
        // rider stood after the previous tick, the boxes sharing x (|dx| < 0.79); R: the first tick
        // after which the rider's top (feet + 1.95) is below the napper's floor 24.0; margin N - R - 1
        let lip_n = (1..ticks as usize).find(|&t| match (p.carts[1][t], p.riders[0][t - 1]) {
            // "to or past" within 1e-4 (a cut move leaves the front exactly on the face)
            (Some((n, _)), Some(r)) => n[2] - 0.49 <= r[2] + 0.3 + 1e-4 && (n[0] - r[0]).abs() < 0.79,
            _ => false,
        });
        let lip_r = (0..ticks as usize).find(|&t| p.riders[0][t].map(|r| r[1] + 1.95 < 24.0).unwrap_or(false));
        let lip = match (lip_n, lip_r) {
            (Some(n), Some(r)) => format!("lip margin {} (N t{}, R t{})", n as i64 - r as i64 - 1, n + 1, r + 1),
            (None, Some(r)) => format!("lip margin never (the napper never reaches the rider; R t{})", r + 1),
            _ => "lip margin -".into(),
        };
        println!(
            "  switch at D {}{} (t{at} = P {}{}): D t{}; the napper's first motion t{}; both over C at {touch:?}; cart boxes meeting at {cart_contact:?}; a cart against the other's rider at {rider_contact:?}; {lip}; the napper {}",
            if off < 0 { "-" } else { "+" },
            off.abs(),
            if at >= 10 { "+" } else { "-" },
            (at as i64 - 10).abs(),
            p.d_edge.map(|t| t.to_string()).unwrap_or("-".into()),
            first_motion.map(|t| t.to_string()).unwrap_or("-".into()),
            match stop {
                Some((t, Some(q))) => format!("STOPPED from t{t} at ({:.3},{:.3},{:.3})", q[0], q[1], q[2]),
                _ => "never stopped (rolled on)".into(),
            }
        );
        let from = d.saturating_sub(2).max(1) as usize;
        let rows: Vec<String> = (from..=(ticks as usize).min(from + 40))
            .map(|t| {
                let f = |c: Option<([f64; 3], f64)>| c.map(|(q, v)| format!("({:.3},{:.3},{:.3})/{v:.3}", q[0], q[1], q[2])).unwrap_or("-".into());
                let r = |c: Option<[f64; 3]>| c.map(|q| format!("({:.3},{:.3},{:.3})", q[0], q[1], q[2])).unwrap_or("-".into());
                format!("t{t} L {} rider {} N {}", f(p.carts[0][t - 1]), r(p.riders[0][t - 1]), f(p.carts[1][t - 1]))
            })
            .collect();
        println!("    {}", rows.join("; "));
    }
    Ok(())
}
