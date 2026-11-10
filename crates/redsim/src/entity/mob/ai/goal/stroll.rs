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

/// `RandomPos.generateRandomPosTowardDirection` (no home: no pull toward it).
fn toward_direction(w: &World, i: usize, d: BlockPos) -> BlockPos {
    let e = &w.entities.list[i];
    BlockPos::new(mth::floor(d.x as f64 + e.pos.x), mth::floor(d.y as f64 + e.pos.y), mth::floor(d.z as f64 + e.pos.z))
}

/// `Level.isOutsideBuildHeight(y)`.
fn outside_build_height(y: i32) -> bool {
    y < super::super::path::walk::MIN_Y || y > super::super::path::navigation::MAX_Y
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
    best_of_ten_by(
        w,
        i,
        |w| {
            let d = random_direction(w, i, h, v);
            let p = toward_direction(w, i, d);
            if outside_build_height(p.y) || !stable_for(w, i, p) {
                return None;
            }
            move_up_out_of_solid(w, i, p)
        },
        weight,
    )
}

/// `DefaultRandomPos.getPos(mob, h, v)`.
pub fn default_random_pos(w: &mut World, i: usize, h: i32, v: i32) -> Option<Vec3> {
    best_of_ten(w, i, |w| {
        let d = random_direction(w, i, h, v);
        let p = toward_direction(w, i, d);
        (!outside_build_height(p.y) && stable_for(w, i, p) && !has_malus(w, i, p)).then_some(p)
    })
}

/// `LandRandomPos.getPos(mob, h, v)`.
pub fn land_random_pos(w: &mut World, i: usize, h: i32, v: i32) -> Option<Vec3> {
    best_of_ten(w, i, |w| {
        let d = random_direction(w, i, h, v);
        let p = toward_direction(w, i, d);
        if outside_build_height(p.y) || !stable_for(w, i, p) {
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

/// `LandRandomPos.getPosInDirection(mob, minH, maxH, v, dir, restrict)` (no home: no restriction).
pub fn land_pos_in_direction(w: &mut World, i: usize, min_h: f64, max_h: f64, v: i32, dir: Vec3) -> Option<Vec3> {
    best_of_ten(w, i, |w| {
        let d = random_direction_within_radians(w, i, min_h, max_h, v, 0, dir.x, dir.z, (std::f64::consts::PI / 2.0) as f32 as f64)?;
        let p = toward_direction(w, i, d);
        if outside_build_height(p.y) || !stable_for(w, i, p) {
            return None;
        }
        move_up_out_of_solid(w, i, p)
    })
}

/// `DefaultRandomPos.getPos(mob, h, v)` (no home: no restriction): ten random directions, each
/// cell inside the world, stable and without malus, the best by walk-target value.
pub fn default_pos(w: &mut World, i: usize, h: i32, v: i32) -> Option<Vec3> {
    best_of_ten(w, i, |w| {
        let d = random_direction(w, i, h, v);
        let p = toward_direction(w, i, d);
        (!outside_build_height(p.y) && stable_for(w, i, p) && !has_malus(w, i, p)).then_some(p)
    })
}

/// `DefaultRandomPos.getPosTowards(mob, h, v, towardsPos, maxRadians)`.
pub fn default_pos_towards(w: &mut World, i: usize, h: i32, v: i32, towards: Vec3, max_rad: f64) -> Option<Vec3> {
    let p = w.entities.list[i].pos;
    let dir = Vec3::new(towards.x - p.x, towards.y - p.y, towards.z - p.z);
    best_of_ten(w, i, |w| {
        let d = random_direction_within_radians(w, i, 0.0, h as f64, v, 0, dir.x, dir.z, max_rad)?;
        let p = toward_direction(w, i, d);
        (!outside_build_height(p.y) && stable_for(w, i, p) && !has_malus(w, i, p)).then_some(p)
    })
}

/// `LandRandomPos.movePosUpOutOfSolid`: up out of solid blocks, then not water, no malus.
pub fn move_up_out_of_solid(w: &World, i: usize, p: BlockPos) -> Option<BlockPos> {
    let mut q = p;
    if crate::behavior::is_solid(&w.get(q)) {
        q = q.above();
        while q.y <= super::super::path::navigation::MAX_Y && crate::behavior::is_solid(&w.get(q)) {
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
        | Block::RespawnAnchor { .. } => true,
        Block::Slab { kind } | Block::StoneSlab { kind } => kind == 2,
        Block::Piston { extended, .. } => !extended,
        _ => false,
    }
}

#[allow(dead_code)]
fn _unused(_t: PathType) {}
