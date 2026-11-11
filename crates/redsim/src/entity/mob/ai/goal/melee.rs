//! `MeleeAttackGoal` and its subclasses (`ZombieAttackGoal`: the raised arms; `Spider` and the
//! iron golem use the plain goal).

use super::super::target::{self, Who};
use super::super::{ai_mut, with_nav};
use crate::world::World;

/// Which class of melee goal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MeleeKind {
    Melee,
    Zombie,
    Spider,
    /// `DrownedAttackGoal`: the zombie's, while the drowned may hunt its target (`okTarget`).
    Drowned,
    /// `AbstractSkeleton.meleeGoal` (an empty-handed skeleton's): an anonymous subclass, whose
    /// simple class name is empty.
    Skeleton,
}

impl MeleeKind {
    pub fn name(self) -> &'static str {
        match self {
            MeleeKind::Melee => "MeleeAttackGoal",
            MeleeKind::Zombie => "ZombieAttackGoal",
            MeleeKind::Spider => "SpiderAttackGoal",
            MeleeKind::Drowned => "DrownedAttackGoal",
            MeleeKind::Skeleton => "",
        }
    }
}

/// `MeleeAttackGoal`'s fields.
#[derive(Clone, PartialEq, Debug)]
pub struct MeleeAttack {
    pub kind: MeleeKind,
    pub speed_modifier: f64,
    pub following_even_if_not_seen: bool,
    pub path: Option<super::super::path::Path>,
    pub pathed_target: (f64, f64, f64),
    pub ticks_until_next_path_recalculation: i32,
    pub ticks_until_next_attack: i32,
    pub last_can_use_check: i64,
    /// `ZombieAttackGoal.raiseArmTicks`.
    pub raise_arm_ticks: i32,
}

impl MeleeAttack {
    pub fn new(kind: MeleeKind, speed: f64, following: bool) -> MeleeAttack {
        MeleeAttack { kind, speed_modifier: speed, following_even_if_not_seen: following, path: None, pathed_target: (0.0, 0.0, 0.0), ticks_until_next_path_recalculation: 0, ticks_until_next_attack: 0, last_can_use_check: 0, raise_arm_ticks: 0 }
    }
}

fn target_who(w: &World, i: usize) -> Option<Who> {
    target::get_target(w, i).and_then(|id| target::who(w, id))
}

/// `canUse()`: at most once per 20 ticks, a target alive and a path to it (or already in reach).
pub fn can_use(w: &mut World, i: usize, g: &mut MeleeAttack) -> bool {
    let time = w.time;
    if time - g.last_can_use_check < 20 {
        return false;
    }
    g.last_can_use_check = time;
    let Some(t) = target_who(w, i) else { return false };
    if !target::is_alive(w, t) {
        return false;
    }
    let tb = target_block(w, t);
    g.path = with_nav(w, i, |w, nm, nav, arena| nav.create_path_pos(w, nm, tb, 0, arena));
    let ok = if g.path.is_some() { true } else { within_melee_range(w, i, t) };
    // DrownedAttackGoal: and a target it may hunt
    if ok && g.kind == MeleeKind::Drowned && !drowned_ok(w, t) {
        return false;
    }
    // SpiderAttackGoal: and no passenger
    ok && !(g.kind == MeleeKind::Spider && !w.entities.list[i].passengers.is_empty())
}

/// `Drowned.okTarget(target)` of target `t`.
fn drowned_ok(w: &World, t: Who) -> bool {
    let in_water = match t {
        Who::Entity(j) => w.entities.list[j].was_touching_water,
        Who::Foreign(f) => w.foreign[f].in_water,
        Who::Player(_) => {
            let p = target::pos_of(w, t);
            let c = crate::pos::BlockPos::new(crate::entity::mth::floor(p.x), crate::entity::mth::floor(p.y), crate::entity::mth::floor(p.z));
            crate::fluid::fluid_state(w, c).fluid.is_water()
        }
    };
    super::drowned::ok_target(w, in_water)
}

/// `target.blockPosition()`.
pub fn target_block(w: &World, t: Who) -> crate::pos::BlockPos {
    let p = target::pos_of(w, t);
    crate::pos::BlockPos::new(crate::entity::mth::floor(p.x), crate::entity::mth::floor(p.y), crate::entity::mth::floor(p.z))
}

/// `canContinueToUse()` (SpiderAttackGoal: in the light a 1 in 100 roll gives up first).
pub fn can_continue(w: &mut World, i: usize, g: &mut MeleeAttack) -> bool {
    if g.kind == MeleeKind::Spider && super::spider::attack_gives_up(w, i) {
        return false;
    }
    let Some(t) = target_who(w, i) else { return false };
    if !target::is_alive(w, t) {
        return false;
    }
    // DrownedAttackGoal.canContinueToUse: super's and okTarget
    let drowned_ok = g.kind != MeleeKind::Drowned || drowned_ok(w, t);
    if !g.following_even_if_not_seen {
        return !crate::entity::mob::ai::nav_of(w, i).is_done() && drowned_ok;
    }
    // isWithinHome (no home here); a spectator or creative player is no target already
    drowned_ok
}

