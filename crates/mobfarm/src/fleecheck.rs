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
    /// The chance one event picks a cell the rider's path reaches (what clears a stamp).
    pub reached_share: f64,
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
    /// A villager standing there (a dropped napper), not a rider seated in a cart.
    pub on_foot: bool,
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
    /// The nearest hostile's distance: the least and the greatest over the place's points, and
    /// the hostile nearest at the least (None: no hostile in the design).
    pub near: Option<(f64, f64, Vec3)>,
    /// The closest rider-zombie pair at this place: the distance, the rider's position, the
    /// zombie's.
    pub worst: Option<(f64, Vec3, Vec3)>,
    /// In the design's `panic:` boxes (the part relies on panic there).
    pub panic_declared: bool,
    /// Every point sees a zombie within PANIC_BAR (the eyes' line clear).
    pub panic_ok: bool,
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

/// The 6-8 rule's bars (ironspec Nov 11 10:54, scene flee-distance bridged): a zombie inside 6.05
/// of a rider position fails; a declared panic place needs one seen within 7.95.
pub const FLEE_BAR: f64 = 6.05;
pub const PANIC_BAR: f64 = 7.95;

/// The zombies where they sit (ironspec Nov 11 10:54: "every position the zombie can take in its
/// post, the boat's drift and the zombie's seat height"): each zombie of the settled world at its
/// own position (a post's zombie seated in its boat, the seat's height included; a boat on its
/// glass has nothing to move it, so the seat is the post's one position); the design's entity lines
/// only when the world holds none.
pub fn zombies_seated(w: &redsim::World, spec: &FarmSpec) -> Vec<Vec3> {
    let v: Vec<Vec3> = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .filter_map(|e| {
            let EntityKind::Mob(m) = &e.kind else { return None };
            if m.ty != redsim::entity::mob::types::MobType::Zombie {
                return None;
            }
            // (seated in a boat: the boat's y plus its passenger attachment, then the zombie's
            // ridingOffset, as `Entity.positionRider` orders them; redsim's `position_rider`
            // seats only cart passengers, so the seat is taken here)
            let vehicle = e.vehicle.and_then(|id| w.entities.list.iter().find(|x| x.id == id && !x.removed));
            Some(match vehicle.and_then(|b| redsim::entity::mob::rider_seat(&b.kind, m).map(|s| (b.pos, s))) {
                Some((bp, (seat, off))) => Vec3::new(bp.x, (bp.y + seat) + off, bp.z),
                None => e.pos,
            })
        })
        .collect();
    if !v.is_empty() {
        return v;
    }
    // (the check's world holds no entities of the design (they arrive later): each zombie line at
    // its own position, and a zombie riding a boat or cart seated: the vehicle's passenger
    // attachment (a boat's 0.5625 / 3, a cart's 0.1875 for a non-villager) plus the zombie's
    // ridingOffset (-0.7), the boat's y first)
    let off = redsim::entity::mob::types::MobType::Zombie.def().riding_offset as f64;
    spec.entities
        .iter()
        .filter_map(|(ty, p, nbt)| {
            if ty.ends_with("zombie") {
                return Some(Vec3::new(p[0], p[1], p[2]));
            }
            let rides = nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie\""));
            if rides && ty.ends_with("boat") {
                return Some(Vec3::new(p[0], (p[1] + (0.5625f32 / 3.0f32) as f64) + off, p[2]));
            }
            if rides && ty.ends_with("minecart") {
                return Some(Vec3::new(p[0], (p[1] + 0.1875f32 as f64) + off, p[2]));
            }
            rides.then(|| Vec3::new(p[0], p[1], p[2]))
        })
        .collect()
}

