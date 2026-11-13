//! Known answers of the weather model (mobinfra): the cycle's statistics, the darkening, rain at
//! a block, the stationary start.

use super::*;

/// The darkening at noon and midnight in clear weather, rain and thunder (the brief's numbers:
/// rain 3, thunder 5 at noon; the night stays 11).
#[test]
fn darkening_in_rain_and_thunder() {
    assert_eq!(sky_darken_at(6000, 0.0, 0.0), 0);
    assert_eq!(sky_darken_at(6000, 1.0, 0.0), 3);
    assert_eq!(sky_darken_at(6000, 1.0, 1.0), 5);
    assert_eq!(sky_darken_at(18000, 0.0, 0.0), 11);
    assert_eq!(sky_darken_at(18000, 1.0, 0.0), 11);
    assert_eq!(sky_darken_at(18000, 1.0, 1.0), 11);
    // the clear-weather path is daylight.rs's unchanged
    for day in (0..24000).step_by(37) {
        assert_eq!(sky_darken_at(day, 0.0, 0.0), crate::daylight::sky_darken(day), "day {day}");
    }
}

/// The level ramps: 21 ticks of rain make `isRaining` (0.01 a tick, > 0.2), 100 a full level;
/// thunder needs rain.
#[test]
fn levels_ramp_by_a_hundredth() {
    let mut w = Weather { on: true, ..Weather::default() };
    w.command(Command::Thunder(100000));
    let mut first_rain = None;
    let mut first_thunder = None;
    for t in 1..=200 {
        w.advance();
        if first_rain.is_none() && w.is_raining() {
            first_rain = Some(t);
        }
        if first_thunder.is_none() && w.is_thundering() {
            first_thunder = Some(t);
        }
    }
    assert_eq!(w.rain_level, 1.0);
    assert_eq!(w.thunder_level, 1.0);
    let (r, th) = (first_rain.unwrap(), first_thunder.unwrap());
    assert!((20..=22).contains(&r), "raining after {r} ticks");
    assert!((95..=100).contains(&th), "thundering after {th} ticks");
}

/// `weather clear` then: both flags off while it counts down, then rain at once (both timers sit
/// at 1), like the game's `advanceWeatherCycle`.
#[test]
fn clear_command_then_rain() {
    let mut w = Weather { on: true, ..Weather::default() };
    w.command(Command::Clear(50));
    for _ in 0..50 {
        w.advance();
        assert!(!w.data.raining && !w.data.thundering);
    }
    w.advance();
    assert!(w.data.raining && w.data.thundering, "{:?}", w.data);
}

/// The cycle's long-run shares: raining 18001/114002 = 15.79%, the thunder flag 9601/105602 =
/// 9.09%, both 1.44% (independent timers); the stationary start gives the same shares.
#[test]
fn cycle_shares() {
    let mut w = Weather::new(Plan::Cycle, 7);
    let n = 40_000_000u64;
    let (mut rain, mut thunder, mut both, mut storm) = (0u64, 0u64, 0u64, 0u64);
    for _ in 0..n {
        w.advance();
        rain += w.data.raining as u64;
        thunder += w.data.thundering as u64;
        both += (w.data.raining && w.data.thundering) as u64;
        storm += (w.is_raining() && w.is_thundering()) as u64;
    }
    let share = |k: u64| k as f64 / n as f64;
    assert!((share(rain) - 18001.0 / 114002.0).abs() < 0.012, "rain {}", share(rain));
    assert!((share(thunder) - 9601.0 / 105602.0).abs() < 0.01, "thunder {}", share(thunder));
    assert!((share(both) - 0.01435).abs() < 0.004, "both {}", share(both));
    assert!(share(storm) <= share(both));
    // stationary starts: the share of starts raining
    let starts = 20000;
    let raining = (0..starts).filter(|s| Weather::new(Plan::Cycle, *s as i64).data.raining).count();
    let p = raining as f64 / starts as f64;
    assert!((p - 18001.0 / 114002.0).abs() < 0.01, "stationary raining share {p}");
}

