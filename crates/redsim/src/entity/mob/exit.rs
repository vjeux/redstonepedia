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

use super::farm::{FarmSpec, MAX_GOLEM_LIFE, RunOpts, WalkMode, golem_breaks_rule, touches_lava};
use super::spawn::{check_spawn_obstruction, possible_spawn_position};
use super::types::MobType;
use super::walk::Walk;
use super::{MobEvent, mob, mob_mut};
use crate::entity::{Aabb, EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The most starts the flood runs before it refuses the build (too open to check): positions and
/// their edge starts (up to 4 each; a 2 x 2 grid of L2d modules has about 330 positions).
pub const MAX_STARTS: usize = 4000;
/// Under still, this many ticks without moving, out of lava and fire: pinned.
pub const PINNED_TICKS: i64 = 100;
/// In a part's flood ([`golem_exit_part`]), this many ticks with no new cell stood on end a run:
/// the golem has shown what it reaches from there (a golem walking into a wall jumps in place, so
/// it never stands still), and nothing in a part kills it.
pub const PART_IDLE_TICKS: i64 = 200;

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
    /// Walking inputs that kept a golem alive past [`MAX_GOLEM_LIFE`] without an escape (brief
    /// rev 9 iteration 13: reported on the card, not refused; golem walking is judged by the AI-on
    /// runs and the game gate).
    pub walk_stalls: Vec<String>,
    /// Lips the flood reached: cells where a golem stood on a dry top with its box in water (a
    /// current cannot carry it there; iteration 13's G1 note: reported, never refused).
    pub lips: usize,
    /// (ironspec Nov 11 14:00) A part's flood ([`golem_exit_part`]: the file holds no bed).
    pub part: bool,
    /// The summoners the spawn spots came from: the villagers, and a part's extra positions.
    pub summoners: usize,
    /// A part's runs whose golem ended alive (it stopped moving, or reached the limit): a part has
    /// no kill.
    pub rested: usize,
    /// A part's runs whose golem walked out of the part, with the first few.
    pub left: usize,
    pub left_at: Vec<String>,
    /// A part's runs whose golem went through a nether portal (the pad's way to the kill in the
    /// farm's Nether part), with the first few.
    pub portal: usize,
    pub portal_at: Vec<String>,
    /// The box of every cell a golem stood on (the spawn spots and the cells the flood reached).
    pub stood: Option<(BlockPos, BlockPos)>,
}

/// Every cell where a golem can spawn in this world: for each villager, its 17 x 17 columns
/// scanned like `trySpawnMob` (`LEGACY_IRON_GOLEM` from y+5 down to y-7) and checked like
/// `checkSpawnObstruction`, in villager then column order, each cell once.
pub fn spawn_spots(w: &World) -> Vec<BlockPos> {
    let all: Vec<u32> = w.entities.list.iter().map(|e| e.id).collect();
    spawn_spots_of(w, &all)
}

/// [`spawn_spots`] of the villagers among `own` (a terrain run's village villagers are not the farm's).
pub fn spawn_spots_of(w: &World, own: &[u32]) -> Vec<BlockPos> {
    let mut out: Vec<BlockPos> = Vec::new();
    for e in &w.entities.list {
        let EntityKind::Mob(m) = &e.kind else { continue };
        if m.ty != MobType::Villager || e.removed || !own.contains(&e.id) {
            continue;
        }
        // its cell as built and the cell above (a villager woken in a 1-cell pod stands at its
        // bed + 1.1 until it falls back: `site::summoner_starts`)
        spots_from(w, e.block_position(), &mut out);
    }
    out
}

/// One drift run's outcome.
#[derive(Default)]
struct Drift {
    died_at: Option<i64>,
    visited: Vec<(BlockPos, Vec3)>,
    fail: Option<String>,
    /// A walking input's golem alive past the limit without an escape (iteration 13: reported).
    stall: Option<String>,
    /// Lips: cells where the golem stood on a dry top while its box touched water (iteration 13's
    /// G1 note: a current cannot move a golem whose feet are dry; reported, never refused).
    lips: Vec<BlockPos>,
    /// (a part's flood) The golem ended alive: it stood still [`PINNED_TICKS`], or reached the
    /// limit, with nothing to kill it (a part has no kill).
    rested: bool,
    /// (a part's flood) Where the golem walked out of the part (the farm it joins takes it on).
    left: Option<Vec3>,
    /// (a part's flood) Where the golem went through a nether portal (the pad's way to the kill).
    portal: Option<Vec3>,
}

