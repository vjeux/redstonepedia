//! The drowned's goals (`Drowned.addBehaviourGoals`, after `Zombie.registerGoals`' turtle eggs and
//! looks): going to water by day, the trident (none modeled: refused when it would start), the
//! attack (`DrownedAttackGoal`: the zombie's, only at a target it may hunt), the beach and the swim
//! up at night (refused when they would start: named in the run's legacy list), the plain stroll;
//! its targets. Its navigation is amphibious (`AmphibiousPathNavigation`).

use super::look::{LookAtPlayer, LookType, RandomLookAround};
use super::melee::{MeleeAttack, MeleeKind};
use super::stroll::RandomStroll;
use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
use super::zombie::TurtleEgg;
use super::{Goal, JUMP, LOOK, MOVE, reduced_tick_delay};
use crate::entity::Vec3;
use crate::entity::mob::ai::{MobAi, ai, with_nav};
use crate::entity::mob::mob;
use crate::pos::BlockPos;
use crate::world::World;

/// The sandbox level's sea level (a flat level's: -63).
pub const SEA_LEVEL: i32 = -63;

#[derive(Clone, PartialEq, Debug)]
pub enum DrownedGoal {
    /// `DrownedGoToWaterGoal`: the water cell it heads for.
    GoToWater { wanted: Vec3 },
    /// `DrownedGoToBeachGoal` (`MoveToBlockGoal(8, 2)`): its `nextStartTick`.
    GoToBeach { next_start_tick: i32 },
    /// `DrownedSwimUpGoal`.
    SwimUp,
    /// `DrownedTridentAttackGoal` (`RangedAttackGoal`).
    TridentAttack,
}

impl DrownedGoal {
    pub fn flags(&self) -> u8 {
        match self {
            DrownedGoal::GoToWater { .. } => MOVE,
            DrownedGoal::GoToBeach { .. } => MOVE | JUMP,
            DrownedGoal::SwimUp => 0,
            DrownedGoal::TridentAttack => MOVE | LOOK,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            DrownedGoal::GoToWater { .. } => "DrownedGoToWaterGoal",
            DrownedGoal::GoToBeach { .. } => "DrownedGoToBeachGoal",
            DrownedGoal::SwimUp => "DrownedSwimUpGoal",
            DrownedGoal::TridentAttack => "DrownedTridentAttackGoal",
        }
    }

    /// `requiresUpdateEveryTick()`: `MoveToBlockGoal`'s and `RangedAttackGoal`'s.
    pub fn every_tick(&self) -> bool {
        matches!(self, DrownedGoal::GoToBeach { .. } | DrownedGoal::TridentAttack)
    }
}

pub fn is_drowned_id(id: &str) -> bool {
    matches!(id, "drowned" | "baby_drowned")
}

/// `Zombie.registerGoals()` (turtle eggs, the looks) then `Drowned.addBehaviourGoals()`.
pub fn register(ai: &mut MobAi) {
    ai.goals.add(4, Goal::ZombieAttackTurtleEgg(TurtleEgg::default()));
    ai.goals.add(8, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    ai.goals.add(8, Goal::RandomLookAround(RandomLookAround::default()));
    ai.goals.add(1, Goal::Drowned(DrownedGoal::GoToWater { wanted: Vec3::ZERO }));
    ai.goals.add(2, Goal::Drowned(DrownedGoal::TridentAttack));
    ai.goals.add(2, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Drowned, 1.0, false)));
    ai.goals.add(5, Goal::Drowned(DrownedGoal::GoToBeach { next_start_tick: 0 }));
    ai.goals.add(6, Goal::Drowned(DrownedGoal::SwimUp));
    ai.goals.add(7, Goal::RandomStroll(RandomStroll::default_interval(1.0)));
    ai.targets.add(1, Goal::HurtByTarget(HurtByTarget::new(true)));
    ai.targets.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::DrownedPlayer, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Villager, 10, false, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::IronGolem, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Axolotl, 10, true, false)));
    ai.targets.add(5, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::BabyTurtle, 10, true, false)));
}

/// `Drowned.okTarget(target)`: by night any, by day only one in water.
pub fn ok_target(w: &World, target_in_water: bool) -> bool {
    !super::zombie::is_bright_outside(w) || target_in_water
}

