//! Attacks and their effects on the victim (26.3 `Mob.doHurtTarget`, `LivingEntity.hurtServer`'s
//! attacker part: `resolveMobResponsibleForDamage`, `dealDefaultKnockback` / `knockback`, the hurt
//! and death sounds' pitch draws, `Mob.playHurtSound`'s ambient reset).
//!
//! `living::hurt` keeps the damage itself (cooldown, armor, death); [`after_hurt`] is its hook for
//! what an AI world adds, and [`do_hurt_target`] hands it the attacker through
//! `AiWorld::attacker`.

use super::target::{self, Who};
use super::{ai, ai_mut, has_ai};
use crate::entity::mob::{Damage, living, mob};
use crate::entity::{Vec3, jmath};
use crate::world::World;

/// `Attributes.ATTACK_DAMAGE` of the modeled attackers.
pub fn attack_damage(w: &World, i: usize) -> f32 {
    match mob(w, i).ty.def().id {
        "zombie" | "baby_zombie" | "zombie_villager" | "baby_zombie_villager" | "husk" | "drowned" | "baby_drowned" => 3.0,
        "iron_golem" => 15.0,
        "spider" | "cave_spider" => 2.0,
        "skeleton" | "stray" | "bogged" => 2.0,
        "pillager" | "vindicator" => 5.0,
        _ => 2.0,
    }
}

/// `swingForAttack(MAIN_HAND)`: the swing animation (client-side only; nothing the server reads).
pub fn swing(_w: &mut World, _i: usize) {}

