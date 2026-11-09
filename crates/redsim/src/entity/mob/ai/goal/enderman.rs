//! The enderman (`Enderman`): its goals (`FloatGoal`, `EndermanFreezeWhenLookedAt` (a player
//! target only), `MeleeAttackGoal`, `WaterAvoidingRandomStrollGoal` with probability 0 (its
//! `nextFloat` still drawn; every cell worth 0: `getWalkTargetValue`), `LookAtPlayerGoal` 8,
//! `RandomLookAroundGoal`, `EndermanLeaveBlockGoal`, `EndermanTakeBlockGoal`) and targets
//! (`EndermanLookForPlayerGoal`: a creative player never angers it, no draws; `HurtByTargetGoal`;
//! endermites; `ResetUniversalAngerTargetGoal`), its `customServerAiStep` (in daylight, seeing the
//! sky in light above 0.5, the random teleport), water and rain's drown damage at the end of its
//! `aiStep`, and `hurtServer`'s teleport (nine times in ten, a damage no living entity caused).
//! A teleport is `LivingEntity.randomTeleport`: down from the drawn cell to a block in
//! `#entities_can_teleport_to`, not one of `#enderman_does_not_teleport_to`, the box free of
//! blocks, liquids and those blocks, not above water; then `snapTo` and the navigation stopped.
//! Taking and leaving blocks follow the goals' own draws and tests (`#enderman_holdable`, the
//! OUTLINE clip, `canPlaceBlock`); a carried block keeps the enderman (requiresCustomPersistence).

use super::{Goal, JUMP, MOVE};
use crate::block::Block;
use crate::entity::Vec3;
use crate::entity::mob::{mob, mob_mut};
use crate::pos::BlockPos;
use crate::world::World;

/// The enderman's own state (`DATA_CARRY_STATE`, `targetChangeTime`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct EndermanState {
    pub carried: Option<Block>,
    pub target_change_time: i32,
}

#[derive(Clone, PartialEq, Debug)]
pub enum EndermanGoal {
    Freeze,
    LeaveBlock,
    TakeBlock,
    LookForPlayer,
}

impl EndermanGoal {
    pub fn flags(&self) -> u8 {
        match self {
            EndermanGoal::Freeze => JUMP | MOVE,
            EndermanGoal::LookForPlayer => super::TARGET,
            _ => 0,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            EndermanGoal::Freeze => "EndermanFreezeWhenLookedAt",
            EndermanGoal::LeaveBlock => "EndermanLeaveBlockGoal",
            EndermanGoal::TakeBlock => "EndermanTakeBlockGoal",
            EndermanGoal::LookForPlayer => "EndermanLookForPlayerGoal",
        }
    }
}

pub fn is_enderman(w: &World, i: usize) -> bool {
    mob(w, i).ty.def().id == "enderman"
}

/// `Enderman.registerGoals`.
pub fn register(a: &mut crate::entity::mob::ai::MobAi) {
    use super::look::{LookAtPlayer, LookType, RandomLookAround};
    use super::melee::{MeleeAttack, MeleeKind};
    use super::stroll::RandomStroll;
    use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
    a.goals.add(0, Goal::Float);
    a.goals.add(1, Goal::Enderman(EndermanGoal::Freeze));
    a.goals.add(2, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Melee, 1.0, false)));
    a.goals.add(7, Goal::RandomStroll(RandomStroll { water_avoiding: Some(0.0), ..RandomStroll::water_avoiding(1.0) }));
    a.goals.add(8, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    a.goals.add(8, Goal::RandomLookAround(RandomLookAround::default()));
    a.goals.add(10, Goal::Enderman(EndermanGoal::LeaveBlock));
    a.goals.add(11, Goal::Enderman(EndermanGoal::TakeBlock));
    a.targets.add(1, Goal::Enderman(EndermanGoal::LookForPlayer));
    a.targets.add(2, Goal::HurtByTarget(HurtByTarget::new(false)));
    a.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Endermite, 10, true, false)));
    a.targets.add(4, Goal::ResetUniversalAnger);
    a.enderman = Some(EndermanState::default());
}

