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
    let base = base_attack_damage(w, i);
    // the main hand's ATTACK_DAMAGE modifier (a natural zombie's iron sword +5, a zombified
    // piglin's golden sword +3; a picked-up item's)
    let d = match &mob(w, i).mainhand {
        Some(h) => base as f64 + super::loot::weapon_damage(&h.item),
        None => base as f64,
    };
    // (tickexact 2026-11-10) strength's ADD_VALUE modifier, 3 x (amplifier + 1): a spider's spawn
    // effect
    let d = if mob(w, i).effects.strength > 0 { d + 3.0 } else { d };
    d as f32
}

/// `getAttributeBaseValue(ATTACK_DAMAGE)` of the modeled attackers (a spear's stab adds to it).
pub fn base_attack_damage(w: &World, i: usize) -> f32 {
    base_attack_damage_by_id(mob(w, i).ty.def().id)
}

/// (mob-AI 2026-11-10) `EnchantmentHelper.modifyDamage(weapon, victim, source, dmg)`: the
/// weapon's `damage` effects in its order (each `add` a `LevelBasedValue.linear` float): sharpness
/// 1 + 0.5 a level past the first; smite 2.5 a level on #sensitive_to_smite (#undead); bane of
/// arthropods 2.5 a level on #arthropod; power 1 + 0.5 a level when an arrow hits (`arrow`).
pub fn enchant_damage(ench: &[(&'static str, i32)], victim: &str, arrow: bool, mut dmg: f32) -> f32 {
    let victim = victim.strip_prefix("baby_").unwrap_or(victim);
    let undead = matches!(victim, "skeleton" | "stray" | "wither_skeleton" | "skeleton_horse" | "bogged" | "parched" | "zombie_horse" | "camel_husk" | "zombie" | "zombie_villager" | "zombified_piglin" | "zoglin" | "drowned" | "husk" | "zombie_nautilus" | "wither" | "phantom");
    let arthropod = matches!(victim, "bee" | "endermite" | "silverfish" | "spider" | "cave_spider");
    for &(e, l) in ench {
        let above = (l - 1) as f32;
        match e {
            "sharpness" => dmg += 1.0f32 + 0.5f32 * above,
            "smite" if undead => dmg += 2.5f32 + 2.5f32 * above,
            "bane_of_arthropods" if arthropod => dmg += 2.5f32 + 2.5f32 * above,
            "power" if arrow => dmg += 1.0f32 + 0.5f32 * above,
            _ => {}
        }
    }
    dmg
}

/// The ids of a weapon's enchantments whose effects on a hit are not modeled (bane of
/// arthropods' slowness on an arthropod, impaling).
fn unmodeled_hit_enchantment(ench: &[(&'static str, i32)], victim: &str) -> Option<&'static str> {
    let victim = victim.strip_prefix("baby_").unwrap_or(victim);
    ench.iter().find_map(|&(e, _)| match e {
        "bane_of_arthropods" if matches!(victim, "bee" | "endermite" | "silverfish" | "spider" | "cave_spider") => Some("a bane of arthropods hit's slowness"),
        "impaling" => Some("an impaling weapon's hit"),
        _ => None,
    })
}

/// [`base_attack_damage`] by mob type id.
pub fn base_attack_damage_by_id(id: &str) -> f32 {
    match id {
        "zombie" | "baby_zombie" | "zombie_villager" | "baby_zombie_villager" | "husk" | "drowned" | "baby_drowned" => 3.0,
        // ZombifiedPiglin.createAttributes
        "zombified_piglin" | "baby_zombified_piglin" => 5.0,
        "iron_golem" => 15.0,
        "spider" | "cave_spider" => 2.0,
        "skeleton" | "stray" | "bogged" => 2.0,
        "pillager" | "vindicator" => 5.0,
        // Wolf.createAttributes
        "wolf" => 4.0,
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
        // (an entity of another level is never in reach: never in line of sight)
        Who::Foreign(_) => {
            let why = "a mob hitting an entity of another dimension is not modeled".to_string();
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
    // (mob-AI 2026-11-10) the held weapon's enchantments (Mob.doHurtTarget's
    // EnchantmentHelper.modifyDamage, getKnockback, doPostAttackEffects)
    let ench: Vec<(&'static str, i32)> = if golem { Vec::new() } else { mob(w, i).mainhand.as_ref().map(|h| h.ench.clone()).unwrap_or_default() };
    let victim_id = mob(w, j).ty.def().id;
    let dmg = if golem {
        // IronGolem.doHurtTarget: half the damage plus nextInt(damage) from its own random
        let a = attack_damage(w, i);
        let r = w.entities.list[i].random.as_mut().expect("random").next_int(a as i32);
        a / 2.0 + r as f32
    } else {
        enchant_damage(&ench, victim_id, false, attack_damage(w, i))
    };
    if let Some(why) = unmodeled_hit_enchantment(&ench, victim_id) {
        let why = format!("{why} is not modeled");
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
    let attacker = w.entities.list[i].id;
    // (mob-AI 2026-11-10) EnchantmentHelper.doPostAttackEffects: thorns armor's post_attack (a
    // level-random chance, then damage to the attacker) is not modeled
    if mob(w, j).armor.has(mob(w, j).helmet.is_some(), "thorns") {
        let why = "a hit on thorns armor (its post-attack effect) is not modeled".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
    if std::env::var("MCTAS_TRACE_HURT").ok().and_then(|v| v.parse::<u32>().ok()) == Some(w.entities.list[j].id) {
        eprintln!("hurt t{}: #{} ({:?}, hand {:?}) hits #{} ({:?}) for {dmg}", w.time, attacker, mob(w, i).ty, mob(w, i).mainhand, w.entities.list[j].id, mob(w, j).ty);
    }
    let prev = w.mob.ai.attacker.replace(attacker);
    let hurt = living::hurt(w, j, Damage::MobAttack, dmg);
    w.mob.ai.attacker = prev;
    if hurt && golem {
        // the toss: +0.4 up, scaled by the target's knockback resistance
        let scale = (1.0f64 - knockback_resistance(w, j)).max(0.0);
        let e = &mut w.entities.list[j];
        e.delta_movement = e.delta_movement.add(0.0, (0.4f32 as f64) * scale, 0.0);
    }
    if hurt {
        // causeExtraKnockback(getKnockback: (ATTACK_KNOCKBACK + the knockback enchantment) / 2):
        // the target knocked along the attacker's yaw, the attacker's motion x 0.6 sideways
        let kb = ench.iter().find(|&&(e, _)| e == "knockback").map_or(0.0f32, |&(_, l)| l as f32) / 2.0f32;
        if kb > 0.0 {
            let yaw = w.entities.list[i].y_rot * (std::f64::consts::PI / 180.0) as f32;
            knockback(w, j, kb as f64, crate::entity::mth::sin(yaw as f64) as f64, -(crate::entity::mth::cos(yaw as f64) as f64));
            let e = &mut w.entities.list[i];
            e.delta_movement = e.delta_movement.multiply(0.6, 1.0, 0.6);
        }
        // doPostAttackEffects: fire aspect's ignite (igniteForSeconds(4 a level))
        let fire = ench.iter().find(|&&(e, _)| e == "fire_aspect").map_or(0, |&(_, l)| l);
        if fire > 0 && !mob(w, j).ty.def().fire_immune {
            let t = crate::entity::mob::ignite_ticks(w, j, ((4.0f32 + 4.0f32 * (fire - 1) as f32) * 20.0f32).floor() as i32);
            let m = crate::entity::mob::mob_mut(w, j);
            if m.fire_ticks < t {
                m.fire_ticks = t;
            }
        }
        // setLastHurtMob
        if has_ai(w, i) {
            ai_mut(w, i).last_hurt_mob = Some(w.entities.list[j].id);
        }
    }
    // Zombie.doHurtTarget: a burning zombie with an empty hand may set the target on fire (with
    // something in its hand: no roll)
    if hurt && mob(w, i).fire_ticks > 0 && mob(w, i).mainhand.is_none() && matches!(mob(w, i).ty.def().id, "zombie" | "baby_zombie" | "zombie_villager" | "husk" | "drowned") {
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
/// `Zombie.killedEntity` (every Zombie class: a drowned's too, whatever it killed it with: its
/// hit, its spear, its trident): a villager it killed on Normal (a coin from its own random) or
/// Hard becomes a zombie villager (`convertVillagerToZombieVillager`); then it answers false (no
/// ENTITY_DIE game event, no death loot). Returns whether the victim's death goes on.
pub fn killed_entity(w: &mut World, victim: usize, src: Damage) -> bool {
    if !src.by_attacker() {
        return true;
    }
    let Some(att) = w.mob.ai.attacker.and_then(|id| crate::entity::index_of(w, id)) else { return true };
    if !crate::entity::mob::natural::is_zombie_class(mob(w, att).ty) || mob(w, victim).ty != crate::entity::mob::types::MobType::Villager {
        return true;
    }
    let d = super::difficulty_id(w);
    if d < 2 {
        return true;
    }
    if d == 2 && w.entities.list[att].random.as_mut().expect("random").next(1) != 0 {
        return true;
    }
    crate::entity::mob::natural::villager_to_zombie_villager(w, victim).is_none()
}

pub fn after_hurt(w: &mut World, i: usize, src: Damage, took_full: bool) {
    if !w.mob.ai.on {
        return;
    }
    let attacker = if src.by_attacker() { w.mob.ai.attacker } else { None };
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
        && src.by_projectile()
        && let Some(m) = w.mob.ai.projectile_motion
    {
        // Projectile.calculateHorizontalHurtKnockbackDirection: along the projectile's motion
        knockback(w, i, 0.4f32 as f64, -m.x, -m.z);
    } else if took_full
        // a spear's stab: #no_knockback (stabAttack pushes on its own)
        && src != Damage::Spear
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

/// `getAttributeValue(KNOCKBACK_RESISTANCE)`: the type's (the iron golem's 1.0) plus a zombie's
/// spawn bonus, clamped to the attribute's 0..1.
pub fn knockback_resistance(w: &World, i: usize) -> f64 {
    let m = mob(w, i);
    (m.ty.def().knockback_resistance + m.knockback_bonus).clamp(0.0, 1.0)
}

/// `LivingEntity.knockback(power, xd, zd)`: the power scaled by 1 less the knockback resistance.
pub fn knockback(w: &mut World, i: usize, power: f64, mut xd: f64, mut zd: f64) {
    let resist = knockback_resistance(w, i);
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
