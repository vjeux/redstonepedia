//! The carts' block contacts: ONE shared definition for every judge (ironspec Nov 13 02:41 / 02:48 /
//! 02:52, amended 07:47 on G1's CHANGES 3; the siding bench's face filter lifted here). A contact is
//! an axis on which the game's collision CLIPPED the cart's ATTEMPTED move: `Entity.move(MoverType.SELF,
//! requested)` returning a different movement on that axis (`!Mth.equal`, the test `horizontalCollision`
//! uses), on the face that component pointed toward, the box located with the 0.011 / 0.01 test:
//! - the move is replayed in the game's step order: y first (the returned y), then the larger
//!   horizontal component of the REQUESTED move, then the other (`Entity.collideWithShapes`: z before
//!   x when |x| < |z|), each axis moved by its returned component;
//! - on a clipped horizontal axis, a collision box counts when the box at that step (after the
//!   returned movement on the axis) is within 0.011 of its face on the side the requested component
//!   points, and overlaps it by more than 0.01 on BOTH other axes (the vertical included), judged
//!   at that step, not after the whole move;
//! - an axis the collision did not clip is never a contact: a rail snap (`setPos`, no move), a wall
//!   beside a centred cart, the supports a resting slope cart's box passes through, a rail's support
//!   under a rolling cart;
//! - a resting cart pushed into a wall by its follower IS one (its pushed motion is the attempted
//!   move the wall clips).
//!
//! redsim records each `move(SELF, …)` per entity (`Entity::self_moves`: the box before, requested,
//! returned, the tick; `World::record_self_moves`, on by default, its no-change test in redsim);
//! [`cart_contacts`] judges a cart's moves of the world's current tick, and [`stale_declared`] the
//! declared stops no contact met. ONE contact rule (ironspec Nov 13 10:02): every bench's contact
//! counts go through these (ironfarm's siding bench, b8949c96's L1 rides, 58f770cc's C10, 7b90c3f1's
//! H14), each caller's control a planted undeclared contact failing through them.

use redsim::entity::collide::SelfMove;
use redsim::entity::vec3::Aabb;
use redsim::pos::BlockPos;
use redsim::world::World;

/// The face tolerance along the clipped axis (ironspec 02:48).
pub const FACE: f64 = 0.011;
/// The least overlap on the other two axes (ironspec 02:52).
pub const OVERLAP: f64 = 0.01;
/// The comparisons' rounding allowance: FACE and OVERLAP are decimal bounds a double can't hold
/// exactly, so a gap of exactly 0.011 counts and an overlap of exactly 0.01 doesn't (G1's edge
/// controls), whatever the last bit of the subtraction.
const EPS: f64 = 1e-9;

/// `Mth.equal(double, double)`: the game's "not clipped" test on an axis.
fn same(a: f64, b: f64) -> bool {
    (b - a).abs() < 1.0e-5f32 as f64
}

/// The collision boxes `shapes` gives that clipped the move `requested` -> `returned` of a box at
/// `before` (one `Entity.move(SELF, …)`; (x, y, z) components).
pub fn face_contacts(shapes: &dyn Fn(Aabb) -> Vec<Aabb>, before: Aabb, requested: (f64, f64, f64), returned: (f64, f64, f64)) -> Vec<Aabb> {
    let mut b = before.move_by(0.0, returned.1, 0.0);
    let z_first = requested.0.abs() < requested.2.abs();
    let mut out: Vec<Aabb> = Vec::new();
    for axis in if z_first { ['z', 'x'] } else { ['x', 'z'] } {
        let (req, ret) = if axis == 'x' { (requested.0, returned.0) } else { (requested.2, returned.2) };
        b = if axis == 'x' { b.move_by(ret, 0.0, 0.0) } else { b.move_by(0.0, 0.0, ret) };
        if same(req, ret) {
            continue;
        }
        for a in shapes(b.inflate(if axis == 'x' { FACE } else { 0.0 }, 0.0, if axis == 'z' { FACE } else { 0.0 })) {
            let oy = b.max_y.min(a.max_y) - b.min_y.max(a.min_y);
            let (other, gap) = if axis == 'x' {
                (b.max_z.min(a.max_z) - b.min_z.max(a.min_z), if req > 0.0 { a.min_x - b.max_x } else { b.min_x - a.max_x })
            } else {
                (b.max_x.min(a.max_x) - b.min_x.max(a.min_x), if req > 0.0 { a.min_z - b.max_z } else { b.min_z - a.max_z })
            };
            if oy > OVERLAP + EPS && other > OVERLAP + EPS && gap.abs() <= FACE + EPS && !out.contains(&a) {
                out.push(a);
            }
        }
    }
    out
}

