//! (farmopt, Nov 12 2026, ironspec's 19:04 / 19:12 rulings: the way-in's water drag) Waterlogged
//! rails: 26.3's `BaseRailBlock` WATERLOGGED state with OldMinecartBehavior's in-water physics
//! (slide x0.2, the step clamped to 0.2, slowdown x0.95 on top of x0.997 ridden), the fluid's spread
//! out of a waterlogged source, and the walk evaluator's typing (a waterlogged rail stays RAIL).
//! The rides are tick-exact's official ones (`mctas mobai water-rails --slopes 6 --flat 12`,
//! crates/mobai/src/waterrails.rs on explore/tickexact; 26.3, a void world).

use redsim::block::Block;
use redsim::entity::mob::ai::path::path_type::{from_state, PathType};
use redsim::pos::BlockPos;
use redsim::world::{flags, World};

fn set(w: &mut World, p: (i32, i32, i32), s: &str) {
    w.set_block(BlockPos::new(p.0, p.1, p.2), Block::parse(s).unwrap_or_else(|e| panic!("{s}: {e}")), flags::ALL);
}

/// tick-exact's rig at `x` (Y = 100): six slopes ascending south at (x, 100 - i, -i) (the cart rolls
/// north), twelve flats at (x, 94, -6 - j), the stop (x, 94..95, -18), the top end at (x, 101, 1);
/// glass under every rail and walls at x +- 1; `wet` waterlogs every rail. The slopes are set again
/// last (a slope placed before its neighbours reshapes).
fn rig(w: &mut World, x: i32, wet: bool) {
    let slope = format!("rail[shape=ascending_south,waterlogged={wet}]");
    let flat = format!("rail[shape=north_south,waterlogged={wet}]");
    for i in 0..6 {
        set(w, (x, 99 - i, -i), "glass");
        for y in (99 - i)..=(101 - i) {
            set(w, (x - 1, y, -i), "glass");
            set(w, (x + 1, y, -i), "glass");
        }
        set(w, (x, 100 - i, -i), &slope);
    }
    for j in 0..12 {
        let z = -6 - j;
        set(w, (x, 93, z), "glass");
        for y in 93..=95 {
            set(w, (x - 1, y, z), "glass");
            set(w, (x + 1, y, z), "glass");
        }
        set(w, (x, 94, z), &flat);
    }
    set(w, (x, 94, -18), "glass");
    set(w, (x, 95, -18), "glass");
    set(w, (x, 100, 1), "glass");
    for y in 100..=102 {
        set(w, (x - 1, y, 1), "glass");
        set(w, (x + 1, y, 1), "glass");
    }
    set(w, (x, 101, 1), &flat);
    set(w, (x, 101, 2), "glass");
    set(w, (x, 102, 2), "glass");
    for i in 0..6 {
        set(w, (x, 100 - i, -i), &slope);
    }
}

/// The ridden cart's (tick, y - 100, z, vz) from its summon at (x + 0.5, 100.5625, 0.5) on the
/// top slope, Motion 0, a villager riding (tick-exact's rider is NoAI, which redsim doesn't model; a
/// riding villager never steers a minecart, so its brain doesn't touch the ride).
fn ride(wet: bool, ticks: u32) -> Vec<(u32, f64, f64, f64)> {
    let mut w = World::new(BlockPos::new(-4, 88, -22), [12, 18, 28]);
    rig(&mut w, 0, wet);
    let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 100.5625, 0.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}")).unwrap();
    let mut out = Vec::new();
    for t in 1..=ticks {
        w.tick();
        let e = w.entities.list.iter().find(|e| e.id == cart && !e.removed).expect("the cart");
        out.push((t, e.pos.y - 100.0, e.pos.z, e.delta_movement.z));
    }
    out
}

