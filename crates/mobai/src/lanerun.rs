//! (tickexact 2026-11-12, ironspec: the continuous-loop term, official) `mctas mobai lane-run`: on the
//! official 26.3 server, one lane of onechunk's module (a9a418d03, placed as seat-clear places it,
//! night held) with F3 laid (the lead pair with S2 under glowstone, F3's napper post with `--post`,
//! the glowstone floors): the lane's queue as built, a gravity queue: the front cart waiting in H
//! (x,16,0) on its closed trapdoor and the riders pressed up the slope behind it. Every `--every`
//! gt the front cart is taken out of H and its rider re-enters at the lane's top the way ironfarm's
//! bench traces the entry (off C's north edge over the drop (x,24,6), stopped by the glass wall
//! (x,24,5) at z 6.49, falling about 1.45 onto the top slope (x,22,6) from a standstill): a fresh
//! cart at rest at (x+0.6, 24.0, 6.49). No other teleports: every rider passes through every real
//! rest position. Each
//! villager has HOME on the module's bed. The brain agent names each line's mob (" @uuid"); per
//! tick the riders' blocks are read. Printed: per rest position (the rider's block) P_null and the
//! landed cells; per rider per lap its panic ticks and clears; H as a seat; MoveToTargetSink's
//! status and remainingCooldown at every panic onset; each rider's rest positions.

use crate::seatclear::{draw_seq_moving, start_world_part, wilson95_pub, Draw};
use anyhow::{Context, Result};
use mctas::vanilla::VanillaServer;
use std::collections::BTreeMap;

const DY: i32 = 80;

fn at(x: f64, y: f64, z: f64) -> String {
    format!("{} {} {}", x, y + DY as f64, z)
}

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

fn pos(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1] - DY as f64, v[2]]))
}

