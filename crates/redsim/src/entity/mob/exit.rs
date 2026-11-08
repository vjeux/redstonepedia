//! The golem exit flood (ironfarm; brief `designs/briefs/farm-iron.md`, "Golem exit"): walking
//! is not modeled as AI, so the judge proves that no walking input can keep a golem out of the
//! kill.
//!
//! The starts are every spawn spot the farm offers (each villager's 17 x 17 columns, scanned and
//! obstruction-checked as `trySpawnMob` does). From each start a lone golem (villagers spawn
//! none) runs under still and the 8 constant inputs ([`Walk::golem_policies`]) with its own
//! physics: its 1.4 x 2.7 box, step height 1, a jump when blocked, falls, water currents. Every
//! run must kill the golem within [`MAX_GOLEM_LIFE`] without breaking a must-have (the build,
//! the zombie's reach, the vehicles). Every cell a run visits on the ground becomes a new start
//! (at the position the golem first stood there), until none is new: an input that turns is a
//! sequence of the constant ones from positions the flood covers. Under still, a golem that does
//! not move for [`PINNED_TICKS`] out of the lava and the fire is refused at once: it is pinned
//! (the currents under it cancel, or there are none).

use super::farm::{FarmSpec, MAX_GOLEM_LIFE, RunOpts, WalkMode, build, golem_breaks_rule, touches_lava};
use super::spawn::{check_spawn_obstruction, possible_spawn_position};
use super::types::MobType;
use super::walk::Walk;
use super::{MobEvent, mob, mob_mut};
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The most positions the flood visits before it refuses the build (too open to check).
pub const MAX_STARTS: usize = 600;
/// Under still, this many ticks without moving, out of lava and fire: pinned.
pub const PINNED_TICKS: i64 = 100;

/// The flood's record.
#[derive(Clone, Debug, Default)]
pub struct ExitReport {
    /// Spawn spots of the farm.
    pub spawn_spots: usize,
    /// Positions the flood started from (the spawn spots and every cell visited on the ground).
    pub starts: usize,
    /// Drift runs.
    pub runs: usize,
    /// The longest start-to-death time of a run.
    pub worst_life: i64,
    /// What broke (empty: every run killed its golem in time).
    pub failures: Vec<String>,
}

/// Every cell where a golem can spawn in this world: for each villager, its 17 x 17 columns
/// scanned like `trySpawnMob` (`LEGACY_IRON_GOLEM` from y+5 down to y-7) and checked like
/// `checkSpawnObstruction`, in villager then column order, each cell once.
pub fn spawn_spots(w: &World) -> Vec<BlockPos> {
    let mut out: Vec<BlockPos> = Vec::new();
    for e in &w.entities.list {
        let EntityKind::Mob(m) = &e.kind else { continue };
        if m.ty != MobType::Villager || e.removed {
            continue;
        }
        let start = e.block_position();
        for dz in -8..=8 {
            for dx in -8..=8 {
                let top = BlockPos::new(start.x + dx, start.y + 6, start.z + dz);
                if let Some(cell) = possible_spawn_position(w, top, 6)
                    && !out.contains(&cell)
                    && check_spawn_obstruction(w, cell, Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5))
                {
                    out.push(cell);
                }
            }
        }
    }
    out
}

/// One drift run's outcome.
struct Drift {
    died_at: Option<i64>,
    visited: Vec<(BlockPos, Vec3)>,
    fail: Option<String>,
}