/// tick-exact's official rides, every tick (tests/fixtures/water_rails_official.txt: `mctas mobai
/// water-rails --slopes 6 --flat 12 --ticks 150 --every-tick` on 26.3; artifact 1149557520931066):
/// (dry, wet), each row (t, y - 100, z, vz).
fn official() -> (Vec<(u32, f64, f64, f64)>, Vec<(u32, f64, f64, f64)>) {
    let text = include_str!("fixtures/water_rails_official.txt");
    let (mut dry, mut wet) = (Vec::new(), Vec::new());
    let mut in_wet = false;
    for l in text.lines() {
        if l.trim_start().starts_with("WATERLOGGED") {
            in_wet = true;
            continue;
        }
        let f: Vec<&str> = l.split(',').map(|s| s.trim()).collect();
        let Some(Ok(t)) = f.first().map(|s| s.parse::<u32>()) else { continue };
        let n = |k: usize| f[k].parse::<f64>().unwrap_or_else(|e| panic!("{l}: {e}"));
        let row = (t, n(2), n(3), n(6));
        if in_wet {
            wet.push(row);
        } else {
            dry.push(row);
        }
    }
    (dry, wet)
}

#[test]
fn the_dry_and_wet_rides_match_the_official_ones_every_tick() {
    let (odry, owet) = official();
    assert_eq!((odry.len(), owet.len()), (150, 150), "the official table");
    for (what, ours, theirs) in [("dry", ride(false, 150), odry), ("wet", ride(true, 150), owet)] {
        for ((t, y, z, vz), (ot, oy, oz, ovz)) in ours.iter().zip(theirs.iter()) {
            assert_eq!(t, ot);
            assert!(
                (y - oy).abs() <= 0.005 && (z - oz).abs() <= 0.005 && (vz - ovz).abs() <= 0.005,
                "{what} t {t}: ours y {y:.6} z {z:.6} vz {vz:.6}, official y {oy:.6} z {oz:.6} vz {ovz:.6}"
            );
        }
    }
}

fn at(r: &[(u32, f64, f64, f64)], t: u32) -> (f64, f64) {
    let (_, _, z, vz) = r[(t - 1) as usize];
    (z, vz.abs())
}

#[test]
fn a_popped_waterlogged_rail_leaves_its_water_source() {
    // (G1 on 4b6ee86e5) the other half of survival: a rail that can't stand any more is destroyed
    // (Level.destroyBlock / removeBlock keep the cell's fluid: the fluid's legacy block), so a wet
    // rail whose support goes leaves a water source in its cell; a dry one leaves air.
    // (ironspec 20:27 (a), G1 on bdc3b682f) a LEVEL-0 SOURCE on the pop tick itself: the rail pops
    // in the support's own neighbour update, before any tick, and the cell then holds water[level=0]
    for (s, wet) in [
        ("rail[shape=north_south,waterlogged=true]", true),
        ("powered_rail[powered=false,shape=north_south,waterlogged=true]", true),
        ("detector_rail[powered=false,shape=north_south,waterlogged=true]", true),
        ("rail[shape=north_south,waterlogged=false]", false),
    ] {
        let mut w = World::new(BlockPos::new(-3, -3, -3), [7, 7, 7]);
        set(&mut w, (0, 0, 0), "glass");
        set(&mut w, (0, 1, 0), s);
        set(&mut w, (0, 0, 0), "air");
        // the pop tick: the support's update has run, no tick yet
        let got = w.get(BlockPos::new(0, 1, 0));
        assert!(!redsim::rails::is_rail(&got), "{s}: the rail popped on its support's update (got {got})");
        if wet {
            assert_eq!(got, Block::Water { level: 0 }, "{s}: a level-0 water source on the pop tick (got {got})");
            assert!(redsim::fluid::fluid_state(&w, BlockPos::new(0, 1, 0)).is_source(), "{s}: its fluid a source on the pop tick");
        } else {
            assert!(got.is_air(), "{s}: a dry rail leaves air on the pop tick (got {got})");
        }
        for _ in 0..3 {
            w.tick();
        }
        let got = w.get(BlockPos::new(0, 1, 0));
        assert!(!redsim::rails::is_rail(&got), "{s}: the rail popped (got {got})");
        let f = redsim::fluid::fluid_state(&w, BlockPos::new(0, 1, 0));
        if wet {
            assert!(f.fluid.is_water() && f.is_source(), "{s}: its cell still holds a water source 3 ticks on (got {got})");
        } else {
            assert!(!f.fluid.is_water() && got.is_air(), "{s}: a dry rail leaves air (got {got})");
        }
    }
}

