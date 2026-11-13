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
        return R1_BUILT.iter().map(|(p, s)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(s).map_err(|e| format!("R1_BUILT {s}: {e}"))?))).collect();
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

/// The queue's release as the bench emulates day-1's RELEASE level: for the south staircase, R1's
/// input driven by a redstone torch standing on glass at queue - (3,0,0) (the repeaters of
/// [`r1_cells`] carry it into the rail; nothing solid-render); for the west one a redstone block
/// (its old emulation).
fn release(w: &mut redsim::world::World, o: &Opts) -> Result<(), String> {
    let q = o.queue;
    if o.south && o.r1_built {
        // NOT-ready's fall: onechunk's wall lever (8,-14,-5) flipped OFF between ticks
        let p = BlockPos::new(8, -14, -5);
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
    ]
    .into_iter()
    .chain(override_lines)
    .collect())
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
    let input = if o.south { BlockPos::new(q[0] - 1, q[1], q[2]) } else { BlockPos::new(q[0], q[1], q[2] + 1) };
    let mut rows: Vec<(String, Option<u32>, Option<Vec3>, f32, bool, bool)> = Vec::new();
    // (G1 09:31 E2) each row's band-bench t0 (the entry cell's centre passed, post-tick), in the rows' order
    let mut centres: Vec<Option<(u32, f64)>> = Vec::new();
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
        let mut restart_cell: Option<P> = None;
        let mut eye_wet = 0u32;
        let mut contacts = 0u32;
        let mut declared_contact_ticks = 0u32;
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
            let e = &run.world.entities.list[ci];
            cart_ticks_n += bar_row as u64;
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
                for c in cells {
                    if !contact_cells.contains(&c) {
                        contact_cells.push(c);
                    }
                }
            }
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
        rows.push((
            format!(
                "{label}{}{}{}{}{} [after the landing: the box's z at most {box_z_max:.4}, vz at most {vz_max:.4}]",
                restart_cell.map_or(String::new(), |c| format!(" (the cart at {c:?} at the restart, {} idle tick(s))", idle_n.unwrap_or(0))),
                if eye_wet > 0 { format!(" (the rider's EYE IN WATER {eye_wet} ticks)") } else { String::new() },
                if at_entry.is_none() { last_seen.map_or(" (the cart gone)".to_string(), |(c, s)| format!(" (the cart last at {c:?} at {s:.4} b/t)")) } else { String::new() },
                if time_jumps > 0 { format!(" (GAME TIME NOT CONTINUOUS at {time_jumps} steps)") } else { String::new() },
                if contacts > 0 {
                    format!(" (CONTACTS {contacts} ticks at {})", contact_cells.iter().filter(|c| !o.declared_contacts.contains(&[c.x, c.y, c.z])).map(|c| format!("{c} {}", run.world.get(*c))).collect::<Vec<_>>().join(", "))
                } else if declared_contact_ticks > 0 {
                    format!(" (declared contacts {declared_contact_ticks} tick(s) at {})", contact_cells.iter().map(|c| format!("{c} {}", run.world.get(*c))).collect::<Vec<_>>().join(", "))
                } else {
                    String::new()
                }
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
    // (ironspec 01:00 (1): freezes print as STRESS) the bar rows are the plain and restart rows; the
    // freeze rows print beside them and never bar
    let bar: Vec<&(String, Option<u32>, Option<Vec3>, f32, bool, bool)> = rows.iter().filter(|r| !r.0.starts_with("frozen")).collect();
    let stress: Vec<&(String, Option<u32>, Option<Vec3>, f32, bool, bool)> = rows.iter().filter(|r| r.0.starts_with("frozen")).collect();
    let ts: Vec<u32> = bar.iter().filter_map(|r| r.1).collect();
    let ss: Vec<f64> = bar.iter().filter_map(|r| r.2.map(|v| (v.x * v.x + v.z * v.z).sqrt())).collect();
    let all_ok = bar.iter().all(|r| r.1.is_some() && r.2.as_ref().is_some_and(ok_v) && r.3 <= 0.0 && !r.4 && !r.5 && !r.0.contains("EYE IN WATER") && !r.0.contains("CONTACTS") && !r.0.contains("GAME TIME NOT"));
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
            stress.iter().filter(|r| !(r.1.is_some() && r.2.as_ref().is_some_and(ok_v) && r.3 <= 0.0 && !r.4 && !r.5 && !r.0.contains("CONTACTS") && !r.0.contains("GAME TIME NOT"))).count()
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
    let pass = !vacuous && all_ok && td_ok && watch_ok && probe_ok && !(fence_min.is_finite() && fence_min <= 0.0);
    out.push(format!(
        "(ride) {} rows ({fluid_n} fluid cell(s) of the chunk given their tick): the entry reached in {} of them at t {}..{} after the release, {:.4}..{:.4} b/t heading west (the bar [{}, {}], inherited: ironspec 21:25 from 58f770cc's A6 park sweep){centre_line}; 0 hurt, on rails, never back south; {}; {}; {}; the closest fence box (post and arms as laid) to the cart's box: {}; {td_line}{watch_line}{}: {}",
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
    Ok((out, pass))
}

/// The queued napper's all-day wait (ironspec 10:18): the rider held on the queue rail from day
/// time `start` for `ticks` gt with its HOME the stop's bed (`bed`, a loaded HOME memory), the
/// post's zombie in place: HOME at every tick, its activity tick by tick, the calm REST ticks
/// (REST active, not PANIC) and their distance to the bed's head, its CANT_REACH stamp's age
/// (the most, and the most at a calm REST tick: the bar 1,000).
pub fn wait_lines(spec: &FarmSpec, o: &Opts, post: P, start: i64, ticks: u32, bed: P, stamp: Option<i64>, follow: u32) -> Result<Vec<String>, String> {
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
    redsim::entity::spawn(&mut run.world, "minecraft:oak_boat", [post[0] as f64 + 0.5, post[1] as f64, post[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    let nbt = format!(
        "{{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
        bed[0], bed[1], bed[2]
    );
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some(&nbt))?;
    run.world.mob.day_offset = start - run.world.time;
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
    let mut out = vec![
        format!(
            "(wait) the rider held on the queue rail from day time {start} for {ticks} gt (its HOME the bed at {bed:?}{}): HOME {}; the rider {}; first PANIC at t {first_panic:?}",
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
            if stamp_calm_max <= 1000 { "PASS" } else { "FAIL" }
        ),
    ];
    if stamp.is_some() {
        out.push(format!("(wait) the armed stamp cleared at t {cleared_at:?} ({})", match cleared_at {
            Some(t) if t <= 100 => "within 100 gt of the arrival (inherited: ironspec 14:47): PASS".to_string(),
            Some(_) => "LATER than 100 gt (inherited: ironspec 14:47): FAIL".to_string(),
            None => "NEVER cleared".to_string(),
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
    Ok(override_lines.into_iter().chain(out).collect())
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
    redsim::entity::spawn(&mut base.world, "minecraft:oak_boat", [post[0] as f64 + 0.5, post[1] as f64, post[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    let nbt = format!(
        "{{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
        bed[0], bed[1], bed[2]
    );
    let cart0 = redsim::entity::spawn(&mut base.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some(&nbt))?;
    base.world.mob.day_offset = start - base.world.time;
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
    // (G1 07:00 / 08:12) the sweep's verdict returned beside its lines (the exit status)
    let pass = armed_after_n == 0 && home_lost_n == 0 && reached_n as usize == n_rows && n_rows > 0;
    // (G1 (6)) the override rule's before -> after lines kept
    Ok((override_lines.into_iter().chain(out).collect(), pass))
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
    for k in 0..phases {
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
            out.push(format!("(visit) phase +{k}: the cart never reached the arrival detector"));
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
                let (mut no_wt_prev, mut since_prev): (bool, Option<i64>) = (false, None);
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
                    // (G1 (2), ironspec 07:16; G1 08:12 CHANGES (1)(2)) the REST-entry stamp bar's
                    // SAMPLES: every tick SetWalkTargetFromBlockMemory RAN, i.e. a REST tick after a
                    // tick with NO walk target in ANY activity (the first REST tick after PANIC or IDLE
                    // included), whatever the stamp; JUDGED on the stamp the behaviour SAW (the previous
                    // tick's stamp, with this tick's time: the behaviour's release rewrites it this tick)
                    if no_wt_prev && act.contains("Rest") && home_prev {
                        swt_samples += 1;
                        row_proxy_ticks.push(r.world.time);
                        if let Some(s) = since_prev {
                            let age = r.world.time - s;
                            swt_max = swt_max.max(age);
                            if age >= 1000 {
                                swt_fails += 1;
                                if swt_fail_where.len() < 8 {
                                    swt_fail_where.push(format!("phase +{k} hold {h}{skip_label} {}: {age} gt", if released { format!("release + {}", t - h) } else { format!("hold t {t}") }));
                                }
                            }
                        }
                    }
                    no_wt_prev = !wt;
                    since_prev = since;
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
                        if let Some(c) = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed) {
                            cart_last = Some(([c.pos.x.floor() as i32, c.pos.y.floor() as i32, c.pos.z.floor() as i32], (c.delta_movement.x.powi(2) + c.delta_movement.z.powi(2)).sqrt()));
                        }
                        if [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32] == entry && at_entry.is_none() {
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
                let missed: Vec<i64> = row_rec.ticks.iter().filter(|t| !row_proxy_ticks.contains(t)).copied().collect();
                let spurious: Vec<i64> = row_proxy_ticks.iter().filter(|t| !row_rec.ticks.contains(t)).copied().collect();
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
        ("(i) the stamp's largest age over the visit < 1,000 gt (ironspec 22:10 (2))".to_string(), rows_n, over_1000_n, String::new()),
        (
            "(ii) the AT-ENTRY bar: the stamp under 1,000 gt at every run of SetWalkTargetFromBlockMemory toward HOME, from redsim's RECORD of its runs (the tick, the branch, the stamp age it SAW; ironspec 08:13)".to_string(),
            tally.samples,
            tally.fails,
            format!(
                "the branches {}; the largest age seen {} gt{}; the proxy beside it (a REST tick after a tick with no walk target, HOME present, the previous tick's stamp): {swt_samples} samples, {swt_fails} failing, the largest {swt_max} gt{}",
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
pub fn water_scan_lines(spec: &FarmSpec, o: &Opts, ticks: u32, lo: P, hi: P) -> Result<Vec<String>, String> {
    Ok(water_scan(spec, o, ticks, lo, hi)?.2)
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
    for t in 0..ticks {
        run.tick();
        if let Some(r) = run.world.entities.list.iter().find(|e| !e.removed && Some(e.id) == rider) {
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
        format!("(leg) on the HOLD detector {hold:?} at t {on_hold:?}; within 16 of the bed's head at t {ball:?} (HOLD's lead {lead}); at the arrival detector at t {at_arrival:?}; at rest on the queue rail at t {at_queue:?}"),
        format!(
            "(leg) the fall ({} airborne ticks before the arrival): the cart's box x {:.3}..{:.3}, z {:.3}..{:.3}; |vx| at most {vmax:.6}; ticks with the box in a full block {contacts} ({})",
            fall.len(),
            fx.0,
            fx.1,
            fz.0,
            fz.1,
            if contacts == 0 && vmax < 1e-9 { "PASS: x fixed, vx 0, no contact" } else { "FAIL" }
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
        }
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
}
