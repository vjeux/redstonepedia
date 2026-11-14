//! (tickexact 2026-11-13, G1 23:06 (2) / D1: the drift off a rail end) `mctas mobai rail-end-drop`: on the official 26.3
//! server (a default world: OldMinecartBehavior), a cart rolling north-to-south along a flat rail line that ends at
//! z = 0 (the last rail cell z -1, y 8 over a stone floor whose top is y 0), leaving the end and falling 8 blocks onto
//! open stone (no walls, no landing rail): one copy per row, 8 apart. Each cart is summoned on the last rail (z -0.5)
//! with its stored speed v south (Motion), EMPTY or RIDDEN (a villager).
//! Every tick: the cart's position and motion, on-ground, the rider's Health. Printed per row: the tick it leaves the
//! rail (y below the rail top), the landing (first on-ground tick off the rails): its position, its cell, the
//! horizontal travel after the rail end (z - 0), the rider's health after. 26.3: off the rails `comeOffTrack`
//! (AbstractMinecart 375-387) clamps x / z to the max speed, moves, then x 0.95 in the air (all three axes); gravity
//! 0.04 before the move; on the ground x 0.5. A ridden cart's rail move is 0.75 x its stored speed (OldMinecartBehavior
//! 199), an empty one's 1.0. Found on the first run: a SLOW cart does not fall: it rests ON the rail line's support
//! block's edge (on ground at y 8, its box still over the support), so a low-speed band STRANDS at the rail end (empty
//! up to 0.20, ridden up to 0.15 in the first run). Asserted (MCTAS_RAIL_END_DROP_PLANT=k inverts assert k): (1) every
//! cart either lands on the floor or strands on the support's edge within 80 gt; (2) stranding is a low-speed band (the
//! fastest stranded speed below the slowest landed one, empty and ridden each); (3) the fastest landed row lands
//! farthest (below it the landing z is not monotone in v: the entry phase); (4)
//! every landed rider at 15 HP (floor(8.0625 - 3) = 5 on stone), every stranded one at 20; (5) no cart lands in the
//! cell straight below the last rail (z < 0); (6) the first run's band edges: empty 0.20 strands and 0.25 lands,
//! ridden 0.15 strands and 0.20 lands. v = 0.10, 0.15, 0.17, 0.20, 0.22, 0.24, 0.25, 0.30, 0.40.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const BASE: i32 = 70;

struct Out {
    label: String,
    v: f64,
    ridden: bool,
    left: Option<u32>,
    landed: Option<(u32, [f64; 3])>,
    health: Option<f64>,
    /// the last row's position (z, y above the floor) and on-ground
    last: Option<(f64, f64, bool)>,
}

impl Out {
    /// at rest ON the rail line's support edge (on ground at the rail line's support top, y 8) and never fallen
    fn stranded(&self) -> bool {
        self.landed.is_none() && self.last.is_some_and(|(z, y, og)| og && (y - 8.0).abs() < 1e-6 && z >= 0.0)
    }
}

