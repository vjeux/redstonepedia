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
    // (a ray's walk looks up every cell it crosses: a fast hash, `FxBuild`)
    by_cell: HashMap<Pos, Vec<u32>, FxBuild>,
    /// The cells holding a still block (with a model or not).
    still: HashSet<Pos, FxBuild>,
    /// The cells holding any block (moving ones too).
    occupied: HashSet<Pos, FxBuild>,
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
        let mut out = Picker { boxes: Vec::new(), by_cell: HashMap::default(), still: cells.keys().copied().collect(), occupied };
        // each state's boxes once, at the origin, then moved to each cell (the same floats: `boxes`
        // adds the cell last; a model lookup per block was 16 ms of a 2492-block farm's camera)
        let mut of_state: HashMap<&str, Vec<([f32; 3], [f32; 3])>, FxBuild> = HashMap::default();
        for (c, st) in list {
            let rel = of_state.entry(st).or_insert_with(|| boxes(atlas, st, [0, 0, 0]));
            for (lo0, hi0) in rel.iter() {
                let lo: [f32; 3] = std::array::from_fn(|k| lo0[k] + c[k] as f32);
                let hi: [f32; 3] = std::array::from_fn(|k| hi0[k] + c[k] as f32);
                let i = out.boxes.len() as u32;
                out.boxes.push((lo, hi, c));
                for x in lo[0].floor() as i32..=hi[0].floor() as i32 {
                    for y in lo[1].floor() as i32..=hi[1].floor() as i32 {
                        for z in lo[2].floor() as i32..=hi[2].floor() as i32 {
                            out.by_cell.entry([x, y, z]).or_default().push(i);
                        }
                    }
                }
            }
        }
        out
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

    /// The first still block the ray hits within `max` blocks, and how far along the ray.
    pub fn hit(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(Pos, f32)> {
        let mut cell = [origin[0].floor() as i32, origin[1].floor() as i32, origin[2].floor() as i32];
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
}

/// The first still block the ray hits within `max` blocks.
pub fn pick(atlas: &Atlas, blocks: &[RBlock], origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<Pos> {
    Picker::new(atlas, blocks).hit(origin, dir, max).map(|(c, _)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

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
