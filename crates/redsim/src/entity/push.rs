//! Entity-to-entity pushes (`Entity.push` and `AbstractBoat.push`, Java 26.3).
//! These are horizontal impulses, separate from swept collision boxes.

use super::{EntityKind, index_of, minecart};
#[cfg(test)]
use super::Vec3;
use crate::World;

fn root(w: &World, i: usize) -> u32 {
    let mut e = &w.entities.list[i];
    // A legal riding graph is acyclic. Bound traversal defensively for hand-built worlds.
    for _ in 0..w.entities.list.len() {
        let Some(v) = e.vehicle else { break };
        let Some(j) = index_of(w, v) else { break };
        e = &w.entities.list[j];
    }
    e.id
}

/// `Entity.isPassengerOfSameVehicle` (ordinary unmounted entities have distinct roots).
pub fn same_vehicle(w: &World, i: usize, j: usize) -> bool {
    root(w, i) == root(w, j)
}

/// `Entity.push(Entity)`: the game's unusual absMax then sqrt, not vector length.
pub fn generic(w: &mut World, i: usize, j: usize) {
    let a = &w.entities.list[i];
    let b = &w.entities.list[j];
    if same_vehicle(w, i, j) || a.no_physics || b.no_physics {
        return;
    }
    let mut x = b.pos.x - a.pos.x;
    let mut z = b.pos.z - a.pos.z;
    let mut distance = x.abs().max(z.abs());
    if distance < 0.01f32 as f64 {
        return;
    }
    distance = distance.sqrt();
    x /= distance;
    z /= distance;
    let power = (1.0 / distance).min(1.0);
    x *= power;
    z *= power;
    x *= 0.05f32 as f64;
    z *= 0.05f32 as f64;
    let push_a = a.passengers.is_empty() && minecart::is_pushable(a);
    let push_b = b.passengers.is_empty() && minecart::is_pushable(b);
    if push_a {
        let a = &mut w.entities.list[i];
        a.delta_movement = a.delta_movement.add(-x, 0.0, -z);
    }
    if push_b {
        let b = &mut w.entities.list[j];
        b.delta_movement = b.delta_movement.add(x, 0.0, z);
    }
}

/// The new Mob kinds enter AbstractBoat's automatic-boarding branch before its push-height
/// test. They are all absent from Java 26.3's `cannot_be_pushed_onto_boats` entity-type tag.
/// Keep the type match exhaustive: adding a Mob type requires checking that tag again.
fn pickup_eligible(boat: &super::Entity, other: &super::Entity) -> bool {
    use super::mob::types::MobType;
    let EntityKind::Boat(state) = &boat.kind else { return false };
    let EntityKind::Mob(mob) = &other.kind else { return false };
    let tag_allows = match mob.ty {
        MobType::Villager | MobType::Zombie | MobType::IronGolem => true,
    };
    let capacity = if state.chest { 1 } else { 2 };
    // This model is server-owned and has no Player controller. `hasEnoughSpaceFor` is strict.
    tag_allows && boat.passengers.len() < capacity && other.vehicle.is_none()
        && other.kind.dims().0 < boat.kind.dims().0
}

/// AbstractBoat's final nearby-entity loop, in EntitySectionStorage query order.
pub fn boat_neighbors(w: &mut World, i: usize) {
    let bb = w.entities.list[i].bb.inflate(0.2f32 as f64, -0.01f32 as f64, 0.2f32 as f64);
    let nearby = w.get_entities(bb, minecart::is_pushable);
    for j in nearby {
        if i == j || same_vehicle(w, i, j) {
            continue;
        }
        if pickup_eligible(&w.entities.list[i], &w.entities.list[j]) {
            w.errors.push("boat: picking up a mob is not modeled".into());
            continue;
        }
        let boat = &w.entities.list[i];
        let other = &w.entities.list[j];
        let reaches = if matches!(other.kind, EntityKind::Boat(_)) {
            other.bb.min_y < boat.bb.max_y
        } else {
            other.bb.min_y <= boat.bb.min_y
        };
        if reaches {
            generic(w, i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockPos;

    #[test]
    fn push_is_horizontal_and_passenger_bearing_boat_is_not_pushed() {
        let mut w = World::new(BlockPos::new(0, 0, 0), [8, 8, 8]);
        w.entities.add(EntityKind::Boat(Default::default()), Vec3::new(2.0, 2.0, 2.0), 0.0);
        w.entities.add(EntityKind::Boat(Default::default()), Vec3::new(3.0, 2.1, 2.0), 0.0);
        w.entities.list[0].passengers.push(99);
        generic(&mut w, 0, 1);
        assert_eq!(w.entities.list[0].delta_movement, Vec3::ZERO);
        assert_eq!(w.entities.list[1].delta_movement, Vec3::new(0.05f32 as f64, 0.0, 0.0));
    }
}
