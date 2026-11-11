//! Java 26.3 `Leashable`: server-owned boat leashes, in Entity.baseTick order.
//!
//! A boat is a single-point holder, NOT a quad holder. The impulse is one-sided; no
//! equal/opposite impulse, vertical collision lift, or client-control approximation is added.
//! `attach` is the ordinary attachment contract; loading unresolved UUIDs is not modeled.
//! A broken lead's unseeded ItemEntity kick is deliberately a model error after unlinking.

use super::{Entity, EntityKind, Vec3, boat, collide, index_of};
use crate::World;

pub const SLACK: f64 = 6.0;
pub const SNAP: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeashData {
    pub holder: u32,
    pub angular_momentum: f64,
}

/// Keep the calibrated domain closed when new entity kinds become representable.
fn domain_error(entity: &Entity, holder: &Entity) -> Option<&'static str> {
    // (mob-AI 2026-11-11) a trader llama leashed to its wandering trader (a single-point lead: a
    // trader is no quad holder)
    if let (EntityKind::Mob(m), EntityKind::Mob(h)) = (&entity.kind, &holder.kind)
        && m.ty.def().id == "trader_llama"
        && h.ty.def().id == "wandering_trader"
    {
        return if entity.vehicle.is_some() || !entity.passengers.is_empty() { Some("leash: a ridden or riding trader llama's lead is not modeled") } else { None };
    }
    if !matches!(entity.kind, EntityKind::Boat(_)) {
        Some("leash: only server-owned boats are modeled as leasees")
    } else if entity.vehicle.is_some() {
        Some("leash: a riding boat's leash dynamics are not modeled")
    } else if matches!(holder.kind, EntityKind::Mob(_)) {
        Some("leash: Mob holders are outside the calibrated boat-lead domain")
    } else if !entity.passengers.is_empty()
        || (matches!(holder.kind, EntityKind::Boat(_)) && !holder.passengers.is_empty())
    {
        Some("leash: boats with passengers are outside the calibrated boat-lead domain")
    } else {
        None
    }
}

/// `Entity.canInteractWithLevel`: every represented kind is nonspectating. A Mob's dying
/// body remains present for 20 gt, but `LivingEntity.isAlive` already rejects health <= 0.
fn can_interact(entity: &Entity) -> bool {
    !entity.removed && match &entity.kind {
        EntityKind::Mob(m) => m.health > 0.0,
        EntityKind::Boat(_) | EntityKind::Minecart(_) | EntityKind::Item(_)
        | EntityKind::ArmorStand(_) | EntityKind::PrimedTnt(_) | EntityKind::Arrow(_) => true,
    }
}

/// No self-link or unresolved/removed endpoint. Only unoccupied server-owned boats as leasees.
/// Higher-level survival transfer eligibility is separate from this physics fixture API.
/// Changing holder preserves angular momentum; cycles are legal, as in the game.
pub fn attach(w: &mut World, entity: u32, holder: u32) -> Result<(), String> {
    let i = index_of(w, entity).ok_or_else(|| format!("leash: missing entity {entity}"))?;
    let j = index_of(w, holder).ok_or_else(|| format!("leash: missing holder {holder}"))?;
    if entity == holder || w.entities.list[i].removed || w.entities.list[j].removed {
        return Err("leash: self-link or removed entity".into());
    }
    if !matches!(w.entities.list[i].kind, EntityKind::Boat(_)) {
        return Err("leash: only server-owned boats are modeled as leasees".into());
    }
    if center_distance(&w.entities.list[i], &w.entities.list[j]) > SNAP {
        return Err("leash: holder is beyond the 12-block attachment range".into());
    }
    if w.entities.list[i].vehicle.is_some() {
        return Err("leash: attaching a riding boat would dismount it (not modeled)".into());
    }
    if let Some(why) = domain_error(&w.entities.list[i], &w.entities.list[j]) {
        return Err(why.into());
    }
    let angular_momentum = w.entities.list[i].leash.map_or(0.0, |l| l.angular_momentum);
    w.entities.list[i].leash = Some(LeashData { holder, angular_momentum });
    Ok(())
}

/// `removeLeash`, without an item drop (e.g. fixture teardown).
pub fn remove(w: &mut World, entity: u32) -> Result<(), String> {
    let i = index_of(w, entity).ok_or_else(|| format!("leash: missing entity {entity}"))?;
    if w.entities.list[i].removed || !matches!(w.entities.list[i].kind, EntityKind::Boat(_)) {
        return Err("leash: missing or unsupported leashee".into());
    }
    w.entities.list[i].leash = None;
    Ok(())
}

/// (mob-AI 2026-11-11) `setLeashedTo(holder, true)` of a mob (the wandering trader spawner's
/// llamas): a new LeashData (no angular momentum); it is not riding anything.
pub fn attach_mob(w: &mut World, entity: u32, holder: u32) {
    let Some(i) = index_of(w, entity) else { return };
    let angular_momentum = w.entities.list[i].leash.map_or(0.0, |l| l.angular_momentum);
    w.entities.list[i].leash = Some(LeashData { holder, angular_momentum });
}

