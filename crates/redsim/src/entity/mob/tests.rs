//! Known answers of the mob layer (ironfarm), from the G1 review of the brief
//! (`designs/briefs/farm-iron.md`): the spawn search's success on a 4x4 pad inside three
//! villagers' squares (about 82%), the 600 gt cycle and its tick order, a golem's terminal drift
//! in a water current (about 0.056 blocks/gt) and its fall out of a follow box (about 22 gt for
//! 15.5 blocks).

use super::types::MobType;
use super::{MobEvent, add, mob_mut, spawn};
use crate::block::Block;
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::{World, flags};

fn world(seed: i64) -> World {
    let mut w = World::new(BlockPos::new(-24, -24, -24), [48, 48, 48]);
    w.mob.seed(seed, 6000);
    w
}

fn put(w: &mut World, x: i32, y: i32, z: i32, s: &str) {
    w.set_block(BlockPos::new(x, y, z), Block::parse(s).unwrap(), flags::RAW);
}

/// Three villagers at x = 0, 2, 4 standing on glass (which no golem spawns on).
fn villagers(w: &mut World) -> Vec<u32> {
    let mut ids = Vec::new();
    for x in [0, 2, 4] {
        put(w, x, -1, 0, "minecraft:glass");
        ids.push(add(w, MobType::Villager, Vec3::new(x as f64 + 0.5, 0.0, 0.5), true));
    }
    ids
}

fn remove_golems(w: &mut World) {
    w.entities.list.retain(|e| !matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::IronGolem));
}

/// The review's 82%: a 4x4 pad 4 below three villagers, inside all their 17 x 17 squares; at a
/// hundred-tick mark each villager makes its own 10 tries in turn until one spawns a golem:
/// 1 - (1 - 16/289)^30 = 0.819.
#[test]
fn four_by_four_pad_three_villagers() {
    let mut w = world(11);
    let ids = villagers(&mut w);
    for x in 0..4 {
        for z in 3..7 {
            put(&mut w, x, -4, z, "minecraft:stone");
        }
    }
    let starts: Vec<BlockPos> = ids.iter().map(|id| w.entities.list[crate::entity::index_of(&w, *id).unwrap()].block_position()).collect();
    let (mut ok, marks) = (0, 20000);
    for _ in 0..marks {
        for s in &starts {
            if spawn::try_spawn_golem(&mut w, *s).is_some() {
                ok += 1;
                remove_golems(&mut w);
                break;
            }
        }
    }
    w.mob.spawn_tries.clear();
    let p = ok as f64 / marks as f64;
    let expect = 1.0 - (1.0f64 - 16.0 / 289.0).powi(30);
    assert!((p - expect).abs() < 0.015, "success {p:.3}, expected {expect:.3}");
    // one villager alone: 1 - (1 - 16/289)^10 = 0.434
    let mut ok1 = 0;
    for _ in 0..marks {
        if spawn::try_spawn_golem(&mut w, starts[0]).is_some() {
            ok1 += 1;
            remove_golems(&mut w);
        }
    }
    let p1 = ok1 as f64 / marks as f64;
    let expect1 = 1.0 - (1.0f64 - 16.0 / 289.0).powi(10);
    assert!((p1 - expect1).abs() < 0.015, "one villager: {p1:.3}, expected {expect1:.3}");
}

