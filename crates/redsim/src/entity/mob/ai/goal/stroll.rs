//! `RandomStrollGoal`, `WaterAvoidingRandomStrollGoal`, and the random positions they pick
//! (`RandomPos`, `DefaultRandomPos`, `LandRandomPos`, `GoalUtils`).

use super::super::path::{PathType, WalkEval};
use super::super::{ai, nav_mob, with_nav};
use crate::block::{Block, Simple};
use crate::entity::mob::mob;
use crate::entity::{Vec3, mth};
use crate::pos::BlockPos;
use crate::world::World;

/// `RandomStrollGoal` (and its water-avoiding subclass: `water_avoiding` = its probability).
#[derive(Clone, PartialEq, Debug)]
pub struct RandomStroll {
    pub speed_modifier: f64,
    pub interval: i32,
    pub force_trigger: bool,
    pub check_no_action_time: bool,
    pub wanted: Vec3,
    pub water_avoiding: Option<f32>,
    pub variant: Variant,
}

/// Which `RandomStrollGoal` subclass (its `canUse` gate and `getPosition`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Plain,
    /// `MoveBackToVillageGoal`: only outside a village, toward the closest village section.
    MoveBackToVillage,
    /// `GolemRandomStrollInVillageGoal`: its choices from the level random.
    GolemInVillage,
    /// `AbstractFish.FishSwimGoal` (`RandomSwimmingGoal`): only a fish that may swim at random
    /// (`canRandomSwim`: no live leader), toward `getRandomSwimmablePos(10, 7)`.
    FishSwim,
}

impl RandomStroll {
    /// `new WaterAvoidingRandomStrollGoal(mob, speed)` (interval 120, probability 0.001).
    pub fn water_avoiding(speed: f64) -> RandomStroll {
        RandomStroll { speed_modifier: speed, interval: 120, force_trigger: false, check_no_action_time: true, wanted: Vec3::ZERO, water_avoiding: Some(0.001), variant: Variant::Plain }
    }

    /// `RandomStrollGoal(mob, speed)`: the plain stroll (interval 120; `DefaultRandomPos.getPos(10, 7)`).
    pub fn default_interval(speed: f64) -> RandomStroll {
        RandomStroll { speed_modifier: speed, interval: 120, force_trigger: false, check_no_action_time: true, wanted: Vec3::ZERO, water_avoiding: None, variant: Variant::Plain }
    }

    /// `new FishSwimGoal(fish)`: `RandomSwimmingGoal(fish, 1.0, 40)`.
    pub fn fish_swim() -> RandomStroll {
        RandomStroll { speed_modifier: 1.0, interval: 40, force_trigger: false, check_no_action_time: true, wanted: Vec3::ZERO, water_avoiding: None, variant: Variant::FishSwim }
    }
}

fn rnd(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("a mob's own random")
}

/// `canUse()`.
pub fn can_use(w: &mut World, i: usize, g: &mut RandomStroll) -> bool {
    if g.variant == Variant::MoveBackToVillage && !super::golem::back_to_village_gate(w, i) {
        return false;
    }
    // FishSwimGoal.canUse: canRandomSwim() first
    if g.variant == Variant::FishSwim && !super::super::fish::can_random_swim(w, i) {
        return false;
    }
    // hasControllingPassenger: a mob steering this one (a spider jockey's skeleton)
    if crate::entity::mob::ai::controlled_by_mob(w, i) {
        return false;
    }
    if !g.force_trigger {
        if g.check_no_action_time && mob(w, i).no_action_time >= 100 {
            return false;
        }
        let n = super::reduced_tick_delay(g.interval);
        if rnd(w, i).next_int(n) != 0 {
            return false;
        }
    }
    let Some(p) = position(w, i, g) else { return false };
    g.wanted = p;
    g.force_trigger = false;
    true
}