/// Rain at a block: raining, the sky seen, nothing motion-blocking above.
#[test]
fn rain_at_a_block() {
    let mut w = World::new(BlockPos::new(0, 0, 0), [4, 8, 4]);
    w.weather = Weather::new(Plan::Cycle, 1);
    w.weather.command(Command::Rain(100000));
    for _ in 0..30 {
        w.tick();
    }
    assert!(w.is_raining());
    let p = BlockPos::new(1, 1, 1);
    assert!(w.is_raining_at(p));
    w.set_block(BlockPos::new(1, 5, 1), Block::Simple(crate::block::Simple::Glass), crate::world::flags::RAW);
    assert!(!w.is_raining_at(p), "glass blocks motion: no rain under it");
    assert!(can_see_sky(&w, p).unwrap(), "but the sky is seen through glass");
    w.set_block(BlockPos::new(1, 5, 1), Block::Air, crate::world::flags::RAW);
    w.set_block(BlockPos::new(1, 5, 1), Block::Simple(crate::block::Simple::Stone), crate::world::flags::RAW);
    assert!(!w.is_raining_at(p));
}

// ------------------------------------------------------------------------------- lightning

use crate::entity::mob::types::MobType;
use crate::entity::{Aabb, EntityKind, Vec3};

fn stormy_world() -> World {
    let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 24, 16]);
    w.mob.seed(3, 6000);
    w.afk = Some(Vec3::new(0.5, -40.0, 0.5));
    w.weather = Weather::new(Plan::Cycle, 3);
    w.weather.command(Command::Thunder(1_000_000));
    for _ in 0..120 {
        w.tick();
    }
    assert!(w.is_raining() && w.is_thundering());
    w
}

fn put(w: &mut World, x: i32, y: i32, z: i32, s: &str) {
    w.set_block(BlockPos::new(x, y, z), Block::parse(s).unwrap(), crate::world::flags::RAW);
}

fn count(w: &World, ty: MobType) -> usize {
    w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == ty)).count()
}

/// A villager under the open sky is a target; under stone it is not; under glass it is picked but
/// no bolt falls (no rain at its block).
#[test]
fn lightning_targets() {
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    let v = crate::entity::mob::add(&mut w, MobType::Villager, Vec3::new(0.5, 2.0, 0.5), true);
    let center = BlockPos::new(1, lightning::WORLD_MAX_Y.min(motion_blocking_height(&w, 1, 1)), 1);
    let c = lightning::entity_candidates(&w, center);
    assert_eq!(c.len(), 1, "the villager sees the sky");
    put(&mut w, 0, 6, 0, "minecraft:glass");
    assert_eq!(lightning::entity_candidates(&w, center).len(), 1, "glass keeps sky light 15");
    let i = crate::entity::index_of(&w, v).unwrap();
    let pos = w.entities.list[i].block_position();
    assert!(!w.is_raining_at(pos), "but no rain under glass: no bolt");
    put(&mut w, 0, 6, 0, "minecraft:stone");
    assert!(lightning::entity_candidates(&w, center).is_empty());
}

/// The candidates come in `getEntitiesOfClass`'s order: by entity section key (x, then z, then y),
/// each section's mobs in the order they entered it, not in the level's entity order; a player
/// there (the AFK player) is a candidate too, first in its section.
#[test]
fn lightning_targets_in_section_order() {
    let mut w = stormy_world();
    // three villagers across the x = -1 / 0 section boundary, added east first
    let east = crate::entity::mob::add(&mut w, MobType::Villager, Vec3::new(1.5, 1.0, 0.5), true);
    let west = crate::entity::mob::add(&mut w, MobType::Villager, Vec3::new(-0.5, 1.0, 0.5), true);
    let east2 = crate::entity::mob::add(&mut w, MobType::Villager, Vec3::new(2.5, 1.0, 0.5), true);
    let center = BlockPos::new(0, motion_blocking_height(&w, 0, 0), 0);
    let ids: Vec<u32> = lightning::entity_candidates(&w, center).into_iter().map(|i| w.entities.list[i].id).collect();
    assert_eq!(ids, vec![west, east, east2], "section x = -1 first, then section 0 in entering order");
    w.mob.ai.players.push(crate::entity::mob::ai::Player { pos: Vec3::new(1.5, 1.0, 1.5), mode: crate::entity::mob::ai::GameMode::Creative, invulnerable: false });
    let t: Vec<lightning::Target> = lightning::targets(&w, center).into_iter().map(|(_, t)| t).collect();
    let player = lightning::Target::Entity(crate::entity::mob::ai::PLAYER_ID_BASE);
    assert_eq!(t, vec![lightning::Target::Entity(west), player, lightning::Target::Entity(east), lightning::Target::Entity(east2)]);
}

