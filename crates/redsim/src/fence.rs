//! Fences (`FenceBlock`, `CrossCollisionBlock`; clk-misc, 2026-10-20), read from the 26.3 source.
//!
//! * The 12 wooden fences (`#minecraft:wooden_fences`) and the nether brick fence (`#fences` only).
//!   Not conductors (their collision shape is not a full cube), solid (`forceSolidOn` for the
//!   overworld woods; the others by their 1.5 block tall collision box), pushable, note block
//!   instrument BASS (nether brick: BASEDRUM).
//! * Shapes: collision = a 4 px post (6..10) 24 px tall plus a 4 px arm 24 px tall toward each
//!   connected side (`makeShapes(4, 24, 4, 0, 24)`); the block support shape is that collision shape
//!   (`getBlockSupportShape` default), so the top face is CENTER-sturdy (torches stand on fences).
//!   The collision box leaves the cell (`hasLargeCollisionShape`).
//! * `updateShape` (a waterlogged fence first schedules its water tick): from a horizontal neighbour,
//!   that side's arm = `connectsTo(neighbour, neighbour.isFaceSturdy(face toward the fence, FULL),
//!   direction from the neighbour)`: a sturdy face of a block that is not an exception (leaves,
//!   barrier, pumpkins, melon, shulker boxes), the same fence family (wooden with wooden, nether
//!   brick with nether brick), or a fence gate whose facing axis is the direction's clockwise axis.
//!   Vertical neighbours change nothing. The state change is what an observer watching the fence sees
//!   (gadget-style BUD: `bud-fence-observer`).

use crate::behavior::{self as bh, Box16, Support};
use crate::block::Block;
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// Fence ids without `_fence`, by `Block::Fence::kind`: the wooden fences, then nether brick.
pub const FENCES: [&str; 13] = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak", "bamboo", "crimson", "warped", "nether_brick"];
/// `Block::Fence::kind` of the nether brick fence (the only fence not in `#wooden_fences`).
pub const NETHER_BRICK: u8 = 12;

/// `state.is(BlockTags.WOODEN_FENCES)`.
pub fn wooden(kind: u8) -> bool {
    kind != NETHER_BRICK
}

/// The collision boxes (= support shape) of a fence with these arms.
pub fn collision_boxes(north: bool, east: bool, south: bool, west: bool) -> Vec<Box16> {
    let mut v: Vec<Box16> = vec![[6, 10, 0, 24, 6, 10]];
    if north {
        v.push([6, 10, 0, 24, 0, 8]);
    }
    if south {
        v.push([6, 10, 0, 24, 8, 16]);
    }
    if east {
        v.push([8, 16, 0, 24, 6, 10]);
    }
    if west {
        v.push([0, 8, 0, 24, 6, 10]);
    }
    v
}

/// `isFaceSturdy(face, support)` of a fence (its support shape is its collision shape).
pub fn sturdy(s: &Block, face: Dir, support: Support) -> bool {
    let Block::Fence { north, east, south, west, .. } = *s else { return false };
    let boxes: Vec<crate::piston::shape::B16> = collision_boxes(north, east, south, west).into_iter().map(crate::piston::shape::B16).collect();
    crate::piston::shape::face_supports(&boxes, face, support)
}

/// `Block.isExceptionForConnection` (`behavior::exception_for_connection`).
fn exception(s: &Block) -> bool {
    bh::exception_for_connection(s)
}

/// `FenceBlock.connectsTo(state, faceSolid, direction)` for a fence of `kind`.
pub fn connects_to(kind: u8, s: &Block, face_solid: bool, d: Dir) -> bool {
    let same = matches!(*s, Block::Fence { kind: k, .. } if wooden(k) == wooden(kind));
    let gate = matches!(*s, Block::FenceGate { facing, .. } if facing.axis() == d.clockwise().axis());
    (!exception(s) && face_solid) || same || gate
}

/// `FenceBlock.updateShape`.
pub fn update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, np: BlockPos, ns: Block) -> Block {
    crate::fluid::waterlogged_update_shape(w, &s, p);
    let Block::Fence { kind, north, east, south, west, waterlogged } = s else { return s };
    if !d.is_horizontal() {
        return s;
    }
    let c = connects_to(kind, &ns, bh::sturdy(w, &ns, np, d.opposite(), Support::Full), d.opposite());
    match d {
        Dir::North => Block::Fence { kind, north: c, east, south, west, waterlogged },
        Dir::East => Block::Fence { kind, north, east: c, south, west, waterlogged },
        Dir::South => Block::Fence { kind, north, east, south: c, west, waterlogged },
        _ => Block::Fence { kind, north, east, south, west: c, waterlogged },
    }
}
