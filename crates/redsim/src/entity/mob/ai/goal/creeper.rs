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
    ai.goals.add(3, Goal::AvoidEntity(super::avoid::AvoidEntity::new(super::avoid::AvoidClass::Ocelot, 6.0, 1.0, 1.2)));
    ai.goals.add(3, Goal::AvoidEntity(super::avoid::AvoidEntity::new(super::avoid::AvoidClass::Cat, 6.0, 1.0, 1.2)));
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

/// `Creeper.tick()` before `super.tick()`: an ignited creeper swells on its own (`swellDir` 1);
/// the fuse (`swell += swellDir`, 0..maxSwell); at maxSwell it explodes ([`explode_creeper`]).
/// Returns whether it exploded (it is gone: `super.tick()` then runs on a removed creeper, which
/// changes nothing).
pub fn creeper_tick(w: &mut World, i: usize) -> bool {
    if mob(w, i).dead || mob(w, i).health <= 0.0 {
        return false;
    }
    let a = ai_mut(w, i);
    if a.creeper_ignited {
        a.swell_dir = 1;
    }
    // (swellDir > 0 at swell 0: the primed sound and the PRIME_FUSE game event)
    let primed = a.swell_dir > 0 && a.swell == 0;
    a.swell += a.swell_dir;
    if a.swell < 0 {
        a.swell = 0;
    }
    if std::env::var_os("MCTAS_TRACE_SWELL").is_some() && (a.swell > 0 || a.swell_dir > 0) {
        let (s, d) = (a.swell, a.swell_dir);
        eprintln!("swell t{} #{} swell {s} dir {d}", w.time, w.entities.list[i].id);
    }
    if primed {
        let p = w.entities.list[i].pos;
        w.game_event_at("prime_fuse", [p.x, p.y, p.z], None, false);
    }
    let a = ai_mut(w, i);
    if a.swell >= a.max_swell {
        a.swell = a.max_swell;
        explode_creeper(w, i);
        return true;
    }
    false
}

/// (mob-AI 2026-11-11) `Creeper.explodeCreeper()`: `ServerLevel.explode(this, x, y, z,
/// explosionRadius x (2 when powered), MOB)` (mobGriefing: DESTROY_WITH_DECAY, the default
/// `mob_explosion_drop_decay`; else KEEP), its lingering cloud (none without effects), then
/// `discard()` (gone: no death, no loot).
fn explode_creeper(w: &mut World, i: usize) {
    let id = w.entities.list[i].id;
    let p = w.entities.list[i].pos;
    let radius = ai(w, i).explosion_radius as f32 * if ai(w, i).creeper_powered { 2.0f32 } else { 1.0f32 };
    let interaction = if w.mob.ai.mob_griefing { crate::explosion::Interaction::DestroyWithDecay } else { crate::explosion::Interaction::Keep };
    crate::explosion::explode_with(w, Some(id), Some(id), p, radius, interaction);
    // spawnLingeringCloud: an AreaEffectCloud of its active effects
    if let Some(j) = crate::entity::index_of(w, id)
        && let crate::entity::EntityKind::Mob(m) = &w.entities.list[j].kind
        && m.effects != Default::default()
    {
        w.mob.ai.legacy.insert("a creeper's lingering cloud of its effects");
    }
    if let Some(j) = crate::entity::index_of(w, id) {
        w.entities.list[j].removed = true;
    }
}
