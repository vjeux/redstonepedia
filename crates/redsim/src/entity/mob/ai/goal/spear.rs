//! The spear (26.3): `SpearUseGoal` (a zombie's at priority 2, a zombified piglin's at 1) and the
//! held spear's `KineticWeapon` (`ItemStack.onUseTick` while it is used: `damageEntities`), read
//! line by line.
//!
//! * The goal: with a target and a spear in hand, it walks up to within 10, lowers the spear
//!   (`startUsingItem`, held for `reducedTickDelay(delay + the damage window)`) and charges at the
//!   target; once in range (2) or at the path's end it backs off toward a spot away from the target
//!   (`LandRandomPos.getPosAway`), and when the spear comes up it flees to one farther away (9-11
//!   beyond the target, 2 more for a rider) for at most 50 ticks. The speeds are x the root
//!   vehicle's `chargeSpeedModifier` (a zombie horse's 1.4).
//! * The weapon, each tick it is used past its delay: the entities along the head's look from
//!   1 to 2.25 blocks out (`AttackRange`'s reach x its mob factor 0.5, plus the forward motion)
//!   within its 0.125 margin (`ProjectileUtil.getHitEntitiesAlong`), each one not stabbed in the
//!   last 10 ticks: the attacker's speed along its look (its last tick's move x 20, a rider's
//!   root vehicle's) against the target's gives the conditions (dismount, knockback, damage:
//!   each within its time and over its speed x 0.2 for a mob) and `stabAttack`'s damage (the
//!   ATTACK_DAMAGE base + floor(relative speed x its multiplier)), the `spear` damage type
//!   (no hurt knockback: `#no_knockback`), the extra knockback 0.4 along the attacker's yaw (the
//!   attacker slowed to 0.6), the dismount.

use super::super::{ai, ai_mut, with_nav};
use super::reduced_tick_delay;
use crate::entity::mob::ai::target::{self, Who};
use crate::entity::mob::{Damage, living, mob};
use crate::entity::{Aabb, EntityKind, Vec3, collide, mth};
use crate::world::World;

/// A `SpearUseGoal` (its state while it runs).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SpearUse {
    pub state: Option<SpearState>,
}

/// `SpearUseGoal.SpearUseState`.
#[derive(Clone, PartialEq, Debug)]
pub struct SpearState {
    pub engage_time: i32,
    pub fleeing_time: i32,
    pub away_pos: Option<Vec3>,
    pub done: bool,
}

impl Default for SpearState {
    fn default() -> SpearState {
        SpearState { engage_time: -1, fleeing_time: -1, away_pos: None, done: false }
    }
}

/// `MAX_FLEEING_TIME`: `reducedTickDelay(100)`.
fn max_fleeing_time() -> i32 {
    reduced_tick_delay(100)
}

/// A spear's `KineticWeapon` (`Item.Properties.spear`): its delay, the dismount and knockback
/// conditions (max ticks, min attacker speed), the damage condition (max ticks, min relative
/// speed) and the damage multiplier.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Kinetic {
    pub delay: i32,
    pub dismount: (i32, f32),
    pub knockback: (i32, f32),
    pub damage: (i32, f32),
    pub multiplier: f32,
}

impl Kinetic {
    /// `computeDamageUseDuration()`: the delay and the damage window.
    fn damage_use_duration(&self) -> i32 {
        self.delay + self.damage.0
    }
}

/// `Item.Properties.spear(material, attackDuration, damageMultiplier, delay, dismountTime,
/// dismountThreshold, knockbackTime, knockbackThreshold, damageTime, damageThreshold)`: each time
/// `(int)(t * 20.0F)`.
#[allow(clippy::too_many_arguments)]
fn spear(damage_multiplier: f32, delay: f32, dismount_time: f32, dismount: f32, knockback_time: f32, knockback: f32, damage_time: f32, damage: f32) -> Kinetic {
    let ticks = |t: f32| (t * 20.0f32) as i32;
    Kinetic { delay: ticks(delay), dismount: (ticks(dismount_time), dismount), knockback: (ticks(knockback_time), knockback), damage: (ticks(damage_time), damage), multiplier: damage_multiplier }
}

