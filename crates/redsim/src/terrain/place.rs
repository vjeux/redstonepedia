//! Placing a design on a terrain sample the way a player builds it (brief "Placement"): the
//! design's ground layer at the spot's ground, the footprint cleared (every cell of the design box's
//! columns from its lowest layer up to the box's top: the site cleared), the design pasted by the
//! judge's usual code, the outside left natural. The game check writes the same cells
//! (`setblock ... strict`), so both worlds are the same.
//!
//! A spot is refused ([`refusal`]) when a footprint cell's change could make the game differ from
//! these strict writes: a natural state beside a footprint cell whose shape depends on that
//! neighbour (its support flag toward it in the natural-state table: the game's own `updateShape`
//! changes it when the neighbour turns from dirt into air: plants and wall lights pop off, fences,
//! walls, panes, stairs, beds, chests and doors reshape), a fluid beside or in the footprint or under
//! it, a tree trunk in it, a structure piece in its columns.

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
    // no structure piece in the footprint's columns (a village house, path or field)
    if let Some(l) = w.land.as_ref() {
        for s in &l.structures {
            for b in &s.pieces {
                if b[0] <= hi.x && b[3] >= lo.x && b[2] <= hi.z && b[5] >= lo.z && b[4] >= lo.y {
                    return Some(format!("a piece of {} ({},{},{})..({},{},{}) is in the footprint", s.id, b[0], b[1], b[2], b[3], b[4], b[5]));
                }
            }
        }
    }
    // no fluid in the footprint's columns from one below its bottom up, nor at their surface (a
    // farm on a river or a lake)
    for z in lo.z..=hi.z {
        for x in lo.x..=hi.x {
            let p = BlockPos::new(x, lo.y - 1, z);
            if is_fluid(&w.get(p)) {
                return Some(format!("{} at {p} is under the footprint", w.get(p)));
            }
            let top = BlockPos::new(x, w.height(super::Heightmap::MotionBlocking, x, z) - 1, z);
            if is_fluid(&w.get(top)) {
                return Some(format!("{} at {top} is the surface under the footprint", w.get(top)));
            }
        }
    }
    for p in footprint_cells(w, lo, hi) {
        let b = w.get(p);
        if is_fluid(&b) {
            return Some(format!("{b} at {p} is in the footprint"));
        }
        if let Some(props) = natural_class(&b) {
            // a trunk: felling it is a player's choice the judge does not make for them (leaves
            // overhanging the site are cleared with it: strict writes, so no leaf's distance changes
            // and none decays, in redsim as in the game check)
            if props.has_tag("minecraft:logs") {
                return Some(format!("{} at {p} is in the footprint (a tree trunk)", props.name));
            }
        }
        // every footprint cell may change (cleared, or written by the design): a natural state
        // beside it whose shape depends on that neighbour (its support flag toward it: dirt to air
        // changes it, `updateShape`), or a fluid, refuses the spot
        for d in crate::pos::Dir::ALL {
            let q = p.rel(d);
            if inside(q) || !w.contains(q) {
                continue;
            }
            let n = w.get(q);
            if is_fluid(&n) {
                return Some(format!("{n} at {q} is beside the footprint cell {p}"));
            }
            if let Some(np) = natural_class(&n)
                && np.support[blocks::dir_index(d.opposite())]
            {
                return Some(format!("{} at {q} depends on its neighbour {p} in the footprint (updateShape)", np.name));
            }
        }
    }
    None
}

/// Clear the footprint (raw writes, as `setblock ... strict`); returns the blocks dug (the
/// natural cells cleared that are not air of some kind: cave air is rewritten as air, not dug).
pub fn clear(w: &mut World, lo: BlockPos, hi: BlockPos) -> u32 {
    let mut dug = 0;
    for p in footprint_cells(w, lo, hi) {
        let b = w.get(p);
        if !b.is_air() {
            if !air_like(&b) {
                dug += 1;
            }
            w.set_block(p, Block::Air, crate::world::flags::RAW);
        }
    }
    dug
}

/// Air or a natural air state (cave air, void air).
pub fn air_like(b: &Block) -> bool {
    b.is_air() || matches!(b, Block::Terrain(id) if blocks::props(*id).air)
}

/// The ground a player builds on in a column: down from MOTION_BLOCKING_NO_LEAVES past tree logs
/// and leaves and blocks without collision (grass, flowers), the first free y above what is left.
pub fn surface_y(w: &World, x: i32, z: i32) -> i32 {
    let mut y = w.height(super::Heightmap::MotionBlockingNoLeaves, x, z) - 1;
    while y > w.min.y {
        let b = w.get(BlockPos::new(x, y, z));
        let tree = matches!(b, Block::Terrain(id) if { let p = blocks::props(id); p.leaves_block || p.has_tag("minecraft:logs") || p.boxes.is_empty() });
        if !(tree || air_like(&b)) {
            break;
        }
        y -= 1;
    }
    y + 1
}

