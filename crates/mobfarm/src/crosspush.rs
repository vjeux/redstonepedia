//! Cross-track pushes (ironspec Nov 11 12:40, a farm-wide hazard found in S3's router: B's branch
//! running back beside A's approach). A minecart pushes every cart whose box meets its own box
//! inflated 0.2 sideways (`OldMinecartBehavior.pushAndPickupEntities`, redsim
//! `minecart::push_and_pickup_entities` and `push_by`), so carts on two level tracks whose centres
//! pass under about 1.18 apart push each other sideways, and each rail turns its push into motion
//! along it: a train's carts thrown back, a branch's cart stopped. A push between carts on the same
//! track (a train pressed together, a cart following another through a junction's curve) is the
//! train's own; between carts on different tracks it fails the run, unless the design declares the
//! rail cell harmless (`cross_push_ok:`, with its bridged rows). Merged pairs (the push skipped,
//! centres under 0.01 apart) are `crate::merge`'s.

use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::pos::BlockPos;
use redsim::world::World;

/// Rail connections within which two carts' rail cells count as one track: two carts on one line
/// or slope whose inflated boxes meet (centres under 1.18 apart) sit at most 2 cells apart, and a
/// cart following another through a junction's curve reaches it within 2.
pub const SAME_TRACK_STEPS: usize = 2;

/// The push skip's threshold on the squared horizontal distance (`1.0E-4F`, as a float).
const PUSH_SKIP_DD: f64 = 1.0e-4f32 as f64;

/// Whether the cart push pairs two carts: `a`'s box inflated 0.2 sideways meets `b`'s (the
/// inflation is sideways only, so it is symmetric) and the centres are at least 0.01 apart
/// horizontally (else the push skips them: a merged pair).
pub fn pushes(pa: Vec3, bba: Aabb, pb: Vec3, bbb: Aabb) -> bool {
    let dd = (pb.x - pa.x) * (pb.x - pa.x) + (pb.z - pa.z) * (pb.z - pa.z);
    dd >= PUSH_SKIP_DD && bba.inflate(0.2, 0.0, 0.2).intersects(bbb)
}

/// Whether a paired push moves the pushed cart `this` (at `pt`, yaw `yrot`) away from `other` (at
/// `po`): `AbstractMinecart.pushOtherMinecart` acts only when the direction to the other cart lies
/// within the pushed cart's facing, |direction . facing| >= 0.8 (redsim `minecart::push_other_minecart`;
/// tick-exact's side-push bridge: a cart whose yaw follows its rail is never pushed from a block to
/// the side, while a stale yaw across the rail, a placed cart's 0 or a creeping one's, is).
pub fn push_acts(pt: Vec3, yrot: f32, po: Vec3) -> bool {
    let dir = Vec3::new(po.x - pt.x, 0.0, po.z - pt.z).normalize();
    let r = yrot * ((std::f64::consts::PI / 180.0) as f32);
    let facing = Vec3::new(redsim::entity::mth::cos(r as f64) as f64, 0.0, redsim::entity::mth::sin(r as f64) as f64).normalize();
    dir.dot(facing).abs() >= 0.8f32 as f64
}

/// The rail cell a cart at `p` runs on: the rail in its own cell, else in the cell below
/// (`AbstractMinecart.getCurrentBlockPosOrRailBelow`); none off the rails (a flight).
pub fn rail_cell(w: &World, p: Vec3) -> Option<BlockPos> {
    let c = BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    [c, c.below()].into_iter().find(|q| redsim::rails::is_rail(&w.get(*q)))
}

/// The rail cells a rail cell's shape connects to: each connection (a slope's high end one up), the
/// rail found there or one down or up, as the game's `RailState` looks for it.
fn neighbours(w: &World, c: BlockPos) -> Vec<BlockPos> {
    let b = w.get(c);
    if !redsim::rails::is_rail(&b) {
        return Vec::new();
    }
    let st = redsim::rails::RailState::new(c, b);
    st.connections().iter().filter_map(|n| [*n, n.below(), n.above()].into_iter().find(|q| redsim::rails::is_rail(&w.get(*q)))).collect()
}