/// Whether a golem standing at `feet` with box `bb` is on a lip: its feet cell dry, its box in
/// water somewhere.
fn on_lip(w: &World, feet: BlockPos, bb: Aabb) -> bool {
    let water = |p: BlockPos| matches!(crate::fluid::fluid_state(w, p).fluid, crate::fluid::Fluid::Water | crate::fluid::Fluid::FlowingWater);
    if water(feet) {
        return false;
    }
    let (x0, y0, z0) = (bb.min_x.floor() as i32, bb.min_y.floor() as i32, bb.min_z.floor() as i32);
    let (x1, y1, z1) = (bb.max_x.floor() as i32, bb.max_y.floor() as i32, bb.max_z.floor() as i32);
    (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| water(BlockPos::new(x, y, z)))))
}

/// A lone golem at `start` under `policy` in a copy of `base` (`own`: the farm's entities).
/// The cells golems' boxes swept in the drift runs while a caller collects them (`mobfarm bury`: a
/// void farm dug into rock keeps open every cell a golem can reach, under any input).
pub static ENVELOPE: std::sync::Mutex<Option<std::collections::BTreeSet<[i32; 3]>>> = std::sync::Mutex::new(None);

/// Every block cell `bb` overlaps, into [`ENVELOPE`] when it collects.
pub fn envelope_add(bb: crate::entity::Aabb) {
    if let Ok(mut g) = ENVELOPE.lock()
        && let Some(set) = g.as_mut()
    {
        let e = 1.0e-7;
        for x in (bb.min_x + e).floor() as i32..=(bb.max_x - e).floor() as i32 {
            for y in (bb.min_y + e).floor() as i32..=(bb.max_y - e).floor() as i32 {
                for z in (bb.min_z + e).floor() as i32..=(bb.max_z - e).floor() as i32 {
                    set.insert([x, y, z]);
                }
            }
        }
    }
}

