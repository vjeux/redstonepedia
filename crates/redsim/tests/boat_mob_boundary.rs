//! The Mob registration must not silently widen the calibrated boat/lead domain.
//! These are source-branch boundary units, not a claim of new vanilla Mob/leash calibration.
use redsim::{BlockPos, World};
use redsim::entity::{self, EntityKind, Vec3, boat, leash, mob, push};
use redsim::entity::mob::types::MobType;

fn world() -> World {
    let mut w = World::new(BlockPos::new(-16, 0, -16), [32, 16, 32]);
    w.mob.seed(13, 0);
    w
}

fn add_boat(w: &mut World, x: f64, chest: bool) -> u32 {
    let mut state = boat::BoatState::default();
    state.chest = chest;
    w.entities.add(EntityKind::Boat(state), Vec3::new(x, 5.0, 0.0), 0.0)
}

fn add_mob(w: &mut World, kind: MobType, x: f64) -> u32 {
    mob::add(w, kind, Vec3::new(x, 5.0, 0.0), true)
}

fn ix(w: &World, id: u32) -> usize {
    entity::index_of(w, id).unwrap()
}

/// (ironfarm 2026-11-09: the boarding branch is modeled now, `push::boat_start_riding`; the
/// cross-4-underground gate's reference runs met it with a natural mob at the zombie's boat)
#[test]
fn pickup_eligible_zombie_boards_the_boat_not_pushed() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, false);
    let zombie = add_mob(&mut w, MobType::Zombie, 1.1);
    let (bi, zi) = (ix(&w, boat), ix(&w, zombie));
    push::boat_neighbors(&mut w, bi);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    assert_eq!(w.entities.list[bi].delta_movement, Vec3::ZERO);
    assert_eq!(w.entities.list[zi].delta_movement, Vec3::ZERO);
    assert_eq!(w.entities.list[zi].vehicle, Some(boat));
    assert_eq!(w.entities.list[bi].passengers, vec![zombie]);
}

#[test]
fn too_wide_golem_is_the_real_push_positive_control() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, false);
    let golem = add_mob(&mut w, MobType::IronGolem, 1.1);
    let (bi, gi) = (ix(&w, boat), ix(&w, golem));
    assert!(w.entities.list[gi].kind.dims().0 >= boat::WIDTH);
    push::boat_neighbors(&mut w, bi);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    assert!(w.entities.list[gi].delta_movement.x > 0.0);
    assert_eq!(w.entities.list[gi].delta_movement.y, 0.0);
    assert!(w.entities.list[gi].vehicle.is_none());
}

#[test]
fn full_chest_boat_pushes_external_mob_but_not_its_own_rider() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, true);
    let rider = add_mob(&mut w, MobType::Zombie, 0.0);
    mob::mount(&mut w, rider, boat).unwrap();
    let other = add_mob(&mut w, MobType::Villager, 1.1);
    let (bi, ri, oi) = (ix(&w, boat), ix(&w, rider), ix(&w, other));
    push::boat_neighbors(&mut w, bi);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    assert_eq!(w.entities.list[bi].delta_movement, Vec3::ZERO);
    assert_eq!(w.entities.list[ri].delta_movement, Vec3::ZERO);
    assert!(w.entities.list[oi].delta_movement.x > 0.0);
}

#[test]
fn uncalibrated_mob_holder_retarget_is_refused_atomically() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, false);
    let old_holder = add_boat(&mut w, 8.0, false);
    let mob = add_mob(&mut w, MobType::Zombie, 8.0);
    leash::attach(&mut w, boat, old_holder).unwrap();
    let bi = ix(&w, boat);
    w.entities.list[bi].leash.as_mut().unwrap().angular_momentum = 3.0;
    let before = w.entities.list[bi].leash;
    let error = leash::attach(&mut w, boat, mob).unwrap_err();
    assert!(error.contains("Mob holders"), "{error}");
    assert_eq!(w.entities.list[bi].leash, before);
}

#[test]
fn injected_live_mob_holder_fails_closed_before_any_force() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, false);
    let mob = add_mob(&mut w, MobType::Zombie, 8.0);
    let bi = ix(&w, boat);
    w.entities.list[bi].leash = Some(leash::LeashData { holder: mob, angular_momentum: 0.0 });
    leash::tick(&mut w, bi);
    assert!(w.errors.iter().any(|e| e.contains("Mob holders")), "{:?}", w.errors);
    assert_eq!(w.entities.list[bi].delta_movement, Vec3::ZERO);
}

#[test]
fn dead_but_not_removed_mob_holder_takes_the_drop_refusal() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, false);
    let mob = add_mob(&mut w, MobType::Zombie, 8.0);
    let (bi, mi) = (ix(&w, boat), ix(&w, mob));
    let m = mob::mob_mut(&mut w, mi);
    m.health = 0.0;
    m.dead = true;
    assert!(!w.entities.list[mi].removed);
    w.entities.list[bi].leash = Some(leash::LeashData { holder: mob, angular_momentum: 0.0 });
    leash::tick(&mut w, bi);
    assert!(w.entities.list[bi].leash.is_none());
    assert!(w.errors.iter().any(|e| e.contains("unseeded ItemEntity")), "{:?}", w.errors);
    assert_eq!(w.entities.list[bi].delta_movement, Vec3::ZERO);
}

#[test]
fn occupied_boat_leash_is_explicitly_outside_the_foundation_domain() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, false);
    let holder = add_boat(&mut w, 8.0, false);
    let rider = add_mob(&mut w, MobType::Zombie, 0.0);
    mob::mount(&mut w, rider, boat).unwrap();
    let error = leash::attach(&mut w, boat, holder).unwrap_err();
    assert!(error.contains("passenger"), "{error}");
    let bi = ix(&w, boat);
    w.entities.list[bi].leash = Some(leash::LeashData { holder, angular_momentum: 0.0 });
    leash::tick(&mut w, bi);
    assert!(w.errors.iter().any(|e| e.contains("passenger")), "{:?}", w.errors);
    assert_eq!(w.entities.list[bi].delta_movement, Vec3::ZERO);
}

#[test]
fn passenger_bearing_boat_holder_is_also_outside_the_domain() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, false);
    let holder = add_boat(&mut w, 8.0, false);
    let rider = add_mob(&mut w, MobType::Zombie, 8.0);
    mob::mount(&mut w, rider, holder).unwrap();
    let error = leash::attach(&mut w, boat, holder).unwrap_err();
    assert!(error.contains("passenger"), "{error}");
    let bi = ix(&w, boat);
    w.entities.list[bi].leash = Some(leash::LeashData { holder, angular_momentum: 0.0 });
    leash::tick(&mut w, bi);
    assert!(w.errors.iter().any(|e| e.contains("passenger")), "{:?}", w.errors);
    assert_eq!(w.entities.list[bi].delta_movement, Vec3::ZERO);
}

#[test]
fn pickup_is_checked_before_the_generic_push_height_gate() {
    let mut w = world();
    let boat = add_boat(&mut w, 0.0, false);
    let zombie = add_mob(&mut w, MobType::Zombie, 1.1);
    let (bi, zi) = (ix(&w, boat), ix(&w, zombie));
    w.entities.set_pos(zi, Vec3::new(1.1, 5.1, 0.0));
    push::boat_neighbors(&mut w, bi);
    // (boarded: the pickup branch runs before the push's height test, which would skip it)
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    assert_eq!(w.entities.list[zi].vehicle, Some(boat));
    assert_eq!(w.entities.list[zi].delta_movement, Vec3::ZERO);
}