/// Whether rail cell `b` is within `steps` connections of `a`, from either end (a junction's soft
/// connections followed as each rail's own shape gives them).
pub fn same_track(w: &World, a: BlockPos, b: BlockPos, steps: usize) -> bool {
    let reach = |from: BlockPos, to: BlockPos| -> bool {
        if from == to {
            return true;
        }
        let mut seen = vec![from];
        let mut front = vec![from];
        for _ in 0..steps {
            let mut next = Vec::new();
            for c in front {
                for n in neighbours(w, c) {
                    if n == to {
                        return true;
                    }
                    if !seen.contains(&n) {
                        seen.push(n);
                        next.push(n);
                    }
                }
            }
            front = next;
        }
        false
    };
    reach(a, b) || reach(b, a)
}

/// One tick's cross-track pair: (id, id, rail cell, rail cell, the centres' horizontal distance,
/// whether the push acts on either cart).
pub type CrossPair = (u32, u32, BlockPos, BlockPos, f64, bool);

/// The yaw a cart moving along a rail of this shape faces, modulo 180 (`atan2` of the axis between
/// the shape's two exits, as `OldMinecartBehavior` sets a moving cart's `yRot`): 90 along z, 0
/// along x, 45 or 135 on a curve.
pub fn rail_yaw(s: redsim::block::RailShape) -> f32 {
    let (a, b) = redsim::rails::exits(s);
    let (dx, dz) = ((b[0] - a[0]) as f64, (b[2] - a[2]) as f64);
    (dz.atan2(dx) * 180.0 / std::f64::consts::PI) as f32
}

/// Every minecart on a rail turned to face along it, as a cart that arrived there faces (ironspec
/// Nov 11 12:48: a placed cart faces +x, yaw 0, and at rest with that yaw across its rail it is
/// pushed along it by a cart passing beside it, tick-exact's side-push bridge). Returns how many
/// carts turned; carts off the rails keep theirs.
pub fn yaw_along_rails(w: &mut World) -> usize {
    let mut turned = 0;
    for i in 0..w.entities.list.len() {
        let e = &w.entities.list[i];
        if e.removed || !matches!(e.kind, EntityKind::Minecart(_)) {
            continue;
        }
        let Some(c) = rail_cell(w, e.pos) else { continue };
        let yaw = rail_yaw(redsim::rails::shape_of(&w.get(c)));
        let e = &mut w.entities.list[i];
        if e.y_rot != yaw {
            turned += 1;
        }
        e.y_rot = yaw;
        e.y_rot_o = yaw;
    }
    turned
}

/// Every minecart at yaw 0 (facing +x), as a player-placed or summoned cart starts whatever its
/// rail (tick-exact; `--placed-yaw`, the assembly's start-up row).
pub fn yaw_placed(w: &mut World) {
    for e in w.entities.list.iter_mut().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
        e.y_rot = 0.0;
        e.y_rot_o = 0.0;
    }
}

/// This tick's cross-track contacts between live minecarts, each with whether the push acts on
/// either cart (`push_acts`); ids in list order; pairs off the rails are left out (a flight).
pub fn cross_pairs(w: &World) -> Vec<CrossPair> {
    let carts: Vec<(u32, Vec3, Aabb, f32)> = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))).map(|e| (e.id, e.pos, e.bb, e.y_rot)).collect();
    let mut out = Vec::new();
    for i in 0..carts.len() {
        for j in i + 1..carts.len() {
            let (a, b) = (&carts[i], &carts[j]);
            if !pushes(a.1, a.2, b.1, b.2) {
                continue;
            }
            let (Some(ra), Some(rb)) = (rail_cell(w, a.1), rail_cell(w, b.1)) else { continue };
            if same_track(w, ra, rb, SAME_TRACK_STEPS) {
                continue;
            }
            let d = ((b.1.x - a.1.x).powi(2) + (b.1.z - a.1.z).powi(2)).sqrt();
            let acts = push_acts(a.1, a.3, b.1) || push_acts(b.1, b.3, a.1);
            out.push((a.0, b.0, ra, rb, d, acts));
        }
    }
    out
}

/// A run's cross-track push tally, fed every tick and printed as one hard line.
#[derive(Default)]
pub struct CrossPushTally {
    /// The distinct pairs whose push acted across tracks outside the declared cells (ids).
    pub pairs: Vec<(u32, u32)>,
    /// Cart-pair ticks with an acting cross-track push outside the declared cells.
    pub ticks: u64,
    /// Cart-pair ticks with boxes met across tracks but no push acting (both yaws along their
    /// rails): report only.
    pub contact_ticks: u64,
    /// Cart-pair ticks with an acting cross-track push at a declared cell (`cross_push_ok:`).
    pub declared_ticks: u64,
    /// The first few cases, as text (where and when).
    pub first: Vec<String>,
}