/// A rod that tops its column takes the strike (one above it); one under a block does not count.
#[test]
fn lightning_trap_horse() {
    // in a mob world a trap brings a skeleton horse to the strike's block corner, before the bolt;
    // nobody keeps it: discarded once its trapTime passes 18000
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    w.mob.natural = Some(crate::entity::mob::natural::Natural::new(vec![Vec3::new(0.5, -40.0, 0.5)], crate::entity::mob::natural::Difficulty::Hard));
    let horse = |w: &World| w.entities.list.iter().position(|e| !e.removed && matches!(&e.kind, crate::entity::EntityKind::Mob(m) if m.ty.def().id == "skeleton_horse"));
    let mut tries = 0;
    while horse(&w).is_none() && tries < 5000 {
        lightning::strike(&mut w, 0, 0);
        tries += 1;
    }
    let i = horse(&w).expect("a trap within 5000 strikes");
    // the earlier strikes' bolts and fires (the trap's own bolt is visual only and lights none)
    w.weather.bolts.clear();
    for x in -1..=1 {
        for z in -1..=1 {
            if matches!(w.get(BlockPos::new(x, 2, z)), Block::Fire { .. }) {
                put(&mut w, x, 2, z, "minecraft:air");
            }
        }
    }
    let at = w.weather.events.iter().rev().find_map(|e| if let WeatherEvent::TrapHorse { at, .. } = e { Some(*at) } else { None }).unwrap();
    let e = &w.entities.list[i];
    assert_eq!([e.pos.x, e.pos.y, e.pos.z], [at[0] as f64, at[1] as f64, at[2] as f64]);
    assert!(matches!(&e.kind, crate::entity::EntityKind::Mob(m) if m.trap_time == Some(0)));
    let id = e.id;
    for k in 0..17_990 {
        w.tick();
        if crate::entity::index_of(&w, id).is_none_or(|j| w.entities.list[j].removed) {
            let nat: Vec<String> = w.mob.natural.as_ref().map(|n| n.log.iter().rev().take(3).map(|e| format!("{e:?}")).collect()).unwrap_or_default();
            let ev: Vec<String> = w.mob.events.iter().rev().take(3).map(|e| format!("{e:?}")).collect();
            panic!("gone at tick {k}: natural {nat:?} mob {ev:?} errors {:?}", w.errors);
        }
    }
    assert!(crate::entity::index_of(&w, id).is_some_and(|j| !w.entities.list[j].removed), "still there before 18000 gt");
    for _ in 0..12 {
        w.tick();
    }
    assert!(crate::entity::index_of(&w, id).is_none_or(|j| w.entities.list[j].removed), "discarded after 18000 gt");
}

/// (mob-AI 2026-11-12) Pig.thunderHit: struck (not Peaceful), a pig becomes a persistent
/// zombified piglin where it stood, holding a golden sword (a golden spear 1 in 20: the pig's own
/// random), and the pig is gone (no fire, no hurt: the conversion replaces Entity.thunderHit).
#[test]
fn lightning_turns_a_pig_into_a_zombified_piglin() {
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    let pig = crate::entity::mob::add(&mut w, MobType::from_id("pig").unwrap(), Vec3::new(0.5, 2.0, 0.5), false);
    let zp = |w: &World| w.entities.list.iter().position(|e| !e.removed && matches!(&e.kind, crate::entity::EntityKind::Mob(m) if m.ty.def().id == "zombified_piglin"));
    let mut tries = 0;
    while zp(&w).is_none() && tries < 200 {
        lightning::strike(&mut w, 0, 0);
        w.tick();
        tries += 1;
    }
    let j = zp(&w).expect("a struck pig within 200 strikes");
    assert!(crate::entity::index_of(&w, pig).is_none_or(|i| w.entities.list[i].removed), "the pig is gone");
    let e = &w.entities.list[j];
    assert_eq!((e.pos.x, e.pos.z), (0.5, 0.5));
    let crate::entity::EntityKind::Mob(m) = &e.kind else { unreachable!() };
    assert!(m.persistence_required);
    let held = m.mainhand.as_ref().map(|h| h.item.as_str());
    assert!(matches!(held, Some("minecraft:golden_sword" | "minecraft:golden_spear")), "{held:?}");
}

