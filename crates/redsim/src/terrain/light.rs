//! The light engine (the terrain lane; brief `designs/briefs/terrain-and-spawning.md` §2): sky
//! light and block light over a whole box, as the game's engines settle them.
//!
//! * Sky (`SkyLightEngine`, `ChunkSkyLightSources`): each column holds 15 from the top down to its
//!   lowest source, the cell above the first occluded edge (`isEdgeOccluded`: the lower state's
//!   dampening is not 0, or the two faces' occlusion shapes cover the face); from the sources light
//!   spreads to the six neighbours losing `LightEngine.getOpacity` = `max(1, getLightDampening())`,
//!   never across faces whose occlusion shapes cover them (`shapeOccludes`).
//! * Block (`BlockLightEngine`): every state's `getLightEmission()` spreads the same way.
//!
//! The values are the engines' fixed point: the maximum over sources of the source's level minus
//! the path's cost. Each state's light behaviour comes from the natural-state table (the game's own
//! `getLightDampening`, `getLightEmission`, `canOcclude`, `useShapeForLightOcclusion`,
//! `getFaceOcclusionShape`).

use crate::block::Block;
use crate::pos::BlockPos;
use std::collections::HashMap;

/// A state's light behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightProps {
    pub dampening: u8,
    pub emission: u8,
    /// The row of the natural-state table holding its face occlusion shapes (`None`: it does not
    /// occlude by shape).
    pub faces: Option<u16>,
}

impl LightProps {
    pub const AIR: LightProps = LightProps { dampening: 0, emission: 0, faces: None };

    /// From a table row.
    pub fn of_row(id: u16) -> LightProps {
        let p = super::blocks::props(id);
        LightProps { dampening: p.dampening.min(15), emission: p.emission, faces: if p.can_occlude && p.shape_occlusion && p.faces.is_some() { Some(id) } else { None } }
    }

    /// `getOcclusionShape(state, dir)` on the 16 x 16 grid (directions: down, up, north, south,
    /// west, east).
    fn face(&self, dir: usize) -> [u16; 16] {
        match self.faces {
            Some(id) => super::blocks::props(id).faces.map(|f| f[dir]).unwrap_or([0; 16]),
            None => [0; 16],
        }
    }
}

/// The light behaviour of a redsim block: a natural state's row, or the row of the state's game
/// string (every block pasted into terrain is in the table: `mctas terrain blockinfo --design`).
pub fn props_of(b: &Block) -> Result<LightProps, String> {
    match *b {
        Block::Air => Ok(LightProps::AIR),
        Block::Terrain(id) => Ok(LightProps::of_row(id)),
        // fire (mobinfra's `Block::Fire`): `BaseFireBlock` lightLevel 15 (soul fire 10), no
        // collision, noOcclusion, propagatesSkylightDown (light dampening 0), no occluding shape
        Block::Fire { soul, .. } => Ok(LightProps { dampening: 0, emission: if soul { 10 } else { 15 }, faces: None }),
        _ => {
            // a state with a block of its own: its row, remembered per state (tickexact
            // 2026-11-08: its name formatted and looked up at every query)
            thread_local! {
                static ROWS: std::cell::RefCell<crate::fxhash::FxHashMap<Block, Result<LightProps, String>>> = Default::default();
            }
            if let Some(v) = ROWS.with(|m| m.borrow().get(b).cloned()) {
                return v;
            }
            let v = props_of_named(b);
            ROWS.with(|m| m.borrow_mut().insert(*b, v.clone()));
            v
        }
    }
}

/// [`props_of`] of a state with a block of its own, by its name in the natural-state table.
fn props_of_named(b: &Block) -> Result<LightProps, String> {
    let name = b.to_string();
    match super::blocks::id_of(&name) {
        Some(id) => Ok(LightProps::of_row(id)),
        None => {
            // redstone dust and tripwire: the same light in every state (no emission, no
            // dampening, no occluding shape), so any of their rows answers
            let base = name.split('[').next().unwrap_or(&name);
            if matches!(base, "minecraft:redstone_wire" | "minecraft:tripwire")
                && let Some(id) = super::blocks::first_of_block(base)
            {
                return Ok(LightProps::of_row(id));
            }
            Err(format!("the light of {name} is not in the natural-state table (mctas terrain blockinfo --state)"))
        }
    }
}