/// `getPosition()`.
fn position(w: &mut World, i: usize, g: &RandomStroll) -> Option<Vec3> {
    match g.variant {
        Variant::MoveBackToVillage => return super::golem::back_to_village_position(w, i),
        Variant::GolemInVillage => return super::golem::golem_village_position(w, i),
        Variant::FishSwim => return super::super::fish::swimmable_pos(w, i, 10, 7),
        Variant::Plain => {}
    }
    match g.water_avoiding {
        None => default_random_pos(w, i, 10, 7),
        Some(prob) => {
            if w.entities.list[i].was_touching_water {
                land_random_pos(w, i, 15, 7).or_else(|| default_random_pos(w, i, 10, 7))
            } else if rnd(w, i).next_float() >= prob {
                land_random_pos(w, i, 10, 7)
            } else {
                default_random_pos(w, i, 10, 7)
            }
        }
    }
}

/// `canContinueToUse()`.
pub fn can_continue(w: &mut World, i: usize, _g: &mut RandomStroll) -> bool {
    !crate::entity::mob::ai::nav_of(w, i).is_done() && !crate::entity::mob::ai::controlled_by_mob(w, i)
}

/// `start()`: `navigation.moveTo(wanted, speed)`.
pub fn start(w: &mut World, i: usize, g: &mut RandomStroll) {
    let (p, s) = (g.wanted, g.speed_modifier);
    with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, p.x, p.y, p.z, 1, s, arena));
}

// ------------------------------------------------------------------- random positions

/// `RandomPos.generateRandomDirection(random, h, v)`.
pub fn random_direction(w: &mut World, i: usize, h: i32, v: i32) -> BlockPos {
    let r = rnd(w, i);
    let x = r.next_int(2 * h + 1) - h;
    let y = r.next_int(2 * v + 1) - v;
    let z = r.next_int(2 * h + 1) - h;
    BlockPos::new(x, y, z)
}

/// `RandomPos.generateRandomPosTowardDirection(mob, xzDist, random, direction)`: with a home and
/// `xzDist` over 1, x and z pulled toward it by `nextDouble() * xzDist / 2` each (mob-AI
/// 2026-11-11).
fn toward_direction(w: &mut World, i: usize, h: f64, d: BlockPos) -> BlockPos {
    let (mut xt, mut zt) = (d.x as f64, d.z as f64);
    if let Some((c, _)) = home(w, i)
        && h > 1.0
    {
        let p = w.entities.list[i].pos;
        let a = rnd(w, i).next_double() * h / 2.0;
        if p.x > c.x as f64 {
            xt -= a;
        } else {
            xt += a;
        }
        let b = rnd(w, i).next_double() * h / 2.0;
        if p.z > c.z as f64 {
            zt -= b;
        } else {
            zt += b;
        }
    }
    let e = &w.entities.list[i];
    BlockPos::new(mth::floor(xt + e.pos.x), mth::floor(d.y as f64 + e.pos.y), mth::floor(zt + e.pos.z))
}

/// (mob-AI 2026-11-11) `Mob.homePosition` and `homeRadius` of mob `i` (`None`: no home).
pub fn home(w: &World, i: usize) -> Option<(BlockPos, i32)> {
    if super::super::has_ai(w, i) { super::super::ai(w, i).home } else { None }
}

/// `GoalUtils.mobRestricted(mob, h)`: a home whose centre is within its radius + h + 1.
fn mob_restricted(w: &World, i: usize, h: f64) -> bool {
    match home(w, i) {
        Some((c, r)) => {
            let p = w.entities.list[i].pos;
            let (dx, dy, dz) = (c.x as f64 + 0.5 - p.x, c.y as f64 + 0.5 - p.y, c.z as f64 + 0.5 - p.z);
            let d = r as f64 + h + 1.0;
            dx * dx + dy * dy + dz * dz < d * d
        }
        None => false,
    }
}

/// `Mob.isWithinHome(pos)`: no home, or the squared block distance under the radius squared.
pub fn within_home(w: &World, i: usize, p: BlockPos) -> bool {
    match home(w, i) {
        None => true,
        Some((c, r)) => {
            let (dx, dy, dz) = ((c.x - p.x) as f64, (c.y - p.y) as f64, (c.z - p.z) as f64);
            dx * dx + dy * dy + dz * dz < (r * r) as f64
        }
    }
}

/// `Level.isOutsideBuildHeight(y)` (the level's own height: the Nether's 0..255).
fn outside_build_height(w: &World, y: i32) -> bool {
    y < w.level_min_y() || y > w.level_max_y()
}

