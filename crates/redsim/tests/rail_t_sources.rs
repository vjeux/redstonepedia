//! (onechunk, Nov 12 2026, G1 / ironspec 19:55 and 20:05) A plain rail T re-shapes on a neighbour
//! update whose cause is a signal source (26.3 `BaseRailBlock.updateState`: the source block's
//! `isSignalSource()`); the daylight detector, the jukebox and the lightning rod are sources, and
//! the signal sources redsim doesn't model are refused by name.

use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::rails::{junction_at, shape_of};
use redsim::world::{flags, World};

/// A T: rails at (-1,1,0), (0,1,0), (1,1,0) and (0,1,1) on stone, the centre stored in its
/// unpowered settle, and a block at (0,1,-1) beside it.
fn t_world(beside: &str) -> (World, BlockPos) {
    let mut w = World::new(BlockPos::new(-3, -1, -3), [7, 6, 7]);
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for x in -2..=2 {
        for z in -2..=2 {
            cells.push((BlockPos::new(x, 0, z), Block::parse("stone").unwrap()));
        }
    }
    cells.push((BlockPos::new(-1, 1, 0), Block::parse("rail[shape=east_west]").unwrap()));
    cells.push((BlockPos::new(1, 1, 0), Block::parse("rail[shape=east_west]").unwrap()));
    cells.push((BlockPos::new(0, 1, 1), Block::parse("rail[shape=north_south]").unwrap()));
    cells.push((BlockPos::new(0, 1, 0), Block::parse("rail[shape=east_west]").unwrap()));
    cells.push((BlockPos::new(0, 1, -1), Block::parse(beside).unwrap()));
    w.place_raw(&cells);
    let c = BlockPos::new(0, 1, 0);
    let j = junction_at(&w, c).expect("a T");
    w.place_raw(&[(c, Block::parse(&format!("rail[shape={}]", j.unpowered.name())).unwrap())]);
    (w, c)
}

#[test]
fn a_daylight_detector_toggling_beside_a_t_reshapes_it() {
    let (mut w, c) = t_world("daylight_detector[inverted=false,power=0]");
    let j = junction_at(&w, c).unwrap();
    assert_ne!(j.unpowered, j.powered, "the T switches with its signal");
    assert_eq!(shape_of(&w.get(c)), j.unpowered);
    // (the detector's power goes 0 -> 15, as its 20-gt update does at dawn: a source's change)
    w.set_block(BlockPos::new(0, 1, -1), Block::parse("daylight_detector[inverted=false,power=15]").unwrap(), flags::ALL);
    assert_eq!(shape_of(&w.get(c)), j.powered, "re-shaped to its powered settle");
}

/// (G1's CHANGES 2 on bc9a03fee: a control that bites) The T powered by a redstone block laid RAW
/// beside it (no update) sits in its unpowered settle while powered: a non-source neighbour change
/// leaves it as it is (a gate passing every change would re-shape it), a source change (a lever
/// toggled at another neighbour) re-shapes it.
#[test]
fn only_a_source_change_reshapes_a_powered_t() {
    let powered_t = || {
        let (mut w, c) = t_world("redstone_block");
        // (a ceiling for a lever over the T, laid RAW)
        w.place_raw(&[(BlockPos::new(0, 3, 0), Block::parse("stone").unwrap())]);
        let j = junction_at(&w, c).unwrap();
        assert!(j.signal, "the redstone block powers the T");
        assert_eq!(shape_of(&w.get(c)), j.unpowered, "laid RAW: still its unpowered settle");
        (w, c, j)
    };
    let (mut w, c, j) = powered_t();
    w.set_block(BlockPos::new(0, 2, 0), Block::parse("glass").unwrap(), flags::ALL);
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "a non-source change leaves the powered T as it is");
    let (mut w, c, j) = powered_t();
    w.place_raw(&[(BlockPos::new(0, 2, 0), Block::parse("lever[face=ceiling,facing=north,powered=false]").unwrap())]);
    w.set_block(BlockPos::new(0, 2, 0), Block::parse("lever[face=ceiling,facing=north,powered=true]").unwrap(), flags::ALL);
    assert_eq!(shape_of(&w.get(c)), j.powered, "a lever's toggle re-shapes it to its powered settle");
}

