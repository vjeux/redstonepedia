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

/// Rain puts a burning mob out; a burnt-out mob goes back to -1 (a bolt then sets 8 s of fire).
#[test]
fn rain_puts_fire_out() {
    let mut w = stormy_world();
    put(&mut w, 0, 1, 0, "minecraft:stone");
    let g = crate::entity::mob::add(&mut w, MobType::IronGolem, Vec3::new(0.5, 2.0, 0.5), true);
    let i = crate::entity::index_of(&w, g).unwrap();
    crate::entity::mob::mob_mut(&mut w, i).fire_ticks = 300;
    w.tick();
    let i = crate::entity::index_of(&w, g).unwrap();
    assert_eq!(crate::entity::mob::mob(&w, i).fire_ticks, -1);
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
    let q = lightning::CHUNK_STRIKE / 256.0;
    let open = lightning::measure(&w, &[], &[(BlockPos::new(2, 2, 0), 2.0)], inner);
    assert!((open.witch / q - 49.0).abs() < 1e-6, "{open:?}");
    assert!((open.iron / q - 98.0).abs() < 1e-6, "{open:?}");
    assert_eq!(open.vehicle, 0.0);
    assert!((open.breaks - open.witch).abs() < 1e-12);
    for x in -2..=2 {
        for z in -2..=2 {
            put(&mut w, x, 7, z, "minecraft:stone");
        }
    }
    let roofed = lightning::measure(&w, &[], &[], inner);
    assert_eq!(roofed.witch, 0.0, "{roofed:?}");
    // the roof's top is on the farm: a trap horse can land there (6.4% of those bolts on Hard)
    assert!((roofed.trap / q - 25.0 * lightning::trap_chance(&lightning::Settings::default())).abs() < 1e-6, "{roofed:?}");
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