/// `Drowned.wantsToSwim()`: searching for land (only the swim up, refused) or a target in water.
pub fn wants_to_swim(w: &World, i: usize) -> bool {
    super::super::target::get_target(w, i).and_then(|id| crate::entity::index_of(w, id)).is_some_and(|ti| w.entities.list[ti].was_touching_water)
}

fn legacy(w: &mut World, what: &'static str) {
    w.mob.ai.legacy.insert(what);
}

pub fn can_use(w: &mut World, i: usize, g: &mut DrownedGoal) -> bool {
    match g {
        // canUse: by day, out of water, a water cell among 10 tries around (nextInt(20) - 10,
        // 2 - nextInt(8), nextInt(20) - 10)
        DrownedGoal::GoToWater { wanted } => {
            if !super::zombie::is_bright_outside(w) || w.entities.list[i].was_touching_water {
                return false;
            }
            let p = w.entities.list[i].block_position();
            for _ in 0..10 {
                let r = w.entities.list[i].random.as_mut().expect("random");
                let (dx, dy, dz) = (r.next_int(20) - 10, 2 - r.next_int(8), r.next_int(20) - 10);
                let q = BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
                if is_water_block(w, q) {
                    *wanted = Vec3::new(q.x as f64 + 0.5, q.y as f64, q.z as f64 + 0.5);
                    return true;
                }
            }
            false
        }
        // MoveToBlockGoal.canUse (its countdown, then nextStartTick's draw and the search), then by
        // night, in water, at most 3 under the sea level: going to a beach is not modeled
        DrownedGoal::GoToBeach { next_start_tick } => {
            if *next_start_tick > 0 {
                *next_start_tick -= 1;
                return false;
            }
            let n = w.entities.list[i].random.as_mut().expect("random").next_int(200);
            *next_start_tick = reduced_tick_delay(200 + n);
            if !super::zombie::is_bright_outside(w) && w.entities.list[i].was_touching_water && w.entities.list[i].pos.y >= (SEA_LEVEL - 3) as f64 {
                legacy(w, "a drowned looking for a beach at night (DrownedGoToBeachGoal)");
            }
            false
        }
        // by night, in water, more than 2 under the sea level: swimming up is not modeled
        DrownedGoal::SwimUp => {
            if !super::zombie::is_bright_outside(w) && w.entities.list[i].was_touching_water && w.entities.list[i].pos.y < (SEA_LEVEL - 2) as f64 {
                legacy(w, "a drowned swimming up at night (DrownedSwimUpGoal)");
            }
            false
        }
        // RangedAttackGoal.canUse (a live target) and a trident in its hand: none here
        DrownedGoal::TridentAttack => {
            if ai(w, i).trident && super::super::target::get_target(w, i).is_some() {
                legacy(w, "a drowned's trident (DrownedTridentAttackGoal: it attacks in melee instead)");
            }
            false
        }
    }
}

pub fn can_continue(w: &mut World, i: usize, g: &mut DrownedGoal) -> bool {
    match g {
        // canContinueToUse: the navigation still going
        DrownedGoal::GoToWater { .. } => !crate::entity::mob::ai::nav_of(w, i).is_done(),
        _ => false,
    }
}

pub fn start(w: &mut World, i: usize, g: &mut DrownedGoal) {
    if let DrownedGoal::GoToWater { wanted } = g {
        let p = *wanted;
        with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, p.x, p.y, p.z, 1, 1.0, arena));
    }
}

/// `BlockState.is(Blocks.WATER)`: the water block (a source or flowing), not a waterlogged one.
fn is_water_block(w: &World, p: BlockPos) -> bool {
    match w.get(p) {
        crate::block::Block::Water { .. } => true,
        crate::block::Block::Terrain(id) => crate::terrain::blocks::props(id).block_id() == "minecraft:water",
        _ => false,
    }
}

/// `DrownedMoveControl.tick()` before `MoveControl.tick()`: swimming (it wants to and is in
/// water) is not modeled; else off the ground it sinks 0.008 more.
pub fn move_control_before(w: &mut World, i: usize) {
    if wants_to_swim(w, i) && w.entities.list[i].was_touching_water {
        legacy(w, "a drowned swimming after a target (DrownedMoveControl)");
    }
    if !w.entities.list[i].on_ground {
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.add(0.0, -0.008, 0.0);
    }
}

/// Whether mob `i` is a drowned (its AI's own parts).
pub fn is_drowned(w: &World, i: usize) -> bool {
    is_drowned_id(mob(w, i).ty.def().id)
}