/// `start()`.
pub fn start(w: &mut World, i: usize, g: &mut MeleeAttack) {
    let (p, s) = (g.path.clone(), g.speed_modifier);
    with_nav(w, i, |w, nm, nav, _| nav.move_to_path(w, nm, p, s));
    ai_mut(w, i).aggressive = true;
    g.ticks_until_next_path_recalculation = 0;
    g.ticks_until_next_attack = 0;
    if matches!(g.kind, MeleeKind::Zombie | MeleeKind::Drowned) {
        g.raise_arm_ticks = 0;
    }
}

/// `stop()`: a target that is not a creative or spectator player is dropped.
pub fn stop(w: &mut World, i: usize, _g: &mut MeleeAttack) {
    // EntitySelector.NO_CREATIVE_OR_SPECTATOR.test(target): a null target fails it too
    let keep = target::get_target(w, i).and_then(|id| target::who(w, id)).is_some_and(|t| match t {
        Who::Player(k) => w.mob.ai.players[k].mode == super::super::GameMode::Survival,
        Who::Entity(_) | Who::Foreign(_) => true,
    });
    if !keep {
        target::set_target(w, i, None);
    }
    ai_mut(w, i).aggressive = false;
    with_nav(w, i, |_, _, nav, _| nav.stop());
}

/// `tick()` (with `ZombieAttackGoal.tick`'s arms after it).
pub fn tick(w: &mut World, i: usize, g: &mut MeleeAttack) {
    if let Some(t) = target_who(w, i) {
        let te = target::eye_of(w, t);
        ai_mut(w, i).look.set_look_at(te.x, te.y, te.z, 30.0, 30.0);
        g.ticks_until_next_path_recalculation = (g.ticks_until_next_path_recalculation - 1).max(0);
        let tp = target::pos_of(w, t);
        let seen = g.following_even_if_not_seen || target::has_line_of_sight(w, i, t);
        if seen && g.ticks_until_next_path_recalculation <= 0 {
            let (px, py, pz) = g.pathed_target;
            let moved = (px == 0.0 && py == 0.0 && pz == 0.0) || tp.distance_to_sqr(crate::entity::Vec3::new(px, py, pz)) >= 1.0;
            let go = moved || w.entities.list[i].random.as_mut().expect("random").next_float() < 0.05;
            if go {
                g.pathed_target = (tp.x, tp.y, tp.z);
                let n = w.entities.list[i].random.as_mut().expect("random").next_int(7);
                g.ticks_until_next_path_recalculation = 4 + n;
                let d = w.entities.list[i].pos.distance_to_sqr(tp);
                if d > 1024.0 {
                    g.ticks_until_next_path_recalculation += 10;
                } else if d > 256.0 {
                    g.ticks_until_next_path_recalculation += 5;
                }
                let tb = target_block(w, t);
                let s = g.speed_modifier;
                let ok = with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity(w, nm, tb, 0, s, arena));
                if !ok {
                    g.ticks_until_next_path_recalculation += 15;
                }
                // adjustedTickDelay: the goal is ticked every tick
            }
        }
        g.ticks_until_next_attack = (g.ticks_until_next_attack - 1).max(0);
        check_and_perform_attack(w, i, g, t);
    }
    if matches!(g.kind, MeleeKind::Zombie | MeleeKind::Drowned) {
        g.raise_arm_ticks += 1;
        ai_mut(w, i).aggressive = g.raise_arm_ticks >= 5 && g.ticks_until_next_attack < 20 / 2;
    }
}

/// `checkAndPerformAttack(target)`.
fn check_and_perform_attack(w: &mut World, i: usize, g: &mut MeleeAttack, t: Who) {
    if g.ticks_until_next_attack <= 0 && within_melee_range(w, i, t) && target::has_line_of_sight(w, i, t) {
        g.ticks_until_next_attack = 20;
        super::super::attack::swing(w, i);
        super::super::attack::do_hurt_target(w, i, t);
    }
}

/// `Mob.isWithinMeleeAttackRange(target)`: the attack box (with the vehicle's), inflated by the
/// default reach `sqrt(2.04) - 0.6`, meets the target's box.
pub fn within_melee_range(w: &World, i: usize, t: Who) -> bool {
    let reach = 2.04f64.sqrt() - 0.6;
    let e = &w.entities.list[i];
    let own = e.bb;
    let bb = match e.vehicle.and_then(|v| crate::entity::index_of(w, v)) {
        Some(vi) => {
            let m = w.entities.list[vi].bb;
            crate::entity::Aabb::new(own.min_x.min(m.min_x), own.min_y, own.min_z.min(m.min_z), own.max_x.max(m.max_x), own.max_y, own.max_z.max(m.max_z))
        }
        None => own,
    };
    bb.inflate(reach, 0.0, reach).intersects(target::box_of(w, t))
}
