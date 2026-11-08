//! The creeper's goals (`Creeper.registerGoals`) and its fuse (`Creeper.tick`): it floats, swells
//! near its target (3 blocks, in sight), avoids cats and ocelots (none modeled: never), walks at
//! its target, strolls, looks at players and around; it targets players it sees and what hurt it.

use super::look::{LookAtPlayer, LookType, RandomLookAround};
use super::melee::{MeleeAttack, MeleeKind};
use super::stroll::RandomStroll;
use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
use super::Goal;
use crate::entity::mob::ai::target;
use crate::entity::mob::ai::{MobAi, ai, ai_mut, with_nav};
use crate::entity::mob::mob;
use crate::world::World;

/// `Creeper.registerGoals()`.
pub fn register(ai: &mut MobAi) {
    ai.goals.add(1, Goal::Float);
    ai.goals.add(2, Goal::Swell);
    ai.goals.add(3, Goal::AvoidEntity);
    ai.goals.add(3, Goal::AvoidEntity);
    ai.goals.add(4, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Melee, 1.0, false)));
    ai.goals.add(5, Goal::RandomStroll(RandomStroll::water_avoiding(0.8)));
    ai.goals.add(6, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    ai.goals.add(6, Goal::RandomLookAround(RandomLookAround::default()));
    ai.targets.add(1, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Player, 10, true, false)));
    ai.targets.add(2, Goal::HurtByTarget(HurtByTarget::new(false)));
}

/// `SwellGoal.canUse()`: swelling already, or a living target within 3.
pub fn swell_can_use(w: &mut World, i: usize) -> bool {
    if ai(w, i).swell_dir > 0 {
        return true;
    }
    let Some(t) = target::get_target(w, i).and_then(|id| target::who(w, id)) else { return false };
    target::is_alive(w, t) && target::pos_of(w, t).distance_to_sqr(w.entities.list[i].pos) < 9.0
}

/// `SwellGoal.start()`: the navigation stops; the target kept.
pub fn swell_start(w: &mut World, i: usize) {
    with_nav(w, i, |_, _, nav, _| nav.stop());
    let t = target::get_target(w, i);
    ai_mut(w, i).swell_target = t;
}

/// `SwellGoal.tick()`: swell while the target is within 7 and in sight, else shrink.
pub fn swell_tick(w: &mut World, i: usize) {
    let dir = match ai(w, i).swell_target.and_then(|id| target::who(w, id)) {
        Some(t) if target::is_alive(w, t) => {
            if target::pos_of(w, t).distance_to_sqr(w.entities.list[i].pos) > 49.0 || !target::has_line_of_sight(w, i, t) {
                -1
            } else {
                1
            }
        }
        _ => -1,
    };
    ai_mut(w, i).swell_dir = dir;
}

/// `Creeper.tick()` before `super.tick()`: the fuse (`swell += swellDir`, 0..30); at 30 it
/// explodes (refused: a creeper's explosion is not modeled).
pub fn creeper_tick(w: &mut World, i: usize) {
    if mob(w, i).dead || mob(w, i).health <= 0.0 {
        return;
    }
    let a = ai_mut(w, i);
    a.swell += a.swell_dir;
    if a.swell < 0 {
        a.swell = 0;
    }
    if a.swell >= 30 {
        a.swell = 30;
        let why = "a creeper's explosion is not modeled".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
}
