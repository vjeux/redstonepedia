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
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};
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
        Block::Rail { shape, .. } | Block::PlainRail { shape } | Block::DetectorRail { shape, .. } => {
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
    let extra = o.extra.iter().map(|(p, b)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(b).map_err(|e| format!("{b}: {e}"))?))).collect::<Result<Vec<_>, String>>()?;
    place_checked(&mut w, &extra)?;
    if let Some(p) = post {
        if !o.custom_post {
            place_checked(
                &mut w,
                &[
                    (BlockPos::new(p[0], p[1] - 1, p[2]), Block::parse("minecraft:glass").map_err(|e| e.to_string())?),
                    (BlockPos::new(p[0], p[1] + 3, p[2]), Block::parse("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]").map_err(|e| e.to_string())?),
                ],
            )?;
        }
        redsim::entity::spawn(&mut w, "minecraft:oak_boat", [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    }
    let q = o.queue;
    // (the queue cell: a shut bottom-half iron trapdoor the cart rests on, 3/16 high; or with
    // `rail`, a powered rail on glowstone ascending to the east, the cart at its middle)
    let qy = if o.rail {
        let cells = rail_cells(q, o.south).iter().map(|(p, b, _)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(b).map_err(|e| e.to_string())?))).collect::<Result<Vec<_>, String>>()?;
        place_checked(&mut w, &cells)?;
        q[1] as f64 + 0.5625
    } else {
        if w.get(BlockPos::new(q[0], q[1], q[2])).is_air() {
            place_checked(&mut w, &[(BlockPos::new(q[0], q[1], q[2]), Block::parse("minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]").map_err(|e| e.to_string())?)])?;
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
        "(c) the zombie at ({:.2},{:.2},{:.2}): from the rider {d:.3} (6.05..7.95: {}); in its sight {} ; its eye cell ({},{},{}) {} (sky light {})",
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
        "(b) every other position: the nearest {nearest:.2} from the zombie; within 8 and in sight: {}; hidden within 6.05: {} ({}) ({})",
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
        "(e) the rider's own route (the shaft, the detectors, the run): positions within 8 of the zombie and in its sight: {}; under 6.05 and in sight: {} ({})",
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
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(rail_cells(q, o.south).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    place_checked(&mut run.world, &cells)?;
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
    // settle on the braked rail, then release
    for _ in 0..40 {
        run.tick();
    }
    let held_at = run.world.entities.list.iter().find(|e| e.id == cart).map(|e| e.pos);
    let input = if o.south { BlockPos::new(q[0] - 1, q[1], q[2]) } else { BlockPos::new(q[0], q[1], q[2] + 1) };
    run.world.set_block(input, Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
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
    ])
}

/// (ironspec 12:44, the drop-and-roll's conditions (1)-(2)) The ride from the queue in rows, the
/// world built once and cloned per row: every release phase (the cart held `k` more gt before the
/// release, k < `phases`) and every restart (a save/stop/start at tick T after the release in each
/// reload order). Per row: the entry tick after the release, the speed and heading on the entry, the
/// rider's health lost, and at the landing cell (`landing`) whether the cart stayed on rails and never
/// moved back south past it. Then the min..max over the rows.
pub fn ride_rows(spec: &FarmSpec, o: &Opts, entry: P, ticks: u32, phases: u32, restart_ticks: &[i64], landing: Option<P>) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + phases + 60, WalkMode::Random) };
    let (mut base, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(rail_cells(q, o.south).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    place_checked(&mut base.world, &cells)?;
    let cart0 = redsim::entity::spawn(&mut base.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
    for _ in 0..40 {
        base.tick();
    }
    let input = if o.south { BlockPos::new(q[0] - 1, q[1], q[2]) } else { BlockPos::new(q[0], q[1], q[2] + 1) };
    let mut rows: Vec<(String, Option<u32>, Option<Vec3>, f32, bool, bool)> = Vec::new();
    let mut specs: Vec<(String, u32, Option<(i64, crate::liveness::Restart)>)> = (0..phases).map(|k| (format!("phase +{k}"), k, None)).collect();
    for &t in restart_ticks {
        for how in crate::liveness::Restart::rows() {
            specs.push((format!("restart at t {t} ({})", how.label()), 0, Some((t, how))));
        }
    }
    for (label, k, restart) in specs {
        let mut run = base.clone();
        for _ in 0..k {
            run.tick();
        }
        run.world.set_block(input, Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
        let mut cart = cart0;
        let rider_of = |w: &redsim::world::World, c: u32| w.entities.list.iter().position(|v| !v.removed && v.vehicle == Some(c));
        let h0 = rider_of(&run.world, cart).map(|i| redsim::entity::mob::mob(&run.world, i).health).unwrap_or(0.0);
        let mut h_min = h0;
        let (mut at_entry, mut v_entry, mut landed, mut derailed, mut back_south) = (None, None, None::<u32>, false, false);
        let (mut box_z_max, mut vz_max) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for t in 0..ticks {
            if let Some((rt, how)) = restart {
                if t as i64 == rt {
                    let map = crate::liveness::restart(&mut run.world, how)?;
                    cart = *map.get(&cart).ok_or("the cart lost in the restart")?;
                }
            }
            run.tick();
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == cart && !e.removed) else { break };
            let c = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
            let (pz, vz) = (e.pos.z, e.delta_movement.z);
            let v = e.delta_movement;
            if let Some(i) = rider_of(&run.world, cart) {
                h_min = h_min.min(redsim::entity::mob::mob(&run.world, i).health);
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
            if at_entry.is_none() && c == entry {
                at_entry = Some(t);
                v_entry = Some(v);
                break;
            }
        }
        rows.push((format!("{label} [after the landing: the box's z at most {box_z_max:.4}, vz at most {vz_max:.4}]"), at_entry, v_entry, h0 - h_min, derailed || (landing.is_some() && landed.is_none()), back_south));
    }
    let mut out = Vec::new();
    let ok_v = |v: &Vec3| {
        let s = (v.x * v.x + v.z * v.z).sqrt();
        (0.02..=0.62).contains(&s) && v.x < 0.0 && v.x.abs() > v.z.abs()
    };
    for (label, t, v, hurt, derail, back) in &rows {
        out.push(format!(
            "(ride) {label}: the entry at t {} after the release, {}; hurt {hurt:.1}; {}{}",
            t.map_or("NEVER".to_string(), |t| t.to_string()),
            v.map_or("-".to_string(), |v| format!("{:.4} b/t (vx {:.4}, vz {:.4})", (v.x * v.x + v.z * v.z).sqrt(), v.x, v.z)),
            if *derail { "OFF THE RAILS or no landing" } else { "on rails from the landing" },
            if *back { ", MOVED BACK SOUTH" } else { "" }
        ));
    }
    let ts: Vec<u32> = rows.iter().filter_map(|r| r.1).collect();
    let ss: Vec<f64> = rows.iter().filter_map(|r| r.2.map(|v| (v.x * v.x + v.z * v.z).sqrt())).collect();
    let all_ok = rows.iter().all(|r| r.1.is_some() && r.2.as_ref().is_some_and(ok_v) && r.3 <= 0.0 && !r.4 && !r.5);
    out.push(format!(
        "(ride) {} rows: the entry at t {}..{} after the release, {:.4}..{:.4} b/t heading west (A6's band 0.02..0.62); 0 hurt, on rails, never back south: {}",
        rows.len(),
        ts.iter().min().copied().unwrap_or(0),
        ts.iter().max().copied().unwrap_or(0),
        ss.iter().cloned().fold(f64::INFINITY, f64::min),
        ss.iter().cloned().fold(0.0, f64::max),
        if all_ok { "PASS" } else { "FAIL" }
    ));
    Ok(out)
}

/// The queued napper's all-day wait (ironspec 10:18): the rider held on the queue rail from day
/// time `start` for `ticks` gt with its HOME the stop's bed (`bed`, a loaded HOME memory), the
/// post's zombie in place: HOME at every tick, its activity tick by tick, the calm REST ticks
/// (REST active, not PANIC) and their distance to the bed's head, its CANT_REACH stamp's age
/// (the most, and the most at a calm REST tick: the bar 1,000).
pub fn wait_lines(spec: &FarmSpec, o: &Opts, post: P, start: i64, ticks: u32, bed: P) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(rail_cells(q, o.south).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    if !o.custom_post {
        cells.push((BlockPos::new(post[0], post[1] - 1, post[2]), Block::parse("minecraft:glass").map_err(|e| e.to_string())?));
        cells.push((BlockPos::new(post[0], post[1] + 3, post[2]), Block::parse("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]").map_err(|e| e.to_string())?));
    }
    place_checked(&mut run.world, &cells)?;
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
    for t in 0..ticks {
        run.tick();
        let w = &run.world;
        let Some(i) = w.entities.list.iter().position(|e| !e.removed && e.vehicle == Some(cart)) else {
            left = Some(t);
            break;
        };
        if redsim::entity::mob::ai::brain::brain(w, i).home.is_none() && home_lost.is_none() {
            home_lost = Some((t, w.day_time()));
        }
        if !redsim::entity::mob::ai::brain::has_exact(w, i) {
            continue;
        }
        let x = redsim::entity::mob::ai::brain::ex(w, i);
        let act = format!("{:?}", x.active);
        let stamp = x.mem.cant_reach_since.map_or(0, |s| w.time - s);
        stamp_max = stamp_max.max(stamp);
        if act.contains("Panic") && first_panic.is_none() {
            first_panic = Some(t);
        }
        if act.contains("Rest") {
            calm_rest += 1;
            stamp_calm_max = stamp_calm_max.max(stamp);
            let d = w.entities.list[i].pos.distance_to_sqr(head).sqrt();
            d_min = d_min.min(d);
            d_max = d_max.max(d);
        }
        *acts.entry(act).or_default() += 1;
    }
    Ok(vec![
        format!(
            "(wait) the rider held on the queue rail from day time {start} for {ticks} gt (its HOME the bed at {bed:?}): HOME {}; the rider {}; first PANIC at t {first_panic:?}",
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
            "(wait) calm REST ticks {calm_rest}{}; the stamp's age: most {stamp_max}, most at a calm REST tick {stamp_calm_max} (bar 1,000: {})",
            if calm_rest > 0 { format!(" at {d_min:.2}..{d_max:.2} from the bed's head (inside 16: the 16-block rule)") } else { String::new() },
            if stamp_calm_max <= 1000 { "PASS" } else { "FAIL" }
        ),
    ])
}

/// The napper's way down (the leg, the shaft, the queue): a cart with its rider started at `start`
/// with `motion`, followed until it rests on the queue rail: the tick it is on the HOLD detector
/// (`hold`), the tick it first comes within 16 of the bed's head (`bed`: ValidateNearbyPoi's
/// 16-ball), the tick it reaches the arrival detector and the queue cell: HOLD's lead.
pub fn leg_lines(spec: &FarmSpec, o: &Opts, start: [f64; 3], motion: [f64; 3], hold: P, bed: P, ticks: u32) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(rail_cells(q, o.south).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    place_checked(&mut run.world, &cells)?;
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&format!("{{Motion:[{:.4}d,{:.4}d,{:.4}d],Passengers:[{{id:\"minecraft:villager\"}}]}}", motion[0], motion[1], motion[2])))?;
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
    for t in 0..ticks {
        run.tick();
        let Some(e) = run.world.entities.list.iter().find(|e| e.id == cart && !e.removed) else { break };
        let c = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
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
        }
        if at_queue.is_none() && c == q && e.delta_movement.length_sqr() < 1e-6 {
            at_queue = Some(t);
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
        format!("(leg) the cells: {:?}", path.iter().take(60).collect::<Vec<_>>()),
    ])
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
        rail_cells(q, o.south)
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
            if !o.replace.contains(p) {
                return Err(format!("{p:?} ({what}) already holds {have}"));
            }
            lines.push(format!("  {p:?} {b} ({what}; REPLACES the module's {have})"));
            d.blocks.insert(*p, b.to_string());
            continue;
        }
        d.blocks.insert(*p, b.to_string());
        lines.push(format!("  {p:?} {b} ({what})"));
    }
    let cart = mctas::design::EntitySpec { kind: "minecraft:minecart".into(), pos: [q[0] as f64 + 0.5, q[1] as f64 + if o.rail { 0.5625 } else { 0.1875 }, q[2] as f64 + 0.5], nbt: Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".into()) };
    let boat = mctas::design::EntitySpec { kind: "minecraft:oak_boat".into(), pos: [post[0] as f64 + 0.5, post[1] as f64, post[2] as f64 + 0.5], nbt: Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into()) };
    lines.push(format!("  entity {} {:?} (the seated queue rider, {})", cart.kind, cart.pos, if o.rail { "on the braked queue rail" } else { "on the trapdoor" }));
    lines.push(format!("  entity {} {:?} (the queue's zombie)", boat.kind, boat.pos));
    d.entities.push(cart);
    d.entities.push(boat);
    d.save(out).map_err(|e| e.to_string())?;
    lines.insert(0, format!("wrote {} (the module {} with the way-in line's queue cell and post):", out.display(), module.display()));
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