/// The 600 gt cycle: three willing villagers scared by a zombie in view, a large pad, every
/// golem removed at once (no villager ever sees it): golems come exactly 600 gt apart, no
/// faster, and (tick order: at T+600 only the last spawner and the villagers after it in the
/// list can count 3) mostly from the same villager.
#[test]
fn six_hundred_tick_cycle() {
    let mut w = world(5);
    let ids = villagers(&mut w);
    for x in -4..9 {
        for z in 3..9 {
            put(&mut w, x, -4, z, "minecraft:stone");
        }
    }
    for id in &ids {
        let i = crate::entity::index_of(&w, *id).unwrap();
        if let Some(b) = mob_mut(&mut w, i).brain.as_deref_mut() {
            b.last_slept = Some(0);
        }
    }
    put(&mut w, 2, -1, -4, "minecraft:glass");
    // (mobinfra) shaded: a zombie under the open sky at noon burns (`Mob.burnUndead`)
    put(&mut w, 2, 2, -4, "minecraft:stone");
    add(&mut w, MobType::Zombie, Vec3::new(2.5, 0.0, -3.5), true);
    let mut spawns: Vec<(i64, u32)> = Vec::new();
    for _ in 0..12000 {
        w.tick();
        for e in std::mem::take(&mut w.mob.events) {
            if let MobEvent::GolemTry { t, villager, golem: Some(_), .. } = e {
                spawns.push((t, villager));
            }
        }
        remove_golems(&mut w);
        w.mob.spawn_tries.clear();
    }
    assert!(spawns.len() >= 15, "only {} golems in 12000 gt: {spawns:?}", spawns.len());
    let gaps: Vec<i64> = spawns.windows(2).map(|p| p[1].0 - p[0].0).collect();
    assert!(gaps.iter().all(|g| *g >= 600 && g % 100 == 0), "gaps {gaps:?}");
    let exact = gaps.iter().filter(|g| **g == 600).count();
    assert!(exact * 10 >= gaps.len() * 9, "gaps {gaps:?}");
    let same = spawns.windows(2).filter(|p| p[1].1 == p[0].1).count();
    assert!(same * 10 >= gaps.len() * 8, "spawners {spawns:?}");
}

/// The review's terminal drift: a non-player in a uniform current gets 0.014 a tick and keeps 0.8
/// of its speed (`travelInWater`): v = (v + 0.014) x 0.8 -> 0.056 blocks/gt.
#[test]
fn golem_terminal_drift_in_water() {
    let mut w = world(3);
    for x in -2..14 {
        for z in -2..3 {
            put(&mut w, x, -1, z, "minecraft:stone");
        }
        put(&mut w, x, 0, -2, "minecraft:stone");
        put(&mut w, x, 0, 2, "minecraft:stone");
    }
    for z in -1..2 {
        put(&mut w, -2, 0, z, "minecraft:stone");
        w.set_block_and_update(BlockPos::new(-1, 0, z), Block::parse("minecraft:water[level=0]").unwrap());
    }
    for _ in 0..80 {
        w.tick();
    }
    let id = add(&mut w, MobType::IronGolem, Vec3::new(0.5, 0.0, 0.5), true);
    let mut v = 0.0;
    for _ in 0..30 {
        w.tick();
        let i = crate::entity::index_of(&w, id).unwrap();
        v = w.entities.list[i].delta_movement.x;
    }
    assert!((v - 0.056).abs() < 0.002, "terminal drift {v}");
}

/// The review's fall: from rest, v <- (v - 0.08) x 0.98; 15.5 blocks take about 22 gt.
#[test]
fn golem_fall_out_of_range() {
    let mut w = world(3);
    put(&mut w, 0, -20, 0, "minecraft:stone");
    let id = add(&mut w, MobType::IronGolem, Vec3::new(0.5, 0.0, 0.5), true);
    let mut t = 0;
    loop {
        w.tick();
        t += 1;
        let i = crate::entity::index_of(&w, id).unwrap();
        if w.entities.list[i].pos.y <= -15.5 || t > 100 {
            break;
        }
    }
    assert!((21..=23).contains(&t), "fell 15.5 blocks in {t} gt");
}

/// A mob is pushable while alive (`LivingEntity.isPushable`; boats and minecarts filter by it).
#[test]
fn a_live_mob_is_pushable_a_dead_one_is_not() {
    let mut w = world(3);
    let z = add(&mut w, MobType::Zombie, Vec3::new(1.4, 0.0, 0.5), true);
    let zi = crate::entity::index_of(&w, z).unwrap();
    assert!(crate::entity::minecart::is_pushable(&w.entities.list[zi]));
    mob_mut(&mut w, zi).dead = true;
    assert!(!crate::entity::minecart::is_pushable(&w.entities.list[zi]));
}