/// A rod that tops its column takes the strike (one above it); one under a block does not count.
#[test]
fn lightning_rods() {
    let mut w = stormy_world();
    put(&mut w, 5, 10, 5, "minecraft:stone");
    put(&mut w, 5, 11, 5, "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]");
    assert_eq!(lightning::rod_targets(&w, BlockPos::new(-7, 1, -7)), vec![BlockPos::new(5, 12, 5)]);
    put(&mut w, 5, 12, 5, "minecraft:stone");
    assert!(lightning::rod_targets(&w, BlockPos::new(-7, 1, -7)).is_empty());
    put(&mut w, 5, 12, 5, "minecraft:air");
    // a strike anywhere goes to the rod, which powers for 8 gt
    lightning::strike(&mut w, -7, -7);
    let at = w.weather.events.iter().rev().find_map(|e| if let WeatherEvent::Strike { at, .. } = e { Some(*at) } else { None });
    assert_eq!(at, Some([5, 12, 5]));
    w.tick();
    assert!(matches!(w.get(BlockPos::new(5, 11, 5)), Block::LightningRod { powered: true, .. }));
    for _ in 0..10 {
        w.tick();
    }
    assert!(matches!(w.get(BlockPos::new(5, 11, 5)), Block::LightningRod { powered: false, .. }));
}

/// The bolt hits through a roof: a villager 1 block under a stone roof struck on the roof becomes
/// a witch; a boat in the box is destroyed; a roof 5 above the villager's floor keeps it safe.
#[test]
fn lightning_hits_through_roofs() {
    let mut w = stormy_world();
    // floor at y 1, villager at y 2 (head 3.95), roof at y 4 (bolt at 5, box down to 2)
    put(&mut w, 0, 1, 0, "minecraft:stone");
    put(&mut w, 0, 4, 0, "minecraft:stone");
    crate::entity::mob::add(&mut w, MobType::Villager, Vec3::new(0.5, 2.0, 0.5), true);
    lightning::strike(&mut w, 0, 0);
    w.tick();
    assert_eq!(count(&w, MobType::Villager), 0);
    assert_eq!(count(&w, MobType::Witch), 1);
    assert!(w.weather.events.iter().any(|e| matches!(e, WeatherEvent::Converted { .. })));
    // safe under a roof 5 above the floor: the box reaches down to roof - 2
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    put(&mut w, 0, 6, 0, "minecraft:stone");
    crate::entity::mob::add(&mut w, MobType::Villager, Vec3::new(0.5, 2.0, 0.5), true);
    lightning::strike(&mut w, 0, 0);
    for _ in 0..40 {
        w.tick();
    }
    assert_eq!(count(&w, MobType::Villager), 1);
    // a boat in the box
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    crate::entity::spawn(&mut w, "minecraft:oak_boat", [2.5, 2.0, 0.5], None).unwrap();
    lightning::strike(&mut w, 0, 0);
    w.tick();
    assert!(w.weather.events.iter().any(|e| matches!(e, WeatherEvent::VehicleDestroyed { .. })));
}

/// Fires: a bolt on stone sets a fire (a block) that goes out in the rain or with age; next to
/// leaves too.
#[test]
fn lightning_fires() {
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    lightning::strike(&mut w, 0, 0);
    w.tick();
    assert!(w.weather.events.iter().any(|e| matches!(e, WeatherEvent::Fire { at: [0, 2, 0], .. })), "{:?}", w.weather.events);
    assert!(matches!(w.get(BlockPos::new(0, 2, 0)), Block::Fire { soul: false, .. }));
    for _ in 0..2000 {
        w.tick();
    }
    assert!(!matches!(w.get(BlockPos::new(0, 2, 0)), Block::Fire { .. }), "a fire with nothing to burn goes out");
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    put(&mut w, 1, 2, 0, "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]");
    lightning::strike(&mut w, 0, 0);
    w.tick();
    assert!(w.weather.events.iter().any(|e| matches!(e, WeatherEvent::Fire { .. })));
}

