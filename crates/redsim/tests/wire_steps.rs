//! (onechunk, Nov 12 2026, ironspec 21:40 PT: the glass-stair finding on day-1's R1 line) A dust
//! signal climbs a step over any block but descends one only over a conductor
//! (`RedStoneWireBlock.calculateTargetStrength`: a dust reads the dust one block up beside it only
//! when the block under that dust conducts, and the dust one block down beside it when the block
//! between them doesn't).

use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::world::{World, flags};

const DUST: &str = "redstone_wire[east=none,north=none,power=0,south=none,west=none]";

/// A two-dust stair: the upper dust at (1,1,0) on `support` (1,0,0), the lower dust at (0,0,0) on
/// glass (0,-1,0); a floor lever on glass beside the upper dust at (2,1,0) (`power_upper`) or beside
/// the lower one at (-1,0,0), switched on. Returns the two dusts' powers (upper, lower).
fn stair(support: &str, power_upper: bool) -> (i32, i32) {
    let mut w = World::new(BlockPos::new(-3, -3, -3), [8, 8, 8]);
    let cells = [((1, 0, 0), support), ((0, -1, 0), "glass"), ((2, 0, 0), "glass"), ((-1, -1, 0), "glass"), ((1, 1, 0), DUST), ((0, 0, 0), DUST)];
    for ((x, y, z), s) in cells {
        w.set_block(BlockPos::new(x, y, z), Block::parse(&format!("minecraft:{s}")).unwrap(), flags::ALL);
    }
    let lever = if power_upper { BlockPos::new(2, 1, 0) } else { BlockPos::new(-1, 0, 0) };
    w.set_block(lever, Block::parse("minecraft:lever[face=floor,facing=east,powered=false]").unwrap(), flags::ALL);
    w.set_block(lever, Block::parse("minecraft:lever[face=floor,facing=east,powered=true]").unwrap(), flags::ALL);
    let p = |q: BlockPos| w.get(q).to_string().split("power=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|v| v.parse().ok()).unwrap_or(-1);
    (p(BlockPos::new(1, 1, 0)), p(BlockPos::new(0, 0, 0)))
}

#[test]
fn a_dust_signal_descends_a_step_only_over_a_conductor() {
    let (up, low) = stair("glass", true);
    assert!(up > 0 && low == 0, "over glass the signal doesn't descend (upper {up}, lower {low})");
    let (up, low) = stair("smooth_stone", true);
    assert!(up > 0 && low > 0, "over a conductor it does (upper {up}, lower {low})");
}

#[test]
fn a_dust_signal_climbs_a_step_over_glass() {
    let (up, low) = stair("glass", false);
    assert!(low > 0 && up > 0, "the signal climbs a glass step (lower {low}, upper {up})");
}
