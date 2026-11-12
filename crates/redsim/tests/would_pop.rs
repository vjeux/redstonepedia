//! (farmopt, Nov 12 2026, ironspec's ruling after onechunk found ironfarm's lane top slopes
//! unsupported) `rails::would_pop`: 26.3's `BaseRailBlock.shouldBeRemoved` on a raw paste.

use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::rails::would_pop;
use redsim::world::World;

fn world(cells: &[((i32, i32, i32), &str)]) -> World {
    let mut w = World::new(BlockPos::new(-2, -2, -2), [6, 6, 6]);
    let cells: Vec<(BlockPos, Block)> = cells.iter().map(|((x, y, z), s)| (BlockPos::new(*x, *y, *z), Block::parse(s).unwrap())).collect();
    w.place_raw(&cells);
    w
}

#[test]
fn an_ascending_rail_needs_a_rigid_block_below_and_on_its_high_side() {
    let rail = BlockPos::new(0, 1, 0);
    // ascending south: the high side is south (z + 1)
    let open = world(&[((0, 0, 0), "glass"), ((0, 1, 0), "rail[shape=ascending_south]")]);
    assert!(would_pop(&open, rail), "air on the high side pops");
    let held = world(&[((0, 0, 0), "glass"), ((0, 1, 0), "rail[shape=ascending_south]"), ((0, 1, 1), "glass")]);
    assert!(!would_pop(&held, rail), "glass on the high side holds it");
    let wrong_side = world(&[((0, 0, 0), "glass"), ((0, 1, 0), "rail[shape=ascending_south]"), ((0, 1, -1), "stone")]);
    assert!(would_pop(&wrong_side, rail), "a block on the low side doesn't");
    // a flat rail needs only its rigid top below; a bottom slab's top is no rigid top
    assert!(!would_pop(&world(&[((0, 0, 0), "stone"), ((0, 1, 0), "rail[shape=north_south]")]), rail));
    assert!(would_pop(&world(&[((0, 0, 0), "stone_slab[type=bottom]"), ((0, 1, 0), "rail[shape=north_south]")]), rail));
    assert!(would_pop(&world(&[((0, 1, 0), "rail[shape=north_south]")]), rail), "nothing below pops");
    // no rail, no pop
    assert!(!would_pop(&world(&[((0, 1, 0), "stone")]), rail));
}