/// A lone golem at `start` under `policy` in a copy of `base`.
fn drift(base: &World, spec: &FarmSpec, start: Vec3, policy: Walk) -> Drift {
    let mut w = base.clone();
    let id = super::add(&mut w, MobType::IronGolem, start, false);
    let Some(g) = crate::entity::index_of(&w, id) else { return Drift { died_at: None, visited: Vec::new(), fail: Some("the golem could not be added".into()) } };
    mob_mut(&mut w, g).walk = policy;
    w.mob.events.clear();
    let inner = spec.inner();
    let mut visited: Vec<(BlockPos, Vec3)> = Vec::new();
    let mut last = start;
    let mut still_for = 0;
    let fail = |visited, why: String| Drift { died_at: None, visited, fail: Some(why) };
    for t in 1..=MAX_GOLEM_LIFE {
        w.tick();
        let died = w.mob.events.iter().any(|e| matches!(e, MobEvent::Death { id: d, .. } if *d == id));
        w.mob.events.clear();
        if died {
            return Drift { died_at: Some(t), visited, fail: None };
        }
        let Some(g) = crate::entity::index_of(&w, id) else { return fail(visited, "the golem vanished without dying".into()) };
        if let Some(why) = golem_breaks_rule(&w, g, inner, t) {
            return fail(visited, why);
        }
        let e = &w.entities.list[g];
        if e.on_ground {
            let c = e.block_position();
            if !visited.iter().any(|(q, _)| *q == c) {
                visited.push((c, e.pos));
            }
        }
        if policy == Walk::Still && !mob(&w, g).dead {
            let moved = e.pos.distance_to_sqr(last) > 1.0e-10;
            let burning = touches_lava(&w, e.bb) || mob(&w, g).fire_ticks > 0;
            still_for = if moved || burning { 0 } else { still_for + 1 };
            last = e.pos;
            if still_for >= PINNED_TICKS {
                return fail(visited, format!("pinned at {:.2} {:.2} {:.2}: it stood still {PINNED_TICKS} gt out of the lava (no current carries it)", e.pos.x, e.pos.y, e.pos.z));
            }
        }
    }
    fail(visited, format!("alive {MAX_GOLEM_LIFE} gt after it started: nothing kills it"))
}

/// The golem exit flood of a farm (`threads` drift runs at once).
pub fn golem_exit(spec: &FarmSpec, seed: i64, threads: usize) -> Result<ExitReport, String> {
    let o = RunOpts { seed, ticks: 0, day0: 6000, walk: WalkMode::Random, slept: true };
    let mut base = build(spec, &o)?;
    base.mob.golem_spawning = false;
    let spots = spawn_spots(&base);
    if spots.is_empty() {
        return Err("no golem can spawn anywhere in the build".into());
    }
    let mut rep = ExitReport { spawn_spots: spots.len(), ..Default::default() };
    let mut seen: HashSet<BlockPos> = spots.iter().copied().collect();
    let mut starts: Vec<Vec3> = spots.iter().map(|c| Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5)).collect();
    let policies = Walk::golem_policies();
    let mut done = 0;
    while done < starts.len() {
        let jobs: Vec<(Vec3, Walk)> = starts[done..].iter().flat_map(|s| policies.iter().map(move |p| (*s, *p))).collect();
        done = starts.len();
        let results = run_parallel(&base, spec, &jobs, threads);
        for ((s, p), d) in jobs.iter().zip(results) {
            rep.runs += 1;
            if let Some(f) = d.fail {
                rep.failures.push(format!("a golem from {:.2} {:.2} {:.2} walking {}: {f}", s.x, s.y, s.z, p.name()));
                if rep.failures.len() >= 6 {
                    rep.starts = starts.len();
                    return Ok(rep);
                }
            }
            if let Some(t) = d.died_at {
                rep.worst_life = rep.worst_life.max(t);
            }
            for (c, pos) in d.visited {
                if seen.insert(c) {
                    if starts.len() >= MAX_STARTS {
                        rep.failures.push(format!("a golem can stand on more than {MAX_STARTS} positions: too open for the exit flood"));
                        rep.starts = starts.len();
                        return Ok(rep);
                    }
                    starts.push(pos);
                }
            }
        }
    }
    rep.starts = starts.len();
    Ok(rep)
}

fn run_parallel(base: &World, spec: &FarmSpec, jobs: &[(Vec3, Walk)], threads: usize) -> Vec<Drift> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<Drift>>> = Mutex::new((0..jobs.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1).min(jobs.len()) {
            // a world is not Sync (its cells' caches): each thread gets its own copy
            let mine = base.clone();
            let (next, out) = (&next, &out);
            s.spawn(move || {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= jobs.len() {
                        break;
                    }
                    let d = drift(&mine, spec, jobs[k].0, jobs[k].1);
                    out.lock().unwrap()[k] = Some(d);
                }
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|d| d.unwrap_or(Drift { died_at: None, visited: Vec::new(), fail: Some("not run".into()) })).collect()
}
