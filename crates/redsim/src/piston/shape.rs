//! The collision (= support) shapes of the piston blocks, in 1/16 block units, and the face
//! tests of `isFaceSturdy` (`SupportType.FULL / CENTER / RIGID`) on them: `getFaceShape` takes
//! the voxel layer just inside the face (`findIndex(axis, 0.9999999 | 1e-7)`), extrudes it
//! through the cell, and compares it with the block (FULL: identical), the center column
//! (`Block.column(2, 0, 10)`) or the rigid ring (block minus `box(2, 0, 2, 14, 16, 14)`).
//! Moving pistons take their shape from the block entity (`PistonMovingBlockEntity
//! .getCollisionShape`): the moved block offset by the progress, a retracting base keeping its
//! extended base box.

use crate::behavior::Support;
use crate::block::Block;
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// An axis-aligned box in 1/16 units: [x0, x1) x [y0, y1) x [z0, z1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct B16(pub [i32; 6]);

impl B16 {
    pub fn full() -> B16 {
        B16([0, 16, 0, 16, 0, 16])
    }
    pub fn shifted(self, d: [i32; 3]) -> B16 {
        let b = self.0;
        B16([b[0] + d[0], b[1] + d[0], b[2] + d[1], b[3] + d[1], b[4] + d[2], b[5] + d[2]])
    }
}

fn axis_index(d: Dir) -> usize {
    match d {
        Dir::West | Dir::East => 0,
        Dir::Down | Dir::Up => 1,
        Dir::North | Dir::South => 2,
    }
}

fn positive(d: Dir) -> bool {
    matches!(d, Dir::East | Dir::Up | Dir::South)
}

/// A box spanning [lo, hi) along `facing`'s axis (in the frame where the face is at 0 for a
/// negative facing), full on the other two axes.
fn along(facing: Dir, lo: i32, hi: i32, others: (i32, i32)) -> B16 {
    let a = axis_index(facing);
    let mut b = [others.0, others.1, others.0, others.1, others.0, others.1];
    // positive facings mirror the range: [16 - hi, 16 - lo)
    let (l, h) = if positive(facing) { (16 - hi, 16 - lo) } else { (lo, hi) };
    b[2 * a] = l;
    b[2 * a + 1] = h;
    B16(b)
}

/// The extended base: the 12 px box behind the head.
pub fn base_extended(facing: Dir) -> Vec<B16> {
    vec![along(facing, 4, 16, (0, 16))]
}

/// `PistonHeadBlock` shapes: the 4 px plate at the front, the 4x4 arm behind it (20 px long, or
/// 16 px when short).
pub fn head(facing: Dir, short: bool) -> Vec<B16> {
    vec![along(facing, 0, 4, (0, 16)), along(facing, 4, if short { 16 } else { 20 }, (6, 10))]
}

/// A moved block's own collision shape (the empty collision context).
pub fn block_shape(b: Block) -> Vec<B16> {
    match b {
        Block::Piston { extended: false, .. } => vec![B16::full()],
        Block::Piston { facing, extended: true, .. } => base_extended(facing),
        Block::PistonHead { facing, short, .. } => head(facing, short),
        // every other block: its collision shape (redmodel's table: honey, soul sand, mud,
        // trapdoors, slabs, fence gates, scaffolding...; empty for air)
        _ => crate::behavior::collision_boxes(&b).into_iter().map(B16).collect(),
    }
}

/// [`block_shape`] in blocks, exact: a terrain state's boxes straight from the game's table (an
/// iron chain's 6.5..9.5 px column is not a whole number of pixels: tickexact 2026-11-11, cart-pole),
/// every other block's from its 1/16 boxes. Boxes as [x0, y0, z0, x1, y1, z1].
pub fn block_shape_exact(b: Block) -> Vec<[f64; 6]> {
    if let Block::Terrain(id) = b {
        return crate::terrain::blocks::props(id).boxes.clone();
    }
    let f = |v: i32| v as f64 / 16.0;
    block_shape(b).into_iter().map(|c| [f(c.0[0]), f(c.0[2]), f(c.0[4]), f(c.0[1]), f(c.0[3]), f(c.0[5])]).collect()
}