fn state(w: &World, i: usize) -> &EndermanState {
    crate::entity::mob::ai::ai(w, i).enderman.as_ref().expect("an enderman")
}

fn state_mut(w: &mut World, i: usize) -> &mut EndermanState {
    crate::entity::mob::ai::ai_mut(w, i).enderman.as_mut().expect("an enderman")
}

fn rnd(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("a mob's own random")
}

fn refuse(w: &mut World, why: String) {
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

pub fn can_use(w: &mut World, i: usize, g: &mut EndermanGoal) -> bool {
    match g {
        // a player target only: a creative player is never one
        EndermanGoal::Freeze | EndermanGoal::LookForPlayer => false,
        EndermanGoal::LeaveBlock => state(w, i).carried.is_some() && w.mob.ai.mob_griefing && rnd(w, i).next_int(super::reduced_tick_delay(2000)) == 0,
        EndermanGoal::TakeBlock => state(w, i).carried.is_none() && w.mob.ai.mob_griefing && rnd(w, i).next_int(super::reduced_tick_delay(20)) == 0,
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut EndermanGoal) {
    match g {
        EndermanGoal::TakeBlock => take_block(w, i),
        EndermanGoal::LeaveBlock => leave_block(w, i),
        _ => {}
    }
}

/// The natural-state row of a cell's state (a design block by its game string).
fn props_at(w: &World, p: BlockPos) -> Option<&'static crate::terrain::blocks::Props> {
    use crate::terrain::blocks;
    match w.get(p) {
        Block::Terrain(id) => Some(blocks::props(id)),
        Block::Air => blocks::id_of("minecraft:air").map(blocks::props),
        b => blocks::id_of(&b.to_string()).map(blocks::props),
    }
}

/// `EndermanTakeBlockGoal.tick()`.
fn take_block(w: &mut World, i: usize) {
    let e = &w.entities.list[i];
    let (x, y, z) = (e.pos.x, e.pos.y, e.pos.z);
    let (bx, bz) = (e.block_position().x, e.block_position().z);
    let r = rnd(w, i);
    let xt = crate::entity::mth::floor(x - 2.0 + r.next_double() * 4.0);
    let yt = crate::entity::mth::floor(y + r.next_double() * 3.0);
    let zt = crate::entity::mth::floor(z - 2.0 + r.next_double() * 4.0);
    let pos = BlockPos::new(xt, yt, zt);
    let from = Vec3::new(bx as f64 + 0.5, yt as f64 + 0.5, bz as f64 + 0.5);
    let to = Vec3::new(xt as f64 + 0.5, yt as f64 + 0.5, zt as f64 + 0.5);
    // the clip (OUTLINE, no fluids): a miss reports the end cell
    let reachable = crate::entity::mob::los::first_hit_outline(w, from, to).unwrap_or(pos) == pos;
    let holdable = props_at(w, pos).is_some_and(|p| p.has_tag("minecraft:enderman_holdable"));
    if holdable && reachable {
        let carried = match w.get(pos) {
            Block::Terrain(id) => {
                let name = crate::terrain::blocks::props(id).block_id().to_string();
                crate::terrain::land::block_of(&name).ok()
            }
            b => Block::parse(b.to_string().split('[').next().unwrap_or("")).ok(),
        };
        let Some(carried) = carried else {
            refuse(w, format!("an enderman taking {} (its default state) is not modeled", w.get(pos)));
            return;
        };
        // removeBlock(pos, false): the cell's fluid's legacy block, else air (flags 3)
        if !crate::fluid::fluid_state(w, pos).is_empty() {
            refuse(w, "an enderman taking a block out of a fluid is not modeled".into());
            return;
        }
        w.set_block(pos, Block::Air, crate::world::flags::ALL);
        state_mut(w, i).carried = Some(carried);
    }
}

/// `EndermanLeaveBlockGoal.tick()`.
fn leave_block(w: &mut World, i: usize) {
    let e = &w.entities.list[i];
    let (x, y, z) = (e.pos.x, e.pos.y, e.pos.z);
    let r = rnd(w, i);
    let xt = crate::entity::mth::floor(x - 1.0 + r.next_double() * 2.0);
    let yt = crate::entity::mth::floor(y + r.next_double() * 2.0);
    let zt = crate::entity::mth::floor(z - 1.0 + r.next_double() * 2.0);
    let pos = BlockPos::new(xt, yt, zt);
    let below = pos.below();
    let Some(carried) = state(w, i).carried else { return };
    let target_air = w.get(pos).is_air();
    let below_ok = !w.get(below).is_air() && props_at(w, below).is_some_and(|p| p.block_id() != "minecraft:bedrock" && p.full_collision);
    if !(target_air && below_ok) {
        return;
    }
    // Block.updateFromNeighbourShapes and canSurvive: a full block survives anywhere and keeps its
    // state but a snowy one's snowy flag, read from the cell above
    let name = carried.to_string();
    let base = name.split('[').next().unwrap_or("").to_string();
    let full = props_at_name(&name).is_some_and(|p| p.full_collision && p.solid_render);
    if !full {
        refuse(w, format!("an enderman placing {base} (its canSurvive and shape update) is not modeled"));
        return;
    }
    let placed = if name.contains("snowy=") {
        let above = w.get(pos.above()).to_string();
        let snowy = above.starts_with("minecraft:snow[") || above == "minecraft:snow_block" || above.starts_with("minecraft:snow ");
        let s = name.replace("snowy=true", "snowy=false").replace("snowy=false", if snowy { "snowy=true" } else { "snowy=false" });
        crate::terrain::land::block_of(&s).unwrap_or(carried)
    } else {
        carried
    };
    // no entity but this one in the cell (`getEntities(enderman, unitCube)`)
    let me = w.entities.list[i].id;
    let cube = crate::entity::Aabb::of_block(pos);
    if !w.get_entities(cube, |e| e.id != me && !e.removed).is_empty() {
        return;
    }
    w.set_block(pos, placed, crate::world::flags::ALL);
    state_mut(w, i).carried = None;
}

fn props_at_name(name: &str) -> Option<&'static crate::terrain::blocks::Props> {
    crate::terrain::blocks::id_of(name).map(crate::terrain::blocks::props)
}