/// Direction index (down, up, north, south, west, east) and its step.
const DIRS: [(usize, [i32; 3]); 6] = [(0, [0, -1, 0]), (1, [0, 1, 0]), (2, [0, 0, -1]), (3, [0, 0, 1]), (4, [-1, 0, 0]), (5, [1, 0, 0])];
pub(crate) const DIRS_PUB: [(usize, [i32; 3]); 6] = DIRS;

pub(crate) fn shape_occludes_pub(from: &LightProps, to: &LightProps, d: usize) -> bool {
    shape_occludes(from, to, d)
}

pub(crate) fn edge_occluded_pub(top: &LightProps, bottom: &LightProps) -> bool {
    edge_occluded(top, bottom)
}

fn opposite(d: usize) -> usize {
    d ^ 1
}

/// `Shapes.faceShapeOccludes`: the union of the two faces covers the face.
fn faces_cover(a: &[u16; 16], b: &[u16; 16]) -> bool {
    (0..16).all(|v| a[v] | b[v] == 0xffff)
}

/// `LightEngine.shapeOccludes(from, to, dir)`.
fn shape_occludes(from: &LightProps, to: &LightProps, d: usize) -> bool {
    if from.faces.is_none() && to.faces.is_none() {
        return false;
    }
    faces_cover(&from.face(d), &to.face(opposite(d)))
}

/// `ChunkSkyLightSources.isEdgeOccluded(top, bottom)`.
fn edge_occluded(top: &LightProps, bottom: &LightProps) -> bool {
    bottom.dampening != 0 || ((top.faces.is_some() || bottom.faces.is_some()) && faces_cover(&top.face(0), &bottom.face(1)))
}

/// A box of cells with their light behaviour, indexed x fastest, then z, then y.
pub struct Grid {
    pub min: BlockPos,
    pub size: [i32; 3],
    /// Per cell: an index into `props`.
    pub cells: Vec<u16>,
    pub props: Vec<LightProps>,
}

impl Grid {
    pub fn index(&self, x: i32, y: i32, z: i32) -> usize {
        (((y - self.min.y) * self.size[2] + (z - self.min.z)) * self.size[0] + (x - self.min.x)) as usize
    }

    fn at(&self, i: usize) -> &LightProps {
        &self.props[self.cells[i] as usize]
    }
}

/// The light of every cell of a grid: (sky, block). Above the grid is open sky (air up to the
/// world's top), around it nothing (light does not enter from the sides).
pub fn compute(g: &Grid) -> (Vec<u8>, Vec<u8>) {
    let n = g.cells.len();
    let mut sky = vec![0u8; n];
    let mut queue: Vec<Vec<u32>> = vec![Vec::new(); 16];
    let [sx, sy, sz] = g.size;
    for z in 0..sz {
        for x in 0..sx {
            let mut top = LightProps::AIR;
            let mut y = sy - 1;
            while y >= 0 {
                let i = (((y * sz) + z) * sx + x) as usize;
                let bottom = *g.at(i);
                if edge_occluded(&top, &bottom) {
                    break;
                }
                sky[i] = 15;
                queue[15].push(i as u32);
                top = bottom;
                y -= 1;
            }
        }
    }
    spread(g, &mut sky, &mut queue);
    let mut block = vec![0u8; n];
    let mut queue: Vec<Vec<u32>> = vec![Vec::new(); 16];
    for i in 0..n {
        let e = g.at(i).emission.min(15);
        if e > 0 {
            block[i] = e;
            queue[e as usize].push(i as u32);
        }
    }
    spread(g, &mut block, &mut queue);
    (sky, block)
}