/// tick-exact's official restart rows on the wet copy (tests/fixtures/water_rails_restarts_official.txt,
/// re-recorded with `mctas mobai water-rails --restarts`, 0ff8adad0): each row whose cart HELD STILL
/// across the load (game time +1: one idle tick), as (the first tick after the load, every tick's
/// (t, y - 100, z, vz)); the rows a harness artefact MOVED are not citable and left out.
fn official_restarts() -> Vec<(u32, Vec<(u32, f64, f64, f64)>)> {
    let text = include_str!("fixtures/water_rails_restarts_official.txt");
    let mut rows: Vec<(u32, Vec<(u32, f64, f64, f64)>)> = Vec::new();
    let mut cur: Option<(u32, Vec<(u32, f64, f64, f64)>)> = None;
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix("  restart between t") {
            if let Some(r) = cur.take() {
                rows.push(r);
            }
            if !l.contains("held still") {
                continue;
            }
            let first = rest.split(" (t").nth(1).and_then(|s| s.split(' ').next()).and_then(|s| s.parse::<u32>().ok()).unwrap_or_else(|| panic!("{l}"));
            cur = Some((first, Vec::new()));
            continue;
        }
        if l.starts_with("  ") && !l.starts_with("    ") {
            if let Some(r) = cur.take() {
                rows.push(r);
            }
            continue;
        }
        let Some((_, ticks)) = cur.as_mut() else { continue };
        let f: Vec<&str> = l.split(',').map(|s| s.trim()).collect();
        let Some(Ok(t)) = f.first().map(|s| s.parse::<u32>()) else { continue };
        let n = |k: usize| f[k].parse::<f64>().unwrap_or_else(|e| panic!("{l}: {e}"));
        ticks.push((t, n(3), n(4), n(7)));
    }
    if let Some(r) = cur.take() {
        rows.push(r);
    }
    rows
}

/// tick-exact's MIXED rig of the restart rows (`water-rails --both-branches`' wet copy): `rig`'s cells
/// with the six slopes and the top end DRY and the twelve flats waterlogged.
fn rig_mixed(w: &mut World, x: i32) {
    rig(w, x, true);
    for i in 0..6 {
        set(w, (x, 100 - i, -i), "rail[shape=ascending_south,waterlogged=false]");
    }
    set(w, (x, 101, 1), "rail[shape=north_south,waterlogged=false]");
}