/// Fire as a block (`FireBlock`): on stone with nothing to burn it goes out (age over 3); beside
/// leaves, with a player near and no rain, it burns them (`checkBurnOut`); with no player its
/// tick does nothing (`canSpreadFireAround`). Lava lights the air beside planks
/// (`LavaFluid.randomTick`) only with a player within 128.
#[test]
fn fire_burns_and_lava_lights_it() {
    let near = Some(Vec3::new(0.5, 1.0, 0.5));
    let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 24, 16]);
    w.afk = near;
    put(&mut w, 0, 1, 0, "minecraft:stone");
    assert!(fire::place_fire(&mut w, BlockPos::new(0, 2, 0)));
    for _ in 0..2000 {
        w.tick();
    }
    assert!(!matches!(w.get(BlockPos::new(0, 2, 0)), Block::Fire { .. }), "nothing to burn: it goes out");
    put(&mut w, 1, 2, 0, "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]");
    assert!(fire::place_fire(&mut w, BlockPos::new(0, 2, 0)));
    for _ in 0..6000 {
        w.tick();
        if !matches!(w.get(BlockPos::new(1, 2, 0)), Block::Leaves { .. }) {
            break;
        }
    }
    assert!(!matches!(w.get(BlockPos::new(1, 2, 0)), Block::Leaves { .. }), "the leaves burned");
    assert!(w.weather.events.iter().any(|e| matches!(e, WeatherEvent::Burned { .. })), "{:?}", w.weather.events);
    // no player: the fire stays as it is, its leaves too
    let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 24, 16]);
    put(&mut w, 0, 1, 0, "minecraft:stone");
    put(&mut w, 1, 2, 0, "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]");
    assert!(fire::place_fire(&mut w, BlockPos::new(0, 2, 0)));
    for _ in 0..3000 {
        w.tick();
    }
    assert!(matches!(w.get(BlockPos::new(1, 2, 0)), Block::Leaves { .. }));
    assert!(matches!(w.get(BlockPos::new(0, 2, 0)), Block::Fire { age: 0, .. }));
    // lava beside planks: a fire within a few hundred random ticks, none without a player
    for afk in [near, None] {
        let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 24, 16]);
        w.afk = afk;
        put(&mut w, 0, 1, 0, "minecraft:lava");
        put(&mut w, 1, 1, 0, "minecraft:oak_planks");
        let mut lit = false;
        for _ in 0..400 {
            fire::lava_random_tick(&mut w, BlockPos::new(0, 1, 0));
            lit = (-1..=1).any(|x| (1..=3).any(|y| (-1..=1).any(|z| matches!(w.get(BlockPos::new(x, y, z)), Block::Fire { .. }))));
            if lit {
                break;
            }
        }
        assert_eq!(lit, afk.is_some());
    }
}

/// Rain puts a burning mob out; a burnt-out mob goes back to 0 (`-getFireImmuneTicks()` in 26.3:
/// a bolt's +1 then burns it for a tick, not 8 s).
#[test]
fn rain_puts_fire_out() {
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    let g = crate::entity::mob::add(&mut w, MobType::IronGolem, Vec3::new(0.5, 2.0, 0.5), true);
    let i = crate::entity::index_of(&w, g).unwrap();
    crate::entity::mob::mob_mut(&mut w, i).fire_ticks = 300;
    w.tick();
    let i = crate::entity::index_of(&w, g).unwrap();
    assert_eq!(crate::entity::mob::mob(&w, i).fire_ticks, 0);
    assert!(w.weather.events.iter().any(|e| matches!(e, WeatherEvent::RainedOut { .. })));
}

/// The exhaustive check: a villager on a stone floor under the open sky is hit by the bolts aimed
/// at it and at the columns around it; a stone roof 6 above its floor keeps every column-top bolt
/// out of its cell (the box reaches 3 down); a lightning rod topping a column far off takes
/// every strike and nothing is hit.
#[test]
fn every_bolt_that_can_fall() {
    let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 24, 16]);
    w.mob.seed(3, 6000);
    put(&mut w, 0, 1, 0, "minecraft:stone");
    crate::entity::mob::add(&mut w, MobType::Villager, Vec3::new(0.5, 2.0, 0.5), true);
    let inner = Aabb::new(-8.0, 0.0, -8.0, 8.0, 24.0, 8.0);
    let open = lightning::static_check(&w, &[], &[], inner);
    assert!(open.iter().any(|m| m.contains("aims at a villager") && m.contains("hits the villager at")), "{open:?}");
    for x in -2..=2 {
        for z in -2..=2 {
            put(&mut w, x, 7, z, "minecraft:stone");
        }
    }
    let roofed = lightning::static_check(&w, &[], &[], inner);
    assert!(!roofed.iter().any(|m| m.contains("hits the villager")), "{roofed:?}");
    // the roof's own top is on the farm: a trap horse can land there
    assert!(roofed.iter().any(|m| m.contains("trap horse")), "{roofed:?}");
    put(&mut w, 7, 20, 7, "minecraft:stone");
    put(&mut w, 7, 21, 7, "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]");
    let rod = lightning::static_check(&w, &[], &[], Aabb::new(-3.0, 0.0, -3.0, 3.0, 9.0, 3.0));
    assert!(rod.is_empty(), "{rod:?}");
}

