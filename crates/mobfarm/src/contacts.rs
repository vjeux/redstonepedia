//! The carts' block contacts: ONE shared definition for every judge (ironspec Nov 13 02:41 / 02:48 /
//! 02:52, G1's notes; the siding bench's face filter lifted here). A contact is a collision box the
//! cart's move was clipped by, judged the way `Entity.move` collides:
//! - the box steps along y first, then the larger horizontal axis of the move, then the other
//!   (`Entity.collideWithShapes`: z before x when |x| < |z|);
//! - on each horizontal step, a box counts when the step's box is within 0.011 of its face along that
//!   axis, on the side the move goes, and overlaps it by more than 0.01 on BOTH other axes (the
//!   vertical included), judged against the box AT THAT STEP, not after the whole move (a clip on x
//!   whose z step then slides the box off a fence post still counts);
//! - walls beside a centred cart (no motion toward them), the supports a riding or resting slope
//!   cart's box passes through (deep overlap, not a face), and a rail's support under a rolling cart
//!   (no vertical overlap) are no contacts.
//!
//! Callers pass carts whose move collided horizontally (`horizontal_collision`), each with its box
//! before and after the tick and its motion before the tick (the move's direction and axis order).

use redsim::entity::vec3::Aabb;
use redsim::pos::BlockPos;
use redsim::world::World;

/// The face tolerance along the clipped axis.
pub const FACE: f64 = 0.011;
/// The least overlap on the other two axes.
pub const OVERLAP: f64 = 0.01;

/// The collision boxes `shapes` gives that the cart's move from `before` to `after` was clipped by,
/// with `motion` (x, y, z) the move's direction (its larger horizontal component steps first).
pub fn face_contacts(shapes: &dyn Fn(Aabb) -> Vec<Aabb>, before: Aabb, after: Aabb, motion: (f64, f64, f64)) -> Vec<Aabb> {
    let (dx, dy, dz) = (after.min_x - before.min_x, after.min_y - before.min_y, after.min_z - before.min_z);
    let mut b = before.move_by(0.0, dy, 0.0);
    let z_first = motion.0.abs() < motion.2.abs();
    let mut out: Vec<Aabb> = Vec::new();
    for axis in if z_first { ['z', 'x'] } else { ['x', 'z'] } {
        b = if axis == 'x' { b.move_by(dx, 0.0, 0.0) } else { b.move_by(0.0, 0.0, dz) };
        let m = if axis == 'x' { motion.0 } else { motion.2 };
        if m.abs() < 1e-7 {
            continue;
        }
        for a in shapes(b.inflate(FACE, 0.0, FACE)) {
            let oy = b.max_y.min(a.max_y) - b.min_y.max(a.min_y);
            let (other, gap) = if axis == 'x' {
                (b.max_z.min(a.max_z) - b.min_z.max(a.min_z), if m > 0.0 { a.min_x - b.max_x } else { b.min_x - a.max_x })
            } else {
                (b.max_x.min(a.max_x) - b.min_x.max(a.min_x), if m > 0.0 { a.min_z - b.max_z } else { b.min_z - a.max_z })
            };
            if oy > OVERLAP && other > OVERLAP && gap.abs() <= FACE && !out.contains(&a) {
                out.push(a);
            }
        }
    }
    out
}