fn drift(base: &World, spec: &FarmSpec, own: &[u32], start: Vec3, policy: Walk, part: bool) -> Drift {
    let mut w = base.clone();
    let id = super::add(&mut w, MobType::IronGolem, start, false);
    let Some(g) = crate::entity::index_of(&w, id) else { return Drift { died_at: None, visited: Vec::new(), fail: Some("the golem could not be added".into()), stall: None, lips: Vec::new(), ..Default::default() } };
    mob_mut(&mut w, g).walk = policy;
    w.mob.events.clear();
    let inner = spec.inner();
    let mut visited: Vec<(BlockPos, Vec3)> = Vec::new();
    let mut lips: Vec<BlockPos> = Vec::new();
    let mut last = start;
    let mut still_for = 0;
    let mut since_new = 0;
    let mut at = start;
    let fail = |visited, why: String| Drift { died_at: None, visited, fail: Some(why), stall: None, lips: Vec::new(), ..Default::default() };
    for t in 1..=MAX_GOLEM_LIFE {
        w.tick();
        let died = w.mob.events.iter().any(|e| matches!(e, MobEvent::Death { id: d, .. } if *d == id));
        // (removed without dying: through a nether portal, `portal::leave`)
        let removed = w.mob.events.iter().any(|e| matches!(e, MobEvent::Removed { id: d, .. } if *d == id));
        w.mob.events.clear();
        if died {
            return Drift { died_at: Some(t), visited, fail: None, stall: None, lips, ..Default::default() };
        }
        // a part's golem through the farm's own nether portal: the pad's way to the kill (the
        // Nether part of the farm it joins)
        if part && removed && crate::entity::index_of(&w, id).is_none() {
            return Drift { visited, lips, portal: Some(at), ..Default::default() };
        }
        let Some(g) = crate::entity::index_of(&w, id) else { return fail(visited, "the golem vanished without dying".into()) };
        at = w.entities.list[g].pos;
        // a part's golem walking out of the part ends its run: the farm it joins takes it on (the
        // whole farm's flood judges that way; a part has no kill)
        if part {
            let e = &w.entities.list[g];
            if !mob(&w, g).dead && (!inner.intersects(e.bb) || e.bb.min_y < inner.min_y) {
                return Drift { visited, lips, left: Some(e.pos), ..Default::default() };
            }
        }
        if let Some(why) = golem_breaks_rule(&w, g, inner, t, own) {
            return fail(visited, why);
        }
        let e = &w.entities.list[g];
        envelope_add(e.bb);
        // (`MCTAS_DRIFT_TRACE=x,y,z`: every tick of the drifts that start there, for finding the
        // way a golem left)
        if let Ok(tr) = std::env::var("MCTAS_DRIFT_TRACE") {
            let at: Vec<f64> = tr.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if at.len() == 3 && (start.x - at[0]).abs() < 0.01 && (start.y - at[1]).abs() < 0.01 && (start.z - at[2]).abs() < 0.01 {
                eprintln!("drift {} t {t}: {:.2} {:.2} {:.2} ground {}", policy.name(), e.pos.x, e.pos.y, e.pos.z, e.on_ground);
            }
        }
        since_new += 1;
        if e.on_ground {
            let c = e.block_position();
            if !visited.iter().any(|(q, _)| *q == c) {
                visited.push((c, e.pos));
                since_new = 0;
            }
            if !lips.contains(&c) && on_lip(&w, c, e.bb) {
                lips.push(c);
            }
        }
        // under still a golem that stops moving ends its run (a refusal, or in a part its rest)
        if policy == Walk::Still && !mob(&w, g).dead {
            let moved = e.pos.distance_to_sqr(last) > 1.0e-10;
            let burning = touches_lava(&w, e.bb) || mob(&w, g).fire_ticks > 0;
            still_for = if moved || burning { 0 } else { still_for + 1 };
            last = e.pos;
            if still_for >= PINNED_TICKS {
                if part {
                    return Drift { visited, lips, rested: true, ..Default::default() };
                }
                return fail(visited, format!("pinned at {:.2} {:.2} {:.2}: it stood still {PINNED_TICKS} gt out of the lava (no current carries it)", e.pos.x, e.pos.y, e.pos.z));
            }
        }
        if part && since_new >= PART_IDLE_TICKS && !mob(&w, g).dead {
            return Drift { visited, lips, rested: true, ..Default::default() };
        }
    }
    // alive at the limit: under still a refusal (dead water: nothing carries it), under a walking
    // input a stall the card reports (brief rev 9 iteration 13: a constant walk is no golem's
    // goal; the AI-on runs and the game gate judge walking); in a part, the run's end (no kill)
    if part {
        return Drift { visited, lips, rested: true, ..Default::default() };
    }
    let p = crate::entity::index_of(&w, id).map(|g| w.entities.list[g].pos);
    let at = p.map_or(String::new(), |p| format!(" (at {:.2} {:.2} {:.2})", p.x, p.y, p.z));
    let why = format!("alive {MAX_GOLEM_LIFE} gt after it started{at}: nothing kills it");
    if policy == Walk::Still {
        fail(visited, why)
    } else {
        Drift { died_at: None, visited, fail: None, stall: Some(why), lips, ..Default::default() }
    }
}

/// The golem exit flood of a farm (`threads` drift runs at once).
pub fn golem_exit(spec: &FarmSpec, seed: i64, threads: usize) -> Result<ExitReport, String> {
    golem_exit_in(spec, None, seed, threads)
}

/// [`golem_exit`] in a terrain site (`site`: the spec already moved there, `Site::spec`): the
/// golems drift in the farm built into the sample, natural rock for its walls, with no natural
/// spawning (a lone golem's own physics).
pub fn golem_exit_in(spec: &FarmSpec, site: Option<&'static super::site::Site>, seed: i64, threads: usize) -> Result<ExitReport, String> {
    flood(spec, site, seed, threads, None)
}

/// The golem exit flood of a part file that holds no bed ([`FarmSpec::has_bed`]; ironspec Nov 11
/// 14:00: a station or loop part's golem flood runs before the farm is assembled): no villager
/// gets a HOME, every declared villager position (each villager's cell and the cell above, as
/// [`spawn_spots_of`] takes them) and every `extra` position is a summoner, and a run ends
/// without a refusal when its golem stops moving, finds no new cell for [`PART_IDLE_TICKS`],
/// reaches the limit, walks out of the part or goes through a nether portal (a part has no kill:
/// the farm it joins takes the golem on, its pad's portal leading to the kill). A golem that
/// reaches a zombie or touches a vehicle, or a part too open to flood, still fails.
pub fn golem_exit_part(spec: &FarmSpec, extra: &[BlockPos], seed: i64, threads: usize) -> Result<ExitReport, String> {
    flood(spec, None, seed, threads, Some(extra))
}