fn nums(s: &str) -> Vec<f64> {
    s.split('[').nth(1).unwrap_or("").split(',').filter_map(|x| x.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect()
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_RAIL_END_DROP_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

pub fn rail_end_drop(version: &str) -> Result<()> {
    println!("== rail-end-drop on the official {version} server (OldMinecartBehavior): a flat rail line ending at z 0 (the last rail z -1) 8 above a stone floor (top y 0, frame y given above {BASE}); each cart summoned on the last rail at z -0.5 with its stored speed south");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("forceload add -16 -32 159 31")?;
    srv.step(20)?;
    let speeds = [0.10, 0.15, 0.17, 0.20, 0.22, 0.24, 0.25, 0.30, 0.40];
    let mut rows: Vec<(String, f64, bool, i32)> = Vec::new();
    let mut x = 0;
    for ridden in [false, true] {
        for v in speeds {
            rows.push((format!("{} v {v:.2}", if ridden { "RIDDEN" } else { "empty" }), v, ridden, x));
            x += 8;
        }
    }
    // the floor and the rail lines, one row's box at a time (fill's 32768-block limit): the support under each rail is
    // stone, the last rail at z -1
    for (_, _, _, cx) in &rows {
        srv.cmd_ok(&format!("fill {} {} -12 {} {} 20 minecraft:stone", cx - 3, BASE - 1, cx + 3, BASE - 1))?;
        srv.cmd_ok(&format!("fill {} {BASE} -12 {} {} 20 minecraft:air", cx - 3, cx + 3, BASE + 12))?;
    }
    for (_, _, _, cx) in &rows {
        srv.cmd_ok(&format!("fill {cx} {} -10 {cx} {} -1 minecraft:stone", BASE + 7, BASE + 7))?;
        srv.cmd_ok(&format!("fill {cx} {} -10 {cx} {} -1 minecraft:rail[shape=north_south]", BASE + 8, BASE + 8))?;
    }
    srv.step(5)?;
    for (i, (_, v, ridden, cx)) in rows.iter().enumerate() {
        let rider = if *ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"rdv".to_string() + &i.to_string() + "\"]}]" } else { String::new() };
        srv.cmd_ok(&format!("summon minecraft:minecart {cx}.5 {:.4} -0.5 {{Tags:[\"rdc{i}\"],Motion:[0.0d,0.0d,{v:.4}d]{rider}}}", BASE as f64 + 8.0625))?;
    }
    let mut out: Vec<Out> = rows.iter().map(|(l, v, r, _)| Out { label: l.clone(), v: *v, ridden: *r, left: None, landed: None, health: None, last: None }).collect();
    for t in 1..=80u32 {
        srv.step(1)?;
        let mut line = format!("  t{t:>2}:");
        for (i, (_, _, ridden, cx)) in rows.iter().enumerate() {
            let p = nums(&srv.cmd_long(&format!("data get entity @e[tag=rdc{i},limit=1] Pos"))?);
            if p.len() != 3 {
                continue;
            }
            let og = srv.cmd_long(&format!("data get entity @e[tag=rdc{i},limit=1] OnGround"))?.contains("1b");
            let o = &mut out[i];
            let y = p[1] - BASE as f64;
            if o.left.is_none() && y < 8.0 {
                o.left = Some(t);
            }
            if o.landed.is_none() && o.left.is_some() && og && y < 1.0 {
                o.landed = Some((t, [p[0] - *cx as f64, y, p[2]]));
            }
            o.last = Some((p[2], y, og));
            if *ridden {
                o.health = srv.cmd_long(&format!("data get entity @e[tag=rdv{i},limit=1] Health"))?.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches('f').parse().ok());
            }
            line.push_str(&format!(" | {i}: z {:.3} y {y:.3}{}", p[2], if og { " G" } else { "" }));
        }
        println!("{line}");
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    for o in &out {
        println!(
            "  {}: leaves the rail top on t{}; {}; rider {}",
            o.label,
            o.left.map(|t| t.to_string()).unwrap_or_else(|| "-".into()),
            match (o.landed, o.stranded()) {
                (Some((t, p)), _) => format!("lands t{t} at z {:.3} (cell z {}, {:.3} past the rail end), y {:.3}", p[2], p[2].floor() as i64, p[2], p[1]),
                (None, true) => format!("STRANDED on the support's edge at z {:.3} (on ground, y 8)", o.last.map(|l| l.0).unwrap_or(f64::NAN)),
                (None, false) => format!("neither landed nor stranded: last {:?}", o.last),
            },
            if o.ridden { o.health.map(|h| format!("{h:.1} HP")).unwrap_or_else(|| "gone".into()) } else { "-".into() }
        );
    }
    pin(1, out.iter().all(|o| o.landed.is_some() || o.stranded()), format!("(1) every cart either lands on the floor or strands on the rail line's support edge within 80 gt: {:?}", out.iter().map(|o| (o.label.clone(), o.landed.map(|(t, _)| t), o.stranded())).collect::<Vec<_>>()))?;
    let band = |r: bool| -> (Option<f64>, Option<f64>) {
        let s = out.iter().filter(|o| o.ridden == r && o.stranded()).map(|o| o.v).reduce(f64::max);
        let l = out.iter().filter(|o| o.ridden == r && o.landed.is_some()).map(|o| o.v).reduce(f64::min);
        (s, l)
    };
    let (se, le) = band(false);
    let (sr, lr) = band(true);
    pin(2, se.zip(le).is_none_or(|(s, l)| s < l) && sr.zip(lr).is_none_or(|(s, l)| s < l), format!("(2) stranding is a low-speed band: the fastest stranded speed is below the slowest landed one: empty {se:?} < {le:?}, ridden {sr:?} < {lr:?}"))?;
    let zs = |r: bool| out.iter().filter(|o| o.ridden == r && o.landed.is_some()).map(|o| o.landed.map(|(_, p)| p[2]).unwrap_or(f64::NAN)).collect::<Vec<_>>();
    let farthest = |r: bool| {
        let z = zs(r);
        let top = out.iter().filter(|o| o.ridden == r && o.landed.is_some()).map(|o| o.v).reduce(f64::max);
        let ztop = out.iter().find(|o| o.ridden == r && Some(o.v) == top).and_then(|o| o.landed.map(|(_, p)| p[2]));
        ztop.is_some_and(|zt| z.iter().all(|x| *x <= zt))
    };
    pin(3, farthest(false) && farthest(true), format!("(3) the fastest landed row lands farthest (the landing z is NOT monotone below it: the entry phase): empty {:?}, ridden {:?}", zs(false), zs(true)))?;
    pin(4, out.iter().filter(|o| o.ridden).all(|o| if o.landed.is_some() { o.health == Some(15.0) } else { o.health == Some(20.0) }), format!("(4) every landed rider at 15 HP (floor(8.0625 - 3) on stone), every stranded rider at 20: {:?}", out.iter().filter(|o| o.ridden).map(|o| (o.v, o.health)).collect::<Vec<_>>()))?;
    pin(5, out.iter().all(|o| o.landed.is_none_or(|(_, p)| p[2] >= 0.0)), format!("(5) no cart lands in the cell straight below the last rail (z < 0): {:?}", out.iter().map(|o| o.landed.map(|(_, p)| p[2])).collect::<Vec<_>>()))?;
    let get = |r: bool, v: f64| out.iter().find(|o| o.ridden == r && (o.v - v).abs() < 1e-9);
    pin(6, get(false, 0.20).is_some_and(Out::stranded) && get(false, 0.25).is_some_and(|o| o.landed.is_some()) && get(true, 0.15).is_some_and(Out::stranded) && get(true, 0.20).is_some_and(|o| o.landed.is_some()), "(6) the first run's band edges: empty 0.20 strands, 0.25 lands; ridden 0.15 strands, 0.20 lands".to_string())?;
    println!("  every known answer held (asserts 1..6); the stranding band: empty up to {se:?} (lands from {le:?}), ridden up to {sr:?} (lands from {lr:?})");
    Ok(())
}

/// (tickexact 2026-11-14, ironspec 02:48 / 02:49 (5), b8949c96 02:47: PIN the rail-end stranding threshold)
/// `mctas mobai rail-end-threshold`: rail-end-drop's geometry (a flat rail line ending at z 0, its last rail z -1 on a
/// stone support, an 8-block drop beyond) with every stored speed 0.15 .. 0.60 by 0.01 and five ENTRY PHASES (the cart
/// summoned on the last rail at z0 = -0.95, -0.75, -0.55, -0.35, -0.15: where in the last cell its moves fall), EMPTY and
/// RIDDEN; after 80 gt each cart is LANDED (on the floor) or STRANDED (on ground on the support's top, z >= 0). Printed:
/// per speed the phases that strand; the fastest speed any phase strands at and the slowest at which every phase lands.
/// Asserted (the first run's answers; MCTAS_RAIL_END_THRESHOLD_PLANT=k inverts assert k): (1) EMPTY: every phase strands
/// at 0.15 .. 0.21; (2) EMPTY: the fastest stranding speed is 0.26 (0.22 .. 0.26 mixed and not monotone: 0.24 / 0.25 land
/// at every sampled phase); (3) EMPTY: every phase lands at 0.27 .. 0.60; (4) RIDDEN: every phase strands at 0.15 ..
/// 0.18; (5) RIDDEN: mixed at every speed 0.19 .. 0.24, 0.24 the fastest stranding; (6) RIDDEN: every phase lands at
/// 0.25 .. 0.60. The all-phase bounds (empty 0.27, ridden 0.25) are what a designed rail end is judged against.
pub fn rail_end_threshold(version: &str) -> Result<()> {
    println!("== rail-end-threshold on the official {version} server (OldMinecartBehavior; AbstractMinecart.comeOffTrack 375-387): rail-end-drop's line (last rail z -1 on stone, an 8-block drop past z 0), speeds 0.15..0.60 by 0.01 x entry phases z0 -0.95 / -0.75 / -0.55 / -0.35 / -0.15, empty and ridden; the state after 80 gt");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    let speeds: Vec<f64> = (15..=60).map(|c| c as f64 / 100.0).collect();
    let phases = [-0.95, -0.75, -0.55, -0.35, -0.15];
    // one line per row, 3 apart along x
    let mut rows: Vec<(bool, f64, f64, i32)> = Vec::new();
    for ridden in [false, true] {
        for &v in &speeds {
            for &z0 in &phases {
                rows.push((ridden, v, z0, 3 * rows.len() as i32));
            }
        }
    }
    let xmax = rows.last().map(|r| r.3).unwrap_or(0) + 3;
    // (forceload takes at most 256 chunks a call)
    let mut fx = -16;
    while fx <= xmax + 16 {
        let fx1 = (fx + 50 * 16 - 1).min(xmax + 16);
        srv.cmd_ok(&format!("forceload add {fx} -32 {fx1} 31"))?;
        fx = fx1 + 1;
    }
    srv.step(20)?;
    let mut x0 = -2;
    while x0 <= xmax {
        let x1 = (x0 + 100).min(xmax);
        srv.cmd_ok(&format!("fill {x0} {} -12 {x1} {} 20 minecraft:stone", BASE - 1, BASE - 1))?;
        srv.cmd_ok(&format!("fill {x0} {BASE} -12 {x1} {} 12 minecraft:air", BASE + 10))?;
        x0 = x1 + 1;
    }
    for (_, _, _, cx) in &rows {
        srv.cmd_ok(&format!("fill {cx} {} -6 {cx} {} -1 minecraft:stone", BASE + 7, BASE + 7))?;
        srv.cmd_ok(&format!("fill {cx} {} -6 {cx} {} -1 minecraft:rail[shape=north_south]", BASE + 8, BASE + 8))?;
    }
    srv.step(5)?;
    for (i, (ridden, v, z0, cx)) in rows.iter().enumerate() {
        let rider = if *ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
        srv.cmd_ok(&format!("summon minecraft:minecart {cx}.5 {:.4} {z0:.4} {{Tags:[\"rt{i}\"],Motion:[0.0d,0.0d,{v:.4}d]{rider}}}", BASE as f64 + 8.0625))?;
    }
    srv.step(80)?;
    // per row: Some(true) stranded, Some(false) landed, None neither
    let mut fate: Vec<Option<bool>> = Vec::new();
    for (i, _) in rows.iter().enumerate() {
        let p = nums(&srv.cmd_long(&format!("data get entity @e[tag=rt{i},limit=1] Pos"))?);
        let og = srv.cmd_long(&format!("data get entity @e[tag=rt{i},limit=1] OnGround"))?.contains("1b");
        fate.push(if p.len() != 3 {
            None
        } else if og && (p[1] - BASE as f64 - 8.0).abs() < 1e-6 && p[2] >= 0.0 {
            Some(true)
        } else if og && p[1] - (BASE as f64) < 1.0 {
            Some(false)
        } else {
            None
        });
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    // per (ridden, speed): the number of phases that strand, of how many, and how many neither
    let mut tally: Vec<(bool, f64, usize, usize, usize)> = Vec::new();
    for ridden in [false, true] {
        println!("  {}:", if ridden { "RIDDEN" } else { "EMPTY" });
        let mut strands_max: Option<f64> = None;
        let mut all_land_from: Option<f64> = None;
        for &v in &speeds {
            let mine: Vec<(f64, Option<bool>)> = rows.iter().zip(&fate).filter(|((r, rv, _, _), _)| *r == ridden && (*rv - v).abs() < 1e-9).map(|((_, _, z0, _), f)| (*z0, *f)).collect();
            let stranded: Vec<f64> = mine.iter().filter(|(_, f)| *f == Some(true)).map(|(z0, _)| *z0).collect();
            let neither: Vec<f64> = mine.iter().filter(|(_, f)| f.is_none()).map(|(z0, _)| *z0).collect();
            tally.push((ridden, v, stranded.len(), mine.len(), neither.len()));
            if !stranded.is_empty() {
                strands_max = Some(v);
                all_land_from = None;
            } else if neither.is_empty() && all_land_from.is_none() {
                all_land_from = Some(v);
            }
            println!(
                "    v {v:.2}: {}{}",
                if stranded.is_empty() { "every phase LANDS".to_string() } else if stranded.len() == mine.len() { "every phase STRANDS".to_string() } else { format!("STRANDS from z0 {stranded:?}, the other phases land") },
                if neither.is_empty() { String::new() } else { format!("; NEITHER (still moving or elsewhere) from z0 {neither:?}") }
            );
        }
        println!("    => {}: the fastest speed any phase strands at {strands_max:?}; every phase lands from {all_land_from:?} up (to 0.60)", if ridden { "ridden" } else { "empty" });
    }
    // (G1's pattern, the first run's answers; ironspec 03:10: the all-phase bound every designed rail end is judged
    // against) MCTAS_RAIL_END_THRESHOLD_PLANT=k inverts assert k
    let at = |r: bool, lo: f64, hi: f64, f: &dyn Fn(usize, usize, usize) -> bool| tally.iter().filter(|t| t.0 == r && t.1 >= lo - 1e-9 && t.1 <= hi + 1e-9).all(|t| f(t.2, t.3, t.4));
    let fastest_strand = |r: bool| tally.iter().filter(|t| t.0 == r && t.2 >= 1).map(|t| t.1).fold(f64::NAN, f64::max);
    pin_t(1, at(false, 0.15, 0.21, &|s, n, _| s == n), "(1) EMPTY: every phase strands at 0.15 .. 0.21".into())?;
    pin_t(2, (fastest_strand(false) - 0.26).abs() < 1e-9, format!("(2) EMPTY: the fastest speed any phase strands at is 0.26 (between 0.22 and 0.26 the phases are mixed and not monotone in speed: 0.24 and 0.25 land at every sampled phase): {:.2}", fastest_strand(false)))?;
    pin_t(3, at(false, 0.27, 0.60, &|s, _, x| s == 0 && x == 0), "(3) EMPTY: every phase lands at 0.27 .. 0.60 (the all-phase bound)".into())?;
    pin_t(4, at(true, 0.15, 0.18, &|s, n, _| s == n), "(4) RIDDEN: every phase strands at 0.15 .. 0.18".into())?;
    pin_t(5, (fastest_strand(true) - 0.24).abs() < 1e-9 && at(true, 0.19, 0.24, &|s, n, _| s >= 1 && s < n), format!("(5) RIDDEN: mixed at every speed 0.19 .. 0.24 (some phases strand, some land), 0.24 the fastest stranding: {:.2}", fastest_strand(true)))?;
    pin_t(6, at(true, 0.25, 0.60, &|s, _, x| s == 0 && x == 0), "(6) RIDDEN: every phase lands at 0.25 .. 0.60 (the all-phase bound)".into())?;
    println!("  every known answer held (asserts 1..6)");
    Ok(())
}

fn pin_t(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_RAIL_END_THRESHOLD_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}