#[test]
fn the_restart_rows_match_the_official_ones_every_tick() {
    // (ironspec 20:27 (b), G1 on bdc3b682f) the DRY FIRST TICK after a load: AbstractMinecart.tick runs
    // its behaviour before the fluid update and wasTouchingWater isn't saved, so the first tick after a
    // load moves the wet cart as a dry one. Every held-still official restart row, every tick, on the
    // fixture's mixed rig (dry slopes onto wet flats): the load between t - 1 and t as the official one
    // at a world open with one idle tick (game time +1): every chunk saved, unloaded and loaded again in
    // the save order, then one whole-chunk idle tick
    let rows = official_restarts();
    assert!(rows.len() >= 15, "the official held-still restart rows ({})", rows.len());
    let mut worst = 0.0f64;
    let mut wet_before = 0u32;
    for (first, ticks) in &rows {
        let mut w = World::new(BlockPos::new(-4, 88, -22), [12, 18, 28]);
        rig_mixed(&mut w, 0);
        redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 100.5625, 0.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}")).unwrap();
        for &(t, oy, oz, ovz) in ticks {
            if t == *first {
                // (G1 on aeeaaecf0, note 1) the row tests the dry first tick only if the cart was in
                // water on the tick before the load: counted, and at least one required
                let wet = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).is_some_and(|e| e.was_touching_water);
                wet_before += wet as u32;
                redsim::reload::unload_all(&mut w);
                redsim::reload::load_all_ordered(&mut w, redsim::reload::Order::Save);
                // the world open's one idle tick: game time +1, nothing of the world ticks (the rig
                // has no weather and its day time is never read; `World::tick_time` once main has it)
                w.time += 1;
            }
            w.tick();
            let e = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).expect("the cart");
            let (y, z, vz) = (e.pos.y - 100.0, e.pos.z, e.delta_movement.z);
            let d = (y - oy).abs().max((z - oz).abs()).max((vz - ovz).abs());
            worst = worst.max(d);
            assert!(d <= 0.005, "the restart before t {first}, t {t}: ours y {y:.6} z {z:.6} vz {vz:.6}, official y {oy:.6} z {oz:.6} vz {ovz:.6}");
        }
    }
    eprintln!("{} official restart rows, every tick within {worst:.6}; {wet_before} of them with the cart in water on the tick before the load", rows.len());
    assert!(wet_before >= 1, "no restart row loads a cart that was in water: the dry first tick is untested");
}

#[test]
fn motion_blocking_counts_a_waterlogged_rail_by_its_fluid() {
    // (ironspec 20:27 (c), G1 on bdc3b682f) Heightmap.Types.MOTION_BLOCKING is
    // `blocksMotion() || !getFluidState().isEmpty()`: a waterlogged rail (no collision) tops the
    // column by its water, a dry one doesn't; WORLD_SURFACE takes either (not air)
    use redsim::terrain::Heightmap;
    for (s, motion, surface) in [("rail[shape=north_south,waterlogged=true]", 2, 2), ("rail[shape=north_south,waterlogged=false]", 1, 2)] {
        let mut w = World::new(BlockPos::new(-2, -2, -2), [5, 6, 5]);
        set(&mut w, (0, 0, 0), "glass");
        set(&mut w, (0, 1, 0), s);
        assert_eq!(w.height(Heightmap::MotionBlocking, 0, 0), motion, "{s}: MOTION_BLOCKING's first free y");
        assert_eq!(w.height(Heightmap::MotionBlockingNoLeaves, 0, 0), motion, "{s}: MOTION_BLOCKING_NO_LEAVES's first free y");
        assert_eq!(w.height(Heightmap::WorldSurface, 0, 0), surface, "{s}: WORLD_SURFACE's first free y");
    }
}

#[test]
fn the_wet_rig_spills_exactly_the_official_seven_cells() {
    // tick-exact: the wet copy's flowing (non-source) water within 3 of its rail column after the
    // ride: 7 cells, each the cell over the next lower slope (and over the top slope, fed by the top
    // flat; and over the first flat); none past the walls, none over the flats
    let mut w = World::new(BlockPos::new(-4, 88, -22), [12, 18, 28]);
    rig(&mut w, 0, true);
    for _ in 0..150 {
        w.tick();
    }
    let mut flowing = Vec::new();
    for x in -3..=3 {
        for y in 88..106 {
            for z in -21..=4 {
                let p = BlockPos::new(x, y, z);
                if redsim::fluid::fluid_state(&w, p).fluid.is_water() && redsim::fluid::waterlogged(&w.get(p)) != Some(true) {
                    flowing.push((x, y - 100, z));
                }
            }
        }
    }
    flowing.sort();
    let mut official = vec![(0, 1, 0), (0, 0, -1), (0, -1, -2), (0, -2, -3), (0, -3, -4), (0, -4, -5), (0, -5, -6)];
    official.sort();
    assert_eq!(flowing, official);
}

