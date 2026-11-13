//! (9fc3155f, ironspec Nov 11 20:55: tiling rules (a)-(g)) The cross-module margins of a design
//! tiled with a copy of itself at an offset (optionally mirrored in z): (a) a golem spawned on one
//! module's pad sensed by the other's riders (G1's per-axis box test, ironspec Nov 12 12:48: the
//! rider's box inflated 16 meets the golem's iff |dx|, |dz| < 17.0 and the golem's feet < 17.95
//! above / < 18.7 below the rider's; the earlier worst-case follow range 17.84, |dx|, |dz| < 18.84
//! and < 19.79 above / < 20.54 below, kept as the conservative bound, opt-in), (b) a summon in one module
//! marking the other's riders (|dx|, |dz| < 10.6 and feet within 11.95), (e) one module's zombies
//! within 6.05 of the other's riders, and within 8 of its napper's sleep and stand-up points, (f)
//! the beds' distance (a neighbour's bed is a village POI within 8 sections: no bar here), (g) the
//! chunk alignment. Positions: every rider position along every rail cell (3 points a cell, the
//! cart's feet: a villager sits at the cart's y), the napper on foot (each bed's sleep point and
//! stand-up cells) and, separately, the design's own placed riders (the waiting places); the
//! summoners the three placed riders whose summon searches find the most golem cells (the stand);
//! the golem cells the cells the summoners' searches spawn on; the zombies as seated.

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::mob::spawn;
use redsim::entity::Vec3;
use redsim::pos::BlockPos;

/// One module's point sets.
pub struct Sets {
    /// Every rider position along every rail cell (feet).
    pub riders: Vec<Vec3>,
    /// The design's placed riders (feet): the waiting places.
    pub waiting: Vec<Vec3>,
    /// The stand's summoners (feet).
    pub summoners: Vec<Vec3>,
    /// The golem spawn cells' feet centres.
    pub golems: Vec<Vec3>,
    /// The zombies (feet, seated).
    pub zombies: Vec<Vec3>,
    /// The beds' heads.
    pub beds: Vec<BlockPos>,
    /// The napper on foot: each bed's sleep point and stand-up cells (feet); also in `riders`.
    pub napper: Vec<Vec3>,
}

/// The sets of `spec`'s module (its world built and settled).
pub fn sets(spec: &FarmSpec) -> Result<Sets, String> {
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &run.world;
    let mut riders = Vec::new();
    for (c, b) in &spec.blocks {
        let s = b.to_string();
        if s.contains("rail") {
            for t in [0.1, 0.5, 0.9] {
                riders.push(crate::fleecheck::along_rail(*c, &s, t));
            }
        }
    }
    let mut waiting: Vec<Vec3> = spec
        .entities
        .iter()
        .filter(|(ty, _, nbt)| ty.ends_with("minecart") && nbt.as_deref().is_some_and(|n| n.contains("minecraft:villager")))
        .map(|(_, p, _)| Vec3::new(p[0], p[1], p[2]))
        .collect();
    waiting.sort_by(|a, b| a.y.total_cmp(&b.y));
    // (the summoners by role, G1's review: the three placed riders whose summon searches find the
    // most golem spawn cells (the stand over its pad), not the three lowest (wrong for F3, whose
    // queue is below the stand))
    let mut scored: Vec<(usize, Vec3, Vec<Vec3>)> = waiting
        .iter()
        .map(|s| {
            let start = BlockPos::new(s.x.floor() as i32, s.y.floor() as i32, s.z.floor() as i32);
            let cells: Vec<Vec3> = spawn::golem_spawn_cells_from(w, start).into_iter().filter(|(_, ok)| ok.is_ok()).map(|(c, _)| Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5)).collect();
            (cells.len(), *s, cells)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.y.total_cmp(&b.1.y)));
    let summoners: Vec<Vec3> = scored.iter().filter(|(n, _, _)| *n > 0).take(3).map(|(_, s, _)| *s).collect();
    let mut golems: Vec<Vec3> = Vec::new();
    for (_, _, cells) in scored.iter().filter(|(n, _, _)| *n > 0).take(3) {
        for g in cells {
            if !golems.iter().any(|h| (h.x - g.x).abs() < 1e-9 && (h.y - g.y).abs() < 1e-9 && (h.z - g.z).abs() < 1e-9) {
                golems.push(*g);
            }
        }
    }
    let zombies = crate::fleecheck::zombies_seated(w, spec);
    let beds: Vec<BlockPos> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("_bed[") && b.to_string().contains("part=head")).map(|(p, _)| *p).collect();
    // (the napper on foot, G1's review: its sleep point (the bed head, a sleeping villager at the
    // bed's y + 0.6875, `setPosToBed`) and its stand-up cells (the head's four sides at floor level;
    // `BedBlock.findStandUpPosition` also tries the cells round the foot and one up, which this
    // module's bed stop walls off) as rider positions)
    let mut napper = Vec::new();
    for b in &beds {
        napper.push(Vec3::new(b.x as f64 + 0.5, b.y as f64 + 0.6875, b.z as f64 + 0.5));
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let c = BlockPos::new(b.x + dx, b.y, b.z + dz);
            if w.get(c).is_air() && w.get(BlockPos::new(c.x, c.y + 1, c.z)).is_air() {
                napper.push(Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5));
            }
        }
    }
    riders.extend(napper.iter().copied());
    Ok(Sets { riders, waiting, summoners, golems, zombies, beds, napper })
}

