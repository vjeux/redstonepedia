//! Line of sight (ironfarm): `LivingEntity.hasLineOfSight(target)` = `level.clip(new ClipContext(eye,
//! targetEye, ClipContext.Block.COLLIDER, ClipContext.Fluid.NONE, this)).getType() == MISS` within
//! 128 blocks. `BlockGetter.clip` visits the cells along the segment (`traverseBlocks`) and asks
//! each block's collision shape (`VoxelShape.clip`: the point 0.001 along the segment inside the
//! shape, else a box face the segment enters); fluids are ignored. Glass stops the ray (a full
//! collision box), water and a slab's empty half do not.

use crate::entity::{Aabb, Vec3, collide};
use crate::pos::BlockPos;
use crate::world::World;

/// `VoxelShape.clip(from, to, pos) != null` for a shape made of `boxes` (world coordinates).
fn shape_hits(boxes: &[Aabb], p: BlockPos, from: Vec3, to: Vec3) -> bool {
    if boxes.is_empty() {
        return false;
    }
    let diff = to.subtract_v(from);
    if diff.length_sqr() < 1.0e-7 {
        return false;
    }
    let t = from.add_v(diff.scale(0.001));
    // `isFullWide(findIndex(..))`: half-open in each axis, in the cell's own coordinates
    let (rx, ry, rz) = (t.x - p.x as f64, t.y - p.y as f64, t.z - p.z as f64);
    for b in boxes {
        let (x0, y0, z0) = (b.min_x - p.x as f64, b.min_y - p.y as f64, b.min_z - p.z as f64);
        let (x1, y1, z1) = (b.max_x - p.x as f64, b.max_y - p.y as f64, b.max_z - p.z as f64);
        if x0 <= rx && rx < x1 && y0 <= ry && ry < y1 && z0 <= rz && rz < z1 {
            return true;
        }
    }
    boxes.iter().any(|b| collide::clip(*b, from, to).is_some())
}

/// `level.clip(COLLIDER, Fluid.NONE)` between two points is a MISS: no collision box along the
/// way.
pub fn clear(w: &World, from: Vec3, to: Vec3) -> bool {
    !collide::traverse_blocks(from, to, |p| {
        let s = w.get(p);
        if s.is_air() {
            return false;
        }
        let boxes = collide::boxes_at(w, &s, p);
        shape_hits(&boxes, p, from, to)
    })
}

/// `LivingEntity.hasLineOfSight(target)` from the eye `from` to the target's eye `to` (the
/// target's x and z, its eye y): false past 128 blocks.
pub fn has_line_of_sight(w: &World, from: Vec3, to: Vec3) -> bool {
    if to.distance_to_sqr(from).sqrt() > 128.0 {
        return false;
    }
    clear(w, from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{Block, Simple};

    fn world() -> World {
        World::new(BlockPos::new(-8, -8, -8), [16, 16, 16])
    }

    #[test]
    fn glass_blocks_water_does_not() {
        let mut w = world();
        let a = Vec3::new(0.5, 1.62, 0.5);
        let b = Vec3::new(4.5, 1.74, 0.5);
        assert!(clear(&w, a, b));
        w.set_block(BlockPos::new(2, 1, 0), Block::Simple(Simple::Glass), crate::world::flags::RAW);
        assert!(!clear(&w, a, b));
        w.set_block(BlockPos::new(2, 1, 0), Block::Water { level: 0 }, crate::world::flags::RAW);
        assert!(clear(&w, a, b));
    }

    #[test]
    fn a_bottom_slab_leaves_its_top_half_open() {
        let mut w = world();
        w.set_block(BlockPos::new(2, 1, 0), Block::Slab { kind: 0 }, crate::world::flags::RAW);
        assert!(clear(&w, Vec3::new(0.5, 1.75, 0.5), Vec3::new(4.5, 1.75, 0.5)));
        assert!(!clear(&w, Vec3::new(0.5, 1.25, 0.5), Vec3::new(4.5, 1.25, 0.5)));
    }
}
