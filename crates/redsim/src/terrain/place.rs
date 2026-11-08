//! Placing a design on a terrain sample the way a player builds it (brief "Placement"): the
//! design's ground layer one above the spot's surface, the footprint cleared (every cell of the
//! design box's columns from its lowest layer up to the box's top: the site cleared), the design
//! pasted by the judge's usual code, the outside left natural. The game check writes the same cells
//! (`setblock ... strict`), so both worlds are the same.
//!
//! A spot is refused when the clearing would leave the world inconsistent with what the game does
//! when a player digs (`UPDATE_ALL`): a natural fluid beside a cleared cell (it would flow in), a log
//! or leaves in the footprint (the tree's leaves would decay), or a natural state beside a cleared
//! cell that hangs on it or reshapes with it (wall-attached plants and lights, fences, walls, panes,
//! stairs, beds, chests, doors: their classes from the natural-state table).

use super::blocks;
use crate::block::Block;
use crate::pos::BlockPos;
use crate::world::World;

/// Where a design goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The world column of the design's (0, 0) column.
    pub at: [i32; 2],
    /// The world y of the design's ground layer (the layer that stands on the surface).
    pub y: i32,
}

/// The classes of natural states that hang on a neighbour or reshape with it.
const ATTACHED: &[&str] = &[
    "VineBlock",
    "LadderBlock",
    "WallTorchBlock",
    "RedstoneWallTorchBlock",
    "CocoaBlock",
    "MultifaceBlock",
    "WallSignBlock",
    "WallHangingSignBlock",
    "WallBannerBlock",
    "TripWireHookBlock",
    "FaceAttachedHorizontalDirectionalBlock",
    "WallSkullBlock",
    "FenceBlock",
    "WallBlock",
    "IronBarsBlock",
    "CrossCollisionBlock",
    "StairBlock",
    "BedBlock",
    "ChestBlock",
    "DoorBlock",
    "LanternBlock",
    "BellBlock",
];

fn is_fluid(b: &Block) -> bool {
    match *b {
        Block::Water { .. } | Block::Lava { .. } | Block::BubbleColumn { .. } | Block::Kelp { .. } | Block::KelpPlant => true,
        Block::Terrain(id) => blocks::props(id).fluid != blocks::FluidKind::Empty,
        _ => false,
    }
}

fn natural_class(b: &Block) -> Option<&'static blocks::Props> {
    match *b {
        Block::Terrain(id) => Some(blocks::props(id)),
        _ => None,
    }
}

/// The world cells of the footprint (the design box `lo..=hi` in world coordinates, every y from
/// `lo.y` to the world box's top).
pub fn footprint_cells(w: &World, lo: BlockPos, hi: BlockPos) -> Vec<BlockPos> {
    let top = w.min.y + w.size[1] - 1;
    let mut out = Vec::new();
    for y in lo.y..=top {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                out.push(BlockPos::new(x, y, z));
            }
        }
    }
    out
}

/// Why a footprint cannot be cleared there (`None`: it can).
pub fn refusal(w: &World, lo: BlockPos, hi: BlockPos) -> Option<String> {
    let inside = |p: BlockPos| p.x >= lo.x && p.x <= hi.x && p.z >= lo.z && p.z <= hi.z && p.y >= lo.y;
    for p in footprint_cells(w, lo, hi) {
        let b = w.get(p);
        if b.is_air() {
            continue;
        }
        if let Some(props) = natural_class(&b) {
            if props.leaves_block || props.has_tag("minecraft:logs") {
                return Some(format!("{} at {p} is in the footprint (a tree: its leaves would decay)", props.name));
            }
        }
        for d in crate::pos::Dir::ALL {
            let q = p.rel(d);
            if inside(q) || !w.contains(q) {
                continue;
            }
            let n = w.get(q);
            if is_fluid(&n) {
                return Some(format!("{n} at {q} would flow into the cleared cell {p}"));
            }
            if let Some(np) = natural_class(&n)
                && np.classes.iter().any(|c| ATTACHED.contains(&c.as_str()))
            {
                return Some(format!("{} at {q} hangs on or reshapes with the cleared cell {p}", np.name));
            }
        }
    }
    None
}

/// Clear the footprint (raw writes, as `setblock ... strict`); returns the blocks dug (the
/// natural non-air cells cleared).
pub fn clear(w: &mut World, lo: BlockPos, hi: BlockPos) -> u32 {
    let mut dug = 0;
    for p in footprint_cells(w, lo, hi) {
        if !w.get(p).is_air() {
            dug += 1;
            w.set_block(p, Block::Air, crate::world::flags::RAW);
        }
    }
    dug
}

/// The surface height a design stands on at a spot: the highest MOTION_BLOCKING_NO_LEAVES over the
/// footprint's columns (the ground layer goes one above it).
pub fn ground_y(w: &World, lo_xz: [i32; 2], hi_xz: [i32; 2]) -> i32 {
    let mut top = i32::MIN;
    for z in lo_xz[1]..=hi_xz[1] {
        for x in lo_xz[0]..=hi_xz[0] {
            top = top.max(w.height(super::Heightmap::MotionBlockingNoLeaves, x, z));
        }
    }
    top
}