/// A copy's point: mirrored in z (a cell z to -z: a point z to 1 - z) when `mirror_z`, then moved
/// by `at`.
pub fn place(p: Vec3, at: [i32; 3], mirror_z: bool) -> Vec3 {
    let z = if mirror_z { 1.0 - p.z } else { p.z };
    Vec3::new(p.x + at[0] as f64, p.y + at[1] as f64, z + at[2] as f64)
}

/// The sense margin of a golem (feet centre `g`) to a rider (feet `v`), G1's per-axis box test
/// (ironspec Nov 12 12:48, the tool's default): a villager senses an entity whose box meets its own
/// box inflated by 16 on every axis, so the rider (0.6 wide, 1.95 tall) senses the golem (1.4 wide,
/// 2.7 tall) iff every per-axis box gap is under 16: |dx|, |dz| < 17.0, the golem's feet less than
/// 17.95 above the rider's and less than 18.7 below them; negative when the rider senses it.
pub fn sight_margin(g: Vec3, v: Vec3) -> f64 {
    ((g.x - v.x).abs() - 17.0).max((g.z - v.z).abs() - 17.0).max((g.y - v.y) - 17.95).max((v.y - g.y) - 18.7)
}

/// The conservative bound for (a), opt-in (--conservative): the worst-case follow range 17.84 (the
/// earlier metric, superseded by G1's box test: |dx|, |dz| < 18.84, the golem's feet < 19.79 above /
/// < 20.54 below the rider's); negative inside it.
pub fn sight_margin_conservative(g: Vec3, v: Vec3) -> f64 {
    ((g.x - v.x).abs() - 18.84).max((g.z - v.z).abs() - 18.84).max((v.y - g.y) - 20.54).max((g.y - v.y) - 19.79)
}

/// The summon-mark margin of a summoner (feet `s`) to a rider (feet `v`): negative when marked.
pub fn mark_margin(s: Vec3, v: Vec3) -> f64 {
    ((s.x - v.x).abs() - 10.6).max((s.z - v.z).abs() - 10.6).max((v.y - s.y).abs() - 11.95)
}

/// The least margin over the pairs (a source of one module, a rider of the other), with the pair.
pub fn least(src: &[Vec3], dst: &[Vec3], f: impl Fn(Vec3, Vec3) -> f64) -> Option<(f64, Vec3, Vec3)> {
    let mut best: Option<(f64, Vec3, Vec3)> = None;
    for s in src {
        for d in dst {
            let m = f(*s, *d);
            if best.is_none_or(|b| m < b.0) {
                best = Some((m, *s, *d));
            }
        }
    }
    best
}

