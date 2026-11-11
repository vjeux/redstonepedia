//! The skeleton's goals (`AbstractSkeleton.registerGoals`, then `reassessWeaponGoal` adding the
//! weapon goal last): it keeps out of the sun (`RestrictSunGoal`: the navigation's avoid-sun;
//! burning, `FleeSunGoal` looks for shade), avoids wolves (none modeled: never), strolls, looks at
//! players and around; it targets what hurt it, players, iron golems and baby turtles (none
//! modeled). With a bow in hand it shoots (`RangedBowAttackGoal`: closing in, strafing, the bow
//! drawn 20 ticks, `entity::arrow`); empty-handed it fights in melee (its anonymous
//! `MeleeAttackGoal`, 1.2).

use super::look::{LookAtPlayer, LookType, RandomLookAround};
use super::melee::{MeleeAttack, MeleeKind};
use super::stroll::RandomStroll;
use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
use super::Goal;
use crate::entity::Vec3;
use crate::entity::mob::ai::target::Who;
use crate::entity::mob::ai::{MobAi, target, with_nav};
use crate::entity::mob::mob;
use crate::pos::BlockPos;
use crate::world::World;

/// `FleeSunGoal`'s wanted position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FleeSun {
    pub wanted: Vec3,
}

/// `AbstractSkeleton.registerGoals()`; the weapon goal (`reassessWeaponGoal`) comes last: the
/// constructor's, an empty hand's melee goal, until a bow in hand (`reassess_weapon_goal`: a
/// natural skeleton's `populateDefaultEquipmentSlots`, a summon's `equipment`) makes it the bow's.
pub fn register(ai: &mut MobAi) {
    ai.goals.add(2, Goal::RestrictSun);
    ai.goals.add(3, Goal::FleeSun(FleeSun::default()));
    ai.goals.add(3, Goal::AvoidEntity(super::avoid::AvoidEntity::new(super::avoid::AvoidClass::Wolf, 6.0, 1.0, 1.2)));
    ai.goals.add(5, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
    ai.goals.add(6, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    ai.goals.add(6, Goal::RandomLookAround(RandomLookAround::default()));
    ai.goals.add(4, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Skeleton, 1.2, false)));
    ai.targets.add(1, Goal::HurtByTarget(HurtByTarget::new(false)));
    ai.targets.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Player, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::IronGolem, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::BabyTurtle, 10, true, false)));
}

/// A skeleton-family mob (`AbstractSkeleton` with a bow goal: skeletons, strays, bogged).
pub fn is_skeleton(w: &World, i: usize) -> bool {
    matches!(mob(w, i).ty.def().id, "skeleton" | "stray" | "bogged")
}

/// `isHolding(Items.BOW)` (the main hand: the off hand is never modeled to hold one).
pub fn holding_bow(w: &World, i: usize) -> bool {
    mob(w, i).mainhand.as_ref().is_some_and(|h| h.item == "minecraft:bow")
}

/// `AbstractSkeleton.reassessWeaponGoal()`: the melee and bow goals out (a running one stopped
/// first), then the bow goal at priority 4 when a bow is in hand (its interval: 20 ticks on Hard,
/// else 40; a bogged's 50 and 70), else the melee goal, last in the selector.
pub fn reassess_weapon_goal(w: &mut World, i: usize) {
    if !crate::entity::mob::ai::has_ai(w, i) || !is_skeleton(w, i) {
        return;
    }
    let weapon = |g: &Goal| matches!(g, Goal::RangedBow(_)) || matches!(g, Goal::MeleeAttack(m) if m.kind == MeleeKind::Skeleton);
    loop {
        let Some(k) = crate::entity::mob::ai::ai(w, i).goals.goals.iter().position(|g| weapon(&g.goal)) else { break };
        if crate::entity::mob::ai::ai(w, i).goals.goals[k].running {
            let mut g = crate::entity::mob::ai::ai(w, i).goals.goals[k].goal.clone();
            super::stop(w, i, &mut g);
            let a = crate::entity::mob::ai::ai_mut(w, i);
            a.goals.goals[k].goal = g;
            a.goals.goals[k].running = false;
        }
        let a = crate::entity::mob::ai::ai_mut(w, i);
        a.goals.goals.remove(k);
        // the locked flags point at the goals by index: drop the removed one's, shift the rest
        for l in a.goals.locked.iter_mut() {
            match *l {
                Some(h) if h == k => *l = None,
                Some(h) if h > k => *l = Some(h - 1),
                _ => {}
            }
        }
    }
    let goal = if holding_bow(w, i) {
        let hard = crate::entity::mob::ai::difficulty_id(w) == 3;
        let bogged = mob(w, i).ty.def().id == "bogged";
        let interval = match (bogged, hard) {
            (false, true) => 20,
            (false, false) => 40,
            (true, true) => 50,
            (true, false) => 70,
        };
        Goal::RangedBow(RangedBow::new(interval))
    } else {
        Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Skeleton, 1.2, false))
    };
    crate::entity::mob::ai::ai_mut(w, i).goals.add(4, goal);
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