/// A passenger's data goes through the summoned mob's contract: a persistent zombie rides its
/// boat; a passenger with NoAI (or any field the layer does not model) is refused, never dropped.
#[test]
fn passenger_data_is_validated() {
    let mut w = world(3);
    let ok = crate::entity::spawn(&mut w, "minecraft:oak_boat", [0.5, 0.0, 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    let boat = ok.expect("a persistent zombie in a boat");
    let rider = w.entities.list.iter().find(|e| e.vehicle == Some(boat)).expect("the zombie rides the boat");
    assert!(matches!(&rider.kind, EntityKind::Mob(m) if m.persistence_required));
    for bad in ["{Passengers:[{id:\"minecraft:zombie\",NoAI:1b}]}", "{Passengers:[{id:\"minecraft:zombie\",IsBaby:1b}]}"] {
        let r = crate::entity::spawn(&mut w, "minecraft:oak_boat", [4.5, 0.0, 0.5], Some(bad));
        assert!(r.is_err(), "{bad} was accepted");
    }
    assert!(crate::entity::spawn(&mut w, "minecraft:zombie", [8.5, 0.0, 0.5], Some("{NoAI:1b}")).is_err());
}

/// Climbing is not modeled: a mob in scaffolding is a model error from any entry point, and it
/// is not pushable while it climbs (`LivingEntity.isPushable`: `!onClimbable`).
#[test]
fn a_mob_in_scaffolding_is_refused() {
    let mut w = world(3);
    put(&mut w, 0, -1, 0, "minecraft:stone");
    put(&mut w, 0, 0, 0, "minecraft:scaffolding[bottom=false,distance=0,waterlogged=false]");
    let z = add(&mut w, MobType::Zombie, Vec3::new(0.5, 0.0, 0.5), true);
    w.tick();
    assert!(w.errors.iter().any(|e| e.contains("climbable")), "{:?}", w.errors);
    let zi = crate::entity::index_of(&w, z).unwrap();
    assert!(!crate::entity::minecart::is_pushable(&w.entities.list[zi]));
}

/// The carousel's boarding and dismount (ironfarm, read in the 26.3 source): a plain minecart
/// rolling at 0.4 a tick down a rail takes the villager standing on it as a passenger
/// (`OldMinecartBehavior.pushAndPickupEntities`: `startRiding`), the villager sits at the cart's
/// own y (`LOWERED_PASSENGER_ATTACHMENT`), and a powered activator rail throws it out beside the
/// track (`Minecart.activateMinecart`: `ejectPassengers`, `getDismountLocationForPassenger`: the
/// motion direction's right first) with a 60 gt boarding cooldown; the cart wobbles (hurt 10).
#[test]
fn a_cart_picks_up_a_villager_and_an_activator_rail_throws_it_out() {
    let mut w = world(3);
    for x in 0..16 {
        put(&mut w, x, -1, 0, "minecraft:stone");
        put(&mut w, x, -1, 1, "minecraft:stone");
        put(&mut w, x, -1, -1, "minecraft:stone");
        put(&mut w, x, 0, 0, "minecraft:rail[shape=east_west,waterlogged=false]");
    }
    put(&mut w, 12, 0, 0, "minecraft:activator_rail[powered=true,shape=east_west,waterlogged=false]");
    put(&mut w, 12, -2, 0, "minecraft:redstone_block");
    let cart = w.entities.add(EntityKind::Minecart(Default::default()), Vec3::new(0.5, 0.0625, 0.5), 0.0);
    let ci = crate::entity::index_of(&w, cart).unwrap();
    w.entities.list[ci].delta_movement = Vec3::new(0.4, 0.0, 0.0);
    let v = add(&mut w, MobType::Villager, Vec3::new(5.5, 0.0, 0.5), true);
    let mut boarded = None;
    let mut thrown = None;
    for t in 1..=60 {
        w.tick();
        assert!(w.errors.is_empty(), "{:?}", w.errors);
        let vi = crate::entity::index_of(&w, v).unwrap();
        let ci = crate::entity::index_of(&w, cart).unwrap();
        match (boarded, w.entities.list[vi].vehicle) {
            (None, Some(c)) => {
                assert_eq!(c, cart);
                boarded = Some(t);
            }
            (Some(_), Some(_)) if t > boarded.unwrap() + 1 => {
                // seated at the cart's own y
                assert!((w.entities.list[vi].pos.y - w.entities.list[ci].pos.y).abs() < 1.0e-9);
            }
            (Some(_), None) if thrown.is_none() => {
                thrown = Some(t);
                let p = w.entities.list[vi].pos;
                // (60 when thrown; the villager, after the cart in the list, ticks once more this
                // tick as a free mob: 59)
                assert_eq!(w.entities.list[vi].boarding_cooldown, 59, "the boarding cooldown");
                assert_eq!(w.entities.list[ci].passengers.len(), 0);
                // moving east, getMotionDirection is east's... the first free offset beside the
                // track (z +-1), on the stone (y 0), at a cell's centre
                assert!(p.x.fract() == 0.5 && p.z.abs().fract() == 0.5 && p.y == 0.0, "thrown to {p:?}");
                assert!((p.z - 0.5).abs() == 1.0, "beside the track: {p:?}");
                assert!(matches!(w.entities.list[ci].kind, EntityKind::Minecart(m) if m.hurt_time == 10 || m.hurt_time == 9));
            }
            _ => {}
        }
    }
    assert!(boarded.is_some(), "the cart never took the villager");
    assert!(thrown.is_some(), "the activator rail never threw the villager out");
}

/// A mob's `pushEntities` meeting a minecart it does not ride: `doPush` = `AbstractMinecart.push`,
/// the cart pushed away by 0.05 (its delta, along the rail) and the mob by a quarter of it.
#[test]
fn a_villager_touching_a_parked_cart_pushes_it_along_its_rail() {
    let mut w = world(3);
    for x in 0..6 {
        put(&mut w, x, -1, 0, "minecraft:stone");
        put(&mut w, x, 0, 0, "minecraft:rail[shape=east_west,waterlogged=false]");
    }
    let cart = w.entities.add(EntityKind::Minecart(Default::default()), Vec3::new(2.5, 0.0, 0.5), 0.0);
    let _v = add(&mut w, MobType::Villager, Vec3::new(3.2, 0.0, 0.5), false);
    for _ in 0..3 {
        w.tick();
        assert!(w.errors.is_empty(), "{:?}", w.errors);
    }
    let ci = crate::entity::index_of(&w, cart).unwrap();
    assert!(w.entities.list[ci].pos.x < 2.5, "the cart pushed west: {:?}", w.entities.list[ci].pos);
}

/// `AbstractBoat.tick`'s boarding branch: a mob meeting the zombie's boat (its box inflated 0.2
/// sideways) takes the boat's free seat; with two, `positionRider` seats the zombie 0.2 forward and
/// the newcomer 0.6 back (a boat at yaw 0 faces +z: `Vec3(0, y, f).yRot(0)`).
#[test]
fn a_mob_meeting_the_zombies_boat_boards_it_and_the_zombie_moves_forward() {
    let mut w = world(3);
    for x in 0..6 {
        for z in -2..3 {
            put(&mut w, x, -1, z, "minecraft:stone");
        }
    }
    let boat = w.entities.add(EntityKind::Boat(Default::default()), Vec3::new(2.5, 0.0, 0.5), 0.0);
    let z = add(&mut w, MobType::Zombie, Vec3::new(2.5, 0.0, 0.5), true);
    super::mount(&mut w, z, boat).unwrap();
    let v = add(&mut w, MobType::Villager, Vec3::new(3.45, 0.0, 0.5), false);
    let mut boarded = false;
    for _ in 0..3 {
        w.tick();
        assert!(w.errors.is_empty(), "{:?}", w.errors);
        let vi = crate::entity::index_of(&w, v).unwrap();
        boarded |= w.entities.list[vi].vehicle == Some(boat);
    }
    assert!(boarded, "the boat never took the villager");
    let bi = crate::entity::index_of(&w, boat).unwrap();
    assert_eq!(w.entities.list[bi].passengers, vec![z, v]);
    let (bp, yaw) = (w.entities.list[bi].pos, w.entities.list[bi].y_rot);
    let zp = w.entities.list[crate::entity::index_of(&w, z).unwrap()].pos;
    let vp = w.entities.list[crate::entity::index_of(&w, v).unwrap()].pos;
    let a = -yaw * (std::f32::consts::PI / 180.0);
    let (c, s) = (crate::entity::mth::cos(a as f64) as f64, crate::entity::mth::sin(a as f64) as f64);
    assert!((zp.x - (bp.x + 0.2f32 as f64 * s)).abs() < 1e-9 && (zp.z - (bp.z + 0.2f32 as f64 * c)).abs() < 1e-9, "the zombie 0.2 forward: {zp:?} (boat {bp:?} yaw {yaw})");
    assert!((vp.x - (bp.x - 0.6f32 as f64 * s)).abs() < 1e-9 && (vp.z - (bp.z - 0.6f32 as f64 * c)).abs() < 1e-9, "the villager 0.6 back: {vp:?}");
}

/// A rider travels like any server-side mob (`canSimulateMovement && isEffectiveAi`): from the
/// zero motion `rideTick` gives it, gravity leaves (0 - 0.08) x 0.98 = -0.0784.
#[test]
fn a_rider_travels() {
    let mut w = world(3);
    put(&mut w, 0, -1, 0, "minecraft:stone");
    let cart = w.entities.add(EntityKind::Minecart(Default::default()), Vec3::new(0.5, 0.0, 0.5), 0.0);
    let z = add(&mut w, MobType::Zombie, Vec3::new(0.5, 0.0, 0.5), true);
    super::mount(&mut w, z, cart).unwrap();
    w.tick();
    let zi = crate::entity::index_of(&w, z).unwrap();
    let v = w.entities.list[zi].delta_movement;
    let expect = -0.08 * (0.98f32 as f64);
    assert!((v.y - expect).abs() < 1.0e-12, "rider motion {v:?}");
}

/// A sign stops flowing water (a `LiquidBlockContainer` that takes only a water source) and lets
/// a golem fall through it (no collision box).
#[test]
fn signs_stop_water_and_let_golems_through() {
    let mut w = world(3);
    for x in 0..4 {
        put(&mut w, x, -1, 0, "minecraft:stone");
        put(&mut w, x, 0, -1, "minecraft:stone");
        put(&mut w, x, 0, 1, "minecraft:stone");
    }
    put(&mut w, -1, 0, 0, "minecraft:stone");
    put(&mut w, 4, -1, 1, "minecraft:stone");
    put(&mut w, 4, 0, 1, "minecraft:stone");
    put(&mut w, 4, 0, 0, "minecraft:oak_wall_sign[facing=north,waterlogged=false]");
    w.set_block_and_update(BlockPos::new(0, 0, 0), Block::parse("minecraft:water[level=0]").unwrap());
    for _ in 0..100 {
        w.tick();
    }
    assert!(matches!(w.get(BlockPos::new(3, 0, 0)), Block::Water { .. }), "the water reached the sign's cell");
    assert!(matches!(w.get(BlockPos::new(4, 0, 0)), Block::Sign { .. }), "the water entered the sign: {}", w.get(BlockPos::new(4, 0, 0)));
    // a golem over a 2x2 of signs falls through them
    for (x, z) in [(10, 10), (11, 10), (10, 11), (11, 11)] {
        put(&mut w, x, 4, z, "minecraft:oak_sign[rotation=0,waterlogged=false]");
        put(&mut w, x, 3, z, "minecraft:stone");
    }
    put(&mut w, 10, -10, 10, "minecraft:stone");
    let g = add(&mut w, MobType::IronGolem, Vec3::new(11.0, 6.0, 11.0), true);
    for _ in 0..20 {
        w.tick();
    }
    let gi = crate::entity::index_of(&w, g).unwrap();
    assert!((w.entities.list[gi].pos.y - 4.0).abs() < 1.0e-9, "the golem stands on the stone under the signs at {}", w.entities.list[gi].pos.y);
}

/// An open fence gate holds no fluid (26.3 `FlowingFluid.canHoldAnyFluid`: neither a
/// `LiquidBlockContainer` nor washed away): water stops at it, lava cannot flow into it, and a
/// golem walks through it (no collision box when open).
/// A golem whose box overlaps a floor by 0.0005 stands on it (collisions shrink a box by only
/// 1e-7), and one 0.0005 short of it falls. The game's golems stayed on the rim of a hole that way
/// for thousands of ticks (the old L1, 2026-11-08), where the current no longer reached their fluid
/// box (deflated by 0.001): the exit flood starts golems there.
#[test]
fn a_golem_stands_on_a_sliver_of_floor() {
    for (x, stays) in [(3.3005, true), (3.2995, false)] {
        let mut w = world(3);
        for z in 4..=7 {
            put(&mut w, 4, -4, z, "minecraft:stone");
        }
        put(&mut w, 0, -20, 0, "minecraft:stone");
        let g = add(&mut w, MobType::IronGolem, Vec3::new(x, -3.0, 6.0), true);
        for _ in 0..40 {
            w.tick();
        }
        let gi = crate::entity::index_of(&w, g).unwrap();
        let y = w.entities.list[gi].pos.y;
        assert_eq!(y == -3.0, stays, "golem at x {x}: y {y}");
    }
}

#[test]
fn open_fence_gates_stop_fluids_not_golems() {
    let mut w = world(3);
    let gate = "minecraft:oak_fence_gate[facing=north,in_wall=false,open=true,powered=false]";
    for x in 0..4 {
        put(&mut w, x, -1, 0, "minecraft:stone");
        put(&mut w, x, 0, -1, "minecraft:stone");
        put(&mut w, x, 0, 1, "minecraft:stone");
    }
    put(&mut w, -1, 0, 0, "minecraft:stone");
    put(&mut w, 4, 0, 0, gate);
    w.set_block_and_update(BlockPos::new(0, 0, 0), Block::parse("minecraft:water[level=0]").unwrap());
    for _ in 0..100 {
        w.tick();
    }
    assert!(matches!(w.get(BlockPos::new(3, 0, 0)), Block::Water { .. }), "the water reached the gate's cell");
    assert!(matches!(w.get(BlockPos::new(4, 0, 0)), Block::FenceGate { .. }), "the water entered the gate: {}", w.get(BlockPos::new(4, 0, 0)));
    // lava over a gate, walled on its sides, cannot flow
    put(&mut w, 10, 4, 10, gate);
    for (x, z) in [(9, 10), (11, 10), (10, 9), (10, 11)] {
        put(&mut w, x, 5, z, "minecraft:stone");
    }
    put(&mut w, 10, 5, 10, "minecraft:lava[level=0]");
    assert_eq!(super::blocks::lava_can_flow(&w, BlockPos::new(10, 5, 10)), None);
    // a golem falls through a 2x2 of open gates onto the stone under them
    for (x, z) in [(14, 14), (15, 14), (14, 15), (15, 15)] {
        put(&mut w, x, 4, z, gate);
        put(&mut w, x, 2, z, "minecraft:stone");
    }
    let g = add(&mut w, MobType::IronGolem, Vec3::new(15.0, 6.0, 15.0), true);
    for _ in 0..30 {
        w.tick();
    }
    let gi = crate::entity::index_of(&w, g).unwrap();
    assert!((w.entities.list[gi].pos.y - 3.0).abs() < 1.0e-9, "the golem stands on the stone under the gates at {}", w.entities.list[gi].pos.y);
}

/// The daylight detector follows the mob layer's day time: an inverted one reads 0 by day and
/// the sky's darkening (11 at midnight under the open sky) by night.
#[test]
fn daylight_detector_follows_the_day() {
    let mut w = World::new(BlockPos::new(-4, -4, -4), [8, 8, 8]);
    w.mob.seed(1, 6000 - 100);
    w.set_block_and_update(BlockPos::new(0, 0, 0), Block::parse("minecraft:daylight_detector[inverted=true,power=0]").unwrap());
    for _ in 0..140 {
        w.tick();
    }
    assert!(matches!(w.get(BlockPos::new(0, 0, 0)), Block::DaylightDetector { power: 0, .. }), "noon: {}", w.get(BlockPos::new(0, 0, 0)));
    for _ in 0..12000 {
        w.tick();
    }
    assert!(matches!(w.get(BlockPos::new(0, 0, 0)), Block::DaylightDetector { power: 11, .. }), "midnight: {}", w.get(BlockPos::new(0, 0, 0)));
}

/// (9fc3155f, ironspec Nov 12 04:32) A pillager is modeled only seated: on its own its spawn is
/// refused; seated in an oak boat it rides at the boat's y + 0.1875 (AbstractBoat's attachment,
/// its height / 3) - 0.6 (its riding offset), with the namespaced id as a design writes it;
/// villagers beyond its crossbow's 8 change nothing, and a villager it sees inside 8 is a model
/// error (its AI would charge and shoot); added unseated it is an error too.
#[test]
fn seated_pillager() {
    let mut w = world(1);
    assert!(matches!(spawn(&mut w, "minecraft:pillager", [0.5, 0.0, 0.5], None), Some(Err(_))));
    for x in -3..=14 {
        put(&mut w, x, -1, 0, "minecraft:glass");
    }
    // (the namespaced id straight into the passenger hook, as G1 asked: no raft seat for a boat)
    let boat = super::spawn_hook(&mut w, "minecraft:oak_boat", [0.5, 0.0, 0.5], Some("{Passengers:[{id:\"minecraft:pillager\",PersistenceRequired:1b}]}"));
    assert!(matches!(boat, Some(Ok(_))), "{boat:?}");
    let by = w.entities.list.iter().find(|e| matches!(&e.kind, EntityKind::Boat(_))).map(|e| e.pos.y).expect("the boat");
    let p = w.entities.list.iter().find(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Pillager)).expect("the pillager");
    assert!(p.vehicle.is_some(), "the pillager rides its boat");
    assert!((p.pos.y - (by + 0.1875 - 0.6)).abs() < 1e-6, "seat: pillager y {} on a boat at {by} (expected boat y - 0.4125)", p.pos.y);
    add(&mut w, MobType::Villager, Vec3::new(10.5, 0.0, 0.5), true);
    for _ in 0..40 {
        w.tick();
    }
    assert!(!w.errors.iter().any(|e| e.contains("pillager")), "a villager 10 off: {:?}", w.errors);
    add(&mut w, MobType::Villager, Vec3::new(5.5, 0.0, 0.5), true);
    for _ in 0..5 {
        w.tick();
    }
    assert!(w.errors.iter().any(|e| e.contains("seated pillager")), "a villager 5 off: {:?}", w.errors);
    // (an unseated one, added past the spawn's refusal: an error at its first tick)
    let mut w2 = world(2);
    put(&mut w2, 0, -1, 0, "minecraft:glass");
    add(&mut w2, MobType::Pillager, Vec3::new(0.5, 0.0, 0.5), true);
    w2.tick();
    assert!(w2.errors.iter().any(|e| e.contains("rides no vehicle")), "unseated: {:?}", w2.errors);
}

/// (9fc3155f, G1 on ff000679f) An iron golem targets a seated pillager it sees within its 16
/// (IronGolem's NearestAttackableTargetGoal on `Mob` with `Enemy && !Creeper`: the pillager's
/// monster row), the farm's scarer posts' golem question.
#[test]
fn golem_targets_seated_pillager() {
    use super::ai::goal::targets::is_enemy;
    assert!(is_enemy(MobType::Pillager));
    let mut w = world(3);
    w.mob.ai.on = true;
    for x in -3..=14 {
        for z in -3..=3 {
            put(&mut w, x, -1, z, "minecraft:stone");
        }
    }
    let boat = crate::entity::spawn(&mut w, "minecraft:oak_boat", [0.5, 0.0, 0.5], Some("{Passengers:[{id:\"minecraft:pillager\",PersistenceRequired:1b}]}"));
    assert!(boat.is_ok(), "{boat:?}");
    let pid = w.entities.list.iter().find(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Pillager)).map(|e| e.id).expect("the pillager");
    let g = add(&mut w, MobType::IronGolem, Vec3::new(12.5, 0.0, 0.5), true);
    let mut targeted = None;
    for t in 0..200 {
        w.tick();
        let gi = crate::entity::index_of(&w, g).expect("the golem");
        if let EntityKind::Mob(m) = &w.entities.list[gi].kind
            && m.ai.as_deref().and_then(|a| a.target) == Some(pid)
        {
            targeted = Some(t);
            break;
        }
    }
    assert!(targeted.is_some(), "a golem 12 off never targeted the seated pillager in 200 gt");
    // (the negative cases, G1 on a60da59c8: one 18 off, beyond its 16, and one 12 off behind a glass
    // wall it cannot see through, never target it)
    for (gx, wall) in [(18.5, false), (12.5, true)] {
        let mut w = world(4);
        w.mob.ai.on = true;
        for x in -3..=20 {
            for z in -3..=3 {
                put(&mut w, x, -1, z, "minecraft:stone");
            }
        }
        if wall {
            for y in 0..=4 {
                for z in -3..=3 {
                    put(&mut w, 6, y, z, "minecraft:glass");
                }
            }
        }
        let boat = crate::entity::spawn(&mut w, "minecraft:oak_boat", [0.5, 0.0, 0.5], Some("{Passengers:[{id:\"minecraft:pillager\",PersistenceRequired:1b}]}"));
        assert!(boat.is_ok(), "{boat:?}");
        let pid = w.entities.list.iter().find(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Pillager)).map(|e| e.id).expect("the pillager");
        let g = add(&mut w, MobType::IronGolem, Vec3::new(gx, 0.0, 0.5), true);
        // (the positive case's 200 gt, G1 on 255f22a1a: a late acquisition must not pass a short window)
        for _ in 0..200 {
            w.tick();
            let gi = crate::entity::index_of(&w, g).expect("the golem");
            if let EntityKind::Mob(m) = &w.entities.list[gi].kind {
                assert!(m.ai.as_deref().and_then(|a| a.target) != Some(pid), "a golem at x {gx} (wall {wall}) targeted the pillager");
            }
        }
    }
}