/// Every cell where a golem can spawn for a summoner at `start` (its cell and the cell above), into
/// `out` once each: [`spawn_spots_of`]'s scan for one position.
fn spots_from(w: &World, start: BlockPos, out: &mut Vec<BlockPos>) {
    for s in [start, start.above()] {
        for dz in -8..=8 {
            for dx in -8..=8 {
                let top = BlockPos::new(s.x + dx, s.y + 6, s.z + dz);
                if let Some(cell) = possible_spawn_position(w, top, 6)
                    && !out.contains(&cell)
                    && check_spawn_obstruction(w, cell, Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5))
                {
                    out.push(cell);
                }
            }
        }
    }
}

/// The flood of [`golem_exit_in`] (`part`: [`golem_exit_part`]'s extra summoners).
fn flood(spec: &FarmSpec, site: Option<&'static super::site::Site>, seed: i64, threads: usize, part: Option<&[BlockPos]>) -> Result<ExitReport, String> {
    // the drift under the walking policies (a probe of the farm's walls, the mob AI off)
    let o = RunOpts { seed, ticks: 0, day0: 6000, walk: WalkMode::Random, slept: true, slept_rest: false, site, weather: crate::weather::Plan::Off, ai: false, raw: false };
    let r = if part.is_some() { super::farm::Run::new_homeless(spec, &o)? } else { super::farm::Run::new(spec, &o)? };
    let own = r.own.clone();
    let mut base = r.world;
    base.mob.golem_spawning = false;
    base.mob.natural = None;
    // (a lone golem in the farm: a terrain sample's village and natural mobs left out)
    base.entities.list.retain(|e| own.contains(&e.id));
    // every villager's spots, in terrain too: a blind villager panics as well when it is hurt or
    // sees any hostile (mobinfra's gate, run 5: a golem the third villager of D1U summoned onto the
    // surface escaped), and the site's caps cover every villager's search (`site::covers_at`)
    let starters: Vec<u32> = own.clone();
    let _ = site;
    let mut spots = spawn_spots_of(&base, &starters);
    if let Some(extra) = part {
        for p in extra {
            spots_from(&base, *p, &mut spots);
        }
    }
    if spots.is_empty() {
        return Err("no golem can spawn anywhere in the build".into());
    }
    let villagers = base.entities.list.iter().filter(|e| matches!(&e.kind, crate::entity::EntityKind::Mob(m) if m.ty == MobType::Villager)).count();
    let mut rep = ExitReport { spawn_spots: spots.len(), part: part.is_some(), summoners: villagers + part.map_or(0, |x| x.len()), ..Default::default() };
    let mut seen: HashSet<BlockPos> = spots.iter().copied().collect();
    let mut lips: HashSet<BlockPos> = HashSet::new();
    let mut edges: HashSet<[i64; 3]> = HashSet::new();
    let mut starts: Vec<Vec3> = Vec::new();
    for c in &spots {
        let s = Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5);
        starts.push(s);
        starts.extend(edge_starts(&base, s, &mut edges));
    }
    let policies = Walk::golem_policies();
    let mut done = 0;
    while done < starts.len() {
        let jobs: Vec<(Vec3, Walk)> = starts[done..].iter().flat_map(|s| policies.iter().map(move |p| (*s, *p))).collect();
        done = starts.len();
        let results = run_parallel(&base, spec, &own, &jobs, threads, part.is_some());
        for ((s, p), d) in jobs.iter().zip(results) {
            rep.runs += 1;
            if let Some(f) = d.fail {
                rep.failures.push(format!("a golem from {:.4} {:.2} {:.4} walking {}: {f}", s.x, s.y, s.z, p.name()));
                if rep.failures.len() >= 6 {
                    rep.starts = starts.len();
                    rep.stood = stood_box(&seen);
                    return Ok(rep);
                }
            }
            if let Some(t) = d.died_at {
                rep.worst_life = rep.worst_life.max(t);
            }
            if let Some(st) = d.stall
                && rep.walk_stalls.len() < 6
            {
                rep.walk_stalls.push(format!("a golem from {:.4} {:.2} {:.4} walking {}: {st}", s.x, s.y, s.z, p.name()));
            }
            if d.rested {
                rep.rested += 1;
            }
            if let Some(at) = d.left {
                rep.left += 1;
                if rep.left_at.len() < 6 {
                    rep.left_at.push(format!("a golem from {:.4} {:.2} {:.4} walking {} left the part at {:.2} {:.2} {:.2}", s.x, s.y, s.z, p.name(), at.x, at.y, at.z));
                }
            }
            if let Some(at) = d.portal {
                rep.portal += 1;
                if rep.portal_at.len() < 6 {
                    rep.portal_at.push(format!("a golem from {:.4} {:.2} {:.4} walking {} went through a nether portal at {:.2} {:.2} {:.2}", s.x, s.y, s.z, p.name(), at.x, at.y, at.z));
                }
            }
            lips.extend(d.lips.iter().copied());
            for (c, pos) in d.visited {
                if seen.insert(c) {
                    if starts.len() >= MAX_STARTS {
                        rep.failures.push(format!("a golem can stand on more than {MAX_STARTS} positions: too open for the exit flood"));
                        rep.starts = starts.len();
                        rep.stood = stood_box(&seen);
                        return Ok(rep);
                    }
                    starts.push(pos);
                    starts.extend(edge_starts(&base, pos, &mut edges));
                }
            }
        }
    }
    rep.starts = starts.len();
    rep.lips = lips.len();
    rep.stood = stood_box(&seen);
    Ok(rep)
}

