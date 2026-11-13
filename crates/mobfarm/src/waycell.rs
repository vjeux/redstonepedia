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
    let mut extra = o.extra.iter().map(|(p, b)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(b).map_err(|e| format!("{b}: {e}"))?))).collect::<Result<Vec<_>, String>>()?;
    extra.extend(r1_cells(o));
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
        let cells = queue_cells(o).iter().map(|(p, b, _)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(b).map_err(|e| e.to_string())?))).collect::<Result<Vec<_>, String>>()?;
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
/// day-1's release stage R1 (17:57), laid in every bench world as emulated cells (not written into
/// the design: they are day-1's): the repeaters (8,-14,-4) and (7,-14,-4) facing west (input west)
/// into the queue rail, each on glass; none solid-render.
pub fn r1_cells(o: &Opts) -> Vec<(BlockPos, Block)> {
    if !o.south {
        return Vec::new();
    }
    let q = o.queue;
    let g = Block::parse("minecraft:glass").unwrap();
    let rep = Block::parse("minecraft:repeater[delay=1,facing=west,locked=false,powered=false]").unwrap();
    vec![
        (BlockPos::new(q[0] - 1, q[1] - 1, q[2]), g),
        (BlockPos::new(q[0] - 1, q[1], q[2]), rep),
        (BlockPos::new(q[0] - 2, q[1] - 1, q[2]), g),
        (BlockPos::new(q[0] - 2, q[1], q[2]), rep),
    ]
}