/// `PathNavigation.isStableDestination(pos)`: the block below renders solid.
pub fn is_stable_destination(w: &World, p: BlockPos) -> bool {
    is_solid_render(&w.get(p.below()))
}

/// `mob.getNavigation().isStableDestination(pos)`: an amphibious navigation's (a drowned's) only
/// wants a block below that is not air; a water-bound one's (a fish's) a cell that does not render
/// solid.
fn stable_for(w: &World, i: usize, p: BlockPos) -> bool {
    match ai(w, i).nav.kind {
        super::super::path::navigation::NavKind::Amphibious => return !w.get(p.below()).is_air(),
        super::super::path::navigation::NavKind::WaterBound => return !is_solid_render(&w.get(p)),
        _ => {}
    }
    is_solid_render(&w.get(p.below()))
}

/// `GoalUtils.hasMalus(mob, pos)`: the static path type there costs something.
fn has_malus(w: &World, i: usize, p: BlockPos) -> bool {
    let arena = &mut super::super::path::node::Arena::default();
    let eval = WalkEval::new(w, nav_mob(w, i).to_path_mob(), ai(w, i).nav.flags, arena);
    let t = eval.path_type_static(p.x, p.y, p.z);
    ai(w, i).malus.get(t) != 0.0
}

/// The weight `RandomPos.generateRandomPos` keeps the best of (`getWalkTargetValue`): a monster's
/// is minus the light cost (`-getPathfindingCostFromLightLevels`), others 0.
pub fn walk_target_value(w: &mut World, i: usize, p: BlockPos) -> f64 {
    let ty = mob(w, i).ty;
    // an Animal's: 10 on a grass block, else the light's cost
    if super::animal::is_animal(ty.def().id) {
        return super::animal::walk_target_value(w, p);
    }
    // an enderman's: every cell 0 (`Enderman.getWalkTargetValue`)
    if !ty.def().monster || ty.def().id == "enderman" {
        return 0.0;
    }
    -(light_magic(w, p) - 0.5) as f64
}

/// `getLightLevelDependentMagicValue(pos)` (overworld: ambient light 0): the raw brightness
/// `max(block, sky - skyDarken)`, curved. A design world (no land) has no block light engine: the
/// block light is 0 when no light source is within its reach of `pos`, else a model error.
pub fn light_magic(w: &mut World, p: BlockPos) -> f32 {
    let darken = w.sky_darken();
    let raw = if w.land.is_some() {
        match w.raw_brightness(p, darken) {
            Ok(v) => v,
            Err(e) => {
                if !w.errors.contains(&e) {
                    w.errors.push(e);
                }
                15
            }
        }
    } else {
        // a solid-rendering block holds no light (the engine's getLightBlock 15); the rest from
        // the world's sky light (cached until a block changes: daylight's per-query fixed point
        // is costly under a roof)
        let sky = if is_solid_render(&w.get(p)) {
            Ok(0)
        } else if let Some(v) = w.mob.ai.sky_cache.get(&p) {
            Ok(*v)
        } else {
            let v = crate::daylight::sky_light(w, p);
            if let Ok(s) = v {
                w.mob.ai.sky_cache.insert(p, s);
            }
            v
        };
        let sky = match sky {
            Ok(v) => v,
            Err(e) => {
                if !w.errors.contains(&e) {
                    w.errors.push(e);
                }
                15
            }
        };
        // the block light, where a light source can reach (the light engine's propagation over
        // the cube that can lift the reading above the sky's: daylight::block_light_from; cached
        // until a block changes)
        let s = (sky - darken).max(0);
        let block = if is_solid_render(&w.get(p)) || light_source_near(w, p).is_none() {
            0
        } else if let Some(v) = w.mob.ai.block_cache.get(&(p, s)) {
            *v
        } else {
            match crate::daylight::block_light_from(w, p, s + 1, &crate::entity::mob::sun::emission) {
                Ok(v) => {
                    w.mob.ai.block_cache.insert((p, s), v);
                    v
                }
                Err(e) => {
                    let why = format!("the block light at {p}: {e}");
                    if !w.errors.contains(&why) {
                        w.errors.push(why);
                    }
                    0
                }
            }
        };
        s.max(block)
    };
    let v = raw as f32 / 15.0;
    v / (4.0 - 3.0 * v)
}