/// [`face_contacts`] on a world: each clipping box's cell (the box's centre, floored) and block.
pub fn face_contacts_world(world: &World, before: Aabb, after: Aabb, motion: (f64, f64, f64)) -> Vec<(BlockPos, String)> {
    let shapes = |bb: Aabb| world.collision_shapes(bb);
    face_contacts(&shapes, before, after, motion)
        .into_iter()
        .map(|a| {
            let p = BlockPos::new(((a.min_x + a.max_x) / 2.0).floor() as i32, ((a.min_y + a.max_y) / 2.0).floor() as i32, ((a.min_z + a.max_z) / 2.0).floor() as i32);
            (p, world.get(p).to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The boxes of full blocks at `cells`, as `World::collision_shapes` gives them (those meeting the query box).
    fn blocks(cells: &[(i32, i32, i32)]) -> impl Fn(Aabb) -> Vec<Aabb> + '_ {
        move |q: Aabb| {
            cells
                .iter()
                .map(|&(x, y, z)| Aabb::new(x as f64, y as f64, z as f64, x as f64 + 1.0, y as f64 + 1.0, z as f64 + 1.0))
                .filter(|a| a.intersects(q))
                .collect()
        }
    }

    /// A cart's box (0.98 x 0.7) bottom-centred at (x, y, z).
    fn cart(x: f64, y: f64, z: f64) -> Aabb {
        Aabb::new(x - 0.49, y, z - 0.49, x + 0.49, y + 0.7, z + 0.49)
    }

    /// Positive: a backstop in the cart's path counts (the drop entry's (x,24,5): the cart moving north
    /// stops with its box front on the glass's south face).
    #[test]
    fn a_backstop_in_the_path_counts() {
        let s = blocks(&[(3, 24, 5)]);
        let c = face_contacts(&s, cart(3.5, 24.0, 6.7), cart(3.5, 24.0, 6.49), (0.0, 0.0, -0.3));
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!((c[0].min_x, c[0].min_z), (3.0, 5.0));
    }

    /// Positive (ironspec 02:48 (b)): R58-m8's fence post. The x step clips at 8.625 while the box's z
    /// still overlaps the post (box max z -3.61 against the post's -3.625..-3.375); the z step then
    /// slides the box off. Judged at the x step it counts; the box after the whole move would miss it.
    #[test]
    fn the_fence_post_clipped_before_the_z_step_counts() {
        let post = Aabb::new(8.375, 0.0, -3.625, 8.625, 1.5, -3.375);
        let s = move |q: Aabb| if post.intersects(q) { vec![post] } else { Vec::new() };
        let before = Aabb::new(8.7, 0.0, -4.59, 9.68, 0.7, -3.61);
        let after = Aabb::new(8.625, 0.0, -4.64, 9.605, 0.7, -3.66);
        let c = face_contacts(&s, before, after, (-0.3, 0.0, -0.05));
        assert_eq!(c, vec![post]);
        // (the same move judged on the box after it: no z overlap left, so a final-box rule misses it)
        assert!(after.max_z < post.min_z);
    }

    /// Negative: a cart rolling centred between walls (a 1-wide path, its box 0.01 from each wall)
    /// counts 0.
    #[test]
    fn a_cart_between_walls_counts_0() {
        let walls: Vec<(i32, i32, i32)> = (-3..=3).flat_map(|z| [(2, 24, z), (4, 24, z)]).collect();
        let s = blocks(&walls);
        assert!(face_contacts(&s, cart(3.5, 24.0, 1.0), cart(3.5, 24.0, 0.7), (0.0, 0.0, -0.3)).is_empty());
    }

    /// Negative: a pressed queue resting on slopes counts 0 supports (a cart on an ascending rail has
    /// its box through the next slope's support: deep overlap, no face; its motion only a residue).
    #[test]
    fn a_resting_slope_queue_counts_0_supports() {
        let s = blocks(&[(3, 18, 3), (3, 19, 4), (3, 17, 2)]);
        let b = cart(3.5, 18.6, 2.9);
        assert!(face_contacts(&s, b, b, (0.0, 0.0, -0.001)).is_empty());
        assert!(face_contacts(&s, b, b, (0.0, 0.0, 0.001)).is_empty());
    }

    /// Negative (G1, ironspec 02:52): a cart rolling over flat rails on their supports counts 0 (each
    /// support's top is the cart's floor: no vertical overlap, at every cell boundary).
    #[test]
    fn a_cart_rolling_over_flat_rails_counts_0() {
        let supports: Vec<(i32, i32, i32)> = (0..=6).map(|z| (3, 23, z)).collect();
        let s = blocks(&supports);
        for y in [24.0, 24.0625] {
            assert!(face_contacts(&s, cart(3.5, y, 5.2), cart(3.5, y, 4.8), (0.0, 0.0, -0.4)).is_empty());
            assert!(face_contacts(&s, cart(3.5, y, 2.9), cart(3.5, y, 3.3), (0.0, 0.0, 0.4)).is_empty());
        }
    }
}