/// `RangedBowAttackGoal`'s fields (`new RangedBowAttackGoal<>(this, 1.0, 20, 15.0F)`, its
/// interval set by `reassessWeaponGoal`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangedBow {
    pub speed_modifier: f64,
    pub attack_interval_min: i32,
    pub attack_radius_sqr: f32,
    pub attack_time: i32,
    pub see_time: i32,
    pub strafing_clockwise: bool,
    pub strafing_backwards: bool,
    pub strafing_time: i32,
}

impl RangedBow {
    pub fn new(interval: i32) -> RangedBow {
        RangedBow { speed_modifier: 1.0, attack_interval_min: interval, attack_radius_sqr: 15.0f32 * 15.0f32, attack_time: -1, see_time: 0, strafing_clockwise: false, strafing_backwards: false, strafing_time: -1 }
    }
}

/// `RangedBowAttackGoal.canUse()`: a target and the bow in hand.
pub fn bow_can_use(w: &World, i: usize) -> bool {
    target::get_target(w, i).is_some() && holding_bow(w, i)
}

/// `canContinueToUse()`: (still a target, or the navigation not done) and the bow in hand.
pub fn bow_can_continue(w: &World, i: usize) -> bool {
    (bow_can_use(w, i) || !crate::entity::mob::ai::nav_of(w, i).is_done()) && holding_bow(w, i)
}

/// `start()`: aggressive.
pub fn bow_start(w: &mut World, i: usize) {
    crate::entity::mob::ai::ai_mut(w, i).aggressive = true;
}

/// `stop()`: not aggressive, the sight time and the attack time reset, the bow released.
pub fn bow_stop(w: &mut World, i: usize, g: &mut RangedBow) {
    crate::entity::mob::ai::ai_mut(w, i).aggressive = false;
    g.see_time = 0;
    g.attack_time = -1;
    stop_using_item(w, i);
}

/// `LivingEntity.startUsingItem(hand)`: a bow in hand (`getUseDuration` 72000), the
/// `ITEM_INTERACT_START` game event at the mob.
pub fn start_using_item(w: &mut World, i: usize) {
    if mob(w, i).mainhand.is_none() || crate::entity::mob::ai::ai(w, i).using_item {
        return;
    }
    let a = crate::entity::mob::ai::ai_mut(w, i);
    a.using_item = true;
    a.use_item_remaining = 72000;
    let p = w.entities.list[i].pos;
    w.game_event_at("item_interact_start", [p.x, p.y, p.z], None, false);
}

/// `LivingEntity.stopUsingItem()`: the `ITEM_INTERACT_FINISH` game event when it was using one.
pub fn stop_using_item(w: &mut World, i: usize) {
    if !crate::entity::mob::ai::has_ai(w, i) {
        return;
    }
    let was = crate::entity::mob::ai::ai(w, i).using_item;
    let a = crate::entity::mob::ai::ai_mut(w, i);
    a.using_item = false;
    a.use_item_remaining = 0;
    if was {
        let p = w.entities.list[i].pos;
        w.game_event_at("item_interact_finish", [p.x, p.y, p.z], None, false);
    }
}