/// One recorded move judged on a world: each clipping box's cell (the box's centre, floored) and
/// block.
pub fn face_contacts_world(world: &World, mv: &SelfMove) -> Vec<(BlockPos, String)> {
    let shapes = |bb: Aabb| world.collision_shapes(bb);
    face_contacts(&shapes, mv.bb_before, (mv.requested.x, mv.requested.y, mv.requested.z), (mv.returned.x, mv.returned.y, mv.returned.z))
        .into_iter()
        .map(|a| {
            let p = BlockPos::new(((a.min_x + a.max_x) / 2.0).floor() as i32, ((a.min_y + a.max_y) / 2.0).floor() as i32, ((a.min_z + a.max_z) / 2.0).floor() as i32);
            (p, world.get(p).to_string())
        })
        .collect()
}

/// The contacts of the entity at index `i` over its moves of the world's current tick (call after the
/// tick).
pub fn cart_contacts(world: &World, i: usize) -> Vec<(BlockPos, String)> {
    let t = world.time;
    world.entities.list[i].self_moves.iter().filter(|m| m.tick == t).flat_map(|m| face_contacts_world(world, m)).collect()
}

/// The declared stops no contact met (G1's CHANGES 3 (2), ironspec Nov 13 10:02 (b)): a declaration
/// that never applies is STALE and fails its row; `hits` counts a cell's contacts over the run.
pub fn stale_declared(declared: &[BlockPos], hits: &dyn Fn(BlockPos) -> usize) -> Vec<BlockPos> {
    declared.iter().copied().filter(|p| hits(*p) == 0).collect()
}