/// The `KineticWeapon` of an item (the spears of `Items`).
pub fn kinetic(item: &str) -> Option<Kinetic> {
    Some(match item.strip_prefix("minecraft:").unwrap_or(item) {
        "wooden_spear" => spear(0.7, 0.75, 5.0, 14.0, 10.0, 5.1, 15.0, 4.6),
        "stone_spear" => spear(0.82, 0.7, 4.5, 13.0, 9.0, 5.1, 13.75, 4.6),
        "copper_spear" => spear(0.82, 0.65, 4.0, 12.0, 8.25, 5.1, 12.5, 4.6),
        "iron_spear" => spear(0.95, 0.6, 2.5, 11.0, 6.75, 5.1, 11.25, 4.6),
        "golden_spear" => spear(0.7, 0.7, 3.5, 13.0, 8.5, 5.1, 13.75, 4.6),
        "diamond_spear" => spear(1.075, 0.5, 3.0, 10.0, 6.5, 5.1, 10.0, 4.6),
        "netherite_spear" => spear(1.2, 0.4, 2.5, 9.0, 5.5, 5.1, 8.75, 4.6),
        _ => return None,
    })
}

/// The spear's `AttackRange(2.0, 4.5, 2.0, 6.5, 0.125, 0.5)` for a mob: its reach x the mob
/// factor (as floats) and its hitbox margin.
const MIN_REACH: f32 = 2.0f32 * 0.5f32;
const MAX_REACH: f32 = 4.5f32 * 0.5f32;
const HITBOX_MARGIN: f32 = 0.125;

/// The main hand's spear (a mob's `getMainHandItem().get(KINETIC_WEAPON)`).
pub fn held(w: &World, i: usize) -> Option<Kinetic> {
    mob(w, i).mainhand.as_ref().and_then(|h| kinetic(&h.item))
}

/// `ableToAttack()`: a target and a spear in hand.
fn able_to_attack(w: &World, i: usize) -> bool {
    ai(w, i).target.is_some() && held(w, i).is_some()
}

pub fn can_use(w: &mut World, i: usize, _g: &mut SpearUse) -> bool {
    able_to_attack(w, i) && !ai(w, i).using_item
}

pub fn can_continue(w: &mut World, i: usize, g: &mut SpearUse) -> bool {
    g.state.as_ref().is_some_and(|s| !s.done) && able_to_attack(w, i)
}

pub fn start(w: &mut World, i: usize, g: &mut SpearUse) {
    ai_mut(w, i).aggressive = true;
    g.state = Some(SpearState::default());
}

pub fn stop(w: &mut World, i: usize, g: &mut SpearUse) {
    with_nav(w, i, |_, _, nav, _| nav.stop());
    ai_mut(w, i).aggressive = false;
    g.state = None;
    super::skeleton::stop_using_item(w, i);
}

/// The root vehicle of entity `i` (itself when it rides nothing).
fn root_vehicle(w: &World, i: usize) -> usize {
    let mut k = i;
    for _ in 0..16 {
        match w.entities.list[k].vehicle.and_then(|v| crate::entity::index_of(w, v)) {
            Some(v) => k = v,
            None => break,
        }
    }
    k
}