/// `Enderman.customServerAiStep` (after the navigation's tick): in daylight 600 ticks after its
/// last target change, light above 0.5 under the open sky, `nextFloat() * 30 < (br - 0.4) * 2`
/// drops its target and teleports.
pub fn custom_server_ai_step(w: &mut World, i: usize) {
    let tick_count = w.entities.list[i].tick_count;
    if !super::zombie::is_bright_outside(w) || tick_count < state(w, i).target_change_time + 600 {
        return;
    }
    let e = &w.entities.list[i];
    let eye = crate::entity::mob::mob(w, i).eye() as f64;
    let cell = BlockPos::new(crate::entity::mth::floor(e.pos.x), crate::entity::mth::floor(e.pos.y + eye), crate::entity::mth::floor(e.pos.z));
    let feet = e.block_position();
    let br = super::stroll::light_magic(w, cell);
    if br > 0.5 && w.can_see_sky(feet).unwrap_or(false) && rnd(w, i).next_float() * 30.0 < (br - 0.4) * 2.0 {
        set_target(w, i, None);
        teleport(w, i);
    }
}

/// `Enderman.setTarget` (target::set_target keeps its target change time).
pub fn set_target(w: &mut World, i: usize, t: Option<u32>) {
    super::super::target::set_target(w, i, t);
}

/// The movement speed's attack boost (`SPEED_MODIFIER_ATTACKING`: +0.15 while it has a target).
pub fn speed_bonus(w: &World, i: usize) -> f64 {
    if super::super::target::get_target(w, i).is_some() { 0.15f32 as f64 } else { 0.0 }
}