/// A design's declared contacts, strictly (G1's CHANGES 3 (3)): every `# contacts:` and `# stops:`
/// line's entries (split by the shared `pushscan::header_entries`, so a REASON may hold "; "), each
/// `x,y,z[+x,y,z…]=REASON`; a malformed cell or an entry with no '=' is an error, never dropped.
pub fn declared_contacts_strict(text: &str) -> Result<Vec<(BlockPos, String)>, String> {
    let mut out = Vec::new();
    for key in ["contacts:", "stops:"] {
        for entry in crate::pushscan::header_entries(text, key) {
            let (cells, reason) = entry.split_once('=').ok_or_else(|| format!("# {key} {entry}: x,y,z[+x,y,z…]=REASON"))?;
            // (G1's note on 329b5acc3: a declaration with an empty REASON is as bad as none)
            if reason.trim().is_empty() {
                return Err(format!("# {key} {entry}: an empty REASON (x,y,z[+x,y,z…]=REASON)"));
            }
            for cell in cells.split('+') {
                let v: Vec<i32> = cell.trim().trim_matches(|c| c == '(' || c == ')').split(',').map(|t| t.trim().replace('\u{2212}', "-").parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("# {key} {entry}: cell {cell:?}: {e}"))?;
                if v.len() != 3 {
                    return Err(format!("# {key} {entry}: cell {cell:?} is not x,y,z"));
                }
                out.push((BlockPos::new(v[0], v[1], v[2]), reason.trim().to_string()));
            }
        }
    }
    Ok(out)
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
                .filter(|a| a.intersects(q) || a.min_x == q.max_x || a.max_x == q.min_x || a.min_z == q.max_z || a.max_z == q.min_z)
                .collect()
        }
    }

    /// A cart's box (0.98 x 0.7) bottom-centred at (x, y, z).
    fn cart(x: f64, y: f64, z: f64) -> Aabb {
        Aabb::new(x - 0.49, y, z - 0.49, x + 0.49, y + 0.7, z + 0.49)
    }

    /// Positive: a backstop in the cart's path counts (the drop entry's (x,24,5): the cart moving north
    /// at 0.3 is clipped to 0.21, its box front on the glass's south face).
    #[test]
    fn a_backstop_in_the_path_counts() {
        let s = blocks(&[(3, 24, 5)]);
        let c = face_contacts(&s, cart(3.5, 24.0, 6.7), (0.0, 0.0, -0.3), (0.0, 0.0, -0.21));
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!((c[0].min_x, c[0].min_z), (3.0, 5.0));
    }

    /// Positive (ironspec 02:48 (b)): R58-m8's fence post. The x step is clipped at 8.625 while the
    /// box's z still overlaps the post (box max z -3.61 against the post's -3.625..-3.375); the z step
    /// then slides the box off. Judged at the x step it counts; the box after the whole move misses it.
    #[test]
    fn the_fence_post_clipped_before_the_z_step_counts() {
        let post = Aabb::new(8.375, 0.0, -3.625, 8.625, 1.5, -3.375);
        let s = move |q: Aabb| if post.intersects(q) || q.min_x == post.max_x { vec![post] } else { Vec::new() };
        let before = Aabb::new(8.7, 0.0, -4.59, 9.68, 0.7, -3.61);
        let c = face_contacts(&s, before, (-0.3, 0.0, -0.05), (-0.075, 0.0, -0.05));
        assert_eq!(c, vec![post]);
        let after = before.move_by(-0.075, 0.0, -0.05);
        assert!(after.max_z < post.min_z);
    }

    /// The fence post's mirror (G1's edge control 3): a |z| > |x| move steps z FIRST, clipped by a
    /// post the box still overlaps in x; the x step then slides it off. The z-first order catches it;
    /// an x-first replay would slide off first and miss it.
    #[test]
    fn z_first_order_catches_the_mirrored_post() {
        let post = Aabb::new(-3.625, 0.0, 8.375, -3.375, 1.5, 8.625);
        let s = move |q: Aabb| if post.intersects(q) || q.min_z == post.max_z { vec![post] } else { Vec::new() };
        let before = Aabb::new(-4.59, 0.0, 8.7, -3.61, 0.7, 9.68);
        let c = face_contacts(&s, before, (-0.05, 0.0, -0.3), (-0.05, 0.0, -0.075));
        assert_eq!(c, vec![post]);
        // (an x-first replay: x slides the box off the post (max x -3.66 < -3.625) before the z step)
        assert!(before.move_by(-0.05, 0.0, 0.0).max_x < post.min_x);
    }

    /// G1's edge control 1: on a clipped axis, a face exactly 0.011 away counts; 0.012 doesn't.
    #[test]
    fn the_face_gap_bound_is_0_011() {
        let wall = Aabb::new(3.0, 24.0, 4.0, 4.0, 25.0, 5.0);
        let s = move |_q: Aabb| vec![wall];
        // (the box's front stops 0.011 / 0.012 short of the wall's south face z 5.0)
        let at = |gap: f64| Aabb::new(3.01, 24.0, 5.0 + gap, 3.99, 24.7, 5.98 + gap);
        assert_eq!(face_contacts(&s, at(0.011), (0.0, 0.0, -0.2), (0.0, 0.0, 0.0)), vec![wall]);
        assert!(face_contacts(&s, at(0.012), (0.0, 0.0, -0.2), (0.0, 0.0, 0.0)).is_empty());
    }

    /// G1's edge control 2: the other-axis overlap must exceed 0.01: exactly 0.01 doesn't count,
    /// 0.0101 does.
    #[test]
    fn the_other_axis_overlap_bound_is_0_01() {
        let wall = Aabb::new(3.0, 24.0, 4.0, 4.0, 25.0, 5.0);
        let s = move |_q: Aabb| vec![wall];
        let at = |ov: f64| Aabb::new(4.0 - ov, 24.0, 5.0, 4.98 - ov, 24.7, 5.98);
        assert!(face_contacts(&s, at(0.01), (0.0, 0.0, -0.2), (0.0, 0.0, 0.0)).is_empty());
        assert_eq!(face_contacts(&s, at(0.0101), (0.0, 0.0, -0.2), (0.0, 0.0, 0.0)), vec![wall]);
    }

    /// G1's edge control 4: a contact that exists only after the y step. A cart dropping 0.5 while
    /// rolling north (its box 24.0..24.7 before, 23.5..24.2 after the y step) is clipped in z by a block
    /// at y 23..24: after the y step their vertical overlap is 0.5; judged on the box before the y step
    /// they would only touch (no vertical overlap) and it would be missed.
    #[test]
    fn a_contact_after_the_y_step_counts() {
        let low = Aabb::new(3.0, 23.0, 4.0, 4.0, 24.0, 5.0);
        let s = move |_q: Aabb| vec![low];
        let before = Aabb::new(3.01, 24.0, 5.0, 3.99, 24.7, 5.98);
        let c = face_contacts(&s, before, (0.0, -0.5, -0.2), (0.0, -0.5, 0.0));
        assert_eq!(c, vec![low]);
        // (the box before the y step meets the block's top face only: no vertical overlap)
        assert!(before.min_y >= low.max_y);
    }

    /// Negative: a centred cart rolling between walls 0.01 away (a 1-wide path): no axis clipped.
    #[test]
    fn a_cart_between_walls_counts_0() {
        let walls: Vec<(i32, i32, i32)> = (-3..=3).flat_map(|z| [(2, 24, z), (4, 24, z)]).collect();
        let s = blocks(&walls);
        assert!(face_contacts(&s, cart(3.5, 24.0, 1.0), (0.0, 0.0, -0.3), (0.0, 0.0, -0.3)).is_empty());
    }

    /// Negative (G1's control): a rail SNAP beside a wall it never touches. The snap is a `setPos`, not
    /// a move: the cart's recorded move clips nothing, so its box 0.005 from a wall's face, moving
    /// toward it, counts 0 (the old motion-toward-the-face rule would have counted it).
    #[test]
    fn a_snap_beside_a_wall_counts_0() {
        let wall = Aabb::new(3.0, 24.0, 4.0, 4.0, 25.0, 5.0);
        let s = move |_q: Aabb| vec![wall];
        let b = Aabb::new(3.01, 24.0, 5.005, 3.99, 24.7, 5.985);
        assert!(face_contacts(&s, b, (0.0, 0.0, -0.004), (0.0, 0.0, -0.004)).is_empty());
    }

    /// Negative: a pressed queue resting on slopes counts 0 supports (no axis clipped; the box passes
    /// through the next slope's support: deep overlap, no face).
    #[test]
    fn a_resting_slope_queue_counts_0_supports() {
        let s = blocks(&[(3, 18, 3), (3, 19, 4), (3, 17, 2)]);
        let b = cart(3.5, 18.6, 2.9);
        assert!(face_contacts(&s, b, (0.0, 0.0, -0.001), (0.0, 0.0, -0.001)).is_empty());
        assert!(face_contacts(&s, b, (0.0, 0.0, 0.001), (0.0, 0.0, 0.001)).is_empty());
    }

    /// Negative (G1, ironspec 02:52): a cart rolling over flat rails on their supports counts 0.
    #[test]
    fn a_cart_rolling_over_flat_rails_counts_0() {
        let supports: Vec<(i32, i32, i32)> = (0..=6).map(|z| (3, 23, z)).collect();
        let s = blocks(&supports);
        for y in [24.0, 24.0625] {
            assert!(face_contacts(&s, cart(3.5, y, 5.2), (0.0, 0.0, -0.4), (0.0, 0.0, -0.4)).is_empty());
            assert!(face_contacts(&s, cart(3.5, y, 2.9), (0.0, 0.0, 0.4), (0.0, 0.0, 0.4)).is_empty());
        }
    }

    /// Positive through redsim (G1's control): a resting cart pushed into a wall by its follower. Cart
    /// A rests on a flat north_south track against a stone wall at its north end; cart B rolls north
    /// into it. A's pushed motion is an attempted move the wall clips: A's recorded moves count the
    /// wall.
    #[test]
    fn a_resting_cart_pushed_into_a_wall_counts() {
        use redsim::block::Block;
        let mut w = World::new(BlockPos::new(-2, 0, -4), [6, 4, 16]);
        let stone = Block::parse("minecraft:stone").unwrap();
        let rail = Block::parse("minecraft:rail[shape=north_south,waterlogged=false]").unwrap();
        for z in -3..12 {
            w.set_block(BlockPos::new(0, 0, z), stone.clone(), redsim::world::flags::RAW);
        }
        for z in 0..12 {
            w.set_block(BlockPos::new(0, 1, z), rail.clone(), redsim::world::flags::RAW);
        }
        w.set_block(BlockPos::new(0, 1, -1), stone.clone(), redsim::world::flags::RAW);
        let a = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0, 0.5], Some("{Motion:[0.0d,0.0d,0.0d]}")).unwrap();
        let _b = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0, 3.5], Some("{Motion:[0.0d,0.0d,-0.4d]}")).unwrap();
        let mut hit = Vec::new();
        for _ in 0..40 {
            w.tick();
            let Some(i) = w.entities.list.iter().position(|e| e.id == a && !e.removed) else { break };
            hit.extend(cart_contacts(&w, i));
        }
        assert!(hit.iter().any(|(p, _)| *p == BlockPos::new(0, 1, -1)), "{hit:?}");
        // (ironspec 10:02 (b), the stale class through the same record: the wall, declared, is hit;
        // a declared cell no cart ever met reads stale)
        let count = |p: BlockPos| hit.iter().filter(|(q, _)| *q == p).count();
        assert_eq!(stale_declared(&[BlockPos::new(0, 1, -1), BlockPos::new(0, 1, 11)], &count), vec![BlockPos::new(0, 1, 11)]);
    }

    /// The strict `# contacts:` reader (G1's CHANGES 3 (3)): every entry of every line, cells joined
    /// by '+'; a malformed cell, a 2-number cell or an entry with no '=' is an error.
    #[test]
    fn declared_contacts_read_strictly() {
        let ok = "# contacts: -1,24,5+3,24,5+7,24,5=the drop entry's backstop; a REASON with \"; \" in it; 2,0,4=another\n";
        let d = declared_contacts_strict(ok).unwrap();
        assert_eq!(d.len(), 4);
        assert_eq!(d[1].0, BlockPos::new(3, 24, 5));
        assert_eq!(d[3], (BlockPos::new(2, 0, 4), "another".to_string()));
        assert!(declared_contacts_strict("# contacts: -1,24,x=bad\n").is_err());
        assert!(declared_contacts_strict("# contacts: -1,24=short\n").is_err());
        assert!(declared_contacts_strict("# contacts: -1,24,5\n").is_err());
        assert!(declared_contacts_strict("# contacts: -1,24,5=\n").is_err());
        assert!(declared_contacts_strict("# contacts: -1,24,5=   \n").is_err());
    }
}
