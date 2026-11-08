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