/// `Mob.chargeSpeedModifier()`: a zombie horse's 1.4, every other mob's 1.
fn charge_speed_modifier(w: &World, k: usize) -> f32 {
    match &w.entities.list[k].kind {
        EntityKind::Mob(m) if matches!(m.ty.def().id, "zombie_horse" | "baby_zombie_horse") => 1.4,
        _ => 1.0,
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut SpearUse) {
    let Some(state) = g.state.as_mut() else { return };
    let Some(tid) = target::get_target(w, i) else { return };
    let Some(t) = target::who(w, tid) else { return };
    let Who::Entity(j) = t else {
        w.mob.ai.legacy.insert("a mob's spear charge at a player (SpearUseGoal)");
        return;
    };
    let tp = w.entities.list[j].pos;
    let dist_sqr = w.entities.list[i].pos.distance_to_sqr(tp);
    // the root vehicle (itself on foot) is a Mob: its chargeSpeedModifier
    let root = root_vehicle(w, i);
    let speed = charge_speed_modifier(w, root) as f64;
    let mount_distance = if w.entities.list[i].vehicle.is_some() { 2.0 } else { 0.0 };
    crate::entity::mob::ai::mob_look_at(w, i, j, 30.0, 30.0);
    let te = target::eye_of(w, t);
    ai_mut(w, i).look.set_look_at(te.x, te.y, te.z, 30.0, 30.0);
    let target_block = super::melee::target_block(w, t);
    if state.engage_time < 0 {
        // notEngagedYet: walk up to within 10
        if dist_sqr > 10.0f32 as f64 * 10.0f32 as f64 {
            with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity_default(w, nm, target_block, speed * 1.0, arena));
            return;
        }
        let duration = held(w, i).map_or(0, |k| k.damage_use_duration());
        state.engage_time = reduced_tick_delay(duration);
        super::skeleton::start_using_item(w, i);
        ai_mut(w, i).stabbed.clear();
    }
    // tickAndCheckEngagement: the spear comes up, the flight begins
    let mut engagement_over = false;
    if state.engage_time > 0 {
        state.engage_time -= 1;
        engagement_over = state.engage_time == 0;
    }
    if engagement_over {
        super::skeleton::stop_using_item(w, i);
        let d = dist_sqr.sqrt();
        state.away_pos = super::stroll::land_pos_away(w, i, (9.0 + mount_distance - d).max(0.0), (11.0 + mount_distance - d).max(1.0), 7, tp);
        state.fleeing_time = 1;
    }
    // tickAndCheckFleeing: done 50 ticks into the flight
    let mut fled = false;
    if state.fleeing_time > 0 {
        state.fleeing_time += 1;
        if state.fleeing_time as f64 > max_fleeing_time() as f64 {
            state.done = true;
            fled = true;
        }
    }
    if fled {
        return;
    }
    match state.away_pos {
        Some(away) => {
            with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, away.x, away.y, away.z, 1, speed * 1.0, arena));
            if crate::entity::mob::ai::nav_of(w, i).is_done() {
                if state.fleeing_time > 0 {
                    state.done = true;
                    return;
                }
                state.away_pos = None;
            }
        }
        None => {
            with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity_default(w, nm, target_block, speed * 1.0, arena));
            if dist_sqr < 2.0f32 as f64 * 2.0f32 as f64 || crate::entity::mob::ai::nav_of(w, i).is_done() {
                let d = dist_sqr.sqrt();
                state.away_pos = super::stroll::land_pos_away(w, i, 6.0 + mount_distance - d, 7.0 + mount_distance - d, 7, tp);
            }
        }
    }
}

// ------------------------------------------------------------------------- the weapon

/// `Entity.calculateViewVector(xRot, yRot)` (Mth's sine table).
fn view_vector(x_rot: f32, y_rot: f32) -> Vec3 {
    let x = x_rot * (std::f64::consts::PI / 180.0) as f32;
    let y = -y_rot * (std::f64::consts::PI / 180.0) as f32;
    let (y_cos, y_sin) = (mth::cos(y as f64), mth::sin(y as f64));
    let (x_cos, x_sin) = (mth::cos(x as f64), mth::sin(x as f64));
    Vec3::new((y_sin * x_cos) as f64, -x_sin as f64, (y_cos * x_cos) as f64)
}

/// `KineticWeapon.getMotion(entity)`: a mob rider's root vehicle's last move, x 20.
fn motion(w: &World, k: usize) -> Vec3 {
    let r = root_vehicle(w, k);
    w.entities.list[r].last_known_speed.scale(20.0)
}

/// `PiercingWeapon.canHitEntity(jabber, target)`: alive, pickable (a mob, a boat, a minecart, an
/// armor stand), not riding with it.
fn can_hit(w: &World, i: usize, j: usize) -> bool {
    let e = &w.entities.list[j];
    let alive = !e.removed && !matches!(&e.kind, EntityKind::Mob(m) if m.health <= 0.0);
    let pickable = matches!(e.kind, EntityKind::Mob(_) | EntityKind::Boat(_) | EntityKind::Minecart(_) | EntityKind::ArmorStand(_));
    alive && pickable && root_vehicle(w, i) != root_vehicle(w, j)
}

