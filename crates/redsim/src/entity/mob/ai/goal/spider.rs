//! The spider's goals (`Spider.registerGoals`): it floats, avoids armadillos (none modeled:
//! never), leaps at its target and bites it (`SpiderAttackGoal`: MeleeAttackGoal(1.0, true), which
//! gives up in the light with a 1 in 100 roll), strolls, looks at players and around; it targets
//! what hurt it, and players and iron golems only in the dark (`SpiderTargetGoal`: brightness under
//! 0.5). Its navigation climbs (`WallClimberNavigation`: it walks straight at a target it has no
//! path to) and so does its body (`Spider.tick`: climbing while it collides horizontally).

use super::look::{LookAtPlayer, LookType, RandomLookAround};
use super::melee::{MeleeAttack, MeleeKind};
use super::stroll::RandomStroll;
use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
use super::Goal;
use crate::entity::mob::ai::{MobAi, target};
use crate::entity::mob::{mob, mob_mut};
use crate::world::World;

/// `Spider.registerGoals()`.
pub fn register(ai: &mut MobAi) {
    ai.goals.add(1, Goal::Float);
    ai.goals.add(2, Goal::AvoidEntity);
    ai.goals.add(3, Goal::LeapAtTarget);
    ai.goals.add(4, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Spider, 1.0, true)));
    ai.goals.add(5, Goal::RandomStroll(RandomStroll::water_avoiding(0.8)));
    ai.goals.add(6, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    ai.goals.add(6, Goal::RandomLookAround(RandomLookAround::default()));
    ai.targets.add(1, Goal::HurtByTarget(HurtByTarget::new(false)));
    ai.targets.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::SpiderPlayer, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::SpiderIronGolem, 10, true, false)));
}

/// `getLightLevelDependentMagicValue()` at the mob's eye.
pub fn eye_magic(w: &mut World, i: usize) -> f32 {
    let e = &w.entities.list[i];
    let eye = crate::pos::BlockPos::new(e.pos.x.floor() as i32, (e.pos.y + mob(w, i).eye() as f64).floor() as i32, e.pos.z.floor() as i32);
    super::stroll::light_magic(w, eye)
}

/// `LeapAtTargetGoal.canUse()` (0.4 up): no controlling passenger, a target 2 to 4 away, on the
/// ground, then `nextInt(reducedTickDelay(5)) == 0` on its own random.
pub fn leap_can_use(w: &mut World, i: usize) -> bool {
    // hasControllingPassenger
    if crate::entity::mob::ai::controlled_by_mob(w, i) {
        return false;
    }
    let Some(t) = target::get_target(w, i).and_then(|id| target::who(w, id)) else { return false };
    let d = target::pos_of(w, t).distance_to_sqr(w.entities.list[i].pos);
    if !(4.0..=16.0).contains(&d) || !w.entities.list[i].on_ground {
        return false;
    }
    let bound = super::reduced_tick_delay(5);
    w.entities.list[i].random.as_mut().expect("random").next_int(bound) == 0
}

/// `LeapAtTargetGoal.start()`: toward the target 0.4 (with a fifth of its motion), 0.4 up.
pub fn leap_start(w: &mut World, i: usize) {
    let Some(t) = target::get_target(w, i).and_then(|id| target::who(w, id)) else { return };
    let tp = target::pos_of(w, t);
    let e = &w.entities.list[i];
    let delta = e.delta_movement;
    let mut v = crate::entity::Vec3::new(tp.x - e.pos.x, 0.0, tp.z - e.pos.z);
    if v.length_sqr() > 1.0e-7 {
        v = v.normalize().scale(0.4).add_v(delta.scale(0.2));
    }
    w.entities.list[i].delta_movement = crate::entity::Vec3::new(v.x, 0.4f32 as f64, v.z);
}

/// `LeapAtTargetGoal.canContinueToUse()`: in the air.
pub fn leap_can_continue(w: &World, i: usize) -> bool {
    !w.entities.list[i].on_ground
}

/// `SpiderAttackGoal.canContinueToUse()`'s light check, before MeleeAttackGoal's: in the light
/// (0.5 and up) a 1 in 100 roll drops the target. True when it gave up.
pub fn attack_gives_up(w: &mut World, i: usize) -> bool {
    let br = eye_magic(w, i);
    if br >= 0.5f32 && w.entities.list[i].random.as_mut().expect("random").next_int(100) == 0 {
        target::set_target(w, i, None);
        return true;
    }
    false
}

/// `Spider.tick()` after `super.tick()`: climbing while it collides horizontally (read by
/// `onClimbable` next tick).
pub fn after_tick(w: &mut World, i: usize) {
    let hc = w.entities.list[i].horizontal_collision;
    mob_mut(w, i).climbing = hc;
}