/// A rider's cart summoned at C moving north into the drop, the rider mounted into it.
/// A rider (re-)entering at the lane's top as ironfarm's bench traces it: off C's north edge at
/// y 24.00 over the drop (x,24,6), stopped by the glass wall (x,24,5) at z 6.49 with no horizontal
/// speed, falling about 1.45 onto the top slope (x,22,6) and rolling north from a standstill: a
/// fresh cart at (x+0.6, 24.0, 6.49) at rest, the rider mounted. False when the mount failed.
fn enter_at_top(srv: &mut VanillaServer, x: i32, k: usize, n: u32) -> Result<bool> {
    let tag = format!("lc{k}_{n}");
    let (ex, ey, ez) = (x as f64 + 0.6, 24.0, 6.49);
    srv.cmd_ok(&format!("summon minecraft:minecart {} {{Tags:[\"lane\",\"{tag}\"]}}", at(ex, ey, ez)))?;
    srv.cmd_ok(&format!("tp @e[tag=r{k},limit=1] {}", at(ex, ey, ez)))?;
    srv.cmd_ok(&format!("ride @e[tag=r{k},limit=1] mount @e[tag={tag},limit=1]"))?;
    Ok(srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Passengers"))?.contains("villager"))
}

pub fn lane_run(version: &str, module: &std::path::Path, x: i32, riders: usize, ticks: u32, every: u32, post: bool, glow: &[[i32; 3]], part: Option<&std::path::Path>, draws_out: Option<&std::path::Path>) -> Result<()> {
    println!(
        "== lane-run on the official {version} server: lane x {x} of {} as built (H at ({x},16,0), the pressed slope), {riders} riders, every {every} gt the front out of H and its rider back at the top (ironfarm's traced entry: off C over the drop, stopped by the wall at z 6.49, falling onto ({x},22,6) from a standstill), {ticks} gt; F3: the lead pair{}, {} glowstone floors",
        module.display(),
        if post { " and the napper post (1,24,-5)" } else { "" },
        glow.len()
    );
    let (mut srv, mut agent, bed) = start_world_part(version, module, post, glow, part)?;
    if let Some(p) = part {
        println!("  the part laid over the module: {}", p.display());
    }
    // the riders: villagers with HOME on the bed, each entering at the top in turn
    let mut uuids: Vec<String> = Vec::new();
    let mut failed_mounts = 0u32;
    for k in 0..riders {
        srv.cmd_ok(&format!(
            "summon minecraft:villager {} {{Tags:[\"r{k}\",\"rider\"],PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}",
            at(x as f64 + 0.6, 24.0, 6.49),
            bed[0],
            bed[1] + DY,
            bed[2]
        ))?;
        uuids.push(uuid_of(&mut srv, &format!("@e[tag=r{k},limit=1]"))?);
        if !enter_at_top(&mut srv, x, k, 0)? {
            failed_mounts += 1;
        }
        srv.step(30)?;
    }
    srv.step(100)?;
    let setup_lines = agent.drain(&srv)?;
    println!("  agent lines during the fill: {}", setup_lines.len());
    let idx: BTreeMap<String, usize> = uuids.iter().enumerate().map(|(i, u)| (u.clone(), i)).collect();
    // the activity each rider is in when the run starts (from the fill's switches)
    let mut panicking = vec![false; riders];
    for l in &setup_lines {
        if let Some(&k) = l.rsplit(" @").next().and_then(|u| idx.get(u.trim())) {
            if l.starts_with("activity ") {
                panicking[k] = l.starts_with("activity panic");
            }
        }
    }
    println!("  panicking when the run starts: {panicking:?}");
    // each rider's CANT_REACH stamp (a game time) when the run starts, from the fill's lines
    let mut stamp: Vec<Option<i64>> = vec![None; riders];
    for l in &setup_lines {
        if let Some(&k) = l.rsplit(" @").next().and_then(|u| idx.get(u.trim())) {
            if l.starts_with("set minecraft:cant_reach_walk_target_since = ") {
                stamp[k] = l["set minecraft:cant_reach_walk_target_since = ".len()..].split(' ').next().and_then(|v| v.trim_end_matches('L').parse::<i64>().ok());
            } else if l.starts_with("erase minecraft:cant_reach_walk_target_since") {
                stamp[k] = None;
            }
        }
    }
    let gt0: i64 = srv.game_time()?;
    // the largest stamp age at a calm (not panicking) tick: (age, rider, its block, tick)
    let mut calm_age: (i64, usize, [i32; 3], u32) = (i64::MIN, 0, [0, 0, 0], 0);
    // per rider: its lines (tick, line), its block by tick, its panic state and lap counters
    let mut ev: Vec<Vec<(u32, String)>> = vec![Vec::new(); riders];
    let mut blocks: Vec<BTreeMap<u32, [i32; 3]>> = vec![BTreeMap::new(); riders];
    let mut lap_panic = vec![0u32; riders];
    let mut lap_clears = vec![0u32; riders];
    let mut lap_no = vec![0u32; riders];
    let mut laps: Vec<String> = Vec::new();
    let mut onsets: BTreeMap<String, u32> = BTreeMap::new();
    let mut rests: Vec<BTreeMap<[i32; 3], u32>> = vec![BTreeMap::new(); riders];
    let mut last_pos: Vec<Option<[f64; 3]>> = vec![None; riders];
    let mut still = vec![0u32; riders];
    let mut entries = 1u32;
    let mut pulse_off: Option<u32> = None;
    let (mut unmatched, mut unmatched_sample) = (0u32, Vec::<String>::new());
    for t in 1..=ticks {
        srv.step(1)?;
        for l in agent.drain(&srv)? {
            let Some(u) = l.rsplit(" @").next() else { continue };
            let Some(&k) = idx.get(u.trim()) else {
                unmatched += 1;
                if unmatched_sample.len() < 3 {
                    unmatched_sample.push(l.clone());
                }
                continue;
            };
            if l.starts_with("activity ") {
                let p = l.starts_with("activity panic");
                if p && !panicking[k] {
                    let s = l.split(" (MoveToTargetSink ").nth(1).and_then(|s| s.split(')').next()).unwrap_or("?").to_string();
                    *onsets.entry(s).or_default() += 1;
                }
                panicking[k] = p;
            }
            if l.starts_with("erase minecraft:cant_reach_walk_target_since") && !l.contains("(absent)") && l.contains("by MoveToTargetSink") {
                lap_clears[k] += 1;
            }
            if l.starts_with("set minecraft:cant_reach_walk_target_since = ") {
                stamp[k] = l["set minecraft:cant_reach_walk_target_since = ".len()..].split(' ').next().and_then(|v| v.trim_end_matches('L').parse::<i64>().ok());
            } else if l.starts_with("erase minecraft:cant_reach_walk_target_since") {
                stamp[k] = None;
            }
            ev[k].push((t, l));
        }
        for k in 0..riders {
            if !panicking[k] {
                if let Some(s) = stamp[k] {
                    let age = gt0 + t as i64 - s;
                    if age > calm_age.0 {
                        calm_age = (age, k, blocks[k].values().next_back().copied().unwrap_or([0, 0, 0]), t);
                    }
                }
            }
        }
        for k in 0..riders {
            if panicking[k] {
                lap_panic[k] += 1;
            }
            let p = pos(&mut srv, &format!("@e[tag=r{k},limit=1]"))?;
            if let Some(q) = p {
                let b = [q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32];
                blocks[k].insert(t, b);
                // at rest: within 0.01 for 10 ticks
                if last_pos[k].is_some_and(|o| (o[0] - q[0]).abs() + (o[1] - q[1]).abs() + (o[2] - q[2]).abs() < 0.01) {
                    still[k] += 1;
                    if still[k] == 10 {
                        *rests[k].entry(b).or_default() += 1;
                    }
                } else {
                    still[k] = 0;
                }
                last_pos[k] = Some(q);
            }
        }
        if pulse_off == Some(t) {
            // (the rail may already have re-settled unpowered on an update: then nothing changes)
            srv.cmd(&format!("setblock {x} {} 1 minecraft:powered_rail[shape=ascending_south,powered=false] strict", 17 + DY))?;
            pulse_off = None;
        }
        if t % every == 0 {
            // the station's turn: the cart in H (on its closed trapdoor (x,16,0)) leaves, its rider
            // back at the top; then R_1 (x,17,1) is powered for 12 gt (the release pulse, set
            // without block updates so H's trapdoor stays shut) and the front rolls off it into H
            let mut front = None;
            for k in 0..riders {
                // the rider of the cart on H's trapdoor: its block (x,16,0) (seat at cart + 0.5)
                if blocks[k].get(&t).is_some_and(|b| b[0] == x && b[2] == 0 && (16..=17).contains(&b[1])) {
                    front = Some(k);
                }
            }
            if let Some(k) = front {
                laps.push(format!("r{k} lap {}: panic {} gt, clears {}", lap_no[k], lap_panic[k], lap_clears[k]));
                lap_no[k] += 1;
                lap_panic[k] = 0;
                lap_clears[k] = 0;
                srv.cmd_ok(&format!("ride @e[tag=r{k},limit=1] dismount"))?;
                srv.cmd_ok(&format!("kill @e[type=minecraft:minecart,tag=lane,x={},y={},z=0,dx=0,dy=0,dz=0]", x, 16 + DY))?;
                if !enter_at_top(&mut srv, x, k, entries)? {
                    failed_mounts += 1;
                }
                entries += 1;
            }
            srv.cmd_ok(&format!("setblock {x} {} 1 minecraft:powered_rail[shape=ascending_south,powered=true] strict", 17 + DY))?;
            pulse_off = Some(t + 12);
        }
    }
    // per rest position: the draws by the rider's block at the draw tick
    let mut by_block: BTreeMap<[i32; 3], (u32, u32, BTreeMap<([i32; 3], &'static str), u32>)> = BTreeMap::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut draw_log: Vec<String> = Vec::new();
    for k in 0..riders {
        let seq: Vec<Draw> = draw_seq_moving(&ev[k], 0, [0, 0, 0], &blocks[k]);
        for (j, d) in seq.iter().enumerate() {
            // the per-draw log: tick, the rider's cell, the nearest hostile then, the target cell,
            // the outcome, and the first path the sink computed for it (its end and whether whole)
            let b = blocks[k].get(&d.t).copied().unwrap_or([0, 0, 0]);
            let next_t = seq.get(j + 1).map(|n| n.t).unwrap_or(u32::MAX);
            let hostile = ev[k]
                .iter()
                .filter(|(t, l)| *t <= d.t && l.contains("minecraft:nearest_hostile"))
                .last()
                .map(|(_, l)| if l.starts_with("erase") { "none".to_string() } else { l.split(" = ").nth(1).unwrap_or("").split(" by ").next().unwrap_or("").to_string() })
                .unwrap_or_else(|| "none seen".into());
            let path = ev[k]
                .iter()
                .find(|(t, l)| *t >= d.t && *t < next_t && l.starts_with("set minecraft:path"))
                .map(|(t, l)| {
                    let p = l.split(" = ").nth(1).unwrap_or("").split(" by ").next().unwrap_or("");
                    format!("+{} {} ({})", t - d.t, if p.contains("reach true") { "complete" } else { "PARTIAL" }, p)
                })
                .unwrap_or_else(|| "never pathed".into());
            draw_log.push(format!(
                "t{} r{k} rider cell ({},{},{}) nearest hostile {hostile} | {}",
                d.t,
                b[0],
                b[1],
                b[2],
                match d.cell {
                    None => "null".to_string(),
                    Some(c) => format!("target ({},{},{}) {} | path {path}", b[0] + c[0], b[1] + c[1], b[2] + c[2], d.outcome),
                }
            ));
        }
    }
    if let Some(out) = draws_out {
        let mut text = String::new();
        for l in &draw_log {
            text.push_str(l);
            text.push('\n');
        }
        std::fs::write(out, text)?;
        println!("  the per-draw log: {} draws in {}", draw_log.len(), out.display());
    }
    for k in 0..riders {
        let seq: Vec<Draw> = draw_seq_moving(&ev[k], 0, [0, 0, 0], &blocks[k]);
        for d in &seq {
            let b = blocks[k].get(&d.t).copied().unwrap_or([0, 0, 0]);
            let e = by_block.entry(b).or_insert((0, 0, BTreeMap::new()));
            e.0 += 1;
            match d.cell {
                None => e.1 += 1,
                Some(c) => *e.2.entry((c, d.outcome)).or_default() += 1,
            }
            if d.outcome == "open" {
                // what followed the draw: its path (computed or not, whole or partial), what erased
                // WALK_TARGET, the activity, where the rider went; the sink at the panic onsets
                let after: Vec<String> = ev[k]
                    .iter()
                    .filter(|(t, l)| *t >= d.t && *t <= d.t + 120 && (l.contains("walk_target") || l.contains("minecraft:path") || l.starts_with("activity") || l.contains("cant_reach")))
                    .map(|(t, l)| format!("+{} {}", t - d.t, l.split(" @").next().unwrap_or(l)))
                    .collect();
                let moved: Vec<String> = (d.t..=d.t + 120).step_by(20).filter_map(|t| blocks[k].get(&t).map(|b| format!("+{} ({},{},{})", t - d.t, b[0], b[1], b[2]))).collect();
                unresolved.push(format!(
                    "r{k} drew at t{} from ({},{},{}) the cell {:?}; the next 120 gt: {}; its blocks: {}",
                    d.t,
                    b[0],
                    b[1],
                    b[2],
                    d.cell,
                    if after.is_empty() { "nothing".to_string() } else { after.join(" | ") },
                    moved.join(" ")
                ));
            }
        }
    }
    for u in &unresolved {
        println!("  unresolved draw: {u}");
    }
    for (b, (n, nulls, cells)) in &by_block {
        let (pl, ph) = wilson95_pub(*nulls, *n);
        println!(
            "  rest block ({},{},{}){}: {n} draws, P_null {:.4} (95% {pl:.4}..{ph:.4}); landed (cell relative, outcome: draws): {}",
            b[0],
            b[1],
            b[2],
            if b[0] == x && b[2] == 0 && (16..=17).contains(&b[1]) { " (H: the rider of the cart on H's trapdoor)" } else { "" },
            *nulls as f64 / (*n).max(1) as f64,
            cells.iter().map(|((c, o), k)| format!("({},{},{}) {o}: {k}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", ")
        );
    }
    println!("  laps (per rider: panic ticks and clears): {}", laps.join("; "));
    println!("  mounts that failed: {failed_mounts}");
    if calm_age.0 > i64::MIN {
        println!(
            "  the largest CANT_REACH stamp age at a calm (not panicking) tick: {} gt (r{} at ({},{},{}), t{})",
            calm_age.0, calm_age.1, calm_age.2[0], calm_age.2[1], calm_age.2[2], calm_age.3
        );
    } else {
        println!("  no calm tick with a stamp set");
    }
    let mut partial: Vec<String> = Vec::new();
    let mut open = 0u32;
    for (b, (_, _, cells)) in &by_block {
        for ((c, o), k) in cells {
            if *o == "partial" {
                partial.push(format!("at ({},{},{}) the cell ({},{},{}): {k}", b[0], b[1], b[2], c[0], c[1], c[2]));
            } else if *o == "open" {
                open += k;
            }
        }
    }
    println!("  partial-path draws over every position: {} ({}); draws left unresolved (still held when the rider moved on or the run ended): {open}", partial.len(), partial.join("; "));
    println!("  agent lines not naming a rider: {unmatched} (e.g. {:?}); the riders: {uuids:?}", unmatched_sample);
    for k in 0..riders {
        let mut acts: BTreeMap<String, u32> = BTreeMap::new();
        for (_, l) in &ev[k] {
            if let Some(a) = l.strip_prefix("activity ") {
                *acts.entry(a.split(' ').next().unwrap_or("").to_string()).or_default() += 1;
            }
        }
        println!("  r{k}: {} lines; activity switches {acts:?}", ev[k].len());
    }
    println!("  MoveToTargetSink at every panic onset (status, remainingCooldown: onsets): {}", onsets.iter().map(|(s, n)| format!("{s}: {n}")).collect::<Vec<_>>().join("; "));
    for k in 0..riders {
        println!(
            "  r{k} rest positions (block: times): {}; HOME at the end: {}",
            rests[k].iter().map(|(b, n)| format!("({},{},{}) {n}", b[0], b[1], b[2])).collect::<Vec<_>>().join(", "),
            srv.cmd_long(&format!("data get entity @e[tag=r{k},limit=1] Brain.memories.\"minecraft:home\""))?.contains("entity data")
        );
    }
    let _ = std::fs::remove_dir_all(&srv.dir).context("remove the world")?;
    Ok(())
}