/// `ProjectileUtil.getHitEntitiesAlong(attacker, attackRange, canHitEntity, COLLIDER)`: the
/// entities (in the level's order) along the head's look between the reach's ends, a block in the
/// way short of the near end hitting none.
fn hit_entities_along(w: &mut World, i: usize) -> Vec<usize> {
    let e = &w.entities.list[i];
    let look = view_vector(e.x_rot, ai(w, i).y_head_rot);
    let eye = crate::entity::mob::ai::eye_pos(w, i);
    let from = eye.add_v(look.scale(MIN_REACH as f64));
    let movement = e.delta_movement.dot(look);
    let mut to = eye.add_v(look.scale(MAX_REACH as f64 + movement.max(0.0)));
    let hit = crate::entity::mob::los::clip_collider(w, eye, to);
    if !hit.miss {
        to = hit.location;
        if eye.distance_to_sqr(to) < eye.distance_to_sqr(from) {
            return Vec::new();
        }
    }
    let m = HITBOX_MARGIN as f64;
    let half = m / 2.0;
    let d = to.subtract_v(from);
    let area = Aabb::new(from.x - half, from.y - half, from.z - half, from.x + half, from.y + half, from.z + half).expand_towards(d.x, d.y, d.z).inflate(1.0, 1.0, 1.0);
    // a player in the way (the AFK player, a scene's): not modeled
    let near_player = w.mob.ai.players.iter().filter(|p| p.mode != crate::entity::mob::ai::GameMode::Spectator).any(|p| crate::entity::make_bounding_box(p.pos, target::PLAYER_WIDTH, target::PLAYER_HEIGHT).intersects(area));
    if near_player {
        w.mob.ai.legacy.insert("a spear's stab reaching a player");
    }
    let id = w.entities.list[i].id;
    let candidates = w.get_entities(area, |x| x.id != id);
    let mut out = Vec::new();
    for j in candidates {
        if !can_hit(w, i, j) {
            continue;
        }
        let bb = w.entities.list[j].bb;
        if collide::aabb_contains(bb, from) || collide::clip(bb, from, to).is_some() {
            out.push(j);
        } else if let Some(outside) = collide::clip(bb.inflate(m, m, m), from, to) {
            let mut toward = bb.center();
            let bh = crate::entity::mob::los::clip_collider(w, outside, toward);
            if !bh.miss {
                toward = bh.location;
            }
            if collide::clip(bb, outside, toward).is_some() {
                out.push(j);
            }
        }
    }
    out
}

/// `KineticWeapon.damageEntities(stack, ticksRemaining, mob, MAINHAND)`.
pub fn damage_entities(w: &mut World, i: usize, remaining: i32) {
    let Some(k) = held(w, i) else { return };
    let mut ticks_used = 72000 - remaining;
    if ticks_used < k.delay {
        return;
    }
    ticks_used -= k.delay;
    let look = view_vector(w.entities.list[i].x_rot, w.entities.list[i].y_rot);
    let attacker_speed = look.dot(motion(w, i));
    let factor = 0.2f32 as f64;
    let base = crate::entity::mob::ai::attack::base_attack_damage(w, i) as f64;
    for j in hit_entities_along(w, i) {
        let tid = w.entities.list[j].id;
        let now = w.time;
        if ai(w, i).stabbed.iter().any(|&(e, t)| e == tid && now - t < 10) {
            continue;
        }
        match ai_mut(w, i).stabbed.iter_mut().find(|(e, _)| *e == tid) {
            Some(s) => s.1 = now,
            None => ai_mut(w, i).stabbed.push((tid, now)),
        }
        let target_speed = look.dot(motion(w, j));
        let relative = (attacker_speed - target_speed).max(0.0);
        let test = |(max, min_speed): (i32, f32), min_relative: f32| ticks_used <= max && attacker_speed >= min_speed as f64 * factor && relative >= min_relative as f64 * factor;
        let dismount = test(k.dismount, 0.0);
        let knockback = test(k.knockback, 0.0);
        let damage = test((k.damage.0, 0.0), k.damage.1);
        if dismount || knockback || damage {
            let dealt = base as f32 + mth::floor(relative * k.multiplier as f64) as f32;
            if std::env::var("MCTAS_TRACE_SPEAR").is_ok() {
                eprintln!("spear t{}: #{} stabs #{} (used {ticks_used}, speed {attacker_speed:.3}, relative {relative:.3}): damage {damage} {dealt}, knockback {knockback}, dismount {dismount}", w.time, w.entities.list[i].id, tid);
            }
            stab_attack(w, i, j, dealt, damage, knockback, dismount);
        }
    }
}