/// The surface height a design stands on at a spot: the highest [`surface_y`] over the footprint's
/// columns (the ground layer goes one above the ground blocks).
pub fn ground_y(w: &World, lo_xz: [i32; 2], hi_xz: [i32; 2]) -> i32 {
    let mut top = i32::MIN;
    for z in lo_xz[1]..=hi_xz[1] {
        for x in lo_xz[0]..=hi_xz[0] {
            top = top.max(surface_y(w, x, z));
        }
    }
    top
}

/// A spot a footprint can go: its min column, the ground layer's y, the blocks it digs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub lo_xz: [i32; 2],
    pub ground: i32,
    pub dug: u32,
}

/// Why spots fail, tallied over the same search as [`best_spot`] (for choosing a sample's start).
pub fn spot_tally(w: &World, size: [i32; 2], below: i32, near: [i32; 2], radius: i32) -> Vec<(String, usize)> {
    let mut tally: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let lo_xz = [near[0] + dx, near[1] + dz];
            let hi_xz = [lo_xz[0] + size[0] - 1, lo_xz[1] + size[1] - 1];
            if lo_xz[0] < w.min.x + 8 || lo_xz[1] < w.min.z + 8 || hi_xz[0] > w.min.x + w.size[0] - 9 || hi_xz[1] > w.min.z + w.size[2] - 9 {
                *tally.entry("outside the box".into()).or_default() += 1;
                continue;
            }
            let ground = ground_y(w, lo_xz, hi_xz);
            let mut low = i32::MAX;
            for z in lo_xz[1]..=hi_xz[1] {
                for x in lo_xz[0]..=hi_xz[0] {
                    low = low.min(surface_y(w, x, z));
                }
            }
            if ground - low > 2 {
                *tally.entry("ground not level".into()).or_default() += 1;
                continue;
            }
            let lo = BlockPos::new(lo_xz[0], ground - below, lo_xz[1]);
            let hi = BlockPos::new(hi_xz[0], ground, hi_xz[1]);
            let why = refusal(w, lo, hi).map(|s| {
                let s = s.split(" at ").next().unwrap_or("").to_string();
                s.split('[').next().unwrap_or("").to_string()
            });
            *tally.entry(why.unwrap_or_else(|| "passes".into())).or_default() += 1;
        }
    }
    let mut v: Vec<(String, usize)> = tally.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

/// The spot for a footprint of `size` columns whose box reaches `below` layers under its ground
/// layer: of the min columns within `radius` of `near` (the whole footprint inside the box with 8
/// columns to spare) the one that passes [`refusal`] on the flattest ground (the smallest spread of
/// MOTION_BLOCKING_NO_LEAVES heights over the footprint: a player builds on level ground), then
/// digging the fewest natural blocks, then the nearest to `near`. Deterministic: the judge and the
/// game check use it.
pub fn best_spot(w: &World, size: [i32; 2], below: i32, near: [i32; 2], radius: i32) -> Option<Spot> {
    let mut best: Option<((i32, u32, i64), Spot)> = None;
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let lo_xz = [near[0] + dx, near[1] + dz];
            let hi_xz = [lo_xz[0] + size[0] - 1, lo_xz[1] + size[1] - 1];
            if lo_xz[0] < w.min.x + 8 || lo_xz[1] < w.min.z + 8 || hi_xz[0] > w.min.x + w.size[0] - 9 || hi_xz[1] > w.min.z + w.size[2] - 9 {
                continue;
            }
            let ground = ground_y(w, lo_xz, hi_xz);
            let mut low = i32::MAX;
            for z in lo_xz[1]..=hi_xz[1] {
                for x in lo_xz[0]..=hi_xz[0] {
                    low = low.min(surface_y(w, x, z));
                }
            }
            let lo = BlockPos::new(lo_xz[0], ground - below, lo_xz[1]);
            let hi = BlockPos::new(hi_xz[0], ground, hi_xz[1]);
            // level ground only: at most 2 blocks between the footprint's highest and lowest ground
            if ground - low > 2 {
                continue;
            }
            let dug = footprint_cells(w, lo, hi).into_iter().filter(|p| !air_like(&w.get(*p))).count() as u32;
            let d2 = (dx as i64) * (dx as i64) + (dz as i64) * (dz as i64);
            let key = (ground - low, dug, d2);
            if best.as_ref().is_some_and(|(k, _)| *k <= key) {
                continue;
            }
            if refusal(w, lo, hi).is_some() {
                continue;
            }
            best = Some((key, Spot { lo_xz, ground, dug }));
        }
    }
    best.map(|b| b.1)
}