/// `Enderman.teleport()`: a cell within 32 (x, z) and 32 (y) drawn from its own random.
pub fn teleport(w: &mut World, i: usize) -> bool {
    if mob(w, i).health <= 0.0 || mob(w, i).dead {
        return false;
    }
    let p = w.entities.list[i].pos;
    let r = rnd(w, i);
    let xx = p.x + (r.next_double() - 0.5) * 64.0;
    let yy = p.y + (r.next_int(64) - 32) as f64;
    let zz = p.z + (r.next_double() - 0.5) * 64.0;
    teleport_to(w, i, xx, yy, zz)
}

/// `Enderman.teleport(x, y, z)` (not a passenger) through `LivingEntity.randomTeleport`.
fn teleport_to(w: &mut World, i: usize, xx: f64, yy: f64, zz: f64) -> bool {
    if w.entities.list[i].vehicle.is_some() {
        return false;
    }
    let mut p = BlockPos::new(crate::entity::mth::floor(xx), crate::entity::mth::floor(yy), crate::entity::mth::floor(zz));
    if !w.contains(p) {
        return false;
    }
    let mut y = yy;
    let min_y = w.min.y;
    while p.y > min_y {
        p = p.below();
        let tags = props_at(w, p);
        if tags.is_some_and(|t| t.has_tag("minecraft:entities_can_teleport_to")) {
            // checkPositionAndTeleport
            if tags.is_some_and(|t| t.has_tag("minecraft:enderman_does_not_teleport_to")) {
                return false;
            }
            let (wd, ht) = {
                let d = mob(w, i).ty.def();
                (d.width as f64, d.height as f64)
            };
            let bb = crate::entity::Aabb::new(xx - wd / 2.0, y, zz - wd / 2.0, xx + wd / 2.0, y + ht, zz + wd / 2.0);
            if !crate::entity::collide::no_collision(w, i, bb) || crate::entity::mob::blocks::box_has_liquid(w, bb) {
                return false;
            }
            // findBlocksIn(box): any of the box's cells in the tag
            let (x0, y0, z0) = (crate::entity::mth::floor(bb.min_x), crate::entity::mth::floor(bb.min_y), crate::entity::mth::floor(bb.min_z));
            let (x1, y1, z1) = (crate::entity::mth::floor(bb.max_x), crate::entity::mth::floor(bb.max_y), crate::entity::mth::floor(bb.max_z));
            for cy in y0..=y1 {
                for cz in z0..=z1 {
                    for cx in x0..=x1 {
                        if props_at(w, BlockPos::new(cx, cy, cz)).is_some_and(|t| t.has_tag("minecraft:enderman_does_not_teleport_to")) {
                            return false;
                        }
                    }
                }
            }
            // canRandomlyTeleportTo: not above water
            let under = BlockPos::new(crate::entity::mth::floor(xx), crate::entity::mth::floor(y), crate::entity::mth::floor(zz)).below();
            if crate::fluid::fluid_state(w, under).fluid.is_water() {
                return false;
            }
            // teleportTo: snapTo (the old positions too), the navigation stopped
            let pos = Vec3::new(xx, y, zz);
            w.entities.set_pos(i, pos);
            let e = &mut w.entities.list[i];
            (e.xo, e.yo, e.zo) = (xx, y, zz);
            (e.x_old, e.y_old, e.z_old) = (xx, y, zz);
            crate::entity::mob::ai::nav_of_mut(w, i).stop();
            return true;
        }
        y -= 1.0;
    }
    false
}

/// `Enderman.hurtServer`'s tail: a damage no living entity caused teleports it nine times in ten
/// (the roll drawn whether it lived or not).
pub fn after_hurt(w: &mut World, i: usize, src: crate::entity::mob::Damage) {
    if src == crate::entity::mob::Damage::MobAttack {
        return;
    }
    if rnd(w, i).next_int(10) != 0 {
        teleport(w, i);
    }
}

/// The end of `LivingEntity.aiStep`: water-sensitive in water or rain, drowned for 1.
pub fn water_tick(w: &mut World, i: usize) {
    if crate::entity::mob::sun::in_water_or_rain(w, i) {
        crate::entity::mob::living::hurt(w, i, crate::entity::mob::Damage::Drown, 1.0);
    }
    let _ = mob_mut;
}