#[test]
fn the_dry_and_wet_rides_match_the_official_ones() {
    let dry = ride(false, 150);
    let wet = ride(true, 150);
    for t in 1..=150 {
        let ((dz, dv), (wz, wv)) = (at(&dry, t), at(&wet, t));
        if t % 5 == 0 || t < 8 || (36..=44).contains(&t) {
            eprintln!("t {t:>3}: dry z {dz:>9.4} v {dv:.4} | wet z {wz:>9.4} v {wv:.4}");
        }
    }
    // the slopes' foot: dry t 38 at 0.5941, wet t 42 at 0.3903 then 0.3316 at t 45 and 0.2528 at
    // t 50 (official)
    for (r, t, official, what) in [(&dry, 38, 0.5941, "dry"), (&wet, 42, 0.3903, "wet"), (&wet, 45, 0.3316, "wet"), (&wet, 50, 0.2528, "wet")] {
        let (_, v) = at(r, t);
        assert!((v - official).abs() <= 0.005, "{what} t {t}: v {v:.4} (official {official})");
    }
    // dry: stopped at the glass by t 65 at z -16.510 (official)
    let (z, v) = at(&dry, 65);
    assert!((z + 16.510).abs() <= 0.005 && v < 0.005, "dry t 65: z {z:.4} v {v:.4} (official z -16.510, stopped)");
    // wet: the flat's decay x0.947 a tick: 0.1469 at t 60, 0.0496 at t 80, 0.0011 at t 150 at
    // z -11.204 (official)
    for (t, official) in [(60, 0.1469), (80, 0.0496), (150, 0.0011)] {
        let (_, v) = at(&wet, t);
        assert!((v - official).abs() <= 0.005, "wet t {t}: v {v:.4} (official {official})");
    }
    let (z, _) = at(&wet, 150);
    assert!((z + 11.204).abs() <= 0.005, "wet t 150: z {z:.4} (official -11.204)");
}

#[test]
fn a_waterlogged_rail_holds_a_water_source_and_stays_rail_for_the_walker() {
    for s in ["rail[shape=north_south,waterlogged=true]", "powered_rail[powered=false,shape=east_west,waterlogged=true]", "detector_rail[powered=false,shape=north_south,waterlogged=true]"] {
        let b = Block::parse(s).unwrap();
        assert_eq!(redsim::fluid::waterlogged(&b), Some(true), "{s}");
        assert!(redsim::fluid::fluid_state_of(&b).fluid.is_water(), "{s} holds water");
        // BaseRailBlock is typed RAIL before the fluid check: no WATER, so no WATER_BORDER beside it
        assert_eq!(from_state(&b), PathType::Rail, "{s}");
        // the state round-trips through its name
        assert_eq!(Block::parse(&b.to_string()).unwrap(), b, "{s}");
    }
    let dry = Block::parse("rail[shape=north_south,waterlogged=false]").unwrap();
    assert_eq!(redsim::fluid::waterlogged(&dry), Some(false));
    assert!(!redsim::fluid::fluid_state_of(&dry).fluid.is_water());
    // survival: a waterlogged rail stands on the same rigid top as a dry one (BaseRailBlock's
    // canSurvive reads the block below, never the water)
    let mut w = World::new(BlockPos::new(-2, -2, -2), [5, 5, 5]);
    w.place_raw(&[(BlockPos::new(0, 0, 0), Block::parse("glass").unwrap()), (BlockPos::new(0, 1, 0), Block::parse("rail[shape=north_south,waterlogged=true]").unwrap())]);
    assert!(!redsim::rails::would_pop(&w, BlockPos::new(0, 1, 0)), "a wet rail on glass stands");
    let mut w = World::new(BlockPos::new(-2, -2, -2), [5, 5, 5]);
    w.place_raw(&[(BlockPos::new(0, 1, 0), Block::parse("rail[shape=north_south,waterlogged=true]").unwrap())]);
    assert!(redsim::rails::would_pop(&w, BlockPos::new(0, 1, 0)), "a wet rail over air pops");
}