/// The box (lowest and highest corner) of the cells a golem stood on.
fn stood_box(seen: &HashSet<BlockPos>) -> Option<(BlockPos, BlockPos)> {
    let mut it = seen.iter();
    let first = *it.next()?;
    let (mut lo, mut hi) = (first, first);
    for c in it {
        lo = BlockPos::new(lo.x.min(c.x), lo.y.min(c.y), lo.z.min(c.z));
        hi = BlockPos::new(hi.x.max(c.x), hi.y.max(c.y), hi.z.max(c.z));
    }
    Some((lo, hi))
}

/// How far a start at the edge of the blocks under a golem still overlaps them.
pub const EDGE_OVERLAP: f64 = 0.0005;

/// The golem's box at `p` (1.4 wide, 2.7 tall).
fn golem_box(p: Vec3) -> Aabb {
    Aabb::new(p.x - 0.7, p.y, p.z - 0.7, p.x + 0.7, p.y + 2.7, p.z + 0.7)
}

/// A golem at `p` stands: no block shape meets its box, and one stops it moving down.
fn stands(w: &World, p: Vec3) -> bool {
    let bb = golem_box(p);
    !crate::entity::collide::collides_with_blocks(w, bb) && crate::entity::collide::collide_box(w, bb, Vec3::new(0.0, -0.01, 0.0)).y > -0.01
}

/// The golem's box slides from `a` to `b` (same height) without meeting a block shape, checked
/// every 0.05 blocks: an edge start must be one the golem can reach from where it stood.
fn free_path(w: &World, a: Vec3, b: Vec3) -> bool {
    let n = ((b.x - a.x).abs().max((b.z - a.z).abs()) / 0.05).ceil().max(1.0) as usize;
    (0..=n).all(|i| {
        let t = i as f64 / n as f64;
        let p = Vec3::new(a.x + (b.x - a.x) * t, a.y, a.z + (b.z - a.z) * t);
        !crate::entity::collide::collides_with_blocks(w, golem_box(p))
    })
}

/// The starts at the edge of what a golem stands on ("every position the golem can stand on",
/// review G1): every spot within 2 blocks of `start` (same height) where one of the box's sides
/// sits [`EDGE_OVERLAP`] inside or outside a block boundary, each axis on its own or both (a
/// corner), that stands and stops standing (or meets a wall) 0.002 further one way. A current
/// that ends with the floor stops pushing a golem there (the fluid box is the golem's box deflated
/// by 0.001) while its collision box still rests on the floor by the overlap (collisions shrink a
/// box by only 1e-7): the game's golems stayed on the L1 hole's rim that way for thousands of
/// ticks (2026-11-08). `seen` keeps each spot once across starts.
fn edge_starts(w: &World, start: Vec3, seen: &mut HashSet<[i64; 3]>) -> Vec<Vec3> {
    let cands = |c: f64| -> Vec<f64> {
        let mut v = vec![c];
        for n in ((c - 2.0).floor() as i64)..=((c + 2.0).ceil() as i64) {
            for half in [-0.7, 0.7] {
                for s in [-1.0, 1.0] {
                    v.push(n as f64 - half + s * EDGE_OVERLAP);
                }
            }
        }
        v
    };
    let mut out = Vec::new();
    for x in cands(start.x) {
        for z in cands(start.z) {
            let p = Vec3::new(x, start.y, z);
            if !seen.insert([(x * 1.0e4).round() as i64, (start.y * 1.0e4).round() as i64, (z * 1.0e4).round() as i64]) || !stands(w, p) || !free_path(w, start, p) {
                continue;
            }
            let edge = [(0.002, 0.0), (-0.002, 0.0), (0.0, 0.002), (0.0, -0.002)].iter().any(|(dx, dz)| !stands(w, Vec3::new(x + dx, start.y, z + dz)));
            if edge {
                out.push(p);
            }
        }
    }
    out
}