/// [`moving_noclip`] in blocks, exact ([`block_shape_exact`] for the moved block), relative to
/// the cell, boxes as [x0, y0, z0, x1, y1, z1].
pub fn moving_noclip_exact(w: &World, p: BlockPos, noclip: Option<Dir>) -> Vec<[f64; 6]> {
    let Some(be) = w.piston.be(p) else { return vec![] };
    let f = |c: B16| -> [f64; 6] {
        let g = |v: i32| v as f64 / 16.0;
        [g(c.0[0]), g(c.0[2]), g(c.0[4]), g(c.0[1]), g(c.0[3]), g(c.0[5])]
    };
    let mut out = Vec::new();
    if !be.extending && be.source {
        if let Block::Piston { facing, .. } = be.moved {
            out.extend(base_extended(facing).into_iter().map(f));
        }
    }
    if be.progress < 2 && noclip == Some(be.push_direction()) {
        return out;
    }
    let prog16 = be.progress as i32 * 8;
    let moved: Vec<[f64; 6]> = if be.source {
        let short = be.extending != (16 - prog16 < 4);
        head(be.direction, short).into_iter().map(f).collect()
    } else {
        block_shape_exact(be.moved)
    };
    let ext = (if be.extending { prog16 - 16 } else { 16 - prog16 }) as f64 / 16.0;
    let st = be.direction.step();
    let (dx, dy, dz) = (st[0] as f64 * ext, st[1] as f64 * ext, st[2] as f64 * ext);
    out.extend(moved.into_iter().map(|b| [b[0] + dx, b[1] + dy, b[2] + dz, b[3] + dx, b[4] + dy, b[5] + dz]));
    out
}

/// `PistonMovingBlockEntity.getCollisionShape` (no entity being moved: no no-clip direction).
pub fn moving(w: &World, p: BlockPos) -> Vec<B16> {
    moving_noclip(w, p, None)
}

/// `PistonMovingBlockEntity.getCollisionShape` while an entity is moved by a piston in direction
/// `noclip` (`NOCLIP`, set by `moveEntityByPiston`): a block still moving that way keeps only
/// the extended base box of a retracting base (entity pushing, clk-misc).
pub fn moving_noclip(w: &World, p: BlockPos, noclip: Option<Dir>) -> Vec<B16> {
    let Some(be) = w.piston.be(p) else { return vec![] };
    let mut out = Vec::new();
    if !be.extending && be.source {
        if let Block::Piston { facing, .. } = be.moved {
            out.extend(base_extended(facing));
        }
    }
    if be.progress < 2 && noclip == Some(be.push_direction()) {
        return out;
    }
    let prog16 = be.progress as i32 * 8;
    let moved = if be.source {
        // PISTON_HEAD facing `direction`, short = extending != (1 - progress < 0.25)
        let short = be.extending != (16 - prog16 < 4);
        head(be.direction, short)
    } else {
        block_shape(be.moved)
    };
    let ext = if be.extending { prog16 - 16 } else { 16 - prog16 };
    let st = be.direction.step();
    out.extend(moved.into_iter().map(|b| b.shifted([st[0] * ext, st[1] * ext, st[2] * ext])));
    out
}

/// The collision shape of a piston block.
pub fn of(w: &World, s: Block, p: BlockPos) -> Vec<B16> {
    match s {
        Block::Piston { extended: false, .. } => vec![B16::full()],
        Block::Piston { facing, extended: true, .. } => base_extended(facing),
        Block::PistonHead { facing, short, .. } => head(facing, short),
        Block::MovingPiston { .. } => moving(w, p),
        _ => vec![],
    }
}

