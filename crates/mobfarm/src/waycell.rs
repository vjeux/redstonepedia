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

/// The module built as the judge places it, with the post's glass and leaves and its zombie boat
/// when given, and the queue cart with its rider; a few ticks to settle. The rider's and the
/// zombie's (feet, eye), the world.
fn scene(spec: &FarmSpec, o: &Opts, post: Option<P>) -> Result<(redsim::world::World, (Vec3, Vec3), Option<(Vec3, Vec3)>), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let mut w = run.world;
    // (the extra cells: the run, an occluder)
    let extra = o.extra.iter().map(|(p, b)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(b).map_err(|e| format!("{b}: {e}"))?))).collect::<Result<Vec<_>, String>>()?;
    w.place_raw(&extra);
    if let Some(p) = post {
        if !o.custom_post {
            w.place_raw(&[
                (BlockPos::new(p[0], p[1] - 1, p[2]), Block::parse("minecraft:glass").map_err(|e| e.to_string())?),
                (BlockPos::new(p[0], p[1] + 3, p[2]), Block::parse("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]").map_err(|e| e.to_string())?),
            ]);
        }
        redsim::entity::spawn(&mut w, "minecraft:oak_boat", [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    }
    let q = o.queue;
    // (the queue cell: a shut bottom-half iron trapdoor the cart rests on, 3/16 high; or with
    // `rail`, a powered rail on glowstone ascending to the east, the cart at its middle)
    let qy = if o.rail {
        w.place_raw(&rail_cells(q).iter().map(|(p, b, _)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(b).map_err(|e| e.to_string())?))).collect::<Result<Vec<_>, String>>()?);
        q[1] as f64 + 0.5625
    } else {
        if w.get(BlockPos::new(q[0], q[1], q[2])).is_air() {
            w.place_raw(&[(BlockPos::new(q[0], q[1], q[2]), Block::parse("minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]").map_err(|e| e.to_string())?)]);
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
    // (the burn rule: `Zombie.isSunBurnTick` needs `level.canSeeSky` at its eye; the leaves end
    // the sky column there; the eye's sky light printed when the light model reaches it)
    let sees_sky = redsim::terrain::land::can_see_sky(&w, eye);
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
            Ok(false) => "cannot see the sky: no sunburn, PASS".to_string(),
            Ok(true) => "SEES THE SKY (FAIL)".to_string(),
            Err(e) => format!("{e} (FAIL)"),
        },
        sky.as_ref().map_or_else(|e| e.clone(), |l| l.to_string())
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
pub fn rail_cells(q: P) -> Vec<(P, &'static str, &'static str)> {
    vec![
        ([q[0] + 1, q[1], q[2]], "minecraft:glass", "under the arrival detector (glass: a rail on glass is never a stroll draw)"),
        ([q[0] + 1, q[1] + 1, q[2]], "minecraft:detector_rail[powered=false,shape=ascending_east,waterlogged=false]", "the arrival detector (the shaft's foot: sets HELD, starts the hold timer)"),
        ([q[0], q[1] - 1, q[2]], "minecraft:glowstone", "under the queue rail (a solid-render floor: the rider's own cell a reached draw)"),
        (q, "minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false]", "the queue rail (unpowered it holds the cart; day-1's gate powers it as a level: the cart rolls west down the run)"),
        ([q[0] - 1, q[1] - 2, q[2]], "minecraft:glass", "under the departure detector (glass: never a stroll draw)"),
        ([q[0] - 1, q[1] - 1, q[2]], "minecraft:detector_rail[powered=false,shape=ascending_east,waterlogged=false]", "the departure detector (resets HELD and the RS)"),
    ]
}

/// The ride from the queue: the queue rail powered from its input cell (a redstone block at queue +
/// (0,0,1), as day-1's RELEASE level would), the cart with its rider followed for `ticks`: the cells
/// its cart passes, the tick it leaves the queue cell, crosses the departure detector and reaches
/// `entry` (the stop's first rail), the rider still seated.
pub fn ride_lines(spec: &FarmSpec, o: &Opts, entry: P, ticks: u32) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(rail_cells(q).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    run.world.place_raw(&cells);
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [q[0] as f64 + 0.5, q[1] as f64 + 0.5625, q[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
    // settle on the braked rail, then release
    for _ in 0..40 {
        run.tick();
    }
    let held_at = run.world.entities.list.iter().find(|e| e.id == cart).map(|e| e.pos);
    run.world.set_block(BlockPos::new(q[0], q[1], q[2] + 1), Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
    let (mut left, mut dep, mut at_entry) = (None, None, None);
    let mut path: Vec<P> = Vec::new();
    let mut seated = true;
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
        if dep.is_none() && c == [q[0] - 1, q[1] - 1, q[2]] {
            dep = Some(t);
        }
        if at_entry.is_none() && c == entry {
            at_entry = Some(t);
        }
        seated &= run.world.entities.list.iter().any(|v| !v.removed && v.vehicle == Some(cart));
    }
    Ok(vec![
        format!("(ride) held at {:?} after 40 gt on the braked rail; released (the rail powered) at t 0", held_at.map(|p| (p.x, p.y, p.z))),
        format!("(ride) left the queue cell at t {left:?}, on the departure detector at t {dep:?}, at the stop's entry {entry:?} at t {at_entry:?}; the rider {} seated", if seated { "stayed" } else { "did NOT stay" }),
        format!("(ride) the cells: {:?}", path.iter().take(24).collect::<Vec<_>>()),
    ])
}

/// The queued napper's all-day wait (ironspec 10:18): the rider held on the queue rail from day
/// time `start` for `ticks` gt with its HOME the stop's bed (`bed`, a loaded HOME memory), the
/// post's zombie in place: HOME at every tick, its activity tick by tick, the calm REST ticks
/// (REST active, not PANIC) and their distance to the bed's head, its CANT_REACH stamp's age
/// (the most, and the most at a calm REST tick: the bar 1,000).
pub fn wait_lines(spec: &FarmSpec, o: &Opts, post: P, start: i64, ticks: u32, bed: P) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, ticks + 10, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(rail_cells(q).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    if !o.custom_post {
        cells.push((BlockPos::new(post[0], post[1] - 1, post[2]), Block::parse("minecraft:glass").map_err(|e| e.to_string())?));
        cells.push((BlockPos::new(post[0], post[1] + 3, post[2]), Block::parse("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]").map_err(|e| e.to_string())?));
    }
    run.world.place_raw(&cells);
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
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let q = o.queue;
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for (p, b) in o.extra.iter().map(|(p, b)| (*p, b.clone())).chain(rail_cells(q).into_iter().map(|(p, b, _)| (p, b.to_string()))) {
        cells.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&b).map_err(|e| format!("{b}: {e}"))?));
    }
    run.world.place_raw(&cells);
    let cart = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&format!("{{Motion:[{:.4}d,{:.4}d,{:.4}d],Passengers:[{{id:\"minecraft:villager\"}}]}}", motion[0], motion[1], motion[2])))?;
    let head = Vec3::new(bed[0] as f64 + 0.5, bed[1] as f64 + 0.5625, bed[2] as f64 + 0.5);
    let arrival = [q[0] + 1, q[1] + 1, q[2]];
    let (mut on_hold, mut ball, mut at_arrival, mut at_queue) = (None, None, None, None);
    let mut path: Vec<P> = Vec::new();
    for t in 0..ticks {
        run.tick();
        let Some(e) = run.world.entities.list.iter().find(|e| e.id == cart && !e.removed) else { break };
        let c = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
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
    Ok(vec![
        format!("(leg) on the HOLD detector {hold:?} at t {on_hold:?}; within 16 of the bed's head at t {ball:?} (HOLD's lead {lead}); at the arrival detector at t {at_arrival:?}; at rest on the queue rail at t {at_queue:?}"),
        format!("(leg) the cells: {:?}", path.iter().take(60).collect::<Vec<_>>()),
    ])
}

/// The way-in line's cells laid into the module file (a WIP design against its frame): the queue
/// cell's iron trapdoor (shut, bottom half: the cart rests on it, the drop under it when day-1's
/// gate opens it), the post's glass, its leaves, the queue cart with its rider and the post's boat
/// with its zombie (persistent). Refuses a cell the module already holds.
pub fn write_layout(module: &std::path::Path, out: &std::path::Path, o: &Opts, post: P) -> Result<Vec<String>, String> {
    let mut d = mctas::design::Design::load(module).map_err(|e| e.to_string())?;
    let q = o.queue;
    let mut cells: Vec<(P, &str, &str)> = if o.rail {
        rail_cells(q)
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
    lines.push(format!("  entity {} {:?} (the seated queue rider, on the trapdoor)", cart.kind, cart.pos));
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
