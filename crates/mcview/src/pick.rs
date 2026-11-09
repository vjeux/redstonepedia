//! Clicking a block: the ray from the camera through the pointer walks the cells it crosses
//! (Amanatides-Woo) and stops at the first block whose model boxes it hits.

use crate::atlas::Atlas;
use crate::camera::FxBuild;
use crate::design::Pos;
use crate::models;
use crate::session::RBlock;
use std::collections::{HashMap, HashSet};

fn quarter(p: [f32; 3], xrot: u8, yrot: u8) -> [f32; 3] {
    let mut q = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
    for _ in 0..xrot % 4 {
        q = [q[0], q[2], -q[1]];
    }
    for _ in 0..yrot % 4 {
        q = [-q[2], q[1], q[0]];
    }
    [q[0] + 0.5, q[1] + 0.5, q[2] + 0.5]
}

/// Distance along the ray to a box, if it hits.
fn hit_box(o: [f32; 3], d: [f32; 3], lo: [f32; 3], hi: [f32; 3]) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for k in 0..3 {
        if d[k].abs() < 1e-9 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// The boxes (world coordinates) of a block's model at a cell, a little padded so thin blocks
/// (dust, levers) stay easy to click.
pub fn boxes(atlas: &Atlas, state: &str, cell: Pos) -> Vec<([f32; 3], [f32; 3])> {
    let Some(m) = models::model_for(state, atlas) else { return Vec::new() };
    let mut out = Vec::new();
    for p in &m.parts {
        let a = quarter([p.from[0] / 16.0, p.from[1] / 16.0, p.from[2] / 16.0], m.xrot, m.yrot);
        let b = quarter([p.to[0] / 16.0, p.to[1] / 16.0, p.to[2] / 16.0], m.xrot, m.yrot);
        let mut lo = [0.0; 3];
        let mut hi = [0.0; 3];
        for k in 0..3 {
            lo[k] = a[k].min(b[k]).max(0.0) - 0.06 + cell[k] as f32;
            hi[k] = a[k].max(b[k]).min(1.0) + 0.06 + cell[k] as f32;
        }
        out.push((lo, hi));
    }
    out
}

/// The still blocks' model boxes, ready for many rays ([`Picker::hit`]): each box is listed in
/// every cell it reaches, so a ray walks its cells with one lookup each (the default camera casts
/// thousands: `scene::best_camera`).
pub struct Picker {
    boxes: Vec<([f32; 3], [f32; 3], Pos)>,
    // (a ray's walk looks up every cell it crosses)
    by_cell: CellLists,
    /// The cells holding a still block (with a model or not).
    still: HashSet<Pos, FxBuild>,
    /// The cells holding any block (moving ones too).
    occupied: HashSet<Pos, FxBuild>,
}

/// The boxes listed in each cell they reach, each list in the boxes' order: dense over the cells'
/// box when it is modest (a farm in its terrain: 1.3 million listings, which a map of lists took
/// 90 ms natively to build), a map of lists otherwise.
enum CellLists {
    Dense { lo: Pos, dims: [i32; 3], start: Vec<u32>, items: Vec<u32> },
    Map(HashMap<Pos, Vec<u32>, FxBuild>, Option<(Pos, Pos)>),
}

impl CellLists {
    /// The boxes reaching cell `c`, in their order (None: none).
    fn get(&self, c: &Pos) -> Option<&[u32]> {
        match self {
            CellLists::Dense { lo, dims, start, items } => {
                let d = [c[0] - lo[0], c[1] - lo[1], c[2] - lo[2]];
                if (0..3).any(|k| d[k] < 0 || d[k] >= dims[k]) {
                    return None;
                }
                let i = ((d[1] * dims[2] + d[2]) * dims[0] + d[0]) as usize;
                let (s, e) = (start[i] as usize, start[i + 1] as usize);
                (e > s).then(|| &items[s..e])
            }
            CellLists::Map(m, _) => m.get(c).map(Vec::as_slice),
        }
    }

    /// The cells' box: the lowest cell and the highest (None: no box listed).
    fn bounds(&self) -> Option<(Pos, Pos)> {
        match self {
            CellLists::Dense { lo, dims, .. } => Some((*lo, [lo[0] + dims[0] - 1, lo[1] + dims[1] - 1, lo[2] + dims[2] - 1])),
            CellLists::Map(_, b) => *b,
        }
    }

    /// From each box's cells (`reach`: the cells' ranges per box, in the boxes' order).
    fn build(reach: &[[[i32; 2]; 3]]) -> CellLists {
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for r in reach {
            for k in 0..3 {
                lo[k] = lo[k].min(r[k][0]);
                hi[k] = hi[k].max(r[k][1]);
            }
        }
        let dims = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        let cells = dims.iter().map(|&d| d.max(0) as i64).product::<i64>();
        if reach.is_empty() || cells > 1 << 24 {
            return CellLists::map(reach);
        }
        let at = |x: i32, y: i32, z: i32| (((y - lo[1]) * dims[2] + (z - lo[2])) * dims[0] + (x - lo[0])) as usize;
        // each cell's count, then where its list starts, then the lists filled in the boxes' order
        let mut start = vec![0u32; cells as usize + 1];
        for r in reach {
            for x in r[0][0]..=r[0][1] {
                for y in r[1][0]..=r[1][1] {
                    for z in r[2][0]..=r[2][1] {
                        start[at(x, y, z) + 1] += 1;
                    }
                }
            }
        }
        for i in 1..start.len() {
            start[i] += start[i - 1];
        }
        let mut next = start.clone();
        let mut items = vec![0u32; *start.last().unwrap() as usize];
        for (i, r) in reach.iter().enumerate() {
            for x in r[0][0]..=r[0][1] {
                for y in r[1][0]..=r[1][1] {
                    for z in r[2][0]..=r[2][1] {
                        let c = at(x, y, z);
                        items[next[c] as usize] = i as u32;
                        next[c] += 1;
                    }
                }
            }
        }
        CellLists::Dense { lo, dims, start, items }
    }

    /// [`CellLists::build`] as a map of lists.
    fn map(reach: &[[[i32; 2]; 3]]) -> CellLists {
        let mut m: HashMap<Pos, Vec<u32>, FxBuild> = HashMap::default();
        let mut bounds: Option<(Pos, Pos)> = None;
        for (i, r) in reach.iter().enumerate() {
            let (lo, hi) = bounds.get_or_insert(([r[0][0], r[1][0], r[2][0]], [r[0][1], r[1][1], r[2][1]]));
            for k in 0..3 {
                lo[k] = lo[k].min(r[k][0]);
                hi[k] = hi[k].max(r[k][1]);
            }
            for x in r[0][0]..=r[0][1] {
                for y in r[1][0]..=r[1][1] {
                    for z in r[2][0]..=r[2][1] {
                        m.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        CellLists::Map(m, bounds)
    }
}

impl Picker {
    pub fn new(atlas: &Atlas, blocks: &[RBlock]) -> Picker {
        let mut cells: HashMap<Pos, &str, FxBuild> = HashMap::default();
        let mut occupied: HashSet<Pos, FxBuild> = HashSet::default();
        for b in blocks {
            occupied.insert(b.cell);
            if b.offset == [0.0; 3] {
                cells.insert(b.cell, b.state.as_str());
            }
        }
        let mut list: Vec<(Pos, &str)> = cells.iter().map(|(c, s)| (*c, *s)).collect();
        list.sort_by_key(|(c, _)| *c);
        let mut boxes_out: Vec<([f32; 3], [f32; 3], Pos)> = Vec::new();
        let mut reach: Vec<[[i32; 2]; 3]> = Vec::new();
        // each state's boxes once, at the origin, then moved to each cell (the same floats: `boxes`
        // adds the cell last; a model lookup per block was 16 ms of a 2492-block farm's camera)
        let mut of_state: HashMap<&str, Vec<([f32; 3], [f32; 3])>, FxBuild> = HashMap::default();
        for (c, st) in list {
            let rel = of_state.entry(st).or_insert_with(|| boxes(atlas, st, [0, 0, 0]));
            for (lo0, hi0) in rel.iter() {
                let lo: [f32; 3] = std::array::from_fn(|k| lo0[k] + c[k] as f32);
                let hi: [f32; 3] = std::array::from_fn(|k| hi0[k] + c[k] as f32);
                boxes_out.push((lo, hi, c));
                reach.push(std::array::from_fn(|k| [lo[k].floor() as i32, hi[k].floor() as i32]));
            }
        }
        Picker { boxes: boxes_out, by_cell: CellLists::build(&reach), still: cells.keys().copied().collect(), occupied }
    }

    /// Whether a still block stands at `c`.
    pub fn has(&self, c: Pos) -> bool {
        self.still.contains(&c)
    }

    /// Whether any block (moving ones too) is at `c`.
    pub fn occupied(&self, c: Pos) -> bool {
        self.occupied.contains(&c)
    }

    /// The model box of the cell's block with the most volume (its main part), if any.
    pub fn main_box(&self, c: Pos) -> Option<([f32; 3], [f32; 3])> {
        let list = self.by_cell.get(&c)?;
        list.iter().map(|&i| self.boxes[i as usize]).filter(|(_, _, b)| *b == c).map(|(lo, hi, _)| (lo, hi)).max_by(|a, b| {
            let v = |(lo, hi): &([f32; 3], [f32; 3])| (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2]);
            v(a).total_cmp(&v(b))
        })
    }

    /// The first still block the ray hits within `max` blocks, and how far along the ray. (The
    /// walk starts a cell's diagonal before the ray enters the boxes' cells, and stops once it
    /// leaves them: a camera's ray from 50 blocks away walked every empty cell to the build;
    /// the same hit: test `hits_skip_the_empty_cells`.)
    pub fn hit(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(Pos, f32)> {
        let Some((blo, bhi)) = self.by_cell.bounds() else { return None };
        let (lo, hi) = (blo.map(|v| v as f32), bhi.map(|v| v as f32 + 1.0));
        // (every box lies in the cells' box: a ray missing it hits nothing, one entering it
        // beyond `max` nothing within `max`)
        let (t_in, t_out) = enter_box(origin, dir, lo, hi)?;
        if t_in > max {
            return None;
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt().max(1e-12);
        let t0 = (t_in - 1.75 / len).max(0.0);
        let start = [origin[0] + dir[0] * t0, origin[1] + dir[1] * t0, origin[2] + dir[2] * t0];
        self.walk(origin, dir, max, start, Some(t_out))
    }

    /// [`Picker::hit`]'s walk from the ray's cell at `start` (a point on it), until a box is
    /// surely the first hit, past `max`, or past `t_out` (no box beyond).
    fn walk(&self, origin: [f32; 3], dir: [f32; 3], max: f32, start: [f32; 3], t_out: Option<f32>) -> Option<(Pos, f32)> {
        let mut cell = [start[0].floor() as i32, start[1].floor() as i32, start[2].floor() as i32];
        let step: [i32; 3] = std::array::from_fn(|k| if dir[k] > 0.0 { 1 } else { -1 });
        let mut tmax = [0f32; 3];
        let mut tdelta = [0f32; 3];
        for k in 0..3 {
            if dir[k].abs() < 1e-9 {
                tmax[k] = f32::MAX;
                tdelta[k] = f32::MAX;
            } else {
                let next = if dir[k] > 0.0 { cell[k] as f32 + 1.0 } else { cell[k] as f32 };
                tmax[k] = (next - origin[k]) / dir[k];
                tdelta[k] = 1.0 / dir[k].abs();
            }
        }
        let mut best: Option<(f32, Pos)> = None;
        loop {
            let exit = tmax[0].min(tmax[1]).min(tmax[2]);
            if let Some(list) = self.by_cell.get(&cell) {
                for &i in list {
                    let (lo, hi, c) = self.boxes[i as usize];
                    if let Some(h) = hit_box(origin, dir, lo, hi)
                        && best.is_none_or(|(b, _)| h < b)
                    {
                        best = Some((h, c));
                    }
                }
            }
            // every box hit before this cell's exit was met on the way (each box is listed in
            // every cell it reaches)
            if let Some((h, c)) = best
                && h <= exit + 1e-4
            {
                return (h <= max).then_some((c, h));
            }
            if exit > max {
                return None;
            }
            // (past the boxes' cells: nothing more to meet)
            if t_out.is_some_and(|t| exit > t + 1e-3) {
                return best.filter(|(h, _)| *h <= max).map(|(h, c)| (c, h));
            }
            let k = if tmax[0] < tmax[1] && tmax[0] < tmax[2] {
                0
            } else if tmax[1] < tmax[2] {
                1
            } else {
                2
            };
            cell[k] += step[k];
            tmax[k] += tdelta[k];
        }
    }

    /// [`Picker::hit`] walking every cell from the ray's origin (a test's oracle).
    #[cfg(test)]
    fn hit_from_origin(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(Pos, f32)> {
        self.walk(origin, dir, max, origin, None)
    }
}

/// Where a ray (`origin` + t `dir`, t >= 0) is inside the box `lo..hi`: (t entering, t leaving);
/// None: it misses it.
fn enter_box(o: [f32; 3], d: [f32; 3], lo: [f32; 3], hi: [f32; 3]) -> Option<(f32, f32)> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for k in 0..3 {
        if d[k].abs() < 1e-9 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, t1))
}

/// The first still block the ray hits within `max` blocks.
pub fn pick(atlas: &Atlas, blocks: &[RBlock], origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<Pos> {
    Picker::new(atlas, blocks).hit(origin, dir, max).map(|(c, _)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ray's walk started where it enters the boxes' cells finds the very hit of the walk from
    /// its origin (the block and its distance, bit for bit): rays from 60 blocks around a farm in
    /// its terrain, a farm alone and a clock, at random points of the build and around it, and
    /// rays starting inside it.
    #[test]
    fn hits_skip_the_empty_cells() {
        let atlas = Atlas::build();
        let mut seed = 0x0ddc_0ffe_e15e_ed01u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f32 / (1u64 << 53) as f32
        };
        for design in ["designs/farms/iron/day-1/d1u-underground.txt", "designs/circuits/mobfarm/archive/iron-day1.txt", "designs/clocks/archive/clk-sculk-ringbig16-p224-1.txt"] {
            let text = std::fs::read_to_string(format!("{}/../../{design}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let p = crate::player::Player::new_web(&text, 1.0).unwrap();
            let picker = Picker::new(&atlas, &p.session.render_blocks(0.0));
            let (lo, hi) = picker.by_cell.bounds().unwrap();
            let (lo, hi) = (lo.map(|v| v as f32), hi.map(|v| v as f32 + 1.0));
            let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
            let (mut hits, mut rays) = (0, 0);
            for i in 0..3000 {
                let target = [lo[0] + (hi[0] - lo[0]) * rnd(), lo[1] + (hi[1] - lo[1]) * rnd(), lo[2] + (hi[2] - lo[2]) * rnd()];
                let origin = if i % 5 == 0 {
                    // (inside the build)
                    [lo[0] + (hi[0] - lo[0]) * rnd(), lo[1] + (hi[1] - lo[1]) * rnd(), lo[2] + (hi[2] - lo[2]) * rnd()]
                } else {
                    let (a, b) = (rnd() * std::f32::consts::TAU, rnd() * 1.4 - 0.2);
                    [mid[0] + 60.0 * b.cos() * a.cos(), mid[1] + 60.0 * b.sin(), mid[2] + 60.0 * b.cos() * a.sin()]
                };
                let v = [target[0] - origin[0], target[1] - origin[1], target[2] - origin[2]];
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
                let d = [v[0] / len, v[1] / len, v[2] / len];
                let max = if i % 7 == 0 { len * 0.5 } else { len + 1.0 };
                let (a, b) = (picker.hit(origin, d, max), picker.hit_from_origin(origin, d, max));
                assert_eq!(a.map(|(c, h)| (c, h.to_bits())), b.map(|(c, h)| (c, h.to_bits())), "{design}: ray {i} from {origin:?} along {d:?} (max {max})");
                hits += a.is_some() as u32;
                rays += 1;
            }
            assert!(hits > 20, "{design}: {hits} of {rays} rays hit");
        }
    }

    /// The dense cell lists are the map's (each cell's boxes, in their order): a farm in its
    /// terrain's blocks near it (the camera's picker) and a farm alone, every cell of their box
    /// and around it.
    #[test]
    fn dense_cell_lists_are_the_maps() {
        let atlas = Atlas::build();
        for design in ["designs/farms/iron/day-1/d1u-underground.txt", "designs/circuits/mobfarm/archive/iron-day1.txt"] {
            let text = std::fs::read_to_string(format!("{}/../../{design}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let p = crate::player::Player::new_web(&text, 1.0).unwrap();
            let blocks = p.session.render_blocks(0.0);
            let picker = Picker::new(&atlas, &blocks);
            let reach: Vec<[[i32; 2]; 3]> = picker.boxes.iter().map(|(lo, hi, _)| std::array::from_fn(|k| [lo[k].floor() as i32, hi[k].floor() as i32])).collect();
            let map = CellLists::map(&reach);
            let CellLists::Dense { lo, dims, .. } = &picker.by_cell else { panic!("{design}: not dense") };
            let mut listed = 0;
            for y in lo[1] - 1..=lo[1] + dims[1] {
                for z in lo[2] - 1..=lo[2] + dims[2] {
                    for x in lo[0] - 1..=lo[0] + dims[0] {
                        let c = [x, y, z];
                        assert_eq!(picker.by_cell.get(&c), map.get(&c), "{design}: cell {c:?}");
                        listed += picker.by_cell.get(&c).map_or(0, |l| l.len());
                    }
                }
            }
            assert!(listed > 1000, "{design}: {listed} listings");
        }
    }

    /// Each state's boxes found once and moved to each cell are the very boxes found per block
    /// (the same floats in the same order).
    #[test]
    fn a_picker_built_per_state_is_the_one_built_per_block() {
        let atlas = Atlas::build();
        let text = std::fs::read_to_string(format!("{}/../../designs/circuits/mobfarm/archive/iron-day1.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let s = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
        let blocks = s.render_blocks(0.0);
        let p = Picker::new(&atlas, &blocks);
        let mut list: Vec<(Pos, &str)> = blocks.iter().filter(|b| b.offset == [0.0; 3]).map(|b| (b.cell, b.state.as_str())).collect();
        list.sort_by_key(|(c, _)| *c);
        list.dedup_by_key(|(c, _)| *c);
        let per_block: Vec<([f32; 3], [f32; 3], Pos)> = list.iter().flat_map(|(c, st)| boxes(&atlas, st, *c).into_iter().map(move |(lo, hi)| (lo, hi, *c))).collect();
        assert!(per_block.len() > 100);
        assert_eq!(format!("{:?}", p.boxes), format!("{per_block:?}"));
    }

    #[test]
    fn picks_the_nearest_block() {
        let atlas = Atlas::build();
        let blocks = vec![
            RBlock { cell: [0, 0, 0], offset: [0.0; 3], state: "minecraft:stone".into() },
            RBlock { cell: [0, 0, 3], offset: [0.0; 3], state: "minecraft:lever[face=floor,facing=north,powered=false]".into() },
        ];
        let o = [0.5, 0.5, 10.0];
        let d = [0.0, 0.0, -1.0];
        assert_eq!(pick(&atlas, &blocks, o, d, 50.0), Some([0, 0, 3]));
        assert_eq!(pick(&atlas, &blocks, [0.5, 0.9, 10.0], d, 50.0), Some([0, 0, 0]));
        assert_eq!(pick(&atlas, &blocks, [5.5, 0.5, 10.0], d, 50.0), None);
    }
}