impl CrossPushTally {
    /// Records this tick's cross-track pushes. `ok`: the rail cells the design declares harmless
    /// (world coordinates); `when` names the tick; (dx, dz) the design's shift, taken off the cells
    /// printed.
    pub fn observe(&mut self, w: &World, ok: &[[i32; 3]], when: &str, dx: i32, dz: i32) {
        for (a, b, ra, rb, d, acts) in cross_pairs(w) {
            if !acts {
                self.contact_ticks += 1;
                continue;
            }
            if ok.iter().any(|c| *c == [ra.x, ra.y, ra.z] || *c == [rb.x, rb.y, rb.z]) {
                self.declared_ticks += 1;
                continue;
            }
            self.ticks += 1;
            if !self.pairs.contains(&(a, b)) {
                self.pairs.push((a, b));
                if self.first.len() < 3 {
                    self.first.push(format!("carts {a} and {b} at {when} on rails {},{},{} and {},{},{}, centres {d:.3} apart", ra.x - dx, ra.y, ra.z - dz, rb.x - dx, rb.y, rb.z - dz));
                }
            }
        }
    }

    /// The hard line: the pairs, the cart-pair ticks, the declared ones and the contacts with no
    /// push acting, the first cases.
    pub fn line(&self) -> String {
        format!(
            "cross-track pushes (carts on different tracks whose boxes meet inflated 0.2 sideways and whose push acts, |direction . facing| >= 0.8; ironspec Nov 11 12:40): {} pairs over {} cart-pair ticks{}; contacts with no push acting (yaw along the rail): {} cart-pair ticks{}",
            self.pairs.len(),
            self.ticks,
            if self.declared_ticks > 0 { format!(", {} more at declared cross_push_ok: cells", self.declared_ticks) } else { String::new() },
            self.contact_ticks,
            if self.first.is_empty() { String::new() } else { format!(" ({})", self.first.join("; ")) }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redsim::block::Block;

    /// A minecart's box (0.98 wide, 0.7 tall) at a centre-bottom position.
    fn cart(x: f64, y: f64, z: f64) -> (Vec3, Aabb) {
        (Vec3::new(x, y, z), Aabb::new(x - 0.49, y, z - 0.49, x + 0.49, y + 0.7, z + 0.49))
    }

    fn rail(shape: &str) -> Block {
        Block::parse(&format!("minecraft:rail[shape={shape},waterlogged=false]")).unwrap()
    }

    /// Two straight tracks one block apart along z (x 0 and x 1, z 0..9), a slope line beside them
    /// (x 4, rising north from z 9 to z 5), and a junction (x 7: a south_west curve at z 3 whose west
    /// leg runs west along z 3).
    fn world() -> World {
        let mut w = World::new(BlockPos::new(-4, 0, -4), [20, 12, 20]);
        let stone = Block::parse("minecraft:stone").unwrap();
        for x in -3..14 {
            for z in -3..14 {
                w.set_block(BlockPos::new(x, 0, z), stone.clone(), redsim::world::flags::RAW);
            }
        }
        for z in 0..10 {
            for x in [0, 1] {
                w.set_block(BlockPos::new(x, 1, z), rail("north_south"), redsim::world::flags::RAW);
            }
        }
        for (k, z) in (5..10).rev().enumerate() {
            // (ascending north: the cell at z rises to z - 1; supports under each)
            for y in 1..(1 + k as i32) {
                w.set_block(BlockPos::new(4, y, z), stone.clone(), redsim::world::flags::RAW);
            }
            w.set_block(BlockPos::new(4, 1 + k as i32, z), rail("ascending_north"), redsim::world::flags::RAW);
        }
        w.set_block(BlockPos::new(7, 1, 3), rail("south_west"), redsim::world::flags::RAW);
        for z in 4..8 {
            w.set_block(BlockPos::new(7, 1, z), rail("north_south"), redsim::world::flags::RAW);
        }
        for x in 4..7 {
            w.set_block(BlockPos::new(x, 1, 3), rail("east_west"), redsim::world::flags::RAW);
        }
        w
    }

    #[test]
    fn side_by_side_tracks_push_across() {
        // the positive control: carts on tracks a block apart, abreast (centres 1.0 apart)
        let w = world();
        let (pa, ba) = cart(0.5, 1.0625, 4.5);
        let (pb, bb) = cart(1.5, 1.0625, 4.7);
        assert!(pushes(pa, ba, pb, bb) && pushes(pb, bb, pa, ba));
        let (ra, rb) = (rail_cell(&w, pa).unwrap(), rail_cell(&w, pb).unwrap());
        assert_eq!((ra, rb), (BlockPos::new(0, 1, 4), BlockPos::new(1, 1, 4)));
        assert!(!same_track(&w, ra, rb, SAME_TRACK_STEPS));
        // and 1.2 apart they no longer meet
        let (pc, bc) = cart(1.7, 1.0625, 4.5);
        assert!(!pushes(pa, ba, pc, bc));
    }

    #[test]
    fn a_pressed_train_is_one_track() {
        let w = world();
        // on one straight line, 0.79 apart and 1.15 apart (cells 4/5 and 4/5 or 6)
        for (z1, z2) in [(4.5, 5.29), (4.9, 6.05)] {
            let (pa, ba) = cart(0.5, 1.0625, z1);
            let (pb, bb) = cart(0.5, 1.0625, z2);
            assert!(pushes(pa, ba, pb, bb));
            let (ra, rb) = (rail_cell(&w, pa).unwrap(), rail_cell(&w, pb).unwrap());
            assert!(same_track(&w, ra, rb, SAME_TRACK_STEPS), "{ra:?} {rb:?}");
        }
        // pressed on the slope: cells (4,3,7) and (4,4,6), a level apart
        let (ra, rb) = (BlockPos::new(4, 3, 7), BlockPos::new(4, 4, 6));
        assert!(redsim::rails::is_rail(&w.get(ra)) && redsim::rails::is_rail(&w.get(rb)));
        assert!(same_track(&w, ra, rb, SAME_TRACK_STEPS));
        // through the junction's curve: the straight leg's cell next to the curve, the west leg's
        assert!(same_track(&w, BlockPos::new(7, 1, 4), BlockPos::new(6, 1, 3), SAME_TRACK_STEPS));
        // but three connections apart is not one track at this reach
        assert!(!same_track(&w, BlockPos::new(7, 1, 5), BlockPos::new(5, 1, 3), SAME_TRACK_STEPS));
    }

    #[test]
    fn stacked_tracks_do_not_meet_and_merged_pairs_are_not_pushes() {
        // a block apart vertically: the boxes don't meet (no vertical inflation)
        let (pa, ba) = cart(0.5, 1.0625, 4.5);
        let (pb, bb) = cart(0.5, 2.0625, 4.6);
        assert!(!pushes(pa, ba, pb, bb));
        // centres under 0.01 apart: the push skips them (`crate::merge` counts them)
        let (pc, bc) = cart(0.5, 1.0625, 4.505);
        assert!(!pushes(pa, ba, pc, bc));
    }

    #[test]
    fn a_side_push_acts_only_on_a_yaw_across_the_rail() {
        // carts abreast on north-south rails a block apart: the push direction is along x
        let (pa, pb) = (Vec3::new(0.5, 1.0625, 4.5), Vec3::new(1.5, 1.0625, 4.6));
        // facing along their rails (yaw 90, along z): never pushed from the side (tick-exact)
        assert!(!push_acts(pa, 90.0, pb) && !push_acts(pb, 90.0, pa));
        // a placed cart's yaw 0 faces +x, across its rail: pushed
        assert!(push_acts(pa, 0.0, pb));
        // the rails' own yaws: z 90, x 0, curves 45 or 135 (modulo 180)
        use redsim::block::RailShape;
        assert_eq!(rail_yaw(RailShape::NorthSouth), 90.0);
        assert_eq!(rail_yaw(RailShape::EastWest), 0.0);
        assert_eq!(rail_yaw(RailShape::AscendingNorth).rem_euclid(180.0), 90.0);
        let c = rail_yaw(RailShape::SouthEast).rem_euclid(180.0);
        assert!(c == 45.0 || c == 135.0, "{c}");
        // and turned along its rail, the placed cart is no longer pushed
        let mut w = world();
        let id = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0625, 4.5], None).unwrap();
        assert!(yaw_along_rails(&mut w) >= 1);
        let e = w.entities.list.iter().find(|e| e.id == id).unwrap();
        assert!(!push_acts(e.pos, e.y_rot, pb));
        yaw_placed(&mut w);
        let e = w.entities.list.iter().find(|e| e.id == id).unwrap();
        assert!(push_acts(e.pos, e.y_rot, pb));
    }
}
