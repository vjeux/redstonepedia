//! The fast liveness ring (ironspec Nov 11 18:02, 7b90c3f1's latency targets): a sealed loop of 24
//! rail cells (a 7 x 7 square: 4 curved corners, 20 straights) with 2 empty carts half a lap apart and
//! 8 detector rails every 3 cells, so a cart reaches a detector about every 3.75 gt. Only a detector's
//! RISING edge (a cart arriving) is evidence that carts move: its falling edge comes up to 20 gt after
//! the cart leaves even when entities are frozen. ALIVE = the 8 rising edges ORed and stretched;
//! the bench measures the gaps between rising edges over a long run (every gap must stay under the
//! stretcher: no false drop), each detector's own spacing (it must fall between carts), the carts'
//! lap, and a positive control: the carts gone at a tick, so the edges stop and ALIVE drops.

use mctas::design::{Design, EntitySpec};
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;

/// The loop's cells in travel order from the corner (0, 1, 0), counterclockwise seen from above (+x
/// along z 0, +z along x 6, -x along z 6, -z along x 0), and each cell's role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Corner,
    Detector,
    Powered,
}

pub fn loop_cells() -> Vec<([i32; 3], Role, &'static str)> {
    let mut v = Vec::new();
    let det = [1usize, 4, 7, 10, 13, 16, 19, 22];
    for i in 0..24usize {
        let (x, z, shape) = match i {
            0 => (0, 0, "south_east"),
            1..=5 => (i as i32, 0, "east_west"),
            6 => (6, 0, "south_west"),
            7..=11 => (6, i as i32 - 6, "north_south"),
            12 => (6, 6, "north_west"),
            13..=17 => (18 - i as i32, 6, "east_west"),
            18 => (0, 6, "north_east"),
            _ => (0, 24 - i as i32, "north_south"),
        };
        let role = if i % 6 == 0 { Role::Corner } else if det.contains(&i) { Role::Detector } else { Role::Powered };
        v.push(([x, 1, z], role, shape));
    }
    v
}