/// `Mob.doHurtTarget(level, target)` for the modeled melee attackers.
pub fn do_hurt_target(w: &mut World, i: usize, t: Who) -> bool {
    let j = match t {
        Who::Entity(j) => j,
        Who::Player(_) => {
            let why = "a mob hitting a player is not modeled".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
            return false;
        }
    };
    if !matches!(w.entities.list[j].kind, crate::entity::EntityKind::Mob(_)) {
        return false;
    }
    let golem = mob(w, i).ty == crate::entity::mob::types::MobType::IronGolem;
    let dmg = if golem {
        // IronGolem.doHurtTarget: half the damage plus nextInt(damage) from its own random
        let a = attack_damage(w, i);
        let r = w.entities.list[i].random.as_mut().expect("random").next_int(a as i32);
        a / 2.0 + r as f32
    } else {
        attack_damage(w, i)
    };
    let attacker = w.entities.list[i].id;
    let prev = w.mob.ai.attacker.replace(attacker);
    let hurt = living::hurt(w, j, Damage::MobAttack, dmg);
    w.mob.ai.attacker = prev;
    if hurt && golem {
        // the toss: +0.4 up, scaled by the target's knockback resistance
        let resist = if mob(w, j).ty == crate::entity::mob::types::MobType::IronGolem { 1.0 } else { 0.0 };
        let scale = (1.0f64 - resist).max(0.0);
        let e = &mut w.entities.list[j];
        e.delta_movement = e.delta_movement.add(0.0, (0.4f32 as f64) * scale, 0.0);
    }
    if hurt {
        // causeExtraKnockback: no ATTACK_KNOCKBACK on the modeled melee mobs; setLastHurtMob
        if has_ai(w, i) {
            ai_mut(w, i).last_hurt_mob = Some(w.entities.list[j].id);
        }
    }
    // Zombie.doHurtTarget: a burning zombie with an empty hand may set the target on fire
    if hurt && mob(w, i).fire_ticks > 0 && matches!(mob(w, i).ty.def().id, "zombie" | "baby_zombie" | "zombie_villager" | "husk" | "drowned") {
        let why = "a burning zombie's hit (the fire it passes on, regional difficulty) is not modeled".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
    hurt
}

/// The pitch draws of a sound the mob makes (`getVoicePitch`: two `nextFloat`).
fn voice_pitch_draws(w: &mut World, i: usize) {
    if let Some(r) = w.entities.list[i].random.as_mut() {
        r.next_float();
        r.next_float();
    }
}

/// `hurtServer`'s steps after the damage, for a mob of an AI world (`living::hurt` calls it with
/// whether the hit took full damage): the attacker as `lastHurtByMob`, the knockback away from it,
/// the death or hurt sound's pitch draws (`Mob.playHurtSound` resets the ambient sound time).
/// The killer's `killedEntity(level, victim, source)` (the mob AI on): a zombie that kills a
/// villager on Hard, or on Normal when its own random's `nextBoolean()` is false, converts it
/// into a zombie villager: refused (zombie villagers are not modeled).
pub fn killed_entity(w: &mut World, victim: usize, src: Damage) {
    if src != Damage::MobAttack {
        return;
    }
    let Some(att) = w.mob.ai.attacker.and_then(|id| crate::entity::index_of(w, id)) else { return };
    if !matches!(mob(w, att).ty.def().id, "zombie" | "baby_zombie") || mob(w, victim).ty != crate::entity::mob::types::MobType::Villager {
        return;
    }
    let d = super::difficulty_id(w);
    if d < 2 {
        return;
    }
    if d == 2 && w.entities.list[att].random.as_mut().expect("random").next(1) != 0 {
        return;
    }
    let why = "a zombie converting the villager it killed into a zombie villager is not modeled".to_string();
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

pub fn after_hurt(w: &mut World, i: usize, src: Damage, took_full: bool) {
    if !w.mob.ai.on {
        return;
    }
    let attacker = if src == Damage::MobAttack { w.mob.ai.attacker } else { None };
    if has_ai(w, i) {
        ai_mut(w, i).last_damage_entity = attacker;
    }
    if let Some(a) = attacker
        && has_ai(w, i)
    {
        let tc = w.entities.list[i].tick_count;
        let s = ai_mut(w, i);
        s.last_hurt_by_mob = Some(a);
        s.last_hurt_by_mob_timestamp = tc;
    }
    if took_full
        && let Some(a) = attacker
        && let Some(ai_idx) = crate::entity::index_of(w, a)
    {
        let src_pos = w.entities.list[ai_idx].pos;
        let e = &w.entities.list[i];
        knockback(w, i, 0.4f32 as f64, src_pos.x - e.pos.x, src_pos.z - e.pos.z);
    }
    if mob(w, i).health <= 0.0 {
        if took_full {
            voice_pitch_draws(w, i);
        }
    } else if took_full {
        // Mob.playHurtSound: resetAmbientSoundTime (-getAmbientSoundInterval())
        if has_ai(w, i) {
            ai_mut(w, i).ambient_sound_time = -super::ambient_interval(w, i);
        }
        voice_pitch_draws(w, i);
    }
}

/// `LivingEntity.knockback(power, xd, zd)` (no knockback resistance on the modeled victims but the
/// iron golem's 1.0).
pub fn knockback(w: &mut World, i: usize, power: f64, mut xd: f64, mut zd: f64) {
    let resist = if mob(w, i).ty.def().id == "iron_golem" { 1.0 } else { 0.0 };
    let power = power * (1.0 - resist);
    if power <= 0.0 {
        return;
    }
    // a victim at the attacker's very position: a random direction from its own random
    while xd * xd + zd * zd < 1.0e-5f32 as f64 {
        let r = w.entities.list[i].random.as_mut().expect("random");
        let (a, b) = (r.next_double(), r.next_double());
        xd = (a - b) * 0.01;
        let (a, b) = (r.next_double(), r.next_double());
        zd = (a - b) * 0.01;
    }
    let e = &mut w.entities.list[i];
    let d = e.delta_movement;
    let len = (xd * xd + zd * zd).sqrt();
    let (nx, nz) = if len < 1.0e-5f32 as f64 { (0.0, 0.0) } else { (xd / len * power, zd / len * power) };
    let y = if e.on_ground { (d.y / 2.0 + power).min(0.4) } else { d.y };
    e.delta_movement = Vec3::new(d.x / 2.0 - nx, y, d.z / 2.0 - nz);
    let _ = jmath::sin(0.0);
}

/// `LivingEntity.baseTick`'s attacker memory: a dead attacker, or one older than 100 ticks, is
/// forgotten (`setLastHurtByMob(null)` stamps the time too).
pub fn forget_attacker(w: &mut World, i: usize) {
    let Some(a) = ai(w, i).last_hurt_by_mob else { return };
    let alive = target::who(w, a).is_some_and(|t| target::is_alive(w, t));
    let tc = w.entities.list[i].tick_count;
    if !alive || tc - ai(w, i).last_hurt_by_mob_timestamp > 100 {
        let s = ai_mut(w, i);
        s.last_hurt_by_mob = None;
        s.last_hurt_by_mob_timestamp = tc;
    }
}
