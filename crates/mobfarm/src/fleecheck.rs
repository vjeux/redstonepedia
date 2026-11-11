//! (tickexact 2026-11-11, ironspec's HOME rule after 58f770cc's finding) The walk-target static
//! check: from every place a rider can be (each rail cell of the design, the rider seated in a cart
//! there), every cell its brain can take as a walk target, the chance each event picks it, and
//! whether the rider has a complete path to it (redsim's villager pathfinder: a path is capped by the
//! villager's FOLLOW_RANGE, 16 x (1 + a spawn bonus within +-0.11485), its visited nodes by 16x
//! that). A target with no complete path starts the rider's CANT_REACH count, which nothing clears
//! until a path reaches; at its first REST tick by the bed a count over 1,200 gt releases HOME (the
//! bench's z -5 stone: scene flee-stone bridges the flee's part against the game).
//!
//! The sources, by activity:
//! - **PANIC** (a hostile within 8 seen): SetWalkTargetAwayFrom(NEAREST_HOSTILE) while it is within
//!   6 (`LandRandomPos.getPosAway(mob, 16, 7)`, ten calls of ten draws), and VillageBoundRandomStroll
//!   (2/2).
//! - **IDLE** (calm): VillageBoundRandomStroll (10/7: a land draw in a village, else
//!   DefaultRandomPos toward the section closest to the village), and the walks to another villager
//!   within 8 (InteractWith; SetWalkTargetFromLookTarget): every other rider place within 8.
//! - **Job site / bell walks within 48** (SetWalkTargetFromBlockMemory): the job-site blocks and
//!   bells of the design, listed (a farm's riders hold none of them).
//! - **REST**: the HOME walk (the bed itself), for the brief's 1,000-gt line.
//!
//! A cell only the draws' boundary reaches (a share of 0: an offset of exactly 16) is listed as an
//! edge cell and kept out of the verdict. The hostiles are taken whether the rider sees them in the
//! design as placed or not (a hatch, a gate or a cart moves; the sight now is printed). The check
//! reads the design's own world (built and settled as the judge places it); a void bench offers only
//! the design's tops as candidates (ironspec: label its HOME rows "void bench").

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// One source's result at a place: its cells, and those with no complete path with the chance one
/// event picks each (a share of 0: an edge cell).
#[derive(Default)]
pub struct Source {
    pub cells: usize,
    pub unreached: Vec<(BlockPos, f64)>,
}

impl Source {
    /// The chance one event picks an unreached cell.
    pub fn risk(&self) -> f64 {
        self.unreached.iter().map(|(_, s)| *s).sum()
    }

    /// Unreached cells a draw can really pick (a share over 0).
    pub fn live(&self) -> usize {
        self.unreached.iter().filter(|(_, s)| *s > 1e-9).count()
    }
}

/// One rider place's result.
pub struct Place {
    pub cell: BlockPos,
    /// The hostiles within 6 (the flee's) and within 8 (PANIC), and how many it sees as placed.
    pub flee_from: usize,
    pub panic_from: usize,
    pub seen: usize,
    pub village: bool,
    pub flee: Source,
    pub panic_stroll: Source,
    pub stroll: Source,
    pub others: Source,
    pub home_reached: Option<bool>,
}

/// The hostiles of the design's entity lines (a zombie, or a zombie riding a boat or a cart): their
/// feet.
fn hostiles(spec: &FarmSpec) -> Vec<Vec3> {
    spec.entities
        .iter()
        .filter(|(ty, _, nbt)| ty.ends_with("zombie") || nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie\"")))
        .map(|(_, p, _)| Vec3::new(p[0], p[1], p[2]))
        .collect()
}

/// The job-site blocks and bells of `spec` (each a SetWalkTargetFromBlockMemory target within 48 for
/// a villager holding it).
pub fn job_and_bell_blocks(spec: &FarmSpec) -> Vec<(BlockPos, String)> {
    const JOBS: [&str; 14] = ["bell", "composter", "lectern", "barrel", "blast_furnace", "brewing_stand", "cartography_table", "cauldron", "fletching_table", "grindstone", "loom", "smithing_table", "smoker", "stonecutter"];
    spec.blocks
        .iter()
        .filter_map(|(p, b)| {
            let s = b.to_string();
            let name = s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
            JOBS.contains(&name.as_str()).then_some((*p, name))
        })
        .collect()
}