/// Spread queued levels (a bucket queue from 15 down) to their fixed point.
fn spread(g: &Grid, level: &mut [u8], queue: &mut [Vec<u32>]) {
    let [sx, sy, sz] = g.size;
    for l in (2..=15usize).rev() {
        while let Some(i) = queue[l].pop() {
            let i = i as usize;
            if level[i] as usize != l {
                continue;
            }
            let x = (i as i32) % sx;
            let z = ((i as i32) / sx) % sz;
            let y = (i as i32) / (sx * sz);
            let from = *g.at(i);
            for (d, [dx, dy, dz]) in DIRS {
                let (qx, qy, qz) = (x + dx, y + dy, z + dz);
                if qx < 0 || qy < 0 || qz < 0 || qx >= sx || qy >= sy || qz >= sz {
                    continue;
                }
                let j = (((qy * sz) + qz) * sx + qx) as usize;
                let to = g.at(j);
                let new = l as i32 - (to.dampening.max(1) as i32);
                if new > level[j] as i32 && !shape_occludes(&from, to, d) {
                    level[j] = new as u8;
                    if new > 1 {
                        queue[new as usize].push(j as u32);
                    }
                }
            }
        }
    }
}

/// The grid of a terrain file (its palette's rows).
pub fn grid_of_terrain(t: &super::Terrain) -> Result<Grid, String> {
    let mut props = Vec::with_capacity(t.palette.len());
    for s in &t.palette {
        let id = super::blocks::id_of(s).ok_or_else(|| format!("{s} is not in the natural-state table"))?;
        props.push(LightProps::of_row(id));
    }
    Ok(Grid { min: BlockPos::new(t.origin[0], t.origin[1], t.origin[2]), size: t.size, cells: t.cells.clone(), props })
}

/// The grid of a redsim world box (each distinct block's light behaviour looked up once).
pub fn grid_of_world(w: &crate::world::World) -> Result<Grid, String> {
    let mut index: HashMap<Block, u16> = HashMap::new();
    let mut props = Vec::new();
    let [sx, sy, sz] = w.size;
    let mut cells = Vec::with_capacity((sx * sy * sz) as usize);
    for y in 0..sy {
        for z in 0..sz {
            for x in 0..sx {
                let b = w.get(w.min.offset(x, y, z));
                let id = match index.get(&b) {
                    Some(&i) => i,
                    None => {
                        let i = props.len() as u16;
                        props.push(props_of(&b)?);
                        index.insert(b, i);
                        i
                    }
                };
                cells.push(id);
            }
        }
    }
    Ok(Grid { min: w.min, size: w.size, cells, props })
}

/// How a computed light compares with the game's saved values, over the cells light can be read
/// in (not opaque: dampening below 15) and the opaque ones apart.
#[derive(Clone, Debug, Default)]
pub struct Comparison {
    pub cells: usize,
    pub open_cells: usize,
    pub sky_diff_open: usize,
    pub block_diff_open: usize,
    pub sky_diff_opaque: usize,
    pub block_diff_opaque: usize,
    /// A few differing open cells: (x, y, z, ours, game's, which).
    pub examples: Vec<(i32, i32, i32, u8, u8, &'static str)>,
}

/// Compare the engine's light of a terrain file with the light the game saved in it, over the
/// inner box (the file's box less `margin` on each horizontal side: light from outside the file
/// does not reach in here).
pub fn compare_with_saved(t: &super::Terrain, margin: i32) -> Result<Comparison, String> {
    if t.sky.is_empty() || t.block.is_empty() {
        return Err("the file holds no saved light".into());
    }
    let g = grid_of_terrain(t)?;
    let (sky, block) = compute(&g);
    let mut c = Comparison::default();
    let [ox, oy, oz] = t.origin;
    let [sx, sy, sz] = t.size;
    for y in oy..oy + sy {
        for z in oz + margin..oz + sz - margin {
            for x in ox + margin..ox + sx - margin {
                let i = t.index(x, y, z);
                c.cells += 1;
                let open = g.at(i).dampening < 15;
                let (ds, db) = (sky[i] != t.sky[i], block[i] != t.block[i]);
                if open {
                    c.open_cells += 1;
                    c.sky_diff_open += ds as usize;
                    c.block_diff_open += db as usize;
                    if ds && c.examples.len() < 12 {
                        c.examples.push((x, y, z, sky[i], t.sky[i], "sky"));
                    }
                    if db && c.examples.len() < 12 {
                        c.examples.push((x, y, z, block[i], t.block[i], "block"));
                    }
                } else {
                    c.sky_diff_opaque += ds as usize;
                    c.block_diff_opaque += db as usize;
                }
            }
        }
    }
    Ok(c)
}