#[test]
fn flowing_water_is_refused_by_a_dry_rail_and_a_walled_wet_line_spills_nothing() {
    // a source beside a dry rail on glass: the flowing water fills the air around, never the rail
    let mut w = World::new(BlockPos::new(-4, -2, -4), [9, 6, 9]);
    for x in -3..=3 {
        for z in -3..=3 {
            set(&mut w, (x, 0, z), "glass");
        }
    }
    set(&mut w, (0, 1, 0), "rail[shape=north_south,waterlogged=false]");
    set(&mut w, (1, 1, 0), "water[level=0]");
    for _ in 0..60 {
        w.tick();
    }
    let r = w.get(BlockPos::new(0, 1, 0));
    assert_eq!(redsim::fluid::waterlogged(&r), Some(false), "the dry rail took flowing water: {r}");
    assert!(redsim::fluid::fluid_state(&w, BlockPos::new(2, 1, 0)).fluid.is_water(), "the source spreads into the air");
    // a level run of waterlogged rails walled in glass on both sides, dry rails at its ends: 0
    // flowing cells after a day of ticks
    let mut w = World::new(BlockPos::new(-2, -2, -9), [5, 6, 12]);
    for z in -7..=1 {
        set(&mut w, (0, 0, z), "glass");
        set(&mut w, (-1, 1, z), "glass");
        set(&mut w, (1, 1, z), "glass");
    }
    set(&mut w, (0, 1, 1), "rail[shape=north_south,waterlogged=false]");
    set(&mut w, (0, 1, -7), "rail[shape=north_south,waterlogged=false]");
    for z in -6..=0 {
        set(&mut w, (0, 1, z), "rail[shape=north_south,waterlogged=true]");
    }
    for _ in 0..200 {
        w.tick();
    }
    let mut flowing = Vec::new();
    for x in -2..=2 {
        for y in -2..=3 {
            for z in -9..=2 {
                let p = BlockPos::new(x, y, z);
                let f = redsim::fluid::fluid_state(&w, p);
                if f.fluid.is_water() && redsim::fluid::waterlogged(&w.get(p)) != Some(true) {
                    flowing.push(p);
                }
            }
        }
    }
    assert!(flowing.is_empty(), "water outside the waterlogged rails: {flowing:?}");
}

/// A villager-sized walker standing at `feet` (on the ground, dry) for the static typing checks.
fn walker(feet: (f64, f64, f64)) -> redsim::entity::mob::ai::path::PathMob {
    use redsim::entity::vec3::{Aabb, Vec3};
    redsim::entity::mob::ai::path::PathMob {
        pos: Vec3::new(feet.0, feet.1, feet.2),
        bb: Aabb { min_x: feet.0 - 0.3, min_y: feet.1, min_z: feet.2 - 0.3, max_x: feet.0 + 0.3, max_y: feet.1 + 1.95, max_z: feet.2 + 0.3 },
        width: 0.6,
        height: 1.95,
        max_up_step: 0.6,
        on_ground: true,
        in_floatable: false,
        malus: redsim::entity::mob::ai::path::Malus::villager(),
        max_fall: 3,
    }
}

/// `getPathTypeStatic` of the open cell (0,1,0) over glass, with `around` placed first.
fn static_type(around: &[((i32, i32, i32), &str)]) -> PathType {
    let mut w = World::new(BlockPos::new(-3, -2, -3), [7, 7, 7]);
    for x in -2..=2 {
        for z in -2..=2 {
            set(&mut w, (x, 0, z), "glass");
        }
    }
    for (p, s) in around {
        set(&mut w, *p, s);
    }
    let mut arena = redsim::entity::mob::ai::path::node::Arena::default();
    let eval = redsim::entity::mob::ai::path::WalkEval::new(&w, walker((0.5, 1.0, 0.5)), redsim::entity::mob::ai::path::EvalFlags { can_float: true, ..Default::default() }, &mut arena);
    eval.path_type_static(0, 1, 0)
}

