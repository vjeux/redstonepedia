//! (b8949c96 2026-11-12) The way-in line's queue cell in the loop frame, against a module file:
//! ironspec's loop-frame acceptance and G1's five points on 78d4acf34 (farmopt 07:40).
//!
//! - (a) the queue rider cannot summon: a panicking villager calls `spawnGolemIfNeeded(level, T,
//!   3)` (VillagerPanicTrigger), counting every villager whose box meets its own box inflated by
//!   10 (itself included); so no position of another villager but the one other napper's (the
//!   stop and its lift: one villager, A) may lie in that box. The napper's own route (the shaft,
//!   the queue, the drop, the run) is the rider itself.
//! - (b) its zombie frightens no one else: from every position but the rider's own route, at
//!   least 6.05 (SetWalkTargetAwayFrom's 6) and, within 8 (VillagerHostilesSensor's zombie
//!   reach), out of sight (`LivingEntity.hasLineOfSight`, eye to eye, Block.COLLIDER);
//! - (c) the post: inside the module's chunk, 6.05..7.95 from the rider (panic, no flee walk),
//!   in sight of it, its eye cell under 15 sky light (leaves over it), on glass (no golem floor);
//!   the way from the queue to the stop: every own-route position within 8 of the zombie and in
//!   its sight printed (the panic must end before the bed walk).
//!
//! Positions are cells a villager's feet can be in (the module's `.positions.txt`: riders' cells,
//! rail cells, declared off-rail cells); a villager there is taken as 0.6 wide, its box from the
//! cell's floor up to 2.95 (a rider's offset included), its eye 1.62 over the floor.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

pub type P = [i32; 3];
pub type B6 = [i32; 6];

#[derive(Clone, Debug)]
pub struct Opts {
    /// The queue cart's cell.
    pub queue: P,
    /// The zombie's boat cell (glass under it, leaves 3 over it); None: scan every free cell.
    pub post: Option<P>,
    /// Boxes of the napper's own route (x0,y0,z0,x1,y1,z1, inclusive).
    pub own: Vec<B6>,
    /// Boxes where the one other napper can be (the stop, its lift).
    pub napper: Vec<B6>,
    /// Boxes no post block may take (the reserved routes, the controller box).
    pub keep_clear: Vec<B6>,
    /// The queue cell as a powered rail on glowstone sloping down to the west (unpowered it holds
    /// the cart, powered it lets it roll down the run; the rider's own cell over a solid-render
    /// floor), not a shut trapdoor.
    pub rail: bool,
    /// The module's chunk: x0, z0, x1, z1.
    pub chunk: [i32; 4],
    pub positions: Vec<P>,
    /// Boxes of the cells a pad golem's feet can be in (the pad's summon spots): none of their
    /// melee reach may meet the zombie (onechunk 08:42: a golem's attack box, its box inflated
    /// across by sqrt(2.04) - 0.6, meeting the zombie's: it attacks and kills the scarer).
    pub pad: Vec<B6>,
    /// More cells to lay with the queue (the run, an occluder): position and block state.
    pub extra: Vec<(P, String)>,
    /// The post's own cells come in `extra` (a walled post: floor, walls, roof, occluders): no
    /// default glass under the boat and leaves over the eye.
    pub custom_post: bool,
    /// Extra cells that replace the module's block there (`!x,y,z=...`).
    pub replace: Vec<P>,
    /// The queue's staircase descends south (slopes ascending north: the corner shaft (9,.,-5)),
    /// not west.
    pub south: bool,
    /// The queue rail's floor glass, not glowstone (its clearer elsewhere).
    pub queue_glass: bool,
    /// The ride rows' cart spawned here instead of the queue rail's centre (a real rest point).
    pub seat_at: Option<[f64; 3]>,
    /// (C)'s entry bar at A6's entry (ironspec 21:25: A6's operating band [0.11, 0.24], inherited from
    /// 58f770cc's park sweep).
    pub band: [f64; 2],
    /// (ironspec 22:19 (1)) record7's built R1 with day-1's line-bench levers instead of the torch
    /// stand-in; the release is NOT-ready's lever flipped off.
    pub r1_built: bool,
    /// (R6, V: ironspec 20:35 (2) (c), day-1 20:40) the built R1's layout, laid relative to the queue:
    /// record7's (today's) or V's (N's comparator re-routed down to the queue's level).
    pub r1_layout: R1Layout,
    /// (ironspec 02:33 / 03:01) visit rows with the night skipped this many gt after the release.
    pub visit_skip: Vec<i64>,
    /// (ironspec 03:22 / 03:30 (2)) a NoAI villager standing here through every visit row (the control).
    pub bystander: Option<[f64; 3]>,
    /// (farmopt 04:11, the positive control) every visit row's CANT_REACH stamp erased at the release.
    pub visit_clear_stamp: bool,
    /// (G1 via farmopt 05:08, the "0 HOME lost" bar's known positive) a CANT_REACH stamp planted at
    /// every visit row's release, already this many gt old.
    pub visit_plant_stamp: Option<i64>,
    /// (G1 05:41 / 05:44, the HOME LOST bar's known positive, in the world) a second villager asleep in
    /// the napper's bed from every visit row's start.
    pub visit_plant_erase: bool,
    /// (G1 05:50) start state (ii): the napper holds its bed's ticket (the default is (i), the design
    /// start: HOME a memory, the ticket free).
    pub visit_start_held: bool,
    /// (G1 (14)) FARM-STOP's "lay down, then no HOME" positive: the napper's bed broken once it lies
    /// down.
    pub visit_break_bed: bool,
    /// (tick-exact's forced holds, 04:29) the visit's cart summoned where `--leg` says and held there:
    /// no leg to the arrival detector, the rows start at once (a hold past the row's end never releases).
    pub visit_at_rest: bool,
    /// (ironspec 05:09, the R2 search's gate) the HOME target of the ride rows' per-tick probe: the
    /// path from the rider's actual state at every descent tick (bar 0 partial).
    pub probe_bed: Option<P>,
    /// (G1 03:27, ironspec 03:30 (2)) the cells the ride rows watch: the rider's box and in-wall eye
    /// box against each one's full cube every tick (bar 0 eye overlaps where the cell is solid).
    pub watch_cells: Vec<P>,
    /// The design's declared contacts (its `# contacts:` header entries): a ride row touching only
    /// these prints them as declared, not as a contact failure.
    pub declared_contacts: Vec<P>,
    /// (ironspec 10:42 / 10:58, main's ONE bounded judge) the design's bounded `# contacts:` / `# stops:`
    /// declarations in the ride's own scope (the way-in's box), read strictly by
    /// `contacts::declared_contacts_bounded` ([`bounded_in_scope`]): every bar row is judged by
    /// `contacts::judge` against them.
    pub declared_bounded: Vec<crate::contacts::Decl>,
    /// This run is the declarations' own `ref=` run: `contacts::reference_check` mismatches FAIL it
    /// (printed only otherwise).
    pub reference_run: bool,
    /// (ironspec 18:39 (4)) the rail end's all-clear speed (the start-phase sweep's figure): the ride
    /// rows' entry speed into (9,-15,-2) must be at least it + 10 % (None: printed only).
    pub rail_end_least: Option<f64>,
    /// (R6; ironspec 21:54) the bed head's block: the ride rows print the queue cart's distance at rest
    /// to its centre and judge it > 17 (None: not printed).
    pub ball_head: Option<P>,
}

/// `x,y,z=block;...` (a block state, `minecraft:` optional; a leading `!` on a cell replaces what
/// the module holds there).
pub fn parse_extra(s: Option<&str>) -> Result<Vec<(P, String)>, String> {
    let Some(s) = s else { return Ok(Vec::new()) };
    s.split(';')
        .filter(|x| !x.trim().is_empty())
        .map(|c| {
            let c = c.trim().trim_start_matches('!');
            let (p, b) = c.split_once('=').ok_or(format!("{c:?}: x,y,z=block"))?;
            let v: Vec<i32> = p.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("{c:?}: {e}"))).collect::<Result<_, _>>()?;
            if v.len() != 3 {
                return Err(format!("{c:?}: x,y,z=block"));
            }
            let b = b.trim();
            Ok(([v[0], v[1], v[2]], if b.contains(':') { b.to_string() } else { format!("minecraft:{b}") }))
        })
        .collect()
}

pub fn parse_boxes(s: Option<&str>) -> Result<Vec<B6>, String> {
    let Some(s) = s else { return Ok(Vec::new()) };
    s.split(';')
        .filter(|x| !x.trim().is_empty())
        .map(|b| {
            let v: Vec<i32> = b.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("box {b:?}: {e}"))).collect::<Result<_, _>>()?;
            if v.len() != 6 {
                return Err(format!("box {b:?}: six numbers x0,y0,z0,x1,y1,z1"));
            }
            Ok([v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5]), v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])])
        })
        .collect()
}

pub fn read_positions(text: &str) -> Result<Vec<P>, String> {
    let mut v = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let n: Vec<i32> = l.split_whitespace().map(|x| x.parse::<i32>().map_err(|e| format!("{l:?}: {e}"))).collect::<Result<_, _>>()?;
        if n.len() != 3 {
            return Err(format!("{l:?}: x y z"));
        }
        v.push([n[0], n[1], n[2]]);
    }
    Ok(v)
}

fn in_box(p: &P, b: &B6) -> bool {
    p[0] >= b[0] && p[0] <= b[3] && p[1] >= b[1] && p[1] <= b[4] && p[2] >= b[2] && p[2] <= b[5]
}

/// (ironspec 10:30: the eye model tick-exact's run agrees with, 9fc3155f's) a position's eye: the
/// cart's position + 1.62 on a rail (a slope's middle 0.5 up, a flat rail 0.0625 up), the feet + 1.62
/// on foot.
fn eye_in(w: &redsim::world::World, p: &P) -> Vec3 {
    let b = w.get(BlockPos::new(p[0], p[1], p[2]));
    let up = match b {
        Block::Rail { shape, .. } | Block::PlainRail { shape, .. } | Block::DetectorRail { shape, .. } => {
            if shape.is_ascending() {
                0.5
            } else {
                0.0625
            }
        }
        _ => 0.0,
    };
    Vec3::new(p[0] as f64 + 0.5, p[1] as f64 + up + 1.62, p[2] as f64 + 0.5)
}

fn feet_of(p: &P) -> Vec3 {
    Vec3::new(p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5)
}

/// Every cell placed, or the run fails naming the cells outside the world's box (ironspec 11:05:
/// nothing silently dropped; `World::place_raw` skips a cell outside its box without a word, which
/// let the first east ride's casing vanish).
fn place_checked(w: &mut redsim::world::World, cells: &[(BlockPos, Block)]) -> Result<(), String> {
    let inside = |p: &BlockPos| p.x >= w.min.x && p.x < w.min.x + w.size[0] && p.y >= w.min.y && p.y < w.min.y + w.size[1] && p.z >= w.min.z && p.z < w.min.z + w.size[2];
    let out: Vec<BlockPos> = cells.iter().map(|(p, _)| *p).filter(|p| !inside(p)).collect();
    if !out.is_empty() {
        return Err(format!(
            "{} way-in cell(s) outside the world's box {}..{}: {}",
            out.len(),
            w.min,
            BlockPos::new(w.min.x + w.size[0] - 1, w.min.y + w.size[1] - 1, w.min.z + w.size[2] - 1),
            out.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")
        ));
    }
    w.place_raw(cells);
    // (9fc3155f's audit, farmopt 11:45: an ascending rail with no rigid block on its high side, or
    // any rail with no rigid top under it, pops at its first update: refuse it here, as the shared
    // composer's `Run::start` now does)
    let pops: Vec<BlockPos> = cells.iter().map(|(p, _)| *p).filter(|p| redsim::rails::would_pop(w, *p)).collect();
    if !pops.is_empty() {
        return Err(format!("{} way-in rail(s) would pop (no rigid block under them, or none on a slope's high side): {}", pops.len(), pops.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")));
    }
    Ok(())
}

/// (ironspec 04:07: a bench override that changes nothing fails the run) The bench's cells laid over
/// the design: the first `explicit` are `--extra` overrides, the rest emulated cells (the queue's
/// stand-in cells, R1's built cells and inputs). Each cell that changes its cell is laid and printed
/// before -> after; an emulated cell the design already holds as laid is not an override (counted);
/// an `--extra` override that changes nothing FAILS.
fn place_overrides(w: &mut redsim::world::World, explicit: usize, cells: &[(BlockPos, Block)]) -> Result<Vec<String>, String> {
    let (mut laid, mut lines, mut in_file, mut noop): (Vec<(BlockPos, Block)>, Vec<String>, usize, Vec<String>) = (Vec::new(), Vec::new(), 0, Vec::new());
    for (k, (p, b)) in cells.iter().enumerate() {
        let before = w.get(*p);
        if before == *b {
            if k < explicit {
                noop.push(format!("{p} {b:?}"));
            } else {
                in_file += 1;
            }
            continue;
        }
        lines.push(format!("  {p}: {before:?} -> {b:?} ({})", if k < explicit { "--extra" } else { "emulated" }));
        laid.push((*p, b.clone()));
    }
    if !noop.is_empty() {
        return Err(format!("{} --extra override(s) change nothing (ironspec 04:07: a no-op override fails the run): {}", noop.len(), noop.join("; ")));
    }
    place_checked(w, &laid)?;
    // (R6; ironspec 21:55's standing rule) every laid cell READ BACK: a cell the world doesn't hold as
    // laid (a rail that popped, a dust that reshaped on its neighbours' updates) fails the run by name
    let mismatched: Vec<String> = laid.iter().filter(|(p, b)| w.get(*p) != *b).map(|(p, b)| format!("{p}: laid {b:?}, read back {:?}", w.get(*p))).collect();
    if !mismatched.is_empty() {
        return Err(format!("{} override(s) not held as laid (read back): {}", mismatched.len(), mismatched.join("; ")));
    }
    lines.insert(0, format!("(overrides) {} cell(s) laid over the design, each before -> after; {in_file} emulated cell(s) the design already holds as laid (not overrides, not laid)", laid.len()));
    Ok(lines)
}

/// The design's box grown to hold every cell the way-in lays (the queue, the extra cells, the scan's
/// chunk, a margin): redsim's world is the design's box, and a cell outside it is never placed (the
/// east gap chunk's shaft, casing and queue).
fn widened(spec: &FarmSpec, o: &Opts) -> FarmSpec {
    let mut s = spec.clone();
    let mut grow = |p: P| {
        s.lo = BlockPos::new(s.lo.x.min(p[0] - 3), s.lo.y.min(p[1] - 4), s.lo.z.min(p[2] - 3));
        s.hi = BlockPos::new(s.hi.x.max(p[0] + 3), s.hi.y.max(p[1] + 4), s.hi.z.max(p[2] + 3));
    };
    grow(o.queue);
    for (p, _) in &o.extra {
        grow(*p);
    }
    if let Some(p) = o.post {
        grow(p);
    }
    let [x0, z0, x1, z1] = o.chunk;
    grow([x0, o.queue[1], z0]);
    grow([x1, o.queue[1], z1]);
    s
}

/// The module built as the judge places it, with the post's glass and leaves and its zombie boat
/// when given, and the queue cart with its rider; a few ticks to settle. The rider's and the
/// zombie's (feet, eye), the world.
fn scene(spec: &FarmSpec, o: &Opts, post: Option<P>) -> Result<(redsim::world::World, (Vec3, Vec3), Option<(Vec3, Vec3)>), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut w = run.world;
    // (the extra cells: the run, an occluder)
    let mut extra = o.extra.iter().map(|(p, b)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(b).map_err(|e| format!("{b}: {e}"))?))).collect::<Result<Vec<_>, String>>()?;
    extra.extend(r1_cells(o)?);
    // (G1 07:16 (6)) the override rule here too: every cell laid printed before -> after, a no-op
    // --extra failing
    for l in place_overrides(&mut w, o.extra.len(), &extra)? {
        println!("{l}");
    }
    settle_r1(&mut w, o)?;
    if let Some(p) = post {
        if !o.custom_post {
            // (G1 07:16 (6)) the post's bench cells through the override rule (printed when they change)
            let post_cells = [
                (BlockPos::new(p[0], p[1] - 1, p[2]), Block::parse("minecraft:glass").map_err(|e| e.to_string())?),
                (BlockPos::new(p[0], p[1] + 3, p[2]), Block::parse("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]").map_err(|e| e.to_string())?),
            ];
            for l in place_overrides(&mut w, 0, &post_cells)? {
                println!("{l}");
            }
        }
        redsim::entity::spawn(&mut w, "minecraft:oak_boat", [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    }
    let q = o.queue;
    // (the queue cell: a shut bottom-half iron trapdoor the cart rests on, 3/16 high; or with
    // `rail`, a powered rail on glowstone ascending to the east, the cart at its middle)
    let qy = if o.rail {
        let cells = queue_cells(o).iter().map(|(p, b, _)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(b).map_err(|e| e.to_string())?))).collect::<Result<Vec<_>, String>>()?;
        for l in place_overrides(&mut w, 0, &cells)? {
            println!("{l}");
        }
        q[1] as f64 + 0.5625
    } else {
        if w.get(BlockPos::new(q[0], q[1], q[2])).is_air() {
            let cells = [(BlockPos::new(q[0], q[1], q[2]), Block::parse("minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]").map_err(|e| e.to_string())?)];
            for l in place_overrides(&mut w, 0, &cells)? {
                println!("{l}");
            }
        }
        q[1] as f64 + 0.1875
    };
    let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [q[0] as f64 + 0.5, qy, q[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
    let rider = w.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart)).ok_or("no rider in the queue cart")?;
    let r = (rider.pos, Vec3::new(rider.pos.x, rider.pos.y + rider.kind.eye_height() as f64, rider.pos.z));
    let z = post.and_then(|p| {
        w.entities
            .list
            .iter()
            .find(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty.game_id() == "minecraft:zombie") && (e.pos.x - (p[0] as f64 + 0.5)).abs() < 1.0 && (e.pos.z - (p[2] as f64 + 0.5)).abs() < 1.0)
            .map(|e| (e.pos, Vec3::new(e.pos.x, e.pos.y + e.kind.eye_height() as f64, e.pos.z)))
    });
    Ok((w, r, z))
}

/// (a): the positions of other villagers inside the queue rider's summon box.
pub fn summon_box_lines(spec: &FarmSpec, o: &Opts) -> Result<(bool, Vec<String>), String> {
    let (_w, (feet, _), _) = scene(spec, o, None)?;
    // the rider's box (0.6 x 1.95) inflated by 10
    let (lo, hi) = (Vec3::new(feet.x - 10.3, feet.y - 10.0, feet.z - 10.3), Vec3::new(feet.x + 10.3, feet.y + 11.95, feet.z + 10.3));
    let meets = |p: &P| {
        let (a, b) = (Vec3::new(p[0] as f64 + 0.2, p[1] as f64, p[2] as f64 + 0.2), Vec3::new(p[0] as f64 + 0.8, p[1] as f64 + 2.95, p[2] as f64 + 0.8));
        a.x < hi.x && b.x > lo.x && a.y < hi.y && b.y > lo.y && a.z < hi.z && b.z > lo.z
    };
    let mut others = Vec::new();
    let mut nappers = 0usize;
    for p in o.positions.iter().filter(|p| meets(p)) {
        if o.own.iter().any(|b| in_box(p, b)) {
            continue;
        }
        if o.napper.iter().any(|b| in_box(p, b)) {
            nappers += 1;
            continue;
        }
        others.push(*p);
    }
    let pass = others.is_empty();
    let mut out = vec![format!(
        "(a) the queue rider at ({:.2},{:.2},{:.2}): its summon box (its box inflated by 10: y {:.2}..{:.2}) holds {} position(s) of other villagers{}; {} of the one other napper's (the stop, its lift: one villager), the rider's own route left out: {}",
        feet.x,
        feet.y,
        feet.z,
        lo.y,
        hi.y,
        others.len(),
        if others.is_empty() { String::new() } else { format!(" {:?}", others.iter().take(12).collect::<Vec<_>>()) },
        nappers,
        if pass { "it can never count 3 (itself and at most the napper): PASS" } else { "FAIL" }
    )];
    // the nearest other position at every height band, as a margin
    if let Some(m) = o.positions.iter().filter(|p| !o.own.iter().any(|b| in_box(p, b)) && !o.napper.iter().any(|b| in_box(p, b))).map(|p| (p[1] as f64 - (feet.y + 11.95), *p)).filter(|(d, _)| *d >= 0.0).min_by(|a, b| a.0.total_cmp(&b.0)) {
        out.push(format!("    the margin above: the lowest other position over the box {:?}, its floor {:.2} over the box's top", m.1, m.0));
    }
    Ok((pass, out))
}

/// (b) and (c) for one post: every check's line, and whether all pass.
pub fn post_lines(spec: &FarmSpec, o: &Opts, post: P) -> Result<(bool, Vec<String>), String> {
    let mut out = Vec::new();
    let mut pass = true;
    let [x0, z0, x1, z1] = o.chunk;
    let cells: Vec<P> = if o.custom_post { vec![post, [post[0], post[1] + 1, post[2]]] } else { vec![post, [post[0], post[1] - 1, post[2]], [post[0], post[1] + 1, post[2]], [post[0], post[1] + 2, post[2]], [post[0], post[1] + 3, post[2]]] };
    let inside = cells.iter().all(|c| c[0] >= x0 && c[0] <= x1 && c[2] >= z0 && c[2] <= z1);
    let (w0, _, _) = scene(spec, o, None)?;
    let taken: Vec<P> = cells.iter().filter(|c| !w0.get(BlockPos::new(c[0], c[1], c[2])).is_air()).copied().collect();
    let reserved: Vec<P> = cells.iter().filter(|c| o.keep_clear.iter().chain(o.own.iter()).any(|b| in_box(c, b))).copied().collect();
    pass &= inside && taken.is_empty() && reserved.is_empty();
    out.push(format!(
        "(c) the post {:?}: its cells (glass, boat, the zombie's head, air, leaves) {}{}{}",
        post,
        if inside { "inside the chunk" } else { "OUTSIDE the chunk (FAIL)" },
        if taken.is_empty() { String::new() } else { format!("; taken in the module {taken:?} (FAIL)") },
        if reserved.is_empty() { String::new() } else { format!("; on a reserved route {reserved:?} (FAIL)") }
    ));
    let (w, (rf, re), z) = scene(spec, o, Some(post))?;
    let Some((zf, ze)) = z else {
        out.push("(c) no zombie at the post after the spawn (FAIL)".to_string());
        return Ok((false, out));
    };
    let d = rf.distance_to_sqr(zf).sqrt();
    let sight = redsim::entity::mob::los::has_line_of_sight(&w, re, ze);
    let eye = BlockPos::new(ze.x.floor() as i32, ze.y.floor() as i32, ze.z.floor() as i32);
    // (the burn rule: `Zombie.isSunBurnTick` needs `level.canSeeSky` at its eye, a sky light of
    // 15, which is exactly a sky source: `daylight::sky_source`'s column scan from the world's top
    // down to the eye (exact, and independent of light the flood fill cannot model nearby, e.g. a
    // bubble column within 15); the eye's sky light printed when the flood fill models it)
    let sees_sky = redsim::daylight::sky_source(&w, eye);
    let sky = redsim::daylight::sky_light(&w, eye);
    let ok_d = d > 6.05 && d < 7.95;
    let ok_sky = matches!(sees_sky, Ok(false));
    pass &= ok_d && sight && ok_sky;
    out.push(format!(
        "(c) the zombie at ({:.2},{:.2},{:.2}): from the rider {d:.3} (6.05..7.95, inherited: ironspec's 6-8 rule: {}); in its sight {} ; its eye cell ({},{},{}) {} (sky light {})",
        zf.x,
        zf.y,
        zf.z,
        if ok_d { "PASS" } else { "FAIL" },
        if sight { "yes (PASS)" } else { "NO (FAIL)" },
        eye.x,
        eye.y,
        eye.z,
        match &sees_sky {
            Ok(false) => "is no sky source (its column occluded above it: canSeeSky false): no sunburn, PASS".to_string(),
            Ok(true) => "SEES THE SKY (FAIL)".to_string(),
            Err(e) => format!("{e} (FAIL)"),
        },
        sky.as_ref().map_or_else(|e| format!("not printed: {e}"), |l| l.to_string())
    ));
    // (b): every other villager's position
    let (mut near, mut seen_near, mut nearest) = (Vec::new(), Vec::new(), f64::INFINITY);
    let mut route = Vec::new();
    for p in &o.positions {
        if *p == o.queue {
            continue;
        }
        let dd = feet_of(p).distance_to_sqr(zf).sqrt();
        let own = o.own.iter().any(|b| in_box(p, b));
        let sees = dd < 8.0 && redsim::entity::mob::los::has_line_of_sight(&w, eye_in(&w, p), ze);
        if own {
            if sees {
                route.push((*p, dd));
            }
            continue;
        }
        nearest = nearest.min(dd);
        if dd < 6.05 {
            near.push((*p, dd));
        }
        if sees {
            seen_near.push((*p, dd));
        }
    }
    let ok_b = near.is_empty() && seen_near.is_empty();
    // (ironspec 10:20: the 6.05 bar is sight-gated, but a hidden position within 6.05 passes only on
    // robust hiding) the "hidden within 6.05" class: every ray from the position's eye, sampled
    // every 0.1 up its cell (the cart's position + 1.62), to the zombie's eye must end on a
    // full-collider block (`isCollisionShapeFullBlock`: glass or solid; never a pane, a fence or a
    // partial block)
    let mut weak: Vec<(P, f64, String)> = Vec::new();
    for (p, dd) in &near {
        if seen_near.iter().any(|(q, _)| q == p) {
            continue;
        }
        for k in 0..=10 {
            let eye = Vec3::new(p[0] as f64 + 0.5, p[1] as f64 + 0.1 * k as f64 + 1.62, p[2] as f64 + 0.5);
            let hit = redsim::entity::mob::los::clip_collider(&w, eye, ze);
            if hit.miss {
                weak.push((*p, *dd, format!("in sight from y +{:.1}", 0.1 * k as f64)));
                break;
            }
            let b = w.get(hit.pos);
            if !redsim::behavior::full_cube(&b) {
                weak.push((*p, *dd, format!("hidden only by {b} at {}", hit.pos)));
                break;
            }
        }
    }
    let hidden: Vec<&(P, f64)> = near.iter().filter(|(p, _)| !seen_near.iter().any(|(q, _)| q == p)).collect();
    let ok_b = ok_b || (seen_near.is_empty() && weak.is_empty());
    pass &= ok_b;
    out.push(format!(
        "(b) every other position: the nearest {nearest:.2} from the zombie; within 8 and in sight: {}; hidden within 6.05 (inherited: ironspec 10:20): {} ({}) ({})",
        if seen_near.is_empty() { "none".to_string() } else { format!("{seen_near:?}") },
        if hidden.is_empty() { "none".to_string() } else { format!("{} position(s) {:?}", hidden.len(), hidden.iter().map(|(p, d)| format!("{p:?} {d:.2}")).collect::<Vec<_>>()) },
        if hidden.is_empty() { "-".to_string() } else if weak.is_empty() { "robust: every ray from eyes every 0.1 up each cell ends on a full-collider block".to_string() } else { format!("NOT robust: {weak:?}") },
        if ok_b { "PASS" } else { "FAIL" }
    ));
    // (e) the rider's own route (the shaft, the detectors, the run): no position under 6.05 in its
    // sight (ironspec's (b): the falling rider's pass hidden, or an exception)
    let route_close: Vec<&(P, f64)> = route.iter().filter(|(_, d)| *d < 6.05).collect();
    pass &= route_close.is_empty();
    out.push(format!(
        "(e) the rider's own route (the shaft, the detectors, the run): positions within 8 of the zombie and in its sight: {}; under 6.05 and in sight (inherited: G1's points): {} ({})",
        if route.is_empty() { "none".to_string() } else { format!("{route:?}") },
        if route_close.is_empty() { "none".to_string() } else { format!("{route_close:?}") },
        if route_close.is_empty() { "PASS" } else { "FAIL" }
    ));
    // (d) the pad golems' melee reach: `Mob.getAttackBoundingBox` (its box inflated across by
    // sqrt(2.04) - 0.6, not up) against the zombie's box
    if !o.pad.is_empty() {
        let reach = 2.04f64.sqrt() - 0.6;
        let (zlo, zhi) = (Vec3::new(zf.x - 0.3, zf.y, zf.z - 0.3), Vec3::new(zf.x + 0.3, zf.y + 1.95, zf.z + 0.3));
        let mut hits = Vec::new();
        let mut spots = 0usize;
        for b in &o.pad {
            for x in b[0]..=b[3] {
                for y in b[1]..=b[4] {
                    for z in b[2]..=b[5] {
                        spots += 1;
                        let (gx, gy, gz) = (x as f64 + 0.5, y as f64, z as f64 + 0.5);
                        let (alo, ahi) = (Vec3::new(gx - 0.7 - reach, gy, gz - 0.7 - reach), Vec3::new(gx + 0.7 + reach, gy + 2.7, gz + 0.7 + reach));
                        if alo.x < zhi.x && ahi.x > zlo.x && alo.y < zhi.y && ahi.y > zlo.y && alo.z < zhi.z && ahi.z > zlo.z {
                            hits.push([x, y, z]);
                        }
                    }
                }
            }
        }
        pass &= hits.is_empty();
        out.push(format!(
            "(d) the pad golems' melee reach ({spots} spot cells): {} (the zombie's box y {:.2}..{:.2})",
            if hits.is_empty() { "no spot reaches the zombie: PASS".to_string() } else { format!("{} spot(s) reach it {:?} (FAIL)", hits.len(), hits.iter().take(8).collect::<Vec<_>>()) },
            zlo.y,
            zhi.y
        ));
    }
    Ok((pass, out))
}

/// The seat's panic-stroll draws (`VillageBoundRandomStroll` 2/2, the village rule as the brain
/// takes it): `draws` of them from the queue rider, the share with no position (P_null), on its own
/// cell (a reached draw over a solid-render floor), on any other cell (each a walk its cart cannot
/// make: none-or-all wants none).
pub fn seat_lines(spec: &FarmSpec, o: &Opts, post: Option<P>, draws: usize) -> Result<(bool, Vec<String>), String> {
    let (mut w, _, _) = scene(spec, o, post)?;
    let vi = w
        .entities
        .list
        .iter()
        .position(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty.game_id() == "minecraft:villager") && e.vehicle.is_some() && (e.pos.x - (o.queue[0] as f64 + 0.5)).abs() < 1.0 && (e.pos.z - (o.queue[2] as f64 + 0.5)).abs() < 1.0)
        .ok_or("no rider at the queue")?;
    let me = w.entities.list[vi].block_position();
    let village = redsim::entity::mob::ai::poi::is_village(&w, me);
    let (mut null, mut own) = (0usize, 0usize);
    let mut other: std::collections::BTreeMap<P, usize> = std::collections::BTreeMap::new();
    for _ in 0..draws {
        let p = if village {
            redsim::entity::mob::ai::goal::stroll::land_random_pos(&mut w, vi, 2, 2)
        } else {
            let s = (me.x.div_euclid(16), me.y.div_euclid(16), me.z.div_euclid(16));
            let best = redsim::entity::mob::ai::poi::section_closest_to_village(&w, s, 2);
            if best != s {
                let c = Vec3::new((best.0 * 16 + 8) as f64 + 0.5, (best.1 * 16 + 8) as f64, (best.2 * 16 + 8) as f64 + 0.5);
                redsim::entity::mob::ai::goal::stroll::default_pos_towards(&mut w, vi, 2, 2, c, (std::f64::consts::PI / 2.0) as f32 as f64)
            } else {
                redsim::entity::mob::ai::goal::stroll::land_random_pos(&mut w, vi, 2, 2)
            }
        };
        match p {
            None => null += 1,
            Some(v) => {
                let c = [v.x.floor() as i32, v.y.floor() as i32, v.z.floor() as i32];
                if c == [me.x, me.y, me.z] {
                    own += 1;
                } else {
                    *other.entry(c).or_default() += 1;
                }
            }
        }
    }
    let ok = other.is_empty();
    Ok((
        ok,
        vec![format!(
            "(seat) the rider at ({},{},{}){}: {draws} panic-stroll draws (2/2): P_null {:.4}, its own cell {:.4}, other cells {} ({})",
            me.x,
            me.y,
            me.z,
            if village { " (a village section)" } else { "" },
            null as f64 / draws as f64,
            own as f64 / draws as f64,
            if other.is_empty() { "none".to_string() } else { format!("{other:?}") },
            if ok { "none-or-all: no walk ever fails at the queue, PASS" } else { "a cell its cart cannot reach: FAIL" }
        )],
    ))
}

/// The queue's rails with `rail` (one slope down to the west, a step a cell): the arrival detector
/// rail at the shaft's foot (queue + (1, 1, 0): the falling cart lands on it and rolls on; its
/// pulse sets the "held" latch and starts the hold timer), the holding powered rail at the queue
/// cell on glowstone (unpowered it brakes and holds the cart; day-1's gate powers it as a level),
/// the departure detector rail (queue - (1, 1, 0): its pulse resets the held latch and the RS), each
/// on its support.
/// record7's BUILT R1 and its hatch branch (day-1, onechunk's record7 aefff19cb) with day-1's
/// line-bench inputs (the three levers and the two lever supports are the declared stand-ins).
pub const R1_BUILT: [([i32; 3], &str); 28] = [
    // N's comparator: rear REST_l, side DAY's repeater
    ([6, -13, -2], "minecraft:glass"),
    ([6, -12, -2], "minecraft:comparator[facing=south,mode=subtract,powered=false]"),
    ([5, -13, -2], "minecraft:glass"),
    ([5, -12, -2], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
    // N's dust and the hatch branch
    ([6, -13, -3], "minecraft:smooth_stone"),
    ([6, -12, -3], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=side]"),
    ([6, -13, -4], "minecraft:glass"),
    ([6, -12, -4], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]"),
    ([6, -12, -5], "minecraft:iron_trapdoor[facing=east,half=bottom,open=false,powered=false,waterlogged=false]"),
    // the N line's steps down to R1
    ([5, -14, -3], "minecraft:smooth_stone"),
    ([5, -13, -3], "minecraft:redstone_wire[east=up,north=side,power=0,south=none,west=none]"),
    ([5, -15, -4], "minecraft:glass"),
    ([5, -14, -4], "minecraft:redstone_wire[east=side,north=none,power=0,south=up,west=none]"),
    ([6, -15, -4], "minecraft:glass"),
    ([6, -14, -4], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
    // R1's comparator (rear the N line, side the latch's dust) and the rail's repeater
    ([7, -15, -4], "minecraft:glass"),
    ([7, -14, -4], "minecraft:comparator[facing=west,mode=subtract,powered=false]"),
    ([7, -15, -5], "minecraft:glass"),
    ([7, -14, -5], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([8, -15, -4], "minecraft:glass"),
    ([8, -14, -4], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
    // the line-bench inputs (declared stand-ins): REST_l ON, DAY OFF, NOT-ready ON
    ([6, -13, -1], "minecraft:glass"),
    ([6, -12, -1], "minecraft:lever[face=floor,facing=north,powered=true]"),
    ([4, -13, -2], "minecraft:glass"),
    ([4, -12, -2], "minecraft:lever[face=floor,facing=north,powered=false]"),
    ([8, -14, -5], "minecraft:lever[face=wall,facing=west,powered=true]"),
    // the WIP's old (C') hatch repeater and its support, replaced by record7's (6,-12,-4)
    ([6, -12, -6], "minecraft:air"),
    ([6, -13, -6], "minecraft:air"),
];

/// (R6) Which built R1 the bench lays (relative to the queue): record7's, today's, or V's re-route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum R1Layout {
    #[default]
    Record7,
    V,
}

impl std::str::FromStr for R1Layout {
    type Err = String;
    fn from_str(s: &str) -> Result<R1Layout, String> {
        match s {
            "record7" => Ok(R1Layout::Record7),
            "v" | "V" => Ok(R1Layout::V),
            _ => Err(format!("--r1-layout {s}: record7 or v")),
        }
    }
}

/// (R6; ironspec 20:35 (2) (c), day-1 20:40: CONFIRMED AS LAID OUT) V's built R1, written for V's queue
/// (9,-10,-4): record7's line raised 4 (the rail's repeater, R1's comparator, the latch's dust, the N
/// line's repeater and dust), N's comparator re-routed down to (4,-10,-4) facing west (rear REST_l,
/// side DAY through one repeater (4,-10,-3) facing south), its output into the N line's dust; the old
/// step-down dust and the (C') hatch branch dropped; the line-bench inputs moved with it.
pub const R1_BUILT_V: [([i32; 3], &str); 19] = [
    // N's comparator: rear REST_l (west), side DAY's repeater (south), output east into the N line's dust
    ([4, -11, -4], "minecraft:glass"),
    ([4, -10, -4], "minecraft:comparator[facing=west,mode=subtract,powered=false]"),
    ([4, -11, -3], "minecraft:glass"),
    ([4, -10, -3], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]"),
    // the N line's dust and repeater into R1's comparator's rear
    ([5, -11, -4], "minecraft:glass"),
    ([5, -10, -4], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([6, -11, -4], "minecraft:glass"),
    ([6, -10, -4], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
    // R1's comparator (rear the N line, side the latch's dust) and the rail's repeater
    ([7, -11, -4], "minecraft:glass"),
    ([7, -10, -4], "minecraft:comparator[facing=west,mode=subtract,powered=false]"),
    ([7, -11, -5], "minecraft:glass"),
    ([7, -10, -5], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([8, -11, -4], "minecraft:glass"),
    ([8, -10, -4], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
    // the line-bench inputs (declared stand-ins): REST_l ON, DAY OFF, NOT-ready ON
    ([3, -11, -4], "minecraft:glass"),
    ([3, -10, -4], "minecraft:lever[face=floor,facing=north,powered=true]"),
    ([4, -11, -2], "minecraft:glass"),
    ([4, -10, -2], "minecraft:lever[face=floor,facing=north,powered=false]"),
    ([8, -10, -5], "minecraft:lever[face=wall,facing=west,powered=true]"),
];

/// Every fluid cell (water or lava, waterlogged blocks included) of the chunk box `[x0, z0, x1, z1]`
/// (y -64..=80) given its fluid tick in 5 gt, as a placement with updates schedules it; the count.
pub fn schedule_fluids(w: &mut redsim::world::World, chunk: [i32; 4]) -> usize {
    let mut n = 0;
    for x in chunk[0]..=chunk[2] {
        for z in chunk[1]..=chunk[3] {
            for y in -64..=80 {
                let p = BlockPos::new(x, y, z);
                let f = redsim::fluid::fluid_state(w, p);
                if f.fluid.is_water() {
                    w.schedule_fluid_tick(p, redsim::fluid::Fluid::Water, 5);
                    n += 1;
                }
            }
        }
    }
    n
}

/// (G1's W7 on R2) [`schedule_fluids`] for a bench that lays its cells raw: the chunk's water given its
/// tick, the count printed beside the world's water cells; water in the world with none scheduled is
/// a FAIL (the bench's water would stand frozen). Returns (the line, ok).
pub fn fluids_line(w: &mut redsim::world::World, chunk: [i32; 4], what: &str) -> (String, bool) {
    let n = schedule_fluids(w, chunk);
    let all = world_water_cells(w);
    let ok = !(all > 0 && n == 0);
    (format!("({what}) {n} water cell(s) of the chunk given their tick, of {all} in the world{}", if ok { "" } else { ": the water stands FROZEN (none scheduled): FAIL" }), ok)
}

/// (ironspec 10:42 / 10:58) one declared contact cell's figures over a run's bar rows: the bounded
/// form's reference (ticks, events as maximal consecutive runs, the most ticks in one event and events
/// in one row, the largest clipped amount per axis, the horizontal speed into and out of its ticks,
/// the ticks after which the cart did not move).
#[derive(Default, Debug)]
pub(crate) struct DeclStats {
    pub ticks: u64,
    pub events: u64,
    pub rows: u32,
    pub max_event_ticks: u32,
    pub max_row_events: u32,
    pub clip: [f64; 3],
    pub v_in: Option<(f64, f64)>,
    pub v_out: Option<(f64, f64)>,
    pub stalls: u64,
}

/// (G1's B1 / B2 on R4) One bar row's hits as the ride rows judge them over the set: each tagged with the
/// row as its sample (a declaration's `rate=… rows`) and keyed to it (the judge's cart: rows sharing the
/// cart's entity id and consecutive ticks never merge into one event; hits within the row still merge
/// by consecutive ticks, G1 18:34 (1)).
pub fn rekey_row(row: u32, hits: Vec<crate::contacts::Hit>) -> impl Iterator<Item = crate::contacts::Hit> {
    hits.into_iter().map(move |h| crate::contacts::Hit { cart: row, sample: Some(row as u64), ..h })
}

/// (G1 14:51 (4)) the ride rows' scope (the way-in's box: x 2..9, y -27..-11, z -8..3): the declared
/// contacts the ride bench judges. (G1's B11 on R4: its source) the same box the ride rows' fence scan
/// and the old stale count use, set by hand from the way-in's cells: the queue (9,-14,-4) and its
/// staircase, the drop stop (9,-15,0), the shaft (9,-16..-23,-1), the landing booster (9,-24,-1) and its
/// slime / torch / glass (9,-25..-27,-1), the roll (8..4,-25,-5) to the stop's entry (3,-25,-5), the
/// R1 levers (4..6,-12..-13,-1..-2), with a cell's margin; the stop (A) (x <= -1) lies outside it.
pub const RIDE_SCOPE: ([i32; 3], [i32; 3]) = ([2, -27, -8], [9, -11, 3]);

/// (R6, V) The ride rows' scope relative to the queue cell q: today's box (q = (9,-14,-4): y -27..-11)
/// moved with the queue's height (V's q = (9,-10,-4): y -23..-7), its south edge at z 2 (the way-in holds
/// nothing at z 3; V's napper-lift foot, moved up 7 with the stop, straddles z 3..5 there and would be cut).
pub fn ride_scope(q: P) -> ([i32; 3], [i32; 3]) {
    ([RIDE_SCOPE.0[0], q[1] - 13, RIDE_SCOPE.0[2]], [RIDE_SCOPE.1[0], q[1] + 3, 2])
}

/// (G1's W8 on R2) The shapes of a way-cell run's mode flags: each field the flag given (or its value
/// count), for [`way_cell_mode`].
#[derive(Default, Clone, Debug)]
pub struct ModeFlags {
    pub write: bool,
    pub post: bool,
    pub water_search: bool,
    pub water_scan: bool,
    pub scan_box: usize,
    pub path_probe: bool,
    pub visit: bool,
    pub leg: usize,
    pub hold: usize,
    pub bed: usize,
    pub ride_to: usize,
    pub leg_day: bool,
    pub release_sweep: bool,
    pub wait: usize,
    /// --release-phases, --restart-at, --restart-every or --freeze-every given (the ride rows)
    pub rows: bool,
    /// --release-pulse given (the boost-margin row; with --ride-to, --south, --r1-built)
    pub release_pulse: usize,
    /// --launch-speeds given (the rail end's least launch speed)
    pub launch_speeds: usize,
    /// (R6) --leg-train given (several nappers down the leg)
    pub leg_train: usize,
    /// (R6) --hold-sweep given (the hold's braking / relaunch sweep)
    pub hold_sweep: usize,
    pub south: bool,
    pub r1_built: bool,
}

/// (G1's W8 on R2) The run's mode from its primary flag: at most one mode a run, each with its
/// companion flags present and well-shaped; a mis-shaped mode is an error, never a fall-through into
/// another mode. No primary flag: the queue-cell checks.
pub fn way_cell_mode(f: &ModeFlags) -> Result<&'static str, String> {
    let mut modes: Vec<&'static str> = Vec::new();
    if f.write {
        modes.push("write");
    }
    if f.water_search {
        modes.push("water search");
    }
    if f.water_scan && !f.water_search {
        modes.push("water scan");
    }
    if f.path_probe {
        modes.push("path probe");
    }
    if f.visit {
        modes.push("visit");
    }
    if f.release_sweep {
        modes.push("sweep");
    }
    if f.release_pulse > 0 {
        modes.push("release pulse");
    }
    if f.launch_speeds > 0 {
        modes.push("launch speed");
    }
    // (the leg train takes --ride-to as the stop's entry: its health judged up to it)
    if (f.ride_to > 0 || f.rows) && !f.visit && !f.release_sweep && !f.water_search && f.release_pulse == 0 && (f.leg_train == 0 || f.rows) {
        modes.push(if f.rows { "ride rows" } else { "ride" });
    }
    if f.wait > 0 && !f.release_sweep {
        modes.push("wait");
    }
    if f.leg_train > 0 {
        modes.push("leg train");
    }
    if f.hold_sweep > 0 {
        modes.push("hold sweep");
    }
    if f.leg > 0 && !f.visit && f.leg_train == 0 && f.hold_sweep == 0 {
        modes.push("leg");
    }
    if modes.len() > 1 {
        return Err(format!("one mode a run: {} given together", modes.join(", ")));
    }
    let m = modes.first().copied().unwrap_or("queue cell");
    let need = |ok: bool, what: &str| -> Result<(), String> { if ok { Ok(()) } else { Err(format!("the {m} mode needs {what}")) } };
    match m {
        "write" => need(f.post, "--post")?,
        "water search" => {
            need(f.scan_box == 6, "--scan-box x0,y0,z0,x1,y1,z1")?;
            need(f.bed == 3, "--bed x,y,z")?;
            need(f.ride_to == 3, "--ride-to x,y,z")?;
        }
        "water scan" => need(f.scan_box == 6, "--scan-box x0,y0,z0,x1,y1,z1")?,
        "path probe" => need(f.bed == 3, "--bed x,y,z")?,
        "visit" => {
            need(f.leg == 6, "--leg x,y,z,vx,vy,vz")?;
            need(f.bed == 3, "--bed x,y,z")?;
            need(f.ride_to == 3, "--ride-to x,y,z")?;
            need(f.leg_day, "--leg-day")?;
        }
        "sweep" => {
            need(f.wait == 2, "--wait START,TICKS")?;
            need(f.post, "--post")?;
            need(f.bed == 3, "--bed x,y,z")?;
            need(f.ride_to == 3, "--ride-to x,y,z")?;
        }
        "ride rows" | "ride" => need(f.ride_to == 3, "--ride-to x,y,z")?,
        "release pulse" => {
            need(f.release_pulse == 2, "--release-pulse A,B")?;
            need(f.ride_to == 3, "--ride-to x,y,z")?;
            need(f.south && f.r1_built, "--south --r1-built (the built R1 gate)")?;
        }
        "launch speed" => need(f.launch_speeds == 3, "--launch-speeds A,B,STEP")?,
        "wait" => {
            need(f.wait == 2, "--wait START,TICKS")?;
            need(f.post, "--post")?;
            need(f.bed == 3, "--bed x,y,z")?;
        }
        "leg" => {
            need(f.leg == 6, "--leg x,y,z,vx,vy,vz")?;
            need(f.hold == 3, "--hold x,y,z")?;
            need(f.bed == 3, "--bed x,y,z")?;
        }
        "leg train" => {
            need(f.leg_train == 2, "--leg-train COUNT,SPACING")?;
            need(f.leg == 6, "--leg x,y,z,vx,vy,vz")?;
            need(f.bed == 3, "--bed x,y,z")?;
            need(f.leg_day, "--leg-day")?;
            need(f.south && f.r1_built, "--south --r1-built (the built R1 gate, re-armed after each release)")?;
        }
        "hold sweep" => {
            need(f.hold_sweep == 3, "--hold-sweep A,B,STEP")?;
            need(f.leg == 6, "--leg x,y,z,vx,vy,vz (the start and heading)")?;
        }
        _ => {}
    }
    Ok(m)
}

/// (ironspec 10:42 / 10:58, G1 14:51 (4)) the design's bounded `# contacts:` / `# stops:` declarations
/// whose cells lie in `scope`, read STRICTLY by main's `contacts::declared_contacts_bounded` (a malformed
/// one in scope is an error); the record's other declarations (another bench's scope) are left to their
/// benches; a declaration with cells both inside and outside the scope is an error (it can't be judged
/// by halves).
pub fn bounded_in_scope(text: &str, scope: ([i32; 3], [i32; 3])) -> Result<Vec<crate::contacts::Decl>, String> {
    bounded_scope_split(text, scope).map(|(d, _)| d)
}

/// (G1 18:34 (3) on R4) The same split, with the entries left OUTSIDE the scope named (their cells), so
/// a caller prints how many declarations the scope took and which it left (a scope silently excluding
/// the line under test would hide a failure). Every cell is read by main's shared cell grammar
/// (`pushscan::parse_cell_group`, G1 16:18), strictly: a malformed one is an error (G1's B10 on R4).
pub fn bounded_scope_split(text: &str, scope: ([i32; 3], [i32; 3])) -> Result<(Vec<crate::contacts::Decl>, Vec<String>), String> {
    let (lo, hi) = scope;
    let inside = |p: &[i32; 3]| (0..3).all(|k| lo[k] <= p[k] && p[k] <= hi[k]);
    let mut keep = String::new();
    let mut outside: Vec<String> = Vec::new();
    for key in ["contacts:", "stops:"] {
        for entry in crate::pushscan::header_entries(text, key) {
            let cells = entry.split_once('=').map_or(entry.as_str(), |(c, _)| c);
            let mut parsed: Vec<[i32; 3]> = Vec::new();
            for c in cells.split('+') {
                let Some((x, y, z)) = crate::pushscan::parse_cell_group(c) else {
                    return Err(format!("# {key} {entry}: cell {c:?} is not x,y,z"));
                };
                parsed.push([x, y, z]);
            }
            if !parsed.is_empty() && parsed.iter().all(inside) {
                keep.push_str(&format!("# {key} {entry}\n"));
            } else if parsed.iter().any(inside) {
                return Err(format!("# {key} {entry}: cells both inside and outside the scope {lo:?}..{hi:?}"));
            } else {
                outside.push(format!("{} ({key})", cells.trim()));
            }
        }
    }
    Ok((crate::contacts::declared_contacts_bounded(&keep)?, outside))
}

// (ironspec 10:02: ONE CONTACT RULE) the ride rows' contacts come from the shared
// `crate::contacts::cart_contacts` (no copy of the contact definition here).

/// (ironspec 22:09 (a)) The gap between the cart's box and the nearest fence collision box (the post
/// and every arm as the fence stands in the world, `behavior::collision_boxes`) over `fences` (every
/// fence of the way-in's box, at any distance, so each tick is a sample): the largest axis gap of
/// the closest fence box (0 = touching or overlapping), None when there is no fence.
fn fence_gap(w: &redsim::world::World, bb: &redsim::entity::vec3::Aabb, fences: &[BlockPos]) -> Option<f64> {
    let mut best: Option<f64> = None;
    for &p in fences {
        let s = w.get(p);
        let (x, y, z) = (p.x, p.y, p.z);
        for b in redsim::behavior::collision_boxes(&s) {
            let (x0, x1) = (x as f64 + b[0] as f64 / 16.0, x as f64 + b[1] as f64 / 16.0);
            let (y0, y1) = (y as f64 + b[2] as f64 / 16.0, y as f64 + b[3] as f64 / 16.0);
            let (z0, z1) = (z as f64 + b[4] as f64 / 16.0, z as f64 + b[5] as f64 / 16.0);
            let gap = [x0 - bb.max_x, bb.min_x - x1, y0 - bb.max_y, bb.min_y - y1, z0 - bb.max_z, bb.min_z - z1].into_iter().fold(f64::NEG_INFINITY, f64::max).max(0.0);
            best = Some(best.map_or(gap, |g: f64| g.min(gap)));
        }
    }
    best
}

/// Every fence in the box `lo..=hi` of `w` (the fence gap's samples).
fn fences_in(w: &redsim::world::World, lo: P, hi: P) -> Vec<BlockPos> {
    let mut v = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let p = BlockPos::new(x, y, z);
                if matches!(w.get(p), Block::Fence { .. }) {
                    v.push(p);
                }
            }
        }
    }
    v
}

/// The built R1 laid raw (no updates) never computes its line: each of its cells placed again with
/// updates, in [`R1_BUILT`]'s order (supports first, the inputs last), so the N line, the latch's
/// dust and R1's comparator settle from the levers as a built circuit does (the bench's settle ticks
/// then run the repeaters' delays). Nothing for the torch stand-in.
fn settle_r1(w: &mut redsim::world::World, o: &Opts) -> Result<(), String> {
    if !(o.south && o.r1_built) {
        return Ok(());
    }
    for (p, b) in r1_cells(o)? {
        w.set_block(p, Block::Air, redsim::world::flags::ALL);
        w.set_block(p, b, redsim::world::flags::ALL);
    }
    Ok(())
}

/// day-1's release stage R1 (17:57), laid in every bench world as emulated cells (not written into
/// the design: they are day-1's): the repeaters (8,-14,-4) and (7,-14,-4) facing west (input west)
/// into the queue rail, each on glass; none solid-render. With `o.r1_built` (ironspec 22:19 (1)):
/// record7's BUILT R1 instead (day-1's 19:19 + 21:41 cells, read off onechunk's record7
/// aefff19cb) with day-1's line-bench inputs: REST_l a floor lever ON at (6,-12,-1) on glass
/// (6,-13,-1), DAY a floor lever OFF at (4,-12,-2) on glass (4,-13,-2), NOT-ready onechunk's wall
/// lever ON at (8,-14,-5) on the file's glass (9,-14,-5); the release is that lever flipped OFF.
pub fn r1_cells(o: &Opts) -> Result<Vec<(BlockPos, Block)>, String> {
    if !o.south {
        return Ok(Vec::new());
    }
    if o.r1_built {
        // (R6) the layout's cells, written for its own queue cell, moved with the bench's queue
        let (cells, home): (&[([i32; 3], &str)], P) = match o.r1_layout {
            R1Layout::Record7 => (&R1_BUILT, [9, -14, -4]),
            R1Layout::V => (&R1_BUILT_V, [9, -10, -4]),
        };
        let d = [o.queue[0] - home[0], o.queue[1] - home[1], o.queue[2] - home[2]];
        return cells.iter().map(|(p, s)| Ok((BlockPos::new(p[0] + d[0], p[1] + d[1], p[2] + d[2]), Block::parse(s).map_err(|e| format!("R1 {s}: {e}"))?))).collect();
    }
    let q = o.queue;
    let g = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    let rep = Block::parse("minecraft:repeater[delay=1,facing=west,locked=false,powered=false]").map_err(|e| e.to_string())?;
    Ok(vec![
        (BlockPos::new(q[0] - 1, q[1] - 1, q[2]), g),
        (BlockPos::new(q[0] - 1, q[1], q[2]), rep),
        (BlockPos::new(q[0] - 2, q[1] - 1, q[2]), g),
        (BlockPos::new(q[0] - 2, q[1], q[2]), rep),
    ])
}

/// (R6, V: ironspec 21:57 (2)) The south staircase's cells relative to the queue cell q (today's q =
/// (9,-14,-4); V's (9,-10,-4)): the rail end q + (0,-1,+2) (the flat the launched cart leaves), NOT-ready's
/// wall lever q + (-1,0,-1) (on the arrival's support), the drop's shaft column x = q.x, z = q.z + 3.
pub fn rail_end_of(q: P) -> P {
    [q[0], q[1] - 1, q[2] + 2]
}

pub fn not_ready_lever_of(q: P) -> BlockPos {
    BlockPos::new(q[0] - 1, q[1], q[2] - 1)
}

pub fn shaft_xz_of(q: P) -> (i32, i32) {
    (q[0], q[2] + 3)
}

/// The queue's release as the bench emulates day-1's RELEASE level: for the south staircase, R1's
/// input driven by a redstone torch standing on glass at queue - (3,0,0) (the repeaters of
/// [`r1_cells`] carry it into the rail; nothing solid-render); for the west one a redstone block
/// (its old emulation).
fn release(w: &mut redsim::world::World, o: &Opts) -> Result<(), String> {
    let q = o.queue;
    if o.south && o.r1_built {
        // NOT-ready's fall: onechunk's wall lever (today (8,-14,-5): queue + (-1,0,-1)) flipped OFF
        // between ticks
        let p = not_ready_lever_of(q);
        w.set_block(p, Block::parse("minecraft:lever[face=wall,facing=west,powered=false]").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
        return Ok(());
    }
    if o.south {
        for (p, b) in r1_cells(o)? {
            if w.get(p) != b {
                w.set_block(p, b, redsim::world::flags::ALL);
            }
        }
        let support = BlockPos::new(q[0] - 3, q[1] - 1, q[2]);
        if w.get(support).is_air() {
            w.set_block(support, Block::parse("minecraft:glass").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
        }
        w.set_block(BlockPos::new(q[0] - 3, q[1], q[2]), Block::parse("minecraft:redstone_torch[lit=true]").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
    } else {
        w.set_block(BlockPos::new(q[0], q[1], q[2] + 1), Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
    }
    Ok(())
}

/// (superseded by R1) The queue's release as a torch on glass at the input cell itself.
#[allow(dead_code)]
fn release_torch_at_input(w: &mut redsim::world::World, o: &Opts) -> Result<(), String> {
    let q = o.queue;
    if o.south {
        let support = BlockPos::new(q[0] - 1, q[1] - 1, q[2]);
        if w.get(support).is_air() {
            w.set_block(support, Block::parse("minecraft:glass").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
        }
        w.set_block(BlockPos::new(q[0] - 1, q[1], q[2]), Block::parse("minecraft:redstone_torch[lit=true]").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
    } else {
        w.set_block(BlockPos::new(q[0], q[1], q[2] + 1), Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
    }
    Ok(())
}

/// The queue's cells as laid ([`rail_cells`]), the queue rail's floor glass instead of glowstone
/// with `queue_glass` (ironspec 14:47: the seat's own cell no candidate, its clearer moved to the
/// pocket (8,-14,-6) the extra cells lay).
pub fn queue_cells(o: &Opts) -> Vec<(P, &'static str, &'static str)> {
    let mut v = rail_cells(o.queue, o.south);
    if o.queue_glass {
        for c in v.iter_mut() {
            if c.1 == "minecraft:glowstone" {
                *c = (c.0, "minecraft:glass", "under the queue rail (glass: the seat's own cell no stroll candidate; its clearer the pocket)");
            }
        }
    }
    v
}

pub fn rail_cells(q: P, south: bool) -> Vec<(P, &'static str, &'static str)> {
    if south {
        // (the deep or the stop-9-down queue at the corner shaft (9,.,-5): the staircase descends
        // south, every slope ascending north, so each high side is inside the chunk)
        return vec![
            ([q[0], q[1], q[2] - 1], "minecraft:glass", "under the arrival detector (glass: a rail on glass is never a stroll draw)"),
            ([q[0], q[1] + 1, q[2] - 2], "minecraft:glass", "the arrival detector's high side (an ascending rail needs a rigid block there)"),
            ([q[0], q[1] + 1, q[2] - 1], "minecraft:detector_rail[powered=false,shape=ascending_north,waterlogged=false]", "the arrival detector (the shaft's foot)"),
            ([q[0], q[1] - 1, q[2]], "minecraft:glowstone", "under the queue rail (a solid-render floor: the rider's own cell a reached draw)"),
            (q, "minecraft:powered_rail[powered=false,shape=ascending_north,waterlogged=false]", "the queue rail (unpowered it holds the cart; day-1's gate powers it as a level: the cart rolls south)"),
            ([q[0], q[1] - 2, q[2] + 1], "minecraft:glass", "under the departure detector (glass: never a stroll draw)"),
            ([q[0], q[1] - 1, q[2] + 1], "minecraft:detector_rail[powered=false,shape=ascending_north,waterlogged=false]", "the departure detector (resets HELD and the RS)"),
        ];
    }
    vec![
        ([q[0] + 1, q[1], q[2]], "minecraft:glass", "under the arrival detector (glass: a rail on glass is never a stroll draw)"),
        ([q[0] + 1, q[1] + 1, q[2]], "minecraft:detector_rail[powered=false,shape=ascending_east,waterlogged=false]", "the arrival detector (the shaft's foot: sets HELD, starts the hold timer)"),
        ([q[0], q[1] - 1, q[2]], "minecraft:glowstone", "under the queue rail (a solid-render floor: the rider's own cell a reached draw)"),
        (q, "minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false]", "the queue rail (unpowered it holds the cart; day-1's gate powers it as a level: the cart rolls west down the run)"),
        ([q[0] - 1, q[1] - 2, q[2]], "minecraft:glass", "under the departure detector (glass: never a stroll draw)"),
        ([q[0] - 1, q[1] - 1, q[2]], "minecraft:detector_rail[powered=false,shape=ascending_east,waterlogged=false]", "the departure detector (resets HELD and the RS)"),
    ]
}

/// (ironspec 15:58: (C) decided) The rail descent replacing the second fall: from the queue's
/// departure (9,-15,-3) the cart never leaves the rails, so a calm rider's HOME path always starts
/// on a rail cell (58f770cc's fact 1) and none starts in free fall (fact 2). Ten slopes, every rail
/// on glass, every slope's high side the previous rail's support, 2-high head room over each:
/// south (9,-16,-2) .. (9,-19,1); the curve (9,-19,2); west (8,-20,2), (7,-21,2); the curve
/// (6,-21,2); north (6,-22,1), (6,-23,0); the curve (6,-23,-1); east (7,-23,-1), (8,-23,-1); the curve
/// (9,-23,-1); north (9,-24,-2), (9,-25,-3); then the drop-and-roll's (9,-25,-4), the curve
/// (9,-25,-5) and west (8..4,-25,-5) into the entry. `brakes` are cells whose rail becomes an
/// unpowered powered rail (each halves the cart's speed a tick while it is on it).
pub fn c_run(brakes: &[P]) -> Vec<(P, String)> {
    let glass = "minecraft:glass".to_string();
    let rail = |p: P, s: &str| -> String {
        if brakes.contains(&p) && !s.starts_with("north_") && !s.starts_with("south_") {
            format!("minecraft:powered_rail[powered=false,shape={s},waterlogged=false]")
        } else {
            format!("minecraft:rail[shape={s},waterlogged=false]")
        }
    };
    let mut v: Vec<(P, String)> = Vec::new();
    let mut put = |p: P, s: &str| {
        v.push(([p[0], p[1] - 1, p[2]], glass.clone()));
        v.push((p, rail(p, s)));
    };
    put([9, -16, -2], "ascending_north");
    put([9, -17, -1], "ascending_north");
    put([9, -18, 0], "ascending_north");
    put([9, -19, 1], "ascending_north");
    put([9, -19, 2], "north_west");
    put([8, -20, 2], "ascending_east");
    put([7, -21, 2], "ascending_east");
    put([6, -21, 2], "north_east");
    put([6, -22, 1], "ascending_south");
    put([6, -23, 0], "ascending_south");
    put([6, -23, -1], "south_east");
    put([7, -23, -1], "east_west");
    put([8, -23, -1], "east_west");
    put([9, -23, -1], "north_west");
    put([9, -24, -2], "ascending_south");
    put([9, -25, -3], "ascending_south");
    put([9, -25, -4], "north_south");
    put([9, -25, -5], "south_west");
    for x in (4..=8).rev() {
        put([x, -25, -5], "east_west");
    }
    v
}

/// (ironspec 12:23: the queue is the stop 9 down) The run from the queue's departure (9,-15,-3)
/// down to the moved stop's entry (3,-25,-5), every rail on glass. A staircase of 10 unpowered
/// slopes reached the entry at 1.04 b/t (A6's band tops at 0.62), so the run drops instead: the
/// flat (9,-15,-2) carries the cart off its end south into (9,-15,-1), the wall (9,-15,0) stops it,
/// it falls straight down (9,-16..-23,-1) onto the landing slope (9,-24,-1) (ascending south, its
/// high side (9,-24,0) glass) and rolls north from rest: the flat (9,-24,-2), the slope (9,-25,-3),
/// the flat (9,-25,-4), the curve (9,-25,-5), west (8..4,-25,-5) into the entry.
pub fn s9_run() -> Vec<(P, String)> {
    let rail = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
    let glass = "minecraft:glass".to_string();
    let mut v: Vec<(P, String)> = vec![([9, -15, 0], glass.clone()), ([9, -24, 0], glass.clone())];
    let mut put = |p: P, b: String| {
        v.push(([p[0], p[1] - 1, p[2]], glass.clone()));
        v.push((p, b));
    };
    put([9, -15, -2], rail("north_south"));
    put([9, -24, -1], rail("ascending_south"));
    put([9, -24, -2], rail("north_south"));
    put([9, -25, -3], rail("ascending_south"));
    put([9, -25, -4], rail("north_south"));
    put([9, -25, -5], rail("south_west"));
    for x in (4..=8).rev() {
        put([x, -25, -5], rail("east_west"));
    }
    v
}

/// (ironspec 11:47/11:52: the deep queue's way down) The leg from the split row's end to the
/// corner shaft (9,.,-5), every rail on glass: the row's last rail (9,25,9) replaced by a
/// north-west curve, (9,25,8) north-south, three slopes down north (9,24,7), (9,23,6), (9,22,5)
/// (each high side the previous rail's glass), flat north to (9,22,-4); off its end the cart's
/// northward speed is caught by the stop wall (9,22,-6), (9,21,-6), and it falls straight down
/// (9,.,-5) to the arrival detector. Returns the cells and the replaced cell.
pub fn deep_leg() -> (Vec<(P, String)>, Vec<P>) {
    let rail = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
    let glass = "minecraft:glass".to_string();
    let mut v: Vec<(P, String)> = vec![([9, 25, 9], rail("north_west"))];
    let mut put = |p: P, b: String| {
        v.push(([p[0], p[1] - 1, p[2]], glass.clone()));
        v.push((p, b));
    };
    put([9, 25, 8], rail("north_south"));
    put([9, 24, 7], rail("ascending_south"));
    put([9, 23, 6], rail("ascending_south"));
    put([9, 22, 5], rail("ascending_south"));
    for z in (-4..=4).rev() {
        put([9, 22, z], rail("north_south"));
    }
    v.push(([9, 22, -6], glass.clone()));
    v.push(([9, 21, -6], glass));
    (v, vec![[9, 25, 9]])
}

/// (ironspec 11:52: the deep queue) The climb from the deep queue's departure (9,-29,-3) back up to
/// the stop's entry (3,-16,-5), every rail on glass inside the module's chunk: the connector
/// (9,-29,-2) north-west, (8,-29,-2); a spiral of 13 slopes round the column (6,.,-3) (a 3 x 3 ring:
/// the slopes on its sides' middles, flat curves at its corners, one level a side), each slope's
/// high side the next corner's glass support; the top (7,-16,-4) north-south, the exit (7,-16,-5)
/// south-west and (6..4,-16,-5) east-west into the entry. Every `powered_every`-th slope is a
/// powered rail, powered by a redstone torch standing on glass in the ring's centre column beside it
/// (nothing solid-render inside the stop's box).
pub fn deep_climb(powered_every: usize) -> Vec<(P, String)> {
    let rail = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
    let glass = "minecraft:glass".to_string();
    let mut v: Vec<(P, String)> = Vec::new();
    let mut put = |v: &mut Vec<(P, String)>, p: P, b: String| {
        v.push(([p[0], p[1] - 1, p[2]], glass.clone()));
        v.push((p, b));
    };
    put(&mut v, [9, -29, -2], rail("north_west"));
    put(&mut v, [8, -29, -2], rail("east_west"));
    put(&mut v, [7, -29, -2], rail("north_east"));
    // the sides in turn: north along x 7, west along z -4, south along x 5, east along z -2
    let sides: [([i32; 2], [i32; 2], &str, &str); 4] = [
        ([7, -2], [0, -1], "ascending_north", "south_west"),
        ([7, -4], [-1, 0], "ascending_west", "south_east"),
        ([5, -4], [0, 1], "ascending_south", "north_east"),
        ([5, -2], [1, 0], "ascending_east", "north_west"),
    ];
    let mut torches: Vec<i32> = Vec::new();
    for k in 0..13usize {
        let (start, d, slope, corner) = sides[k % 4];
        let level = -29 + k as i32;
        let mid = [start[0] + d[0], level, start[1] + d[1]];
        let powered = powered_every > 0 && k % powered_every == 0;
        let s = if powered { format!("minecraft:powered_rail[powered=true,shape={slope},waterlogged=false]") } else { rail(slope) };
        put(&mut v, mid, s);
        if powered {
            torches.push(level);
        }
        let end = [start[0] + 2 * d[0], level + 1, start[1] + 2 * d[1]];
        put(&mut v, end, if k == 12 { rail("north_south") } else { rail(corner) });
    }
    for l in torches {
        v.push(([6, l - 1, -3], glass.clone()));
        v.push(([6, l, -3], "minecraft:redstone_torch[lit=true]".to_string()));
    }
    put(&mut v, [7, -16, -5], rail("south_west"));
    for x in [6, 5, 4] {
        put(&mut v, [x, -16, -5], rail("east_west"));
    }
    v
}

/// The ride from the queue: the queue rail powered from its input cell (a redstone block at queue +
/// (0,0,1), as day-1's RELEASE level would), the cart with its rider followed for `ticks`: the cells
/// its cart passes, the tick it leaves the queue cell, crosses the departure detector and reaches
/// `entry` (the stop's first rail), the rider still seated.
pub fn ride_lines(spec: &FarmSpec, o: &Opts, entry: P, ticks: u32) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o)?);
    // (G1 (6)) the override rule's before -> after lines kept, not discarded
    let override_lines = place_overrides(&mut run.world, o.extra.len(), &cells)?;
    settle_r1(&mut run.world, o)?;
    // (G1's W7 on R2) the chunk's water given its tick; frozen water refuses this information mode
    let (fluid_line, fluid_ok) = fluids_line(&mut run.world, o.chunk, "ride");
    if !fluid_ok {
        return Err(fluid_line);
    }
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
    // settle on the braked rail, then release
    for _ in 0..40 {
        run.tick();
    }
    let held_at = run.world.entities.list.iter().find(|e| e.id == cart).map(|e| e.pos);
    let input = if o.south { BlockPos::new(q[0] - 1, q[1], q[2]) } else { BlockPos::new(q[0], q[1], q[2] + 1) };
    let _ = input;
    release(&mut run.world, o)?;
    let (mut left, mut dep, mut at_entry) = (None, None, None);
    let mut path: Vec<P> = Vec::new();
    let mut seated = true;
    // (58f770cc 11:54: A6's tested entry band 0.10..0.30 b/t heading west, measured on the entry
    // booster) the cart's speed and heading the tick before the entry and on it, and the least
    // speed on the climb (a stall shows as 0)
    let (mut before, mut on_entry, mut least) = (None, None, f64::INFINITY);
    let mut prev_v = None;
    for t in 0..ticks {
        run.tick();
        let Some(e) = run.world.entities.list.iter().find(|e| e.id == cart && !e.removed) else { break };
        let c = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
        if path.last() != Some(&c) {
            path.push(c);
        }
        if left.is_none() && c != q {
            left = Some(t);
        }
        if dep.is_none() && c == (if o.south { [q[0], q[1] - 1, q[2] + 1] } else { [q[0] - 1, q[1] - 1, q[2]] }) {
            dep = Some(t);
        }
        let v = e.delta_movement;
        if left.is_some() && at_entry.is_none() && t > 2 {
            least = least.min((v.x * v.x + v.z * v.z).sqrt());
        }
        if at_entry.is_none() && c == entry {
            at_entry = Some(t);
            before = prev_v;
            on_entry = Some(v);
        }
        prev_v = Some(v);
        if at_entry.is_none() || at_entry == Some(t) {
            seated &= run.world.entities.list.iter().any(|v| !v.removed && v.vehicle == Some(cart));
        }
    }
    let speed = |v: Option<Vec3>| match v {
        Some(v) => format!("{:.4} b/t (vx {:.4}, vz {:.4}{})", (v.x * v.x + v.z * v.z).sqrt(), v.x, v.z, if v.x < 0.0 && v.x.abs() > v.z.abs() { ", west" } else { ", NOT west" }),
        None => "-".to_string(),
    };
    Ok(vec![
        format!("(ride) held at {:?} after 40 gt on the braked rail; released (the rail powered) at t 0", held_at.map(|p| (p.x, p.y, p.z))),
        format!("(ride) left the queue cell at t {left:?}, on the departure detector at t {dep:?}, at the stop's entry {entry:?} at t {at_entry:?}; the rider {} seated up to the entry", if seated { "stayed" } else { "did NOT stay" }),
        format!("(ride) the speed the tick before the entry {}, on the entry {}; the least speed on the way {least:.4} b/t", speed(before), speed(on_entry)),
        format!("(ride) the cells: {:?}", path.iter().take(60).collect::<Vec<_>>()),
        fluid_line,
    ]
    .into_iter()
    .chain(override_lines)
    .collect())
}

/// (ironspec 17:38: THE BOOST-MARGIN ROW) The built R1 gate's RELEASE level held for only `k` gt, k in
/// `pulses`: the cart settled 40 gt on the braked queue rail, the release (NOT-ready's lever OFF), and
/// after k gt NOT-ready back ON (the rail's boost cut once R1's repeaters carry it); then followed up
/// to `ticks` gt: where the cart left the queue cell, whether it reached the shaft (9,·,-1) below the
/// rail end and the stop's entry, else where it ended and at what speed (a stall). The stated minimum:
/// the least k from which every longer pulse reaches the entry. Needs --south --r1-built.
pub fn release_pulse_lines(spec: &FarmSpec, o: &Opts, entry: P, ticks: u32, pulses: (u32, u32)) -> Result<(Vec<String>, bool), String> {
    if !(o.south && o.r1_built) {
        return Err("--release-pulse cuts the built R1 gate's release (NOT-ready's lever): it needs --south --r1-built".to_string());
    }
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 60, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o)?);
    let override_lines = place_overrides(&mut run.world, o.extra.len(), &cells)?;
    settle_r1(&mut run.world, o)?;
    let (fluid_line, fluid_ok) = fluids_line(&mut run.world, o.chunk, "release pulse");
    if !fluid_ok {
        return Err(fluid_line);
    }
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
    for _ in 0..40 {
        run.tick();
    }
    let lever = not_ready_lever_of(q);
    let (re, (sx, sz)) = (rail_end_of(q), shaft_xz_of(q));
    let ready_on = Block::parse("minecraft:lever[face=wall,facing=west,powered=true]").map_err(|e| e.to_string())?;
    // (ironspec 18:39 (4)) the departure detector (9,-15,-3) and the arrival detector (9,-13,-5) with the
    // cart at rest on the queue: neither may be pressed (a departure detector pressed at rest resets
    // RS_ready at once); the queue rail's power
    let powered = |w: &redsim::world::World, p: BlockPos| w.get(p).to_string().contains("powered=true");
    let (dep_cell, arr_cell, rail_cell) = (BlockPos::new(q[0], q[1] - 1, q[2] + 1), BlockPos::new(q[0], q[1] + 1, q[2] - 1), BlockPos::new(q[0], q[1], q[2]));
    let at_rest = run.world.entities.list.iter().find(|e| e.id == cart && !e.removed).map(|e| (e.pos, e.delta_movement));
    let mut out = vec![
        fluid_line,
        format!("(release pulse) the cart held 40 gt on the braked queue rail; per pulse k: the release (NOT-ready's lever {lever} OFF) at t 0 and the lever back ON at t k"),
        format!(
            "(release pulse) at rest before the release: the cart at {:?}; the departure detector {dep_cell} {} ({}), the arrival detector {arr_cell} {} ({}), the queue rail {rail_cell} {}: {}",
            at_rest.map(|(p, v)| format!("({:.4}, {:.4}, {:.4}) moving {:.4}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt())),
            run.world.get(dep_cell),
            if powered(&run.world, dep_cell) { "PRESSED" } else { "not pressed" },
            run.world.get(arr_cell),
            if powered(&run.world, arr_cell) { "PRESSED" } else { "not pressed" },
            if powered(&run.world, rail_cell) { "powered" } else { "unpowered (braking)" },
            if powered(&run.world, dep_cell) || powered(&run.world, arr_cell) { "a detector pressed at rest: FAIL" } else { "neither pressed: PASSES" }
        ),
    ];
    let rest_ok = !powered(&run.world, dep_cell) && !powered(&run.world, arr_cell);
    let mut reached: Vec<(u32, bool)> = Vec::new();
    let mut holds: Vec<(u32, Option<u32>, u32, Option<f64>)> = Vec::new();
    for k in pulses.0..=pulses.1 {
        let mut r = run.clone();
        release(&mut r.world, o)?;
        let (mut left, mut shaft, mut at_entry) = (None::<u32>, None::<u32>, None::<u32>);
        let mut last: Option<([i32; 3], f64)> = None;
        // the first departure-detector press, the queue rail's powered ticks, the speed entering the
        // rail end (9,-15,-2) (the tick the cart's cell first becomes it: its horizontal speed)
        let (mut dep_press, mut rail_on, mut rail_end_v) = (None::<u32>, 0u32, None::<f64>);
        for t in 0..ticks {
            if t == k {
                r.world.set_block(lever, ready_on.clone(), redsim::world::flags::ALL);
            }
            r.tick();
            if dep_press.is_none() && powered(&r.world, dep_cell) {
                dep_press = Some(t);
            }
            if powered(&r.world, rail_cell) {
                rail_on += 1;
            }
            let Some(e) = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed) else { break };
            let c = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
            last = Some((c, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt()));
            if rail_end_v.is_none() && c == re {
                rail_end_v = Some((e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt());
            }
            if left.is_none() && c != q {
                left = Some(t);
            }
            if shaft.is_none() && c[0] == sx && c[2] == sz && c[1] < re[1] {
                shaft = Some(t);
            }
            if c == entry {
                at_entry = Some(t);
                break;
            }
        }
        reached.push((k, at_entry.is_some()));
        holds.push((k, dep_press, rail_on, rail_end_v));
        out.push(format!(
            "(release pulse) k {k} gt: left the queue cell at t {left:?}, the departure detector {dep_cell} first pressed at t {dep_press:?}, the queue rail powered {rail_on} tick(s), entering the rail end {re:?} at {}, in the shaft at t {shaft:?}, at the stop's entry {entry:?} at t {at_entry:?}{}",
            rail_end_v.map_or("- (never entered)".to_string(), |v| format!("{v:.4} b/t")),
            if at_entry.is_some() { String::new() } else { last.map_or("; the cart gone".to_string(), |(c, v)| format!("; it ENDS at {c:?} at {v:.4} b/t after {ticks} gt")) }
        ));
    }
    // the stated minimum: the least k from which every longer pulse of the sweep reaches the entry
    let min_k = (0..reached.len()).find(|&i| reached[i..].iter().all(|(_, ok)| *ok)).map(|i| reached[i].0);
    // (ironspec 18:39 (4)) the built gate's hold as wired: RS_ready resets at the cart's SENSED departure,
    // so the hold ends 2 gt after the first departure-detector press (the reset's two repeater ticks):
    // the hold = that press + 2, read from the longest pulse (an unbroken release)
    let (_, first_press, _, _) = holds.last().copied().unwrap_or((0, None, 0, None));
    let built_hold = first_press.map(|t| t + 2);
    out.push(format!(
        "(release pulse) the built gate's hold (RS_ready reset at the sensed departure, ironspec 18:39 (4)): the departure detector first pressed at t {first_press:?} after the release, so the hold ends at t {} (press + 2): {} the stated minimum{}",
        built_hold.map_or("-".to_string(), |h| h.to_string()),
        match (built_hold, min_k) {
            (Some(h), Some(m)) if h >= m => format!("{} gt over", h - m),
            (Some(h), Some(m)) => format!("{} gt SHORT of", m - h),
            _ => "NOT JUDGED against".to_string(),
        },
        if rest_ok { "" } else { " (a detector pressed at rest: the reset would fire before the release)" }
    ));
    let pass = min_k.is_some() && rest_ok && matches!((built_hold, min_k), (Some(h), Some(m)) if h >= m);
    out.push(format!(
        "(release pulse) the stated minimum over k {}..{}: {}",
        pulses.0,
        pulses.1,
        match min_k {
            Some(k) => format!("{k} gt (every pulse of {k} gt or more reaches the entry)"),
            None => "NONE: the longest pulse swept doesn't reach the entry".to_string(),
        }
    ));
    Ok((override_lines.into_iter().chain(out).collect(), pass))
}

/// (G1 17:28: the design margin for the fault notes; G1 18:40: the whole range at every phase) Whether a
/// cart leaving the rail end (9,-15,-2) clears into the shaft: per start phase (the cart's z inside the
/// rail end's cell) and per speed v, a cart (with a rider) set there moving south at v, followed 60 gt:
/// in the shaft (9,·,-1) below y -16, or held (resting on the rail end's support). Per phase: the held
/// bands and the least speed from which every faster speed clears; over the phases: the least speed
/// from which every faster speed clears at EVERY phase, against the designed launch's 0.4000.
pub fn launch_speed_lines(spec: &FarmSpec, o: &Opts, speeds: &[f64], phases: u32) -> Result<(Vec<String>, bool), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 120, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    let override_lines = place_overrides(&mut run.world, o.extra.len(), &cells)?;
    let rend = rail_end_of(o.queue);
    let (sx, sz) = shaft_xz_of(o.queue);
    let end = BlockPos::new(rend[0], rend[1], rend[2]);
    if !run.world.get(end).to_string().contains("rail") {
        return Err(format!("--launch-speeds: no rail at the rail end {end}"));
    }
    let mut out = vec![format!(
        "(launch speed) a cart with a rider set on the rail end {end} at {} start phase(s) along it (z = {} + (k + 0.5) / {phases}), moving south (+z) at each of {} speed(s) {:.4}..{:.4}, followed 60 gt (G1 18:40: the whole range at every phase)",
        phases,
        rend[2],
        speeds.len(),
        speeds.first().copied().unwrap_or(0.0),
        speeds.last().copied().unwrap_or(0.0)
    )];
    let fmt_bands = |b: &[(f64, f64)]| if b.is_empty() { "none".to_string() } else { b.iter().map(|(a, z)| if a == z { format!("{a:.4}") } else { format!("{a:.4}..{z:.4}") }).collect::<Vec<_>>().join(", ") };
    // per phase: the held bands (maximal runs of held speeds) and the least speed from which every faster
    // speed swept clears
    let mut worst_all_clear: Option<f64> = Some(0.0);
    let mut held_union: Vec<(f64, f64, f64)> = Vec::new();
    // (R6; G1 23:06) the landings over every cleared (phase, speed): cell and what it lands in / on,
    // counted; the ones off a rail and off slime (or with the rider hurt) listed
    let mut footprint: std::collections::BTreeMap<([i32; 3], String), u32> = std::collections::BTreeMap::new();
    let (mut off, mut landed, mut health_least): (Vec<String>, u32, f32) = (Vec::new(), 0, f32::INFINITY);
    for k in 0..phases.max(1) {
        let z0 = rend[2] as f64 + (k as f64 + 0.5) / phases.max(1) as f64;
        let mut results: Vec<(f64, bool)> = Vec::new();
        for &v in speeds {
            let mut r = run.clone();
            let cart = redsim::entity::spawn(&mut r.world, "minecraft:minecart", [rend[0] as f64 + 0.5, rend[1] as f64, z0], Some(&format!("{{Motion:[0.0d,0.0d,{v:.6}d],Passengers:[{{id:\"minecraft:villager\",NoAI:1b}}]}}")))?;
            let mut fell = None;
            let mut last = None;
            // (R6; G1 23:06: off the rails a cart keeps its horizontal speed, so a slow launch may drift
            // past the landing) the landing: the first tick after the fall began that the cart stops
            // falling (vy >= -0.0001) or stands in a rail cell; its cell, that cell's block and the one
            // under it; the rider's least health over the run
            let mut landing: Option<(u32, [i32; 3], String, String, bool)> = None;
            let mut hmin = f32::INFINITY;
            let mut h0: Option<f32> = None;
            for t in 0..60u32 {
                r.tick();
                if let Some(rd) = r.world.entities.list.iter().find(|x| !x.removed && x.vehicle == Some(cart)) {
                    if let EntityKind::Mob(m) = &rd.kind {
                        h0.get_or_insert(m.health);
                        hmin = hmin.min(m.health);
                    }
                }
                let Some(e) = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed) else { break };
                last = Some((e.pos, e.delta_movement));
                if fell.is_none() && e.pos.y < (rend[1] - 1) as f64 && e.pos.x.floor() as i32 == sx && e.pos.z.floor() as i32 == sz {
                    fell = Some(t);
                }
                // the landing is tracked for any fall below the rail end (in the shaft or beside it)
                if e.pos.y < (rend[1] - 1) as f64 && landing.is_none() {
                    let c = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
                    let here = r.world.get(BlockPos::new(c[0], c[1], c[2]));
                    let on_rail = redsim::rails::is_rail(&here);
                    if on_rail || e.delta_movement.y >= -0.0001 {
                        let under = r.world.get(BlockPos::new(c[0], c[1] - 1, c[2]));
                        landing = Some((t, c, here.to_string(), under.to_string(), on_rail));
                    }
                }
            }
            if let Some((t, c, here, under, on_rail)) = &landing {
                let ok = *on_rail || under.contains("slime_block") || here.contains("slime_block");
                let hurt = h0.is_some_and(|h| hmin < h);
                *footprint.entry((*c, if *on_rail { here.split('[').next().unwrap_or("").to_string() } else { format!("on {}", under.split('[').next().unwrap_or("")) })).or_insert(0) += 1;
                if !ok || hurt {
                    off.push(format!("v {v:.4} z0 {z0:.4}: lands at t {t} in {c:?} ({here} over {under}){}", if hurt { format!(", the rider hurt to {hmin:.1}") } else { String::new() }));
                }
                landed += 1;
            }
            health_least = health_least.min(hmin);
            results.push((v, fell.is_some()));
            if phases <= 1 {
                out.push(format!(
                    "(launch speed) v {v:.4} b/t: {}",
                    match (fell, last) {
                        (Some(t), _) => format!("CLEARS into the shaft (below y -16 at t {t})"),
                        (None, Some((p, dm))) => format!("HELD at ({:.4}, {:.4}, {:.4}), stored motion ({:.4}, {:.4}, {:.4}) after 60 gt", p.x, p.y, p.z, dm.x, dm.y, dm.z),
                        (None, None) => "the cart gone".to_string(),
                    }
                ));
            }
        }
        let mut bands: Vec<(f64, f64)> = Vec::new();
        let mut prev_held = false;
        for &(v, clears) in &results {
            if !clears {
                match bands.last_mut() {
                    Some((_, z)) if prev_held => *z = v,
                    _ => bands.push((v, v)),
                }
            }
            prev_held = !clears;
        }
        // the least speed from which every faster speed swept clears (None: the fastest is held)
        let all_clear = match results.iter().rposition(|r| !r.1) {
            None => results.first().map(|r| r.0),
            Some(i) => results.get(i + 1).map(|r| r.0),
        };
        worst_all_clear = match (worst_all_clear, all_clear) {
            (Some(w), Some(a)) => Some(w.max(a)),
            _ => None,
        };
        held_union.extend(bands.iter().map(|&(a, b)| (z0, a, b)));
        out.push(format!(
            "(launch speed) start z {z0:.4}: every speed from {} up clears; held bands {}",
            all_clear.map_or("NONE (the fastest swept is held)".to_string(), |a| format!("{a:.4}")),
            fmt_bands(&bands)
        ));
    }
    let pass_clear = worst_all_clear.is_some();
    // (R6; G1 23:06) every landing in a rail cell or on slime, and the rider unhurt, or the sweep fails
    let landing_ok = off.is_empty() && landed > 0;
    let pass = pass_clear && landing_ok;
    let union_lo = held_union.iter().map(|b| b.1).fold(f64::INFINITY, f64::min);
    out.push(format!(
        "(launch speed) over the {} phase(s): every speed from {} up clears at EVERY phase (the designed launch 0.4000{}); held speeds anywhere in {:.4}..{:.4} over the phases ({} held band(s) in all)",
        phases.max(1),
        worst_all_clear.map_or("NONE".to_string(), |w| format!("{w:.4}")),
        worst_all_clear.filter(|w| *w > 0.0).map_or(String::new(), |w| format!(", margin x{:.2}", 0.4 / w)),
        if union_lo.is_finite() { union_lo } else { 0.0 },
        held_union.iter().map(|b| b.2).fold(0.0, f64::max),
        held_union.len()
    ));
    out.push(format!(
        "(launch speed) the landing footprint over the {landed} falling (phase, speed) run(s) (G1 23:06: every fall below the rail end, in the shaft or beside it; the landing the first tick after it that the cart stops falling or stands in a rail cell): {}; the rider's least health {}; BAR every landing in a rail cell or on slime, the rider unhurt: {}{}",
        if footprint.is_empty() { "none".to_string() } else { footprint.iter().map(|((c, what), n)| format!("{c:?} {what} x{n}")).collect::<Vec<_>>().join(", ") },
        if health_least.is_finite() { format!("{health_least:.1}") } else { "-".to_string() },
        if landing_ok { "PASSES" } else if landed == 0 { "NOT JUDGED (no landing): FAILS" } else { "FAILS" },
        if off.is_empty() { String::new() } else { format!(" ({} run(s): {})", off.len(), off.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    Ok((override_lines.into_iter().chain(out).collect(), pass))
}

/// (ironspec 12:44, the drop-and-roll's conditions (1)-(2)) The ride from the queue in rows, the
/// world built once and cloned per row: every release phase (the cart held `k` more gt before the
/// release, k < `phases`) and every restart (a save/stop/start at tick T after the release in each
/// reload order). Per row: the entry tick after the release, the speed and heading on the entry, the
/// rider's health lost, and at the landing cell (`landing`) whether the cart stayed on rails and never
/// moved back south past it. Then the min..max over the rows.
pub fn ride_rows(spec: &FarmSpec, o: &Opts, entry: P, ticks: u32, phases: u32, restart_ticks: &[i64], landing: Option<P>) -> Result<(Vec<String>, bool), String> {
    ride_rows_with(spec, o, entry, ticks, phases, restart_ticks, &[], landing)
}

/// [`ride_rows`] with entity freezes too (ironspec 18:46 (a)): each (start, length) a row with the
/// part frozen at level 32 ([`crate::liveness::freeze32`]: redstone running, entities frozen) from
/// the tick `start` after the release for `length` gt.
#[allow(clippy::too_many_arguments)]
pub fn ride_rows_with(spec: &FarmSpec, o: &Opts, entry: P, ticks: u32, phases: u32, restart_ticks: &[i64], freezes: &[(i64, i64)], landing: Option<P>) -> Result<(Vec<String>, bool), String> {
    ride_rows_core(spec, o, entry, ticks, phases, restart_ticks, freezes, landing, None)
}

/// [`ride_rows_with`], also recording the cart's position at every tick of the plain release-phase-0
/// row into `positions` (the descent probe's positions).
#[allow(clippy::too_many_arguments)]
pub fn ride_rows_core(spec: &FarmSpec, o: &Opts, entry: P, ticks: u32, phases: u32, restart_ticks: &[i64], freezes: &[(i64, i64)], landing: Option<P>, mut positions: Option<&mut Vec<[f64; 3]>>) -> Result<(Vec<String>, bool), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + phases + 60, WalkMode::Random) };
    let (mut base, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    // (ironspec 14:57) the spawner's state stated, never a light-read default: the ride rows' world has
    // no natural spawner (no terrain site), asserted
    if base.world.mob.natural.is_some() {
        return Err("the ride rows' world has a natural spawner (mob.natural set): the bench states it OFF (ironspec 14:57)".to_string());
    }
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o)?);
    let override_lines = place_overrides(&mut base.world, o.extra.len(), &cells)?;
    settle_r1(&mut base.world, o)?;
    // (ironspec 00:27 (3): water in every bench) every fluid cell of the module's chunk given its
    // fluid tick, as a placement with updates schedules it (a raw placement schedules none); the count
    // printed with the rows (0 where the design has water fails)
    let fluid_n = schedule_fluids(&mut base.world, o.chunk);
    let spawn_at = o.seat_at.unwrap_or([q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5]);
    let cart0 = redsim::entity::spawn(&mut base.world, "minecraft:minecart", spawn_at, Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
    for _ in 0..40 {
        base.tick();
    }
    // (R6; ironspec 21:54 / 21:57: the seat's 16-ball distance) the queue cart at rest after the hold,
    // its distance to the bed head's centre (9fc3155f's convention: the game's closerToCenterThan, the
    // seated villager's position = its cart's), against the design bar > 17
    let ball_line = o.ball_head.map(|h| {
        let at = base.world.entities.list.iter().find(|e| e.id == cart0 && !e.removed).map(|e| e.pos);
        match at {
            Some(p) => {
                let d = ((p.x - (h[0] as f64 + 0.5)).powi(2) + (p.y - (h[1] as f64 + 0.5)).powi(2) + (p.z - (h[2] as f64 + 0.5)).powi(2)).sqrt();
                (format!("(ride) the queue cart at rest after the hold at ({:.4}, {:.4}, {:.4}): {d:.3} from the bed head {h:?}'s centre (9fc3155f's convention); the bar > 17: {}", p.x, p.y, p.z, if d > 17.0 { "PASSES" } else { "FAILS" }), d > 17.0)
            }
            None => ("(ride) the queue cart gone after the hold: the 16-ball distance NOT JUDGED: FAILS".to_string(), false),
        }
    });
    let input = if o.south { BlockPos::new(q[0] - 1, q[1], q[2]) } else { BlockPos::new(q[0], q[1], q[2] + 1) };
    let mut rows: Vec<(String, Option<u32>, Option<Vec3>, f32, bool, bool)> = Vec::new();
    // (G1 09:31 E2) each row's band-bench t0 (the entry cell's centre passed, post-tick), in the rows' order
    let mut centres: Vec<Option<(u32, f64)>> = Vec::new();
    let mut rail_end_speeds: Vec<Option<f64>> = Vec::new();
    let mut specs: Vec<(String, u32, Option<(i64, crate::liveness::Restart)>, Option<(i64, i64)>)> = (0..phases).map(|k| (format!("phase +{k}"), k, None, None)).collect();
    for &t in restart_ticks {
        for how in crate::liveness::Restart::rows() {
            specs.push((format!("restart at t {t} ({})", how.label()), 0, Some((t, how)), None));
        }
    }
    for &(s, len) in freezes {
        specs.push((format!("frozen t {s}..{}", s + len), 0, None, Some((s, len))));
    }
    let trace = std::env::var("WAYCELL_RIDE_TRACE").ok();
    // (G1 on ac58d99cb, note c) the row with the least fence gap, named in the summary
    let mut fence_min_row = String::new();
    // (ironspec 22:09 (a)) the closest any fence box (post and arms as laid) comes to the cart's box
    let mut fence_min = f64::INFINITY;
    // (ironspec 23:56: no vacuous passes) each bar's samples: game-time steps, cart ticks (contacts),
    // rider ticks (the eye), fence-gap samples over the way-in box's fences
    let fences = fences_in(&base.world, [2, -27, -8], [9, -11, 3]);
    // (ironspec 10:02 (b), the shared `contacts::stale_declared`) the declared contacts in the ride's
    // own scope (the way-in's box, the fences' box): each must be met at least once over the bar rows
    // or it is STALE (the row set fails); the record's other declarations (lanes, drop entries) are
    // out of this bench's scope
    let declared_in_scope: Vec<BlockPos> = o.declared_contacts.iter().filter(|c| (2..=9).contains(&c[0]) && (-27..=-11).contains(&c[1]) && (-8..=3).contains(&c[2])).map(|c| BlockPos::new(c[0], c[1], c[2])).collect();
    let mut declared_hits: std::collections::HashMap<[i32; 3], usize> = std::collections::HashMap::new();
    // (ironspec 10:42 / 10:58: the bounded form's REFERENCE figures over the bar rows) per declared
    // cell: its contact ticks and events, the most ticks in one event and events in one row, the
    // largest clipped amount per axis, the speed into and out of its ticks, the stalls
    let mut decl_stats: std::collections::BTreeMap<[i32; 3], DeclStats> = std::collections::BTreeMap::new();
    // (ironspec 10:42 / 10:58, G1 14:51 (4); G1's B1 / B2 on R4) main's ONE bounded judge over the bar
    // rows: every bar row's hits tagged with the row as their sample and RE-KEYED to it (the judge's cart
    // key = the row, so adjacent phases never merge into one event), judged ONCE over the set with the
    // samples (rows, "rows") (rate and STALE judged on the set), each failure mapped back to its row; the
    // same pooled hits fold `contacts::reference_check`
    let mut judge_rows = 0u32;
    let mut all_hits: Vec<crate::contacts::Hit> = Vec::new();
    let mut all_motion: std::collections::HashMap<(u32, i64), (f64, f64)> = std::collections::HashMap::new();
    // each `rows` entry's judge row (None for a freeze row), and each judge row's release tick
    let mut row_judge_idx: Vec<Option<u32>> = Vec::new();
    let mut row_release: Vec<i64> = Vec::new();
    // (ironspec 14:52) the first bar row with a contact: its contact ticks' moves, printed
    let mut evidence: Vec<String> = Vec::new();
    let mut evidence_row: Option<String> = None;
    let (mut steps_n, mut cart_ticks_n, mut rider_ticks_n, mut fence_n) = (0u64, 0u64, 0u64, 0u64);
    // (b8949c96 09:51) the bar rows' ticks with an UNDECLARED contact (the contact bar's count; the
    // declared ones print per row)
    let mut undeclared_contact_ticks = 0u64;
    let mut trapdoor_signal_n = 0u64;
    // (G1 03:27, ironspec 03:30 (2)) each watched cell: the rider's box / eye box overlaps, least gaps,
    // the first eye overlap and the least eye gap's row
    let mut watch: Vec<(u64, u64, f64, f64, Option<String>, String)> = o.watch_cells.iter().map(|_| (0, 0, f64::INFINITY, f64::INFINITY, None, String::new())).collect();
    // (ironspec 03:22 / 03:30) the redstone block powering the iron trapdoor (8,-25,-4), from below
    // (8,-26,-4) or above (8,-24,-4): its five other neighbours checked every tick (no component)
    // the power source: a redstone block or a lit redstone (wall) torch among the trapdoor's six
    // neighbours (fill (i): the block under it; fill (iii): the wall torch south of it)
    let power_src = [[8, -26, -4], [8, -24, -4], [8, -25, -3], [8, -25, -5], [7, -25, -4], [9, -25, -4]].into_iter().find(|c| {
        let s = format!("{}", base.world.get(BlockPos::new(c[0], c[1], c[2])));
        s == "minecraft:redstone_block" || (s.starts_with("minecraft:redstone_wall_torch[") || s.starts_with("minecraft:redstone_torch[")) && s.contains("lit=true")
    });
    let keep_nb: Vec<BlockPos> = power_src.map_or(Vec::new(), |c| {
        [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
            .iter()
            .map(|d| BlockPos::new(c[0] + d[0], c[1] + d[1], c[2] + d[2]))
            .filter(|p| *p != BlockPos::new(8, -25, -4))
            .collect()
    });
    // inert: no redstone component (air, glass, stone-like solids, glowstone, a PLAIN rail: none of
    // them reacts to the source's power)
    let inert = |b: &redsim::block::Block| -> bool {
        let s = format!("{b}");
        b.is_air() || s.starts_with("minecraft:glass") || s.starts_with("minecraft:rail[") || s == "minecraft:smooth_stone" || s == "minecraft:stone" || s == "minecraft:glowstone"
    };
    let mut keep_violation_n = 0u64;
    // (ironspec 05:09) the per-tick actual-state HOME probe over the descent (its cache per row: G1
    // 07:16 (7), the row's world, not another row's, answers each probe)
    let (mut probe_n, mut probe_partial_n, mut probe_known_positive) = (0u64, 0u64, false);
    let mut probe_computed_n = 0u64;
    let (mut probe_partial_cells, mut probe_partial_where): (Vec<[i32; 3]>, Vec<String>) = (Vec::new(), Vec::new());
    let src_block0 = power_src.map_or(Block::Air, |c| base.world.get(BlockPos::new(c[0], c[1], c[2])));
    let mut src_changed_n = 0u64;
    for (label, k, restart, freeze) in specs {
        let mut run = base.clone();
        let mut probe_cache: std::collections::HashMap<(i64, i64, i64, bool), bool> = std::collections::HashMap::new();
        for _ in 0..k {
            run.tick();
        }
        release(&mut run.world, o)?;
        // the release's game time (the bounded judge's hit ticks printed relative to it)
        let t_release = run.world.time;
        let mut cart = cart0;
        let mut frozen: Option<[i32; 4]> = None;
        let rider_of = |w: &redsim::world::World, c: u32| w.entities.list.iter().position(|v| !v.removed && v.vehicle == Some(c));
        let h0 = rider_of(&run.world, cart).map(|i| redsim::entity::mob::mob(&run.world, i).health).unwrap_or(0.0);
        let mut h_min = h0;
        let (mut at_entry, mut v_entry, mut landed, mut derailed, mut back_south) = (None, None, None::<u32>, false, false);
        // (G1 09:31 E2) the band bench's t0 in this row: the tick and speed when the cart's centre is at
        // or past the entry cell's centre heading west, post-tick
        let mut centre: Option<(u32, f64)> = None;
        let mut last_seen: Option<([i32; 3], f64)> = None;
        let (mut box_z_max, mut vz_max) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut rail_end_v: Option<f64> = None;
        let mut restart_cell: Option<P> = None;
        let mut eye_wet = 0u32;
        let mut contacts = 0u32;
        let mut declared_contact_ticks = 0u32;
        // (ironspec 10:20, the bounded-contact rule's terms, printed until its form is fixed) each
        // declared contact tick of this row: the cell, the clipped amount per axis (requested -
        // returned of the cart's Entity.move(SELF) calls that tick), the horizontal speed before and
        // after the tick, and whether the cart moved on the next tick (no stall)
        let mut declared_events: Vec<String> = Vec::new();
        let mut v_prev: Option<f64> = None;
        let mut stall_check: Option<(Option<usize>, [f64; 3], Vec<[i32; 3]>)> = None;
        // (ironspec 10:42 / 10:58) this row's declared-contact runs: the cells met last tick, each
        // cell's current run length and its events (maximal consecutive runs) in this row
        let mut decl_prev: Vec<[i32; 3]> = Vec::new();
        let mut row_runs: std::collections::HashMap<[i32; 3], u32> = std::collections::HashMap::new();
        let mut row_events: std::collections::HashMap<[i32; 3], u32> = std::collections::HashMap::new();
        // (ironspec 10:42 / 10:58, G1 14:51 (4)) this row's hits and motion for main's bounded judge
        let mut row_hits: Vec<crate::contacts::Hit> = Vec::new();
        let mut row_motion: std::collections::HashMap<i64, (f64, f64)> = std::collections::HashMap::new();
        // the cart's stored motion at the previous tick's end (the evidence print's "before")
        let mut dm_before = [0.0f64; 3];
        let mut contact_cells: Vec<BlockPos> = Vec::new();
        let mut time_jumps = 0u32;
        let mut idle_n: Option<i64> = None;
        for t in 0..ticks {
            // (ironspec 20:35: the game-time continuity check; G1's CHANGES 1 on ac58d99cb) every step
            // one game tick, a restart step 1 + the world open's N idle ticks (the helper's count)
            let t_before = run.world.time;
            let mut expect = 1i64;
            if let Some((rt, how)) = restart {
                if t as i64 == rt {
                    // (G1 on e9f0f4cbf: the cart's cell at the restart, so a row shows where it was)
                    restart_cell = run.world.entities.list.iter().find(|e| e.id == cart && !e.removed).map(|e| [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32]);
                    let rs = crate::liveness::restart(&mut run.world, how)?;
                    expect = 1 + rs.entity_free;
                    idle_n = Some(rs.entity_free);
                    cart = *rs.get(&cart).ok_or("the cart lost in the restart")?;
                }
            }
            if let Some((fs, fl)) = freeze {
                if t as i64 == fs {
                    frozen = Some(crate::liveness::freeze32(&mut run.world));
                }
                if t as i64 == fs + fl {
                    if let Some(pb) = frozen.take() {
                        crate::liveness::thaw32(&mut run.world, pb);
                    }
                }
            }
            run.tick();
            // (G1 07:16 (7)) only the bar rows (plain and restart) feed the bars' samples; a freeze row
            // (STRESS) prints beside them and never bars, its samples included
            let bar_row = freeze.is_none();
            steps_n += bar_row as u64;
            if run.world.time - t_before != expect {
                time_jumps += 1;
            }
            // (ironspec 02:48 (1)) the oak trapdoor at the bottom curve's inner corner must never see a
            // signal (it would toggle shut): hasNeighborSignal at (8,-25,-4) every tick
            if bar_row && run.world.has_neighbor_signal(BlockPos::new(8, -25, -4)) {
                trapdoor_signal_n += 1;
            }
            if bar_row && keep_nb.iter().any(|p| !inert(&run.world.get(*p))) {
                keep_violation_n += 1;
            }
            // (G1 on fill (iii), check 1) the power source itself survives every tick (a torch is
            // destroyed by flowing water): its block unchanged
            if let (true, Some(c)) = (bar_row, power_src) {
                if run.world.get(BlockPos::new(c[0], c[1], c[2])) != src_block0 {
                    src_changed_n += 1;
                }
            }
            let Some(ci) = run.world.entities.list.iter().position(|e| e.id == cart && !e.removed) else { break };
            // (ironspec 10:02: ONE CONTACT RULE) the cart's contacts this tick through the SHARED function
            // (mobfarm::contacts::cart_contacts on redsim's recorded Entity.move(SELF) calls: an axis the
            // collision clipped of the attempted move; ironfarm's 329b5acc3, carried here until it lands)
            let shared = crate::contacts::cart_contacts(&run.world, ci);
            // (ironspec 10:42 / 10:58 / 14:52, G1 14:51 (4)) the row's hits for main's ONE bounded judge:
            // the horizontal face contacts and the vertical classes (floor, ceiling, low ceiling,
            // embedded), each as a Hit; the cart's motion this tick for the judge's stall rule
            if bar_row {
                let hz = crate::contacts::cart_hits(&run.world, ci);
                let vt: Vec<crate::contacts::Hit> = crate::contacts::cart_vertical_hits(&run.world, ci).into_iter().map(crate::contacts::Hit::from).collect();
                // (ironspec 14:52: classify a contact before declaring it) the first bar row's contact
                // ticks with the cart's moves: requested and returned per axis, the box before the move,
                // the stored motion before and after the tick, and each hit's block and face
                if evidence_row.is_none() && (!hz.is_empty() || !vt.is_empty()) {
                    evidence_row = Some(label.clone());
                }
                if evidence_row.as_deref() == Some(label.as_str()) && (!hz.is_empty() || !vt.is_empty()) {
                    let e = &run.world.entities.list[ci];
                    let dm = e.delta_movement;
                    for m in e.self_moves.iter().filter(|m| m.tick == run.world.time) {
                        let b = m.bb_before;
                        evidence.push(format!(
                            "(contact evidence, {label}) t {} after the release: Entity.move(SELF) requested ({:.4}, {:.4}, {:.4}) returned ({:.4}, {:.4}, {:.4}); the cart's box before the move x {:.4}..{:.4} y {:.4}..{:.4} z {:.4}..{:.4}; its stored motion before the tick ({:.4}, {:.4}, {:.4}), after ({:.4}, {:.4}, {:.4}); the hits: {}",
                            run.world.time - t_release,
                            m.requested.x, m.requested.y, m.requested.z,
                            m.returned.x, m.returned.y, m.returned.z,
                            b.min_x, b.max_x, b.min_y, b.max_y, b.min_z, b.max_z,
                            dm_before[0], dm_before[1], dm_before[2],
                            dm.x, dm.y, dm.z,
                            hz.iter().chain(vt.iter()).map(|h| format!("({},{},{}) {} face {} depth {:.4}", h.cell.x, h.cell.y, h.cell.z, run.world.get(h.cell), h.face.name(), h.depth)).collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
                row_hits.extend(hz);
                row_hits.extend(vt);
                let dm = run.world.entities.list[ci].delta_movement;
                row_motion.insert(run.world.time, ((dm.x * dm.x + dm.z * dm.z).sqrt(), dm.y));
            }
            {
                let dm = run.world.entities.list[ci].delta_movement;
                dm_before = [dm.x, dm.y, dm.z];
            }
            let e = &run.world.entities.list[ci];
            cart_ticks_n += bar_row as u64;
            let v_now = (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt();
            // the previous declared event's stall check: did the cart move on the tick after it?
            if let Some((ev, p0, cells0)) = stall_check.take() {
                let moved = (e.pos.x - p0[0]).abs() + (e.pos.y - p0[1]).abs() + (e.pos.z - p0[2]).abs() > 1e-6;
                if let Some(s) = ev.and_then(|ev| declared_events.get_mut(ev)) {
                    s.push_str(if moved { ", the next tick moving (no stall)" } else { ", the next tick NOT MOVING (a stall)" });
                }
                if !moved {
                    for c in cells0 {
                        decl_stats.entry(c).or_default().stalls += 1;
                    }
                }
            }
            // (ironspec 10:42 / 10:58: the bounded form's reference figures) the declared cells this
            // tick met, for the per-cell events (maximal consecutive runs) and ticks
            let mut decl_now: Vec<[i32; 3]> = Vec::new();
            if !shared.is_empty() {
                let cells: Vec<BlockPos> = shared.iter().map(|(p, _)| *p).collect();
                // a tick whose contact cells are all declared (the design's `# contacts:` / `# stops:`,
                // read by the shared strict reader) is a declared contact, not a failure; one
                // undeclared cell makes it a contact tick
                let undeclared = cells.iter().any(|c| !o.declared_contacts.contains(&[c.x, c.y, c.z]));
                if undeclared {
                    contacts += 1;
                    undeclared_contact_ticks += bar_row as u64;
                } else {
                    declared_contact_ticks += 1;
                }
                let t_now = run.world.time;
                // the tick's clipped amount per axis (requested - returned, the largest of its moves)
                let mut clip = [0.0f64; 3];
                for m in e.self_moves.iter().filter(|m| m.tick == t_now) {
                    for (k, d) in [m.requested.x - m.returned.x, m.requested.y - m.returned.y, m.requested.z - m.returned.z].into_iter().enumerate() {
                        if d.abs() > clip[k].abs() {
                            clip[k] = d;
                        }
                    }
                }
                decl_now = cells.iter().filter(|c| o.declared_contacts.contains(&[c.x, c.y, c.z])).map(|c| [c.x, c.y, c.z]).collect();
                if bar_row {
                    for c in &decl_now {
                        let run_n = if decl_prev.contains(c) { row_runs.get(c).copied().unwrap_or(0) + 1 } else { 1 };
                        row_runs.insert(*c, run_n);
                        let st = decl_stats.entry(*c).or_default();
                        st.ticks += 1;
                        if run_n == 1 {
                            st.events += 1;
                            *row_events.entry(*c).or_default() += 1;
                        }
                        st.max_event_ticks = st.max_event_ticks.max(run_n);
                        for k in 0..3 {
                            st.clip[k] = st.clip[k].max(clip[k].abs());
                        }
                        if let Some(vp) = v_prev {
                            st.v_in = Some(st.v_in.map_or((vp, vp), |(a, b)| (a.min(vp), b.max(vp))));
                        }
                        st.v_out = Some(st.v_out.map_or((v_now, v_now), |(a, b)| (a.min(v_now), b.max(v_now))));
                    }
                }
                let pushed = if !undeclared && declared_events.len() < 4 {
                    declared_events.push(format!(
                        "{} at t {t}: clipped x {:.4} y {:.4} z {:.4}; speed {} -> {v_now:.4} b/t",
                        cells.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("+"),
                        clip[0],
                        clip[1],
                        clip[2],
                        v_prev.map_or("-".to_string(), |v| format!("{v:.4}"))
                    ));
                    Some(declared_events.len() - 1)
                } else {
                    None
                };
                // the stall check on the next tick: the printed event's, and the declared cells' stats
                if !decl_now.is_empty() {
                    stall_check = Some((pushed, [e.pos.x, e.pos.y, e.pos.z], if bar_row { decl_now.clone() } else { Vec::new() }));
                }
                for c in cells {
                    // (ironspec 10:02 (b)) each declared cell's hits over the bar rows (the stale check)
                    if bar_row && o.declared_contacts.contains(&[c.x, c.y, c.z]) {
                        *declared_hits.entry([c.x, c.y, c.z]).or_default() += 1;
                    }
                    if !contact_cells.contains(&c) {
                        contact_cells.push(c);
                    }
                }
            }
            v_prev = Some(v_now);
            decl_prev = decl_now;
            if k == 0 && restart.is_none() && freeze.is_none() {
                if let Some(v) = positions.as_deref_mut() {
                    v.push([e.pos.x, e.pos.y, e.pos.z]);
                }
            }
            if let (true, Some(g)) = (bar_row, fence_gap(&run.world, &e.bb, &fences)) {
                fence_n += 1;
                if g < fence_min {
                    fence_min_row = label.clone();
                }
                fence_min = fence_min.min(g);
            }
            last_seen = Some(([e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32], (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt()));
            let c = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
            let px = e.pos.x;
            // (ironspec 20:21: name the freeze rows' extra speed) WAYCELL_RIDE_TRACE=<a label's text>:
            // that row tick by tick
            if trace.as_deref().is_some_and(|f| label.contains(f)) {
                let cell = BlockPos::new(c[0], c[1], c[2]);
                eprintln!(
                    "ride-trace [{label}] t {t}: cart ({:.4},{:.4},{:.4}) v ({:.4},{:.4},{:.4}) wet {} frozen {} ticking {} cell {} below {}",
                    e.pos.x,
                    e.pos.y,
                    e.pos.z,
                    e.delta_movement.x,
                    e.delta_movement.y,
                    e.delta_movement.z,
                    e.was_touching_water,
                    frozen.is_some(),
                    run.world.entity_ticks_at(e.pos.x, e.pos.z),
                    run.world.get(cell),
                    run.world.get(cell.below())
                );
            }
            let (pz, vz) = (e.pos.z, e.delta_movement.z);
            let v = e.delta_movement;
            if let Some(i) = rider_of(&run.world, cart) {
                rider_ticks_n += bar_row as u64;
                h_min = h_min.min(redsim::entity::mob::mob(&run.world, i).health);
                // (ironspec 19:12 (d)) the rider's eye above the water at every position
                let r = &run.world.entities.list[i];
                // (G1 03:27 on the redstone block (8,-24,-4)) the rider's box and its in-wall eye box
                // (`isInWall`: width * 0.8 around the eye, 1e-6 tall) against each watched cell's full
                // cube, every tick: the overlaps counted and the least clearance printed (ironspec 03:30
                // (2): bar 0 eye overlaps at every new solid cell); bar rows only (G1 07:16 (7))
                for (wi, wc) in o.watch_cells.iter().enumerate().filter(|_| bar_row) {
                    let (cx, cy, cz) = (wc[0] as f64, wc[1] as f64, wc[2] as f64);
                    let gap = |b: &redsim::entity::vec3::Aabb| -> f64 {
                        let dx = (cx - b.max_x).max(b.min_x - (cx + 1.0)).max(0.0);
                        let dy = (cy - b.max_y).max(b.min_y - (cy + 1.0)).max(0.0);
                        let dz = (cz - b.max_z).max(b.min_z - (cz + 1.0)).max(0.0);
                        if dx == 0.0 && dy == 0.0 && dz == 0.0 { -1.0 } else { (dx * dx + dy * dy + dz * dz).sqrt() }
                    };
                    let half = 0.6 * 0.8 / 2.0;
                    let ey = r.pos.y + r.kind.eye_height() as f64;
                    let eb = redsim::entity::vec3::Aabb { min_x: r.pos.x - half, min_y: ey - 5.0e-7, min_z: r.pos.z - half, max_x: r.pos.x + half, max_y: ey + 5.0e-7, max_z: r.pos.z + half };
                    let (gb, ge) = (gap(&r.bb), gap(&eb));
                    let w = &mut watch[wi];
                    if gb < 0.0 {
                        w.0 += 1;
                    } else {
                        w.2 = w.2.min(gb);
                    }
                    if ge < 0.0 {
                        w.1 += 1;
                        if w.4.is_none() {
                            w.4 = Some(format!("{label} t {t} rider ({:.3},{:.3},{:.3})", r.pos.x, r.pos.y, r.pos.z));
                        }
                    } else if ge < w.3 {
                        w.3 = ge;
                        w.5 = format!("{label} t {t}");
                    }
                }
                let eye = BlockPos::new(r.pos.x.floor() as i32, (r.pos.y + r.kind.eye_height() as f64).floor() as i32, r.pos.z.floor() as i32);
                if redsim::fluid::fluid_state(&run.world, eye).fluid.is_water() {
                    eye_wet += 1;
                }
            }
            if let Some(l) = landing {
                if landed.is_none() && c == l {
                    landed = Some(t);
                }
                if landed.is_some() && at_entry.is_none() {
                    let on_rail = redsim::rails::is_rail(&run.world.get(BlockPos::new(c[0], c[1], c[2]))) || redsim::rails::is_rail(&run.world.get(BlockPos::new(c[0], c[1] - 1, c[2])));
                    derailed |= !on_rail;
                    // back south over the high side: the cart's box reaching into the high side's
                    // cell (z >= the landing cell's south face); its southward speeds printed
                    box_z_max = box_z_max.max(e.bb.max_z);
                    vz_max = vz_max.max(vz);
                    back_south |= e.bb.max_z > (l[2] + 1) as f64 + 1e-9;
                    let _ = pz;
                }
            }
            // (ironspec 18:39 (4): the rail end's normal-operation bar) the cart's horizontal speed the
            // tick its cell first becomes the rail end (9,-15,-2)
            if rail_end_v.is_none() && c == rail_end_of(o.queue) {
                rail_end_v = Some((e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt());
            }
            // (ironspec 05:09: the R2 search's gate) the HOME path from the rider's ACTUAL state at every
            // descent tick of every row (its box and wasTouchingWater as the tick left them, so
            // getStart's water branch applies as in the game): partial or complete, cached by the
            // rider's position (1e-4) and water state, per row (G1 07:16 (7)); the partial ones listed
            // (bar: 0); bar rows only
            if let (Some(bed), true, true) = (o.probe_bed, at_entry.is_none(), freeze.is_none()) {
                if let Some(i) = rider_of(&run.world, cart) {
                    let (p, wet) = (run.world.entities.list[i].pos, run.world.entities.list[i].was_touching_water);
                    let key = ((p.x * 1e4).round() as i64, (p.y * 1e4).round() as i64, (p.z * 1e4).round() as i64, wet);
                    let reached = match probe_cache.get(&key) {
                        Some(r) => *r,
                        None => {
                            let bed_p = BlockPos::new(bed[0], bed[1], bed[2]);
                            let res = redsim::entity::mob::ai::with_nav(&mut run.world, i, |w, nm, nav, arena| {
                                let mut copy = nav.clone();
                                copy.create_path_pos(w, nm, bed_p, 0, arena).map(|pa| pa.reached)
                            });
                            let r = res.unwrap_or(false);
                            probe_cache.insert(key, r);
                            probe_computed_n += 1;
                            r
                        }
                    };
                    probe_n += 1;
                    if !reached {
                        probe_partial_n += 1;
                        let cell = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
                        if !probe_partial_cells.contains(&cell) {
                            probe_partial_cells.push(cell);
                        }
                        if probe_partial_where.len() < 8 {
                            probe_partial_where.push(format!("{label} t {t} rider ({:.3},{:.3},{:.3}){}", p.x, p.y, p.z, if wet { " in water" } else { "" }));
                        }
                        // the known positive (ironspec 05:09): R58-m8t's (7.745, -22.9375, -0.5)
                        if (p.x - 7.745).abs() < 0.06 && (p.y + 22.9375).abs() < 0.05 && (p.z + 0.5).abs() < 0.05 {
                            probe_known_positive = true;
                        }
                    }
                }
            }
            if at_entry.is_none() && c == entry {
                at_entry = Some(t);
                v_entry = Some(v);
            }
            // (G1 09:31 E2) the band bench's t0: the cart's centre at or past the entry cell's centre
            // heading west (x <= entry.x + 0.5) while in the entry cell, post-tick; the row ticks on
            // past the cell entry until then (at most 8 gt, or until the cart leaves the cell)
            if let Some(t_in) = at_entry {
                if centre.is_none() && c == entry && px <= entry[0] as f64 + 0.5 {
                    centre = Some((t, (v.x * v.x + v.z * v.z).sqrt()));
                }
                if centre.is_some() || c != entry || t >= t_in + 8 {
                    break;
                }
            }
        }
        centres.push(centre);
        // (ironspec 18:39 (4)) the rail end's entry speed of every bar row (a freeze row's prints apart)
        if freeze.is_none() {
            rail_end_speeds.push(rail_end_v);
        }
        // (ironspec 10:42 / 10:58) the row's events per declared cell, for the per-row maximum
        if freeze.is_none() {
            for (c, n) in &row_events {
                let st = decl_stats.entry(*c).or_default();
                st.max_row_events = st.max_row_events.max(*n);
                st.rows += 1;
            }
            // (G1's B1 / B2 on R4) the row's hits tagged with the row as their sample and re-keyed to it,
            // its motion keyed by (row, tick): judged once over the set below
            let ri = judge_rows;
            judge_rows += 1;
            all_hits.extend(rekey_row(ri, row_hits));
            row_release.push(t_release);
            all_motion.extend(row_motion.into_iter().map(|(t, m)| ((ri, t), m)));
            row_judge_idx.push(Some(ri));
        } else {
            row_judge_idx.push(None);
        }
        rows.push((
            format!(
                "{label}{}{}{}{}{} [after the landing: {}]",
                restart_cell.map_or(String::new(), |c| format!(" (the cart at {c:?} at the restart, {} idle tick(s))", idle_n.unwrap_or(0))),
                if eye_wet > 0 { format!(" (the rider's EYE IN WATER {eye_wet} ticks)") } else { String::new() },
                if at_entry.is_none() { last_seen.map_or(" (the cart gone)".to_string(), |(c, s)| format!(" (the cart last at {c:?} at {s:.4} b/t)")) } else { String::new() },
                if time_jumps > 0 { format!(" (GAME TIME NOT CONTINUOUS at {time_jumps} steps)") } else { String::new() },
                if contacts > 0 {
                    format!(" (CONTACTS {contacts} ticks at {})", contact_cells.iter().filter(|c| !o.declared_contacts.contains(&[c.x, c.y, c.z])).map(|c| format!("{c} {}", run.world.get(*c))).collect::<Vec<_>>().join(", "))
                } else if declared_contact_ticks > 0 {
                    format!(
                        " (declared contacts {declared_contact_ticks} tick(s) at {}: {})",
                        contact_cells.iter().map(|c| format!("{c} {}", run.world.get(*c))).collect::<Vec<_>>().join(", "),
                        declared_events.join(" | ")
                    )
                } else {
                    String::new()
                },
                // (G1 03:13) a row with no tick after the landing prints so, never "-inf"
                if box_z_max.is_finite() { format!("the box's z at most {box_z_max:.4}, vz at most {vz_max:.4}") } else { "no tick after the landing".to_string() }
            ),
            at_entry,
            v_entry,
            h0 - h_min,
            derailed || (landing.is_some() && landed.is_none()),
            back_south,
        ));
    }
    let mut out = override_lines;
    // (C)'s entry bar at A6's entry heading west: `o.band` (ironspec 21:25: A6's operating band
    // [0.11, 0.24]; inherited from 58f770cc's A6 park sweep)
    let band = o.band;
    let ok_v = |v: &Vec3| {
        let s = (v.x * v.x + v.z * v.z).sqrt();
        (band[0]..=band[1]).contains(&s) && v.x < 0.0 && v.x.abs() > v.z.abs()
    };
    for ((label, t, v, hurt, derail, back), centre) in rows.iter().zip(&centres) {
        out.push(format!(
            "(ride) {label}: the entry at t {} after the release (the first post-tick state in the entry cell), {}{}; the entry cell's centre (the band bench's t0: x <= {:.1}, post-tick) at {}; hurt {hurt:.1}; {}{}",
            t.map_or("NEVER".to_string(), |t| t.to_string()),
            v.map_or("-".to_string(), |v| format!("{:.4} b/t (vx {:.4}, vz {:.4})", (v.x * v.x + v.z * v.z).sqrt(), v.x, v.z)),
            if v.as_ref().is_some_and(ok_v) { "" } else { " OUT OF THE BAR" },
            entry[0] as f64 + 0.5,
            centre.map_or("NEVER".to_string(), |(ct, cs)| format!("t {ct}, {cs:.4} b/t")),
            if *derail { "OFF THE RAILS or no landing" } else { "on rails from the landing" },
            if *back { ", MOVED BACK SOUTH" } else { "" }
        ));
    }
    // (G1 09:31 E2) the band bench's t0 over the bar rows: the entry cell's centre passed, post-tick
    let centre_ts: Vec<u32> = rows.iter().zip(&centres).filter(|(r, _)| !r.0.starts_with("frozen")).filter_map(|(_, c)| c.map(|c| c.0)).collect();
    let centre_ss: Vec<f64> = rows.iter().zip(&centres).filter(|(r, _)| !r.0.starts_with("frozen")).filter_map(|(_, c)| c.map(|c| c.1)).collect();
    let centre_line = format!(
        "; the entry cell's centre (the band bench's t0: x <= {:.1}, post-tick) passed in {} of the bar rows at t {}..{} after the release, {:.4}..{:.4} b/t",
        entry[0] as f64 + 0.5,
        centre_ts.len(),
        centre_ts.iter().min().copied().unwrap_or(0),
        centre_ts.iter().max().copied().unwrap_or(0),
        centre_ss.iter().cloned().fold(f64::INFINITY, f64::min),
        centre_ss.iter().cloned().fold(0.0, f64::max)
    );
    // (G1's B1 / B2 / B12 on R4) main's ONE bounded judge, once over every bar row's re-keyed hits (the
    // samples: the rows), each failure mapped back to its row (an undeclared hit's key, the row named
    // "cart N" in an exceeded bound or a stall), the failing rows marked JUDGE in their text
    let verdict = crate::contacts::judge(&o.declared_bounded, &all_hits, &|c, t| all_motion.get(&(c, t)).copied(), crate::contacts::StallRule::IRONSPEC_1042, Some((judge_rows as u64, "rows")));
    let row_of = |s: &str| -> Option<u32> {
        let i = s.find("cart ")? + 5;
        s[i..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
    };
    let mut row_fail: std::collections::BTreeMap<u32, Vec<String>> = std::collections::BTreeMap::new();
    let rel = |h: &crate::contacts::Hit| h.tick - row_release.get(h.cart as usize).copied().unwrap_or(0);
    for h in &verdict.undeclared {
        row_fail.entry(h.cart).or_default().push(format!("undeclared t {} ({},{},{}) face {} depth {:.4}", rel(h), h.cell.x, h.cell.y, h.cell.z, h.face.name(), h.depth));
    }
    for s in verdict.exceeded.iter().chain(verdict.stalls.iter()) {
        if let Some(r) = row_of(s) {
            row_fail.entry(r).or_default().push(s.clone());
        }
    }
    for (r, idx) in rows.iter_mut().zip(row_judge_idx.iter()) {
        if let Some(fails) = idx.and_then(|i| row_fail.get(&i)) {
            r.0.push_str(&format!(" (JUDGE: {})", fails.join("; ")));
        }
    }
    // (ironspec 01:00 (1): freezes print as STRESS) the bar rows are the plain and restart rows; the
    // freeze rows print beside them and never bar
    let bar: Vec<&(String, Option<u32>, Option<Vec3>, f32, bool, bool)> = rows.iter().filter(|r| !r.0.starts_with("frozen")).collect();
    let stress: Vec<&(String, Option<u32>, Option<Vec3>, f32, bool, bool)> = rows.iter().filter(|r| r.0.starts_with("frozen")).collect();
    let ts: Vec<u32> = bar.iter().filter_map(|r| r.1).collect();
    let ss: Vec<f64> = bar.iter().filter_map(|r| r.2.map(|v| (v.x * v.x + v.z * v.z).sqrt())).collect();
    // (G1's B9 on R4) the old per-cell CONTACTS count is information: the judge's verdict gates the run
    let all_ok = bar.iter().all(|r| r.1.is_some() && r.2.as_ref().is_some_and(ok_v) && r.3 <= 0.0 && !r.4 && !r.5 && !r.0.contains("EYE IN WATER") && !r.0.contains("GAME TIME NOT"));
    let st_ts: Vec<u32> = stress.iter().filter_map(|r| r.1).collect();
    let st_ss: Vec<f64> = stress.iter().filter_map(|r| r.2.map(|v| (v.x * v.x + v.z * v.z).sqrt())).collect();
    let stress_line = if stress.is_empty() {
        String::new()
    } else {
        format!(
            "; STRESS (freezes, never a bar): {} rows, the entry reached in {} at t {}..{}, {:.4}..{:.4} b/t, {} out of the bar or flagged",
            stress.len(),
            st_ts.len(),
            st_ts.iter().min().copied().unwrap_or(0),
            st_ts.iter().max().copied().unwrap_or(0),
            st_ss.iter().cloned().fold(f64::INFINITY, f64::min),
            st_ss.iter().cloned().fold(0.0, f64::max),
            stress.iter().filter(|r| !(r.1.is_some() && r.2.as_ref().is_some_and(ok_v) && r.3 <= 0.0 && !r.4 && !r.5 && !r.0.contains("GAME TIME NOT"))).count()
        )
    };
    // (ironspec 23:56: no vacuous passes) each bar with its samples; a bar with none is NOT JUDGED
    // and fails the run, but the fence gap's declared scope: no fence in the way-in's box
    let judged = |n: u64, what: &str| if n == 0 { format!("{what}: NOT JUDGED (0 samples)") } else { format!("{what} over {n} samples") };
    let vacuous = bar.is_empty() || ts.is_empty() || steps_n == 0 || cart_ticks_n == 0 || rider_ticks_n == 0 || (!fences.is_empty() && fence_n == 0);
    // (ironspec 02:48 / 03:22) the bottom curve's inner-corner trapdoor (8,-25,-4): an oak one must
    // never see a signal (it would shut); an iron one is held open by the redstone block (8,-24,-4)
    // over it, so it must see the signal every tick, and that block's other five neighbours must hold
    // no redstone component (the inverted keep-out, printed with each neighbour's block)
    let td = base.world.get(BlockPos::new(8, -25, -4));
    let td_s = format!("{td:?}");
    let (td_line, td_ok) = if td_s.contains("IronTrapdoor") || td_s.contains("iron_trapdoor") || td_s.contains("iron: true") {
        let nb: Vec<String> = keep_nb.iter().map(|p| format!("({},{},{}) {:?}", p.x, p.y, p.z, base.world.get(*p))).collect();
        (
            format!(
                "the iron trapdoor (8,-25,-4) sees a signal {trapdoor_signal_n} of {steps_n} ticks (bar: every tick), held open by {}, the source changed (gone, unlit) on {src_changed_n} of {steps_n} ticks (bar: never); the source's five other neighbours (the keep-out: no component) {}, a component among them on {keep_violation_n} of {steps_n} ticks (bar: never)",
                power_src.map_or("NO redstone block or lit redstone torch beside it".to_string(), |c| format!("{} at ({},{},{})", base.world.get(BlockPos::new(c[0], c[1], c[2])), c[0], c[1], c[2])),
                nb.join(", ")
            ),
            steps_n > 0 && trapdoor_signal_n == steps_n && power_src.is_some() && keep_violation_n == 0 && src_changed_n == 0,
        )
    } else if td_s.contains("Trapdoor") || td_s.contains("trapdoor") {
        (format!("hasNeighborSignal at the oak trapdoor (8,-25,-4) {trapdoor_signal_n} of {steps_n} ticks (bar: never)"), trapdoor_signal_n == 0)
    } else {
        (format!("(8,-25,-4) holds {td:?}: the trapdoor's signal bar out of scope"), true)
    };
    // the watched cells: each one's block, the rider's box and in-wall eye box overlaps (bar 0 eye
    // overlaps where the cell is solid), least gaps
    let mut watch_ok = true;
    let watch_parts: Vec<String> = o
        .watch_cells
        .iter()
        .zip(watch.iter())
        .map(|(c, w)| {
            let b = base.world.get(BlockPos::new(c[0], c[1], c[2]));
            let solid = !b.is_air();
            if solid && w.1 > 0 {
                watch_ok = false;
            }
            format!(
                "({},{},{}) {b:?}: box {} tick(s){}, eye box {} tick(s){} ({})",
                c[0],
                c[1],
                c[2],
                w.0,
                if w.2.is_finite() { format!(" (least gap {:.4})", w.2) } else { String::new() },
                w.1,
                match &w.4 { Some(s) => format!(", first in {s}"), None => if w.3.is_finite() { format!(" (least gap {:.4} in {})", w.3, w.5) } else { String::new() } },
                if solid { "judged: bar 0 eye overlaps" } else { "air: not judged" }
            )
        })
        .collect();
    let watch_line = if watch_parts.is_empty() { String::new() } else { format!("; the rider's box and in-wall eye box against the watched cells: {}", watch_parts.join("; ")) };
    // (ironspec 05:09) the per-tick actual-state HOME probe's line (when asked for)
    let probe_ok = o.probe_bed.is_none() || (probe_n > 0 && probe_partial_n == 0);
    let watch_line = if o.probe_bed.is_some() {
        format!(
            "{watch_line}; the HOME path from the rider's actual state at every descent tick: {probe_n} ticks ({probe_computed_n} states computed, each row's own cache), PARTIAL at {probe_partial_n} (bar 0){}{}",
            if probe_partial_cells.is_empty() { String::new() } else { format!(" in the cells {:?}: {}", probe_partial_cells, probe_partial_where.join("; ")) },
            if probe_known_positive { "; the known positive (7.745, -22.9375, -0.5) came out PARTIAL" } else { "" }
        )
    } else {
        watch_line
    };
    // (G1 07:00 / 08:12: every verdict reaches the exit status) the ride rows' verdict returned beside
    // their lines
    let stale = crate::contacts::stale_declared(&declared_in_scope, &|p: BlockPos| declared_hits.get(&[p.x, p.y, p.z]).copied().unwrap_or(0));
    let stale_line = format!(
        "; (information: the horizontal contacts alone, the old count; main's bounded judge below decides, both axes) the declared contacts in the ride's scope {}: {}; their horizontal figures over the bar rows (ironspec 10:42 / 10:58): {}",
        if declared_in_scope.is_empty() { "(none)".to_string() } else { declared_in_scope.iter().map(|p| format!("{p} hit {} tick(s)", declared_hits.get(&[p.x, p.y, p.z]).copied().unwrap_or(0))).collect::<Vec<_>>().join(", ") },
        if stale.is_empty() { "none unmet horizontally".to_string() } else { format!("{stale:?} unmet horizontally (a floor or ceiling declaration is met on the vertical axis: the judge's STALE decides)") },
        if decl_stats.is_empty() {
            "none met".to_string()
        } else {
            decl_stats
                .iter()
                .map(|(c, s)| {
                    format!(
                        "({},{},{}): {} tick(s) in {} event(s) over {} row(s), at most {} tick(s) an event and {} event(s) a row, the largest clip x {:.4} y {:.4} z {:.4}, speed in {} -> out {} b/t, {} stall(s)",
                        c[0],
                        c[1],
                        c[2],
                        s.ticks,
                        s.events,
                        s.rows,
                        s.max_event_ticks,
                        s.max_row_events,
                        s.clip[0],
                        s.clip[1],
                        s.clip[2],
                        s.v_in.map_or("-".to_string(), |(a, b)| format!("{a:.4}..{b:.4}")),
                        s.v_out.map_or("-".to_string(), |(a, b)| format!("{a:.4}..{b:.4}")),
                        s.stalls
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        }
    );
    // (ironspec 10:58, G1 11:12 (b); G1 14:51 (4); G1's B1 / B2 on R4) main's ONE bounded judge's verdict
    // over the bar rows (judged once, above), and each in-scope design or rest declaration's reference
    // check over the same re-keyed hits (a mismatch fails a --reference-run, printed otherwise; a
    // --reference-run with no design or rest declaration in scope is NOT JUDGED and fails)
    let first = |v: &[String]| if v.is_empty() { String::new() } else { format!(" (first: {})", v.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) };
    let referenced: Vec<&crate::contacts::Decl> = o.declared_bounded.iter().filter(|d| matches!(d.class, crate::contacts::Class::Design | crate::contacts::Class::Rest)).collect();
    let mut ref_mismatch: Vec<String> = Vec::new();
    for d in &referenced {
        ref_mismatch.extend(crate::contacts::reference_check(d, &all_hits));
    }
    let ref_not_judged = o.reference_run && referenced.is_empty();
    // (G1's D1 on R4 v2) a LEGACY declaration in the ride's scope matches a face-less line on any face
    // with no bound (main's judge: legacy lines are judged for STALE and stalls only), so it would excuse
    // the hits it covers: NOT JUDGED, a failure, until the line is bounded
    let legacy_in_scope: Vec<String> = crate::vhits::legacy_lines(&o.declared_bounded);
    let judge_ok = !verdict.failed() && legacy_in_scope.is_empty() && !(o.reference_run && (!ref_mismatch.is_empty() || ref_not_judged));
    let und: Vec<String> = verdict.undeclared.iter().map(|h| format!("[row {}] t {} ({},{},{}) face {} depth {:.4} speed {:.4} -> {:.4}", h.cart, rel(h), h.cell.x, h.cell.y, h.cell.z, h.face.name(), h.depth, h.speed_in, h.speed_out)).collect();
    let judge_line = format!(
        "; main's bounded judge (contacts::judge once over the {judge_rows} bar row(s), each row's hits its own sample and key, the samples ({judge_rows}, \"rows\"): A ROW = one release phase x restart order, at this one build position (the loop frame as laid); the in-scope declarations {}; the stall rule ironspec 10:42's; one build position: a touching-contact bar, the set owed (G1 15:52 (4))){}: {} row(s) failing; {} undeclared hit(s){}; {} bound(s) exceeded{}; {} stall(s){}; {} stale{}; {} stale not judged{}; {} wiring error(s){}; events per declaration {:?}; the reference check ({}): {}",
        if o.declared_bounded.is_empty() { "(none)".to_string() } else { o.declared_bounded.iter().map(|d| format!("{} {:?}", d.cells.iter().map(|c| format!("({},{},{})", c.x, c.y, c.z)).collect::<Vec<_>>().join("+"), d.class)).collect::<Vec<_>>().join(", ") },
        if legacy_in_scope.is_empty() { String::new() } else { format!("; NOT JUDGED: a legacy declaration in the scope judges no bound: {}; bound it (class=…) first: FAILS", legacy_in_scope.join(", ")) },
        row_fail.len(),
        und.len(),
        first(&und),
        verdict.exceeded.len(),
        first(&verdict.exceeded),
        verdict.stalls.len(),
        first(&verdict.stalls),
        verdict.stale.len(),
        first(&verdict.stale),
        verdict.stale_not_judged.len(),
        first(&verdict.stale_not_judged),
        verdict.errors.len(),
        first(&verdict.errors),
        verdict.events,
        if o.reference_run { "--reference-run: judged" } else { "printed, not judged: not the declarations' ref= run" },
        if ref_not_judged {
            "NOT JUDGED: no design or rest declaration in scope (a --reference-run needs one): FAILS".to_string()
        } else if referenced.is_empty() {
            "no design or rest declaration in scope".to_string()
        } else if ref_mismatch.is_empty() {
            format!("each of the {} design / rest declaration(s) reproduces its bounds", referenced.len())
        } else {
            format!("MISMATCH {}", ref_mismatch.join("; "))
        }
    );
    // (ironspec 18:39 (4): the rail end's normal-operation bar) every bar row's speed entering the rail
    // end (9,-15,-2), against the start-phase sweep's all-clear figure + 10 % (--rail-end-least; without
    // it, printed only)
    let re: Vec<f64> = rail_end_speeds.iter().flatten().copied().collect();
    let re_missing = rail_end_speeds.iter().filter(|v| v.is_none()).count();
    let re_min = re.iter().copied().fold(f64::INFINITY, f64::min);
    let re_ok = match o.rail_end_least {
        Some(f) => re_missing == 0 && !re.is_empty() && re_min >= f * 1.1,
        None => true,
    };
    out.push(format!(
        "(ride) the speed entering the rail end {:?} over the {} bar row(s): {}{}; {}",
        rail_end_of(o.queue),
        rail_end_speeds.len(),
        if re.is_empty() { "never entered".to_string() } else { format!("{re_min:.4}..{:.4} b/t", re.iter().copied().fold(0.0, f64::max)) },
        if re_missing > 0 { format!(" ({re_missing} row(s) never entered it)") } else { String::new() },
        match o.rail_end_least {
            Some(f) => format!("the bar >= {:.4} (the all-clear figure {f:.4} + 10 %, ironspec 18:39 (4)): {}", f * 1.1, if re_ok { "PASSES" } else { "FAILS" }),
            None => "no all-clear figure given (--rail-end-least): printed, not judged".to_string(),
        }
    ));
    // (G1 14:51 (4); ONE contact rule) the contact verdicts are main's bounded judge's (both axes): the
    // old horizontal-only stale count is printed as information and no longer gates the run
    let ball_ok = ball_line.as_ref().is_none_or(|(_, ok)| *ok);
    if let Some((l, _)) = &ball_line {
        out.push(l.clone());
    }
    let pass = !vacuous && all_ok && td_ok && watch_ok && probe_ok && judge_ok && re_ok && ball_ok && !(fence_min.is_finite() && fence_min <= 0.0);
    out.push(format!(
        "(ride) {} rows ({fluid_n} fluid cell(s) of the chunk given their tick; natural spawning OFF by construction: the bench's world has no natural spawner, mob.natural None, ironspec 14:57): the entry reached in {} of them at t {}..{} after the release, {:.4}..{:.4} b/t heading west (the bar [{}, {}], inherited: ironspec 21:25 from 58f770cc's A6 park sweep){centre_line}{stale_line}{judge_line}; 0 hurt, on rails, never back south; {}; {}; {}; the closest fence box (post and arms as laid) to the cart's box: {}; {td_line}{watch_line}{}: {}",
        bar.len(),
        ts.len(),
        ts.iter().min().copied().unwrap_or(0),
        ts.iter().max().copied().unwrap_or(0),
        ss.iter().cloned().fold(f64::INFINITY, f64::min),
        ss.iter().cloned().fold(0.0, f64::max),
        band[0],
        band[1],
        judged(cart_ticks_n, &format!("{undeclared_contact_ticks} undeclared contact tick(s) (the declared `# contacts:` cells {:?} aside)", o.declared_contacts)),
        judged(steps_n, "game time continuous"),
        judged(rider_ticks_n, "the rider's eye out of the water"),
        if fences.is_empty() { "out of scope (no fence in the way-in's box)".to_string() } else if fence_n == 0 { format!("NOT JUDGED (0 samples, {} fences)", fences.len()) } else { format!("{fence_min:.4} blocks at the least (in the row {fence_min_row}) over {fence_n} samples of {} fences", fences.len()) },
        stress_line,
        if vacuous { "FAIL (a bar not judged)" } else if pass { "PASS" } else { "FAIL" }
    ));
    // (G1's B12 on R4) the rows the judge failed, their text marked "(JUDGE: …)": the count and the first
    // five printed
    let judged_rows: Vec<&String> = bar.iter().map(|r| &r.0).filter(|t| t.contains("(JUDGE: ")).collect();
    if !judged_rows.is_empty() {
        out.push(format!("(ride) {} bar row(s) the judge failed; the first {}:", judged_rows.len(), judged_rows.len().min(5)));
        out.extend(judged_rows.iter().take(5).map(|t| format!("(ride)   {t}")));
    }
    out.extend(evidence);
    Ok((out, pass))
}

/// The queued napper's all-day wait (ironspec 10:18): the rider held on the queue rail from day
/// time `start` for `ticks` gt with its HOME the stop's bed (`bed`, a loaded HOME memory), the
/// post's zombie in place: HOME at every tick, its activity tick by tick, the calm REST ticks
/// (REST active, not PANIC) and their distance to the bed's head, its CANT_REACH stamp's age
/// (the most, and the most at a calm REST tick: the bar 1,000).
pub fn wait_lines(spec: &FarmSpec, o: &Opts, post: P, start: i64, ticks: u32, bed: P, stamp: Option<i64>, follow: u32) -> Result<(Vec<String>, bool), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + follow + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    if !o.custom_post {
        cells.push((BlockPos::new(post[0], post[1] - 1, post[2]), Block::parse("minecraft:glass").map_err(|e| e.to_string())?));
        cells.push((BlockPos::new(post[0], post[1] + 3, post[2]), Block::parse("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]").map_err(|e| e.to_string())?));
    }
    cells.extend(r1_cells(o)?);
    // (G1 (6)) the override rule's before -> after lines kept, not discarded
    let override_lines = place_overrides(&mut run.world, o.extra.len(), &cells)?;
    settle_r1(&mut run.world, o)?;
    // (G1's W7 on R2) the chunk's water given its tick (the cells were laid raw)
    let (fluid_line, fluid_ok) = fluids_line(&mut run.world, o.chunk, "wait");
    // (G1's W6 on R2) the day set BEFORE the summon, the summoned rider's activity asserted on it
    run.world.mob.day_offset = start - run.world.time;
    redsim::entity::spawn(&mut run.world, "minecraft:oak_boat", [post[0] as f64 + 0.5, post[1] as f64, post[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    let nbt = format!(
        "{{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
        bed[0], bed[1], bed[2]
    );
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some(&nbt))?;
    let summon_act = assert_summon_activity(&run.world, cart, start)?;
    let head = Vec3::new(bed[0] as f64 + 0.5, bed[1] as f64 + 0.5625, bed[2] as f64 + 0.5);
    let mut acts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let (mut home_lost, mut left, mut calm_rest, mut stamp_max, mut stamp_calm_max) = (None, None, 0u32, 0i64, 0i64);
    let (mut d_min, mut d_max) = (f64::INFINITY, 0.0f64);
    let mut first_panic = None;
    // (ironspec 14:47) the rider arriving with its stamp armed `stamp` gt before: set at its first
    // exact-brain tick; when it clears; then, with `follow`, the release and the ride on (the first
    // calm REST tick after the release, HOME there, the stamp's age at every calm REST tick)
    let rider = run.world.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart)).map(|e| e.id).ok_or("no rider in the queue cart")?;
    let (mut armed, mut cleared_at) = (stamp.is_none(), None);
    let mut no_exact = 0u32;
    let (mut released_at, mut first_calm_after, mut home_at_first_calm, mut stamp_after_max) = (None::<u32>, None::<u32>, None::<bool>, 0i64);
    let input = if o.south { BlockPos::new(q[0] - 1, q[1], q[2]) } else { BlockPos::new(q[0], q[1], q[2] + 1) };
    for t in 0..ticks + follow {
        if t == ticks && follow > 0 {
            release(&mut run.world, o)?;
            released_at = Some(t);
        }
        run.tick();
        let w = &run.world;
        let Some(i) = w.entities.list.iter().position(|e| !e.removed && e.id == rider) else {
            left = Some(t);
            break;
        };
        if released_at.is_none() && w.entities.list[i].vehicle != Some(cart) && left.is_none() {
            left = Some(t);
        }
        if redsim::entity::mob::ai::brain::brain(w, i).home.is_none() && home_lost.is_none() {
            home_lost = Some((t, w.day_time()));
        }
        if !redsim::entity::mob::ai::brain::has_exact(w, i) {
            // (G1's W12 on R2) a tick without the exact brain is counted, not skipped silently
            no_exact += 1;
            continue;
        }
        if !armed {
            let now = run.world.time;
            redsim::entity::mob::ai::brain::ex_mut(&mut run.world, i).mem.cant_reach_since = Some(now - stamp.unwrap_or(0));
            armed = true;
            continue;
        }
        let w = &run.world;
        let x = redsim::entity::mob::ai::brain::ex(w, i);
        let act = format!("{:?}", x.active);
        let stamp_age = x.mem.cant_reach_since.map_or(0, |s| w.time - s);
        if stamp.is_some() && cleared_at.is_none() && x.mem.cant_reach_since.is_none() {
            cleared_at = Some(t);
        }
        if released_at.is_some() {
            if act.contains("Rest") {
                stamp_after_max = stamp_after_max.max(stamp_age);
                if first_calm_after.is_none() {
                    first_calm_after = Some(t - ticks);
                    home_at_first_calm = Some(redsim::entity::mob::ai::brain::brain(w, i).home.is_some());
                }
            }
            continue;
        }
        stamp_max = stamp_max.max(stamp_age);
        if act.contains("Panic") && first_panic.is_none() {
            first_panic = Some(t);
        }
        if act.contains("Rest") {
            calm_rest += 1;
            stamp_calm_max = stamp_calm_max.max(stamp_age);
            let d = w.entities.list[i].pos.distance_to_sqr(head).sqrt();
            d_min = d_min.min(d);
            d_max = d_max.max(d);
        }
        *acts.entry(act).or_default() += 1;
    }
    // (G1's W3 on R2) the wait's verdict: calm REST ticks judged (> 0), the stamp bar at them, an armed
    // stamp cleared within 100 gt, the fluids scheduled
    let stamp_ok = calm_rest > 0 && stamp_calm_max <= 1000;
    let cleared_ok = stamp.is_none() || cleared_at.is_some_and(|t| t <= 100);
    let pass = stamp_ok && cleared_ok && fluid_ok;
    let mut out = vec![
        fluid_line,
        format!("(wait) the summoned rider's activity at the summon (the day set first): {summon_act}"),
        format!(
            "(wait) the rider held on the queue rail from day time {start} for {ticks} gt (its HOME the bed at {bed:?}{}): HOME {}; the rider {}; first PANIC at t {first_panic:?}; {no_exact} tick(s) without the exact brain (counted, not judged)",
            stamp.map_or(String::new(), |a| format!(", arriving with its stamp armed {a} gt before")),
            match home_lost {
                None => "kept at every tick".to_string(),
                Some((t, d)) => format!("LOST at t {t} (day time {d})"),
            },
            match left {
                None => "seated throughout".to_string(),
                Some(t) => format!("OUT of its cart at t {t}"),
            }
        ),
        format!("(wait) activity ticks: {acts:?}"),
        format!(
            "(wait) calm REST ticks {calm_rest}{}; the stamp's age: most {stamp_max}, most at a calm REST tick {stamp_calm_max} (bar 1,000, inherited: the 1,200 tooLongUnreachable with margin: {})",
            if calm_rest > 0 { format!(" at {d_min:.2}..{d_max:.2} from the bed's head (inside 16: the 16-block rule)") } else { String::new() },
            if calm_rest == 0 { "NOT JUDGED (0 calm REST ticks): FAIL" } else if stamp_calm_max <= 1000 { "PASS" } else { "FAIL" }
        ),
    ];
    if stamp.is_some() {
        out.push(format!("(wait) the armed stamp cleared at t {cleared_at:?} ({})", match cleared_at {
            Some(t) if t <= 100 => "within 100 gt of the arrival (inherited: ironspec 14:47): PASS".to_string(),
            Some(_) => "LATER than 100 gt (inherited: ironspec 14:47): FAIL".to_string(),
            None => "NEVER cleared: FAIL".to_string(),
        }));
    }
    if follow > 0 {
        out.push(format!(
            "(wait) released at t {released_at:?} and followed {follow} gt: the first calm REST tick {} after the release, HOME there {}; the stamp's age at the calm REST ticks after the release at most {stamp_after_max}; HOME {}",
            first_calm_after.map_or("none".to_string(), |t| format!("{t} gt")),
            home_at_first_calm.map_or("-".to_string(), |h| if h { "kept".to_string() } else { "LOST".to_string() }),
            match home_lost {
                None => "kept to the end".to_string(),
                Some((t, _)) => format!("LOST at t {t} ({} gt after the release)", t as i64 - ticks as i64),
            }
        ));
    }
    // (G1 (6)) the override rule's before -> after lines kept
    out.push(format!("(wait) the verdict (calm REST ticks judged, the stamp bar, an armed stamp cleared within 100 gt, the water scheduled): {}", if pass { "PASS" } else { "FAIL" }));
    Ok((override_lines.into_iter().chain(out).collect(), pass))
}

/// (ironspec 14:58 (2)) The queue's release at every phase: the rider waits `ticks` gt (from day
/// time `start`, its HOME the bed, its stamp armed `stamp` gt before its arrival when given), then
/// for k in 0..`phases` the run is cloned, held k gt more and released (the redstone block at the
/// input cell), and followed up to `follow` gt to the stop's entry: whether a walk target was
/// pending at the release (a draw in MoveToTargetSink, G1's case: pathed later from the drop), the
/// stamp's age at the release and at the entry, whether it armed after the release, HOME at the
/// entry. Then the share of releases with a draw pending, the stamps at the entry, the armings.
#[allow(clippy::too_many_arguments)]
pub fn sweep_lines(spec: &FarmSpec, o: &Opts, post: P, start: i64, ticks: u32, bed: P, stamp: Option<i64>, phases: u32, follow: u32, entry: P, restart_ticks: &[i64]) -> Result<(Vec<String>, bool), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + phases + follow + 10, WalkMode::Random) };
    let (mut base, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    if !o.custom_post {
        cells.push((BlockPos::new(post[0], post[1] - 1, post[2]), Block::parse("minecraft:glass").map_err(|e| e.to_string())?));
        cells.push((BlockPos::new(post[0], post[1] + 3, post[2]), Block::parse("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]").map_err(|e| e.to_string())?));
    }
    cells.extend(r1_cells(o)?);
    let override_lines = place_overrides(&mut base.world, o.extra.len(), &cells)?;
    settle_r1(&mut base.world, o)?;
    // (G1's W7 on R2) the chunk's water given its tick (the cells were laid raw)
    let (fluid_line, fluid_ok) = fluids_line(&mut base.world, o.chunk, "sweep");
    // (G1's W6 on R2) the day set BEFORE the summon, the summoned rider's activity asserted on it
    base.world.mob.day_offset = start - base.world.time;
    redsim::entity::spawn(&mut base.world, "minecraft:oak_boat", [post[0] as f64 + 0.5, post[1] as f64, post[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    let nbt = format!(
        "{{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
        bed[0], bed[1], bed[2]
    );
    let cart0 = redsim::entity::spawn(&mut base.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some(&nbt))?;
    let summon_act = assert_summon_activity(&base.world, cart0, start)?;
    let rider = base.world.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart0)).map(|e| e.id).ok_or("no rider in the queue cart")?;
    let idx = |w: &redsim::world::World, id: u32| w.entities.list.iter().position(|e| !e.removed && e.id == id);
    let mut armed = stamp.is_none();
    // (ironspec 15:40) the PANIC draw ticks before the release: a panicking rider with no walk
    // target draws once a tick (the own cell with p 0.0761: P(still armed) = 0.9239^n)
    let draw_tick = |w: &redsim::world::World, id: u32| -> bool {
        let Some(i) = w.entities.list.iter().position(|e| !e.removed && e.id == id) else { return false };
        redsim::entity::mob::ai::brain::has_exact(w, i) && {
            let x = redsim::entity::mob::ai::brain::ex(w, i);
            format!("{:?}", x.active).contains("Panic") && x.mem.walk_target.is_none()
        }
    };
    let mut n_base = 0u32;
    // (ironspec 16:03) how the arrival stamp cleared: the tick, and the rider's activity then (a
    // calm REST tick's HOME walk, or a PANIC tick's own-cell draw)
    let mut cleared: Option<(u32, String)> = None;
    let mut was_armed = false;
    for t in 0..ticks {
        base.tick();
        n_base += draw_tick(&base.world, rider) as u32;
        if armed && cleared.is_none() {
            if let Some(i) = idx(&base.world, rider) {
                if redsim::entity::mob::ai::brain::has_exact(&base.world, i) {
                    let x = redsim::entity::mob::ai::brain::ex(&base.world, i);
                    if x.mem.cant_reach_since.is_some() {
                        was_armed = true;
                    } else if was_armed {
                        cleared = Some((t, format!("{:?}", x.active)));
                    }
                }
            }
        }
        if !armed {
            if let Some(i) = idx(&base.world, rider) {
                if redsim::entity::mob::ai::brain::has_exact(&base.world, i) {
                    let now = base.world.time;
                    redsim::entity::mob::ai::brain::ex_mut(&mut base.world, i).mem.cant_reach_since = Some(now - stamp.unwrap_or(0));
                    armed = true;
                }
            }
        }
    }
    let input = if o.south { BlockPos::new(q[0] - 1, q[1], q[2]) } else { BlockPos::new(q[0], q[1], q[2] + 1) };
    let age = |w: &redsim::world::World, i: usize| -> Option<i64> {
        if !redsim::entity::mob::ai::brain::has_exact(w, i) {
            return None;
        }
        redsim::entity::mob::ai::brain::ex(w, i).mem.cant_reach_since.map(|s| w.time - s)
    };
    let mut out = vec![format!(
        "(sweep) the arrival stamp{}: {}",
        stamp.map_or(" (none armed)".to_string(), |a| format!(" ({a} gt at the arrival)")),
        match &cleared {
            Some((t, act)) => format!(
                "cleared at t {t} after the rider settled, {}",
                if act.contains("Panic") { format!("on a PANIC tick (an own-cell draw; {act})") } else { format!("on a calm tick ({act}: a complete HOME walk from the seat)") }
            ),
            None if stamp.is_some() => "NOT cleared during the wait".to_string(),
            None => "-".to_string(),
        }
    )];
    let (mut pending_n, mut armed_after_n, mut entry_armed_n, mut home_lost_n, mut reached_n) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut entry_ages: Vec<i64> = Vec::new();
    let mut sources: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    // every release phase, plain and with a save/stop/start at each restart tick in every order
    let mut rows: Vec<(u32, Option<(i64, crate::liveness::Restart)>)> = Vec::new();
    for k in 0..phases {
        rows.push((k, None));
        for &rt in restart_ticks {
            for how in crate::liveness::Restart::rows() {
                rows.push((k, Some((rt, how))));
            }
        }
    }
    let wt_of = |w: &redsim::world::World, i: usize| -> Option<redsim::entity::mob::ai::brain::WalkTarget> {
        if !redsim::entity::mob::ai::brain::has_exact(w, i) {
            return None;
        }
        redsim::entity::mob::ai::brain::ex(w, i).mem.walk_target
    };
    let n_rows = rows.len();
    for (k, restart) in rows {
        let mut r = base.clone();
        let mut n_draws = n_base;
        for _ in 0..k {
            r.tick();
            n_draws += draw_tick(&r.world, rider) as u32;
        }
        let mut rider = rider;
        let i = idx(&r.world, rider).ok_or("the rider gone before the release")?;
        let wt_rel = wt_of(&r.world, i);
        let pending = wt_rel.is_some();
        let since_rel = if redsim::entity::mob::ai::brain::has_exact(&r.world, i) { redsim::entity::mob::ai::brain::ex(&r.world, i).mem.cant_reach_since } else { None };
        let age_rel = age(&r.world, i);
        release(&mut r.world, o)?;
        let (mut armed_after, mut at_entry, mut age_entry, mut home_entry) = (None::<(u32, String)>, None::<u32>, None::<Option<i64>>, None::<bool>);
        let mut prev_wt = wt_rel;
        // (ironspec 15:45: redsim 96% vs the game 37%) the first calm REST tick after the release:
        // the rider's position, falling or not, and the HOME path from there (its start node, end
        // node, complete or partial); and the same path at the arming tick
        let mut first_calm: Option<String> = None;
        let mut arm_path: Option<String> = None;
        let home_probe = |r: &mut Run, i: usize| -> String {
            let e = &r.world.entities.list[i];
            let (p, v) = (e.pos, e.delta_movement);
            let vehicle = e.vehicle;
            let cart_rail = vehicle.and_then(|c| r.world.entities.list.iter().find(|x| x.id == c)).is_some_and(|c| {
                let cc = BlockPos::new(c.pos.x.floor() as i32, c.pos.y.floor() as i32, c.pos.z.floor() as i32);
                redsim::rails::is_rail(&r.world.get(cc)) || redsim::rails::is_rail(&r.world.get(BlockPos::new(cc.x, cc.y - 1, cc.z)))
            });
            let cart_vy = vehicle.and_then(|c| r.world.entities.list.iter().find(|x| x.id == c)).map_or(v.y, |c| c.delta_movement.y);
            let bed_p = BlockPos::new(bed[0], bed[1], bed[2]);
            let res = redsim::entity::mob::ai::with_nav(&mut r.world, i, |w, nm, nav, arena| {
                let mut copy = nav.clone();
                copy.create_path_pos(w, nm, bed_p, 0, arena).map(|pa| (pa.reached, pa.nodes.first().map(|n| (n.x, n.y, n.z)), pa.nodes.last().map(|n| (n.x, n.y, n.z)), pa.nodes.len()))
            });
            format!(
                "at ({:.2},{:.2},{:.2}) {}; the HOME path {}",
                p.x,
                p.y,
                p.z,
                if cart_rail { "on the rails".to_string() } else { format!("FALLING (vy {cart_vy:.3})") },
                match res {
                    Some((reached, s, e, n)) => format!("start {s:?}, end {e:?}, {} ({n} nodes)", if reached { "COMPLETE" } else { "partial" }),
                    None => "none".to_string(),
                }
            )
        };
        for t in 0..follow {
            if let Some((rt, how)) = restart {
                if t as i64 == rt {
                    let map = crate::liveness::restart(&mut r.world, how)?;
                    rider = *map.get(&rider).ok_or("the rider lost in the restart")?;
                }
            }
            r.tick();
            let Some(i) = idx(&r.world, rider) else { break };
            if redsim::entity::mob::ai::brain::has_exact(&r.world, i) {
                if first_calm.is_none() && format!("{:?}", redsim::entity::mob::ai::brain::ex(&r.world, i).active).contains("Rest") {
                    let a = age(&r.world, i);
                    first_calm = Some(format!("+{t} gt {} (the stamp then {})", home_probe(&mut r, i), a.map_or("none".to_string(), |a| format!("{a} gt"))));
                }
                let s = redsim::entity::mob::ai::brain::ex(&r.world, i).mem.cant_reach_since;
                if armed_after.is_none() && s.is_some() && s != since_rel {
                    // the arming's source: the walk target pathed (the one before this tick if the
                    // sink erased it), told apart: a seat draw pending at the release, a draw of
                    // the seat cell after it, a HOME walk (the bed), or another target
                    let wt = wt_of(&r.world, i).or(prev_wt);
                    let target = wt.map(|w| w.target);
                    let seat = redsim::entity::mob::ai::brain::Tracker::Block(BlockPos::new(q[0], q[1], q[2]));
                    let bed_t = redsim::entity::mob::ai::brain::Tracker::Block(BlockPos::new(bed[0], bed[1], bed[2]));
                    let src = match target {
                        Some(tg) if wt == wt_rel && pending => format!("a draw pending at the release ({tg:?})"),
                        Some(tg) if tg == seat => "the seat cell drawn after the release (the departure's tail)".to_string(),
                        Some(tg) if tg == bed_t => "a calm REST HOME walk from the cart".to_string(),
                        Some(tg) => format!("another target {tg:?}"),
                        None => "no walk target seen".to_string(),
                    };
                    arm_path = Some(home_probe(&mut r, i));
                    armed_after = Some((t, src));
                }
                prev_wt = wt_of(&r.world, i).or(prev_wt);
            }
            let p = r.world.entities.list[i].pos;
            let c = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
            if at_entry.is_none() && c == entry {
                at_entry = Some(t);
                age_entry = Some(age(&r.world, i));
                home_entry = Some(redsim::entity::mob::ai::brain::brain(&r.world, i).home.is_some());
                break;
            }
        }
        pending_n += pending as u32;
        if let Some((_, s)) = &armed_after {
            armed_after_n += 1;
            *sources.entry(s.split(" (").next().unwrap_or(s).to_string()).or_default() += 1;
        }
        reached_n += at_entry.is_some() as u32;
        if let Some(Some(a)) = age_entry {
            entry_armed_n += 1;
            entry_ages.push(a);
        }
        home_lost_n += (home_entry == Some(false)) as u32;
        if restart.is_none() || armed_after.is_some() || home_entry != Some(true) {
            out.push(format!(
                "(sweep) release +{k}{}: PANIC draw ticks before it {n_draws} (P(armed) = 0.9239^n = {:.2e}); a draw pending {}; the stamp at the release {}; the first calm REST tick after it {}; armed after it {}{}; at the entry {}: the stamp {}, HOME {}",
                restart.map_or(String::new(), |(rt, how)| format!(", restart at +{rt} ({})", how.label())),
                0.9239f64.powi(n_draws as i32),
                wt_rel.map_or("no".to_string(), |w| format!("YES ({:?})", w.target)),
                age_rel.map_or("none".to_string(), |a| format!("{a} gt")),
                first_calm.as_deref().unwrap_or("none"),
                armed_after.as_ref().map_or("no".to_string(), |(t, s)| format!("at +{t} gt by {s}")),
                arm_path.as_ref().map_or(String::new(), |p| format!(" (there: {p})")),
                at_entry.map_or("NEVER".to_string(), |t| format!("t {t}")),
                age_entry.map_or("-".to_string(), |s| s.map_or("none".to_string(), |a| format!("{a} gt"))),
                home_entry.map_or("-".to_string(), |h| if h { "kept".to_string() } else { "LOST".to_string() })
            ));
        }
    }
    out.push(format!(
        "(sweep) {n_rows} releases ({phases} phases{}) after a {ticks}-gt wait from day time {start}{}: a draw pending at {pending_n} ({:.1}%); the stamp armed after the release in {armed_after_n} (sources {sources:?}); at the entry {reached_n}, with an armed stamp {entry_armed_n} (ages {:?}), HOME lost {home_lost_n}: {}",
        if restart_ticks.is_empty() { String::new() } else { format!(" x (plain + restarts at {restart_ticks:?} in 10 orders)") },
        stamp.map_or(String::new(), |a| format!(" (arrival stamp {a} gt)")),
        100.0 * pending_n as f64 / n_rows.max(1) as f64,
        entry_ages,
        if armed_after_n == 0 && home_lost_n == 0 && reached_n as usize == n_rows { "PASS (0 armings, HOME kept)" } else { "FAIL" }
    ));
    // (G1 07:00 / 08:12) the sweep's verdict returned beside its lines (the exit status); (W7) the water
    // scheduled
    let pass = armed_after_n == 0 && home_lost_n == 0 && reached_n as usize == n_rows && n_rows > 0 && fluid_ok;
    // (G1 (6)) the override rule's before -> after lines kept; (W6 / W7) the summon's activity and the
    // fluids first
    let head = vec![fluid_line, format!("(sweep) the summoned rider's activity at the summon (the day set first): {summon_act}")];
    Ok((override_lines.into_iter().chain(head).chain(out).collect(), pass))
}

/// (ironspec 16:37: the held partial runs) A whole visit, from the split row's end to A6's entry:
/// the rider (HOME the bed) rides the leg and the shaft from day time `day` (REST), settles on the
/// queue, waits `hold` gt from the arrival detector, is released (the torch at the input cell) and
/// rides the descent to the entry; with `post` the queue's zombie is in place, without it the
/// post is absent (the (C') hatch shut at night, or (C'')). Over the arrival phases (the cart
/// spawned k gt later), the holds and, when given, a save/stop/start at each restart tick (from
/// the arrival detector) in every order, per row: the stamp at the arrival, the tick the walk target
/// held from the leg or the shaft is dropped (the held run's end), the tick the stamp clears (and on
/// what tick: calm or PANIC), whether that is before the release, the armings between the release
/// and the entry with their walk targets, the first calm REST tick after the release with its HOME
/// path, the stamp and HOME at the entry.
#[allow(clippy::too_many_arguments)]
/// (ironspec 05:20, the bench rule; G1 07:00: no silent skip) the summoned rider of `cart` asserted
/// on its fresh brain: found, with an exact brain, and in the activity the schedule gives `day`:
/// REST at night (day time 12,000..24,000), not REST by day (IDLE / WORK / MEET by the villager's
/// schedule; a jobless villager cannot enter WORK and stays IDLE). Returns the activity printed.
fn assert_summon_activity(w: &redsim::world::World, cart: u32, day: i64) -> Result<String, String> {
    let ri = w.entities.list.iter().position(|e| !e.removed && e.vehicle == Some(cart)).ok_or_else(|| format!("the bench rule: the summoned rider of cart {cart} not found"))?;
    if !redsim::entity::mob::ai::brain::has_exact(w, ri) {
        return Err(format!("the bench rule: the summoned rider #{} has no exact brain (its activity cannot be asserted)", w.entities.list[ri].id));
    }
    let act = format!("{:?}", redsim::entity::mob::ai::brain::ex(w, ri).active);
    let dt = day.rem_euclid(24000);
    let night = dt >= 12000;
    if night != act.contains("Rest") {
        return Err(format!(
            "the bench rule (ironspec 05:20): the summoned napper's activity is {act} at day time {dt} ({}): the day was not set before the summon",
            if night { "night: REST expected" } else { "day: IDLE / WORK / MEET expected" }
        ));
    }
    Ok(act)
}

/// (G1 (14)) the napper's bed broken in the world (both halves to air, the game's block updates):
/// FARM-STOP's "lay down, then no HOME" positive. An error when no bed head stands at `head`.
fn break_bed(w: &mut redsim::world::World, head: BlockPos) -> Result<(), String> {
    if !matches!(w.get(head), Block::Bed { head: true, .. }) {
        return Err(format!("--visit-break-bed: no bed head at {head}"));
    }
    let foot = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dz)| BlockPos::new(head.x + dx, head.y, head.z + dz))
        .find(|p| matches!(w.get(*p), Block::Bed { head: false, .. }));
    let air = Block::parse("minecraft:air").map_err(|e| format!("air: {e}"))?;
    if let Some(f) = foot {
        w.set_block(f, air.clone(), redsim::world::flags::ALL);
    }
    w.set_block(head, air, redsim::world::flags::ALL);
    Ok(())
}

/// (ironspec 08:13) the runs of SetWalkTargetFromBlockMemory toward HOME by the villagers `ids` since
/// the last call, drained from redsim's record. The record must be on (armed at a row's start; a world
/// open keeps it: the_record_survives_a_world_open): found off, a tick's runs would go unseen, so that is
/// an error.
pub(crate) fn drain_home_walk_runs(w: &mut redsim::world::World, ids: &[u32]) -> Result<Vec<redsim::entity::mob::ai::WalkMemoryRun>, String> {
    let log = w.mob.ai.walk_memory_runs.as_mut().ok_or("the record of SetWalkTargetFromBlockMemory's runs is off (never armed for this row?)")?;
    Ok(log.drain(..).filter(|r| r.mem == redsim::entity::mob::ai::brain::M::Home && ids.contains(&r.id)).collect())
}

/// (ironspec 08:13; 09:13 (a), G1 09:15) the visit rows' tally of SetWalkTargetFromBlockMemory's runs
/// toward the napper's HOME over every row: samples, failing runs (the age seen >= 1,000), the largest
/// age seen, the branch counts, the first failing places.
#[derive(Default)]
pub(crate) struct RecordTally {
    pub samples: u32,
    pub fails: u32,
    pub max: i64,
    pub branches: std::collections::BTreeMap<String, u32>,
    pub fail_where: Vec<String>,
}

/// One row's runs: their game ticks and branches, and its first failing run (tick, age seen).
#[derive(Default)]
pub(crate) struct RowRecord {
    pub ticks: Vec<i64>,
    pub branches: Vec<String>,
    pub first_fail: Option<(i64, i64)>,
}

impl RecordTally {
    /// One tick's share: the record drained ([`drain_home_walk_runs`]) and every run toward HOME by
    /// `ids` tallied. The bench calls it right after each tick, BEFORE anything that can cut the tick's
    /// processing short, so no run is left uncounted.
    pub(crate) fn take(&mut self, w: &mut redsim::world::World, ids: &[u32], row: &mut RowRecord, label: &str) -> Result<(), String> {
        for run in drain_home_walk_runs(w, ids).map_err(|e| format!("{label}: {e}"))? {
            self.samples += 1;
            row.ticks.push(run.t);
            row.branches.push(format!("{:?}", run.branch));
            *self.branches.entry(format!("{:?}", run.branch)).or_default() += 1;
            if let Some(a) = run.stamp_age_seen {
                self.max = self.max.max(a);
                if a >= 1000 {
                    self.fails += 1;
                    if row.first_fail.is_none() {
                        row.first_fail = Some((run.t, a));
                    }
                    if self.fail_where.len() < 8 {
                        self.fail_where.push(format!("{label} game time {}: {a} gt ({:?})", run.t, run.branch));
                    }
                }
            }
        }
        Ok(())
    }
}

/// (G1 / ironspec 09:13 (a), 09:15) the record's log at a row's end: EMPTY (every run drained and
/// tallied on its own tick), else an error naming the runs left; the record off is an error too.
pub(crate) fn record_left_empty(w: &redsim::world::World, label: &str) -> Result<(), String> {
    match w.mob.ai.walk_memory_runs.as_ref() {
        None => Err(format!("{label}: the record of SetWalkTargetFromBlockMemory's runs is OFF at the row's end")),
        Some(log) if !log.is_empty() => Err(format!("{label}: {} run(s) of SetWalkTargetFromBlockMemory left in the record at the row's end (first at game time {}): never counted", log.len(), log[0].t)),
        Some(_) => Ok(()),
    }
}

/// (G1 08:12; the refinement ruled by ironspec 11:46 / 11:48, G1 11:44) one tick's post-tick state as
/// the visit rows' proxy reads it: a walk target held, the CANT_REACH stamp, the REST activity, and
/// MoveToTargetSink running.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ProxyObs {
    pub wt: bool,
    pub stamp: Option<i64>,
    pub rest: bool,
    pub sink_running: bool,
}

/// A proxy sample: a tick SetWalkTargetFromBlockMemory ran, as the post-tick states show it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ProxySample {
    /// A REST tick after a tick with no walk target: judged on the previous tick's stamp (its age at
    /// this tick), none when it was clear.
    AfterNoTarget(Option<i64>),
    /// The erase signature (ironspec 11:46; traced 12:42): the walk target held at the previous tick's
    /// end is gone at this tick's, the stamp went from set to clear in this tick, and MoveToTargetSink
    /// was idle at BOTH ends: the brain's start pass ran its start check, which found the target
    /// reached and erased both before SetWalkTargetFromBlockMemory's own start in the same pass;
    /// judged on the stamp the run saw, none. (A sink RUNNING at the previous tick's end that stops
    /// this tick erases the same memories in the later tick pass, after the start pass: the run comes
    /// the next tick and the old rule samples it.)
    EraseSignature,
}

/// The proxy's verdict for the tick at game time `t` (`prev`: the previous tick's post-tick state;
/// `home_prev`: HOME held before this tick). The old rule (after no target) is checked first and
/// unchanged; the erase signature only ADDS samples (G1 11:44 (3)).
pub(crate) fn proxy_sample(prev: &ProxyObs, home_prev: bool, now: &ProxyObs, t: i64) -> Option<ProxySample> {
    if !now.rest || !home_prev {
        return None;
    }
    if !prev.wt {
        return Some(ProxySample::AfterNoTarget(prev.stamp.map(|s| t - s)));
    }
    if prev.wt && !prev.sink_running && !now.wt && prev.stamp.is_some() && now.stamp.is_none() && !now.sink_running {
        return Some(ProxySample::EraseSignature);
    }
    None
}

/// The cross-check of one row (ironspec 08:15): the record's run ticks the proxy missed and the
/// proxy's sample ticks with no run; any of either fails the row.
pub(crate) fn proxy_mismatch(record: &[i64], proxy: &[i64]) -> (Vec<i64>, Vec<i64>) {
    let missed: Vec<i64> = record.iter().filter(|t| !proxy.contains(t)).copied().collect();
    let spurious: Vec<i64> = proxy.iter().filter(|t| !record.contains(t)).copied().collect();
    (missed, spurious)
}

/// The seat's HOME path's nodes, read from [`path_probe_lines`]'s probe line (`... nodes): [(x, y,
/// z), ...]`); none when the line carries no node list.
fn seat_path_nodes(seat: &[String]) -> Vec<[i32; 3]> {
    seat.iter()
        .find_map(|l| l.split("nodes): [").nth(1))
        .map(|s| {
            s.trim_end_matches(']')
                .split("), (")
                .filter_map(|n| {
                    let v: Vec<i32> = n.trim_matches(|c| c == '(' || c == ')').split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    (v.len() == 3).then(|| [v[0], v[1], v[2]])
                })
                .collect()
        })
        .unwrap_or_default()
}

/// (ironspec 01:46 (1)(iii); G1 07:16 (3): a bar, not a print) the seat's HOME path runs FORWARD: its
/// second node lower than the seat's, or further south (the ride's direction); fewer than two nodes is
/// not forward (judged FAIL, never a vacuous pass).
fn seat_path_forward(nodes: &[[i32; 3]]) -> bool {
    nodes.len() >= 2 && (nodes[1][1] < nodes[0][1] || nodes[1][2] > nodes[0][2])
}

/// (G1 07:16 (4)) the water cells of a built world (sources, flows, waterlogged blocks) over its whole
/// box: the one definition of a DRY layout (0).
fn world_water_cells(w: &redsim::world::World) -> usize {
    let (min, size) = (w.min, w.size);
    let mut n = 0;
    for x in min.x..min.x + size[0] {
        for y in min.y..min.y + size[1] {
            for z in min.z..min.z + size[2] {
                n += redsim::fluid::fluid_state(w, BlockPos::new(x, y, z)).fluid.is_water() as usize;
            }
        }
    }
    n
}

/// (ironspec 06:31 (2)) a bed's ticket line: the POI's free tickets, its occupied flag and every
/// villager whose HOME memory names the bed (a HOME loaded from NBT takes no ticket in the game).
fn bed_line(w: &mut redsim::world::World, bp: BlockPos) -> String {
    let occ = matches!(w.get(bp), Block::Bed { occupied: true, .. });
    let free = redsim::entity::mob::ai::poi::free_tickets(w, bp);
    let holders: Vec<u32> = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .filter_map(|e| match &e.kind {
            redsim::entity::EntityKind::Mob(m) => m.brain.as_deref().filter(|b| b.home == Some(bp)).map(|_| e.id),
            _ => None,
        })
        .collect();
    format!("the bed {bp}: free tickets {}, occupied {occ}, HOME memory holders {holders:?} (ticket state uncertain: pre-fix redsim)", free.map_or("-".to_string(), |f| f.to_string()))
}

pub fn visit_lines(spec: &FarmSpec, o: &Opts, post: Option<P>, start: [f64; 3], motion: [f64; 3], bed: P, day: i64, phases: u32, holds: &[u32], restart_ticks: &[i64], entry: P, follow: u32) -> Result<(Vec<String>, bool), String> {
    // (ironspec 22:10 (2)) the visit followed past A6's entry this many gt (the held walk's clear,
    // the sleep, HOME, armings, how far the napper gets)
    let follow = if follow == 0 { 600 } else { follow };
    let max_hold = holds.iter().copied().max().unwrap_or(80);
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, phases + 900 + max_hold + follow, WalkMode::Random) };
    let (mut base, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o)?);
    let override_lines = place_overrides(&mut base.world, o.extra.len(), &cells)?;
    settle_r1(&mut base.world, o)?;
    // (G1's W7 on R2) the chunk's water given its tick (the cells were laid raw): in the verdict
    let (fluid_line, fluid_ok) = fluids_line(&mut base.world, o.chunk, "visit");
    if let Some(p) = post {
        redsim::entity::spawn(&mut base.world, "minecraft:oak_boat", [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    }
    let q = o.queue;
    let arrival = if o.south { [q[0], q[1] + 1, q[2] - 1] } else { [q[0] + 1, q[1] + 1, q[2]] };
    let rider_nbt = format!(
        "{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}",
        bed[0], bed[1], bed[2]
    );
    let idx = |w: &redsim::world::World, id: u32| w.entities.list.iter().position(|e| !e.removed && e.id == id);
    let mem = |w: &redsim::world::World, i: usize| -> Option<(Option<i64>, bool, String)> {
        if !redsim::entity::mob::ai::brain::has_exact(w, i) {
            return None;
        }
        let x = redsim::entity::mob::ai::brain::ex(w, i);
        Some((x.mem.cant_reach_since, x.mem.walk_target.is_some(), format!("{:?}", x.active)))
    };
    let bed_p = BlockPos::new(bed[0], bed[1], bed[2]);
    let probe = |r: &mut Run, i: usize| -> String {
        let p = r.world.entities.list[i].pos;
        let res = redsim::entity::mob::ai::with_nav(&mut r.world, i, |w, nm, nav, arena| {
            let mut copy = nav.clone();
            copy.create_path_pos(w, nm, bed_p, 0, arena).map(|pa| (pa.reached, pa.nodes.last().map(|n| (n.x, n.y, n.z))))
        });
        format!("at ({:.1},{:.1},{:.1}) the HOME path {}", p.x, p.y, p.z, match res {
            Some((true, _)) => "COMPLETE".to_string(),
            Some((false, e)) => format!("partial (ends {e:?})"),
            None => "none".to_string(),
        })
    };
    let mut out = override_lines;
    if let Some(b) = o.bystander {
        out.push(format!("(overrides) a NoAI villager (persistent) spawned at ({:.2},{:.2},{:.2}) at every visit row's start (the positive control's look target)", b[0], b[1], b[2]));
    }
    if let Some(age) = o.visit_plant_stamp {
        out.push(format!("(overrides) every row's CANT_REACH stamp PLANTED at the release, {age} gt old (the HOME RELEASED count's known positive: kept long enough it must release HOME)"));
    }
    if o.visit_plant_erase {
        out.push(format!("(overrides) a second villager asleep in the napper's bed ({},{},{}) from every row's start (the HOME LOST bar's known positive, in the world: within 16 of the occupied bed the napper's own ValidateNearbyPoi erases its HOME (a villager sleeping there: no ticket released), and the OCCUPIED bed blocks any re-claim (AcquirePoi takes only a bed nobody sleeps in), as the bed lines show)", bed[0], bed[1], bed[2]));
    }
    if o.visit_break_bed {
        out.push(format!("(overrides) the napper's bed ({},{},{}) BROKEN (both halves to air) the tick the napper lies down (FARM-STOP's 'lay down, then without its bed as HOME' positive: it wakes, and its HOME's POI gone, ValidateNearbyPoi erases HOME)", bed[0], bed[1], bed[2]));
    }
    if o.visit_clear_stamp {
        out.push("(overrides) every row's CANT_REACH stamp ERASED at the release (the would-arm class's positive control)".to_string());
    }
    let (mut rows_n, mut before_n, mut armed_after_n, mut home_lost_n, mut entry_stamped_n) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let (mut over_1000_n, mut not_cleared_n, mut escaped_n) = (0u32, 0u32, 0u32);
    let (mut rest_lo, mut rest_hi, mut no_rest_n) = (f64::INFINITY, f64::NEG_INFINITY, 0u32);
    // (ironspec 23:56: no vacuous passes) the held walk's samples: rows with a stamp at any tick
    // ((i)), rows followed into A6 ((ii) is judged on the stamped-at-entry ones, (iii) and (v) on
    // all of them)
    let (mut i_samples, mut followed_n) = (0u32, 0u32);
    // (G1 16:06 / ironspec 16:24) the riders' stamp arms and clears by cause over every row, the rows
    // whose walk-target path completed at some tick, and the oldest stamp
    let mut stamp_arms: std::collections::BTreeMap<&'static str, u32> = std::collections::BTreeMap::new();
    let mut stamp_clears: std::collections::BTreeMap<&'static str, u32> = std::collections::BTreeMap::new();
    let (mut rows_path_complete, mut oldest_stamp) = (0u32, 0i64);
    // (R6; G1 22:11) the rows whose napper lost health (a landing passing the cart's fall on), the rows
    // whose health was read, and the least health seen
    let (mut rows_damaged, mut rows_health_read, mut health_least) = (0u32, 0u32, f32::INFINITY);
    // rows with a seat panic, and the latest sleep after the entry (a stall shows as a late sleep)
    let (mut seat_panic_rows, mut sleep_latest, mut never_slept) = (0u32, 0u32, 0u32);
    // path computations after the release: in the descent (before A6's entry) and partial there
    let (mut descent_paths_n, mut descent_partial_n, mut partial_where): (u32, u32, Vec<String>) = (0, 0, Vec::new());
    // (farmopt 04:11) rows with a partial descent recompute: the stamp cleared at the release (the
    // "would arm" class, or erased by the control) / still armed (a partial recompute keeps it)
    let (mut would_arm_n, mut partial_stamped_n) = (0u32, 0u32);
    // rows whose napper released HOME (a Released event), even when it re-acquired a bed at once
    let mut home_released_n = 0u32;
    // (ironspec 06:31 (1)) rows with the napper without HOME at any tick at the bed stop
    let mut farm_stop_n = 0u32;
    // (G1 (2), ironspec 07:16) the REST-entry stamp bar: the ticks SetWalkTargetFromBlockMemory ran,
    // the failing ones (stamp >= 1,000), the largest age then
    let (mut swt_samples, mut swt_fails, mut swt_max, mut swt_fail_where): (u32, u32, i64, Vec<String>) = (0, 0, 0, Vec::new());
    // (ironspec 11:46 / 11:48) the proxy's samples at the erase signature (judged on none)
    let mut swt_erase_n = 0u32;
    // (ironspec 08:13: THE RULE) the at-entry bar sampled from redsim's RECORD of every run of
    // SetWalkTargetFromBlockMemory toward HOME (its tick, its branch, the stamp age it SAW); the proxy
    // above (a REST tick after a tick with no walk target, the previous tick's stamp) printed beside it
    let mut tally = RecordTally::default();
    let (mut proxy_rows, mut proxy_mismatch_rows, mut proxy_mismatch_where): (u32, u32, Vec<String>) = (0, 0, Vec::new());
    // (G1 (1)) the rows that reached A6 (HOME LOST and FARM-STOP's samples); (G1 (14)) FARM-STOP by
    // condition; (G1 (9)) rows with ANY erasure of the napper's HOME (every source)
    let (mut reached_a6_n, mut stop_never_n, mut stop_homeless_n, mut home_erased_rows) = (0u32, 0u32, 0u32, 0u32);
    // (ironspec 05:10 real) the CARRIED fraction (rows whose stamp is armed at the release) and the
    // WINDOW lengths (the running sink's ticks left at the release in those rows)
    let mut carried_n = 0u32;
    let mut windows: Vec<i64> = Vec::new();
    // (G1's W4 / W5 on R2) every expected row accounted: a phase whose cart never reaches the arrival
    // detector loses all its rows, a row whose cart never reaches the stop's entry (the CART's cell)
    // is a miss, and the held walk's condition is a bar of its own
    let rows_per_phase = holds.len() * (1 + restart_ticks.len() * crate::liveness::Restart::rows().len() + o.visit_skip.len());
    let (mut expected_rows, mut missed_arrival_rows, mut missed_entry_rows, mut held_fail_rows) = (0u32, 0u32, 0u32, 0u32);
    for k in 0..phases {
        expected_rows += rows_per_phase as u32;
        // the leg and the shaft down to the arrival detector, shared by this phase's rows
        let mut w0 = base.clone();
        for _ in 0..k {
            w0.tick();
        }
        let summon_game = w0.world.time;
        // (cc767b78 04:43) the day time set BEFORE the summon, as the game's bench sets the time and
        // then summons: the villager's brain takes its activity from the schedule at its creation
        // (a fresh brain's first schedule update is immediate), so a day set after it left the brain
        // on the bench's old day time until its next update 20 gt later (the t22 offset)
        w0.world.mob.day_offset = day - w0.world.time;
        let cart = redsim::entity::spawn(&mut w0.world, "minecraft:minecart", start, Some(&format!("{{Motion:[{:.4}d,{:.4}d,{:.4}d],Passengers:[{rider_nbt}]}}", motion[0], motion[1], motion[2])))?;
        // (ironspec 05:20, the bench rule; G1 07:00: no silent skip) the day set before the summon,
        // ASSERTED on the fresh brain: REST at night, the day's scheduled activity by day
        let summon_act = assert_summon_activity(&w0.world, cart, day)?;
        let rider = w0.world.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart)).map(|e| e.id).ok_or("no rider")?;
        // (G1 05:50: the start state) (i), the DESIGN start: the napper holds HOME as a memory only and
        // the bed's ticket is FREE (onechunk's composed runs); (ii): the napper holds the ticket too.
        // The summon's HOME memory takes the ticket in redsim, so (i) gives it back
        let bed_bp = BlockPos::new(bed[0], bed[1], bed[2]);
        let summon_line = bed_line(&mut w0.world, bed_bp);
        let free0 = redsim::entity::mob::ai::poi::free_tickets(&mut w0.world, bed_bp);
        // (G1 05:50 / 05:56, ironspec 07:06) the start state, measured at the summon: (i), the design
        // start and the default, the napper holding HOME as a memory only and the bed's ticket FREE;
        // (ii) with --visit-start-held, the napper holding the ticket too. Whichever the summon left
        // (redsim's lazy ticket map counts the memories' claims at its first use: built before the
        // summon here, the ticket stays free), a DECLARED override sets the asked state, printed before
        // -> after; none is laid when the summon already left it
        let mut set_line = String::new();
        if o.visit_start_held && free0 == Some(1) {
            redsim::entity::mob::ai::poi::take(&mut w0.world, bed_bp);
            set_line = format!("; the napper's ticket TAKEN (a declared override for state (ii)), after: {}", bed_line(&mut w0.world, bed_bp));
        } else if !o.visit_start_held && free0 == Some(0) {
            redsim::entity::mob::ai::poi::release(&mut w0.world, bed_bp);
            set_line = format!("; the ticket GIVEN BACK (a declared stand-in for redsim's summon taking it), after: {}", bed_line(&mut w0.world, bed_bp));
        }
        let free1 = redsim::entity::mob::ai::poi::free_tickets(&mut w0.world, bed_bp);
        if free1 != Some(if o.visit_start_held { 0 } else { 1 }) {
            return Err(format!("the start state {} could not be set: at the summon {summon_line}{set_line}", if o.visit_start_held { "(ii)" } else { "(i)" }));
        }
        let start_state = format!(
            "start state {} (ticket state uncertain: pre-fix redsim) (at the summon {summon_line}{set_line})",
            if o.visit_start_held { "(ii) the napper holds the ticket" } else { "(i) the design: HOME a memory, the ticket free" },
        );
        let mut held_since: Option<u32> = None;
        let mut t_arr = None;
        // (ironspec 04:22: tick-exact's official repeats start AT THE REST) a cart summoned in the
        // queue cell itself (`--leg` at the seat, Motion 0) skips the leg: its rows start at once,
        // the hold counting from the summon (the official t40 release is hold 40)
        let in_queue = o.visit_at_rest || [start[0].floor() as i32, start[1].floor() as i32, start[2].floor() as i32] == q || [start[0].floor() as i32, (start[1] - 0.5).floor() as i32, start[2].floor() as i32] == q;
        if in_queue {
            t_arr = Some(0u32);
        }
        for t in 0..(if in_queue { 0 } else { 600u32 }) {
            w0.tick();
            let Some(i) = idx(&w0.world, rider) else { break };
            if let Some((_, wt, _)) = mem(&w0.world, i) {
                if wt && held_since.is_none() {
                    held_since = Some(t);
                }
                if !wt {
                    held_since = None;
                }
            }
            let c = w0.world.entities.list.iter().find(|e| e.id == cart && !e.removed).map(|e| [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32]);
            if c == Some(arrival) {
                t_arr = Some(t);
                break;
            }
        }
        let Some(t_arr) = t_arr else {
            out.push(format!("(visit) phase +{k}: the cart never reached the arrival detector: its {rows_per_phase} row(s) MISSED (W4)"));
            missed_arrival_rows += rows_per_phase as u32;
            continue;
        };
        let i = idx(&w0.world, rider).ok_or("the rider lost")?;
        let stamp_arr = mem(&w0.world, i).and_then(|m| m.0).map(|s| w0.world.time - s);
        for &h in holds {
            let mut rs: Vec<(Option<(i64, crate::liveness::Restart)>, Option<i64>)> = vec![(None, None)];
            for &rt in restart_ticks {
                for how in crate::liveness::Restart::rows() {
                    rs.push((Some((rt, how)), None));
                }
            }
            // (ironspec 02:33 / 03:01: B3's daytime bar) the night skipped s gt after the release
            for &s in &o.visit_skip {
                rs.push((None, Some(s)));
            }
            for (restart, skip) in rs {
                let mut r = w0.clone();
                // (ironspec 08:13) redsim records every run of SetWalkTargetFromBlockMemory in this row
                r.world.mob.ai.walk_memory_runs = Some(Vec::new());
                // (ironspec 03:01 (2): the positive control) a real villager standing in sight
                if let Some(b) = o.bystander {
                    redsim::entity::spawn(&mut r.world, "minecraft:villager", b, Some("{NoAI:1b,PersistenceRequired:1b}"))?;
                }
                // (G1 05:44: the HOME LOST bar's known positive, IN THE WORLD) a second villager asleep in
                // the napper's bed from the row's start: once the napper comes within 16 of it,
                // ValidateNearbyPoi erases its HOME and, the bed's sleeper keeping the ticket, no
                // re-claim can follow (the exposure path itself)
                if o.visit_plant_erase {
                    let bp = BlockPos::new(bed[0], bed[1], bed[2]);
                    let sid = redsim::entity::spawn(&mut r.world, "minecraft:villager", [bed[0] as f64 + 0.5, bed[1] as f64 + 0.5625, bed[2] as f64 + 0.5], Some("{PersistenceRequired:1b}"))?;
                    let si = idx(&r.world, sid).ok_or("the sleeper lost")?;
                    if !redsim::entity::mob::villager::start_sleeping(&mut r.world, si, bp) {
                        return Err(format!("--visit-plant-erase: the second villager could not lie down in the bed {bp}"));
                    }
                }
                let skip_label = skip.map_or(String::new(), |s| format!(", the night skipped at release + {s}"));
                let mut rider = rider;
                // (G1's positive, 05:36: a HOME released then re-acquired at once shows as "kept" at the
                // sample points) the row's Released events for its napper, counted from the row's start
                let (ev0, rider0) = (r.world.mob.events.len(), rider);
                let mut home_prev = idx(&r.world, rider).is_some_and(|ii| redsim::entity::mob::ai::brain::brain(&r.world, ii).home == Some(bed_p));
                let mut home_events: Vec<String> = Vec::new();
                let mut dt_prev = (r.world.time + r.world.mob.day_offset).rem_euclid(24000);
                let (mut erased_at, mut reclaim_delays, mut acq_attempts, mut acq_next_prev): (Option<u32>, Vec<String>, Vec<String>, i64) = (None, Vec::new(), Vec::new(), i64::MIN);
                // the previous tick's observation for the proxy (the row starts as if a walk target were
                // held: its first tick is never an after-no-target sample)
                let mut prev_obs = ProxyObs { wt: true, stamp: None, rest: false, sink_running: false };
                // (G1 16:06 / ironspec 16:24, night check 1) the rider's stamp arms and clears this row,
                // each with its cause read at the tick, and whether a walk-target path completed
                let mut row_path_complete = false;
                // (R6; G1 22:11 / 9fc3155f 22:14) the napper's health over the row: its first read and
                // its least
                let (mut row_h0, mut row_hmin) = (None::<f32>, f32::INFINITY);
                // the stamp at the row's previous judged tick (None until the first: a stamp carried in
                // from the arrival is no arm of this row)
                let mut prev_stamp_seen: Option<Option<i64>> = None;
                // (ironspec 08:15) per row: the record's run ticks and branches, its first failing tick,
                // and the proxy's sample ticks (compared at the row's end)
                let (mut row_rec, mut row_proxy_ticks): (RowRecord, Vec<i64>) = (RowRecord::default(), Vec::new());
                let mut cart = cart;
                let (mut held_end, mut cleared, mut armed_after, mut first_calm, mut at_entry, mut stamp_entry, mut home_entry) =
                    (None::<u32>, None::<(u32, String)>, Vec::<(u32, String)>::new(), None::<String>, None::<u32>, None::<Option<i64>>, None::<bool>);
                let mut was_held = held_since.is_some();
                let mut prev_since = mem(&r.world, i).and_then(|m| m.0);
                let mut released = false;
                // (farmopt 17:07) the seat's own HOME path at its first calm REST tick before the release
                let mut seat_calm: Option<String> = None;
                let (mut cart_last, mut entry_v): (Option<([i32; 3], f64)>, Option<f64>) = (None, None);
                let (mut a6_clear, mut a6_armed, mut a6_sleep, mut a6_far, mut a6_home, mut stamp_max) = (None::<u32>, Vec::<u32>::new(), None::<u32>, 0.0f64, None::<bool>, 0i64);
                let mut rest_z: Option<f64> = None;
                let mut had_stamp = false;
                let mut seat_panic = 0u32;
                let mut path_uid: Option<u64> = None;
                let mut recomputes: Vec<(u32, Option<[i32; 3]>, bool, usize, bool, String)> = Vec::new();
                let (mut stamp_at_release, mut release_game): (Option<Option<i64>>, Option<i64>) = (None, None);
                let mut stamp_after_controls: Option<Option<i64>> = None;
                let mut sink_left_at_release: Option<i64> = None;
                let mut nav_prev: (bool, (usize, usize, bool, i64, f64, bool, i64)) = (false, (0, 0, true, 0, 0.0, false, 0));
                for t in 0..(h + 400 + follow) {
                    if let Some((rt, how)) = restart {
                        if t as i64 == rt {
                            let map = crate::liveness::restart(&mut r.world, how)?;
                            // (ironspec 08:13; 09:13 (b)) the load keeps the AI state, the record with
                            // it (the test the_record_survives_a_world_open): a record found OFF after a
                            // world open is an ERROR, never a silent re-arm
                            if r.world.mob.ai.walk_memory_runs.is_none() {
                                return Err(format!("phase +{k} hold {h}{skip_label}: the record of SetWalkTargetFromBlockMemory's runs found OFF after the world open at t {t} (the load is expected to keep it)"));
                            }
                            rider = *map.get(&rider).ok_or("the rider lost in the restart")?;
                            cart = *map.get(&cart).ok_or("the cart lost in the restart")?;
                        }
                    }
                    if t == h {
                        release(&mut r.world, o)?;
                        released = true;
                        // (farmopt 04:11) the napper's stamp at the release (armed and its age, or
                        // cleared), and the game time of the release (tick-exact's frame)
                        if let Some(ii) = idx(&r.world, rider) {
                            stamp_at_release = Some(mem(&r.world, ii).and_then(|m| m.0).map(|s| r.world.time - s));
                            // (ironspec 05:10 real: the WINDOW) MoveToTargetSink's ticks left at the
                            // release when it runs (the carried stamp lives until that sink ends)
                            if redsim::entity::mob::ai::brain::has_exact(&r.world, ii) {
                                let x = redsim::entity::mob::ai::brain::ex(&r.world, ii);
                                sink_left_at_release = x.nodes.iter().find(|n| matches!(n.kind, redsim::entity::mob::ai::brain::tree::Kind::MoveToTargetSink { .. }) && n.running).map(|n| n.end - r.world.time);
                            }
                            // (the positive control) the stamp erased at the release: a partial
                            // recompute after it must arm it. (G1 (7)) a control that cannot be planted
                            // is an error, and the stamp is read back after it
                            if o.visit_clear_stamp {
                                if !redsim::entity::mob::ai::brain::has_exact(&r.world, ii) {
                                    return Err(format!("phase +{k} hold {h}: --visit-clear-stamp cannot be planted: the napper has no exact brain"));
                                }
                                redsim::entity::mob::ai::brain::ex_mut(&mut r.world, ii).mem.cant_reach_since = None;
                                if redsim::entity::mob::ai::brain::ex(&r.world, ii).mem.cant_reach_since.is_some() {
                                    return Err(format!("phase +{k} hold {h}: --visit-clear-stamp did not take (the stamp read back armed)"));
                                }
                            }
                            // (G1 via farmopt 05:08: the "0 HOME lost" bar's known positive) a stamp
                            // PLANTED at the release, already `age` gt old: kept long enough, it must
                            // release HOME (a strand event)
                            if let Some(age) = o.visit_plant_stamp {
                                if !redsim::entity::mob::ai::brain::has_exact(&r.world, ii) {
                                    return Err(format!("phase +{k} hold {h}: --visit-plant-stamp cannot be planted: the napper has no exact brain"));
                                }
                                let now = r.world.time;
                                redsim::entity::mob::ai::brain::ex_mut(&mut r.world, ii).mem.cant_reach_since = Some(now - age);
                                if redsim::entity::mob::ai::brain::ex(&r.world, ii).mem.cant_reach_since != Some(now - age) {
                                    return Err(format!("phase +{k} hold {h}: --visit-plant-stamp did not take"));
                                }
                            }
                            // (G1 (7)) the would-arm class read on the stamp AFTER the controls, not on
                            // the flag
                            stamp_after_controls = Some(mem(&r.world, ii).and_then(|m| m.0).map(|s| r.world.time - s));
                            // (G1 05:44: the HOME LOST bar's known positive, planted IN THE WORLD, not
                            // through an internal erase) see `visit_plant_erase` at the row's start
                        }
                        release_game = Some(r.world.time);
                    }
                    if let Some(s) = skip {
                        if t as i64 == h as i64 + s {
                            // the skip: the day time set to the next morning (sleeping players' wake)
                            let dt = r.world.time + r.world.mob.day_offset;
                            let next = (dt.div_euclid(24000) + 1) * 24000;
                            r.world.mob.day_offset += next - dt;
                        }
                    }
                    r.tick();
                    // (ironspec 08:13; 09:13 (a), G1 09:15) the record of SetWalkTargetFromBlockMemory's
                    // runs, drained and tallied RIGHT AFTER the tick, before anything that can cut the
                    // tick's processing short (the rider gone, no brain memories), so no run is left
                    // uncounted (armed at the row's start; a world open keeps it; found off is an
                    // error): every run toward the napper's HOME is a sample, judged on the stamp age
                    // it SAW
                    tally.take(&mut r.world, &[rider, rider0], &mut row_rec, &format!("phase +{k} hold {h}{skip_label}"))?;
                    // (ironspec 23:44 (1): B4's coupling to C7's rows) the queue rest: the cart's z
                    // the first tick it stands still on the queue rail before the release
                    if !released && rest_z.is_none() {
                        if let Some(c) = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed) {
                            let cc = [c.pos.x.floor() as i32, c.pos.y.floor() as i32, c.pos.z.floor() as i32];
                            if cc == q && c.delta_movement.x.abs() < 1e-4 && c.delta_movement.y.abs() < 1e-4 && c.delta_movement.z.abs() < 1e-4 {
                                rest_z = Some(c.pos.z);
                            }
                        }
                    }
                    if std::env::var("WAYCELL_VISIT_TRACE").is_ok() && released && t < h + std::env::var("WAYCELL_VISIT_TRACE").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(12) {
                        if let Some(c) = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed) {
                            let cc = BlockPos::new(c.pos.x.floor() as i32, c.pos.y.floor() as i32, c.pos.z.floor() as i32);
                            eprintln!("visit t {t}: cart ({:.3},{:.3},{:.3}) v ({:.3},{:.3},{:.3}) at {:?} {:?}; input {:?}", c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z, cc, r.world.get(cc), r.world.get(BlockPos::new(q[0] - 1, q[1], q[2])));
                        }
                    }
                    let Some(i) = idx(&r.world, rider) else { break };
                    // (farmopt 04:17, cc767b78's list: the side-by-side with tick-exact's official ride)
                    // per tick for the row WAYCELL_NAV_TRACE names: the cart's Pos / Motion, the
                    // navigation (next node / node count, is_done, timeout timer / limit, stuck, speed
                    // modifier), MoveToTargetSink (running, ticks to its end) and, when it stops, the
                    // state that stopped it (path done / timeout / stuck / reached / duration), the
                    // PATH memory's object and CANT_REACH
                    if let Ok(sel) = std::env::var("WAYCELL_NAV_TRACE") {
                        let me = format!("phase +{k}, hold {h}{skip_label}");
                        if me == sel && redsim::entity::mob::ai::brain::has_exact(&r.world, i) {
                            let x = redsim::entity::mob::ai::brain::ex(&r.world, i);
                            let sink = x.nodes.iter().find(|n| matches!(n.kind, redsim::entity::mob::ai::brain::tree::Kind::MoveToTargetSink { .. }));
                            let nav = &redsim::entity::mob::ai::ai(&r.world, i).nav;
                            let (ni, nc) = nav.path.as_ref().map_or((0, 0), |p| (p.next_node_index, p.node_count()));
                            let running = sink.is_some_and(|n| n.running);
                            let left = sink.map_or(0, |n| n.end - r.world.time);
                            let wt = x.mem.walk_target.map(|w| format!("{:?}", w.target)).unwrap_or_else(|| "none".to_string());
                            let stop = if nav_prev.0 && !running {
                                let (pi, pc, pdone, ptimer, plimit, pstuck, pleft) = nav_prev.1;
                                format!(
                                    " STOPPED: {}",
                                    if pleft <= 1 { "its duration ran out" } else if pstuck { "stuck" } else if pdone || (pc > 0 && pi >= pc) { "the path done" } else if ptimer as f64 > plimit { "the node timeout" } else { "canStillUse false (the walk target reached or erased)" }
                                )
                            } else if !nav_prev.0 && running {
                                " STARTED".to_string()
                            } else {
                                String::new()
                            };
                            if let Some(c) = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed) {
                                eprintln!(
                                    "nav-trace [{me}] {} (game time {}): cart ({:.4},{:.4},{:.4}) v ({:.4},{:.4},{:.4}); nav node {ni}/{nc} done {} timeout {} / {:.1} stuck {} speed {:.2}; sink {} (ends in {left}){stop}; walk target {wt}; PATH {:?}; CANT_REACH {:?}; activity {:?}",
                                    if released { format!("release + {}", t - h) } else { format!("hold t {t}") },
                                    r.world.time,
                                    c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z,
                                    nav.is_done(),
                                    nav.timeout_timer,
                                    nav.timeout_limit,
                                    nav.is_stuck,
                                    nav.speed_modifier,
                                    if running { "running" } else { "idle" },
                                    x.mem.path.as_ref().map(|p| (p.uid, p.next_node_index, p.node_count(), p.reached)),
                                    x.mem.cant_reach_since.map(|s| r.world.time - s),
                                    x.active
                                );
                            }
                            nav_prev = (running, (ni, nc, nav.is_done(), nav.timeout_timer, nav.timeout_limit, nav.is_stuck, left));
                        }
                    }
                    let Some((since, wt, act)) = mem(&r.world, i) else { continue };
                    // (G1 (2), ironspec 07:16; G1 08:12 CHANGES (1)(2); the refinement ruled by ironspec
                    // 11:46 / 11:48, G1 11:44) the proxy's SAMPLES (the (ii) cross-check's side): every
                    // tick SetWalkTargetFromBlockMemory RAN as seen from the post-tick state
                    // ([`proxy_sample`]): a REST tick after a tick with NO walk target (judged on the
                    // previous tick's stamp), or the ERASE SIGNATURE (MoveToTargetSink's start check
                    // erased a reached walk target and the stamp earlier in the same tick: judged on the
                    // stamp the run saw, none)
                    let sink_running = redsim::entity::mob::ai::brain::has_exact(&r.world, i)
                        && redsim::entity::mob::ai::brain::ex(&r.world, i).nodes.iter().any(|n| matches!(n.kind, redsim::entity::mob::ai::brain::tree::Kind::MoveToTargetSink { .. }) && n.running);
                    let now_obs = ProxyObs { wt, stamp: since, rest: act.contains("Rest"), sink_running };
                    // (G1 16:06 / ironspec 16:24) the stamp's arm / clear this tick and its cause: the
                    // lie-down (SleepInBed's start sets LAST_SLEPT this tick), the walk target reached
                    // (MoveToTargetSink's erase: the target gone with the stamp), a reaching path (its
                    // path search reached the target; while PANIC, a flee path), else other
                    {
                        let x = redsim::entity::mob::ai::brain::ex(&r.world, i);
                        let path_reached = x.mem.path.as_ref().map(|p| p.reached);
                        row_path_complete |= path_reached == Some(true);
                        if let EntityKind::Mob(m) = &r.world.entities.list[i].kind {
                            row_h0.get_or_insert(m.health);
                            row_hmin = row_hmin.min(m.health);
                        }
                        match (prev_stamp_seen.unwrap_or(since), since) {
                            (None, Some(_)) => {
                                let cause = match path_reached {
                                    Some(false) => "a partial path",
                                    None => "no path",
                                    Some(true) => "re-armed beside a reaching path",
                                };
                                *stamp_arms.entry(cause).or_default() += 1;
                            }
                            (Some(_), None) => {
                                let slept = redsim::entity::mob::ai::brain::brain(&r.world, i).last_slept == Some(r.world.time);
                                let cause = if slept {
                                    "the lie-down (SleepInBed)"
                                } else if prev_obs.wt && !wt {
                                    "the walk target reached (MoveToTargetSink)"
                                } else if path_reached == Some(true) {
                                    if act.contains("Panic") { "a reaching flee path (PANIC)" } else { "a reaching path" }
                                } else {
                                    "other"
                                };
                                *stamp_clears.entry(cause).or_default() += 1;
                            }
                            _ => {}
                        }
                        prev_stamp_seen = Some(since);
                    }
                    if let Some(p) = proxy_sample(&prev_obs, home_prev, &now_obs, r.world.time) {
                        swt_samples += 1;
                        row_proxy_ticks.push(r.world.time);
                        match p {
                            ProxySample::AfterNoTarget(Some(age)) => {
                                swt_max = swt_max.max(age);
                                if age >= 1000 {
                                    swt_fails += 1;
                                    if swt_fail_where.len() < 8 {
                                        swt_fail_where.push(format!("phase +{k} hold {h}{skip_label} {}: {age} gt", if released { format!("release + {}", t - h) } else { format!("hold t {t}") }));
                                    }
                                }
                            }
                            ProxySample::AfterNoTarget(None) => {}
                            ProxySample::EraseSignature => swt_erase_n += 1,
                        }
                    }
                    prev_obs = now_obs;
                    // (G1 05:47: "no re-claim follows" SHOWN) every change of the napper's HOME (lost /
                    // regained) with the bed's free tickets, occupied flag and HOME memory holders
                    // (ironspec 06:31 (2)) at that tick
                    {
                        // (G1 (10)) HOME read as the napper's OWN bed, not any HOME
                        let home_now = redsim::entity::mob::ai::brain::brain(&r.world, i).home == Some(bed_p);
                        let when = if released { format!("release + {}", t - h) } else { format!("hold t {t}") };
                        if home_now != home_prev {
                            let bl = bed_line(&mut r.world, BlockPos::new(bed[0], bed[1], bed[2]));
                            let home_val = redsim::entity::mob::ai::brain::brain(&r.world, i).home;
                            home_events.push(format!("{when} {} (the napper's HOME now {home_val:?}; {bl})", if home_now { "HOME REGAINED" } else { "HOME ERASED" }));
                            if !home_now {
                                erased_at = Some(t);
                            } else if let Some(e0) = erased_at.take() {
                                // (ironspec 07:30 real (2)) the RE-CLAIM delay: from the release to the
                                // bed regained, and where the cart was (past A6's entry or not)
                                reclaim_delays.push(format!("{} gt (regained {}{})", t - e0, when, at_entry.map_or(" before A6's entry".to_string(), |te| format!(", {} gt past A6's entry", t - te))));
                            }
                            home_prev = home_now;
                        }
                        // (ironspec 07:30 real (2)) every AcquirePoi(HOME) ATTEMPT of the napper (its
                        // node's next schedule redrawn), with the bed's retry marker (previous, next,
                        // back-off delay) after it
                        if redsim::entity::mob::ai::brain::has_exact(&r.world, i) {
                            let x = redsim::entity::mob::ai::brain::ex(&r.world, i);
                            for n in &x.nodes {
                                if let redsim::entity::mob::ai::brain::tree::Kind::AcquirePoi { acquire: redsim::entity::mob::ai::brain::M::Home, next_scheduled, batch, .. } = &n.kind {
                                    if *next_scheduled != acq_next_prev {
                                        if acq_next_prev != i64::MIN {
                                            let bp = BlockPos::new(bed[0], bed[1], bed[2]);
                                            let mk = batch.iter().find(|(q, _)| *q == bp).map(|(_, m)| format!("the bed's marker previous {} next {} delay {}", m.previous, m.next, m.delay));
                                            acq_attempts.push(format!("{when} (next try at game time {next_scheduled}{})", mk.map_or(String::new(), |s| format!("; {s}"))));
                                        }
                                        acq_next_prev = *next_scheduled;
                                    }
                                }
                            }
                        }
                        // (ironspec 07:02 real: the FARM-STOP bar is sampled at the visit's END, see
                        // below; a release healed by a re-claim inside the visit is (a), priced)
                        // (G1 05:50) the bed's tickets at each DUSK the row crosses (the orphaned
                        // ticket: taken with no live holder)
                        let dt_now = (r.world.time + r.world.mob.day_offset).rem_euclid(24000);
                        if dt_prev < 12000 && dt_now >= 12000 {
                            let bl = bed_line(&mut r.world, BlockPos::new(bed[0], bed[1], bed[2]));
                            home_events.push(format!("DUSK at {when} ({bl}; the napper's HOME {})", if home_now { "present" } else { "ABSENT" }));
                        }
                        dt_prev = dt_now;
                    }
                    // every new path object (`Path.uid`) the rider's brain holds after the release,
                    // with the cart's cell and whether it reaches its target; a partial one before
                    // A6's entry is a recompute at a partial position
                    if redsim::entity::mob::ai::brain::has_exact(&r.world, i) {
                        if let Some(pth) = redsim::entity::mob::ai::brain::ex(&r.world, i).mem.path.as_ref() {
                            if !released {
                                // the path held before the release (the leg's or the seat's): the
                                // baseline, not a recompute of the ride
                                path_uid = Some(pth.uid);
                            } else if Some(pth.uid) != path_uid {
                                path_uid = Some(pth.uid);
                                let cc = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed).map(|e| [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32]);
                                let first: Vec<String> = pth.nodes.iter().take(3).map(|n| format!("({},{},{})", n.x, n.y, n.z)).collect();
                                let wt_s = redsim::entity::mob::ai::brain::ex(&r.world, i).mem.walk_target.as_ref().map(|w| format!("{:?}", w.target)).unwrap_or_else(|| "none".to_string());
                                let detail = format!(
                                    "release + {} at {:?}: {} ({} nodes, target ({},{},{}), first {}; walk target {wt_s}; activity {act})",
                                    t - h,
                                    cc,
                                    if pth.reached { "COMPLETE" } else { "PARTIAL" },
                                    pth.nodes.len(),
                                    pth.target.x,
                                    pth.target.y,
                                    pth.target.z,
                                    first.join(" ")
                                );
                                recomputes.push((t - h, cc, pth.reached, pth.nodes.len(), at_entry.is_none(), detail));
                            }
                        }
                    }
                    if was_held && !wt && held_end.is_none() {
                        held_end = Some(t);
                    }
                    was_held |= wt && held_end.is_none() && !released;
                    if since.is_none() && prev_since.is_some() && cleared.is_none() {
                        cleared = Some((t, act.clone()));
                    }
                    // (ironspec 00:07 / 00:17: the seat panic) PANIC ticks while the napper waits seated,
                    // before the release (by design none at night: record7's hatch hides the post)
                    if !released && act.contains("Panic") {
                        seat_panic += 1;
                    }
                    if !released && seat_calm.is_none() && act.contains("Rest") {
                        let c = r.world.entities.list[i].vehicle.and_then(|v| r.world.entities.list.iter().find(|e| e.id == v)).map(|e| [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32]);
                        if c == Some(q) {
                            seat_calm = Some(format!("+{t} gt {}", probe(&mut r, i)));
                        }
                    }
                    if released {
                        if since.is_some() && since != prev_since {
                            let tg = redsim::entity::mob::ai::brain::ex(&r.world, i).mem.walk_target.map(|w| format!("{:?}", w.target)).unwrap_or_else(|| "no walk target".to_string());
                            armed_after.push((t - h, tg));
                        }
                        if first_calm.is_none() && act.contains("Rest") {
                            first_calm = Some(format!("+{} gt {}", t - h, probe(&mut r, i)));
                        }
                        let p = r.world.entities.list[i].pos;
                        let _ = p;
                        if let Some(c) = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed) {
                            cart_last = Some(([c.pos.x.floor() as i32, c.pos.y.floor() as i32, c.pos.z.floor() as i32], (c.delta_movement.x.powi(2) + c.delta_movement.z.powi(2)).sqrt()));
                        }
                        // (G1's W5 on R2) the stop's entry detected on the CART's cell (as the ride rows
                        // read it), not the rider's (a seated rider's cell can sit a block above)
                        if cart_last.map(|c| c.0) == Some(entry) && at_entry.is_none() {
                            at_entry = Some(t - h);
                            entry_v = cart_last.map(|c| c.1);
                            stamp_entry = Some(since.map(|s| r.world.time - s));
                            home_entry = Some(redsim::entity::mob::ai::brain::brain(&r.world, i).home == Some(bed_p));
                        }
                        // (ironspec 22:10 (2): the held walk into A6) past the entry: the stamp's clear,
                        // new armings, the sleep, HOME, how far from the bed's head the napper gets
                        if let Some(te) = at_entry {
                            let after = t - h - te;
                            if since.is_none() && prev_since.is_some() && a6_clear.is_none() {
                                a6_clear = Some(after);
                            }
                            if after > 0 && since.is_some() && since != prev_since {
                                a6_armed.push(after);
                            }
                            if a6_sleep.is_none() && redsim::entity::mob::mob(&r.world, i).sleeping_pos.is_some() {
                                a6_sleep = Some(after);
                                // (G1 (14)) FARM-STOP's second condition's positive, in the world: the
                                // bed broken once the napper lies down (it wakes; its HOME's POI gone,
                                // ValidateNearbyPoi erases HOME)
                                if o.visit_break_bed {
                                    // (G1 / ironspec 07:46) the lie-down PRINTED before the break
                                    // (SleepInBed.start: sleeping_pos set), then the bed broken
                                    let sp = redsim::entity::mob::mob(&r.world, i).sleeping_pos;
                                    home_events.push(format!("release + {} the napper LAY DOWN (SleepInBed.start: sleeping_pos {sp:?}), {after} gt past the entry; THEN its bed BROKEN the same tick", t - h));
                                    break_bed(&mut r.world, bed_p)?;
                                }
                            }
                            let d = ((p.x - (bed[0] as f64 + 0.5)).powi(2) + (p.y - bed[1] as f64).powi(2) + (p.z - (bed[2] as f64 + 0.5)).powi(2)).sqrt();
                            a6_far = a6_far.max(d);
                            a6_home = Some(redsim::entity::mob::ai::brain::brain(&r.world, i).home == Some(bed_p));
                            if after >= follow {
                                break;
                            }
                        }
                    }
                    if let Some(s) = since {
                        stamp_max = stamp_max.max(r.world.time - s);
                        had_stamp = true;
                    }
                    prev_since = since;
                }
                rows_n += 1;
                // (G1 (9)) the event list never shrinks inside a row (a restart that rebuilt it would
                // make the count read 0): an error, not a silent 0
                if r.world.mob.events.len() < ev0 {
                    return Err(format!("phase +{k} hold {h}{skip_label}: the world's mob event list shrank inside the row ({} < {ev0}): the Released count cannot be read", r.world.mob.events.len()));
                }
                let released = r.world.mob.events[ev0..].iter().filter(|e| matches!(e, redsim::entity::mob::MobEvent::Released { id, .. } if *id == rider0 || *id == rider)).count();
                home_released_n += (released > 0) as u32;
                // (G1 (9), ironspec 07:16) HOME RELEASED counts ANY erasure of the napper's HOME (every
                // source: SetWalkTargetFromBlockMemory's release, ValidateNearbyPoi's occupied-bed or
                // missing-bed erase), read from the HOME changes the row saw
                home_erased_rows += home_events.iter().any(|e| e.contains("HOME ERASED")) as u32;
                // (G1 / ironspec 09:13 (a), 09:15) the record's log EMPTY at the row's end: every run
                // was drained and tallied on its own tick (a run left behind is an error, never a
                // silent loss)
                record_left_empty(&r.world, &format!("phase +{k} hold {h}{skip_label}"))?;
                // (ironspec 08:15) the proxy's samples against the record's runs, tick by tick: a run
                // the proxy misses or a proxy sample with no run is a mismatch (the cross-check bar)
                let (missed, spurious) = proxy_mismatch(&row_rec.ticks, &row_proxy_ticks);
                proxy_rows += (!row_rec.ticks.is_empty() || !row_proxy_ticks.is_empty()) as u32;
                if !missed.is_empty() || !spurious.is_empty() {
                    proxy_mismatch_rows += 1;
                    if proxy_mismatch_where.len() < 6 {
                        proxy_mismatch_where.push(format!("phase +{k} hold {h}{skip_label}: runs the proxy missed at game time {missed:?}, proxy samples with no run at {spurious:?}"));
                    }
                }
                let mut row_branch_n: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
                for b in &row_rec.branches {
                    *row_branch_n.entry(b.as_str()).or_default() += 1;
                }
                let swt_row = format!(
                    "SetWalkTargetFromBlockMemory toward HOME ran {} time(s) ({}), the first failing run {}; the proxy's samples {} ({})",
                    row_rec.ticks.len(),
                    row_branch_n.iter().map(|(b, n)| format!("{b} {n}")).collect::<Vec<_>>().join(", "),
                    row_rec.first_fail.map_or("none".to_string(), |(t, a)| format!("at game time {t}, the stamp {a} gt")),
                    row_proxy_ticks.len(),
                    if missed.is_empty() && spurious.is_empty() { "the same ticks".to_string() } else { format!("MISMATCH: missed {missed:?}, spurious {spurious:?}") }
                );
                let before = cleared.as_ref().is_some_and(|(t, _)| *t < h) || (stamp_arr.is_none() && cleared.is_none());
                before_n += before as u32;
                armed_after_n += (!armed_after.is_empty() || !a6_armed.is_empty()) as u32;
                home_lost_n += (home_entry == Some(false) || a6_home == Some(false)) as u32;
                entry_stamped_n += matches!(stamp_entry, Some(Some(_))) as u32;
                // (ironspec 22:10 (2)) the held walk into A6 passes when: (i) the stamp's largest age
                // over the visit <= 1,000 (inherited margin under the physical 1,200); (ii) a stamp at
                // the entry clears inside A6; (iii) HOME kept; (v) 0 armings, 0 escapes (the napper
                // past 16 of the bed's head); (iv) entry -> R is 58f770cc's harness's
                let i_ok = stamp_max <= 1000;
                let ii_ok = !matches!(stamp_entry, Some(Some(_))) || a6_clear.is_some();
                let esc = a6_far > 16.0;
                over_1000_n += (!i_ok) as u32;
                not_cleared_n += (!ii_ok) as u32;
                escaped_n += esc as u32;
                let held_ok = i_ok && ii_ok && !esc && armed_after.is_empty() && a6_armed.is_empty() && home_entry == Some(true) && a6_home != Some(false);
                held_fail_rows += (!held_ok) as u32;
                i_samples += had_stamp as u32;
                followed_n += a6_home.is_some() as u32;
                seat_panic_rows += (seat_panic > 0) as u32;
                for (rt, cc, reached, n, descent, _) in &recomputes {
                    if *descent {
                        descent_paths_n += 1;
                        if !reached {
                            descent_partial_n += 1;
                            if partial_where.len() < 12 {
                                partial_where.push(format!("phase +{k} hold {h}{}{skip_label}: release + {rt} at {cc:?} ({n} nodes)", restart.map_or(String::new(), |(t, how)| format!(" restart +{t} ({})", how.label()))));
                            }
                        }
                    }
                }
                // (ironspec 02:58 (3)) every path the ride set, named, on the plain rows: each path set
                // between the release and A6's entry and the one carried in
                let partial_here = recomputes.iter().any(|r| r.4 && !r.2);
                let stamp_rel_s = match stamp_at_release { Some(Some(a)) => format!("ARMED {a} gt old"), Some(None) => "cleared".to_string(), None => "-".to_string() };
                if partial_here {
                    // (farmopt 04:11) the "would arm" class: the stamp cleared at the release and a
                    // partial recompute in the descent
                    if matches!(stamp_after_controls, Some(None)) {
                        would_arm_n += 1;
                    } else {
                        partial_stamped_n += 1;
                    }
                }
                if matches!(stamp_at_release, Some(Some(_))) {
                    carried_n += 1;
                    if let Some(w) = sink_left_at_release {
                        windows.push(w);
                    }
                }
                if restart.is_none() {
                    out.push(format!(
                        "(visit) phase +{k}, hold {h}{skip_label}: {start_state}; summoned at game time {summon_game} (arrival detector at summon + {}), released before game time {} (NOT-ready off); the stamp at the release {stamp_rel_s}{}; the sink's ticks left at the release {}; {swt_row}; the paths the ride set: {}",
                        t_arr + 1,
                        release_game.map_or("-".to_string(), |g| g.to_string()),
                        if o.visit_clear_stamp { " (ERASED at the release: the positive control)" } else { "" },
                        sink_left_at_release.map_or("- (not running)".to_string(), |w| w.to_string()),
                        if recomputes.is_empty() { "none".to_string() } else { recomputes.iter().map(|r| format!("{}{}", r.5, if r.4 { "" } else { " [past the entry]" })).collect::<Vec<_>>().join("; ") }
                    ));
                }
                if skip.is_some() || o.bystander.is_some() {
                    let dt = (r.world.time + r.world.mob.day_offset).rem_euclid(24000);
                    out.push(format!(
                        "(visit) phase +{k}, hold {h}{skip_label}: the stamp at the landing (A6's entry) {}; at the row's end (day time {dt}{}) the stamp {}, HOME {}",
                        match stamp_entry { Some(Some(a)) => format!("{a} gt old"), Some(None) => "none".to_string(), None => "- (never reached)".to_string() },
                        if dt >= 12000 { ", past dusk" } else { ", before dusk" },
                        idx(&r.world, rider).and_then(|i| mem(&r.world, i)).and_then(|m| m.0).map_or("none".to_string(), |s| format!("{} gt old", r.world.time - s)),
                        match a6_home { Some(true) => "kept", Some(false) => "LOST", None => "-" }
                    ));
                }
                // (G1 05:47) the HOME changes with the bed's tickets, occupied flag and HOME memory
                // holders, and the bed at the row's end, on every row where HOME changed or a positive
                // was planted. (ironspec 06:31 (1) / 07:02 real, the FARM-STOP bar, 0) sampled at the
                // VISIT's END: a visit that reached A6 and ends without the napper having lain down, or
                // with the napper without HOME, fails
                let home_end = idx(&r.world, rider).is_some_and(|ii| redsim::entity::mob::ai::brain::brain(&r.world, ii).home == Some(bed_p));
                let farm_stop = at_entry.is_some() && (a6_sleep.is_none() || !home_end);
                farm_stop_n += farm_stop as u32;
                // (G1 (1) / (14)) HOME LOST and FARM-STOP's samples are the rows that reached A6; the
                // farm-stop counted by condition (never lay down / lay down but no HOME at the end)
                reached_a6_n += at_entry.is_some() as u32;
                missed_entry_rows += at_entry.is_none() as u32;
                rows_path_complete += row_path_complete as u32;
                rows_damaged += row_h0.is_some_and(|h| row_hmin < h) as u32;
                rows_health_read += row_h0.is_some() as u32;
                health_least = health_least.min(row_hmin);
                oldest_stamp = oldest_stamp.max(stamp_max);
                stop_never_n += (at_entry.is_some() && a6_sleep.is_none()) as u32;
                stop_homeless_n += (at_entry.is_some() && a6_sleep.is_some() && !home_end) as u32;
                if !home_events.is_empty() || o.visit_plant_erase || o.visit_plant_stamp.is_some() || farm_stop {
                    let bl = bed_line(&mut r.world, BlockPos::new(bed[0], bed[1], bed[2]));
                    // (G1 (12)) the restart rows labelled, and every row's start state on its HOME line
                    let restart_label = restart.map_or(String::new(), |(t, how)| format!(" restart +{t} ({})", how.label()));
                    out.push(format!(
                        "(visit) phase +{k}, hold {h}{restart_label}{skip_label}: {start_state}; HOME changes {}; Released events {released}; AcquirePoi(HOME) attempts {}; re-claim delays {}; at the visit's end the napper {} and its bed as HOME {}{}; {bl}",
                        if home_events.is_empty() { "none".to_string() } else { home_events.join("; ") },
                        if acq_attempts.is_empty() { "none".to_string() } else { format!("{}: {}", acq_attempts.len(), acq_attempts.join("; ")) },
                        if reclaim_delays.is_empty() { if erased_at.is_some() { "NOT RE-CLAIMED in the row".to_string() } else { "none".to_string() } } else { reclaim_delays.join("; ") },
                        match a6_sleep { Some(s) => format!("lay down at + {s} gt past the entry"), None => "NEVER LAY DOWN".to_string() },
                        if home_end { "present" } else { "ABSENT" },
                        if farm_stop { " (FARM-STOP)" } else { "" },
                    ));
                }
                match a6_sleep {
                    Some(s) => sleep_latest = sleep_latest.max(s),
                    None => never_slept += at_entry.is_some() as u32,
                }
                if let Some(z) = rest_z {
                    rest_lo = rest_lo.min(z);
                    rest_hi = rest_hi.max(z);
                } else if h >= 20 {
                    no_rest_n += 1;
                }
                if restart.is_none() || !held_ok || seat_panic > 0 {
                    out.push(format!(
                        "(visit) phase +{k}, hold {h}{}: arrival at t {t_arr} with the stamp {}; the queue rest z {}; seat PANIC ticks before the release {seat_panic}; the held walk {} ; the seat's first calm REST tick {}; the stamp cleared {}; armings after the release {}; the first calm REST tick after it {}; at the entry {}: the stamp {}, HOME {}; held walk: stamp {} at entry, cleared at {} in A6, the largest age {stamp_max} gt (<= 1,000, inherited: ironspec 22:10, under the physical 1,200: {}), asleep at {}, armings in A6 {}, HOME at +{follow} {}, the farthest from the bed's head {a6_far:.2} (16: the ValidateNearbyPoi ball, physical){}",
                        restart.map_or(skip_label.clone(), |(rt, how)| format!(", restart at +{rt} ({})", how.label())),
                        stamp_arr.map_or("none".to_string(), |a| format!("{a} gt")),
                        rest_z.map_or("- (released before the cart stood still)".to_string(), |z| format!("{z:.4}")),
                        match (held_since, held_end) {
                            (None, _) => "none at the arrival".to_string(),
                            (Some(s), Some(e)) => format!("(since leg t {s}) dropped at +{e} gt"),
                            (Some(s), None) => format!("(since leg t {s}) NEVER dropped before the entry"),
                        },
                        seat_calm.as_deref().unwrap_or("none"),
                        cleared.as_ref().map_or("NEVER".to_string(), |(t, a)| format!("at +{t} gt on a {} tick{}", if a.contains("Panic") { "PANIC" } else { "calm" }, if *t < h { " (before the release)" } else { " (AFTER the release)" })),
                        if armed_after.is_empty() { "none".to_string() } else { format!("{armed_after:?}") },
                        first_calm.as_deref().unwrap_or("none"),
                        at_entry.map_or_else(|| format!("NEVER (the cart last at {:?})", cart_last), |t| format!("+{t} gt after the release at {:.4} b/t", entry_v.unwrap_or(0.0))),
                        stamp_entry.map_or("-".to_string(), |s| s.map_or("none".to_string(), |a| format!("{a} gt"))),
                        home_entry.map_or("-".to_string(), |b| if b { "kept".to_string() } else { "LOST".to_string() }),
                        stamp_entry.flatten().map_or("none".to_string(), |a| a.to_string()),
                        a6_clear.map_or(if matches!(stamp_entry, Some(Some(_))) { "NEVER".to_string() } else { "-".to_string() }, |c| format!("+{c} gt")),
                        if i_ok { "yes" } else { "NO" },
                        a6_sleep.map_or("never".to_string(), |s| format!("+{s} gt")),
                        if a6_armed.is_empty() { "none".to_string() } else { format!("{a6_armed:?}") },
                        a6_home.map_or("-".to_string(), |b| if b { "kept".to_string() } else { "LOST".to_string() }),
                        if esc { " (ESCAPED past 16)" } else { "" }
                    ));
                }
            }
        }
    }
    out.push(format!(
        "(visit) {rows_n} rows ({phases} arrival phases x holds {holds:?} x (plain + restarts at {restart_ticks:?} in 10 orders)), the post {}, followed {follow} gt past A6's entry: the queue rest z {} (rows holding >= 20 gt with no rest: {no_rest_n}); rows with a seat panic before the release {seat_panic_rows}; path computations in the descent (release to A6's entry) {descent_paths_n}, PARTIAL {descent_partial_n}{}; rows with a partial descent recompute: {would_arm_n} with the stamp cleared at the release (the WOULD-ARM class{}), {partial_stamped_n} with it still armed (kept, not re-armed); the leg's stamp CARRIED to the release in {carried_n} of {rows_n} rows, its sink's ticks left at the release (the WINDOW) {}; the latest sleep after the entry +{sleep_latest} gt, rows entered and never asleep {never_slept}; the stamp cleared before the release in {before_n}; stamped at the entry {entry_stamped_n}, of them not cleared inside A6 {not_cleared_n}; the stamp's largest age over 1,000 in {over_1000_n}; armed after the release or in A6 in {armed_after_n}; HOME LOST (no HOME at the entry or at the follow window's end: no re-claim; the window's end a CONSERVATIVE PROXY for 'by the next dusk'; the bar, 0) {home_lost_n}; the FARM-STOP bar (at the visit's end: a visit that reached A6 ending with the napper never lain down or without HOME; 0) in {farm_stop_n}; HOME RELEASED (a strand event: a Released event, a bed re-claimed at once included; priced, not barred) in {home_released_n}; escaped {escaped_n}: the held walk {} (entry -> R: 58f770cc's harness)",
        if post.is_some() { "in place" } else { "ABSENT (the (C') hatch shut at night / (C''))" },
        if rest_lo.is_finite() { format!("{rest_lo:.4}..{rest_hi:.4}") } else { "never reached".to_string() },
        if partial_where.is_empty() { String::new() } else { format!(" [{}]", partial_where.join("; ")) },
        if o.visit_clear_stamp { ": every row's stamp ERASED at the release, the positive control" } else { "" },
        if windows.is_empty() {
            "- (no carried row with a running sink)".to_string()
        } else {
            let mut ws = windows.clone();
            ws.sort();
            format!("{}..{} (median {}, over {} rows)", ws[0], ws[ws.len() - 1], ws[ws.len() / 2], ws.len())
        },
        "judged per bar (the BAR lines below)"
    ));
    // (G1 07:16 (1)) EVERY bar its own PASS / FAIL / NOT JUDGED line over its own samples, each
    // contributing to the exit status (NOT JUDGED fails); HOME LOST and FARM-STOP sampled over the
    // rows that reached A6
    let mut bars: Vec<(String, u32, u32, String)> = vec![
        ("(i) the stamp's largest age over the visit <= 1,000 gt (ironspec 22:10 (2); W11: the code's bar, at most 1,000)".to_string(), rows_n, over_1000_n, format!("{i_samples} row(s) carried a stamp at some tick; the others hold the bar at age 0")),
        (
            "(ii) the AT-ENTRY bar: the stamp under 1,000 gt at every run of SetWalkTargetFromBlockMemory toward HOME, from redsim's RECORD of its runs (the tick, the branch, the stamp age it SAW; ironspec 08:13)".to_string(),
            tally.samples,
            tally.fails,
            format!(
                "the branches {}; the largest age seen {} gt{}; the proxy beside it (a REST tick with HOME after a tick with no walk target, judged on the previous tick's stamp; or the erase signature, MoveToTargetSink's start check erasing a reached walk target and the stamp in the same tick, judged on none: ironspec 11:46 / 11:48): {swt_samples} samples ({swt_erase_n} at the erase signature), {swt_fails} failing, the largest {swt_max} gt{}",
                tally.branches.iter().map(|(b, n)| format!("{b} {n}")).collect::<Vec<_>>().join(", "),
                tally.max,
                if tally.fail_where.is_empty() { String::new() } else { format!("; failing at {}", tally.fail_where.join(", ")) },
                if swt_fail_where.is_empty() { String::new() } else { format!(" (at {})", swt_fail_where.join(", ")) }
            ),
        ),
        (
            "(ii) cross-check: the proxy's sample ticks are the record's run ticks (ironspec 08:15: a run the proxy misses or a proxy sample with no run fails the row)".to_string(),
            proxy_rows,
            proxy_mismatch_rows,
            if proxy_mismatch_where.is_empty() { String::new() } else { proxy_mismatch_where.join("; ") },
        ),
        ("(v) 0 armings after the release or in A6 (a partial recompute arms only when the stamp is clear: the descent's partial recomputes judged here)".to_string(), rows_n, armed_after_n, format!("{descent_partial_n} partial recomputes in the descent")),
        ("(v) 0 escapes (the napper past 16 of the bed's head in A6)".to_string(), followed_n, escaped_n, String::new()),
        ("HOME LOST: the napper's own bed as HOME at A6's entry and at the visit's end, every erasure source (ironspec 07:16)".to_string(), reached_a6_n, home_lost_n, String::new()),
    ];
    bars.push((
        format!("FARM-STOP at the visit's end (the follow window {follow} gt, its bound {FOLLOW_BOUND} gt)"),
        if follow >= FOLLOW_BOUND { reached_a6_n } else { 0 },
        farm_stop_n,
        format!("never lay down {stop_never_n}, lay down but without its bed as HOME at the end {stop_homeless_n}{}", if follow >= FOLLOW_BOUND { "" } else { "; the window under its bound: NOT JUDGED" }),
    ));
    // (G1's W4 on R2) every expected row reached the arrival detector and the stop's entry (the cart's
    // cell): a missed row fails, it never passes the other bars by being absent
    bars.push((
        format!("every expected row ({phases} phase(s) x {rows_per_phase} row(s)) reached the arrival detector and the stop's entry (the cart's cell, W4 / W5)"),
        expected_rows,
        missed_arrival_rows + missed_entry_rows,
        format!("{missed_arrival_rows} row(s) of phases that never reached the arrival detector, {missed_entry_rows} row(s) whose cart never reached the entry"),
    ));
    // (G1's W5 on R2) the held walk's condition (i), (ii), no escape, no arming after the release or in
    // A6, HOME at the entry and in A6, as a bar over every row run
    bars.push((
        "the held walk in every row: (i) the stamp <= 1,000, (ii) an entry stamp cleared in A6, no escape, no arming after the release or in A6, HOME at the entry and in A6 (W5)".to_string(),
        rows_n,
        held_fail_rows,
        String::new(),
    ));
    // (G1's W7 on R2) the water given its tick
    bars.push((format!("the chunk's water scheduled (W7): {fluid_line}"), 1, (!fluid_ok) as u32, String::new()));
    // (R6; G1 22:11, ironspec 22:15: the leg's fall is a run's call) the napper's health over every row:
    // a row whose napper lost health (a landing passing the cart's fall on) fails
    bars.push((
        "the napper's health: no damage in any row (a landing that passes the cart's fall on to its rider fails; G1 22:11)".to_string(),
        rows_health_read,
        rows_damaged,
        format!("the least health read {}", if health_least.is_finite() { format!("{health_least:.1}") } else { "-".to_string() }),
    ));
    let mut pass = true;
    for (name, samples, fails, note) in &bars {
        let verdict = if *samples == 0 {
            "NOT JUDGED (0 samples): FAILS".to_string()
        } else if *fails == 0 {
            format!("PASSES over {samples} samples")
        } else {
            format!("FAILS {fails}/{samples}")
        };
        pass &= *samples > 0 && *fails == 0;
        out.push(format!("(visit) BAR {name}: {verdict}{}", if note.is_empty() { String::new() } else { format!(" ({note})") }));
    }
    // priced, not barred (ironspec 06:24 / 07:30): the strand events and the time at the stop
    out.push(format!(
        "(visit) the rider's CANT_REACH stamps over the {rows_n} rows (G1 16:06 / ironspec 16:24, night check 1): the oldest {oldest_stamp} gt; arms {} ({}); clears {} ({}); a walk-target path that completed (reached its target) at some tick in {rows_path_complete} of {rows_n} rows",
        stamp_arms.values().sum::<u32>(),
        if stamp_arms.is_empty() { "none".to_string() } else { stamp_arms.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ") },
        stamp_clears.values().sum::<u32>(),
        if stamp_clears.is_empty() { "none".to_string() } else { stamp_clears.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ") },
    ));
    out.push(format!(
        "(visit) PRICED: HOME RELEASED (any erasure of the napper's HOME, every source) in {home_erased_rows} of {rows_n} rows (with a Released event: {home_released_n}); the leg's stamp at A6's entry in {entry_stamped_n} rows, cleared in A6 in {}; the rows on one world (its seed fixed; the rows differ in the arrival phase and the hold)",
        entry_stamped_n - not_cleared_n.min(entry_stamped_n)
    ));
    out.push(format!(
        "(visit) the follow window {follow} gt against its BOUND {FOLLOW_BOUND} gt (G1 07:06: MoveToTargetSink's longest duration 250 + A6's restart-to-lie-down 89 + 2 for the walk target's re-set and the restart: a sink running at A6's entry ends by entry + 250, so a napper not asleep by the bound is a real farm-stop): {}",
        if follow >= FOLLOW_BOUND { "the window covers it" } else { "TOO SHORT: the farm-stop bar NOT JUDGED" }
    ));
    Ok((out, pass))
}

/// (G1 07:06 (1)) the visit rows' follow window's bound: MoveToTargetSink's longest duration (150 +
/// nextInt(100) < 250) + A6's restart-to-lie-down (89 gt, the normal lie-down past the entry) + 2
/// (the walk target's re-set and the sink's restart).
const FOLLOW_BOUND: u32 = 250 + 89 + 2;

/// (farmopt 17:07: the seat's own HOME path) A fresh rider seated in a cart at each of `at` (cart
/// positions), a few ticks to settle, and the HOME path from there to `bed`: its start node, end
/// node, complete or partial, and its nodes.
pub fn path_probe_lines(spec: &FarmSpec, o: &Opts, at: &[[f64; 3]], bed: P) -> Result<(Vec<String>, Vec<String>), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 40, WalkMode::Random) };
    let (mut base, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o)?);
    let override_lines = place_overrides(&mut base.world, o.extra.len(), &cells)?;
    settle_r1(&mut base.world, o)?;
    let bed_p = BlockPos::new(bed[0], bed[1], bed[2]);
    // (G1 (6)) the override rule's before -> after lines returned beside the probe's own lines (the
    // callers index the probe's lines by position)
    let mut out = Vec::new();
    for a in at {
        let mut r = base.clone();
        let cart = redsim::entity::spawn(&mut r.world, "minecraft:minecart", *a, Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
        for _ in 0..3 {
            r.tick();
        }
        let Some(i) = r.world.entities.list.iter().position(|e| !e.removed && e.vehicle == Some(cart)) else {
            out.push(format!("(path) at {a:?}: no rider"));
            continue;
        };
        let p = r.world.entities.list[i].pos;
        let res = redsim::entity::mob::ai::with_nav(&mut r.world, i, |w, nm, nav, arena| {
            let mut copy = nav.clone();
            copy.create_path_pos(w, nm, bed_p, 0, arena).map(|pa| (pa.reached, pa.nodes.iter().map(|n| (n.x, n.y, n.z)).collect::<Vec<_>>()))
        });
        out.push(match res {
            Some((reached, nodes)) => format!(
                "(path) a rider at ({:.2},{:.2},{:.2}): the HOME path {} ({} nodes): {:?}",
                p.x,
                p.y,
                p.z,
                if reached { "COMPLETE" } else { "partial" },
                nodes.len(),
                nodes
            ),
            None => format!("(path) a rider at ({:.2},{:.2},{:.2}): no path", p.x, p.y, p.z),
        });
    }
    Ok((override_lines, out))
}

/// (farmopt 20:16: tick-exact's official restart row) tick-exact's water rig mixed: six DRY slopes
/// ascending south (the cart rolls north from rest) onto twelve WATERLOGGED flats, glass under every
/// rail and walls at x +- 1, the stop glass at z -18, a ridden cart summoned at (0.5, 100.5625,
/// 0.5); a restart (`how`) between ticks t - 1 and t for each t of `ticks` (and none): the tick and
/// the speed at which the cart leaves the 5th wet flat (z below -10), and its move on the reload
/// tick.
pub fn water_restart_rows(ticks: &[i64], how: crate::liveness::Restart) -> Result<Vec<String>, String> {
    use redsim::world::{flags, World};
    fn set(w: &mut World, p: (i32, i32, i32), s: &str) -> Result<(), String> {
        w.set_block(BlockPos::new(p.0, p.1, p.2), Block::parse(s).map_err(|e| format!("{s}: {e}"))?, flags::ALL);
        Ok(())
    }
    let mut w0 = World::new(BlockPos::new(-4, 88, -22), [12, 18, 28]);
    let slope = "rail[shape=ascending_south,waterlogged=false]";
    let flat = "rail[shape=north_south,waterlogged=true]";
    for i in 0..6 {
        set(&mut w0, (0, 99 - i, -i), "glass")?;
        for y in (99 - i)..=(101 - i) {
            set(&mut w0, (-1, y, -i), "glass")?;
            set(&mut w0, (1, y, -i), "glass")?;
        }
        set(&mut w0, (0, 100 - i, -i), slope)?;
    }
    for j in 0..12 {
        let z = -6 - j;
        set(&mut w0, (0, 93, z), "glass")?;
        for y in 93..=95 {
            set(&mut w0, (-1, y, z), "glass")?;
            set(&mut w0, (1, y, z), "glass")?;
        }
        set(&mut w0, (0, 94, z), flat)?;
    }
    set(&mut w0, (0, 94, -18), "glass")?;
    set(&mut w0, (0, 95, -18), "glass")?;
    set(&mut w0, (0, 100, 1), "glass")?;
    for y in 100..=102 {
        set(&mut w0, (-1, y, 1), "glass")?;
        set(&mut w0, (1, y, 1), "glass")?;
    }
    set(&mut w0, (0, 101, 1), "rail[shape=north_south,waterlogged=false]")?;
    set(&mut w0, (0, 101, 2), "glass")?;
    set(&mut w0, (0, 102, 2), "glass")?;
    for i in 0..6 {
        set(&mut w0, (0, 100 - i, -i), slope)?;
    }
    let cart0 = redsim::entity::spawn(&mut w0, "minecraft:minecart", [0.5, 100.5625, 0.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
    let mut out = Vec::new();
    let t_origin = w0.time;
    for rt in std::iter::once(None).chain(ticks.iter().map(|&t| Some(t))) {
        let mut w = w0.clone();
        let mut cart = cart0;
        let (mut left, mut reload_move) = (None::<(i64, f64)>, None::<(f64, f64, f64, f64)>);
        let mut idle = 0i64;
        for t in 1..=200u32 {
            let before = w.entities.list.iter().find(|e| e.id == cart && !e.removed).map(|e| (e.pos.z, e.delta_movement.z.abs()));
            if rt == Some(t as i64) {
                let rs = crate::liveness::restart(&mut w, how)?;
                idle = rs.entity_free;
                cart = *rs.get(&cart).ok_or("the cart lost in the restart")?;
            }
            w.tick();
            let Some(e) = w.entities.list.iter().find(|e| e.id == cart && !e.removed) else { break };
            if rt == Some(t as i64) {
                if let Some((z0, v0)) = before {
                    reload_move = Some((z0, e.pos.z, v0, e.delta_movement.z.abs()));
                }
            }
            if left.is_none() && e.pos.z < -10.0 {
                // (G1's CHANGES 1 on ac58d99cb) keyed by GAME time (the fixture's world tick), so a
                // world open's idle ticks count as the game counts them
                left = Some((w.time - t_origin, e.delta_movement.z.abs()));
                break;
            }
        }
        out.push(format!(
            "(water-restart) {}: leaves the 5th wet flat at game tick {}{}",
            rt.map_or("no restart".to_string(), |t| format!("a restart at step {t} ({}, {idle} idle tick(s))", how.label())),
            left.map_or("NEVER".to_string(), |(t, v)| format!("{t} at {v:.4} b/t")),
            reload_move.map_or(String::new(), |(z0, z1, v0, v1)| format!("; the reload tick z {z0:.3} -> {z1:.3}, v {v0:.4} -> {v1:.4}"))
        ));
    }
    Ok(out)
}

/// (ironspec 19:12 (c): containment) The module with the extra cells (the way-in's water) run for
/// `ticks` gt: every water cell in the box `lo..=hi` that isn't a waterlogged rail (flowing or source
/// water outside the rails), at checkpoints and at the end; the bar: none.
pub fn water_scan_lines(spec: &FarmSpec, o: &Opts, ticks: u32, lo: P, hi: P) -> Result<(Vec<String>, bool), String> {
    // (G1's W2 on R2) the verdict reaches the caller: PASS only with water judged (samples > 0) and no
    // loose water cell at any checkpoint
    let (scheduled, worst, lines) = water_scan(spec, o, ticks, lo, hi)?;
    Ok((lines, scheduled > 0 && worst == 0))
}

/// [`water_scan_lines`]'s run: (the water cells given their tick (the samples), the most loose
/// water cells at a checkpoint, the lines).
pub fn water_scan(spec: &FarmSpec, o: &Opts, ticks: u32, lo: P, hi: P) -> Result<(usize, usize, Vec<String>), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    // (G1 07:16 (6)) the override rule: every cell laid printed before -> after (the scan's first
    // lines), a no-op --extra failing
    let override_lines = place_overrides(&mut run.world, o.extra.len(), &cells)?;
    // a paste with updates: every water cell's water tick in the box (the file's and the extras'), as
    // its updateShape schedules it when a neighbour lands (a raw placement schedules nothing; without
    // this the water could never spread and the scan would prove nothing). (ironspec 23:56: no
    // vacuous passes) the count of scheduled cells is the scan's sample count: 0 = NOT JUDGED
    let mut scheduled = 0usize;
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let p = BlockPos::new(x, y, z);
                if redsim::fluid::fluid_state(&run.world, p).fluid.is_water() {
                    run.world.schedule_fluid_tick(p, redsim::fluid::Fluid::Water, 5);
                    scheduled += 1;
                }
            }
        }
    }
    for (p, b) in &cells {
        let inside = (0..3).all(|k| [p.x, p.y, p.z][k] >= lo[k] && [p.x, p.y, p.z][k] <= hi[k]);
        if !inside && (redsim::fluid::waterlogged(b) == Some(true) || redsim::fluid::fluid_state_of(b).fluid.is_water()) {
            run.world.schedule_fluid_tick(*p, redsim::fluid::Fluid::Water, 5);
            scheduled += 1;
        }
    }
    let scan = |w: &redsim::world::World| -> (usize, Vec<String>) {
        let (mut wet_rails, mut loose) = (0usize, Vec::new());
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let p = BlockPos::new(x, y, z);
                    if !redsim::fluid::fluid_state(w, p).fluid.is_water() {
                        continue;
                    }
                    let b = w.get(p);
                    if redsim::rails::is_rail(&b) && redsim::fluid::waterlogged(&b) == Some(true) {
                        wet_rails += 1;
                    } else {
                        loose.push(format!("({x},{y},{z}) {b}"));
                    }
                }
            }
        }
        (wet_rails, loose)
    };
    let mut out = vec![format!("(water-scan) the override rule's lines (G1 07:16 (6)):")];
    out.extend(override_lines);
    let mut t = 0u32;
    let mut worst = 0usize;
    for cp in [1u32, 20, 200, 1200, 6000, ticks] {
        if cp > ticks || cp < t {
            continue;
        }
        while t < cp {
            run.tick();
            t += 1;
        }
        let (wet, loose) = scan(&run.world);
        worst = worst.max(loose.len());
        out.push(format!("(water-scan) t {t}: {wet} waterlogged rail(s), {} other water cell(s) in the box {lo:?}..{hi:?}{}", loose.len(), if loose.is_empty() { String::new() } else { format!(": {}", loose.join(", ")) }));
    }
    out.push(format!(
        "(water-scan) over {ticks} gt: {scheduled} water cell(s) given their water tick (the samples), {} checkpoint(s); the most water cells outside the rails at a checkpoint {worst}: {}",
        out.len(),
        if scheduled == 0 { "NOT JUDGED (0 samples): FAIL" } else if worst == 0 { "PASS" } else { "FAIL" }
    ));
    Ok((scheduled, worst, out))
}

/// (ironspec 00:27 (5)/(7): the water rule) Every dry waterloggable cell of the box `lo..=hi` the
/// game would turn into a water source as laid: `FlowingFluid.getNewLiquid` (redsim's
/// `fluid::get_new_liquid_of`, the fluid tick's own code) gives it a source (two or more horizontal
/// sources and a solid block or a source under it). A design with one leaks or floods.
pub fn water_rule_violations(w: &redsim::world::World, lo: P, hi: P) -> Vec<BlockPos> {
    let mut v = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let p = BlockPos::new(x, y, z);
                let s = w.get(p);
                let dry = redsim::fluid::waterlogged(&s) == Some(false) || s.is_air();
                if dry && redsim::fluid::get_new_liquid_of(w, redsim::fluid::Fluid::Water, p, &s).is_source() {
                    v.push(p);
                }
            }
        }
    }
    v
}

/// (ironspec 00:27 (5)) The water-layout search: each candidate (a name and its cells, `--extra`
/// syntax) laid on the design and gated in order, every gate through its row's own code path:
/// (1) the water rule ([`water_rule_violations`] on the laid world); (2) containment
/// ([`water_scan`], every water cell scheduled, the count printed); (3) the seat's HOME path
/// complete ([`path_probe_lines`] at the first rest); (4) every rest x `phases` release phases in
/// the band with 0 contacts ([`ride_rows_with`], its own PASS). Rides only for candidates passing
/// (1) and (2). One line per candidate.
pub fn water_search_lines(spec: &FarmSpec, o: &Opts, cands: &[(String, String)], rests: &[[f64; 3]], bed: P, entry: P, phases: u32, scan_ticks: u32, lo: P, hi: P, sweep: bool) -> Result<(Vec<String>, bool), String> {
    let mut out = Vec::new();
    let mut survivors = 0u32;
    // the base world (no candidate) for each candidate's cost
    let ro0 = RunOpts { ai: true, ..RunOpts::steady(1, 10, WalkMode::Random) };
    let (mut base0, _) = crate::placing::start(&widened(spec, o), &ro0, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut laid0: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        laid0.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    // (G1 07:16 (6)) the base's cells through the override rule, printed once
    out.push("(water-search) the base world's cells (the --extra and the queue's), each before -> after:".to_string());
    out.extend(place_overrides(&mut base0.world, o.extra.len(), &laid0)?);
    for (name, cells) in cands {
        let mut oc = o.clone();
        let cand = parse_extra(Some(cells))?;
        oc.extra.extend(cand.clone());
        oc.replace.extend(parse_replace(Some(cells))?);
        // the cost against the base: glass, fences, water (waterlogged cells), gold (powered rails)
        let (mut glass, mut fences, mut water, mut gold) = (0i32, 0i32, 0i32, 0i32);
        for (p, s) in &cand {
            let new = Block::parse(s).map_err(|e| format!("{s}: {e}"))?;
            let old = base0.world.get(BlockPos::new(p[0], p[1], p[2]));
            let (ns, os) = (new.to_string(), old.to_string());
            glass += ns.ends_with(":glass") as i32 - os.ends_with(":glass") as i32;
            fences += matches!(new, Block::Fence { .. }) as i32 - matches!(old, Block::Fence { .. }) as i32;
            water += (redsim::fluid::waterlogged(&new) == Some(true)) as i32 - (redsim::fluid::waterlogged(&old) == Some(true)) as i32;
            gold += ns.contains(":powered_rail") as i32 - os.contains(":powered_rail") as i32;
        }
        let cost = format!("cost {glass:+} glass, {fences:+} fences, {water:+} water, {gold:+} gold");
        // (1) the rule, on the world as the scan lays it (before any tick)
        let ro = RunOpts { ai: true, ..RunOpts::steady(1, 10, WalkMode::Random) };
        let (mut run, _) = crate::placing::start(&widened(spec, &oc), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
        let mut laid: Vec<(BlockPos, Block)> = Vec::new();
        for (p, b) in oc.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(&oc).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
            laid.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
        }
        // (G1 07:16 (6)) the candidate's cells through the override rule: one that changes nothing
        // fails the candidate (named), not the search
        let cand_overrides = match place_overrides(&mut run.world, oc.extra.len(), &laid) {
            Ok(l) => l,
            Err(e) => {
                out.push(format!("(water-search) {name} ({cost}): FAILS at the override rule: {e}"));
                continue;
            }
        };
        let laid_n = cand_overrides.first().cloned().unwrap_or_default();
        let rule = water_rule_violations(&run.world, lo, hi);
        // (2) containment; a DRY layout has nothing to contain: out of scope, said so (not a vacuous
        // pass). (G1 07:16 (4)) ONE dry definition for the screen and the sweep: no water anywhere in
        // the built world (every cell of the rides' region, not only the scan box)
        let water_cells = world_water_cells(&run.world);
        let dry = water_cells == 0;
        let (scheduled, loose, _) = water_scan(spec, &oc, scan_ticks, lo, hi)?;
        let contained = dry || (scheduled > 0 && loose == 0);
        let mut line = format!(
            "(water-search) {name} ({cost}; {}): the water rule {}; containment over {scan_ticks} gt: {scheduled} water cell(s) scheduled, {loose} loose: {}",
            laid_n.trim_start_matches("(overrides) "),
            if rule.is_empty() { "kept".to_string() } else { format!("BROKEN at {}", rule.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(" ")) },
            if dry { "out of scope (a dry layout: no water anywhere in the built world)".to_string() } else if scheduled == 0 { format!("NOT JUDGED ({water_cells} water cell(s) in the world, 0 scheduled in the box)") } else if loose == 0 { "PASS".to_string() } else { "FAIL".to_string() }
        );
        if !rule.is_empty() {
            line.push_str(": FAILS at the water rule");
            out.push(line);
            continue;
        }
        if !contained {
            line.push_str(": FAILS at containment");
            out.push(line);
            continue;
        }
        // (3) the seat's HOME path
        let (_, seat) = path_probe_lines(spec, &oc, &rests[..1], bed)?;
        // (G1 (15)) the word COMPLETE, not a substring ("INCOMPLETE" never matches)
        let seat_ok = seat.iter().any(|l| l.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "COMPLETE"));
        line.push_str(&format!("; the seat's HOME path {}", if seat_ok { "COMPLETE" } else { "PARTIAL" }));
        if !seat_ok {
            line.push_str(": FAILS at the seat's HOME path");
            out.push(line);
            continue;
        }
        // (4) the rests in the band (the first rest's plain phase-0 positions kept for the probe)
        let mut rides_ok = true;
        let mut why_rides: Vec<String> = Vec::new();
        let mut descent: Vec<[f64; 3]> = Vec::new();
        for (i, r) in rests.iter().enumerate() {
            oc.seat_at = Some(*r);
            let (rows, _) = ride_rows_core(spec, &oc, entry, 800, phases, &[], &[], None, if i == 0 { Some(&mut descent) } else { None })?;
            let summary = rows.last().cloned().unwrap_or_default();
            let pass = summary.ends_with(": PASS");
            rides_ok &= pass;
            if !pass {
                // why: a contact, out of the band, never at the entry, or another flag of the rows
                let contacts = rows.iter().filter(|l| l.contains("CONTACTS")).count();
                let out_of_band = rows.iter().filter(|l| l.contains("OUT OF THE BAR")).count();
                why_rides.push(format!("rest z {:.3}: {contacts} row(s) with a CONTACT, {out_of_band} row(s) out of the band", r[2]));
            }
            let speeds = summary.split(" after the release, ").nth(1).and_then(|s| s.split(" b/t").next()).unwrap_or("-").to_string();
            line.push_str(&format!("; rest z {:.3}: {speeds} {}", r[2], if pass { "PASS" } else { "FAIL" }));
        }
        if !rides_ok {
            line.push_str(&format!(": FAILS at the rides ({})", why_rides.join("; ")));
            out.push(line);
            continue;
        }
        // survivors: the descent probe over the nominal ride's positions (priced, not a gate), each
        // partial position NAMED (ironspec 01:46 (1)(iv): its tick, the cart's cell, its path; a world
        // open can land at any tick, so a recompute can happen at each of them)
        let (_, probe) = path_probe_lines(spec, &oc, &descent, bed)?;
        let partial: Vec<String> = probe
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains("partial"))
            .map(|(t, l)| {
                let c = descent.get(t).map(|p| format!("({},{},{})", p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32)).unwrap_or_default();
                let path = l.split("nodes): ").nth(1).unwrap_or("-");
                format!("t {t} cell {c} path {path} (a world open can land there: yes, any tick)")
            })
            .collect();
        // (ironspec 01:46 (1)(iii), tick-exact's t70..t77 lesson) the seat's HOME path runs FORWARD:
        // its second node down or on along the ride (lower, or further south than the seat), not back
        // up toward the landing
        let nodes = seat_path_nodes(&seat);
        let forward = seat_path_forward(&nodes);
        line.push_str(&format!(
            "; the seat's HOME path {} (its first nodes {:?})",
            if forward { "runs FORWARD" } else { "runs BACK UP THE RIDE" },
            nodes.iter().take(4).collect::<Vec<_>>()
        ));
        // (G1 07:16 (3)) the FORWARD bar fails the candidate (computed, so judged)
        if !forward {
            line.push_str(": FAILS at the seat's HOME path direction");
            out.push(line);
            continue;
        }
        line.push_str(&format!("; the descent probe: {} partial of {} positions{}: PASSES the screen", partial.len(), probe.len(), if partial.is_empty() { String::new() } else { format!(" [{}]", partial.join("; ")) }));
        let mut survived = true;
        if sweep {
            // (ironspec 00:27 (5)) the survivors' full list, first the rides: every rest x phases x a
            // restart at every tick 1..110 (10 orders, pre-official-load) x 1..4-gt freezes at every
            // tick 0..110; then containment over 24,000 gt
            let restarts: Vec<i64> = (1..=110).collect();
            let freezes: Vec<(i64, i64)> = (0..=110).flat_map(|s| (1..=4).map(move |l| (s, l))).collect();
            let mut all = true;
            for r in rests {
                oc.seat_at = Some(*r);
                let (rows, pass) = ride_rows_core(spec, &oc, entry, 800, phases, &restarts, &freezes, None, None)?;
                let summary = rows.last().cloned().unwrap_or_default();
                all &= pass;
                let speeds = summary.split(" after the release, ").nth(1).and_then(|s| s.split(" b/t").next()).unwrap_or("-").to_string();
                let n = summary.split(" rows").next().unwrap_or("").trim_start_matches("(ride) ").to_string();
                line.push_str(&format!("; sweep rest z {:.3}: {n} rows {speeds} {}", r[2], if pass { "PASS" } else { "FAIL" }));
            }
            let (scheduled, loose, _) = water_scan(spec, &oc, 24000, lo, hi)?;
            line.push_str(&format!("; containment over 24,000 gt: {scheduled} scheduled, {loose} loose{}", if dry { " (a dry layout: out of scope, the same definition as the screen's)" } else { "" }));
            // (G1 07:16 (4)) ONE dry definition (the screen's: no water anywhere in the built world)
            survived = all && (dry || (scheduled > 0 && loose == 0));
            line.push_str(if survived { ": PASSES the sweep" } else { ": FAILS the sweep" });
        }
        survivors += survived as u32;
        out.push(line);
    }
    // (G1 07:16 (3)) the search's verdict: at least one candidate survives every gate (the exit status)
    out.push(format!("(water-search) {survivors} of {} candidate(s) survive{}", cands.len(), if survivors == 0 { ": the search FAILS" } else { "" }));
    Ok((out, survivors > 0))
}

/// The napper's way down (the leg, the shaft, the queue): a cart with its rider started at `start`
/// with `motion`, followed until it rests on the queue rail: the tick it is on the HOLD detector
/// (`hold`), the tick it first comes within 16 of the bed's head (`bed`: ValidateNearbyPoi's
/// 16-ball), the tick it reaches the arrival detector and the queue cell: HOLD's lead.
pub fn leg_lines(spec: &FarmSpec, o: &Opts, start: [f64; 3], motion: [f64; 3], hold: P, bed: P, ticks: u32, day: Option<i64>) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o)?);
    // (G1 (6)) the override rule's before -> after lines kept, not discarded
    let override_lines = place_overrides(&mut run.world, o.extra.len(), &cells)?;
    settle_r1(&mut run.world, o)?;
    // (G1's W7 on R2) the chunk's water given its tick; frozen water refuses this information mode
    let (fluid_line, fluid_ok) = fluids_line(&mut run.world, o.chunk, "leg");
    if !fluid_ok {
        return Err(fluid_line);
    }
    // (ironspec 14:58: the real arrival stamp per visit) with `day`, the rider carries its HOME (the
    // bed) and the ride starts at that day time: a calm REST rider computes its HOME walk from the
    // falling cart, partial, and may arrive with its stamp armed
    let rider_nbt = match day {
        Some(_) => format!(
            "{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}",
            bed[0], bed[1], bed[2]
        ),
        None => "{id:\"minecraft:villager\"}".to_string(),
    };
    // (cc767b78 04:43) the day set before the summon (the brain reads the schedule at its creation)
    if let Some(d) = day {
        run.world.mob.day_offset = d - run.world.time;
    }
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&format!("{{Motion:[{:.4}d,{:.4}d,{:.4}d],Passengers:[{rider_nbt}]}}", motion[0], motion[1], motion[2])))?;
    // (ironspec 05:20, the bench rule; G1 07:00) the fresh brain's activity asserted for the leg too
    if let Some(d) = day {
        assert_summon_activity(&run.world, cart, d)?;
    }
    let rider = run.world.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart)).map(|e| e.id);
    let stamp_of = |w: &redsim::world::World| -> Option<i64> {
        let i = w.entities.list.iter().position(|e| !e.removed && Some(e.id) == rider)?;
        if !redsim::entity::mob::ai::brain::has_exact(w, i) {
            return None;
        }
        redsim::entity::mob::ai::brain::ex(w, i).mem.cant_reach_since.map(|s| w.time - s)
    };
    let (mut first_armed, mut stamp_arrival, mut stamp_rest) = (None::<(u32, P)>, None::<Option<i64>>, None::<Option<i64>>);
    let head = Vec3::new(bed[0] as f64 + 0.5, bed[1] as f64 + 0.5625, bed[2] as f64 + 0.5);
    let arrival = if o.south { [q[0], q[1] + 1, q[2] - 1] } else { [q[0] + 1, q[1] + 1, q[2]] };
    let (mut on_hold, mut ball, mut at_arrival, mut at_queue) = (None, None, None, None);
    let mut path: Vec<P> = Vec::new();
    // (onechunk 11:49: a cart falling at x 9.5 clears the pad's east frame at x 8 by 0.01; ironspec
    // 11:52: x and vx at every tick of the fall, 0 frame contacts) the box's x and z range over every
    // airborne tick of the fall, and its x and vx tick by tick (run-length printed)
    let (mut fx, mut fz) = ((f64::INFINITY, f64::NEG_INFINITY), (f64::INFINITY, f64::NEG_INFINITY));
    let mut fall: Vec<(u32, f64, f64)> = Vec::new();
    let mut contacts = 0u32;
    let leg_trace = std::env::var("WAYCELL_LEG_TRACE").is_ok();
    // (ironspec 02:18 (3)(b): bystander stamps) every pocket summoner's 10-box (`spawnGolemIfNeeded`:
    // the villager's box inflated by 10): the pocket riders P0..P2 at (x, 0, 0) and the H riders
    // H0..H2 at (x, 16, 0), x = -1 / 3 / 7, each seated in its cart at the cell's centre (feet at the
    // cart's y, rail y + 0.0625); the napper's box against each, every tick (the ticks inside any and
    // which, the game time's phase mod 100 printed: a summon can fire only at game time = 0 mod 100)
    let summoners: Vec<(String, redsim::entity::vec3::Aabb)> = [("P0", -1, 0), ("P1", 3, 0), ("P2", 7, 0), ("H0", -1, 16), ("H1", 3, 16), ("H2", 7, 16)]
        .iter()
        .map(|(n, x, y)| {
            let (cx, fy, cz) = (*x as f64 + 0.5, *y as f64 + 0.0625, 0.5);
            (n.to_string(), redsim::entity::vec3::Aabb { min_x: cx - 0.3 - 10.0, min_y: fy - 10.0, min_z: cz - 0.3 - 10.0, max_x: cx + 0.3 + 10.0, max_y: fy + 1.95 + 10.0, max_z: cz + 0.3 + 10.0 })
        })
        .collect();
    let mut exposure: Vec<(u32, i64, f64, Vec<String>)> = Vec::new();
    let mut health: Vec<(u32, f32)> = Vec::new();
    for t in 0..ticks {
        run.tick();
        if let Some(r) = run.world.entities.list.iter().find(|e| !e.removed && Some(e.id) == rider) {
            if let EntityKind::Mob(m) = &r.kind {
                health.push((t, m.health));
            }
            let b = &r.bb;
            let inside: Vec<String> = summoners.iter().filter(|(_, s)| b.min_x < s.max_x && b.max_x > s.min_x && b.min_y < s.max_y && b.max_y > s.min_y && b.min_z < s.max_z && b.max_z > s.min_z).map(|(n, _)| n.clone()).collect();
            exposure.push((t, run.world.time.rem_euclid(100), b.min_y, inside));
        }
        let Some(e) = run.world.entities.list.iter().find(|e| e.id == cart && !e.removed) else { break };
        let c = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
        // (tick-exact 01:16: an arrival state for the day-approach sight row) WAYCELL_LEG_TRACE: the
        // cart's Pos and Motion after every tick of the way down
        if leg_trace {
            eprintln!(
                "leg-trace t {t} (game time {}): cart ({:.6},{:.6},{:.6}) v ({:.6},{:.6},{:.6}) cell {}",
                run.world.time,
                e.pos.x,
                e.pos.y,
                e.pos.z,
                e.delta_movement.x,
                e.delta_movement.y,
                e.delta_movement.z,
                run.world.get(BlockPos::new(c[0], c[1], c[2]))
            );
        }
        if !redsim::rails::is_rail(&run.world.get(BlockPos::new(c[0], c[1], c[2]))) && !redsim::rails::is_rail(&run.world.get(BlockPos::new(c[0], c[1] - 1, c[2]))) && at_arrival.is_none() && on_hold.is_some() {
            fx = (fx.0.min(e.bb.min_x), fx.1.max(e.bb.max_x));
            fz = (fz.0.min(e.bb.min_z), fz.1.max(e.bb.max_z));
            fall.push((t, e.pos.x, e.delta_movement.x));
            // a contact: the box overlapping a block's collision box anywhere on the fall
            let bb = e.bb;
            let (x0, x1) = (bb.min_x.floor() as i32, (bb.max_x - 1e-9).floor() as i32);
            let (y0, y1) = (bb.min_y.floor() as i32, (bb.max_y - 1e-9).floor() as i32);
            let (z0, z1) = (bb.min_z.floor() as i32, (bb.max_z - 1e-9).floor() as i32);
            let mut hit = false;
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        let p = BlockPos::new(x, y, z);
                        let s = run.world.get(p);
                        if !s.is_air() && !redsim::rails::is_rail(&s) && redsim::behavior::full_cube(&s) {
                            hit = true;
                        }
                    }
                }
            }
            contacts += hit as u32;
        }
        if path.last() != Some(&c) {
            path.push(c);
        }
        if on_hold.is_none() && c == hold {
            on_hold = Some(t);
        }
        if ball.is_none() && e.pos.distance_to_sqr(head).sqrt() < 16.0 {
            ball = Some(t);
        }
        if at_arrival.is_none() && c == arrival {
            at_arrival = Some(t);
            stamp_arrival = Some(stamp_of(&run.world));
        }
        if first_armed.is_none() && stamp_of(&run.world).is_some() {
            first_armed = Some((t, c));
        }
        if at_queue.is_none() && c == q && e.delta_movement.length_sqr() < 1e-6 {
            at_queue = Some(t);
            stamp_rest = Some(stamp_of(&run.world));
            break;
        }
    }
    let lead = match (on_hold, ball) {
        (Some(h), Some(b)) => format!("{} gt", b as i64 - h as i64),
        _ => "unknown".to_string(),
    };
    let mut runs: Vec<String> = Vec::new();
    let mut i = 0;
    while i < fall.len() {
        let (t0, x, vx) = fall[i];
        let mut j = i;
        while j + 1 < fall.len() && (fall[j + 1].1 - x).abs() < 1e-9 && (fall[j + 1].2 - vx).abs() < 1e-9 && fall[j + 1].0 == fall[j].0 + 1 {
            j += 1;
        }
        runs.push(format!("t {t0}..{}: x {x:.6}, vx {vx:.6}", fall[j].0));
        i = j + 1;
    }
    let vmax = fall.iter().map(|f| f.2.abs()).fold(0.0f64, f64::max);
    Ok(vec![
        // (G1 08:12) the leg rows are INFORMATION: no bar is judged here (their exposure and stamps are
        // priced by the composed runs and the visit rows' bars), so they set no exit status
        "(leg) INFORMATION ONLY: no bar is judged by the leg rows (exit 0 by design; the visit rows carry the bars)".to_string(),
        fluid_line,
        format!("(leg) on the HOLD detector {hold:?} at t {on_hold:?}; within 16 of the bed's head at t {ball:?} (HOLD's lead {lead}); at the arrival detector at t {at_arrival:?}; at rest on the queue rail at t {at_queue:?}"),
        format!(
            "(leg) the fall ({} airborne ticks before the arrival): the cart's box x {:.3}..{:.3}, z {:.3}..{:.3}; |vx| at most {vmax:.6}; ticks with the box in a full block {contacts} ({})",
            fall.len(),
            fx.0,
            fx.1,
            fz.0,
            fz.1,
            if contacts == 0 && vmax < 1e-9 { "x fixed, vx 0, no box in a full block" } else { "x moved, vx not 0 or a box in a full block (information)" }
        ),
        format!("(leg) the fall tick by tick: {}", runs.join("; ")),
        format!(
            "(leg) the rider's CANT_REACH stamp{}: first armed {}; its age at the arrival detector {}, at rest on the queue {}",
            if day.is_some() { " (HOME the bed, the ride from that day time)" } else { " (no HOME: no HOME walk)" },
            first_armed.map_or("never".to_string(), |(t, c)| format!("at t {t} at {c:?}")),
            stamp_arrival.map_or("-".to_string(), |s| s.map_or("none".to_string(), |a| format!("{a} gt"))),
            stamp_rest.map_or("-".to_string(), |s| s.map_or("none".to_string(), |a| format!("{a} gt")))
        ),
        format!("(leg) the cells: {:?}", path.iter().take(60).collect::<Vec<_>>()),
        // (R6; G1 22:11 / 9fc3155f 22:14: the fall's damage is phase-dependent: a run decides) the
        // rider's health every tick: at the start, its least (and the tick), at the end
        match (health.first(), health.iter().min_by(|a, b| a.1.total_cmp(&b.1)), health.last()) {
            (Some(&(_, h0)), Some(&(tm, hm)), Some(&(_, he))) => format!(
                "(leg) the rider's health: {h0:.1} at the start, {hm:.1} at its least (t {tm}), {he:.1} at the end: {}",
                if hm < h0 { format!("DAMAGED by {:.1} (a landing passed the cart's fall to its rider)", h0 - hm) } else { "no damage (every fall erased)".to_string() }
            ),
            _ => "(leg) the rider's health: NOT READ (no rider)".to_string(),
        },
        {
            // the bystander exposure: per summoner, the ticks the napper's box lies inside its 10-box
            // (run-length), and the last tick inside any (the S ticks to avoid end there)
            let mut per: Vec<String> = Vec::new();
            for n in ["P0", "P1", "P2", "H0", "H1", "H2"] {
                let ts: Vec<u32> = exposure.iter().filter(|e| e.3.iter().any(|s| s == n)).map(|e| e.0).collect();
                if ts.is_empty() {
                    per.push(format!("{n} never"));
                    continue;
                }
                let mut runs: Vec<String> = Vec::new();
                let mut start = ts[0];
                for w in ts.windows(2) {
                    if w[1] != w[0] + 1 {
                        runs.push(format!("{start}..{}", w[0]));
                        start = w[1];
                    }
                }
                runs.push(format!("{start}..{}", ts[ts.len() - 1]));
                per.push(format!("{n} t {}", runs.join(", ")));
            }
            let last = exposure.iter().filter(|e| !e.3.is_empty()).map(|e| e.0).max();
            let at_last = last.and_then(|l| exposure.iter().find(|e| e.0 == l)).map(|e| format!("(the box's foot y {:.3}, game time = {} mod 100)", e.2, e.1)).unwrap_or_default();
            format!(
                "(leg) the bystander exposure (the napper's box inside a pocket summoner's 10-box; a summon can fire only at game time = 0 mod 100): {}; the last tick inside any: {} {at_last}",
                per.join("; "),
                last.map_or("none".to_string(), |l| format!("t {l}"))
            )
        },
    ]
    .into_iter()
    .chain(override_lines)
    .collect())
}

/// (R6; ironspec 03:34 / 03:36, G1 03:34) The leg train's planted rows: a ridden cart at rest on the
/// front (the drop must never land on it: the falling rider dies), the token's evidence removed (the hold
/// always powered: two nappers back to back), empty carts at rest where the riderless fault parks them.
#[derive(Clone, Debug, Default)]
pub struct LegPlants {
    pub front: bool,
    pub no_token: bool,
    pub empty: Vec<[f64; 3]>,
    /// the token held (not free at the start) until this tick, so a napper first rests in the hold
    pub token_at: Option<u32>,
    /// the stop's entry: a napper's hurts after it reaches this cell are the stop's, printed apart and not
    /// judged by the way-in's health BAR
    pub entry: Option<P>,
    /// (ironspec 04:32 / 04:37) cells whose `powered` is read every tick (the boosters): each unpowered tick printed
    pub watch_powered: Vec<P>,
    /// (G1 04:41 (2)) the bed head: the waiting seat's least distance to it printed over each napper's wait
    pub bed_head: Option<[f64; 3]>,
}

/// An entity's box as [min_x, min_y, min_z, max_x, max_y, max_z].
fn box_of(e: &redsim::entity::Entity) -> [f64; 6] {
    [e.bb.min_x, e.bb.min_y, e.bb.min_z, e.bb.max_x, e.bb.max_y, e.bb.max_z]
}

/// The xz overlap of two boxes (dx, dz), both > 0 when they overlap in plan.
fn xz_overlap(a: &[f64; 6], b: &[f64; 6]) -> (f64, f64) {
    (a[3].min(b[3]) - a[0].max(b[0]), a[5].min(b[5]) - a[2].max(b[2]))
}

/// (R6; ironspec 23:57: up to three nappers in flight, the stop a single server; (a) a FIFO queue at the
/// way-in, layout + bench level until day-1's one-in-flight answer, farmopt 00:01) Several nappers down
/// the leg: `count` carts, each with its rider (HOME the bed, the day time `day`), started at `start`
/// with `motion`, one every `spacing` gt; the built R1 released at each tick of `releases` (NOT-ready's
/// lever off for `hold_gt` gt, then on again). Per cart: its arrival at the arrival detector, the cell it
/// waits in, its departure over the departure detector, its least health, its stamp's largest age, any
/// tick off a rail after its arrival before its fall into the shaft; per tick the carts waiting at once
/// and the least distance between two carts. BARs: every cart arrives; none off a rail between its
/// arrival and its departure; every rider unhurt; the departures in the arrivals' order (FIFO); every
/// stamp under 1,200 gt; with releases, every cart departs. Returns the lines and the verdict.
#[allow(clippy::too_many_arguments)]
pub fn leg_train_lines(spec: &FarmSpec, o: &Opts, start: [f64; 3], motion: [f64; 3], bed: P, day: i64, count: u32, spacing: u32, gate: Option<u32>, releases: &[u32], release_after: Option<u32>, hold_gt: u32, ticks: u32, hold: Option<P>, watch_cell: Option<P>, plant_landing: bool, freeze: Option<(u32, u32)>, plants: &LegPlants) -> Result<(Vec<String>, bool), String> {
    if !(o.south && o.r1_built) {
        return Err("--leg-train needs the south staircase with the built R1 (--south --r1-built): its release is NOT-ready's lever, re-armed after each release".to_string());
    }
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o)?);
    let override_lines = place_overrides(&mut run.world, o.extra.len(), &cells)?;
    settle_r1(&mut run.world, o)?;
    let (fluid_line, fluid_ok) = fluids_line(&mut run.world, o.chunk, "leg train");
    if !fluid_ok {
        return Err(fluid_line);
    }
    run.world.mob.day_offset = day - run.world.time;
    let rider_nbt = format!(
        "{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}",
        bed[0], bed[1], bed[2]
    );
    let arrival = [q[0], q[1] + 1, q[2] - 1];
    let departure = [q[0], q[1] - 1, q[2] + 1];
    let rend = rail_end_of(q);
    let lever = not_ready_lever_of(q);
    let lever_on = Block::parse("minecraft:lever[face=wall,facing=west,powered=true]").map_err(|e| e.to_string())?;
    // per cart: (cart id, rider id, spawned at, arrival t, departure t, fell t, waits-in cell, least health,
    // first health, largest stamp age, off-rail ticks, first off-rail (t, cell))
    struct C {
        cart: u32,
        rider: Option<u32>,
        at: u32,
        arrival: Option<u32>,
        departure: Option<u32>,
        fell: Option<u32>,
        rest: Option<(u32, P)>,
        hmin: f32,
        hurt_at: Option<(u32, P)>,
        hmin_stop: f32,
        entry_t: Option<u32>,
        v_col: Option<f64>,
        v_shaft: Option<f64>,
        seat_min: f64,
        wait_ticks: u32,
        contacts: Vec<String>,
        h0: Option<f32>,
        stamp: i64,
        off: u32,
        first_off: Option<(u32, P)>,
        gone: Option<u32>,
        released: bool,
        held_at_hold: u32,
        passed_hold: bool,
        watch: Option<u32>,
        col_check: bool,
        shaft_check: bool,
    }
    let mut cs: Vec<C> = Vec::new();
    let (mut waiting_max, mut dmin) = (0usize, f64::INFINITY);
    let mut events: Vec<String> = Vec::new();
    let mut rearm_at: Option<u32> = None;
    // (G1 00:06) the next napper's start: on the fixed spacing, or (with `t_opts.gate`) the gate's delay
    // after the previous napper passes the departure detector (the evidence it left the landing and the
    // column); a release scheduled `release_after` gt after a napper comes to rest on the queue cell
    let mut next_spawn: Option<u32> = Some(0);
    let mut release_at: Option<u32> = None;
    // (ironspec 00:54: ONE napper-only hold upstream of the drop + the front) the hold `hold` (P1 on the
    // y-22 line): a powered rail (north-south) laid there, its power the TOKEN "the front is free": set at
    // the start and by every departure press (the gate's event), cleared when a napper passes the next
    // cell north (it left the hold, toward the drop); the bench emulates the token latch's output as a
    // redstone block in place of the hold's support (powered = go), glass otherwise (braked)
    let glass_b = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    let redstone_b = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let mut token = plants.token_at.is_none();
    let (mut token_sets, mut hold_passes) = (0u32, 0u32);
    if let Some(h) = hold {
        // the hold keeps the rail shape of its cell (a slope hold, like the siding rest R, relaunches a
        // stopped cart when powered; a flat one needs a solid block behind the cart)
        let here = run.world.get(BlockPos::new(h[0], h[1], h[2])).to_string();
        let shape = here.split("shape=").nth(1).and_then(|s| s.split([',', ']']).next()).ok_or_else(|| format!("the hold {h:?} holds {here}, no rail shape"))?.to_string();
        let rail = Block::parse(&format!("minecraft:powered_rail[powered=false,shape={shape},waterlogged=false]")).map_err(|e| e.to_string())?;
        let support = BlockPos::new(h[0], h[1] - 1, h[2]);
        run.world.set_block(support, redstone_b.clone(), redsim::world::flags::ALL);
        run.world.set_block(BlockPos::new(h[0], h[1], h[2]), rail, redsim::world::flags::ALL);
        let read = run.world.get(BlockPos::new(h[0], h[1], h[2]));
        if !read.to_string().contains("powered_rail") {
            return Err(format!("the hold {h:?} read back as {read}, not a powered rail"));
        }
        events.push(format!("the hold {h:?}: a powered rail on the token's output (redstone block {support} = free / go, glass = held), the token free at the start"));
    }
    let mut at_hold: Vec<u32> = Vec::new();
    // (G1 01:35: no landing may hold a ridden cart while another cart can fall onto it) every napper's
    // drop into the column and every queue drop into the shaft checked: the landing and the cells below
    // the drop free of any other cart at that tick; the planted twin (`plant_landing`) leaves a ridden cart
    // on the landing (a second planted cart waiting at the front holds it on the arrival slope)
    let mut drop_checks: Vec<(u32, usize, &'static str, Vec<String>)> = Vec::new();
    let mut landing_rests: Vec<(u32, u32, String)> = Vec::new();
    let (sx, sz) = shaft_xz_of(q);
    let mut planted: Vec<u32> = Vec::new();
    if plant_landing {
        let rider_plain = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        for p in [[q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], [arrival[0] as f64 + 0.5, arrival[1] as f64 + 0.5625, arrival[2] as f64 + 0.5]] {
            planted.push(redsim::entity::spawn(&mut run.world, "minecraft:minecart", p, Some(rider_plain))?);
        }
        events.push(format!("planted (the twin): a ridden cart at the front {q:?} and one on the landing {arrival:?} (held on the arrival slope by the front's cart)"));
    }
    // (ironspec 03:36 (3)) a ridden cart at rest on the front: it comes to rest on the front's braked slope
    // (the game: (9.500, -9.051, -3.887)), its box reaching into the column's footprint
    if plants.front {
        let rider_plain = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        planted.push(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some(rider_plain))?);
        events.push(format!("planted: a ridden cart on the front {q:?} (it settles on the front's braked slope)"));
    }
    // (ironspec 03:34 (2), the riderless player fault) empty carts at rest on the leg line
    let mut empties: Vec<u32> = Vec::new();
    for p in &plants.empty {
        let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", *p, None)?;
        planted.push(id);
        empties.push(id);
        events.push(format!("planted: an EMPTY cart at rest at ({:.4}, {:.4}, {:.4}) (cart {id})", p[0], p[1], p[2]));
    }
    if plants.no_token {
        events.push("the token's evidence removed: the hold stays powered (every napper passes it unheld)".to_string());
    }
    // (G1 03:34 (1)) the detectors' presses read from the world: each rising edge of the arrival
    // detector's and the departure detector's `powered`
    let (mut arr_was, mut dep_was) = (false, false);
    let mut unpowered: Vec<Vec<u32>> = vec![Vec::new(); plants.watch_powered.len()];
    let (mut arr_presses, mut dep_presses): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
    // (ironspec 03:36 (3)) at each release from the hold: every other cart's (and rider's) box against
    // the column's footprint (the falling cart's plan box, centred in the column's cell)
    let footprint = [arrival[0] as f64 + 0.01, f64::NEG_INFINITY, arrival[2] as f64 + 0.01, arrival[0] as f64 + 0.99, f64::INFINITY, arrival[2] as f64 + 0.99];
    let mut release_checks: Vec<(u32, Vec<String>)> = Vec::new();
    let mut dep_boxes: Vec<(u32, u32, [f64; 6], Option<[f64; 6]>, bool)> = Vec::new();
    let mut frozen: Option<[i32; 4]> = None;
    for t in 0..ticks {
        // (ironspec 02:33 (2), the timing row) a k + j = 3 world open: the part's chunks at level 32 for
        // `len` gt from `fs` (entities frozen, redstone and block ticks running)
        if let Some((fs, len)) = freeze {
            if t == fs {
                frozen = Some(crate::liveness::freeze32(&mut run.world));
            }
            if t == fs + len {
                if let Some(pb) = frozen.take() {
                    crate::liveness::thaw32(&mut run.world, pb);
                }
            }
        }
        // (ironspec 03:47) the token held until a chosen tick, so a napper first rests in the hold
        if plants.token_at == Some(t) && !token {
            token = true;
            events.push(format!("t {t}: the token set (planted: --leg-token-at)"));
        }
        if let Some(h) = hold {
            let support = BlockPos::new(h[0], h[1] - 1, h[2]);
            let want = if token || plants.no_token { redstone_b.clone() } else { glass_b.clone() };
            if run.world.get(support) != want {
                run.world.set_block(support, want, redsim::world::flags::ALL);
            }
        }
        if next_spawn == Some(t) && (cs.len() as u32) < count {
            let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&format!("{{Motion:[{:.4}d,{:.4}d,{:.4}d],Passengers:[{rider_nbt}]}}", motion[0], motion[1], motion[2])))?;
            let rider = run.world.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart)).map(|e| e.id);
            cs.push(C { cart, rider, at: t, arrival: None, departure: None, fell: None, rest: None, hmin: f32::INFINITY, hurt_at: None, hmin_stop: f32::INFINITY, entry_t: None, v_col: None, v_shaft: None, seat_min: f64::INFINITY, wait_ticks: 0, contacts: Vec::new(), h0: None, stamp: 0, off: 0, first_off: None, gone: None, released: false, held_at_hold: 0, passed_hold: false, watch: None, col_check: false, shaft_check: false });
            next_spawn = if (cs.len() as u32) < count && gate.is_none() { Some(t + spacing.max(1)) } else { None };
        }
        if releases.contains(&t) || release_at == Some(t) {
            release(&mut run.world, o)?;
            rearm_at = Some(t + hold_gt);
            release_at = None;
            events.push(format!("t {t}: release (NOT-ready's lever {lever} off)"));
        }
        if rearm_at == Some(t) {
            run.world.set_block(lever, lever_on.clone(), redsim::world::flags::ALL);
            rearm_at = None;
            events.push(format!("t {t}: NOT-ready's lever on again (re-armed)"));
        }
        run.tick();
        let mut waiting = 0usize;
        let n_spawned = cs.len() as u32;
        let mut centres: Vec<Vec3> = Vec::new();
        // every cart's cell this tick (the train's and the planted ones), for the drop checks
        let occ: Vec<(u32, [i32; 3])> = cs
            .iter()
            .map(|c| c.cart)
            .chain(planted.iter().copied())
            .filter_map(|id| run.world.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (id, [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32])))
            .collect();
        // (ironspec 03:36 (3)) every cart's box and its rider's box this tick (the drop's footprint judged on
        // boxes, never on cells: a cart resting on the front reaches 0.377 into the column's cell)
        let occ_boxes: Vec<(u32, [f64; 6], Option<[f64; 6]>)> = cs
            .iter()
            .map(|c| c.cart)
            .chain(planted.iter().copied())
            .filter_map(|id| {
                let e = run.world.entities.list.iter().find(|e| e.id == id && !e.removed)?;
                let rb = run.world.entities.list.iter().find(|r| !r.removed && r.vehicle == Some(id)).map(box_of);
                Some((id, box_of(e), rb))
            })
            .collect();
        let overlapping = |me: u32, fp: &[f64; 6], below_y: f64| -> Vec<String> {
            let mut v = Vec::new();
            for (id, cb, rb) in &occ_boxes {
                if *id == me {
                    continue;
                }
                for (what, b) in std::iter::once(("cart", Some(*cb))).chain(std::iter::once(("its rider", *rb))) {
                    let Some(b) = b else { continue };
                    let (dx, dz) = xz_overlap(fp, &b);
                    if dx > 0.0 && dz > 0.0 && b[1] < below_y {
                        v.push(format!("cart {id}'s {} box x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3} (in the footprint by {dx:.3} x {dz:.3})", if what == "cart" { "own" } else { "rider's" }, b[0], b[3], b[1], b[4], b[2], b[5]));
                    }
                }
            }
            v
        };
        // (G1 03:34 (1)) the detectors' presses, read from the world
        let pressed = |p: P| run.world.get(BlockPos::new(p[0], p[1], p[2])).to_string().contains("powered=true");
        let (arr_now, dep_now) = (pressed(arrival), pressed(departure));
        if arr_now && !arr_was {
            arr_presses.push(t);
        }
        if dep_now && !dep_was {
            dep_presses.push(t);
        }
        (arr_was, dep_was) = (arr_now, dep_now);
        // (ironspec 04:32 / 04:37) the boosters' `powered`, read every tick
        for (k, p) in plants.watch_powered.iter().enumerate() {
            if !pressed(*p) {
                unpowered[k].push(t);
            }
        }
        for c in cs.iter_mut() {
            if c.gone.is_some() {
                continue;
            }
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == c.cart && !e.removed) else {
                c.gone = Some(t);
                continue;
            };
            let (pos, v) = (e.pos, e.delta_movement);
            centres.push(pos);
            let cell = [pos.x.floor() as i32, pos.y.floor() as i32, pos.z.floor() as i32];
            // the hold: a napper at rest in the hold's cell counts its held ticks; one passing the next cell
            // north clears the token (it left the hold toward the drop)
            if let Some(h) = hold {
                if cell == h && (v.x * v.x + v.z * v.z).sqrt() < 0.003 {
                    c.held_at_hold += 1;
                }
                if !c.passed_hold && cell == [h[0], h[1], h[2] - 1] {
                    c.passed_hold = true;
                    hold_passes += 1;
                    token = false;
                    at_hold.push(t);
                    // (ironspec 03:36 (3)) the release: no other cart's (or rider's) box in the column's footprint
                    release_checks.push((t, overlapping(c.cart, &footprint, f64::INFINITY)));
                }
            }
            // (G1 01:35) the drop checks: entering the column (x, z of the landing, above it) and entering the
            // shaft below the rail end: every other cart's cell in the cells below that drop printed
            if !c.col_check && cell[0] == arrival[0] && cell[2] == arrival[2] && cell[1] > arrival[1] {
                c.col_check = true;
                // (ironspec 03:36 (3)) judged on boxes: the falling cart's own plan box as its footprint, every
                // other cart's and rider's box below it
                let me = run.world.entities.list.iter().find(|e| e.id == c.cart && !e.removed).map(box_of);
                let below: Vec<String> = me.map_or(Vec::new(), |fb| overlapping(c.cart, &fb, fb[1]));
                drop_checks.push((t, 0, "the column onto the landing", below));
                // (ironspec 03:10 / 03:23) the y-22 end's exit: the speed at the first tick past the rail end
                c.v_col = Some((v.x * v.x + v.z * v.z).sqrt());
            }
            if !c.shaft_check && cell[0] == sx && cell[2] == sz && cell[1] <= rend[1] {
                c.shaft_check = true;
                let me = run.world.entities.list.iter().find(|e| e.id == c.cart && !e.removed).map(box_of);
                let below: Vec<String> = me.map_or(Vec::new(), |fb| overlapping(c.cart, &fb, fb[1]));
                drop_checks.push((t, 1, "the queue drop's shaft onto the booster", below));
                // (G1 03:34) the queue drop's exit: the speed at the first tick past the rail end
                c.v_shaft = Some((v.x * v.x + v.z * v.z).sqrt());
            }
            // (G1 01:29) the resync detector's cell: each napper's first tick in it (its press tick)
            if let Some(wc) = watch_cell {
                if c.watch.is_none() && cell == wc {
                    c.watch = Some(t);
                }
            }
            if let Some(e) = plants.entry {
                if c.entry_t.is_none() && cell == e {
                    c.entry_t = Some(t);
                }
            }
            if c.arrival.is_none() && cell == arrival {
                c.arrival = Some(t);
            }
            if c.departure.is_none() && cell == departure && c.arrival.is_some() {
                c.departure = Some(t);
                // (G1 03:39b) the departing cart's box (and its rider's) at the departure press, against the
                // column's footprint: past it by construction
                let me_box = occ_boxes.iter().find(|(id, _, _)| *id == c.cart).map(|(_, b, r)| (*b, *r));
                if let Some((b, r)) = me_box {
                    let clear = |x: &[f64; 6]| {
                        let (dx, dz) = xz_overlap(&footprint, x);
                        !(dx > 0.0 && dz > 0.0)
                    };
                    dep_boxes.push((t, c.cart, b, r, clear(&b) && r.as_ref().is_none_or(clear)));
                }
                // the departure press sets the token (the front is free: one release per edge)
                if hold.is_some() {
                    token = true;
                    token_sets += 1;
                }
                // (G1 00:06) the gate: the next napper starts `gate` gt after this one's departure
                if let Some(d) = gate {
                    if n_spawned < count && next_spawn.is_none() {
                        next_spawn = Some(t + 1 + d);
                    }
                }
            }
            if c.fell.is_none() && c.departure.is_some() && pos.y < (rend[1] - 1) as f64 {
                c.fell = Some(t);
            }
            let rail_here = redsim::rails::is_rail(&run.world.get(BlockPos::new(cell[0], cell[1], cell[2]))) || redsim::rails::is_rail(&run.world.get(BlockPos::new(cell[0], cell[1] - 1, cell[2])));
            // off a rail between its arrival and its departure (the queue drop after the departure is a
            // designed fall, judged by the drop BAR)
            if c.arrival.is_some() && c.departure.is_none() && !rail_here {
                c.off += 1;
                c.first_off.get_or_insert((t, cell));
            }
            let speed = (v.x * v.x + v.z * v.z).sqrt();
            // (G1 04:41 (2)) the waiting seat: while the napper is on the front (arrived, not departed), its
            // RIDER's distance to the bed head each tick (both push phases and any creep over the wait)
            if c.arrival.is_some() && c.departure.is_none() && cell == q {
                c.wait_ticks += 1;
                if let (Some(bh), Some(r)) = (plants.bed_head, c.rider) {
                    if let Some(e) = run.world.entities.list.iter().find(|x| !x.removed && x.id == r) {
                        let d = ((e.pos.x - bh[0]).powi(2) + (e.pos.y - bh[1]).powi(2) + (e.pos.z - bh[2]).powi(2)).sqrt();
                        c.seat_min = c.seat_min.min(d);
                    }
                }
            }
            // (ironspec 04:32: the stop wall's question) every contact the cart makes from its release from the
            // hold until it lands, through the shared contact rule
            if c.passed_hold && c.arrival.is_none() {
                if let Some(i) = run.world.entities.list.iter().position(|x| !x.removed && x.id == c.cart) {
                    for h in crate::contacts::cart_hits(&run.world, i) {
                        c.contacts.push(format!("t {t} ({},{},{}) face {} depth {:.4} speed {:.4}->{:.4}", h.cell.x, h.cell.y, h.cell.z, h.face.name(), h.depth, h.speed_in, h.speed_out));
                    }
                    for h in crate::contacts::cart_vertical_hits(&run.world, i) {
                        c.contacts.push(format!("t {t} ({},{},{}) {:?} depth {:.4} speed {:.4}->{:.4}", h.cell.x, h.cell.y, h.cell.z, h.class, h.depth, h.speed_in, h.speed_out));
                    }
                }
            }
            if c.arrival.is_some() && c.departure.is_none() && speed < 0.003 {
                waiting += 1;
                c.rest = Some((t, cell));
                // a release `release_after` gt after a napper comes to rest on the queue cell (the stop
                // emulated free), once a napper, never while a release is under way
                if let Some(a) = release_after {
                    if cell == q && !c.released && release_at.is_none() && rearm_at.is_none() {
                        c.released = true;
                        release_at = Some(t + 1 + a);
                    }
                }
            }
            if let Some(r) = c.rider {
                if let Some(i) = run.world.entities.list.iter().position(|x| !x.removed && x.id == r) {
                    if let EntityKind::Mob(m) = &run.world.entities.list[i].kind {
                        c.h0.get_or_insert(m.health);
                        if c.hurt_at.is_none() && c.h0.is_some_and(|h| m.health < h) {
                            c.hurt_at = Some((t, cell));
                        }
                        // (the stop's entry given) the way-in's health is judged until the napper reaches the
                        // stop's entry; a hurt after it is the stop's, printed apart
                        if c.entry_t.is_none() {
                            c.hmin = c.hmin.min(m.health);
                        } else {
                            c.hmin_stop = c.hmin_stop.min(m.health);
                        }
                    }
                    if redsim::entity::mob::ai::brain::has_exact(&run.world, i) {
                        if let Some(s) = redsim::entity::mob::ai::brain::ex(&run.world, i).mem.cant_reach_since {
                            // (ironspec 23:57) the stamp's age while the napper WAITS (until its departure);
                            // after it the stop's rows judge it
                            if c.departure.is_none() {
                                c.stamp = c.stamp.max(run.world.time - s);
                            }
                        }
                    }
                }
            }
        }
        // (G1 02:54) the landing (the napper drop's and the riderless off-the-end fall's): no ridden cart
        // ever at rest on it (any tick), each first rest printed by cart
        for id in cs.iter().map(|c| c.cart).chain(planted.iter().copied()) {
            if landing_rests.iter().any(|(_, x, _)| *x == id) {
                continue;
            }
            if let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed) {
                let cell = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
                let s = (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt();
                let ridden = run.world.entities.list.iter().any(|r| !r.removed && r.vehicle == Some(id));
                if cell == arrival && s < 0.003 && ridden {
                    landing_rests.push((t, id, format!("({:.4}, {:.4}, {:.4})", e.pos.x, e.pos.y, e.pos.z)));
                }
            }
        }
        waiting_max = waiting_max.max(waiting);
        for a in 0..centres.len() {
            for b in a + 1..centres.len() {
                let d = ((centres[a].x - centres[b].x).powi(2) + (centres[a].y - centres[b].y).powi(2) + (centres[a].z - centres[b].z).powi(2)).sqrt();
                dmin = dmin.min(d);
            }
        }
    }
    let mut out = vec![format!(
        "(leg train) {count} napper(s) down the leg from ({:.4}, {:.4}, {:.4}) at ({:.4}, {:.4}, {:.4}) b/t, {} (HOME the bed {bed:?}, the day time {day}); the built R1 released at t {releases:?}{} (NOT-ready's lever off {hold_gt} gt, then on); followed {ticks} gt; the arrival detector {arrival:?}, the queue {q:?}, the departure detector {departure:?}",
        start[0], start[1], start[2], motion[0], motion[1], motion[2],
        match gate {
            Some(d) => format!("each next one started {d} gt after the previous one passes the departure detector (G1 00:06: the gate 'the previous cart has left')"),
            None => format!("one every {spacing} gt"),
        },
        release_after.map_or(String::new(), |a| format!(" and {a} gt after each napper comes to rest on the queue (the stop emulated free)"))
    )];
    for (k, c) in cs.iter().enumerate() {
        out.push(format!(
            "(leg train) napper {k} (spawned t {}): arrival t {}, waits {}, departure t {}, in the shaft t {}; least health {} (of {}); its stamp's largest age while waiting {} gt; off a rail before its departure {} tick(s){}{}",
            c.at,
            c.arrival.map_or("-".to_string(), |x| x.to_string()),
            c.rest.map_or("never at rest".to_string(), |(t, p)| format!("in {p:?} (last at rest t {t})")),
            c.departure.map_or("-".to_string(), |x| x.to_string()),
            c.fell.map_or("-".to_string(), |x| x.to_string()),
            if c.hmin.is_finite() { format!("{:.1}", c.hmin) } else { "-".to_string() },
            c.h0.map_or("-".to_string(), |h| format!("{h:.1}"))
                + &c.hurt_at.map_or(String::new(), |(t, p)| format!(", first hurt at t {t} in {p:?}{}", if c.entry_t.is_some_and(|e| t >= e) { " (IN THE STOP, after its entry)" } else { "" }))
                + &c.entry_t.map_or(String::new(), |e| format!("; at the stop's entry t {e}{}", if c.hmin_stop.is_finite() { format!(", least health in the stop {:.1} (the stop's, not judged here)", c.hmin_stop) } else { String::new() }))
                + &c.v_col.map_or(String::new(), |v| format!("; the y-22 end's exit {v:.4} b/t"))
                + &c.v_shaft.map_or(String::new(), |v| format!("; the queue drop's exit {v:.4} b/t")),
            c.stamp,
            c.off,
            c.first_off.map_or(String::new(), |(t, p)| format!(" (first at t {t} in {p:?})")),
            match hold {
                Some(_) => format!("; held at the hold {} tick(s){}", c.held_at_hold, if c.passed_hold { "" } else { ", NEVER passed it" }),
                None => String::new(),
            } + &watch_cell.map_or(String::new(), |w| format!("; first in {w:?} at t {}", c.watch.map_or("-".to_string(), |t| t.to_string())))
                + &c.gone.map_or(String::new(), |t| format!("; the cart GONE at t {t}"))
                + &(if c.wait_ticks > 0 && c.seat_min.is_finite() { format!("; on the front {} gt, its rider's least distance to the bed head {:.3}", c.wait_ticks, c.seat_min) } else { String::new() })
                + &run.world.entities.list.iter().find(|e| e.id == c.cart && !e.removed).map_or(String::new(), |e| {
                    format!("; ENDS at ({:.4}, {:.4}, {:.4}) in {:?} moving {:.4} b/t", e.pos.x, e.pos.y, e.pos.z, [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32], (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt())
                })
                + &(if c.contacts.is_empty() { if c.passed_hold { "; no contact from the hold to the landing".to_string() } else { String::new() } } else { format!("; contacts from the hold to the landing: {}", c.contacts.join(", ")) })
        ));
    }
    let arrived = cs.iter().filter(|c| c.arrival.is_some()).count();
    let departed: Vec<(u32, usize)> = {
        let mut d: Vec<(u32, usize)> = cs.iter().enumerate().filter_map(|(k, c)| c.departure.map(|t| (t, k))).collect();
        d.sort();
        d
    };
    let arrivals: Vec<usize> = {
        let mut a: Vec<(u32, usize)> = cs.iter().enumerate().filter_map(|(k, c)| c.arrival.map(|t| (t, k))).collect();
        a.sort();
        a.into_iter().map(|(_, k)| k).collect()
    };
    let fifo = departed.iter().map(|(_, k)| *k).eq(arrivals.iter().copied().take(departed.len()));
    let off_total: u32 = cs.iter().map(|c| c.off).sum();
    let hurt = cs.iter().filter(|c| c.h0.is_some_and(|h| c.hmin < h)).count();
    let stamp_max = cs.iter().map(|c| c.stamp).max().unwrap_or(0);
    let gone = cs.iter().filter(|c| c.gone.is_some() && c.fell.is_none()).count();
    let all_depart = (releases.is_empty() && release_after.is_none()) || departed.len() == cs.len();
    let bars = [
        (format!("every napper arrives ({arrived} of {})", cs.len()), arrived == cs.len() && cs.len() as u32 == count),
        (format!("none off a rail between its arrival and its departure ({off_total} tick(s))"), off_total == 0),
        (format!("no cart lost before its fall ({gone})"), gone == 0),
        (
            format!(
                "every rider unhurt on the way-in{} ({hurt} hurt)",
                if plants.entry.is_some() { ", up to the stop's entry (a hurt in the stop is the stop's, printed per napper)" } else { "" }
            ),
            hurt == 0,
        ),
        (format!("the departures in the arrivals' order (FIFO): departures {:?}, arrivals {arrivals:?}", departed.iter().map(|(_, k)| *k).collect::<Vec<_>>()), fifo),
        (format!("every stamp under 1,200 gt while its napper waits, until its departure (the largest {stamp_max} gt)"), stamp_max < 1200),
        (format!("with releases, every napper departs ({} of {})", departed.len(), cs.len()), all_depart),
        // (ironspec 00:54, G1 00:12) one release per departure edge: the hold passed at most once per token
        // set (the first napper on the token's start)
        (
            match hold {
                Some(_) => format!("the hold releases one napper per token (passes {hold_passes} <= the token's start + {token_sets} departure press(es)): passes at t {at_hold:?}"),
                None => "no hold laid (one release per edge not judged)".to_string(),
            },
            hold.is_none() || hold_passes <= 1 + token_sets,
        ),
        // (G1 01:35) no landing holds a ridden cart while another cart can fall onto it
        (
            format!(
                "every drop's landing and the cells below it free of any other cart at the drop ({} drop(s) checked: {} into the column, {} into the queue's shaft){}",
                drop_checks.len(),
                drop_checks.iter().filter(|d| d.1 == 0).count(),
                drop_checks.iter().filter(|d| d.1 == 1).count(),
                {
                    let bad: Vec<String> = drop_checks.iter().filter(|d| !d.3.is_empty()).map(|d| format!("t {} {}: OCCUPIED by {}", d.0, d.2, d.3.join(", "))).collect();
                    if bad.is_empty() { String::new() } else { format!("; {}", bad.join("; ")) }
                }
            ),
            !drop_checks.is_empty() && drop_checks.iter().all(|d| d.3.is_empty()),
        ),
        (
            format!(
                "no ridden cart ever at rest on the landing {arrival:?} (the napper drop's and the riderless off-the-end fall's landing, G1 02:54){}",
                if landing_rests.is_empty() { String::new() } else { format!(": RESTED on the landing: {}", landing_rests.iter().map(|(t, id, p)| format!("cart {id} at t {t} at {p}")).collect::<Vec<_>>().join("; ")) }
            ),
            landing_rests.is_empty(),
        ),
        // (ironspec 03:36 (3)) each release from the hold waits until no other cart's or rider's box is in the
        // column's footprint (not merely the detector's cell)
        (
            format!(
                "at every release from the hold, no other cart's or rider's box in the column's footprint x {:.2}..{:.2} z {:.2}..{:.2} ({} release(s) checked){}",
                footprint[0],
                footprint[3],
                footprint[2],
                footprint[5],
                release_checks.len(),
                {
                    let bad: Vec<String> = release_checks.iter().filter(|r| !r.1.is_empty()).map(|r| format!("t {}: {}", r.0, r.1.join(", "))).collect();
                    if bad.is_empty() { String::new() } else { format!(": IN THE FOOTPRINT at {}", bad.join("; ")) }
                }
            ),
            release_checks.iter().all(|r| r.1.is_empty()),
        ),
        // (G1 03:34 (1)) the detectors read pressed in the world: the arrival detector once per arrival, the
        // departure detector once per departure
        (
            format!(
                "the arrival detector {arrival:?} pressed once per arrival ({} press(es) at t {arr_presses:?}, {} arrival(s)) and the departure detector {departure:?} once per departure ({} press(es) at t {dep_presses:?}, {} departure(s))",
                arr_presses.len(),
                cs.iter().filter(|c| c.arrival.is_some()).count(),
                dep_presses.len(),
                cs.iter().filter(|c| c.departure.is_some()).count()
            ),
            arr_presses.len() == cs.iter().filter(|c| c.arrival.is_some()).count() && dep_presses.len() == cs.iter().filter(|c| c.departure.is_some()).count(),
        ),
        // (G1 03:39b) at every departure press the departing cart's and rider's boxes are past the footprint
        (
            format!(
                "at every departure press the departing cart's and rider's boxes are clear of the column's footprint (z {:.2}..{:.2}): {}",
                footprint[2],
                footprint[5],
                if dep_boxes.is_empty() {
                    "no departure".to_string()
                } else {
                    dep_boxes
                        .iter()
                        .map(|(t, id, b, r, ok)| format!("t {t} cart {id} box z {:.3}..{:.3}{}{}", b[2], b[5], r.map_or(String::new(), |r| format!(", rider z {:.3}..{:.3}", r[2], r[5])), if *ok { "" } else { " IN THE FOOTPRINT" }))
                        .collect::<Vec<_>>()
                        .join("; ")
                }
            ),
            dep_boxes.iter().all(|d| d.4),
        ),
    ];
    let pass = bars.iter().all(|(_, ok)| *ok);
    // (ironspec 04:32 / 04:37) the boosters' per-tick read
    for (k, p) in plants.watch_powered.iter().enumerate() {
        out.push(format!(
            "(leg train) {p:?} read `powered` every tick: {}",
            if unpowered[k].is_empty() { format!("POWERED on all {ticks} tick(s)") } else { format!("UNPOWERED on {} tick(s) (first t {:?})", unpowered[k].len(), unpowered[k].iter().take(8).collect::<Vec<_>>()) }
        ));
    }
    // (ironspec 03:13 / 03:23 (6)) the two designed falls' least exit speed (ridden nappers) against the game's
    // all-phase band: 0.25 ridden, 0.27 empty
    for (name, vs) in [("the y-22 end (9,22,-4) -> the column", cs.iter().filter_map(|c| c.v_col).collect::<Vec<f64>>()), ("the queue drop's rail end -> the shaft", cs.iter().filter_map(|c| c.v_shaft).collect::<Vec<f64>>())] {
        if let Some(least) = vs.iter().copied().reduce(f64::min) {
            out.push(format!("(leg train) {name}: the least exit speed over {} ridden napper(s) {least:.4} b/t (the first tick past the rail end): {:.2}x the ridden all-phase bound 0.25, {:.2}x the empty 0.27", vs.len(), least / 0.25, least / 0.27));
        }
    }
    out.push(format!("(leg train) at most {waiting_max} napper(s) waiting at rest at once; the least distance between two carts {}", if dmin.is_finite() { format!("{dmin:.4}") } else { "-".to_string() }));
    for (name, ok) in &bars {
        out.push(format!("(leg train) BAR {name}: {}", if *ok { "PASSES" } else { "FAILS" }));
    }
    // the planted empty carts' fate: where each ends (still on the leg line, down the column, on the front)
    for id in &empties {
        let fate = run.world.entities.list.iter().find(|e| e.id == *id && !e.removed).map_or("GONE".to_string(), |e| {
            format!("ends at ({:.4}, {:.4}, {:.4}) in {:?} moving {:.4} b/t", e.pos.x, e.pos.y, e.pos.z, [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32], (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt())
        });
        out.push(format!("(leg train) the planted EMPTY cart {id}: {fate}"));
    }
    out.push(format!("(leg train) the releases: {}", if events.is_empty() { "none".to_string() } else { events.join("; ") }));
    Ok((override_lines.into_iter().chain(std::iter::once(fluid_line)).chain(out).collect(), pass))
}

/// (R6; ironspec 01:44b (1), G1 01:46) The hold's sweep: one napper a run down the leg at each start speed
/// (and phase along its start cell), the hold braked: BRAKING (it comes to rest within the hold's cell, no
/// overrun into the next cell `ahead`; its rest position printed), then the hold powered: RELAUNCH (the
/// tick it reaches `ahead`, gravity only), the speed it leaves the line's end at (`edge`, the cell off the
/// y-22 line's end: fed to the column's drop) and its landing on the arrival detector (the rider's
/// health). BAR: every run stops within the cell, relaunches within `relaunch_bound` gt, lands on the
/// arrival detector unhurt.
#[allow(clippy::too_many_arguments)]
pub fn hold_sweep_lines(spec: &FarmSpec, o: &Opts, start: [f64; 3], heading: [f64; 3], speeds: &[f64], phases: u32, hold: P, ahead: P, edge: P, relaunch_bound: u32, empty: bool) -> Result<(Vec<String>, bool), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 1200, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o)?);
    let override_lines = place_overrides(&mut run.world, o.extra.len(), &cells)?;
    settle_r1(&mut run.world, o)?;
    let (fluid_line, fluid_ok) = fluids_line(&mut run.world, o.chunk, "hold sweep");
    if !fluid_ok {
        return Err(fluid_line);
    }
    // the hold: a powered rail in its cell's rail shape, on glass (braked); powered = a redstone block in
    // place of its support (the token's output, emulated)
    let hp = BlockPos::new(hold[0], hold[1], hold[2]);
    let support = BlockPos::new(hold[0], hold[1] - 1, hold[2]);
    let here = run.world.get(hp).to_string();
    let shape = here.split("shape=").nth(1).and_then(|s| s.split([',', ']']).next()).ok_or_else(|| format!("the hold {hold:?} holds {here}, no rail shape"))?.to_string();
    run.world.set_block(hp, Block::parse(&format!("minecraft:powered_rail[powered=false,shape={shape},waterlogged=false]")).map_err(|e| e.to_string())?, redsim::world::flags::ALL);
    run.world.set_block(support, Block::parse("minecraft:glass").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
    let redstone = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let arrival = [q[0], q[1] + 1, q[2] - 1];
    let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
    let mut out = vec![format!(
        "(hold sweep) the hold {hold:?} a powered rail (shape {shape}) braked on glass, then powered (a redstone block at {support}); one napper a run from ({:.4}, {:.4}, {:.4}) heading ({}, {}, {}) at {} speed(s) {:.4}..{:.4} x {} phase(s) along its start cell; the cell ahead {ahead:?}, the line's end {edge:?}, the landing {arrival:?}",
        start[0],
        start[1],
        start[2],
        heading[0],
        heading[1],
        heading[2],
        speeds.len(),
        speeds.first().copied().unwrap_or(0.0),
        speeds.last().copied().unwrap_or(0.0),
        phases.max(1)
    )];
    if empty {
        out.push("(hold sweep) the runs' carts are EMPTY (no rider): the riderless player fault (ironspec 03:34 (2)); the speeds and stops are an empty cart's".to_string());
    }
    let (mut runs, mut fails) = (0u32, Vec::<String>::new());
    let (mut rest_z, mut relaunch_t, mut edge_v): (Vec<f64>, Vec<u32>, Vec<f64>) = (Vec::new(), Vec::new(), Vec::new());
    let mut per_run: Vec<String> = Vec::new();
    let mut contact_agg: std::collections::BTreeMap<String, (std::collections::BTreeSet<u32>, f64, f64, f64)> = std::collections::BTreeMap::new();
    let mut fastest_stop: Option<f64> = None;
    for k in 0..phases.max(1) {
        // the phase moves the start along its heading within one cell
        let f = (k as f64 + 0.5) / phases.max(1) as f64 - 0.5;
        let p0 = [start[0] + heading[0] * f, start[1], start[2] + heading[2] * f];
        for &v in speeds {
            runs += 1;
            let mut r = run.clone();
            // (ironspec 03:34 (2)) with `empty` the cart carries no rider (the riderless player fault)
            let nbt = if empty { format!("{{Motion:[{:.6}d,0.0d,{:.6}d]}}", heading[0] * v, heading[2] * v) } else { format!("{{Motion:[{:.6}d,0.0d,{:.6}d],{}", heading[0] * v, heading[2] * v, &rider[1..]) };
            let cart = redsim::entity::spawn(&mut r.world, "minecraft:minecart", p0, Some(&nbt))?;
            let rid = r.world.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart)).map(|e| e.id);
            let cell_of = |w: &redsim::world::World| w.entities.list.iter().find(|e| e.id == cart && !e.removed).map(|e| ([e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32], e.pos, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt()));
            // braking: run until at rest 5 ticks in a row (or 400 gt); an overrun into `ahead` fails
            let (mut still, mut overrun, mut rested) = (0u32, false, None::<(P, f64)>);
            for _ in 0..400 {
                r.tick();
                let Some((c, p, s)) = cell_of(&r.world) else { break };
                if c == ahead {
                    overrun = true;
                    break;
                }
                still = if s < 0.003 { still + 1 } else { 0 };
                if still >= 5 {
                    rested = Some((c, p.z));
                    break;
                }
            }
            let Some((rc, rz)) = rested.filter(|(c, _)| *c == hold) else {
                fails.push(format!("v {v:.4} phase {k}: {}", if overrun { "OVERRAN the hold into the cell ahead".to_string() } else { format!("did not rest in the hold ({:?})", rested.map(|x| x.0)) }));
                continue;
            };
            let _ = rc;
            rest_z.push(rz);
            fastest_stop = Some(fastest_stop.map_or(v, |x: f64| x.max(v)));
            // relaunch: the hold powered
            r.world.set_block(support, redstone.clone(), redsim::world::flags::ALL);
            let (mut t_ahead, mut v_edge, mut landed, mut hmin) = (None::<u32>, None::<f64>, None::<u32>, f32::INFINITY);
            let mut last: Option<(P, Vec3, f64)> = None;
            for t in 0..400u32 {
                r.tick();
                if let Some(i) = rid.and_then(|id| r.world.entities.list.iter().position(|e| !e.removed && e.id == id)) {
                    if let EntityKind::Mob(m) = &r.world.entities.list[i].kind {
                        hmin = hmin.min(m.health);
                    }
                }
                let Some((c, pos_now, s)) = cell_of(&r.world) else { break };
                last = Some((c, pos_now, s));
                // (ironspec 04:32: the stop wall's question) every contact from the release to the landing,
                // aggregated per cell and face over the runs
                if let Some(i) = r.world.entities.list.iter().position(|e| !e.removed && e.id == cart) {
                    let hits: Vec<(String, f64, f64, f64)> = crate::contacts::cart_hits(&r.world, i)
                        .into_iter()
                        .map(|h| (format!("({},{},{}) face {}", h.cell.x, h.cell.y, h.cell.z, h.face.name()), h.depth, h.speed_in, h.speed_out))
                        .chain(crate::contacts::cart_vertical_hits(&r.world, i).into_iter().map(|h| (format!("({},{},{}) {:?}", h.cell.x, h.cell.y, h.cell.z, h.class), h.depth, h.speed_in, h.speed_out)))
                        .collect();
                    for (key, depth, sin, sout) in hits {
                        let e = contact_agg.entry(key).or_insert((std::collections::BTreeSet::new(), 0.0f64, 0.0f64, f64::INFINITY));
                        e.0.insert(runs);
                        e.1 = e.1.max(depth);
                        e.2 = e.2.max(sin);
                        e.3 = e.3.min(sout);
                    }
                }
                if t_ahead.is_none() && c == ahead {
                    t_ahead = Some(t + 1);
                }
                if v_edge.is_none() && c == edge {
                    v_edge = Some(s);
                }
                if landed.is_none() && c == arrival {
                    landed = Some(t + 1);
                    break;
                }
            }
            match (t_ahead, landed) {
                (Some(ta), Some(_)) if ta <= relaunch_bound && hmin >= 20.0 => {
                    relaunch_t.push(ta);
                    if let Some(e) = v_edge {
                        edge_v.push(e);
                    }
                    // (G1 03:12) every run's own figures: the rest z, the relaunch time, the speed at the
                    // first tick past the rail end (a conservative reading of the speed at the end)
                    per_run.push(format!("v {v:.2} p{k}: z {rz:.4}, {ta} gt, end {}", v_edge.map_or("-".to_string(), |e| format!("{e:.4}"))));
                }
                _ => fails.push(format!(
                    "v {v:.4} phase {k}: rested at z {rz:.4}; relaunch {} (bound {relaunch_bound} gt); landed {}; least health {}; the speed off the line's end {}; ENDS in {}",
                    t_ahead.map_or("NEVER reached the cell ahead".to_string(), |t| format!("reached the cell ahead at t {t}")),
                    landed.map_or("NO".to_string(), |t| format!("at t {t}")),
                    if hmin.is_finite() { format!("{hmin:.1}") } else { "-".to_string() },
                    v_edge.map_or("- (never off it)".to_string(), |e| format!("{e:.4} b/t")),
                    last.map_or("-".to_string(), |(c, p, s)| format!("{c:?} at ({:.4}, {:.4}, {:.4}) moving {s:.4} b/t", p.x, p.y, p.z))
                )),
            }
        }
    }
    let span = |v: &[f64]| if v.is_empty() { "-".to_string() } else { format!("{:.4}..{:.4}", v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max)) };
    out.push(format!(
        "(hold sweep) {runs} run(s): BRAKING the rest z in the hold {} (the fastest start speed that stops within the cell {}); RELAUNCH the cell ahead reached {} gt after the power; the speed leaving the line's end {} b/t (fed to the column's drop: the stop wall confines these speeds)",
        span(&rest_z),
        fastest_stop.map_or("-".to_string(), |v| format!("{v:.4}")),
        if relaunch_t.is_empty() { "-".to_string() } else { format!("{}..{}", relaunch_t.iter().min().unwrap_or(&0), relaunch_t.iter().max().unwrap_or(&0)) },
        span(&edge_v)
    ));
    let pass = fails.is_empty() && runs > 0;
    out.push(format!(
        "(hold sweep) contacts from the release to the landing (the shared contact rule): {}",
        if contact_agg.is_empty() {
            "NONE in any run".to_string()
        } else {
            contact_agg.iter().map(|(k, (rs, d, si, so))| format!("{k}: {} of {runs} run(s), depth <= {d:.4}, speed_in <= {si:.4}, speed_out >= {so:.4}", rs.len())).collect::<Vec<_>>().join("; ")
        }
    ));
    for chunk in per_run.chunks(6) {
        out.push(format!("(hold sweep) per run: {}", chunk.join("; ")));
    }
    out.push(format!(
        "(hold sweep) BAR every run stops within the hold, relaunches within {relaunch_bound} gt of the power and lands on the arrival detector unhurt: {}{}",
        if pass { "PASSES".to_string() } else { format!("FAILS {} of {runs}", fails.len()) },
        if fails.is_empty() { String::new() } else { format!(" ({})", fails.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    Ok((override_lines.into_iter().chain(std::iter::once(fluid_line)).chain(out).collect(), pass))
}

/// The way-in line's cells laid into the module file (a WIP design against its frame): the queue
/// cell (with `rail`, the three rails of the staircase on their glass and glowstone; else the
/// shut iron trapdoor), the extra cells (the leg, the walls, the run), the post's glass (and its
/// leaves unless the post is custom), the queue cart with its rider and the post's boat with its
/// zombie (persistent). Refuses a cell the module already holds unless the extra cell replaces it.
pub fn write_layout(module: &std::path::Path, out: &std::path::Path, o: &Opts, post: P) -> Result<Vec<String>, String> {
    let mut d = mctas::design::Design::load(module).map_err(|e| e.to_string())?;
    let q = o.queue;
    d.meta.insert(
        "way-in".to_string(),
        format!(
            "b8949c96 way-cell --write: the queue rail {:?} ({}), the post {:?} (a zombie in an oak boat{}), {} extra cell(s) (the leg, the walls, the run), laid on {} (WIP, bench, tool unreviewed)",
            q,
            if !o.rail { "a trapdoor" } else if o.south { "the staircase descending south" } else { "the staircase descending west" },
            post,
            if o.custom_post { ", its cells among the extra cells" } else { ", a glass floor and a leaves roof" },
            o.extra.len(),
            module.display()
        ),
    );
    let mut cells: Vec<(P, &str, &str)> = if o.rail {
        queue_cells(o)
    } else {
        vec![(q, "minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]", "the queue cell's trapdoor (the cart rests on it)")]
    };
    cells.extend(o.extra.iter().map(|(p, b)| (*p, b.as_str(), "an extra cell (the run, an occluder)")));
    if !o.custom_post {
        cells.extend([
            ([post[0], post[1] - 1, post[2]], "minecraft:glass", "the post's floor (no golem floor)"),
            ([post[0], post[1] + 3, post[2]], "minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]", "over the zombie's eye (no sky)"),
        ]);
    }
    let mut lines = Vec::new();
    for (p, b, what) in &cells {
        if let Some(have) = d.blocks.get(p) {
            if have.as_str() == *b {
                // (already laid: a module that carries the way-in as a part)
                continue;
            }
            if !o.replace.contains(p) {
                return Err(format!("{p:?} ({what}) already holds {have}"));
            }
            if *b == "minecraft:air" {
                lines.push(format!("  {p:?} REMOVED ({what}; the module's {have})"));
                d.blocks.remove(p);
                continue;
            }
            lines.push(format!("  {p:?} {b} ({what}; REPLACES the module's {have})"));
            d.blocks.insert(*p, b.to_string());
            continue;
        }
        if *b == "minecraft:air" {
            continue;
        }
        d.blocks.insert(*p, b.to_string());
        lines.push(format!("  {p:?} {b} ({what})"));
    }
    let cart = mctas::design::EntitySpec { kind: "minecraft:minecart".into(), pos: [q[0] as f64 + 0.5, q[1] as f64 + if o.rail { 0.5625 } else { 0.1875 }, q[2] as f64 + 0.5], nbt: Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".into()) };
    let boat = mctas::design::EntitySpec { kind: "minecraft:oak_boat".into(), pos: [post[0] as f64 + 0.5, post[1] as f64, post[2] as f64 + 0.5], nbt: Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into()) };
    for e in [cart, boat] {
        let have = d.entities.iter().any(|x| x.kind == e.kind && (0..3).all(|k| (x.pos[k] - e.pos[k]).abs() < 0.01));
        if have {
            lines.push(format!("  entity {} {:?} (already in the module)", e.kind, e.pos));
            continue;
        }
        lines.push(format!("  entity {} {:?} ({})", e.kind, e.pos, if e.kind.ends_with("minecart") { "the seated queue rider" } else { "the queue's zombie" }));
        d.entities.push(e);
    }
    // (G1 07:16 (8)) the module's raw text read BEFORE the save (an in-place write would otherwise read
    // back its own output)
    let raw = std::fs::read_to_string(module).map_err(|e| e.to_string())?;
    d.save(out).map_err(|e| e.to_string())?;
    // (b8949c96 03:05, ironspec 03:01 (4): a rebase onto a record keeps the record's declarations)
    // the design format's saver drops `#` lines, so the written file's header is the module's own
    // raw header (its comment lines and keys in their order) with the saved data: a line the save
    // kept stays in place, a single-valued key the save changed takes the saved line in its place
    // (the `way-in:` line among them), a data line the save no longer carries is dropped (listed),
    // and every other saved line follows; the body as saved
    let saved = std::fs::read_to_string(out).map_err(|e| e.to_string())?;
    let split = |t: &str, what: &str| -> Result<(Vec<String>, Vec<String>), String> {
        let ls: Vec<String> = t.lines().map(|l| l.to_string()).collect();
        // (G1 07:16 (8)) the body starts at the `palette:` line: a file without one is refused (no
        // splice at an assumed place)
        let k = ls.iter().position(|l| l == "palette:").ok_or(format!("{what} has no `palette:` line: the header / body splice refused"))?;
        Ok((ls[..k].to_vec(), ls[k..].to_vec()))
    };
    let (raw_head, _) = split(&raw, &format!("the module {}", module.display()))?;
    let (saved_head, saved_body) = split(&saved, &format!("the saved file {}", out.display()))?;
    let key_of = |l: &str| -> Option<String> {
        if l.starts_with('#') {
            None
        } else {
            l.split_once(':').map(|(k, _)| k.trim().to_string())
        }
    };
    let mut head: Vec<String> = Vec::new();
    let mut used = vec![false; saved_head.len()];
    let (mut changed, mut dropped): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for l in &raw_head {
        if l.starts_with('#') || l.trim().is_empty() {
            head.push(l.clone());
            continue;
        }
        if let Some(i) = (0..saved_head.len()).find(|&i| !used[i] && saved_head[i] == *l) {
            used[i] = true;
            head.push(l.clone());
            continue;
        }
        if let Some(k) = key_of(l) {
            let idx: Vec<usize> = (0..saved_head.len()).filter(|&i| key_of(&saved_head[i]).as_deref() == Some(k.as_str())).collect();
            if idx.len() == 1 && !used[idx[0]] {
                used[idx[0]] = true;
                head.push(saved_head[idx[0]].clone());
                changed.push(k);
                continue;
            }
        }
        dropped.push(l.clone());
    }
    let mut added = 0usize;
    for (i, l) in saved_head.iter().enumerate() {
        if !used[i] {
            head.push(l.clone());
            added += 1;
        }
    }
    let kept = raw_head.iter().filter(|l| l.starts_with('#')).count();
    std::fs::write(out, format!("{}\n{}\n", head.join("\n"), saved_body.join("\n"))).map_err(|e| e.to_string())?;
    lines.insert(
        0,
        format!(
            "wrote {} (the module {} with the way-in line's queue cell and post; the module's header kept, its {kept} comment line(s) included; key(s) the save changed, in place: {}; data line(s) the save no longer carries, dropped: {}; saved line(s) added: {added}):",
            out.display(),
            module.display(),
            if changed.is_empty() { "none".to_string() } else { changed.join(", ") },
            if dropped.is_empty() { "none".to_string() } else { dropped.join(" | ") }
        ),
    );
    Ok(lines)
}

/// The cells `parse_extra` marks with a leading `!` (replacing the module's block there).
pub fn parse_replace(s: Option<&str>) -> Result<Vec<P>, String> {
    let Some(s) = s else { return Ok(Vec::new()) };
    let mut v = Vec::new();
    for c in s.split(';').map(|c| c.trim()).filter(|c| c.starts_with('!')) {
        let (p, _) = c[1..].split_once('=').ok_or(format!("{c:?}: !x,y,z=block"))?;
        let n: Vec<i32> = p.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("{c:?}: {e}"))).collect::<Result<_, _>>()?;
        if n.len() == 3 {
            v.push([n[0], n[1], n[2]]);
        }
    }
    Ok(v)
}

/// Every candidate post: free cells of the chunk 6.05..7.95 (by cell centres) from the queue
/// cell, each checked whole; the passing ones.
pub fn scan(spec: &FarmSpec, o: &Opts) -> Result<Vec<String>, String> {
    let q = Vec3::new(o.queue[0] as f64 + 0.5, o.queue[1] as f64, o.queue[2] as f64 + 0.5);
    let [x0, z0, x1, z1] = o.chunk;
    let mut out = Vec::new();
    let mut tried = 0usize;
    let mut near: Vec<(usize, String)> = Vec::new();
    for y in (o.queue[1] - 8)..=(o.queue[1] + 8) {
        for z in z0..=z1 {
            for x in x0..=x1 {
                let d = Vec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5).distance_to_sqr(q).sqrt();
                if !(6.1..7.9).contains(&d) {
                    continue;
                }
                tried += 1;
                let (ok, lines) = post_lines(spec, o, [x, y, z])?;
                if ok {
                    out.push(format!("PASS post ({x},{y},{z}) at {d:.2}: {}", lines.join(" | ")));
                } else {
                    let fails: Vec<&String> = lines.iter().filter(|l| l.contains("FAIL")).collect();
                    near.push((fails.len(), format!("post ({x},{y},{z}) at {d:.2}: {}", fails.iter().map(|l| l.as_str()).collect::<Vec<_>>().join(" | "))));
                }
            }
        }
    }
    out.insert(0, format!("scan: {tried} candidate post(s) 6.1..7.9 from the queue cell inside the chunk, {} pass every check", out.len()));
    if out.len() == 1 {
        near.sort_by_key(|(n, s)| (*n, s.len()));
        out.push("the nearest misses (fewest failing lines):".to_string());
        for (_, s) in near.iter().take(12) {
            out.push(format!("  {s}"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (ironspec 08:20 / 08:22) the record of SetWalkTargetFromBlockMemory's runs across a world open,
    /// as the visit rows read it: the record found off is an error (a tick's runs would go unseen); the
    /// load keeps it on, and the napper's next HOME run after the load is recorded under its new id.
    #[test]
    fn the_record_survives_a_world_open() {
        let put = |w: &mut redsim::world::World, x: i32, y: i32, z: i32, s: &str| {
            w.set_block(BlockPos::new(x, y, z), Block::parse(s).unwrap(), redsim::world::flags::RAW);
        };
        let mut w = redsim::world::World::new(BlockPos::new(-8, -8, -8), [32, 24, 16]);
        w.mob.seed(1, 18000);
        w.mob.ai.on = true;
        for x in -3..=12 {
            for z in -3..=3 {
                put(&mut w, x, -1, z, "minecraft:stone");
            }
        }
        put(&mut w, 0, 0, 0, "minecraft:red_bed[facing=north,occupied=false,part=head]");
        put(&mut w, 0, 0, 1, "minecraft:red_bed[facing=north,occupied=false,part=foot]");
        w.mob.ai.walk_memory_runs = None;
        let id = redsim::entity::spawn(&mut w, "minecraft:villager", [5.5, 0.0, 0.5], Some("{PersistenceRequired:1b,Brain:{memories:{\"minecraft:home\":{value:{dimension:\"minecraft:overworld\",pos:[I;0,0,0]}}}}}")).unwrap();
        assert!(drain_home_walk_runs(&mut w, &[id]).is_err(), "the record off: an error, not an empty list");
        w.mob.ai.walk_memory_runs = Some(Vec::new());
        // (G1 08:32 (b)) one napper's runs judged as ONE across the open: its ids followed through the
        // load's id map, as the visit rows follow `rider` (the run before and the runs after counted
        // under the same napper)
        let mut ids = vec![id];
        let mut judged: Vec<redsim::entity::mob::ai::WalkMemoryRun> = Vec::new();
        w.tick();
        let before = drain_home_walk_runs(&mut w, &ids).unwrap();
        assert!(!before.is_empty(), "the first REST tick's run recorded");
        judged.extend(before.iter().copied());
        let map = crate::liveness::restart(&mut w, crate::liveness::Restart::load(redsim::reload::Order::Save)).unwrap();
        let nid = *map.get(&id).expect("the villager across the load");
        ids.push(nid);
        // the load keeps the AI state: the record stays on across a world open (the visit rows arm it
        // again only if found off, and a record found off is an error)
        assert!(w.mob.ai.walk_memory_runs.is_some(), "the load turned the record off: arm it again after a world open");
        // its walk target and sink after the load: tick until its next HOME run (the sink's longest
        // duration bounds the wait)
        let mut after = Vec::new();
        for _ in 0..300 {
            w.tick();
            after.extend(drain_home_walk_runs(&mut w, &ids).unwrap());
            if !after.is_empty() {
                break;
            }
        }
        assert!(!after.is_empty(), "a HOME run recorded after the world open under the new id {nid}");
        assert!(after.iter().all(|r| r.id == nid), "the runs after the open carry the new id");
        judged.extend(after.iter().copied());
        // judged as one napper: every run is one of its ids, the samples the sum of both sides
        assert!(judged.iter().all(|r| ids.contains(&r.id)));
        assert_eq!(judged.len(), before.len() + after.len(), "the napper's runs straddle the open and are counted as one napper");
        assert!(judged.iter().any(|r| r.id == id) && judged.iter().any(|r| r.id == nid), "both sides of the open in the one napper's samples");
    }

    /// (ironspec 09:16, G1 09:15) a planted tick WITHOUT the napper's memories (the bench's early
    /// `continue` when `mem()` reads nothing) leaves no run uncounted: the bench's order, the record
    /// tallied right after the tick and only then the memories read, gives the same tally with the
    /// plant as without it, the runs of the planted tick included, and the log is empty at the row's
    /// end; tallied AFTER the early `continue` instead (the old order), the planted tick's runs are
    /// left in the log (the row-end assert catches them) or tallied a tick late.
    #[test]
    fn a_tick_without_memories_leaves_no_run_uncounted() {
        let put = |w: &mut redsim::world::World, x: i32, y: i32, z: i32, s: &str| {
            w.set_block(BlockPos::new(x, y, z), Block::parse(s).unwrap(), redsim::world::flags::RAW);
        };
        let world = || {
            let mut w = redsim::world::World::new(BlockPos::new(-8, -8, -8), [32, 24, 16]);
            w.mob.seed(1, 18000);
            w.mob.ai.on = true;
            for x in -3..=12 {
                for z in -3..=3 {
                    put(&mut w, x, -1, z, "minecraft:stone");
                }
            }
            put(&mut w, 0, 0, 0, "minecraft:red_bed[facing=north,occupied=false,part=head]");
            put(&mut w, 0, 0, 1, "minecraft:red_bed[facing=north,occupied=false,part=foot]");
            let id = redsim::entity::spawn(&mut w, "minecraft:villager", [5.5, 0.0, 0.5], Some("{PersistenceRequired:1b,Brain:{memories:{\"minecraft:home\":{value:{dimension:\"minecraft:overworld\",pos:[I;0,0,0]}}}}}")).unwrap();
            w.mob.ai.walk_memory_runs = Some(Vec::new());
            (w, id)
        };
        // the bench's loop body, `plant` the ticks whose memories read nothing; `tally_first`: the
        // bench's order (tally, then the memories' early continue), else the old order
        let run = |plant: &[u32], tally_first: bool| -> (RecordTally, RowRecord, Result<(), String>) {
            let (mut w, id) = world();
            let (mut tally, mut row) = (RecordTally::default(), RowRecord::default());
            for t in 0..120u32 {
                w.tick();
                if tally_first {
                    tally.take(&mut w, &[id], &mut row, "test").unwrap();
                }
                if plant.contains(&t) {
                    continue; // the bench's `let Some(..) = mem(..) else { continue }`
                }
                if !tally_first {
                    tally.take(&mut w, &[id], &mut row, "test").unwrap();
                }
            }
            let end = record_left_empty(&w, "test");
            (tally, row, end)
        };
        let (plain, plain_row, plain_end) = run(&[], true);
        assert!(plain.samples > 0, "the control: runs recorded");
        assert!(plain_end.is_ok());
        // planted ticks: the first run's tick, and the last tick of the row
        let first = plain_row.ticks[0];
        let (w0, _) = world();
        let t_first = (first - w0.time - 1) as u32;
        let planted = [t_first, 119];
        let (with_plant, plant_row, plant_end) = run(&planted, true);
        assert_eq!(with_plant.samples, plain.samples, "the bench's order: the planted ticks' runs counted");
        assert_eq!(plant_row.ticks, plain_row.ticks, "each run under its own tick");
        assert!(plant_end.is_ok(), "the log empty at the row's end");
        // the old order: the last planted tick's runs left in the log, caught by the row-end assert
        let (_, _, old_end) = run(&planted, false);
        assert!(old_end.is_err(), "the old order leaves the planted last tick's runs behind: the row-end assert fails");
    }

    /// (ironspec 11:46 / 11:48, G1 11:44) the proxy's refinement and its three controls: (1) the erase
    /// signature (a walk target held at t-1 gone at t, the stamp set at t-1 and clear at t, the sink not
    /// running) is a sample judged on none: today's twelve mismatch rows had exactly this shape; (2) a
    /// run where the sink did NOT erase is still counted by the old rule, unchanged (a REST tick after a
    /// tick with no walk target, judged on the previous tick's stamp), and the old rule wins when both
    /// apply; the signature's other conditions each block it (the sink running, the stamp not erased,
    /// the walk target still held, not REST, no HOME); (3) a planted proxy-only sample (no record run at
    /// its tick) fails the cross-check, as does a missed run.
    #[test]
    fn the_proxy_refinement_and_its_controls() {
        let o = |wt: bool, stamp: Option<i64>, sink_running: bool| ProxyObs { wt, stamp, rest: true, sink_running };
        // (1) the erase signature, as at game time 475 (phase +60, 0.37) and 449 (phase +36, 0.44)
        assert_eq!(proxy_sample(&o(true, Some(210), false), true, &o(false, None, false), 475), Some(ProxySample::EraseSignature));
        // (2) the old rule, unchanged: after no walk target, judged on the previous tick's stamp's age
        assert_eq!(proxy_sample(&o(false, Some(210), false), true, &o(true, Some(210), false), 474), Some(ProxySample::AfterNoTarget(Some(264))));
        assert_eq!(proxy_sample(&o(false, None, false), true, &o(false, None, false), 476), Some(ProxySample::AfterNoTarget(None)));
        // the old rule first when the previous tick held no walk target (the refinement adds, never moves)
        assert_eq!(proxy_sample(&o(false, Some(210), false), true, &o(false, None, false), 475), Some(ProxySample::AfterNoTarget(Some(265))));
        // the signature's conditions, each one blocking it
        assert_eq!(proxy_sample(&o(true, Some(210), false), true, &o(false, None, true), 475), None, "the sink running");
        // (traced 12:42, phase +1 at 0.37, game time 416) a sink RUNNING at t-1 that STOPS at t erases
        // the same memories in the tick pass, after the start pass: no run at t (it comes at t+1, the
        // old rule's sample)
        assert_eq!(proxy_sample(&o(true, Some(212), true), true, &o(false, None, false), 416), None, "the sink's stop, not its start check");
        assert_eq!(proxy_sample(&o(false, None, false), true, &o(false, None, false), 417), Some(ProxySample::AfterNoTarget(None)), "the run after the stop: the old rule");
        assert_eq!(proxy_sample(&o(true, Some(210), false), true, &o(false, Some(210), false), 475), None, "the stamp not erased");
        assert_eq!(proxy_sample(&o(true, None, false), true, &o(false, None, false), 475), None, "no stamp to erase");
        assert_eq!(proxy_sample(&o(true, Some(210), false), true, &o(true, None, false), 475), None, "the walk target still held");
        assert_eq!(proxy_sample(&o(true, Some(210), false), false, &o(false, None, false), 475), None, "no HOME");
        assert_eq!(proxy_sample(&o(true, Some(210), false), true, &ProxyObs { rest: false, ..o(false, None, false) }, 475), None, "not REST");
        // (3) the cross-check: matched; a planted proxy-only sample fails; a missed run fails
        assert_eq!(proxy_mismatch(&[474, 475], &[474, 475]), (vec![], vec![]));
        assert_eq!(proxy_mismatch(&[474, 475], &[474, 475, 480]), (vec![], vec![480]), "a planted proxy-only sample");
        assert_eq!(proxy_mismatch(&[474, 475], &[474]), (vec![475], vec![]), "a missed run (the old proxy at the erase signature)");
    }

    /// (G1 07:16 (3)) the water search's FORWARD bar on the seat's HOME path, the back-up-the-ride
    /// control included: a second node lower or further south passes; level and north (back up toward
    /// the landing), or higher, fails; no node list fails (never a vacuous pass).
    #[test]
    fn the_seat_path_forward_bar_and_its_back_up_control() {
        let line = |nodes: &str| vec![format!("(path) a rider at (9.50,-13.44,-3.50): the HOME path COMPLETE (3 nodes): {nodes}")];
        let down = seat_path_nodes(&line("[(9, -14, -4), (9, -15, -4), (9, -15, -3)]"));
        assert_eq!(down, vec![[9, -14, -4], [9, -15, -4], [9, -15, -3]]);
        assert!(seat_path_forward(&down), "lower: forward");
        assert!(seat_path_forward(&seat_path_nodes(&line("[(9, -14, -4), (9, -14, -3)]"))), "level, south: forward");
        assert!(!seat_path_forward(&seat_path_nodes(&line("[(9, -14, -4), (9, -14, -5)]"))), "level, north (back up the ride): FAILS");
        assert!(!seat_path_forward(&seat_path_nodes(&line("[(9, -14, -4), (9, -13, -4)]"))), "higher: FAILS");
        assert!(!seat_path_forward(&seat_path_nodes(&line("[(9, -14, -4)]"))), "one node: FAILS");
        assert!(!seat_path_forward(&seat_path_nodes(&["(path) at [9, -14, -4]: no rider".to_string()])), "no node list: FAILS");
    }

    fn test_opts(queue: P) -> Opts {
        Opts {
            queue,
            post: None,
            own: Vec::new(),
            napper: Vec::new(),
            keep_clear: Vec::new(),
            rail: false,
            chunk: [-6, -6, 9, 9],
            positions: Vec::new(),
            pad: Vec::new(),
            extra: Vec::new(),
            custom_post: true,
            replace: Vec::new(),
            south: false,
            queue_glass: false,
            seat_at: None,
            band: [0.11, 0.24],
            r1_built: false,
            r1_layout: R1Layout::Record7,
            visit_skip: Vec::new(),
            bystander: None,
            visit_clear_stamp: false,
            visit_plant_stamp: None,
            visit_plant_erase: false,
            visit_start_held: false,
            visit_break_bed: false,
            visit_at_rest: false,
            probe_bed: None,
            watch_cells: Vec::new(),
            declared_contacts: Vec::new(),
            declared_bounded: Vec::new(),
            reference_run: false,
            rail_end_least: None,
            ball_head: None,
        }
    }

    /// (G1 14:51 (4)) the bounded declarations in the ride's scope: an entry whose cells lie in the scope
    /// is read strictly (a malformed one an error); one outside it is left to its bench (even when it
    /// wouldn't parse under the bounded grammar); one with cells on both sides is an error.
    #[test]
    fn the_bounded_declarations_in_the_ride_scope() {
        let scope = ([2, -27, -8], [9, -11, 3]);
        let inside = "# contacts: 9,-15,0=the drop stop | face=north | class=design | ref=t `cmd` (rows) | ticks<=1 why=one tick | events<=1 why=once | depth<=0.2057+eps=0.010 why=the launch | speed_in<=0.4000+eps=0.010 why=the cap | speed_out<=0.1943+eps=0.010 why=the gap | rate=1640/1640 rows\n";
        let outside_bad = "# contacts: -3,-27,3=the bed head; class=design; rate=296/408 (no unit, not this bench's)\n";
        let v = bounded_in_scope(&format!("{inside}{outside_bad}"), scope).unwrap();
        assert_eq!(v.len(), 1, "only the in-scope entry");
        assert_eq!(v[0].class, crate::contacts::Class::Design);
        assert_eq!(v[0].cells, vec![BlockPos::new(9, -15, 0)]);
        // a legacy (fieldless) entry in scope reads as Legacy
        let legacy = bounded_in_scope("# contacts: 9,-15,0=the drop stop\n", scope).unwrap();
        assert_eq!(legacy[0].class, crate::contacts::Class::Legacy);
        // a malformed in-scope entry is an error (here: a field with no known key after the fields began)
        assert!(bounded_in_scope("# contacts: 9,-15,0=the drop stop | class=design | ref=x (1; 2)\n", scope).is_err() || bounded_in_scope("# contacts: 9,-15,0=the drop stop | class=design\n", scope).is_err());
        // cells on both sides of the scope
        assert!(bounded_in_scope("# contacts: 9,-15,0+-3,-27,3=split\n", scope).is_err());
        // (G1's B10 on R4) a malformed cell is an error, never dropped
        assert!(bounded_in_scope("# contacts: 9,-15,x=a bad cell\n", scope).unwrap_err().contains("is not x,y,z"));
        // (G1 18:34 (3)) the split names what it left outside: the count in and out
        let (d, out) = bounded_scope_split(&format!("{inside}{outside_bad}"), scope).unwrap();
        assert_eq!((d.len(), out.len()), (1, 1));
        assert!(out[0].starts_with("-3,-27,3"), "{out:?}");
        // the floor line just outside the scope: nothing inside (the caller's NOT JUDGED)
        let floor = "# contacts: 9,-16,-2=the support's top | face=up | class=design | ref=t | ticks<=1 why=w | events<=1 why=w | depth<=0.0400+eps=0.001 why=w | speed_out>=0.1943+eps=0.001 why=w\n";
        let (d, out) = bounded_scope_split(floor, ([2, -15, -8], [9, -11, 3])).unwrap();
        assert!(d.is_empty() && out.len() == 1, "{d:?} {out:?}");
    }

    /// (G1's B1 / B2 on R4, G1 18:34 (1)) the ride rows' re-key: two rows sharing the cart's entity id
    /// with consecutive ticks stay two events of one tick each (no merge across rows), while two hits a
    /// tick apart IN ONE ROW are one event of two ticks; the reference check folds the re-keyed rows (a
    /// lower speed_out becomes the least); a 100 % rate missing one row is STALE; a declaration met in
    /// no row is STALE.
    #[test]
    fn the_ride_rows_rekey_per_row_and_judge_the_rate_over_the_set() {
        use crate::contacts::{Face, Hit, StallRule};
        let h = |tick: i64, sout: f64| Hit { tick, cart: 7, cell: BlockPos::new(9, -15, 0), face: Face::North, depth: 0.2057, speed_in: 0.4, speed_out: sout, sample: None, pos: [9.5, -15.0, -0.49] };
        let decl = |extra: &str| crate::contacts::declared_contacts_bounded(&format!("# contacts: 9,-15,0=the drop stop | face=north | class=design | ref=t | ticks<=1+eps=0 why=w | events<=1+eps=0 why=w | depth<=0.2057+eps=0.001 why=w | speed_out>=0.1943+eps=0.001 why=w{extra}\n")).unwrap();
        // the motion after each event: falling (no stall)
        let falling = |_c: u32, _t: i64| Some((0.0, -0.1));
        // two adjacent-phase rows, one entity id, ticks 17 and 18: two events of 1 tick, one a row
        let rows: Vec<Hit> = rekey_row(0, vec![h(17, 0.1943)]).chain(rekey_row(1, vec![h(18, 0.1943)])).collect();
        let v = crate::contacts::judge(&decl(""), &rows, &falling, StallRule::IRONSPEC_1042, Some((2, "rows")));
        assert!(!v.failed(), "{v:?}");
        assert_eq!(v.events, vec![2]);
        assert!(crate::contacts::reference_check(&decl("")[0], &rows).is_empty(), "ticks 1, events 1 reproduce");
        // the twin: two hits a tick apart in ONE row: one event of 2 ticks (exceeds ticks<=1)
        let one: Vec<Hit> = rekey_row(0, vec![h(17, 0.1943), h(18, 0.1943)]).collect();
        let v = crate::contacts::judge(&decl(""), &one, &falling, StallRule::IRONSPEC_1042, Some((1, "rows")));
        assert_eq!(v.events, vec![1]);
        assert!(v.exceeded.iter().any(|s| s.contains("ticks 2")), "{:?}", v.exceeded);
        // a lower speed_out in one row becomes the reference's least: the declared 0.1943 mismatches
        let low: Vec<Hit> = rekey_row(0, vec![h(17, 0.1943)]).chain(rekey_row(1, vec![h(18, 0.1800)])).collect();
        assert!(!crate::contacts::reference_check(&decl("")[0], &low).is_empty(), "the least speed_out 0.1800 differs from 0.1943");
        // rate=2/2 rows with the contact in only one of the two rows: an under-rate STALE
        let rated = decl(" | rate=2/2 rows");
        let half: Vec<Hit> = rekey_row(0, vec![h(17, 0.1943)]).collect();
        let v = crate::contacts::judge(&rated, &half, &falling, StallRule::IRONSPEC_1042, Some((2, "rows")));
        assert!(!v.stale.is_empty(), "{v:?}");
        // rate=1/2 rows met in no row of 20: STALE (P(0 | 1/2) over 20 rows < 0.001); over 2 rows the same
        // absence can't be judged (P = 0.25): "stale not judged", printed
        let v = crate::contacts::judge(&decl(" | rate=1/2 rows"), &[], &falling, StallRule::IRONSPEC_1042, Some((20, "rows")));
        assert!(!v.stale.is_empty(), "{v:?}");
        let v = crate::contacts::judge(&decl(" | rate=1/2 rows"), &[], &falling, StallRule::IRONSPEC_1042, Some((2, "rows")));
        assert!(v.stale.is_empty() && !v.stale_not_judged.is_empty(), "{v:?}");
        // the untagged wiring error is gone: re-keyed hits carry their row as the sample
        assert!(rows.iter().all(|x| x.sample == Some(x.cart as u64)));
    }

    /// (G1 07:16 (8)) `--write`: the module's raw header read before the save (an in-place write keeps
    /// its comments), the `way-in:` key replaced in its own place (no key twice), and the splice only at
    /// a `palette:` line.
    #[test]
    fn write_keeps_the_header_in_place_without_duplicate_keys() {
        let dir = std::env::temp_dir().join(format!("waycell-write-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let module = dir.join("module.txt");
        std::fs::write(&module, "# about the name\nname: t\n# about the way-in\nway-in: an old line\ncircuit: mob-farm iron_ingot\norigin: 0 0 0\npalette:\n  S minecraft:stone\nlayer 0:\n  SSS\n  SSS\n  SSS\n").unwrap();
        let o = test_opts([1, 1, 1]);
        // in place: the output is the module itself
        let lines = write_layout(&module, &module, &o, [0, 1, 0]).unwrap();
        assert!(lines[0].starts_with("wrote "), "{}", lines[0]);
        let text = std::fs::read_to_string(&module).unwrap();
        let head: Vec<&str> = text.lines().take_while(|l| *l != "palette:").collect();
        assert!(text.lines().any(|l| l == "palette:"), "the body kept");
        assert!(head.contains(&"# about the name") && head.contains(&"# about the way-in"), "the comments kept: {head:?}");
        let k = head.iter().position(|l| *l == "# about the way-in").unwrap();
        assert!(head[k + 1].starts_with("way-in: b8949c96 way-cell --write"), "the new way-in line in the old one's place: {head:?}");
        assert_eq!(head.iter().filter(|l| l.starts_with("way-in:")).count(), 1, "no way-in line twice: {head:?}");
        for key in ["name:", "circuit:", "origin:"] {
            assert_eq!(head.iter().filter(|l| l.starts_with(key)).count(), 1, "no {key} line twice: {head:?}");
        }
        // a module without a palette line: refused (no splice at an assumed place)
        let bad = dir.join("bad.txt");
        std::fs::write(&bad, "name: t\n").unwrap();
        assert!(write_layout(&bad, &dir.join("out.txt"), &o, [0, 1, 0]).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// (ironspec 11:05: nothing silently dropped) a way-in cell outside the world's box fails the
    /// run, naming the cell; one inside is placed.
    #[test]
    fn a_cell_outside_the_world_fails() {
        let mut w = redsim::world::World::new(BlockPos::new(0, 0, 0), [4, 4, 4]);
        let g = Block::parse("minecraft:glass").unwrap();
        assert!(place_checked(&mut w, &[(BlockPos::new(1, 1, 1), g)]).is_ok());
        assert!(!w.get(BlockPos::new(1, 1, 1)).is_air());
        let e = place_checked(&mut w, &[(BlockPos::new(2, 2, 2), g), (BlockPos::new(9, 1, 1), g)]).unwrap_err();
        assert!(e.contains("1 way-in cell(s) outside"), "{e}");
        assert!(w.get(BlockPos::new(2, 2, 2)).is_air(), "nothing placed when any cell is outside");
        // an ascending rail with nothing on its high side pops: refused; with glass there, kept
        let r = Block::parse("minecraft:rail[shape=ascending_east,waterlogged=false]").unwrap();
        let e = place_checked(&mut w, &[(BlockPos::new(1, 2, 1), r)]).unwrap_err();
        assert!(e.contains("would pop"), "{e}");
        assert!(place_checked(&mut w, &[(BlockPos::new(2, 2, 2), g), (BlockPos::new(1, 2, 2), g), (BlockPos::new(1, 3, 2), r)]).is_err(), "no rigid top under (1,3,2): (1,2,2) is glass, its high side (2,3,2) is air");
        assert!(place_checked(&mut w, &[(BlockPos::new(2, 3, 2), g), (BlockPos::new(1, 3, 2), r)]).is_ok());
    }

    /// (G1's W8 on R2) one mode a run, its companion flags present: a mis-shaped mode is an error, never
    /// a fall-through; two modes together refused; no primary flag is the queue-cell checks.
    #[test]
    fn one_mode_a_run_with_its_companion_flags() {
        let base = ModeFlags::default();
        assert_eq!(way_cell_mode(&base), Ok("queue cell"));
        // the visit without --leg-day is refused, not run as another mode
        let visit = ModeFlags { visit: true, leg: 6, bed: 3, ride_to: 3, ..base.clone() };
        assert!(way_cell_mode(&visit).unwrap_err().contains("--leg-day"));
        assert_eq!(way_cell_mode(&ModeFlags { leg_day: true, ..visit.clone() }), Ok("visit"));
        // the sweep without --post, the wait without --bed, the leg without --hold
        assert!(way_cell_mode(&ModeFlags { release_sweep: true, wait: 2, bed: 3, ride_to: 3, ..base.clone() }).unwrap_err().contains("--post"));
        assert!(way_cell_mode(&ModeFlags { wait: 2, post: true, ..base.clone() }).unwrap_err().contains("--bed"));
        assert!(way_cell_mode(&ModeFlags { leg: 6, bed: 3, ..base.clone() }).unwrap_err().contains("--hold"));
        // a mis-shaped --wait (one value) is refused
        assert!(way_cell_mode(&ModeFlags { wait: 1, post: true, bed: 3, ..base.clone() }).unwrap_err().contains("--wait"));
        // two modes at once: the water scan with a path probe
        assert!(way_cell_mode(&ModeFlags { water_scan: true, scan_box: 6, path_probe: true, bed: 3, ..base.clone() }).unwrap_err().contains("one mode a run"));
        // the ride rows need --ride-to; the release pulse needs the built R1 gate
        assert!(way_cell_mode(&ModeFlags { rows: true, ..base.clone() }).unwrap_err().contains("--ride-to"));
        assert!(way_cell_mode(&ModeFlags { release_pulse: 2, ride_to: 3, ..base.clone() }).unwrap_err().contains("--r1-built"));
        assert_eq!(way_cell_mode(&ModeFlags { release_pulse: 2, ride_to: 3, south: true, r1_built: true, ..base.clone() }), Ok("release pulse"));
        assert_eq!(way_cell_mode(&ModeFlags { launch_speeds: 3, ..base.clone() }), Ok("launch speed"));
        // (R6) the leg train: its companions, the built gate; with it, the plain leg mode is not chosen
        let train = ModeFlags { leg_train: 2, leg: 6, bed: 3, leg_day: true, south: true, r1_built: true, ..base.clone() };
        assert_eq!(way_cell_mode(&train), Ok("leg train"));
        assert!(way_cell_mode(&ModeFlags { r1_built: false, ..train.clone() }).unwrap_err().contains("built R1"));
        assert!(way_cell_mode(&ModeFlags { leg_day: false, ..train.clone() }).unwrap_err().contains("--leg-day"));
        assert!(way_cell_mode(&ModeFlags { leg_train: 1, ..train }).unwrap_err().contains("COUNT,SPACING"));
    }

    /// (R6, V) the south staircase's cells follow the queue (today's literal cells reproduced at today's
    /// queue, V's at V's), and the built R1's layouts are laid relative to the queue: record7's at
    /// today's queue is R1_BUILT exactly, record7's at V's queue is it raised 4, V's at V's queue is
    /// R1_BUILT_V exactly (N's comparator at (4,-10,-4), DAY's repeater (4,-10,-3), no hatch).
    #[test]
    fn the_staircase_and_r1_follow_the_queue() {
        assert_eq!(rail_end_of([9, -14, -4]), [9, -15, -2]);
        assert_eq!(not_ready_lever_of([9, -14, -4]), BlockPos::new(8, -14, -5));
        assert_eq!(shaft_xz_of([9, -14, -4]), (9, -1));
        assert_eq!(rail_end_of([9, -10, -4]), [9, -11, -2]);
        assert_eq!(not_ready_lever_of([9, -10, -4]), BlockPos::new(8, -10, -5));
        let mut o = test_opts([9, -14, -4]);
        o.south = true;
        o.r1_built = true;
        let today: Vec<(BlockPos, String)> = r1_cells(&o).unwrap().into_iter().map(|(p, b)| (p, format!("{b:?}"))).collect();
        assert_eq!(today.len(), R1_BUILT.len());
        assert_eq!(today[1].0, BlockPos::new(6, -12, -2), "N's comparator where record7 has it");
        o.queue = [9, -10, -4];
        let raised = r1_cells(&o).unwrap();
        assert!(raised.iter().zip(today.iter()).all(|((p, _), (t, _))| p.x == t.x && p.y == t.y + 4 && p.z == t.z), "record7's layout raised 4 with the queue");
        o.r1_layout = R1Layout::V;
        let v = r1_cells(&o).unwrap();
        assert_eq!(v.len(), R1_BUILT_V.len());
        assert!(v.iter().any(|(p, b)| *p == BlockPos::new(4, -10, -4) && b.to_string().contains("comparator")), "N's comparator re-routed down");
        assert!(v.iter().any(|(p, b)| *p == BlockPos::new(4, -10, -3) && b.to_string().contains("repeater")), "DAY's one repeater");
        assert!(v.iter().all(|(p, b)| !(p.y >= -9 && b.to_string().contains("trapdoor"))), "no hatch, nothing in the pad's layers");
        assert!(v.iter().all(|(p, _)| p.y <= -10), "V's R1 stays at or below the queue's level");
        assert_eq!("v".parse::<R1Layout>(), Ok(R1Layout::V));
        assert!("w".parse::<R1Layout>().is_err());
    }
}