/// The check over every rail cell of `spec` (its own world built and settled), the bed `home` for
/// REST's walk.
pub fn check(spec: &FarmSpec, home: Option<BlockPos>) -> Result<Vec<Place>, String> {
    use redsim::entity::mob::ai::goal::stroll;
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let foes = hostiles(spec);
    let rails: Vec<BlockPos> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, _)| *p).collect();
    let rider_at = |c: BlockPos| Vec3::new(c.x as f64 + 0.5, c.y as f64 + 0.0625, c.z as f64 + 0.5);
    let mut out = Vec::new();
    for &cell in &rails {
        // a test rider: a villager seated in a cart on the rail (the bench's riders' pose)
        let p0 = rider_at(cell);
        let cart = redsim::entity::spawn(w, "minecraft:minecart", [p0.x, p0.y, p0.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
        let Some(ci) = redsim::entity::index_of(w, cart) else { continue };
        let Some(&vid) = w.entities.list[ci].passengers.first() else { continue };
        let Some(i) = redsim::entity::index_of(w, vid) else { continue };
        redsim::entity::position_rider(w, ci, i);
        if let Some(h) = home
            && let EntityKind::Mob(m) = &mut w.entities.list[i].kind
            && let Some(b) = m.brain.as_deref_mut()
        {
            b.home = Some(h);
        }
        let me = w.entities.list[i].pos;
        let eye = Vec3::new(me.x, me.y + 1.62, me.z);
        let dist = |f: &Vec3| ((me.x - f.x).powi(2) + (me.y - f.y).powi(2) + (me.z - f.z).powi(2)).sqrt();
        let near6: Vec<Vec3> = foes.iter().copied().filter(|f| dist(f) < 6.0).collect();
        let near8: Vec<Vec3> = foes.iter().copied().filter(|f| dist(f) <= 8.0).collect();
        let seen = near8.iter().filter(|f| redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z))).count();
        let reaches = |w: &mut redsim::World, t: BlockPos| -> bool {
            redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
                let mut copy = nav.clone();
                copy.create_path_pos(w, nm, t, 0, arena).is_some_and(|p| p.reached)
            })
        };
        let judge = |w: &mut redsim::World, shares: Vec<(BlockPos, f64)>| -> Source {
            let cells = shares.len();
            let mut unreached: Vec<(BlockPos, f64)> = shares.into_iter().filter(|(c, _)| !reaches(w, *c)).collect();
            unreached.sort_by(|a, b| b.1.total_cmp(&a.1));
            Source { cells, unreached }
        };
        // PANIC's flee: every hostile within 6 (the largest share over them; the exact enumeration's
        // edge cells at 0)
        let mut shares: Vec<(BlockPos, f64)> = Vec::new();
        for f in &near6 {
            for (c, s) in stroll::flee_pick_shares(w, i, 16.0, 7, *f) {
                match shares.iter_mut().find(|(x, _)| *x == c) {
                    Some((_, t)) => *t = t.max(s),
                    None => shares.push((c, s)),
                }
            }
            for c in stroll::flee_candidates(w, i, 16.0, 7, *f) {
                if !shares.iter().any(|(x, _)| *x == c) {
                    shares.push((c, 0.0));
                }
            }
        }
        let flee = judge(w, shares);
        // VillageBoundRandomStroll (PANIC's 2/2, IDLE's 10/7): in a village a land draw around; else
        // DefaultRandomPos toward the section closest to the village within 2, or a land draw when
        // that section is its own
        let me_b = w.entities.list[i].block_position();
        let village = redsim::entity::mob::ai::poi::is_village(w, me_b);
        let s = (me_b.x.div_euclid(16), me_b.y.div_euclid(16), me_b.z.div_euclid(16));
        let best = if village { s } else { redsim::entity::mob::ai::poi::section_closest_to_village(w, s, 2) };
        let village_stroll = |w: &redsim::World, h: f64, v: i32| {
            if best != s {
                let c = ((best.0 * 16 + 8) as f64 + 0.5, (best.2 * 16 + 8) as f64 + 0.5);
                stroll::pick_shares(w, i, h, v, Some((c.0 - me.x, c.1 - me.z)), false, 10)
            } else {
                stroll::pick_shares(w, i, h, v, None, true, 10)
            }
        };
        let panic_stroll = if near8.is_empty() { Source::default() } else { judge(w, village_stroll(w, 2.0, 2)) };
        let calm_stroll = village_stroll(w, 10.0, 7);
        let stroll_src = judge(w, calm_stroll);
        // IDLE's walks to another villager within 8 (InteractWith; SetWalkTargetFromLookTarget): every
        // other rider place within 8, its rider's cell
        let mut others: Vec<(BlockPos, f64)> = Vec::new();
        for &o in &rails {
            let q = rider_at(o);
            if o != cell && dist(&q) <= 8.0 {
                others.push((o, 1.0));
            }
        }
        let others = judge(w, others);
        let home_reached = home.map(|h| reaches(w, h));
        out.push(Place { cell, flee_from: near6.len(), panic_from: near8.len(), seen, village, flee, panic_stroll, stroll: stroll_src, others, home_reached });
        let _ = redsim::entity::kill(w, vid);
        let _ = redsim::entity::kill(w, cart);
        w.entities.list.retain(|e| !e.removed);
    }
    Ok(out)
}