/// The card's lightning numbers (vjeux 2026-11-08: tradeoffs, measured): a villager in the open
/// is struck by every strike in the 7 x 7 columns around it (1e-5 / 256 each a tick of thunder),
/// and the iron in a cell beside it burns with it; under a stone roof 5 above its feet, none.
#[test]
fn lightning_measure() {
    let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 24, 16]);
    w.mob.seed(3, 6000);
    put(&mut w, 0, 1, 0, "minecraft:stone");
    crate::entity::mob::add(&mut w, MobType::Villager, Vec3::new(0.5, 2.0, 0.5), true);
    let inner = Aabb::new(-8.0, 0.0, -8.0, 8.0, 24.0, 8.0);
    let farm = (BlockPos::new(-8, 0, -8), BlockPos::new(7, 23, 7));
    let q = lightning::CHUNK_STRIKE / 256.0;
    let open = lightning::measure(&w, &[], &[(BlockPos::new(2, 2, 0), 2.0)], inner, farm).unwrap();
    // (iteration 23's G1: a bolt that brings a trap horse is visual only, so the villager's 49
    // columns harm it at 1 - the trap chance each)
    let harm = 1.0 - lightning::trap_chance(&lightning::Settings::default());
    assert!((open.witch / q - 49.0 * harm).abs() < 1e-6, "{open:?}");
    assert!((open.iron / q - 98.0 * harm).abs() < 1e-6, "{open:?}");
    assert_eq!(open.vehicle, 0.0);
    assert!((open.breaks - open.witch).abs() < 1e-12);
    for x in -2..=2 {
        for z in -2..=2 {
            put(&mut w, x, 7, z, "minecraft:stone");
        }
    }
    let roofed = lightning::measure(&w, &[], &[], inner, farm).unwrap();
    assert_eq!(roofed.witch, 0.0, "{roofed:?}");
    // the roof's top is on the farm: a trap horse can land there (6.4% of those bolts on Hard)
    assert!((roofed.trap / q - 25.0 * lightning::trap_chance(&lightning::Settings::default())).abs() < 1e-6, "{roofed:?}");
}

/// A bolt's fire breaks the farm beside the farm's own flammable blocks (its box: its blocks and
/// the natural ones its walls are made of), not beside a terrain sample's grass outside it (the
/// day-1 lane: D1U's card read the surface grass over the dug-in farm as "a fire every 7 days").
#[test]
fn lightning_fire_counts_the_farms_blocks() {
    let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 24, 16]);
    put(&mut w, 0, 1, 0, "minecraft:oak_planks");
    let inner = Aabb::new(-8.0, 0.0, -8.0, 8.0, 24.0, 8.0);
    let mine = lightning::measure(&w, &[], &[], inner, (BlockPos::new(-8, 0, -8), BlockPos::new(7, 23, 7))).unwrap();
    assert!(mine.fire > 0.0, "{mine:?}");
    let elsewhere = lightning::measure(&w, &[], &[], inner, (BlockPos::new(4, 0, 4), BlockPos::new(7, 23, 7))).unwrap();
    assert_eq!(elsewhere.fire, 0.0, "{elsewhere:?}");
    // (ironspec 2026-11-11, the fire rules) the planks outside the farm's box burn the site, not
    // the farm: a site note
    assert!(elsewhere.site_fire > 0.0, "{elsewhere:?}");
}

/// ironspec 2026-11-11's fire rules: a fire reaches the farm along the game's spread, through
/// natural flammables as well as built ones, and the storm's rain stops what the game's rain
/// stops (a bolt falls only in rain: `FireBlock.tick` lights no cell near rain, and a burnt block
/// turns to fire only where it is not raining at its own cell (`!isRainingAt`, checked while it
/// stands: a solid block holds the rain off its own cell, a grass tuft does not)). A natural fuse
/// leads from a fire to the farm's own planks five blocks away: of grass, it carries the fire under
/// a roof and not in the open; of planks, in the open too; without a fuse, nowhere.
#[test]
fn fire_reach_follows_the_spread_and_stops_in_the_rain() {
    let grass = Block::Terrain(crate::terrain::blocks::id_of("minecraft:short_grass").expect("short grass in the table"));
    let scene = |roof: bool, fuse: Option<&str>| {
        let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 12, 16]);
        for x in -8..8 {
            for z in -8..8 {
                put(&mut w, x, 0, z, "minecraft:stone");
            }
        }
        put(&mut w, 6, 1, 1, "minecraft:oak_planks");
        if roof {
            for x in -2..=7 {
                for z in -3..=4 {
                    put(&mut w, x, 3, z, "minecraft:stone");
                }
            }
        }
        // (the fuse last, the grass placed raw: on stone it could not stay)
        for x in 1..=5 {
            match fuse {
                Some("grass") => w.place_raw(&[(BlockPos::new(x, 1, 1), grass)]),
                Some(s) => put(&mut w, x, 1, 1, s),
                None => {}
            }
        }
        w
    };
    {
        let w = scene(true, Some("grass"));
        assert!((1..=5).all(|x| w.get(BlockPos::new(x, 1, 1)) == grass), "the grass fuse stands");
    }
    let own = [BlockPos::new(6, 1, 1)];
    let start = BlockPos::new(1, 1, 0);
    assert!(lightning::fire_can_reach(&scene(true, Some("grass")), &own, &[], start), "under a roof the fire follows the grass to the farm");
    assert!(!lightning::fire_can_reach(&scene(false, Some("grass")), &own, &[], start), "in the rain a grass fuse carries nothing");
    assert!(lightning::fire_can_reach(&scene(false, Some("minecraft:oak_planks")), &own, &[], start), "a fuse of planks carries it in the rain");
    assert!(!lightning::fire_can_reach(&scene(true, None), &own, &[], start), "no fuse: the fire goes nowhere");
    // a fire beside the farm's own planks burns them, rain or not; one in a cell the farm depends
    // on (a mob's, the drop path, the AFK spot) reaches it at once
    assert!(lightning::fire_can_reach(&scene(false, None), &own, &[], BlockPos::new(5, 1, 1)));
    assert!(lightning::fire_can_reach(&scene(false, None), &[], &[BlockPos::new(-3, 1, -3)], BlockPos::new(-3, 1, -3)));
}