#[test]
fn unmodeled_signal_sources_are_refused_by_name() {
    for name in ["minecraft:trapped_chest[facing=north,type=single,waterlogged=false]", "trapped_chest", "minecraft:lectern[facing=north,has_book=false,powered=false]"] {
        let e = Block::parse(name).expect_err(name);
        assert!(e.contains("not modeled by redsim") && (e.contains("trapped chest") || e.contains("lectern")), "{name}: {e}");
    }
    // (a plain chest still parses)
    assert!(Block::parse("chest[facing=north,type=single,waterlogged=false]").is_ok());
}

// ---- (cc767b78, ironspec 2026-11-13 09:44 / 09:48, G1 09:50) the T's re-shape per signal-source type:
// each source's change reaches the T through its own update footprint (`neighborChanged` with the source as
// the neighbour block), and the re-shape reads the T's power then (`updateDir` with `hasNeighborSignal`).

/// A T as [`t_world`] lays it (rails at (-1,1,0), (0,1,0), (1,1,0), (0,1,1) on stone) with `extra` cells laid
/// RAW over the floor and around it (no update), its centre then stored in its `powered`-or-not settle; its
/// junction (computed before the centre is stored).
fn t_beside(extra: &[(BlockPos, &str)], stored_powered: bool) -> (World, BlockPos, redsim::rails::Junction) {
    let mut w = World::new(BlockPos::new(-4, -2, -6), [9, 7, 11]);
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    for x in -3..=3 {
        for z in -5..=3 {
            let p = BlockPos::new(x, 0, z);
            if !extra.iter().any(|(q, _)| *q == p) {
                cells.push((p, Block::parse("stone").unwrap()));
            }
        }
    }
    for (p, s) in [((-1, 1, 0), "rail[shape=east_west]"), ((1, 1, 0), "rail[shape=east_west]"), ((0, 1, 1), "rail[shape=north_south]"), ((0, 1, 0), "rail[shape=east_west]")] {
        cells.push((BlockPos::new(p.0, p.1, p.2), Block::parse(s).unwrap()));
    }
    for (p, s) in extra {
        cells.push((*p, Block::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))));
    }
    w.place_raw(&cells);
    let c = BlockPos::new(0, 1, 0);
    let j = junction_at(&w, c).expect("a T");
    assert_ne!(j.unpowered, j.powered, "the T switches with its signal");
    let stored = if stored_powered { j.powered } else { j.unpowered };
    w.place_raw(&[(c, Block::parse(&format!("rail[shape={}]", stored.name())).unwrap())]);
    (w, c, j)
}

fn pull(w: &mut World, p: BlockPos) {
    let s = w.get(p);
    redsim::redstone::attach::lever::pull(w, s, p);
}

fn ticks(w: &mut World, n: usize) {
    for _ in 0..n {
        w.tick();
    }
}

