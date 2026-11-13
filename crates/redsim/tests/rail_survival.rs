//! (onechunk, Nov 12 2026, ironspec 20:31 (4a)) A rail survives on a rigid top below
//! (`BaseRailBlock.canSurvive`: `canSupportRigidBlock(below)`), and an ascending one also needs a
//! rigid block on its high side (`shouldBeRemoved`): both of redsim's rail paths (`rails`: plain and
//! detector rails; `redstone::blocks::rail`: powered and activator rails) and `rails::would_pop`.

use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::world::World;

fn world(cells: &[((i32, i32, i32), &str)]) -> World {
    let mut w = World::new(BlockPos::new(-2, -2, -2), [6, 6, 6]);
    let cells: Vec<(BlockPos, Block)> = cells.iter().map(|((x, y, z), s)| (BlockPos::new(*x, *y, *z), Block::parse(s).unwrap_or_else(|e| panic!("{s}: {e}")))).collect();
    w.place_raw(&cells);
    w
}

const PLAIN: &str = "rail[shape=north_south]";
const POWERED: &str = "powered_rail[powered=false,shape=north_south]";

/// The verdicts of both paths and would_pop for a flat rail over `floor` (None: air).
fn verdicts(floor: Option<&str>) -> (bool, bool, bool, bool) {
    let rail = BlockPos::new(0, 1, 0);
    let with = |r: &'static str| -> World {
        match floor {
            Some(f) => world(&[((0, 0, 0), f), ((0, 1, 0), r)]),
            None => world(&[((0, 1, 0), r)]),
        }
    };
    let (wp, wq) = (with(PLAIN), with(POWERED));
    (redsim::rails::can_survive(&wp, rail), redsim::redstone::blocks::rail::can_survive(&wq, rail), redsim::rails::would_pop(&wp, rail), redsim::rails::would_pop(&wq, rail))
}

#[test]
fn a_rail_is_refused_over_a_non_rigid_top() {
    for floor in [Some("water[level=0]"), Some("bubble_column[drag=false]"), None, Some("stone_slab[type=bottom,waterlogged=false]"), Some("oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]")] {
        let (plain, powered, pop_plain, pop_powered) = verdicts(floor);
        assert!(!plain && !powered, "{floor:?}: refused on both paths (plain {plain}, powered {powered})");
        assert!(pop_plain && pop_powered, "{floor:?}: would_pop on both ({pop_plain}, {pop_powered})");
    }
}

#[test]
fn a_rail_stands_over_a_rigid_top() {
    for floor in ["glass", "slime_block", "stone_slab[type=top,waterlogged=false]", "soul_sand", "hopper[enabled=true,facing=down]"] {
        let (plain, powered, pop_plain, pop_powered) = verdicts(Some(floor));
        assert!(plain && powered, "{floor}: stands on both paths (plain {plain}, powered {powered})");
        assert!(!pop_plain && !pop_powered, "{floor}: no pop ({pop_plain}, {pop_powered})");
    }
    // (redsim models no stairs: a design holding an upside-down stair is refused at its parse,
    // so no stair's top is read as anything)
    assert!(Block::parse("stone_stairs[facing=north,half=top,shape=straight,waterlogged=false]").is_err());
}

#[test]
fn an_ascending_rail_is_refused_when_its_high_side_is_water() {
    let rail = BlockPos::new(0, 1, 0);
    // (ascending south: the high side is south, z + 1)
    for r in ["rail[shape=ascending_south]", "powered_rail[powered=false,shape=ascending_south]"] {
        let wet = world(&[((0, 0, 0), "glass"), ((0, 1, 0), r), ((0, 1, 1), "water[level=0]")]);
        assert!(redsim::rails::would_pop(&wet, rail), "{r}: water on the high side pops");
        let held = world(&[((0, 0, 0), "glass"), ((0, 1, 0), r), ((0, 1, 1), "glass")]);
        assert!(!redsim::rails::would_pop(&held, rail), "{r}: glass on the high side holds");
    }
}