/// The ring's design: rails at y 1 on stone (redstone blocks under the powered rails), `carts` empty
/// carts (1 or 2; the second half a lap behind) moving along the loop at `speed` b/t. `sealed`: the
/// ring closed against mobs (its empty carts board any mob they touch above 0.1 b/t): the 5 x 5
/// interior solid at y 1, a wall round the outside at y 1, a roof over every rail at y 2 (an empty
/// cart is 0.7 tall), so no mob can stand, spawn or reach a cart.
pub fn design(speed: f64, carts: usize, sealed: bool) -> Design {
    let mut d = Design { name: format!("liveness-ring-8x{carts}{}", if sealed { "-sealed" } else { "" }), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    for (c, role, shape) in loop_cells() {
        let (rail, under) = match role {
            Role::Corner => (format!("minecraft:rail[shape={shape}]"), "minecraft:stone"),
            Role::Detector => (format!("minecraft:detector_rail[shape={shape},powered=false]"), "minecraft:stone"),
            Role::Powered => (format!("minecraft:powered_rail[shape={shape},powered=true]"), "minecraft:redstone_block"),
        };
        d.blocks.insert(c, rail);
        d.blocks.insert([c[0], 0, c[2]], under.into());
        if sealed {
            d.blocks.insert([c[0], 2, c[2]], "minecraft:stone".into());
        }
    }
    if sealed {
        for x in -1..=7 {
            for z in -1..=7 {
                let interior = (1..=5).contains(&x) && (1..=5).contains(&z);
                let outside = x == -1 || x == 7 || z == -1 || z == 7;
                if interior || outside {
                    d.blocks.insert([x, 1, z], "minecraft:stone".into());
                }
            }
        }
    }
    // (cell 2 runs +x; the second cart 13.5 cells behind it (12 is half the lap, the extra 1.5 cells
    // half a detector spacing), so its rising edges fall between the first's: an edge every half
    // spacing, each detector a cart every half lap)
    d.entities.push(EntitySpec { kind: "minecraft:minecart".into(), pos: [2.5, 1.0625, 0.5], nbt: Some(format!("{{Motion:[{speed:.4}d,0.0d,0.0d]}}")) });
    if carts >= 2 {
        d.entities.push(EntitySpec { kind: "minecraft:minecart".into(), pos: [3.0, 1.0625, 6.5], nbt: Some(format!("{{Motion:[{:.4}d,0.0d,0.0d]}}", -speed)) });
    }
    d.ports.insert("output".into(), [3, 3, 3]);
    d
}

/// The bench: `ticks` gt with the carts spawned, every detector's rising edges; `stretch` gt the
/// ALIVE stretcher; `gone_at`: the carts removed at that tick (the positive control).
pub fn bench(spec: &FarmSpec, ticks: u32, stretch: i64, gone_at: Option<u32>) -> Result<Vec<String>, String> {
    let mut s = spec.clone();
    let ents = std::mem::take(&mut s.entities);
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 20, WalkMode::Random) };
    let mut run = Run::start(&s, &ro)?;
    while run.settle_tick() {}
    let mut ids: Vec<u32> = Vec::new();
    for (ty, pos, nbt) in &ents {
        ids.push(redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?);
    }
    let dets: Vec<BlockPos> = loop_cells().iter().filter(|c| c.1 == Role::Detector).map(|c| BlockPos::new(c.0[0], c.0[1], c.0[2])).collect();
    let mut prev = vec![false; dets.len()];
    let mut last_edge: Option<i64> = None;
    let mut per_last: Vec<Option<i64>> = vec![None; dets.len()];
    let mut gaps: std::collections::BTreeMap<i64, u64> = std::collections::BTreeMap::new();
    let (mut per_min, mut per_max) = (i64::MAX, 0i64);
    let mut worst: Option<(i64, i64, usize)> = None;
    let mut false_drops = 0u64;
    let mut edges = 0u64;
    let mut edges_after_gone = 0u64;
    let mut gone_t: Option<i64> = None;
    let mut speed_sum = 0.0f64;
    let mut speed_n = 0u64;
    // (G1 Nov 11 19:43: each cart's own lap, from its entries into the corner cell (0,1,0))
    let mut in_corner: Vec<bool> = vec![false; ids.len()];
    let mut corner_at: Vec<Option<i64>> = vec![None; ids.len()];
    let mut laps: Vec<(i64, i64)> = vec![(i64::MAX, 0); ids.len()];
    for t in 0..ticks as i64 {
        if gone_at.is_some_and(|g| g as i64 == t) {
            for id in &ids {
                if let Some(i) = redsim::entity::index_of(&run.world, *id) {
                    run.world.entities.list[i].removed = true;
                }
            }
            gone_t = Some(t);
        }
        run.tick();
        for (k, p) in dets.iter().enumerate() {
            let on = format!("{}", run.world.get(*p)).contains("powered=true");
            if on && !prev[k] {
                edges += 1;
                if gone_t.is_some() {
                    edges_after_gone += 1;
                }
                if let Some(l) = last_edge {
                    let g = t - l;
                    *gaps.entry(g).or_default() += 1;
                    if g > stretch && gone_t.is_none() {
                        false_drops += 1;
                    }
                    if worst.is_none_or(|w| g > w.0) {
                        worst = Some((g, t, k));
                    }
                }
                if let Some(l) = per_last[k] {
                    per_min = per_min.min(t - l);
                    per_max = per_max.max(t - l);
                }
                per_last[k] = Some(t);
                last_edge = Some(t);
            }
            prev[k] = on;
        }
        for (ci, id) in ids.iter().enumerate() {
            if let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id && !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
                speed_sum += e.delta_movement.x.hypot(e.delta_movement.z);
                speed_n += 1;
                let at = e.pos.x.floor() as i32 == 0 && e.pos.z.floor() as i32 == 0;
                if at && !in_corner[ci] {
                    if let Some(p) = corner_at[ci] {
                        laps[ci].0 = laps[ci].0.min(t - p);
                        laps[ci].1 = laps[ci].1.max(t - p);
                    }
                    corner_at[ci] = Some(t);
                }
                in_corner[ci] = at;
            }
        }
    }
    let mut out = Vec::new();
    let hist: Vec<String> = gaps.iter().map(|(g, n)| format!("{g}:{n}")).collect();
    out.push(format!("liveness ring (8 detectors, {} cart(s), 24 cells): {ticks} gt, {edges} rising edges; gaps between rising edges (gt:count): {}", ids.len(), hist.join(" ")));
    out.push(format!("the largest gap {} gt{}; false drops with a {stretch}-gt stretcher (a gap over it before the control): {false_drops}", worst.map_or(0, |w| w.0), worst.map_or(String::new(), |w| format!(" (at t {}, detector {} at {:?})", w.1, w.2, dets[w.2]))));
    let lap = per_max * ids.len() as i64;
    let lap_s: Vec<String> = laps.iter().enumerate().map(|(i, (a, b))| if *a == i64::MAX { format!("cart {i} -") } else { format!("cart {i} {a}..{b} gt") }).collect();
    let pmin = if per_min == i64::MAX { 0 } else { per_min };
    out.push(format!("each detector's own spacing between rising edges: {pmin}..{per_max} gt (bar: at least 21 gt, the detector's 20-gt re-check + 1, so it falls between carts: {}); each cart's lap: {} ({:.3} b/t at the shortest)", if pmin >= 21 { "PASS" } else { "FAIL" }, lap_s.join(", "), laps.iter().map(|l| l.0).filter(|l| *l != i64::MAX).min().map_or(0.0, |l| 24.0 / l as f64)));
    let _ = (speed_sum, speed_n, lap);
    if let Some(g) = gone_t {
        let drop = last_edge.map(|l| l + stretch);
        out.push(format!("positive control: the carts removed at t {g}; rising edges after it {edges_after_gone}; the last rising edge t {}; ALIVE drops at t {} (latency {} gt after the carts stopped)", last_edge.map_or("-".into(), |l| l.to_string()), drop.map_or("-".into(), |d| d.to_string()), drop.map_or("-".into(), |d| (d - g).to_string())));
    }
    Ok(out)
}