/// Seven source kinds powering the T and reaching it, then released: the T re-shapes to its powered settle on
/// the change and back to its unpowered one on the release. Lever: on the north face of a stone beside the T
/// (the T powered through the lever's block and updated as that block's neighbour, `LeverBlock.updateNeighbours`'
/// second call). Button: the same placement, pressed, released by its own tick 20 gt later. Torch: under a stone
/// beside the T, toggled by a lever on its support (`RedstoneTorchBlock.onPlace`: every neighbour's neighbours;
/// the torch starts lit, the T stored powered, so its change is to unpowered and back). Dust: beside the T fed by
/// a floor lever, pointing into it (`RedStoneWireBlock`: its neighbours' neighbours). Repeater: beside the T facing
/// away, fed by a floor lever, its output into the T (`updateNeighborsInFront`). Detector rail: on a stone two
/// above the T (its block strongly powered, the T that block's neighbour: `DetectorRailBlock.checkPressed`'
/// `updateNeighborsAt(pos.below())`), pressed by a cart and released. Observer: beside the T watching away, its
/// output into the T, pulsed by a block placed in front of it. (Of `kind_is_signal_source`'s 17 kinds the
/// daylight detector is pinned above and the redstone block by the removal test below; the wall torch, comparator,
/// target, sculk sensor, plates, tripwire hook, jukebox and lightning rod are not pinned here.)
#[test]
fn seven_source_kinds_reshape_their_t_and_their_release_reshapes_it_back() {
    let n = |k: i32| BlockPos::new(0, 1, -k);
    // lever
    let (mut w, c, j) = t_beside(&[(n(1), "stone"), (n(2), "lever[face=wall,facing=north,powered=false]")], false);
    pull(&mut w, n(2));
    assert_eq!(shape_of(&w.get(c)), j.powered, "lever: pulled");
    pull(&mut w, n(2));
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "lever: released");
    // button (released by its own tick)
    let (mut w, c, j) = t_beside(&[(n(1), "stone"), (n(2), "stone_button[face=wall,facing=north,powered=false]")], false);
    let s = w.get(n(2));
    redsim::redstone::attach::button::press(&mut w, s, n(2));
    assert!(matches!(w.get(n(2)), Block::Button { powered: true, .. }), "the button is pressed");
    assert_eq!(shape_of(&w.get(c)), j.powered, "button: pressed");
    ticks(&mut w, 25);
    assert!(matches!(w.get(n(2)), Block::Button { powered: false, .. }), "the button released itself");
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "button: released");
    // torch (lit at the start: the T powered and stored so)
    let torch = BlockPos::new(0, 0, -1);
    let (mut w, c, j) = t_beside(&[(BlockPos::new(0, -1, -1), "stone"), (torch, "redstone_torch[lit=true]"), (n(1), "stone"), (BlockPos::new(0, -1, -2), "lever[face=wall,facing=north,powered=false]")], true);
    assert!(j.signal, "the lit torch powers the T through its block");
    pull(&mut w, BlockPos::new(0, -1, -2));
    ticks(&mut w, 4);
    assert!(matches!(w.get(torch), Block::Torch { lit: false }), "the torch went out");
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "torch: out");
    pull(&mut w, BlockPos::new(0, -1, -2));
    ticks(&mut w, 4);
    assert_eq!(shape_of(&w.get(c)), j.powered, "torch: lit again");
    // dust
    let (mut w, c, j) = t_beside(&[(n(1), "redstone_wire[east=none,north=side,power=0,south=side,west=none]"), (n(2), "lever[face=floor,facing=north,powered=false]")], false);
    pull(&mut w, n(2));
    assert_eq!(shape_of(&w.get(c)), j.powered, "dust: powered");
    pull(&mut w, n(2));
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "dust: unpowered");
    // repeater (facing north: its input from the north, its output south into the T)
    let (mut w, c, j) = t_beside(&[(n(1), "repeater[delay=1,facing=north,locked=false,powered=false]"), (n(2), "lever[face=floor,facing=north,powered=false]")], false);
    pull(&mut w, n(2));
    ticks(&mut w, 4);
    assert_eq!(shape_of(&w.get(c)), j.powered, "repeater: on");
    pull(&mut w, n(2));
    ticks(&mut w, 4);
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "repeater: off");
    // detector rail on a stone two above the T
    let (mut w, c, j) = t_beside(&[(BlockPos::new(0, 2, 0), "stone"), (BlockPos::new(0, 3, 0), "detector_rail[powered=false,shape=north_south]")], false);
    let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 3.0, 0.5], None).unwrap();
    ticks(&mut w, 2);
    assert_eq!(shape_of(&w.get(c)), j.powered, "detector: pressed");
    let ci = redsim::entity::index_of(&w, cart).unwrap();
    w.entities.list[ci].removed = true;
    ticks(&mut w, 30);
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "detector: released");
    // observer (facing north: it watches n(2), its output south into the T), pulsed
    let (mut w, c, j) = t_beside(&[(n(1), "observer[facing=north,powered=false]")], false);
    w.set_block(n(2), Block::parse("stone").unwrap(), flags::ALL);
    let mut seen = Vec::new();
    for _ in 0..8 {
        w.tick();
        seen.push(shape_of(&w.get(c)));
    }
    assert!(seen.contains(&j.powered), "observer: its pulse re-shaped the T ({seen:?})");
    assert_eq!(*seen.last().unwrap(), j.unpowered, "observer: back after its pulse");
}