#[test]
fn water_border_comes_only_from_water_typed_neighbours_at_the_sides() {
    // the positive controls: a water cell at a side or a diagonal makes the walkable cell a border
    assert_eq!(static_type(&[((1, 1, 0), "water[level=0]")]), PathType::WaterBorder, "water at a side");
    assert_eq!(static_type(&[((1, 1, 1), "water[level=0]")]), PathType::WaterBorder, "water at a diagonal");
    assert_eq!(static_type(&[((1, 2, 0), "glass"), ((1, 2, 1), "water[level=0]")]), PathType::WaterBorder, "water a level up at a diagonal");
    // checkNeighbourBlocks skips the cells straight above and below (`i != 0 || k != 0`)
    assert_eq!(static_type(&[((0, 3, 0), "glass"), ((0, 2, 0), "water[level=0]")]), PathType::Walkable, "water straight above");
    // a waterlogged rail is typed RAIL before the fluid: no border beside it
    assert_eq!(static_type(&[((1, 1, 0), "rail[shape=north_south,waterlogged=true]")]), PathType::Walkable, "a waterlogged rail at a side");
    assert_eq!(static_type(&[((1, 1, 1), "rail[shape=north_south,waterlogged=true]")]), PathType::Walkable, "a waterlogged rail at a diagonal");
    // the control without anything around
    assert_eq!(static_type(&[]), PathType::Walkable);
}

#[test]
fn a_rider_in_a_wet_rail_cell_starts_its_path_on_the_rail_and_reaches_along_it() {
    // a level line of waterlogged rails on glass, walled, dry rails at its ends; a villager in a
    // cart on it: in floatable water, its start node climbs the water column to the rail cell (the
    // cell above is dry), and the rail-node rule then lets the path run along the rails
    let mut w = World::new(BlockPos::new(-3, -2, -12), [7, 7, 15]);
    for z in -10..=1 {
        set(&mut w, (0, 0, z), "glass");
        set(&mut w, (-1, 1, z), "glass");
        set(&mut w, (1, 1, z), "glass");
    }
    set(&mut w, (0, 1, 1), "rail[shape=north_south,waterlogged=false]");
    set(&mut w, (0, 1, -10), "rail[shape=north_south,waterlogged=false]");
    for z in -9..=0 {
        set(&mut w, (0, 1, z), "rail[shape=north_south,waterlogged=true]");
    }
    // the mob AI on (the villager gets its brain and navigation)
    w.mob.ai.on = true;
    let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0625, -0.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}")).unwrap();
    for _ in 0..3 {
        w.tick();
    }
    let i = w.entities.list.iter().position(|e| !e.removed && e.vehicle == Some(cart)).expect("the rider");
    assert!(redsim::entity::mob::ai::nav_mob(&w, i).in_floatable, "the rider is in the rail cell's water");
    let target = BlockPos::new(0, 1, -10);
    let path = redsim::entity::mob::ai::with_nav(&mut w, i, |w, nm, nav, arena| {
        let mut copy = nav.clone();
        copy.create_path_pos(w, nm, target, 0, arena).map(|p| (p.reached, p.nodes.iter().map(|n| (n.x, n.y, n.z)).collect::<Vec<_>>()))
    })
    .expect("a path");
    assert_eq!(path.1.first(), Some(&(0, 1, -1)), "the start node is the rider's rail cell: {:?}", path.1);
    assert!(path.0 && path.1.last() == Some(&(0, 1, -10)), "complete along the rails: {:?}", path.1);
}