/// (9fc3155f, ironspec Nov 12 05:31) The seated pillager's portal exemption: a fresh golem 5 off
/// whose box overlaps a nether portal block, its portal cooldown 0, is no error at the pillager's
/// tick (a pad golem crosses at the start of its second tick: tick-exact's official bridge
/// 6c559d25b, 136 of 136, 0 shots); the same golem with its portal row blocked (no portal in its
/// box), or on the portal with its cooldown above 0 (a lingering golem), is the loud error.
#[test]
fn seated_pillager_golem_on_portal() {
    for case in 0..3 {
        let mut w = world(5);
        for x in -3..=10 {
            for z in -3..=3 {
                put(&mut w, x, -1, z, "minecraft:stone");
            }
        }
        if case != 1 {
            let portal = crate::terrain::land::block_of("minecraft:nether_portal[axis=z]").expect("the portal block");
            for y in 0..=2 {
                for z in -1..=1 {
                    w.set_block(BlockPos::new(6, y, z), portal, flags::RAW);
                }
            }
        }
        let boat = crate::entity::spawn(&mut w, "minecraft:oak_boat", [0.5, 0.0, 0.5], Some("{Passengers:[{id:\"minecraft:pillager\",PersistenceRequired:1b}]}"));
        assert!(boat.is_ok(), "{boat:?}");
        let g = add(&mut w, MobType::IronGolem, Vec3::new(5.5, 0.0, 0.5), true);
        if case == 2 {
            let gi = crate::entity::index_of(&w, g).expect("the golem");
            w.entities.list[gi].portal_cooldown = 300;
        }
        w.tick();
        let err = w.errors.iter().any(|e| e.contains("seated pillager"));
        let what = ["on the portal, cooldown 0", "its portal row blocked", "on the portal, cooldown 300"][case];
        assert_eq!(err, case != 0, "{what}: {:?}", w.errors);
    }
}