/// Each source kind's update footprint, at its edge and one cell beyond: the T powered by a redstone block laid
/// RAW over it (stored in its unpowered settle while powered), so that only an update re-shapes it: a source
/// changing at the footprint's edge re-shapes it to its powered settle, the same source one cell further
/// doesn't; in both rows the source is checked to have FIRED (G1 12:19 (2a): a beyond row whose source never
/// changed would pass for nothing). Lever: on a block beside the T (the T is that block's neighbour) / on a block
/// two away. Torch: under a block beside the T (the T a neighbour's neighbour) / one further. Dust: two away (a
/// neighbour's neighbour) / three away. Repeater: its output into a block beside the T (the front's neighbours) /
/// one further. Detector rail: on the redstone block over the T (its block's neighbours) / one block higher.
/// Observer: its output into a block beside the T / one further.
#[test]
fn each_source_kinds_update_footprint_edge_and_beyond() {
    let n = |k: i32| BlockPos::new(0, 1, -k);
    let rb = (BlockPos::new(0, 2, 0), "redstone_block");
    // (the act returns whether its source fired)
    let reshaped = |extra: &[(BlockPos, &str)], act: &dyn Fn(&mut World) -> bool, what: &str| -> bool {
        let mut cells = vec![rb];
        cells.extend_from_slice(extra);
        let (mut w, c, j) = t_beside(&cells, false);
        assert!(j.signal, "{what}: the redstone block powers the T");
        assert!(act(&mut w), "{what}: its source fired");
        let s = shape_of(&w.get(c));
        assert!(s == j.powered || s == j.unpowered, "{what}");
        s == j.powered
    };
    let lever_at = |p: BlockPos| {
        move |w: &mut World| {
            pull(w, p);
            matches!(w.get(p), Block::Lever { powered: true, .. })
        }
    };
    // lever
    assert!(reshaped(&[(n(1), "stone"), (n(2), "lever[face=wall,facing=north,powered=false]")], &lever_at(n(2)), "lever beside"), "lever on the block beside the T");
    assert!(!reshaped(&[(n(1), "stone"), (n(2), "stone"), (n(3), "lever[face=wall,facing=north,powered=false]")], &lever_at(n(3)), "lever beyond"), "lever one block further");
    // torch, put out by a lever on its support
    let torch_out = |k: i32| {
        move |w: &mut World| {
            pull(w, BlockPos::new(0, -1, -k - 1));
            ticks(w, 4);
            matches!(w.get(BlockPos::new(0, 0, -k)), Block::Torch { lit: false })
        }
    };
    let torch = |k: i32| vec![(BlockPos::new(0, -1, -k), "stone"), (BlockPos::new(0, 0, -k), "redstone_torch[lit=true]"), (BlockPos::new(0, -1, -k - 1), "lever[face=wall,facing=north,powered=false]")];
    let mut edge = torch(1);
    edge.push((n(1), "stone"));
    assert!(reshaped(&edge, &torch_out(1), "torch beside"), "torch under the block beside the T");
    let mut beyond = torch(2);
    beyond.push((n(2), "stone"));
    assert!(!reshaped(&beyond, &torch_out(2), "torch beyond"), "torch one further");
    // dust fed by a floor lever behind it
    let dust = "redstone_wire[east=none,north=side,power=0,south=side,west=none]";
    let lever_floor = "lever[face=floor,facing=north,powered=false]";
    let dust_on = |lever: BlockPos, wire: BlockPos| {
        move |w: &mut World| {
            pull(w, lever);
            matches!(w.get(wire), Block::Wire { power, .. } if power > 0)
        }
    };
    assert!(reshaped(&[(n(2), dust), (n(3), lever_floor)], &dust_on(n(3), n(2)), "dust two away"), "dust two away");
    assert!(!reshaped(&[(n(3), dust), (n(4), lever_floor)], &dust_on(n(4), n(3)), "dust three away"), "dust three away");
    // repeater fed by a floor lever behind it, its output into a stone
    let rep = "repeater[delay=1,facing=north,locked=false,powered=false]";
    let rep_on = |lever: BlockPos, r: BlockPos| {
        move |w: &mut World| {
            pull(w, lever);
            ticks(w, 4);
            matches!(w.get(r), Block::Repeater { powered: true, .. })
        }
    };
    assert!(reshaped(&[(n(1), "stone"), (n(2), rep), (n(3), lever_floor)], &rep_on(n(3), n(2)), "repeater beside"), "repeater into the block beside the T");
    assert!(!reshaped(&[(n(1), "stone"), (n(2), "stone"), (n(3), rep), (n(4), lever_floor)], &rep_on(n(4), n(3)), "repeater beyond"), "repeater one further");
    // detector rail on the redstone block over the T / one block higher, pressed by a cart
    let press = |y: i32| {
        move |w: &mut World| {
            redsim::entity::spawn(w, "minecraft:minecart", [0.5, y as f64, 0.5], None).unwrap();
            ticks(w, 2);
            matches!(w.get(BlockPos::new(0, y, 0)), Block::DetectorRail { powered: true, .. })
        }
    };
    assert!(reshaped(&[(BlockPos::new(0, 3, 0), "detector_rail[powered=false,shape=north_south]")], &press(3), "detector over"), "detector on the block over the T");
    assert!(!reshaped(&[(BlockPos::new(0, 3, 0), "stone"), (BlockPos::new(0, 4, 0), "detector_rail[powered=false,shape=north_south]")], &press(4), "detector higher"), "detector one block higher");
    // observer watching away, its output into a stone, pulsed by a block set in front of it
    let pulse = |front: BlockPos, obs: BlockPos| {
        move |w: &mut World| {
            w.set_block(front, Block::parse("stone").unwrap(), flags::ALL);
            let mut fired = false;
            for _ in 0..3 {
                w.tick();
                fired |= matches!(w.get(obs), Block::Observer { powered: true, .. });
            }
            fired
        }
    };
    assert!(reshaped(&[(n(1), "stone"), (n(2), "observer[facing=north,powered=false]")], &pulse(n(3), n(2)), "observer beside"), "observer into the block beside the T");
    assert!(!reshaped(&[(n(1), "stone"), (n(2), "stone"), (n(3), "observer[facing=north,powered=false]")], &pulse(n(4), n(3)), "observer beyond"), "observer one further");
}