/// The queue's release as the bench emulates day-1's RELEASE level: for the south staircase, R1's
/// input driven by a redstone torch standing on glass at queue - (3,0,0) (the repeaters of
/// [`r1_cells`] carry it into the rail; nothing solid-render); for the west one a redstone block
/// (its old emulation).
fn release(w: &mut redsim::world::World, o: &Opts) -> Result<(), String> {
    let q = o.queue;
    if o.south {
        for (p, b) in r1_cells(o) {
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
    cells.extend(r1_cells(o));
    place_checked(&mut run.world, &cells)?;
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
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o));
    place_checked(&mut base.world, &cells)?;
    let spawn_at = o.seat_at.unwrap_or([q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5]);
    let cart0 = redsim::entity::spawn(&mut base.world, "minecraft:minecart", spawn_at, Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
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
        release(&mut run.world, o)?;
        let mut cart = cart0;
        let rider_of = |w: &redsim::world::World, c: u32| w.entities.list.iter().position(|v| !v.removed && v.vehicle == Some(c));
        let h0 = rider_of(&run.world, cart).map(|i| redsim::entity::mob::mob(&run.world, i).health).unwrap_or(0.0);
        let mut h_min = h0;
        let (mut at_entry, mut v_entry, mut landed, mut derailed, mut back_south) = (None, None, None::<u32>, false, false);
        let (mut box_z_max, mut vz_max) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut restart_cell: Option<P> = None;
        for t in 0..ticks {
            if let Some((rt, how)) = restart {
                if t as i64 == rt {
                    // (G1 on e9f0f4cbf: the cart's cell at the restart, so a row shows where it was)
                    restart_cell = run.world.entities.list.iter().find(|e| e.id == cart && !e.removed).map(|e| [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32]);
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
        rows.push((
            format!(
                "{label}{} [after the landing: the box's z at most {box_z_max:.4}, vz at most {vz_max:.4}]",
                restart_cell.map_or(String::new(), |c| format!(" (the cart at {c:?} at the restart)"))
            ),
            at_entry,
            v_entry,
            h0 - h_min,
            derailed || (landing.is_some() && landed.is_none()),
            back_south,
        ));
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
    cells.extend(r1_cells(o));
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
            "(wait) calm REST ticks {calm_rest}{}; the stamp's age: most {stamp_max}, most at a calm REST tick {stamp_calm_max} (bar 1,000: {})",
            if calm_rest > 0 { format!(" at {d_min:.2}..{d_max:.2} from the bed's head (inside 16: the 16-block rule)") } else { String::new() },
            if stamp_calm_max <= 1000 { "PASS" } else { "FAIL" }
        ),
    ];
    if stamp.is_some() {
        out.push(format!("(wait) the armed stamp cleared at t {cleared_at:?} ({})", match cleared_at {
            Some(t) if t <= 100 => "within 100 gt of the arrival: PASS".to_string(),
            Some(_) => "LATER than 100 gt: FAIL".to_string(),
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
    Ok(out)
}

/// (ironspec 14:58 (2)) The queue's release at every phase: the rider waits `ticks` gt (from day
/// time `start`, its HOME the bed, its stamp armed `stamp` gt before its arrival when given), then
/// for k in 0..`phases` the run is cloned, held k gt more and released (the redstone block at the
/// input cell), and followed up to `follow` gt to the stop's entry: whether a walk target was
/// pending at the release (a draw in MoveToTargetSink, G1's case: pathed later from the drop), the
/// stamp's age at the release and at the entry, whether it armed after the release, HOME at the
/// entry. Then the share of releases with a draw pending, the stamps at the entry, the armings.
#[allow(clippy::too_many_arguments)]
pub fn sweep_lines(spec: &FarmSpec, o: &Opts, post: P, start: i64, ticks: u32, bed: P, stamp: Option<i64>, phases: u32, follow: u32, entry: P, restart_ticks: &[i64]) -> Result<Vec<String>, String> {
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
    cells.extend(r1_cells(o));
    place_checked(&mut base.world, &cells)?;
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
    Ok(out)
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
pub fn visit_lines(spec: &FarmSpec, o: &Opts, post: Option<P>, start: [f64; 3], motion: [f64; 3], bed: P, day: i64, phases: u32, holds: &[u32], restart_ticks: &[i64], entry: P) -> Result<Vec<String>, String> {
    let max_hold = holds.iter().copied().max().unwrap_or(80);
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, phases + 900 + max_hold, WalkMode::Random) };
    let (mut base, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o));
    place_checked(&mut base.world, &cells)?;
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
    let mut out = Vec::new();
    let (mut rows_n, mut before_n, mut armed_after_n, mut home_lost_n, mut entry_stamped_n) = (0u32, 0u32, 0u32, 0u32, 0u32);
    for k in 0..phases {
        // the leg and the shaft down to the arrival detector, shared by this phase's rows
        let mut w0 = base.clone();
        for _ in 0..k {
            w0.tick();
        }
        let cart = redsim::entity::spawn(&mut w0.world, "minecraft:minecart", start, Some(&format!("{{Motion:[{:.4}d,{:.4}d,{:.4}d],Passengers:[{rider_nbt}]}}", motion[0], motion[1], motion[2])))?;
        w0.world.mob.day_offset = day - w0.world.time;
        let rider = w0.world.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart)).map(|e| e.id).ok_or("no rider")?;
        let mut held_since: Option<u32> = None;
        let mut t_arr = None;
        for t in 0..600u32 {
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
            let mut rs: Vec<Option<(i64, crate::liveness::Restart)>> = vec![None];
            for &rt in restart_ticks {
                for how in crate::liveness::Restart::rows() {
                    rs.push(Some((rt, how)));
                }
            }
            for restart in rs {
                let mut r = w0.clone();
                let mut rider = rider;
                let mut cart = cart;
                let (mut held_end, mut cleared, mut armed_after, mut first_calm, mut at_entry, mut stamp_entry, mut home_entry) =
                    (None::<u32>, None::<(u32, String)>, Vec::<(u32, String)>::new(), None::<String>, None::<u32>, None::<Option<i64>>, None::<bool>);
                let mut was_held = held_since.is_some();
                let mut prev_since = mem(&r.world, i).and_then(|m| m.0);
                let mut released = false;
                // (farmopt 17:07) the seat's own HOME path at its first calm REST tick before the release
                let mut seat_calm: Option<String> = None;
                let (mut cart_last, mut entry_v): (Option<([i32; 3], f64)>, Option<f64>) = (None, None);
                for t in 0..(h + 400) {
                    if let Some((rt, how)) = restart {
                        if t as i64 == rt {
                            let map = crate::liveness::restart(&mut r.world, how)?;
                            rider = *map.get(&rider).ok_or("the rider lost in the restart")?;
                            cart = *map.get(&cart).ok_or("the cart lost in the restart")?;
                        }
                    }
                    if t == h {
                        release(&mut r.world, o)?;
                        released = true;
                    }
                    r.tick();
                    if std::env::var("WAYCELL_VISIT_TRACE").is_ok() && released && t < h + 12 {
                        if let Some(c) = r.world.entities.list.iter().find(|e| e.id == cart && !e.removed) {
                            let cc = BlockPos::new(c.pos.x.floor() as i32, c.pos.y.floor() as i32, c.pos.z.floor() as i32);
                            eprintln!("visit t {t}: cart ({:.3},{:.3},{:.3}) v ({:.3},{:.3},{:.3}) at {:?} {:?}; input {:?}", c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z, cc, r.world.get(cc), r.world.get(BlockPos::new(q[0] - 1, q[1], q[2])));
                        }
                    }
                    let Some(i) = idx(&r.world, rider) else { break };
                    let Some((since, wt, act)) = mem(&r.world, i) else { continue };
                    if was_held && !wt && held_end.is_none() {
                        held_end = Some(t);
                    }
                    was_held |= wt && held_end.is_none() && !released;
                    if since.is_none() && prev_since.is_some() && cleared.is_none() {
                        cleared = Some((t, act.clone()));
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
                            home_entry = Some(redsim::entity::mob::ai::brain::brain(&r.world, i).home.is_some());
                            break;
                        }
                    }
                    prev_since = since;
                }
                rows_n += 1;
                let before = cleared.as_ref().is_some_and(|(t, _)| *t < h) || (stamp_arr.is_none() && cleared.is_none());
                before_n += before as u32;
                armed_after_n += (!armed_after.is_empty()) as u32;
                home_lost_n += (home_entry == Some(false)) as u32;
                entry_stamped_n += matches!(stamp_entry, Some(Some(_))) as u32;
                if restart.is_none() || !before || !armed_after.is_empty() || home_entry != Some(true) {
                    out.push(format!(
                        "(visit) phase +{k}, hold {h}{}: arrival at t {t_arr} with the stamp {}; the held walk {} ; the seat's first calm REST tick {}; the stamp cleared {}; armings after the release {}; the first calm REST tick after it {}; at the entry {}: the stamp {}, HOME {}",
                        restart.map_or(String::new(), |(rt, how)| format!(", restart at +{rt} ({})", how.label())),
                        stamp_arr.map_or("none".to_string(), |a| format!("{a} gt")),
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
                        home_entry.map_or("-".to_string(), |b| if b { "kept".to_string() } else { "LOST".to_string() })
                    ));
                }
            }
        }
    }
    out.push(format!(
        "(visit) {rows_n} rows ({phases} arrival phases x holds {holds:?} x (plain + restarts at {restart_ticks:?} in 10 orders)), the post {}: the stamp cleared before the release in {before_n}; armed after the release in {armed_after_n}; stamped at the entry {entry_stamped_n}; HOME lost {home_lost_n}: {}",
        if post.is_some() { "in place" } else { "ABSENT (the (C') hatch shut at night / (C''))" },
        if before_n == rows_n && armed_after_n == 0 && home_lost_n == 0 { "PASS" } else { "FAIL" }
    ));
    Ok(out)
}

/// (farmopt 17:07: the seat's own HOME path) A fresh rider seated in a cart at each of `at` (cart
/// positions), a few ticks to settle, and the HOME path from there to `bed`: its start node, end
/// node, complete or partial, and its nodes.
pub fn path_probe_lines(spec: &FarmSpec, o: &Opts, at: &[[f64; 3]], bed: P) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 40, WalkMode::Random) };
    let (mut base, _) = crate::placing::start(&widened(spec, o), &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(queue_cells(o).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    cells.extend(r1_cells(o));
    place_checked(&mut base.world, &cells)?;
    let bed_p = BlockPos::new(bed[0], bed[1], bed[2]);
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
    Ok(out)
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
    cells.extend(r1_cells(o));
    place_checked(&mut run.world, &cells)?;
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
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&format!("{{Motion:[{:.4}d,{:.4}d,{:.4}d],Passengers:[{rider_nbt}]}}", motion[0], motion[1], motion[2])))?;
    if let Some(d) = day {
        run.world.mob.day_offset = d - run.world.time;
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