fn center_distance(entity: &Entity, holder: &Entity) -> f64 {
    entity.bb.center().distance_to_sqr(holder.bb.center()).sqrt()
}

/// `getKnownMovement` for the represented ordinary kinds. The Mob arm is explicit for enum
/// integration, not a Mob/leash certification: domain_error rejects Mob holders and boats with
/// passengers before spring evaluation. Enabling them requires separate calibration and NoAI /
/// controller handling, not a zero-motion fallback.
fn known_movement(e: &Entity) -> Vec3 {
    match e.kind {
        EntityKind::Boat(_) | EntityKind::Minecart(_) | EntityKind::Item(_)
        | EntityKind::ArmorStand(_) | EntityKind::PrimedTnt(_) | EntityKind::Arrow(_) => e.delta_movement,
        EntityKind::Mob(_) => e.delta_movement,
    }
}

/// One `computeDampenedSpringInteraction`, torque BEFORE force damping.
fn spring(pivot: Vec3, position: Vec3, motion: Vec3, lever: Vec3) -> Option<(Vec3, f64)> {
    let distance = position.distance_to_sqr(pivot).sqrt();
    if distance < SLACK {
        return None;
    }
    let mut force = pivot.subtract_v(position).normalize().scale(distance - SLACK);
    let torque = lever.z * force.x - lever.x * force.z;
    if motion.dot(force) >= 0.0 {
        force = force.scale(0.3f32 as f64);
    }
    // Wrench.accumulate starts at zero; scale(1.0) preserves the Java operation order.
    Some((Vec3::ZERO.add_v(force).scale(1.0), (0.0 + torque) * 1.0))
}

fn angular_friction(w: &World, i: usize) -> f32 {
    let e = &w.entities.list[i];
    if e.on_ground {
        let p = collide::get_on_pos(w, e, 0.500001f32);
        boat::block_friction(&w.get(p)) * 0.91f32
    } else if e.fluid.in_water() {
        0.8f32
    } else {
        0.91f32
    }
}

fn drop_lead(w: &mut World, i: usize) {
    w.entities.list[i].leash = None;
    // Entity.spawnAtLocation uses a new, nanoTime-seeded ItemEntity. Never invent its kick
    // or omit the side effect and claim a fully modeled continuation after a snap.
    w.errors.push("leash: dropped lead has unseeded ItemEntity motion (post-drop continuation is not modeled)".into());
}

/// `PathfinderMob.isPanicking()`: a running PanicGoal.
fn panicking(w: &World, i: usize) -> bool {
    use crate::entity::mob::ai::goal::{Goal, animal::AnimalGoal};
    crate::entity::mob::ai::ai(w, i).goals.goals.iter().any(|g| g.running && matches!(g.goal, Goal::Animal(AnimalGoal::Panic { .. })))
}

/// `PathfinderMob.closeRangeLeashBehaviour(holder)`: `enableControlFlag(MOVE)`, then
/// `moveTo(position + (holder - position).normalize() * max(distanceTo - 2F, 0F), followLeashSpeed)`.
fn close_range(w: &mut World, i: usize, j: usize) {
    use crate::entity::mob::ai as mai;
    mai::ai_mut(w, i).goals.set_control_flag(mai::goal::Flag::Move, true);
    let (p, h) = (w.entities.list[i].pos, w.entities.list[j].pos);
    let (dx, dy, dz) = (h.x - p.x, h.y - p.y, h.z - p.z);
    // Entity.distanceTo: in floats
    let (fx, fy, fz) = ((p.x - h.x) as f32, (p.y - h.y) as f32, (p.z - h.z) as f32);
    let dist = crate::entity::mth::sqrt(fx * fx + fy * fy + fz * fz);
    let d = Vec3::new(dx, dy, dz).normalize().scale((dist - 2.0f32).max(0.0f32) as f64);
    let (x, y, z) = (p.x + d.x, p.y + d.y, p.z + d.z);
    let speed = if matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.ty.def().id == "trader_llama") { 2.0 } else { 1.0 };
    mai::with_nav(w, i, |w, nm, nav, arena| {
        nav.move_to_xyz(w, nm, x, y, z, 1, speed, arena);
    });
}

