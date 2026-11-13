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