/// `isFaceSturdy(face, support)` of a shape: the covering boxes' slices (the layer just inside
/// the face, extruded) against the needed cells, box by box (no 64x64 raster: a big farm's
/// harvest, 256 pistons in one tick, spent 44 ms of a 46 ms tick filling and scanning it; the
/// raster's answer, the same bit for bit, is kept as a test's oracle: `face_supports_grid`).
pub fn face_supports(boxes: &[B16], face: Dir, support: Support) -> bool {
    let a = axis_index(face);
    // the layer just inside the face
    let covers = |b: &B16| {
        let (lo, hi) = (b.0[2 * a], b.0[2 * a + 1]);
        if positive(face) { lo < 16 && hi >= 16 } else { lo <= 0 && hi >= 1 }
    };
    let (u, v) = match a {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    // the slices clipped to the raster's [-24, 40)^2 (an empty one covers nothing)
    const LO: i32 = -24;
    const HI: i32 = 40;
    const MAX: usize = 16;
    let mut slices = [(0i32, 0i32, 0i32, 0i32); MAX];
    let (mut n, mut any) = (0usize, false);
    for b in boxes.iter().filter(|b| covers(b)) {
        any = true;
        let (i0, i1, j0, j1) = (b.0[2 * u].max(LO), b.0[2 * u + 1].min(HI), b.0[2 * v].max(LO), b.0[2 * v + 1].min(HI));
        if i0 < i1 && j0 < j1 {
            if n == MAX {
                return face_supports_grid(boxes, face, support);
            }
            slices[n] = (i0, i1, j0, j1);
            n += 1;
        }
    }
    if !any {
        return false;
    }
    let slices = &slices[..n];
    // FULL: the slice must also not stick out of the block
    if support == Support::Full && slices.iter().any(|s| s.0 < 0 || s.1 > 16 || s.2 < 0 || s.3 > 16) {
        return false;
    }
    let need = need_fn(a, support);
    // every needed cell (all within the block's face) covered by a slice
    for i in 0..16 {
        for j in 0..16 {
            if need(i, j) && !slices.iter().any(|s| s.0 <= i && i < s.1 && s.2 <= j && j < s.3) {
                return false;
            }
        }
    }
    true
}

/// The cells of a face a support type needs (`Block.column(2, 0, 10)` for CENTER, the rigid ring
/// for RIGID), in the face's (u, v) frame.
fn need_fn(a: usize, support: Support) -> impl Fn(i32, i32) -> bool {
    move |i: i32, j: i32| -> bool {
        match support {
            Support::Full => (0..16).contains(&i) && (0..16).contains(&j),
            Support::Center => {
                // Block.column(2, 0, 10): x, z in [7, 9), y in [0, 10)
                let (x, y, z) = match a {
                    0 => (None, Some(i), Some(j)),
                    1 => (Some(i), None, Some(j)),
                    _ => (Some(i), Some(j), None),
                };
                x.is_none_or(|x| (7..9).contains(&x)) && y.is_none_or(|y| (0..10).contains(&y)) && z.is_none_or(|z| (7..9).contains(&z))
            }
            Support::Rigid => {
                if a == 1 {
                    (0..16).contains(&i) && (0..16).contains(&j) && !((2..14).contains(&i) && (2..14).contains(&j))
                } else {
                    (0..16).contains(&i) && (0..16).contains(&j)
                }
            }
        }
    }
}

/// [`face_supports`] by rasterizing the slice on [-24, 40)^2 (the original form: kept for shapes of
/// many boxes, and as the oracle its test compares with).
fn face_supports_grid(boxes: &[B16], face: Dir, support: Support) -> bool {
    let a = axis_index(face);
    // the layer just inside the face
    let covers = |b: &B16| {
        let (lo, hi) = (b.0[2 * a], b.0[2 * a + 1]);
        if positive(face) { lo < 16 && hi >= 16 } else { lo <= 0 && hi >= 1 }
    };
    let (u, v) = match a {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    // the extruded slice, rasterized on [-24, 40)^2
    const LO: i32 = -24;
    const N: usize = 64;
    let mut grid = [[false; N]; N];
    let mut any = false;
    for b in boxes.iter().filter(|b| covers(b)) {
        any = true;
        for i in b.0[2 * u].max(LO)..b.0[2 * u + 1].min(LO + N as i32) {
            for j in b.0[2 * v].max(LO)..b.0[2 * v + 1].min(LO + N as i32) {
                grid[(i - LO) as usize][(j - LO) as usize] = true;
            }
        }
    }
    if !any {
        return false;
    }
    let at = |i: i32, j: i32| grid[(i - LO) as usize][(j - LO) as usize];
    let need = need_fn(a, support);
    for i in LO..LO + N as i32 {
        for j in LO..LO + N as i32 {
            let n = need(i, j);
            let c = at(i, j);
            if n && !c {
                return false;
            }
            // FULL: the slice must also not stick out of the block
            if support == Support::Full && c && !n {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The box-by-box answer is the raster's, bit for bit: the piston shapes (bases, heads, short
    /// heads, every facing, moved by every progress a moving block takes), fences' shapes, and a
    /// deterministic scatter of random boxes in and around the cell, on every face, every support.
    #[test]
    fn face_supports_is_the_rasters_answer() {
        let dirs = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East];
        let mut shapes: Vec<Vec<B16>> = Vec::new();
        for f in dirs {
            for short in [false, true] {
                shapes.push(head(f, short));
            }
            shapes.push(base_extended(f));
            // moving: shifted by every offset a progress gives (in 1/16 steps), each axis
            for d in -16..=16 {
                for s in [[d, 0, 0], [0, d, 0], [0, 0, d]] {
                    shapes.push(head(f, false).into_iter().map(|b| b.shifted(s)).collect());
                    shapes.push(base_extended(f).into_iter().map(|b| b.shifted(s)).collect());
                    shapes.push(vec![B16::full().shifted(s)]);
                }
            }
        }
        for mask in 0..16u32 {
            let (n, e, s, w) = (mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0);
            shapes.push(crate::fence::collision_boxes(n, e, s, w).into_iter().map(B16).collect());
        }
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut rnd = move |m: i32| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % m as u64) as i32
        };
        for _ in 0..4000 {
            let k = 1 + rnd(4) as usize;
            let shape: Vec<B16> = (0..k)
                .map(|_| {
                    let mut b = [0; 6];
                    for ax in 0..3 {
                        let lo = rnd(48) - 16;
                        b[2 * ax] = lo;
                        b[2 * ax + 1] = lo + rnd(24);
                    }
                    B16(b)
                })
                .collect();
            shapes.push(shape);
        }
        let mut checked = 0;
        for sh in &shapes {
            for f in dirs {
                for sup in [Support::Full, Support::Center, Support::Rigid] {
                    assert_eq!(face_supports(sh, f, sup), face_supports_grid(sh, f, sup), "{sh:?} {f:?} {sup:?}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 70_000, "{checked}");
    }
}