/// A light source whose light reaches `p` (its emission above the taxicab distance), from the
/// world's list of sources (built on first use, rebuilt after one changes).
fn light_source_near(w: &mut World, p: BlockPos) -> Option<BlockPos> {
    if w.mob.ai.lights.is_none() {
        let mut v = Vec::new();
        let (min, size) = (w.min, w.size);
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let q = BlockPos::new(min.x + x, min.y + y, min.z + z);
                    let e = crate::entity::mob::sun::emission(&w.get(q));
                    if e > 0 {
                        v.push((q, e));
                    }
                }
            }
        }
        w.mob.ai.lights = Some(v);
    }
    w.mob.ai.lights.as_deref().unwrap().iter().find(|(q, e)| (q.x - p.x).abs() + (q.y - p.y).abs() + (q.z - p.z).abs() < *e).map(|(q, _)| *q)
}

/// `RandomPos.generateRandomPos(supplier, weight)`: the best of 10 tries, its bottom centre.
fn best_of_ten(w: &mut World, i: usize, supplier: impl FnMut(&mut World) -> Option<BlockPos>) -> Option<Vec3> {
    best_of_ten_by(w, i, supplier, |w, p| walk_target_value(w, i, p))
}

/// `RandomPos.generateRandomPos(supplier, positionWeight)`: ten tries, the first best weight.
fn best_of_ten_by(w: &mut World, _i: usize, mut supplier: impl FnMut(&mut World) -> Option<BlockPos>, mut weight: impl FnMut(&mut World, BlockPos) -> f64) -> Option<Vec3> {
    let mut best_w = f64::NEG_INFINITY;
    let mut best: Option<BlockPos> = None;
    for _ in 0..10 {
        if let Some(p) = supplier(w) {
            let v = weight(w, p);
            if v > best_w {
                best_w = v;
                best = Some(p);
            }
        }
    }
    best.map(|p| Vec3::new(p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5))
}

/// `LandRandomPos.getPos(mob, h, v, positionWeight)`.
pub fn land_random_pos_by(w: &mut World, i: usize, h: i32, v: i32, weight: impl FnMut(&mut World, BlockPos) -> f64) -> Option<Vec3> {
    let restrict = mob_restricted(w, i, h as f64);
    best_of_ten_by(
        w,
        i,
        |w| {
            let d = random_direction(w, i, h, v);
            let p = toward_direction(w, i, h as f64, d);
            if outside_build_height(w, p.y) || (restrict && !within_home(w, i, p)) || !stable_for(w, i, p) {
                return None;
            }
            move_up_out_of_solid(w, i, p)
        },
        weight,
    )
}

/// `DefaultRandomPos.getPos(mob, h, v)`.
pub fn default_random_pos(w: &mut World, i: usize, h: i32, v: i32) -> Option<Vec3> {
    let restrict = mob_restricted(w, i, h as f64);
    best_of_ten(w, i, |w| {
        let d = random_direction(w, i, h, v);
        let p = toward_direction(w, i, h as f64, d);
        (!outside_build_height(w, p.y) && !(restrict && !within_home(w, i, p)) && stable_for(w, i, p) && !has_malus(w, i, p)).then_some(p)
    })
}

/// `LandRandomPos.getPos(mob, h, v)`.
pub fn land_random_pos(w: &mut World, i: usize, h: i32, v: i32) -> Option<Vec3> {
    let restrict = mob_restricted(w, i, h as f64);
    best_of_ten(w, i, |w| {
        let d = random_direction(w, i, h, v);
        let p = toward_direction(w, i, h as f64, d);
        if outside_build_height(w, p.y) || (restrict && !within_home(w, i, p)) || !stable_for(w, i, p) {
            return None;
        }
        move_up_out_of_solid(w, i, p)
    })
}

