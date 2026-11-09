//! The skeleton's goals (`AbstractSkeleton.registerGoals`, then `reassessWeaponGoal` adding the
//! bow goal last): it keeps out of the sun (`RestrictSunGoal`: the navigation's avoid-sun; burning,
//! `FleeSunGoal` looks for shade), avoids wolves (none modeled: never), strolls, looks at players
//! and around; it targets what hurt it, players, iron golems and baby turtles (none modeled).
//! Shooting (`RangedBowAttackGoal`'s tick: strafing, drawing the bow, the arrow) is refused.

use super::look::{LookAtPlayer, LookType, RandomLookAround};
use super::stroll::RandomStroll;
use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
use super::Goal;
use crate::entity::Vec3;
use crate::entity::mob::ai::{MobAi, ai, target, with_nav};
use crate::entity::mob::mob;
use crate::pos::BlockPos;
use crate::world::World;

/// `FleeSunGoal`'s wanted position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FleeSun {
    pub wanted: Vec3,
}

/// `AbstractSkeleton.registerGoals()` and the bow goal `reassessWeaponGoal` adds after them (a
/// skeleton spawned or summoned holds its bow: `populateDefaultEquipmentSlots`).
pub fn register(ai: &mut MobAi) {
    ai.goals.add(2, Goal::RestrictSun);
    ai.goals.add(3, Goal::FleeSun(FleeSun::default()));
    ai.goals.add(3, Goal::AvoidEntity);
    ai.goals.add(5, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
    ai.goals.add(6, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    ai.goals.add(6, Goal::RandomLookAround(RandomLookAround::default()));
    ai.goals.add(4, Goal::RangedBow);
    ai.targets.add(1, Goal::HurtByTarget(HurtByTarget::new(false)));
    ai.targets.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Player, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::IronGolem, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::BabyTurtle, 10, true, false)));
}

/// `Level.isBrightOutside()`: a dimension without fixed time and a sky darkening under 4.
pub fn bright_outside(w: &World) -> bool {
    w.sky_darken() < 4
}

/// `RestrictSunGoal.canUse()`: bright outside, nothing on its head (no equipment is modeled: a
/// helmet is refused where it would come from), a ground navigation.
pub fn restrict_sun_can_use(w: &World, i: usize) -> bool {
    // `mob.getItemBySlot(HEAD).isEmpty()`: a helmeted skeleton keeps to its path
    bright_outside(w) && mob(w, i).helmet.is_none()
}

/// `FleeSunGoal.canUse()`: no target, bright outside, on fire, under the open sky, then
/// `getHidePos`: ten random cells around (its own random: x and z in -10..9, y in -3..2) for one
/// that does not see the sky and is darker than the middle (`getWalkTargetValue < 0`).
pub fn flee_sun_can_use(w: &mut World, i: usize, g: &mut FleeSun) -> bool {
    if target::get_target(w, i).is_some() || !bright_outside(w) || mob(w, i).fire_ticks <= 0 {
        return false;
    }
    let pos = w.entities.list[i].block_position();
    if !w.can_see_sky(pos).unwrap_or(false) {
        return false;
    }
    // `mob.getItemBySlot(HEAD).isEmpty() && setWantedPos()`
    if mob(w, i).helmet.is_some() {
        return false;
    }
    for _ in 0..10 {
        let r = w.entities.list[i].random.as_mut().expect("random");
        let (dx, dy, dz) = (r.next_int(20) - 10, r.next_int(6) - 3, r.next_int(20) - 10);
        let p = BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz);
        if !w.can_see_sky(p).unwrap_or(true) && super::stroll::walk_target_value(w, i, p) < 0.0 {
            g.wanted = Vec3::new(p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5);
            return true;
        }
    }
    false
}

/// `FleeSunGoal.start()`: `moveTo(wanted, 1.0)`.
pub fn flee_sun_start(w: &mut World, i: usize, g: &FleeSun) {
    let p = g.wanted;
    with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, p.x, p.y, p.z, 1, 1.0, arena));
}

/// `RangedBowAttackGoal.canUse()`: a target (and the bow in hand).
pub fn bow_can_use(w: &World, i: usize) -> bool {
    target::get_target(w, i).is_some()
}

/// `canContinueToUse()`: still a target, or the navigation not done.
pub fn bow_can_continue(w: &World, i: usize) -> bool {
    bow_can_use(w, i) || !ai(w, i).nav.is_done()
}

/// `tick()`: the strafing, the bow drawn and shot: refused.
pub fn bow_tick(w: &mut World) {
    let why = "a skeleton attacking (RangedBowAttackGoal: strafing, the bow, its arrows) is not modeled".to_string();
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}