/// `Leashable.tickLeash`, called after baseTick's fluid interaction and firstTick=false.
pub fn tick(w: &mut World, i: usize) {
    let Some(mut data) = w.entities.list[i].leash else { return };
    let Some(j) = index_of(w, data.holder) else {
        drop_lead(w, i);
        return;
    };
    if !can_interact(&w.entities.list[i]) || !can_interact(&w.entities.list[j]) {
        drop_lead(w, i);
        return;
    }
    if let Some(why) = domain_error(&w.entities.list[i], &w.entities.list[j]) {
        w.errors.push(why.into());
        return;
    }
    // (mob-AI 2026-11-11) whenLeashedTo: PathfinderMob's home on the holder's block, radius
    // (int) 6 - 1 = 5 (the holder's notifyLeashHolder: nothing for a trader)
    let mob_leashee = matches!(w.entities.list[i].kind, EntityKind::Mob(_)) && crate::entity::mob::ai::has_ai(w, i);
    if mob_leashee {
        let hb = w.entities.list[j].block_position();
        crate::entity::mob::ai::ai_mut(w, i).home = Some((hb, 5));
    }
    let e = &w.entities.list[i];
    let holder = &w.entities.list[j];
    let distance = center_distance(e, holder);
    let mut pulled = false;
    if distance > SNAP {
        drop_lead(w, i);
        // Mob.leashTooFarBehaviour: the MOVE control flag off
        if mob_leashee {
            crate::entity::mob::ai::ai_mut(w, i).goals.set_control_flag(crate::entity::mob::ai::goal::Flag::Move, false);
        }
        // tickLeash retains its local LeashData through the final yaw/friction update.
    } else if distance > SLACK - holder.kind.dims().0 as f64 - e.kind.dims().0 as f64 {
        let (width, height) = e.kind.dims();
        let (hw, hh) = holder.kind.dims();
        let rot = e.y_rot * (std::f64::consts::PI / 180.0) as f32;
        let hrot = holder.y_rot * (std::f64::consts::PI / 180.0) as f32;
        let lever = Vec3::new(0.0, 0.5, 0.5).multiply(width as f64, height as f64, width as f64).y_rot(-rot);
        let anchor = Vec3::new(0.0, 0.5, 0.0).multiply(hw as f64, hh as f64, hw as f64).y_rot(-hrot);
        let from = e.pos.add_v(lever);
        let to = holder.pos.add_v(anchor);
        if let Some((force, torque)) = spring(to, from, known_movement(e), lever) {
            data.angular_momentum += 10.0 * torque;
            let relative = known_movement(holder).subtract_v(known_movement(e));
            let impulse = force.multiply(0.8, 0.2, 0.8).add_v(relative.scale(0.11));
            let e = &mut w.entities.list[i];
            e.delta_movement = e.delta_movement.add_v(impulse);
            if e.delta_movement.y > -0.5 && e.fall_distance > 1.0 {
                e.fall_distance = 1.0;
            }
            pulled = true;
        }
    }
    // closeRangeLeashBehaviour (no elastic pull): PathfinderMob's, not panicking: the MOVE control
    // flag on, a walk to 2 short of the holder (followLeashSpeed: a llama's 2.0)
    if mob_leashee && distance <= SNAP && !pulled && !panicking(w, i) {
        close_range(w, i, j);
    }
    let friction = angular_friction(w, i);
    let e = &mut w.entities.list[i];
    e.y_rot = (e.y_rot as f64 - data.angular_momentum) as f32;
    data.angular_momentum *= friction as f64;
    if e.leash.is_some() {
        e.leash = Some(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockPos;

    fn pair(distance: f64) -> World {
        let mut w = World::new(BlockPos::new(-16, -16, -16), [32, 32, 32]);
        for z in [0.0, distance] {
            w.entities.add(EntityKind::Boat(Default::default()), Vec3::new(0.0, 5.0, z), 0.0);
        }
        w
    }

    #[test]
    fn mutual_links_are_allowed_but_self_and_far_are_not() {
        let mut w = pair(7.0);
        attach(&mut w, 0, 1).unwrap();
        attach(&mut w, 1, 0).unwrap();
        assert!(attach(&mut w, 0, 0).is_err());
        assert!(attach(&mut pair(12.01), 0, 1).is_err());
    }

    #[test]
    fn slack_does_not_add_force_and_link_changes_preserve_torque() {
        let mut w = pair(2.0);
        attach(&mut w, 0, 1).unwrap();
        tick(&mut w, 0);
        assert_eq!(w.entities.list[0].delta_movement, Vec3::ZERO);
        w.entities.list[0].leash.as_mut().unwrap().angular_momentum = 3.0;
        attach(&mut w, 0, 1).unwrap();
        assert_eq!(w.entities.list[0].leash.unwrap().angular_momentum, 3.0);
        remove(&mut w, 0).unwrap();
        assert!(w.entities.list[0].leash.is_none());
    }

    #[test]
    fn torque_uses_undamped_force() {
        let lever = Vec3::new(0.0, 0.0, 0.6875);
        let (damped, torque) = spring(Vec3::new(8.0, 0.0, 0.0), Vec3::ZERO, Vec3::ZERO, lever).unwrap();
        let (full, torque2) = spring(Vec3::new(8.0, 0.0, 0.0), Vec3::ZERO, Vec3::new(-1.0, 0.0, 0.0), lever).unwrap();
        assert_eq!(damped.x, full.x * 0.3f32 as f64);
        assert_eq!(torque, torque2);
        assert_eq!(torque, 1.375);
    }

    #[test]
    fn snap_unlinks_but_does_not_silently_omit_the_drop() {
        let mut w = pair(12.0);
        attach(&mut w, 0, 1).unwrap();
        w.entities.set_pos(1, Vec3::new(0.0, 5.0, 12.001));
        tick(&mut w, 0);
        assert!(w.entities.list[0].leash.is_none());
        assert_eq!(w.errors.len(), 1);
        assert!(w.errors[0].contains("dropped lead"));
    }
}