/// A cart's position at fraction `t` along rail `c` (shape from `state`): along the rail's axis,
/// on a slope at its height there (the cart rides 0.0625 over the rail's surface); a curve's centre.
pub fn along_rail(c: BlockPos, state: &str, t: f64) -> Vec3 {
    let (x, y, z) = (c.x as f64, c.y as f64 + 0.0625, c.z as f64);
    if state.contains("ascending_east") {
        Vec3::new(x + t, y + t, z + 0.5)
    } else if state.contains("ascending_west") {
        Vec3::new(x + t, y + 1.0 - t, z + 0.5)
    } else if state.contains("ascending_south") {
        Vec3::new(x + 0.5, y + t, z + t)
    } else if state.contains("ascending_north") {
        Vec3::new(x + 0.5, y + 1.0 - t, z + t)
    } else if state.contains("east_west") {
        Vec3::new(x + t, y, z + 0.5)
    } else if state.contains("north_south") {
        Vec3::new(x + 0.5, y, z + t)
    } else {
        Vec3::new(x + 0.5, y, z + 0.5)
    }
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

/// The check over every rail cell of `spec` (its own world built and settled; a rider seated in a
/// cart there, at three points along the rail's axis), and on foot at every rail cell within 1 of the
/// bed `home` (a dropped napper stands there) and at each `foot` cell; the bed `home` for REST's walk.
pub fn check(spec: &FarmSpec, home: Option<BlockPos>, foot: &[BlockPos], panic_boxes: &[([i32; 3], [i32; 3])]) -> Result<Vec<Place>, String> {
    use redsim::entity::mob::ai::goal::stroll;
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let foes = zombies_seated(w, spec);
    let rails: Vec<(BlockPos, String)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    let rider_at = |c: BlockPos| Vec3::new(c.x as f64 + 0.5, c.y as f64 + 0.0625, c.z as f64 + 0.5);
    // the places: (cell, on foot, the points tried)
    let mut places: Vec<(BlockPos, bool, Vec<Vec3>)> = Vec::new();
    for (c, state) in &rails {
        let along = |t: f64| along_rail(*c, state, t);
        places.push((*c, false, vec![along(0.1), along(0.5), along(0.9)]));
    }
    for (c, _) in &rails {
        if home.is_some_and(|h| (c.x - h.x).abs().max((c.z - h.z).abs()) <= 1 && (c.y - h.y).abs() <= 1) {
            places.push((*c, true, vec![Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5)]));
        }
    }
    for c in foot {
        places.push((*c, true, vec![Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5)]));
    }
    let mut out = Vec::new();
    for (cell, on_foot, points) in places {
        let p0 = points[points.len() / 2];
        let (vid, cart) = if on_foot {
            (redsim::entity::spawn(w, "minecraft:villager", [p0.x, p0.y, p0.z], Some("{PersistenceRequired:1b}"))?, None)
        } else {
            let cart = redsim::entity::spawn(w, "minecraft:minecart", [p0.x, p0.y, p0.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
            let Some(ci) = redsim::entity::index_of(w, cart) else { continue };
            let Some(&vid) = w.entities.list[ci].passengers.first() else { continue };
            (vid, Some(cart))
        };
        let Some(i) = redsim::entity::index_of(w, vid) else { continue };
        if let Some(h) = home
            && let EntityKind::Mob(m) = &mut w.entities.list[i].kind
            && let Some(b) = m.brain.as_deref_mut()
        {
            b.home = Some(h);
        }
        // (put the rider at point `p`: the cart there and the rider seated, or the villager standing)
        let put = |w: &mut redsim::World, p: Vec3| {
            match cart.and_then(|c| redsim::entity::index_of(w, c)) {
                Some(ci) => {
                    w.entities.list[ci].pos = p;
                    redsim::entity::position_rider(w, ci, i);
                }
                None => w.entities.list[i].pos = p,
            }
            w.entities.list[i].pos
        };
        let reaches = |w: &mut redsim::World, t: BlockPos| -> bool {
            redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
                let mut copy = nav.clone();
                copy.create_path_pos(w, nm, t, 0, arena).is_some_and(|p| p.reached)
            })
        };
        let merge = |into: &mut Vec<(BlockPos, f64)>, from: Vec<(BlockPos, f64)>| {
            for (c, s) in from {
                match into.iter_mut().find(|(x, _)| *x == c) {
                    Some((_, t)) => *t = t.max(s),
                    None => into.push((c, s)),
                }
            }
        };
        let (mut flee_s, mut panic_s, mut idle_s) = (Vec::new(), Vec::new(), Vec::new());
        let (mut flee_from, mut panic_from, mut seen, mut village) = (0, 0, 0, false);
        // (the 6-8 rule, ironspec Nov 11 10:18: the nearest hostile's distance at each point, the
        // least and the greatest over the place's points, and that hostile)
        let mut near: Option<(f64, f64, Vec3)> = None;
        // (the closest pair: the distance, the rider's position, the zombie's; and whether every
        // point sees a zombie within PANIC_BAR)
        let mut worst: Option<(f64, Vec3, Vec3)> = None;
        let mut panic_ok = true;
        for &p in &points {
            let me = put(w, p);
            let eye = Vec3::new(me.x, me.y + 1.62, me.z);
            let dist = |f: &Vec3| ((me.x - f.x).powi(2) + (me.y - f.y).powi(2) + (me.z - f.z).powi(2)).sqrt();
            if let Some(f) = foes.iter().min_by(|a, b| dist(a).total_cmp(&dist(b))) {
                let d = dist(f);
                near = Some(match near {
                    None => (d, d, *f),
                    Some((lo, hi, h)) => (lo.min(d), hi.max(d), if d < lo { *f } else { h }),
                });
                if worst.is_none_or(|(wd, _, _)| d < wd) {
                    worst = Some((d, me, *f));
                }
            }
            panic_ok &= foes.iter().any(|f| dist(f) <= PANIC_BAR && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z)));
            let near6: Vec<Vec3> = foes.iter().copied().filter(|f| dist(f) < 6.0).collect();
            let near8: Vec<Vec3> = foes.iter().copied().filter(|f| dist(f) <= 8.0).collect();
            flee_from = flee_from.max(near6.len());
            panic_from = panic_from.max(near8.len());
            seen = seen.max(near8.iter().filter(|f| redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z))).count());
            // PANIC's flee: every hostile within 6 (the largest share over them and the points; the
            // exact enumeration's cells a share of 0 misses kept at 0: an offset of exactly 16)
            for f in &near6 {
                merge(&mut flee_s, stroll::flee_pick_shares(w, i, 16.0, 7, *f));
                let edge: Vec<(BlockPos, f64)> = stroll::flee_candidates(w, i, 16.0, 7, *f).into_iter().map(|c| (c, 0.0)).collect();
                merge(&mut flee_s, edge);
            }
            // VillageBoundRandomStroll (PANIC's 2/2, IDLE's 10/7): in a village a land draw around;
            // else DefaultRandomPos toward the section closest to the village within 2, or a land
            // draw when that section is its own
            let me_b = w.entities.list[i].block_position();
            let v = redsim::entity::mob::ai::poi::is_village(w, me_b);
            village |= v;
            let s = (me_b.x.div_euclid(16), me_b.y.div_euclid(16), me_b.z.div_euclid(16));
            let best = if v { s } else { redsim::entity::mob::ai::poi::section_closest_to_village(w, s, 2) };
            let village_stroll = |w: &redsim::World, h: f64, vv: i32| {
                if best != s {
                    let c = ((best.0 * 16 + 8) as f64 + 0.5, (best.2 * 16 + 8) as f64 + 0.5);
                    stroll::pick_shares(w, i, h, vv, Some((c.0 - me.x, c.1 - me.z)), false, 10)
                } else {
                    stroll::pick_shares(w, i, h, vv, None, true, 10)
                }
            };
            if !near8.is_empty() {
                merge(&mut panic_s, village_stroll(w, 2.0, 2));
            }
            merge(&mut idle_s, village_stroll(w, 10.0, 7));
        }
        let me = put(w, p0);
        let mut judge = |w: &mut redsim::World, shares: Vec<(BlockPos, f64)>| -> Source {
            let cells = shares.len();
            let total: f64 = shares.iter().map(|(_, s)| *s).sum();
            let mut unreached: Vec<(BlockPos, f64)> = shares.into_iter().filter(|(c, _)| !reaches(w, *c)).collect();
            unreached.sort_by(|a, b| b.1.total_cmp(&a.1));
            let reached_share = (total - unreached.iter().map(|(_, s)| *s).sum::<f64>()).max(0.0);
            Source { cells, unreached, reached_share }
        };
        let flee = judge(w, flee_s);
        let panic_stroll = judge(w, panic_s);
        let stroll_src = judge(w, idle_s);
        // IDLE's walks to another villager within 8 (InteractWith; SetWalkTargetFromLookTarget): every
        // other rider place within 8, its rider's cell
        let mut others: Vec<(BlockPos, f64)> = Vec::new();
        for (o, _) in &rails {
            let q = rider_at(*o);
            if *o != cell && ((me.x - q.x).powi(2) + (me.y - q.y).powi(2) + (me.z - q.z).powi(2)).sqrt() <= 8.0 {
                others.push((*o, 1.0));
            }
        }
        let others = judge(w, others);
        let home_reached = home.map(|h| reaches(w, h));
        let panic_declared = panic_boxes.iter().any(|(a, b)| (a[0]..=b[0]).contains(&cell.x) && (a[1]..=b[1]).contains(&cell.y) && (a[2]..=b[2]).contains(&cell.z));
        out.push(Place { cell, on_foot, flee_from, panic_from, seen, village, flee, panic_stroll, stroll: stroll_src, others, home_reached, near, worst, panic_declared, panic_ok });
        let _ = redsim::entity::kill(w, vid);
        if let Some(c) = cart {
            let _ = redsim::entity::kill(w, c);
        }
        w.entities.list.retain(|e| !e.removed);
    }
    Ok(out)
}