/// ironspec 2026-11-11: a strike on the AFK player is its own line (`afk_struck`), not the farm
/// breaking. A bolt hurts every entity in x and z +-3, y -3..+9 of its strike point, so a block
/// over the player's column alone does not shelter it (the strikes on that block land 3 above it,
/// the ground's around it within 3); an opaque 7 x 7 roof over its column, its block 4 above the
/// player's feet, does. The player also draws the strikes near it (a living entity that sees the
/// sky is a target).
#[test]
fn the_afk_player_struck_and_its_roof() {
    let world = |roof: Option<(i32, i32)>| {
        let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 24, 16]);
        for x in -8..8 {
            for z in -8..8 {
                put(&mut w, x, 0, z, "minecraft:stone");
            }
        }
        if let Some((half, y)) = roof {
            for x in -half..=half {
                for z in -half..=half {
                    put(&mut w, x, y, z, "minecraft:stone");
                }
            }
        }
        w
    };
    let inner = Aabb::new(-8.0, 0.0, -8.0, 8.0, 24.0, 8.0);
    let farm = (BlockPos::new(-8, 0, -8), BlockPos::new(7, 23, 7));
    let player = Some(Vec3::new(0.5, 1.0, 0.5));
    // (the player standing there: a terrain site's AFK spot)
    let at = |w: &World| lightning::measure_own(w, &[], &[], inner, player, player, &[]).unwrap();
    let open = at(&world(None));
    let column = at(&world(Some((0, 3))));
    let roofed = at(&world(Some((3, 5))));
    // (a void build's convention spot is no target: no struck line)
    assert_eq!(lightning::measure_at(&world(None), &[], &[], inner, farm, player).unwrap().afk_struck, 0.0);
    assert!(open.afk_struck > 0.0 && open.breaks == 0.0, "{open:?}");
    assert!(column.afk_struck > 0.0, "a block over its column alone: {column:?}");
    assert_eq!(roofed.afk_struck, 0.0, "{roofed:?}");
    // (49 columns' strikes within 3 of it: every one reaches it in the open)
    let q = lightning::CHUNK_STRIKE / 256.0;
    let harm = 1.0 - lightning::trap_chance(&lightning::Settings::default());
    assert!((open.afk_struck / q - 49.0 * harm).abs() < 1e-6, "{open:?}");
}

/// Java's Float.toString for the bridge's weather text.
#[test]
fn java_float_text() {
    for (x, s) in [(0.0f32, "0.0"), (1.0, "1.0"), (0.5, "0.5"), (0.01, "0.01"), (0.29999998, "0.29999998"), (7.450581e-9, "7.450581E-9"), (1.0e7, "1.0E7"), (0.001, "0.001"), (0.000_999, "9.99E-4")] {
        assert_eq!(java_float(x), s, "{x}");
    }
}

