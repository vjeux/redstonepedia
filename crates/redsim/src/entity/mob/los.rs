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
    first_hit(w, from, to).is_none()
}

/// The first cell whose collision shape stops the segment (`None`: [`clear`]).
pub fn first_hit(w: &World, from: Vec3, to: Vec3) -> Option<BlockPos> {
    let mut hit = None;
    collide::traverse_blocks(from, to, |p| {
        let s = w.get(p);
        if s.is_air() {
            return false;
        }
        let boxes = collide::boxes_at(w, &s, p);
        let stop = shape_hits(&boxes, p, from, to);
        if stop {
            hit = Some(p);
        }
        stop
    });
    hit
}

/// `level.clip(OUTLINE, Fluid.NONE)`: the first cell whose outline shape (`getShape`, a natural
/// state's from its row, moved by its cell's offset; a modeled block's from its game string's row,
/// else its collision boxes, reported back) stops the segment.
pub fn first_hit_outline(w: &World, from: Vec3, to: Vec3) -> (Option<BlockPos>, Option<String>) {
    let mut hit = None;
    let mut unknown: Option<String> = None;
    collide::traverse_blocks(from, to, |p| {
        let s = w.get(p);
        if s.is_air() {
            return false;
        }
        // (mob-AI 2026-11-11) a modeled block's outline from the natural-state table too (its
        // game string: a torch has an outline and no collision; cross-4-underground plains b11
        // t97646, an enderman standing in a torch could not take the dirt beside it)
        let boxes: Vec<Aabb> = match s {
            crate::block::Block::Terrain(id) => crate::terrain::blocks::props(id).outline_at(p).iter().map(|b| Aabb::new(b[0], b[1], b[2], b[3], b[4], b[5])).collect(),
            _ => match crate::terrain::blocks::id_of(&s.to_string()) {
                Some(id) => crate::terrain::blocks::props(id).outline_at(p).iter().map(|b| Aabb::new(b[0], b[1], b[2], b[3], b[4], b[5])).collect(),
                None => {
                    unknown.get_or_insert_with(|| s.to_string());
                    collide::boxes_at(w, &s, p)
                }
            },
        };
        let stop = shape_hits(&boxes, p, from, to);
        if stop {
            hit = Some(p);
        }
        stop
    });
    // a modeled block whose outline the table does not describe: its collision boxes stood in
    // (the caller names it in the run's report)
    (hit, unknown)
}

/// `LivingEntity.hasLineOfSight(target)` from the eye `from` to the target's eye `to` (the
/// target's x and z, its eye y): false past 128 blocks.
pub fn has_line_of_sight(w: &World, from: Vec3, to: Vec3) -> bool {
    if to.distance_to_sqr(from).sqrt() > 128.0 {
        return false;
    }
    clear(w, from, to)
}

/// A `BlockHitResult` of [`clip_collider`]: a miss (at `to`) or the hit point and its cell.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BlockHit {
    pub miss: bool,
    pub location: Vec3,
    pub pos: BlockPos,
}

/// `level.clip(new ClipContext(from, to, Block.COLLIDER, Fluid.NONE, entity))` with its hit point:
/// the first cell along the segment whose collision shape `VoxelShape.clip` meets (the point
/// 0.001 along the segment when it lies inside the shape, else the nearest box face over the
/// shape's boxes, `AABB.clip`), else a miss at `to` (`BlockHitResult.miss`, in `to`'s cell).
pub fn clip_collider(w: &World, from: Vec3, to: Vec3) -> BlockHit {
    let mut hit = None;
    collide::traverse_blocks(from, to, |p| {
        let s = w.get(p);
        if s.is_air() {
            return false;
        }
        let boxes = collide::boxes_at(w, &s, p);
        if boxes.is_empty() {
            return false;
        }
        let diff = to.subtract_v(from);
        if diff.length_sqr() < 1.0e-7 {
            return false;
        }
        let t = from.add_v(diff.scale(0.001));
        let (rx, ry, rz) = (t.x - p.x as f64, t.y - p.y as f64, t.z - p.z as f64);
        let inside = boxes.iter().any(|b| {
            let (x0, y0, z0) = (b.min_x - p.x as f64, b.min_y - p.y as f64, b.min_z - p.z as f64);
            let (x1, y1, z1) = (b.max_x - p.x as f64, b.max_y - p.y as f64, b.max_z - p.z as f64);
            x0 <= rx && rx < x1 && y0 <= ry && ry < y1 && z0 <= rz && rz < z1
        });
        let at = if inside { Some(t) } else { collide::clip_boxes(&boxes, from, to) };
        match at {
            Some(l) => {
                hit = Some(BlockHit { miss: false, location: l, pos: p });
                true
            }
            None => false,
        }
    });
    hit.unwrap_or_else(|| BlockHit { miss: true, location: to, pos: BlockPos::new(to.x.floor() as i32, to.y.floor() as i32, to.z.floor() as i32) })
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