/// `mctas mobfarm flee-check <design>`: the report, `show` cells a place; the verdicts last.
pub fn report(places: &[Place], jobs: &[(BlockPos, String)], show: usize) -> Vec<String> {
    report_with(places, jobs, show, false)
}

/// [`report`], with `clearing`: every place's clearing line (what can clear a stamp there).
pub fn report_with(places: &[Place], jobs: &[(BlockPos, String)], show: usize, clearing: bool) -> Vec<String> {
    let mut lines = clearing_lines(places, clearing);
    lines.extend(report_body(places, jobs, show));
    lines
}

/// What can clear a rider's stamp at each place (ironspec Nov 11 11:07; design numbers, not a
/// bar): panicking (a zombie seen within 8), the panic stroll's cells within 2 and 2 its path
/// reaches; calm, the calm stroll's chance an event picks a reachable cell.
fn clearing_lines(places: &[Place], each: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let panicking = |p: &Place| p.seen > 0;
    let pan_ok = places.iter().filter(|p| panicking(p) && p.panic_stroll.cells > p.panic_stroll.unreached.len()).count();
    let pan_none = places.iter().filter(|p| panicking(p) && p.panic_stroll.cells <= p.panic_stroll.unreached.len()).count();
    let calm_ok = places.iter().filter(|p| !panicking(p) && p.stroll.reached_share > 1e-9).count();
    let calm_none = places.iter().filter(|p| !panicking(p) && p.stroll.reached_share <= 1e-9).count();
    lines.push(format!(
        "what can clear a stamp, per place (design numbers, not a bar; the oldest-count rows decide): {} panicking places (a zombie seen within 8) with a reachable panic-stroll cell, {} with none; {} calm places whose calm stroll can pick a reachable cell, {} with none",
        pan_ok, pan_none, calm_ok, calm_none
    ));
    for p in places.iter().filter(|p| each || (panicking(p) && p.panic_stroll.cells <= p.panic_stroll.unreached.len()) || (!panicking(p) && p.stroll.reached_share <= 1e-9)) {
        let tag = if each { "CLEARING" } else { "NO-CLEARING" };
        if panicking(p) {
            lines.push(format!("  {tag} {},{},{}{}: panicking ({} seen within 8); the panic stroll reaches {} of its {} cells (an event picks one {:.0}%)", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, p.seen, p.panic_stroll.cells - p.panic_stroll.unreached.len(), p.panic_stroll.cells, 100.0 * p.panic_stroll.reached_share));
        } else {
            lines.push(format!("  {tag} {},{},{}{}: calm (no zombie seen within 8); the calm stroll reaches {} of its {} cells (an event picks one {:.1}%)", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, p.stroll.cells - p.stroll.unreached.len(), p.stroll.cells, 100.0 * p.stroll.reached_share));
        }
    }
    lines
}

