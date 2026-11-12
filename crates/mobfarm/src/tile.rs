//! (9fc3155f, ironspec Nov 11 20:55: tiling rules (a)-(g)) The cross-module margins of a design
//! tiled with a copy of itself at an offset (optionally mirrored in z): (a) a golem spawned on one
//! module's pad sensed by the other's riders (the worst-case follow range 17.84: |dx|, |dz| < 18.84
//! and the golem's feet < 19.79 above / < 20.54 below the rider's), (b) a summon in one module
//! marking the other's riders (|dx|, |dz| < 10.6 and feet within 11.95), (e) one module's zombies
//! within 6.05 of the other's riders, (f) the beds' distance, (g) the chunk alignment. Positions:
//! every rider position along every rail cell (3 points a cell, the cart's feet: a villager sits at
//! the cart's y) and, separately, the design's own placed riders (the waiting places); the
//! summoners the three lowest placed riders (the stand); the golem cells the cells the summoners'
//! searches spawn on; the zombies as seated.

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
    let summoners: Vec<Vec3> = waiting.iter().take(3).copied().collect();
    let mut golems: Vec<Vec3> = Vec::new();
    for s in &summoners {
        let start = BlockPos::new(s.x.floor() as i32, s.y.floor() as i32, s.z.floor() as i32);
        for (cell, ok) in spawn::golem_spawn_cells_from(w, start) {
            let g = Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5);
            if ok.is_ok() && !golems.iter().any(|h| (h.x - g.x).abs() < 1e-9 && (h.y - g.y).abs() < 1e-9 && (h.z - g.z).abs() < 1e-9) {
                golems.push(g);
            }
        }
    }
    let zombies = crate::fleecheck::zombies_seated(w, spec);
    let beds: Vec<BlockPos> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("_bed[") && b.to_string().contains("part=head")).map(|(p, _)| *p).collect();
    Ok(Sets { riders, waiting, summoners, golems, zombies, beds })
}

/// A copy's point: mirrored in z (a cell z to -z: a point z to 1 - z) when `mirror_z`, then moved
/// by `at`.
pub fn place(p: Vec3, at: [i32; 3], mirror_z: bool) -> Vec3 {
    let z = if mirror_z { 1.0 - p.z } else { p.z };
    Vec3::new(p.x + at[0] as f64, p.y + at[1] as f64, z + at[2] as f64)
}

/// The sight margin of a golem (feet centre `g`) to a rider (feet `v`): negative when the rider
/// senses it.
pub fn sight_margin(g: Vec3, v: Vec3) -> f64 {
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
/// original, both ways.
pub fn candidate(a: &Sets, at: [i32; 3], mirror_z: bool) -> String {
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
    let sight_all = both(&a.golems, &br, &bg, &a.riders, &sight_margin);
    let sight_wait = both(&a.golems, &bw, &bg, &a.waiting, &sight_margin);
    let mark_all = both(&a.summoners, &br, &bs, &a.riders, &mark_margin);
    let mark_wait = both(&a.summoners, &bw, &bs, &a.waiting, &mark_margin);
    let zomb = both(&a.zombies, &br, &bz, &a.riders, &zdist);
    let bed = a.beds.iter().flat_map(|p| {
        let q = place(Vec3::new(p.x as f64, p.y as f64, p.z as f64), at, mirror_z);
        a.beds.iter().map(move |r| ((q.x - r.x as f64).powi(2) + (q.y - r.y as f64).powi(2) + (q.z - r.z as f64).powi(2)).sqrt())
    }).fold(f64::MAX, f64::min);
    format!(
        "at {},{},{}{}: (a) golem sight, every rider position {}; the waiting riders {} | (b) summon mark, every rider position {}; the waiting riders {} | (e) zombie to rider (6.05) {} | (f) beds {} apart | (g) chunk-aligned {}",
        at[0],
        at[1],
        at[2],
        if mirror_z { " mirrored in z" } else { "" },
        fmt(sight_all),
        fmt(sight_wait),
        fmt(mark_all),
        fmt(mark_wait),
        fmt(zomb),
        if bed == f64::MAX { "-".into() } else { format!("{bed:.1}") },
        if at[0].rem_euclid(16) == 0 && at[2].rem_euclid(16) == if mirror_z { 15 } else { 0 } { "yes" } else { "no" }
    )
}
