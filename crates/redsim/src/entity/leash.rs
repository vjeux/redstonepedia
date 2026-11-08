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

/// No self-link or unresolved/removed endpoint. Only server-owned boats as leasees.
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

fn center_distance(entity: &Entity, holder: &Entity) -> f64 {
    entity.bb.center().distance_to_sqr(holder.bb.center()).sqrt()
}

/// All current kinds have no Player controller or NoAI Mob branch. An added kind must
/// explicitly implement Entity.getKnownMovement / Leashable.getHolderMovement here.
fn known_movement(e: &Entity) -> Vec3 {
    match e.kind {
        EntityKind::Boat(_) | EntityKind::Minecart(_) | EntityKind::Item(_)
        | EntityKind::ArmorStand(_) | EntityKind::PrimedTnt(_) => e.delta_movement,
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

/// `Leashable.tickLeash`, called after baseTick's fluid interaction and firstTick=false.
pub fn tick(w: &mut World, i: usize) {
    let Some(mut data) = w.entities.list[i].leash else { return };
    let Some(j) = index_of(w, data.holder) else {
        drop_lead(w, i);
        return;
    };
    if w.entities.list[i].removed || w.entities.list[j].removed {
        drop_lead(w, i);
        return;
    }
    let e = &w.entities.list[i];
    let holder = &w.entities.list[j];
    let distance = center_distance(e, holder);
    if distance > SNAP {
        drop_lead(w, i);
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
        }
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