fn report_body(places: &[Place], jobs: &[(BlockPos, String)], show: usize) -> Vec<String> {
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
                "  {name} {},{},{}{} ({} hostile(s) within 6, {} within 8, {} in sight as placed): {} cells, {} unreached{}: {}",
                p.cell.x,
                p.cell.y,
                p.cell.z,
                if p.on_foot { " on foot" } else { "" },
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
    // (the 6-8 rule, ironspec Nov 11 10:54 (scene flee-distance bridged): every zombie at least
    // FLEE_BAR (6.05) from every rider position, measured from where it sits in its post; a place
    // declared in `panic:` sees one within PANIC_BAR (7.95) from every point. Each place's nearest
    // zombie, least and greatest over its points, and the closest pair overall)
    let inside: Vec<&Place> = places.iter().filter(|p| p.near.is_some_and(|n| n.0 < FLEE_BAR)).collect();
    let beyond = places.iter().filter(|p| p.near.is_none_or(|n| n.1 > PANIC_BAR)).count();
    let declared: Vec<&Place> = places.iter().filter(|p| p.panic_declared).collect();
    let declared_bad: Vec<&Place> = declared.iter().copied().filter(|p| !p.panic_ok).collect();
    lines.push(format!(
        "the 6-8 rule (every zombie at least {FLEE_BAR} from every rider position, where it sits in its post; a declared panic place sees one within {PANIC_BAR}): {} rider places inside {FLEE_BAR}, {} whose nearest zombie is beyond {PANIC_BAR} somewhere, {} at {FLEE_BAR}..{PANIC_BAR} throughout; {} places declared in panic:, {} of them without a seen zombie within {PANIC_BAR} at some point",
        inside.len(),
        beyond,
        places.iter().filter(|p| p.near.is_some_and(|n| n.0 >= FLEE_BAR && n.1 <= PANIC_BAR)).count(),
        declared.len(),
        declared_bad.len()
    ));
    if let Some((p, (d, me, f))) = places.iter().filter_map(|p| p.worst.map(|w| (p, w))).min_by(|a, b| a.1.0.total_cmp(&b.1.0)) {
        lines.push(format!("the closest rider-zombie pair: {d:.3}, the rider at {:.3},{:.3},{:.3} (place {},{},{}{}), the zombie at {:.3},{:.3},{:.3}", me.x, me.y, me.z, p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, f.x, f.y, f.z));
    }
    for p in inside.iter().take(show.max(1) * 20) {
        let (lo, hi, h) = p.near.expect("a place inside the bar has a zombie");
        lines.push(format!("  INSIDE {},{},{}{}: the nearest zombie {lo:.2}..{hi:.2} (at {:.2},{:.2},{:.2})", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, h.x, h.y, h.z));
    }
    for p in declared_bad.iter().take(show.max(1) * 20) {
        lines.push(format!("  PANIC-UNCOVERED {},{},{}: declared in panic:, no seen zombie within {PANIC_BAR} at some point (nearest {})", p.cell.x, p.cell.y, p.cell.z, p.near.map_or("none".to_string(), |n| format!("{:.2}..{:.2}", n.0, n.1))));
    }
    for p in places.iter().filter(|p| p.near.is_none_or(|n| n.1 > PANIC_BAR)).take(show.max(1) * 20) {
        match p.near {
            Some((lo, hi, h)) => lines.push(format!("  BEYOND {},{},{}{}: the nearest zombie {lo:.2}..{hi:.2} (at {:.2},{:.2},{:.2}), {} seen within 8", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, h.x, h.y, h.z, p.seen)),
            None => lines.push(format!("  BEYOND {},{},{}: no zombie in the design", p.cell.x, p.cell.y, p.cell.z)),
        }
    }
    lines.push(format!("the 6-8 gate (a place inside {FLEE_BAR}, or a declared panic place beyond {PANIC_BAR} or out of sight): {}", verdict(inside.len() + declared_bad.len())));
    lines.push(format!(
        "verdicts (every cell a source can pick reachable wherever it runs): flee {}, panic stroll {}, idle stroll {}, walks to villagers {}",
        verdict(flee_bad),
        verdict(panic_bad),
        verdict(stroll_bad),
        verdict(others_bad)
    ));
    lines
}