/// `RandomPos.generateRandomDirectionWithinRadians(random, minH, maxH, v, flyingHeight, xDir,
/// zDir, maxRadians)`.
pub fn random_direction_within_radians(w: &mut World, i: usize, min_h: f64, max_h: f64, v: i32, flying: i32, x_dir: f64, z_dir: f64, max_rad: f64) -> Option<BlockPos> {
    let center = mth::atan2(z_dir, x_dir) - (std::f64::consts::PI / 2.0) as f32 as f64;
    let r = rnd(w, i);
    let f = r.next_float();
    let y_rad = center + (2.0f32 * f - 1.0f32) as f64 * max_rad;
    let d = r.next_double();
    let dist = mth::lerp(d.sqrt(), min_h, max_h) * 2.0f32.sqrt() as f64;
    let xt = -dist * crate::entity::jmath::sin(y_rad);
    let zt = dist * crate::entity::jmath::cos(y_rad);
    if !(xt.abs() > max_h) && !(zt.abs() > max_h) {
        let yt = rnd(w, i).next_int(2 * v + 1) - v + flying;
        Some(BlockPos::new(mth::floor(xt), yt, mth::floor(zt)))
    } else {
        None
    }
}

/// `LandRandomPos.getPosAway(mob, minH, maxH, v, avoidPos)` (`getPosInDirection` away from it).
pub fn land_pos_away(w: &mut World, i: usize, min_h: f64, max_h: f64, v: i32, avoid: Vec3) -> Option<Vec3> {
    let p = w.entities.list[i].pos;
    let mut dir = Vec3::new(p.x - avoid.x, p.y - avoid.y, p.z - avoid.z);
    if (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt() == 0.0 {
        let a = rnd(w, i).next_double() - 0.5;
        let b = rnd(w, i).next_double() - 0.5;
        dir = Vec3::new(a, 0.0, b);
    }
    land_pos_in_direction(w, i, min_h, max_h, v, dir)
}

/// `LandRandomPos.getPosInDirection(mob, minH, maxH, v, dir, restrict)` (`restrict`:
/// `mobRestricted(mob, maxH)`, as both its callers take it).
pub fn land_pos_in_direction(w: &mut World, i: usize, min_h: f64, max_h: f64, v: i32, dir: Vec3) -> Option<Vec3> {
    let restrict = mob_restricted(w, i, max_h);
    best_of_ten(w, i, |w| {
        let d = random_direction_within_radians(w, i, min_h, max_h, v, 0, dir.x, dir.z, (std::f64::consts::PI / 2.0) as f32 as f64)?;
        let p = toward_direction(w, i, max_h, d);
        if outside_build_height(w, p.y) || (restrict && !within_home(w, i, p)) || !stable_for(w, i, p) {
            return None;
        }
        move_up_out_of_solid(w, i, p)
    })
}

/// `DefaultRandomPos.getPos(mob, h, v)`: ten random directions, each cell inside the world,
/// inside the home when restricted, stable and without malus, the best by walk-target value.
pub fn default_pos(w: &mut World, i: usize, h: i32, v: i32) -> Option<Vec3> {
    let restrict = mob_restricted(w, i, h as f64);
    best_of_ten(w, i, |w| {
        let d = random_direction(w, i, h, v);
        let p = toward_direction(w, i, h as f64, d);
        (!outside_build_height(w, p.y) && !(restrict && !within_home(w, i, p)) && stable_for(w, i, p) && !has_malus(w, i, p)).then_some(p)
    })
}

/// `DefaultRandomPos.getPosAway(mob, h, v, avoidPos)`: ten random directions within a quarter
/// turn of the way away from it, each cell inside the world, inside the home when restricted,
/// stable and without malus, the best by walk-target value.
pub fn default_pos_away(w: &mut World, i: usize, h: i32, v: i32, avoid: Vec3) -> Option<Vec3> {
    let p = w.entities.list[i].pos;
    let dir = Vec3::new(p.x - avoid.x, p.y - avoid.y, p.z - avoid.z);
    let restrict = mob_restricted(w, i, h as f64);
    best_of_ten(w, i, |w| {
        let d = random_direction_within_radians(w, i, 0.0, h as f64, v, 0, dir.x, dir.z, (std::f64::consts::PI / 2.0) as f32 as f64)?;
        let p = toward_direction(w, i, h as f64, d);
        (!outside_build_height(w, p.y) && !(restrict && !within_home(w, i, p)) && stable_for(w, i, p) && !has_malus(w, i, p)).then_some(p)
    })
}

/// `DefaultRandomPos.getPosTowards(mob, h, v, towardsPos, maxRadians)`.
pub fn default_pos_towards(w: &mut World, i: usize, h: i32, v: i32, towards: Vec3, max_rad: f64) -> Option<Vec3> {
    let p = w.entities.list[i].pos;
    let dir = Vec3::new(towards.x - p.x, towards.y - p.y, towards.z - p.z);
    let restrict = mob_restricted(w, i, h as f64);
    best_of_ten(w, i, |w| {
        let d = random_direction_within_radians(w, i, 0.0, h as f64, v, 0, dir.x, dir.z, max_rad)?;
        let p = toward_direction(w, i, h as f64, d);
        (!outside_build_height(w, p.y) && !(restrict && !within_home(w, i, p)) && stable_for(w, i, p) && !has_malus(w, i, p)).then_some(p)
    })
}

/// (tickexact 2026-11-11, the flee static check) Every cell `LandRandomPos.getPosAway(mob, 0, h, v,
/// avoid)` (`land_pos_away`) can return for mob `i` as it stands, each draw's cell before
/// `best_of_ten`'s choice: the columns whose square meets the draw's offsets (|x| and |z| at most h,
/// x² + z² under 2h², within a quarter turn of the way away from `avoid`), each y within v; stable,
/// moved up out of solid, not water, no malus. (A villager has no `Mob` home: the offsets are not
/// pulled.)
pub fn flee_candidates(w: &World, i: usize, h: f64, v: i32, avoid: Vec3) -> Vec<BlockPos> {
    let p = w.entities.list[i].pos;
    let (dx, dz) = (p.x - avoid.x, p.z - avoid.z);
    let any_way = dx == 0.0 && dz == 0.0;
    let ok = |x: f64, z: f64| x.abs() <= h && z.abs() <= h && x * x + z * z < 2.0 * h * h && (any_way || x * dx + z * dz >= 0.0);
    let mut out: Vec<BlockPos> = Vec::new();
    for ox in -(h as i32) - 1..=(h as i32) {
        for oz in -(h as i32) - 1..=(h as i32) {
            // (RandomPos floors the draw's offset first: the integer square [ox, ox+1) x [oz, oz+1)
            // in offset terms, sampled on a 9 x 9 grid with its edges; the cell then floor(o + pos))
            let meets = (0..=8).any(|a| (0..=8).any(|b| ok(ox as f64 + (a as f64 / 8.0).min(0.999_999), oz as f64 + (b as f64 / 8.0).min(0.999_999))));
            if !meets {
                continue;
            }
            let (cx, cz) = (mth::floor(ox as f64 + p.x), mth::floor(oz as f64 + p.z));
            for yt in -v..=v {
                let q = BlockPos::new(cx, mth::floor(yt as f64 + p.y), cz);
                if outside_build_height(w, q.y) || !stable_for(w, i, q) {
                    continue;
                }
                if let Some(r) = move_up_out_of_solid(w, i, q)
                    && !out.contains(&r)
                {
                    out.push(r);
                }
            }
        }
    }
    out
}

/// (tickexact 2026-11-11, the calm static check) Every cell `LandRandomPos.getPos(mob, h, v)`
/// (`land_random_pos`) can return for mob `i` as it stands: each offset x, z in -h..h and y in -v..v,
/// stable, moved up out of solid, not water, no malus.
pub fn stroll_candidates(w: &World, i: usize, h: i32, v: i32) -> Vec<BlockPos> {
    let p = w.entities.list[i].pos;
    let mut out: Vec<BlockPos> = Vec::new();
    for x in -h..=h {
        for z in -h..=h {
            for y in -v..=v {
                let q = BlockPos::new(mth::floor(x as f64 + p.x), mth::floor(y as f64 + p.y), mth::floor(z as f64 + p.z));
                if outside_build_height(w, q.y) || !stable_for(w, i, q) {
                    continue;
                }
                if let Some(r) = move_up_out_of_solid(w, i, q)
                    && !out.contains(&r)
                {
                    out.push(r);
                }
            }
        }
    }
    out
}

/// (tickexact 2026-11-11, the static checks) The cells a random-position draw of mob `i` can
/// return, each with the chance one event picks it. `dir`: the angle within a quarter turn of it,
/// the radius `sqrt(d) * h * sqrt 2`, |x| and |z| at most h (`RandomPos.generateRandomDirection
/// WithinRadians`, integrated on a 720 x 256 grid); None: x and z uniform in -h..h
/// (`generateRandomDirection`). y uniform in -v..v. `land`: LandRandomPos's filters (stable, moved up
/// out of solid, not water, no malus); else DefaultRandomPos's (stable, no malus). `draws`: the
/// draws an event makes before it gives up (the first valid one taken: a villager's walk-target
/// value is 0 everywhere, so `best_of_ten` keeps the first): 10 for one `getPos*` call, 100 for
/// SetWalkTargetAwayFrom's ten calls of getPosAway.
pub fn pick_shares(w: &World, i: usize, h: f64, v: i32, dir: Option<(f64, f64)>, land: bool, draws: i32) -> Vec<(BlockPos, f64)> {
    let p = w.entities.list[i].pos;
    let mut cols: std::collections::BTreeMap<(i32, i32), f64> = std::collections::BTreeMap::new();
    match dir {
        Some((dx, dz)) => {
            let center = mth::atan2(dz, dx) - (std::f64::consts::PI / 2.0) as f32 as f64;
            let max_rad = (std::f64::consts::PI / 2.0) as f32 as f64;
            let (na, nr) = (720usize, 256usize);
            for a in 0..na {
                let f = (a as f64 + 0.5) / na as f64;
                let y_rad = center + (2.0 * f - 1.0) * max_rad;
                for r in 0..nr {
                    let d = (r as f64 + 0.5) / nr as f64;
                    let dist = mth::lerp(d.sqrt(), 0.0, h) * 2.0f32.sqrt() as f64;
                    let xt = -dist * crate::entity::jmath::sin(y_rad);
                    let zt = dist * crate::entity::jmath::cos(y_rad);
                    if xt.abs() > h || zt.abs() > h {
                        continue;
                    }
                    // (RandomPos floors the offset, then toward_direction floors offset + pos)
                    *cols.entry((mth::floor(mth::floor(xt) as f64 + p.x), mth::floor(mth::floor(zt) as f64 + p.z))).or_insert(0.0) += 1.0 / (na * nr) as f64;
                }
            }
        }
        None => {
            let hi = h as i32;
            let n = ((2 * hi + 1) * (2 * hi + 1)) as f64;
            for x in -hi..=hi {
                for z in -hi..=hi {
                    *cols.entry((mth::floor(x as f64 + p.x), mth::floor(z as f64 + p.z))).or_insert(0.0) += 1.0 / n;
                }
            }
        }
    }
    let ny = (2 * v + 1) as f64;
    let mut cells: Vec<(BlockPos, f64)> = Vec::new();
    for ((cx, cz), wgt) in cols {
        for yt in -v..=v {
            let q = BlockPos::new(cx, mth::floor(yt as f64 + p.y), cz);
            if outside_build_height(w, q.y) || !stable_for(w, i, q) {
                continue;
            }
            let r = if land { move_up_out_of_solid(w, i, q) } else { (!has_malus(w, i, q)).then_some(q) };
            if let Some(r) = r {
                match cells.iter_mut().find(|(c, _)| *c == r) {
                    Some((_, s)) => *s += wgt / ny,
                    None => cells.push((r, wgt / ny)),
                }
            }
        }
    }
    let valid: f64 = cells.iter().map(|(_, s)| s).sum();
    let any = 1.0 - (1.0 - valid).powi(draws);
    cells.into_iter().map(|(c, s)| (c, if valid > 0.0 { s / valid * any } else { 0.0 })).collect()
}

/// (tickexact 2026-11-11, the flee static check) [`pick_shares`] for PANIC's flee from `avoid`
/// (SetWalkTargetAwayFrom: ten calls of `LandRandomPos.getPosAway(mob, 0, h, v)`, ten draws each).
pub fn flee_pick_shares(w: &World, i: usize, h: f64, v: i32, avoid: Vec3) -> Vec<(BlockPos, f64)> {
    let p = w.entities.list[i].pos;
    pick_shares(w, i, h, v, Some((p.x - avoid.x, p.z - avoid.z)), true, 100)
}

/// `LandRandomPos.movePosUpOutOfSolid`: up out of solid blocks, then not water, no malus.
pub fn move_up_out_of_solid(w: &World, i: usize, p: BlockPos) -> Option<BlockPos> {
    let mut q = p;
    if crate::behavior::is_solid(&w.get(q)) {
        q = q.above();
        while q.y <= w.level_max_y() && crate::behavior::is_solid(&w.get(q)) {
            q = q.above();
        }
    }
    let water = crate::fluid::fluid_state(w, q).fluid.is_water();
    (!water && !has_malus(w, i, q)).then_some(q)
}

/// `BlockState.isSolidRender()`: it can occlude and its occlusion shape is the full block.
pub fn is_solid_render(b: &Block) -> bool {
    match *b {
        Block::Terrain(id) => crate::terrain::blocks::props(id).solid_render,
        Block::Simple(s) => !matches!(
            s,
            Simple::Glass | Simple::TintedGlass | Simple::SlimeBlock | Simple::HoneyBlock | Simple::WaxedCopperGrate | Simple::WhiteStainedGlass | Simple::BlackStainedGlass
        ),
        Block::Glazed { .. }
        | Block::RedstoneBlock
        | Block::Observer { .. }
        | Block::Lamp { .. }
        | Block::Bulb { .. }
        | Block::Note { .. }
        | Block::Target { .. }
        | Block::Dispenser { .. }
        | Block::Crafter { .. }
        | Block::Log { .. }
        | Block::Falling { .. }
        | Block::ChiseledBookshelf { .. }
        | Block::Jukebox { .. }
        | Block::Furnace { .. }
        | Block::Tnt { .. }
        | Block::RespawnAnchor { .. }
        // (G1's N38: a jack o'lantern and a froglight are full opaque cubes, solid-render in the game)
        | Block::JackOLantern { .. }
        | Block::Froglight { .. } => true,
        Block::Slab { kind } | Block::StoneSlab { kind } => kind == 2,
        // (a double slab of any material is a full cube too; a waterlogged slab is never double)
        Block::AnySlab { kind, .. } => kind == 2,
        Block::Piston { extended, .. } => !extended,
        _ => false,
    }
}

#[allow(dead_code)]
fn _unused(_t: PathType) {}

#[cfg(test)]
mod solid_render_tests {
    use super::*;

    /// G1's N38: a jack o'lantern, a froglight and a double slab of any material are full opaque cubes,
    /// solid-render in the game; glass and a bottom slab aren't. A candidate cell on a froglight floor is a
    /// stable destination (it never was before).
    #[test]
    fn full_opaque_cubes_render_solid() {
        for s in ["minecraft:jack_o_lantern[facing=north]", "minecraft:ochre_froglight[axis=y]", "minecraft:prismarine_slab[type=double,waterlogged=false]"] {
            assert!(is_solid_render(&Block::parse(s).unwrap()), "{s} renders solid");
        }
        for s in ["minecraft:glass", "minecraft:prismarine_slab[type=bottom,waterlogged=false]"] {
            assert!(!is_solid_render(&Block::parse(s).unwrap()), "{s} doesn't");
        }
        let mut w = World::new(BlockPos::new(-2, -2, -2), [5, 5, 5]);
        w.set_block(BlockPos::new(0, -1, 0), Block::parse("minecraft:verdant_froglight[axis=y]").unwrap(), crate::world::flags::ALL);
        assert!(is_stable_destination(&w, BlockPos::new(0, 0, 0)), "a candidate cell on a froglight floor is a stable destination");
        assert!(!is_stable_destination(&w, BlockPos::new(1, 0, 0)), "and one on air isn't");
    }
}