/// One tiling candidate's line: the copy at `at` (mirrored in z when `mirror_z`) against the
/// original, both ways; (a) by G1's box test, or by the conservative bound when `conservative`.
pub fn candidate(a: &Sets, at: [i32; 3], mirror_z: bool, conservative: bool) -> String {
    let mv = |v: &Vec<Vec3>| -> Vec<Vec3> { v.iter().map(|p| place(*p, at, mirror_z)).collect() };
    let (br, bw, bs, bg, bz) = (mv(&a.riders), mv(&a.waiting), mv(&a.summoners), mv(&a.golems), mv(&a.zombies));
    let both = |src_a: &[Vec3], dst_b: &[Vec3], src_b: &[Vec3], dst_a: &[Vec3], f: &dyn Fn(Vec3, Vec3) -> f64| -> Option<(f64, Vec3, Vec3)> {
        let x = least(src_a, dst_b, f);
        let y = least(src_b, dst_a, f);
        match (x, y) {
            (Some(p), Some(q)) => Some(if p.0 <= q.0 { p } else { q }),
            (p, q) => p.or(q),
        }
    };
    let fmt = |r: Option<(f64, Vec3, Vec3)>| match r {
        Some((m, s, d)) => format!("{m:+.2} ({}: {:.1},{:.1},{:.1} -> {:.1},{:.1},{:.1})", if m < 0.0 { "FAIL" } else { "ok" }, s.x, s.y, s.z, d.x, d.y, d.z),
        None => "-".into(),
    };
    let zdist = |z: Vec3, v: Vec3| ((z.x - v.x).powi(2) + (z.y - v.y).powi(2) + (z.z - v.z).powi(2)).sqrt() - 6.05;
    let sense: &dyn Fn(Vec3, Vec3) -> f64 = if conservative { &sight_margin_conservative } else { &sight_margin };
    let sight_all = both(&a.golems, &br, &bg, &a.riders, sense);
    let sight_wait = both(&a.golems, &bw, &bg, &a.waiting, sense);
    let mark_all = both(&a.summoners, &br, &bs, &a.riders, &mark_margin);
    let mark_wait = both(&a.summoners, &bw, &bs, &a.waiting, &mark_margin);
    let zomb = both(&a.zombies, &br, &bz, &a.riders, &zdist);
    // ((e) for the napper, G1's review: a neighbour's zombie within 8 of the napper's sleep point or
    // a stand-up cell, distance only (sight needs the composite world: the flee-check screen))
    let bn = mv(&a.napper);
    let nap8 = both(&a.zombies, &bn, &bz, &a.napper, &|z: Vec3, v: Vec3| ((z.x - v.x).powi(2) + (z.y - v.y).powi(2) + (z.z - v.z).powi(2)).sqrt() - 8.0);
    let bed = a.beds.iter().flat_map(|p| {
        let q = place(Vec3::new(p.x as f64, p.y as f64, p.z as f64), at, mirror_z);
        a.beds.iter().map(move |r| ((q.x - r.x as f64).powi(2) + (q.y - r.y as f64).powi(2) + (q.z - r.z as f64).powi(2)).sqrt())
    }).fold(f64::MAX, f64::min);
    format!(
        "at {},{},{}{}: (a) golem {}, every rider position {}; the waiting riders {} | (b) summon mark, every rider position {}; the waiting riders {} | (e) zombie to rider (6.05) {}; to the napper's sleep and stand-up points (8, distance only) {} | (f) beds {} apart (a neighbour's bed within 8 sections, 128 blocks, is a village POI to this module's riders: no bar here; ironspec's condition is the composite flee-check with the neighbours' beds in every ticket state and no path to them) | (g) chunk-aligned {}",
        at[0],
        at[1],
        at[2],
        if mirror_z { " mirrored in z" } else { "" },
        if conservative { "sight, the conservative bound (the worst-case follow range 17.84: |dx|, |dz| < 18.84, feet < 19.79 above / < 20.54 below)" } else { "sensed (G1's per-axis box test: every box gap < 16, |dx|, |dz| < 17.0, the golem's feet < 17.95 above / < 18.7 below the rider's)" },
        fmt(sight_all),
        fmt(sight_wait),
        fmt(mark_all),
        fmt(mark_wait),
        fmt(zomb),
        fmt(nap8),
        if bed == f64::MAX { "-".into() } else { format!("{bed:.1}") },
        if at[0].rem_euclid(16) == 0 && at[2].rem_euclid(16) == if mirror_z { 15 } else { 0 } { "yes" } else { "no" }
    )
}