/// `LivingEntity.updatingUsingItem()` (`tick()` after `baseTick`): the item still in hand ticks
/// (`onUseTick`: a spear's `KineticWeapon.damageEntities`) and counts down (72000 never run out
/// here), else the use stops.
pub fn updating_using_item(w: &mut World, i: usize) {
    if !crate::entity::mob::ai::has_ai(w, i) || !crate::entity::mob::ai::ai(w, i).using_item {
        return;
    }
    let spear = super::spear::held(w, i).is_some();
    // (ItemStack.isSameItem: the bow, the spear or the trident it started with, still in hand)
    if !holding_bow(w, i) && !spear && !super::drowned::holds_trident(w, i) {
        stop_using_item(w, i);
        return;
    }
    if spear {
        let remaining = crate::entity::mob::ai::ai(w, i).use_item_remaining;
        super::spear::damage_entities(w, i, remaining);
        if w.entities.list[i].removed || !crate::entity::mob::ai::ai(w, i).using_item {
            return;
        }
    }
    let a = crate::entity::mob::ai::ai_mut(w, i);
    a.use_item_remaining -= 1;
    if a.use_item_remaining == 0 {
        let why = "an item used for 72000 ticks (completeUsingItem) is not modeled".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
}

/// `getTicksUsingItem()`.
fn ticks_using_item(w: &World, i: usize) -> i32 {
    let a = crate::entity::mob::ai::ai(w, i);
    if a.using_item { 72000 - a.use_item_remaining } else { 0 }
}

/// `BowItem.getPowerForTime(ticks)`.
fn power_for_time(ticks: i32) -> f32 {
    let mut pow = ticks as f32 / 20.0f32;
    pow = (pow * pow + pow * 2.0f32) / 3.0f32;
    if pow > 1.0 {
        pow = 1.0;
    }
    pow
}

/// `RangedBowAttackGoal.tick()`: the sight time, closing in (`moveTo(target, 1.0)`) or, in range
/// (15) and seen for 20 ticks, strafing (a 0.3 chance each 20 ticks to turn and to back off; back
/// off under a quarter of the range, closer over three quarters; `Mob.lookAt` the target, and the
/// vehicle too), the bow drawn and shot after 20 ticks with the target in sight (a pull of 20 or
/// more: power 1), then the interval.
pub fn bow_tick(w: &mut World, i: usize, g: &mut RangedBow) {
    let Some(tid) = target::get_target(w, i) else { return };
    let Some(t) = target::who(w, tid) else { return };
    let tp = target::pos_of(w, t);
    let target_dist_sqr = w.entities.list[i].pos.distance_to_sqr(tp);
    let has_los = target::has_line_of_sight(w, i, t);
    let had_los = g.see_time > 0;
    if has_los != had_los {
        g.see_time = 0;
    }
    if has_los {
        g.see_time += 1;
    } else {
        g.see_time -= 1;
    }
    if !(target_dist_sqr > g.attack_radius_sqr as f64) && g.see_time >= 20 {
        with_nav(w, i, |_, _, nav, _| nav.stop());
        g.strafing_time += 1;
    } else {
        let tb = crate::entity::mob::ai::goal::melee::target_block(w, t);
        let s = g.speed_modifier;
        // PathNavigation.moveTo(entity, speed): reach 1 (a wall climber's own rule when it rides one)
        with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity_default(w, nm, tb, s, arena));
        g.strafing_time = -1;
    }
    if g.strafing_time >= 20 {
        let r = w.entities.list[i].random.as_mut().expect("random");
        if (r.next_float() as f64) < 0.3 {
            g.strafing_clockwise = !g.strafing_clockwise;
        }
        let r = w.entities.list[i].random.as_mut().expect("random");
        if (r.next_float() as f64) < 0.3 {
            g.strafing_backwards = !g.strafing_backwards;
        }
        g.strafing_time = 0;
    }
    let Who::Entity(j) = t else {
        let why = "a skeleton shooting at a player is not modeled".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
        return;
    };
    if g.strafing_time > -1 {
        if target_dist_sqr > (g.attack_radius_sqr * 0.75f32) as f64 {
            g.strafing_backwards = false;
        } else if target_dist_sqr < (g.attack_radius_sqr * 0.25f32) as f64 {
            g.strafing_backwards = true;
        }
        let (f, r) = (if g.strafing_backwards { -0.5f32 } else { 0.5f32 }, if g.strafing_clockwise { 0.5f32 } else { -0.5f32 });
        let owner = crate::entity::mob::ai::nav_owner(w, i);
        crate::entity::mob::ai::ai_mut(w, owner).move_ctl.strafe(f, r);
        if let Some(v) = crate::entity::mob::ai::controlled_vehicle(w, i) {
            crate::entity::mob::ai::mob_look_at(w, v, j, 30.0, 30.0);
        }
        crate::entity::mob::ai::mob_look_at(w, i, j, 30.0, 30.0);
    } else {
        let te = target::eye_of(w, t);
        crate::entity::mob::ai::ai_mut(w, i).look.set_look_at(te.x, te.y, te.z, 30.0, 30.0);
    }
    if crate::entity::mob::ai::ai(w, i).using_item {
        if !has_los && g.see_time < -60 {
            stop_using_item(w, i);
        } else if has_los {
            let pull = ticks_using_item(w, i);
            if pull >= 20 {
                stop_using_item(w, i);
                let effect = match mob(w, i).ty.def().id {
                    "stray" => Some("a stray's slowness arrow (its effect on the hit)"),
                    "bogged" => Some("a bogged's poison arrow (its effect on the hit)"),
                    _ => None,
                };
                let enchanted = mob(w, i).mainhand.as_ref().is_some_and(|h| h.enchanted);
                crate::entity::arrow::shoot_from_mob(w, i, j, power_for_time(pull), effect, enchanted);
                g.attack_time = g.attack_interval_min;
            }
        }
    } else {
        g.attack_time -= 1;
        if g.attack_time <= 0 && g.see_time >= -60 {
            start_using_item(w, i);
        }
    }
}