/// Mirroring the bridge (the level random and its chunk order), the cycle draws from the level
/// random in the game's order (thunder first, then rain: `UniformInt.sample`); the bridge's text
/// of it.
#[test]
fn weather_on_the_level_random() {
    let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 16, 16]);
    w.mirror_level_random(7, [-1, -1, 0, 0]);
    w.weather = Weather::new(Plan::Cycle, 3);
    w.weather.data = WeatherData { clear_weather_time: 0, rain_time: 0, thunder_time: 0, raining: false, thundering: false };
    (w.weather.o_rain_level, w.weather.rain_level, w.weather.o_thunder_level, w.weather.thunder_level) = (0.0, 0.0, 0.0, 0.0);
    let mut r = crate::rng::LegacyRandom::new(7);
    let thunder = uniform(&mut r, THUNDER_DELAY);
    let rain = uniform(&mut r, RAIN_DELAY);
    tick(&mut w);
    assert_eq!((w.weather.data.thunder_time, w.weather.data.rain_time), (thunder, rain));
    assert_eq!(w.level_random.as_ref().unwrap().state(), r.state());
    assert_eq!(w.weather.bridge_text(), format!("clear 0 rain {rain} thunder {thunder} levels 0.0 0.0"));
    w.weather.command(Command::Rain(6000));
    tick(&mut w);
    assert!(w.weather.bridge_text().contains(" raining levels 0.01 0.0"), "{}", w.weather.bridge_text());
}

/// The cycle's thunderstorms: rain about 15.8% of the time and the thunder flag 9.1%, independent,
/// so about 1.4% (a little less: the levels ramp); a storm lasts a few thousand gt.
#[test]
fn thunderstorm_share() {
    let s = storm_stats(1, 20_000);
    assert!((0.010..0.018).contains(&s.share), "{s:?}");
    assert!((2000.0..12000.0).contains(&s.mean_len), "{s:?}");
}

/// Natural states (the terrain lane's table) burn with FireBlock's own odds and block motion by
/// the table's flags.
#[test]
fn natural_states_burn_and_block() {
    let id = |s: &str| crate::terrain::blocks::id_of(s).unwrap_or_else(|| panic!("{s} not in the table"));
    let leaves = Block::Terrain(id("minecraft:oak_leaves[distance=7,persistent=false,waterlogged=false]"));
    assert_eq!(lightning::flammable(&leaves), Some((30, 60)));
    assert!(blocks_motion(&leaves) && motion_blocking(&leaves));
    let stone = Block::Terrain(id("minecraft:stone"));
    assert_eq!(lightning::flammable(&stone), None);
    assert!(blocks_motion(&stone));
    let grass = Block::Terrain(id("minecraft:short_grass"));
    assert!(lightning::flammable(&grass).is_some());
    assert!(!blocks_motion(&grass) && !motion_blocking(&grass));
    // lava lights a fire beside the blocks the table says lava ignites
    assert!(fire::ignited_by_lava(&leaves) && !fire::ignited_by_lava(&stone));
}

/// (cc767b78, G1 14:55 H2) The lightning statics fail closed on an unreadable sky: a land-less farm
/// with a brewing stand (a block whose light behaviour isn't modeled) over a golem's cell fails
/// `check_farm` with a light read failure, and `measure_farm` refuses with an error; the same farm
/// without the stand reads its sky and the measure goes through.
#[test]
fn the_lightning_statics_fail_closed_on_an_unreadable_sky() {
    use crate::entity::mob::farm::{FarmSpec, RunReport};
    let stone = Block::parse("minecraft:stone").unwrap();
    let stand = Block::parse("minecraft:brewing_stand[has_bottle_0=false,has_bottle_1=false,has_bottle_2=false]").unwrap();
    let spec = |with_stand: bool| {
        let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
        for x in -2..=2 {
            for z in -2..=2 {
                blocks.push((BlockPos::new(x, 0, z), stone));
            }
        }
        if with_stand {
            blocks.push((BlockPos::new(0, 4, 0), stand));
        }
        FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(-2, 1, -2), lo: BlockPos::new(-4, -2, -4), hi: BlockPos::new(4, 6, 4), block_entities: Vec::new(), rock: false, nether: None, land: false }
    };
    let runs = vec![RunReport { golem_cells: vec![([0, 1, 0], 10)], cell_ticks: 100, ..RunReport::default() }];
    let fails = lightning::check_farm(&spec(true), &runs);
    assert!(fails.iter().any(|f| f.contains("brewing_stand")), "{fails:?}");
    let m = lightning::measure_farm(&spec(true), &runs);
    assert!(m.as_ref().is_err_and(|e| e.contains("brewing_stand")), "{:?}", m.as_ref().err());
    assert!(lightning::measure_farm(&spec(false), &runs).is_ok(), "without the stand the sky reads");
    assert!(!lightning::check_farm(&spec(false), &runs).iter().any(|f| f.contains("light read")), "without the stand: no light failure");
}