/// `mctas mobfarm flee-check <design>`: the report, `show` cells a place; the verdicts last.
pub fn report(places: &[Place], jobs: &[(BlockPos, String)], show: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let fmt = |v: &[(BlockPos, f64)], pct: bool| {
        v.iter()
            .take(show)
            .map(|(c, s)| if pct { format!("{},{},{} ({:.3}%)", c.x, c.y, c.z, 100.0 * s) } else { format!("{},{},{}", c.x, c.y, c.z) })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut section = |name: &str, what: &str, pick: &dyn Fn(&Place) -> Option<&Source>, pct: bool| -> usize {
        let runs: Vec<&Place> = places.iter().filter(|p| pick(p).is_some()).collect();
        let bad: Vec<&Place> = runs.iter().copied().filter(|p| pick(p).is_some_and(|s| s.live() > 0)).collect();
        let edge = runs.iter().filter(|p| pick(p).is_some_and(|s| s.live() == 0 && !s.unreached.is_empty())).count();
        lines.push(format!("{name}: {what}: {} rider places where it runs, {} with cells it picks that no path reaches{}", runs.len(), bad.len(), if edge > 0 { format!(" ({edge} more with edge cells only)") } else { String::new() }));
        for p in &bad {
            let s = pick(p).expect("a source");
            lines.push(format!(
                "  {name} {},{},{} ({} hostile(s) within 6, {} within 8, {} in sight as placed): {} cells, {} unreached{}: {}",
                p.cell.x,
                p.cell.y,
                p.cell.z,
                p.flee_from,
                p.panic_from,
                p.seen,
                s.cells,
                s.live(),
                if pct { format!(" ({:.3}% of events)", 100.0 * s.risk()) } else { String::new() },
                fmt(&s.unreached, pct)
            ));
        }
        bad.len()
    };
    let flee_bad = section("FLEE", "PANIC's SetWalkTargetAwayFrom (getPosAway 16/7, a hostile within 6)", &|p| (p.flee_from > 0).then_some(&p.flee), true);
    let panic_bad = section("PANIC-STROLL", "PANIC's VillageBoundRandomStroll (2/2)", &|p| (p.panic_from > 0).then_some(&p.panic_stroll), true);
    let stroll_bad = section("STROLL", "IDLE's VillageBoundRandomStroll (10/7; a land draw in a village, else toward the village's section)", &|p| Some(&p.stroll), true);
    let others_bad = section("VILLAGER", "IDLE's walks to another villager within 8 (InteractWith, SetWalkTargetFromLookTarget): each other rider place within 8", &|p| Some(&p.others), false);
    lines.push(format!(
        "job site / bell walks within 48: {}",
        if jobs.is_empty() { "no job-site block or bell in the design".to_string() } else { jobs.iter().map(|(p, n)| format!("{n} {},{},{}", p.x, p.y, p.z)).collect::<Vec<_>>().join(", ") }
    ));
    let homes: Vec<&Place> = places.iter().filter(|p| p.home_reached.is_some()).collect();
    if !homes.is_empty() {
        let reached: Vec<String> = homes.iter().filter(|p| p.home_reached == Some(true)).map(|p| format!("{},{},{}", p.cell.x, p.cell.y, p.cell.z)).collect();
        lines.push(format!(
            "REST's HOME walk: the bed reached from {} of {} rider places (from each other one a resting rider's CANT_REACH count runs: the brief's 1,000-gt line){}",
            reached.len(),
            homes.len(),
            if reached.is_empty() { String::new() } else { format!(": {}", reached.iter().take(12).cloned().collect::<Vec<_>>().join(" ")) }
        ));
    }
    let verdict = |n: usize| if n == 0 { "PASS".to_string() } else { format!("FAIL at {n} places") };
    lines.push(format!(
        "verdicts (every cell a source can pick reachable wherever it runs): flee {}, panic stroll {}, idle stroll {}, walks to villagers {}",
        verdict(flee_bad),
        verdict(panic_bad),
        verdict(stroll_bad),
        verdict(others_bad)
    ));
    lines
}
