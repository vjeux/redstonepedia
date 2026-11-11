//! Merged carts: the one shared check every bench calls (tail-bench, queue-loop-run, the siding
//! bench, day-1's runner). Two minecarts are merged when the game's cart push would pair them but
//! never separates them: the minecart's own collector (`OldMinecartBehavior.pushAndPickupEntities`) collects the entities whose box meets this cart's
//! box inflated 0.2 sideways (`getBoundingBox().inflate(0.2, 0.0, 0.2)`), and
//! `AbstractMinecart.push` does nothing when the squared horizontal distance between the two
//! centres is under 1.0E-4 (redsim: `minecart::push_by`). So a merged pair is two carts whose
//! horizontal distance is under 0.01 and whose boxes meet once inflated sideways; such a pair stays
//! merged and later swaps order (the brief's "No merged carts" must-have: a hard fail anywhere in
//! the loop). Carts on stacked tracks a block apart don't meet vertically, so they are not merged.

use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::world::World;

/// The push skip's threshold on the squared horizontal distance (`1.0E-4F`, as a float).
const PUSH_SKIP_DD: f64 = 1.0e-4f32 as f64;

/// Whether two carts (centres and boxes) are merged: the push would pair them (`a`'s box inflated
/// 0.2 sideways meets `b`'s) and skips them (squared horizontal distance under 1.0E-4).
pub fn is_merged(pa: Vec3, bba: Aabb, pb: Vec3, bbb: Aabb) -> bool {
    let dd = (pb.x - pa.x) * (pb.x - pa.x) + (pb.z - pa.z) * (pb.z - pa.z);
    !(dd >= PUSH_SKIP_DD) && bba.inflate(0.2, 0.0, 0.2).intersects(bbb)
}

/// Every merged pair of live minecarts in the world at this tick end: (id, id, the first cart's
/// position), ids in list order.
pub fn merged_pairs(w: &World) -> Vec<(u32, u32, Vec3)> {
    let carts: Vec<(u32, Vec3, Aabb)> = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))).map(|e| (e.id, e.pos, e.bb)).collect();
    let mut out = Vec::new();
    for i in 0..carts.len() {
        for j in i + 1..carts.len() {
            let (a, b) = (&carts[i], &carts[j]);
            if is_merged(a.1, a.2, b.1, b.2) {
                out.push((a.0, b.0, a.1));
            }
        }
    }
    out
}

/// A run's merged-carts tally, for a bench to feed every tick and print as one hard line.
#[derive(Default)]
pub struct MergeTally {
    /// The distinct merged pairs seen (ids).
    pub pairs: Vec<(u32, u32)>,
    /// The first few cases, as text (where and when).
    pub first: Vec<String>,
}

impl MergeTally {
    /// Records this tick's merged pairs; `when` names the tick for the first cases.
    pub fn observe(&mut self, w: &World, when: &str) {
        for (a, b, p) in merged_pairs(w) {
            if !self.pairs.contains(&(a, b)) {
                self.pairs.push((a, b));
                if self.first.len() < 3 {
                    self.first.push(format!("carts {a} and {b} at {when} ({:.2},{:.2},{:.2})", p.x, p.y, p.z));
                }
            }
        }
    }

    /// The hard line: the count of merged pairs and the first cases.
    pub fn line(&self) -> String {
        format!("merged carts (horizontal distance under 0.01, boxes paired by the push; the game never separates them): {} {}", self.pairs.len(), self.first.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minecart's box (0.98 wide, 0.7 tall) at a centre-bottom position.
    fn cart(x: f64, y: f64, z: f64) -> (Vec3, Aabb) {
        (Vec3::new(x, y, z), Aabb::new(x - 0.49, y, z - 0.49, x + 0.49, y + 0.7, z + 0.49))
    }

    #[test]
    fn two_carts_0_005_apart_are_merged() {
        // the positive control: two carts on one rail 0.005 apart
        let (pa, ba) = cart(0.5, 1.0625, 30.090);
        let (pb, bb) = cart(0.5, 1.0625, 30.095);
        assert!(is_merged(pa, ba, pb, bb));
        assert!(is_merged(pb, bb, pa, ba));
    }

    #[test]
    fn carts_on_stacked_tracks_a_block_apart_are_not_merged() {
        // same x and z, one track a block above the other: the boxes don't meet vertically
        let (pa, ba) = cart(0.5, 1.0625, 30.09);
        let (pb, bb) = cart(0.5, 2.0625, 30.09);
        assert!(!is_merged(pa, ba, pb, bb));
        assert!(!is_merged(pb, bb, pa, ba));
    }

    #[test]
    fn carts_in_contact_on_a_slope_are_not_merged() {
        // a pressed queue on a slope: 0.79 apart horizontally, 0.79 apart vertically
        let (pa, ba) = cart(0.5, 3.18, 30.88);
        let (pb, bb) = cart(0.5, 3.97, 30.09);
        assert!(!is_merged(pa, ba, pb, bb));
        // and 0.02 apart on one level: the push still separates them
        let (pc, bc) = cart(0.5, 1.0625, 30.11);
        let (pd, bd) = cart(0.5, 1.0625, 30.09);
        assert!(!is_merged(pc, bc, pd, bd));
    }
}