/// `LivingEntity.stabAttack(MAINHAND, target, damage, dealsDamage, dealsKnockback, dismounts)`.
fn stab_attack(w: &mut World, i: usize, j: usize, damage: f32, deals_damage: bool, deals_knockback: bool, dismounts: bool) -> bool {
    let living_target = matches!(w.entities.list[j].kind, EntityKind::Mob(_));
    // (mob-AI 2026-11-10) EnchantmentHelper.modifyDamage on the stab (the spear's sharpness,
    // smite, ...), its knockback enchantment's extra knockback, its post-attack effects
    let ench: Vec<(&'static str, i32)> = mob(w, i).mainhand.as_ref().map(|h| h.ench.clone()).unwrap_or_default();
    let damage = if living_target { crate::entity::mob::ai::attack::enchant_damage(&ench, mob(w, j).ty.def().id, false, damage) } else { damage };
    let mut affected = deals_knockback;
    let dealt = deals_damage && living_target && {
        let attacker = w.entities.list[i].id;
        let prev = w.mob.ai.attacker.replace(attacker);
        let hurt = living::hurt(w, j, Damage::Spear, damage);
        w.mob.ai.attacker = prev;
        hurt
    };
    if deals_damage && !living_target {
        w.mob.ai.legacy.insert("a spear's stab hurting a vehicle (a boat, a minecart, an armor stand)");
    }
    affected |= dealt;
    if deals_knockback && living_target && !w.entities.list[j].removed {
        // causeExtraKnockback(0.4), then causeExtraKnockback(getKnockback: (ATTACK_KNOCKBACK 0 +
        // the knockback enchantment) / 2): each along the attacker's yaw, the attacker slowed to
        // 0.6
        let yaw = w.entities.list[i].y_rot * (std::f64::consts::PI / 180.0) as f32;
        let (s, c) = (mth::sin(yaw as f64), mth::cos(yaw as f64));
        let kb = ench.iter().find(|&&(e, _)| e == "knockback").map_or(0.0f32, |&(_, l)| l as f32) / 2.0f32;
        for power in [0.4f32, kb] {
            if power > 0.0 {
                crate::entity::mob::ai::attack::knockback(w, j, power as f64, s as f64, -c as f64);
                let e = &mut w.entities.list[i];
                e.delta_movement = e.delta_movement.multiply(0.6, 1.0, 0.6);
            }
        }
    }
    if dismounts && w.entities.list[j].vehicle.is_some() {
        affected = true;
        crate::entity::stop_riding(w, j);
    }
    // doPostAttackEffects after a hurt: fire aspect's ignite
    let fire = ench.iter().find(|&&(e, _)| e == "fire_aspect").map_or(0, |&(_, l)| l);
    if dealt && fire > 0 && !mob(w, j).ty.def().fire_immune {
        let t = crate::entity::mob::ignite_ticks(w, j, ((4.0f32 + 4.0f32 * (fire - 1) as f32) * 20.0f32).floor() as i32);
        let m = crate::entity::mob::mob_mut(w, j);
        if m.fire_ticks < t {
            m.fire_ticks = t;
        }
    }
    if !affected {
        return false;
    }
    let tid = w.entities.list[j].id;
    ai_mut(w, i).last_hurt_mob = Some(tid);
    true
}