fn run_parallel(base: &World, spec: &FarmSpec, own: &[u32], jobs: &[(Vec3, Walk)], threads: usize, part: bool) -> Vec<Drift> {
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
                    let d = drift(&mine, spec, own, jobs[k].0, jobs[k].1, part);
                    out.lock().unwrap()[k] = Some(d);
                }
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|d| d.unwrap_or(Drift { died_at: None, visited: Vec::new(), fail: Some("not run".into()), stall: None, lips: Vec::new(), ..Default::default() })).collect()
}

#[cfg(test)]
mod part_tests {
    use super::*;
    use crate::block::Block;

    /// A stone room, 5 x 5 inside and 4 high (floor y 10), its walls 3 thick (a golem spawns with
    /// its box up to 0.2 into a wall and walks through what it overlaps, as in the game) and its
    /// roof 5 thick (no golem spot on it), with a villager in a corner of its floor and no bed: a
    /// part's file. `zombie`: a zombie in a boat in the opposite corner as well.
    fn room(zombie: bool) -> FarmSpec {
        let stone = Block::parse("minecraft:stone").unwrap();
        let mut blocks = Vec::new();
        for x in -5..=5 {
            for z in -5..=5 {
                blocks.push((BlockPos::new(x, 10, z), stone.clone()));
                let wall = x.abs() >= 3 || z.abs() >= 3;
                for y in 11..=19 {
                    if wall || y >= 15 {
                        blocks.push((BlockPos::new(x, y, z), stone.clone()));
                    }
                }
            }
        }
        let mut entities = vec![("minecraft:villager".to_string(), [-1.5, 11.0, -1.5], Some("{PersistenceRequired:1b}".to_string()))];
        if zombie {
            entities.push(("minecraft:oak_boat".to_string(), [2.0, 11.0, 2.0], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".to_string())));
        }
        FarmSpec { blocks, entities, output: BlockPos::new(0, 11, -2), lo: BlockPos::new(-9, 6, -9), hi: BlockPos::new(9, 24, 9), block_entities: Vec::new(), rock: false, nether: None, land: false }
    }

    #[test]
    fn a_part_with_no_bed_floods_and_still_fails_on_a_zombie() {
        let spec = room(false);
        assert!(!spec.has_bed());
        // the whole farm's flood pairs every villager with a bed: a part has none
        let whole = golem_exit(&spec, 1, 4);
        assert!(whole.as_ref().is_err_and(|e| e.contains("no bed of its own")), "{:?}", whole.map(|r| r.failures));
        // the part's flood: its villager summons, the golem rests in the room, nothing fails
        let rep = golem_exit_part(&spec, &[], 1, 4).unwrap();
        assert!(rep.part && rep.summoners == 1 && rep.spawn_spots > 0, "{} spots from {} summoners", rep.spawn_spots, rep.summoners);
        assert!(rep.failures.is_empty(), "{:?}", rep.failures);
        assert!(rep.rested > 0 && rep.left == 0 && rep.portal == 0, "rested {} left {} portal {}: {:?} stood {:?}", rep.rested, rep.left, rep.portal, rep.left_at, rep.stood);
        // (inside the room, or at most into its walls' first layer)
        let (lo, hi) = rep.stood.unwrap();
        assert!(lo.x >= -4 && hi.x <= 4 && lo.z >= -4 && hi.z <= 4 && lo.y == 11 && hi.y == 11, "{lo:?} {hi:?}");
        // an extra summoner position in the same room adds no new spot
        let more = golem_exit_part(&spec, &[BlockPos::new(1, 11, 1)], 1, 4).unwrap();
        assert_eq!((more.summoners, more.spawn_spots), (2, rep.spawn_spots));
        // positive control: a zombie in reach fails the part's flood as it fails a farm's
        let z = golem_exit_part(&room(true), &[], 1, 4).unwrap();
        assert!(z.failures.iter().any(|f| f.contains("zombie") || f.contains("vehicle")), "{:?}", z.failures);
    }
}