/// (G1 12:19 (2c)) A neighbour update names the block that WAS there (`Level.markAndNotifyBlock` →
/// `updateNeighborsAt(pos, oldState.getBlock())`), so removing a source re-shapes the T and placing one over air
/// doesn't: a redstone block set beside a T in its unpowered settle powers it but leaves it unpowered-shaped (the
/// update's block is air, no signal source); a redstone block beside a T stored powered, then removed, re-shapes it
/// to its unpowered settle (the update's block is the redstone block).
#[test]
fn removing_a_source_reshapes_the_t_placing_one_over_air_does_not() {
    let beside = BlockPos::new(0, 1, -1);
    let (mut w, c, j) = t_beside(&[], false);
    assert!(!j.signal, "nothing powers the T yet");
    w.set_block(beside, Block::parse("redstone_block").unwrap(), flags::ALL);
    assert!(junction_at(&w, c).unwrap().signal, "the placed block powers the T");
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "placed over air: the update names air, the T keeps its unpowered settle");
    let (mut w, c, j) = t_beside(&[(beside, "redstone_block")], true);
    assert!(j.signal && shape_of(&w.get(c)) == j.powered, "stored powered beside the block");
    w.set_block(beside, Block::Air, flags::ALL);
    assert!(!junction_at(&w, c).unwrap().signal, "unpowered once it's gone");
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "removed: the update names the redstone block, the T re-shapes");
}

/// (ironspec 13:54 / 14:00; tick-exact's game row 14:03, https://artifacts.internalmeta.com/a/1072581002473618)
/// A detector press reaches a connected T by more than `updatePowerToConnected` (which names the T's own
/// rail, no signal source): its POWERED flip is a rail's state change, so `LevelChunk.setBlockState` runs
/// the detector's `affectNeighborsAfterRemoval` (26.3, `blockChanged || newBlock instanceof BaseRailBlock`),
/// whose sloped case updates `pos.above()`'s neighbours naming the detector. The T's south arm a detector
/// rail one lower (ascending north into the T's centre), the centre stored in its powered settle while
/// nothing powers it, a cart on the detector: in the tick the detector fires, the centre goes to its
/// unpowered settle, as the game's probes show (w3: the detector powered, the centre south_east).
#[test]
fn a_detector_press_resettles_a_connected_t_through_its_after_removal() {
    let d = BlockPos::new(0, 0, 1);
    let (mut w, c, j) = t_beside(&[(BlockPos::new(0, 1, 1), "air"), (d, "detector_rail[powered=false,shape=ascending_north]"), (BlockPos::new(0, -1, 1), "stone")], true);
    assert!(!j.signal, "nothing powers the T");
    assert_eq!(shape_of(&w.get(c)), j.powered, "stored off its settle");
    assert_eq!(j.powered.name(), "south_west", "the game row's stored shape");
    redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 0.5, 1.5], None).unwrap();
    w.tick();
    assert!(matches!(w.get(d), Block::DetectorRail { powered: true, .. }), "the detector fired: {}", w.get(d));
    assert!(!junction_at(&w, c).unwrap().signal, "the T is still unpowered");
    assert_eq!(shape_of(&w.get(c)), j.unpowered, "re-settled in the same tick");
    assert_eq!(j.unpowered.name(), "south_east", "the game row's re-settled shape");
}
