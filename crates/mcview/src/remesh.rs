//! A mob farm's blocks' mesh updated in place: when a few blocks change (grass spreading and dying
//! in its terrain, a pressure plate, fire), only the faces that can change are drawn again, those
//! of the blocks within a cell of a changed block or of a cell whose light changed, and spliced
//! into the mesh in place of their old ones. A farm in its terrain draws 150k-500k vertices: built
//! whole (0.4 s natively for cross-4-underground, more in the browser), a farm whose blocks change
//! every few ticks never showed its blocks as they were.
//!
//! The faces of a block depend on its own state, on the cells within one of it (culling against
//! its neighbours' occupancy and names, the corners' shade, a water cell's neighbours' levels) and
//! on the light of the cells within one of it (smooth lighting): so a changed block redraws the
//! blocks around it, and a changed light (a fire, a torch, a block that lets the sky in) the blocks
//! around every cell whose light moved. The light itself is the whole box's again
//! ([`crate::light::Light::compute`]), compared cell by cell. Anything else (a moving block, the
//! light's box or floor moving, an output's box, a TNT duper's ground) builds the mesh whole.
//!
//! The faces come in the same order as a whole build's for the blocks it knew (each block's in
//! place); a block new since (a cell the run wrote outside the blocks drawn at first) comes after
//! them: the same triangles, a few in another order.

use crate::atlas::Atlas;
use crate::mesh::{AsItem, Builder, BuilderParts, Mesh, Quad, Vertex};
use crate::session::{RBlock, Session};
use std::collections::{HashMap, HashSet};

type Pos = [i32; 3];

/// What a whole build of a mob farm's blocks left to update its mesh in place
/// ([`crate::scene::Staged::keeping`]).
pub struct Remesh {
    /// The blocks in the mesh's order (a block gone: `removed`).
    blocks: Vec<RBlock>,
    removed: Vec<bool>,
    /// The builder after the blocks' first pass and faces: names, occupancy, water, the cells
    /// for the light, the light, the cells' index, each block's ranges in the mesh.
    parts: Option<BuilderParts>,
    /// Where the blocks' own faces end in the mesh (solid vertices, translucent quads): after
    /// them, what is drawn whole (outlines, markers), kept as it is.
    items_end: (usize, usize),
}

impl Remesh {
    /// From a whole build's blocks and builder (with its ranges, its light put back) and where
    /// its blocks' faces end.
    pub fn new(blocks: Vec<RBlock>, mut parts: BuilderParts, items_end: (usize, usize)) -> Option<Remesh> {
        let n = blocks.len();
        if parts.ranges().is_none_or(|r| r.len() != n) || parts.light_cells_len() != n {
            return None;
        }
        parts.clear_mesh();
        Some(Remesh { removed: vec![false; n], blocks, parts: Some(parts), items_end })
    }

    /// The mesh `old` (the last one built or updated) with the blocks at the world cells
    /// `changed` as the session draws them now; None when only a whole build can tell (the
    /// mesh is then built whole and this dropped).
    pub fn update(&mut self, atlas: &Atlas, s: &Session, changed: &[Pos], old: &Mesh) -> Option<Mesh> {
        let parts = self.parts.take()?;
        let mut b = Builder::from_parts(atlas, parts);
        let mut states = b.take_states()?;
        // the blocks changed, in the first pass (names, occupancy, water, their light's cell)
        let mut touched: Vec<Pos> = Vec::new();
        let mut seen: HashSet<Pos> = HashSet::new();
        for &w in changed {
            if !seen.insert(w) {
                continue;
            }
            let now = s.drawn_block_at(w).ok()?;
            let v = match &now {
                Some(r) => r.cell,
                None => {
                    let off = s.view_offset();
                    [w[0] - off[0], w[1] - off[1], w[2] - off[2]]
                }
            };
            match (states.get(&v).copied(), now) {
                (Some(i), Some(r)) => {
                    let i = i as usize;
                    if !self.removed[i] && self.blocks[i].state == r.state {
                        continue;
                    }
                    self.removed[i] = false;
                    self.blocks[i].state = r.state;
                    b.forget_cell(v);
                    let lc = b.begin_item(&self.blocks[i].item());
                    b.set_light_cell(i, lc);
                }
                (Some(i), None) => {
                    let i = i as usize;
                    if self.removed[i] {
                        continue;
                    }
                    self.removed[i] = true;
                    b.forget_cell(v);
                    b.set_light_cell(i, (v, crate::light::Cell::default(), 0));
                }
                (None, Some(r)) => {
                    let i = self.blocks.len();
                    states.insert(v, i as u32);
                    self.blocks.push(r);
                    self.removed.push(false);
                    let lc = b.begin_item(&self.blocks[i].item());
                    b.set_light_cell(i, lc);
                }
                (None, None) => continue,
            }
            touched.push(v);
        }
        // the light again, and the blocks whose faces can change: those within two cells of a
        // changed block (a face's ambient occlusion looks one cell past its corner's neighbours,
        // along its normal: `Builder::face_light`'s `permeable`) or within one of a cell whose
        // light changed
        let lit = b.relight()?;
        if std::env::var("MCVIEW_REMESH_LOG").is_ok() {
            eprintln!("remesh: {} changed cells, {} blocks touched {:?}, {} cells relit {:?}", changed.len(), touched.len(), &touched[..touched.len().min(6)], lit.len(), &lit[..lit.len().min(6)]);
        }
        let mut redraw: HashSet<u32> = HashSet::new();
        for (cells, r) in [(&touched, 2), (&lit, 1)] {
            for c in cells.iter() {
                for dy in -r..=r {
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if let Some(&i) = states.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) {
                                redraw.insert(i);
                            }
                        }
                    }
                }
            }
        }
        let mut redraw: Vec<u32> = redraw.into_iter().collect();
        redraw.sort_unstable();
        // their faces anew, into a scratch mesh
        let blocks = &self.blocks;
        let lookup = |p: Pos| states.get(&p).map(|&k| blocks[k as usize].state.as_str());
        let mut fresh: HashMap<u32, [u32; 4]> = HashMap::with_capacity(redraw.len());
        for &i in &redraw {
            let r = if self.removed[i as usize] { [0; 4] } else { b.emit_item(&blocks[i as usize].item(), &lookup) };
            fresh.insert(i, r);
        }
        let scratch = std::mem::take(&mut b.mesh);
        // the mesh: each block's faces, the old ones or the fresh, in the blocks' order, then what
        // follows them as it was
        let old_ranges = b.ranges.take()?;
        let mut solid: Vec<Vertex> = Vec::with_capacity(old.solid.len() + scratch.solid.len());
        let mut translucent: Vec<Quad> = Vec::with_capacity(old.translucent.len() + scratch.translucent.len());
        let mut ranges: Vec<[u32; 4]> = Vec::with_capacity(self.blocks.len());
        for i in 0..self.blocks.len() {
            let (src_s, src_t, r) = match fresh.get(&(i as u32)) {
                Some(r) => (&scratch.solid, &scratch.translucent, *r),
                None => (&old.solid, &old.translucent, old_ranges[i]),
            };
            let (s0, t0) = (solid.len() as u32, translucent.len() as u32);
            solid.extend_from_slice(&src_s[r[0] as usize..r[1] as usize]);
            translucent.extend_from_slice(&src_t[r[2] as usize..r[3] as usize]);
            ranges.push([s0, solid.len() as u32, t0, translucent.len() as u32]);
        }
        let items_end = (solid.len(), translucent.len());
        solid.extend_from_slice(&old.solid[self.items_end.0..]);
        translucent.extend_from_slice(&old.translucent[self.items_end.1..]);
        self.items_end = items_end;
        b.ranges = Some(ranges);
        b.put_states(states);
        self.parts = Some(b.into_parts());
        Some(Mesh { solid, translucent, sprites: old.sprites.clone(), flames: old.flames.clone(), lo: old.lo, hi: old.hi })
    }

    /// (For a test: the cells whose block or light differ from `other`'s.)
    #[cfg(test)]
    fn differences(&self, other: &Remesh) -> (Vec<String>, Option<Vec<Pos>>) {
        let ours: HashMap<Pos, &str> = self.blocks.iter().zip(&self.removed).filter(|(_, r)| !**r).map(|(b, _)| (b.cell, b.state.as_str())).collect();
        let theirs: HashMap<Pos, &str> = other.blocks.iter().zip(&other.removed).filter(|(_, r)| !**r).map(|(b, _)| (b.cell, b.state.as_str())).collect();
        let mut blocks: Vec<String> = ours.iter().filter(|(c, s)| theirs.get(*c) != Some(*s)).map(|(c, s)| format!("{c:?} ours {s} theirs {:?}", theirs.get(c))).collect();
        blocks.extend(theirs.iter().filter(|(c, _)| !ours.contains_key(*c)).map(|(c, s)| format!("{c:?} ours none theirs {s}")));
        let light = match (self.parts.as_ref().and_then(|p| p.light()), other.parts.as_ref().and_then(|p| p.light())) {
            (Some(a), Some(b)) => a.changed_from(b),
            _ => None,
        };
        (blocks, light)
    }
}

#[cfg(test)]
mod tests {
    use crate::atlas::Atlas;
    use crate::mesh::{Mesh, Vertex};

    fn read(path: &str) -> String {
        std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// A mesh's triangles and translucent quads, each as its bytes, sorted (the same triangles in
    /// any order compare equal).
    fn triangles(m: &Mesh) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        let mut s: Vec<Vec<u8>> = m.solid.chunks(3).map(|t| bytemuck::cast_slice::<Vertex, u8>(t).to_vec()).collect();
        let mut t: Vec<Vec<u8>> = m.translucent.iter().map(|q| bytemuck::cast_slice::<Vertex, u8>(&q.v).to_vec()).collect();
        s.sort_unstable();
        t.sort_unstable();
        (s, t)
    }

    /// `design` played `ticks` ticks `per` a frame, its blocks' mesh brought up to date each frame
    /// (updated in place when blocks change) and, every time it changed, built whole from the
    /// session's blocks of then and compared: (frames whose mesh changed, the same as whole).
    fn check(atlas: &Atlas, design: &str, ticks: u32, per: u32) -> (u32, u32) {
        let text = read(design);
        let mut p = crate::player::Player::new_web(&text, 1.0).unwrap();
        let (mut changed, mut same) = (0, 0);
        let mut key = 0;
        for _ in 0..ticks / per {
            for _ in 0..per {
                p.session.step();
            }
            p.triangles_split_within(atlas, &mut || false).unwrap();
            if p.fixed_key() == key {
                continue;
            }
            key = p.fixed_key();
            changed += 1;
            let look = p.look.clone();
            let mut st = crate::scene::Staged::new(&p.session, 0.0, &look, crate::scene::Part::Blocks).keeping();
            let whole = st.step(atlas, &p.session, 0.0, &look, &mut || false).unwrap();
            let whole_remesh = st.take_remesh();
            let ours = p.blocks_mesh().unwrap();
            let (a, b) = (triangles(ours), triangles(&whole));
            assert_eq!(a.0.len(), b.0.len(), "{design} tick {}: the solid triangles updated in place {} against {} built whole", p.session.tick, a.0.len(), b.0.len());
            if a.0 != b.0 {
                // (where they differ: the triangles in one and not the other, by their first vertex)
                let only = |x: &Vec<Vec<u8>>, y: &Vec<Vec<u8>>| -> Vec<String> {
                    let ys: std::collections::HashSet<&Vec<u8>> = y.iter().collect();
                    x.iter().filter(|t| !ys.contains(t)).take(3).map(|t| format!("{:?}", bytemuck::cast_slice::<u8, Vertex>(t).iter().map(|v| (v.pos, v.color, v.light)).collect::<Vec<_>>())).collect()
                };
                let diff = match (p.remesh(), whole_remesh.as_ref()) {
                    (Some(a), Some(b)) => format!("{:?}", a.differences(b)),
                    _ => "no remesh to compare".into(),
                };
                panic!("{design} tick {}: the solid triangles updated in place are not the whole build's:\n in place only: {:?}\n whole only: {:?}\n blocks and light that differ: {diff}", p.session.tick, only(&a.0, &b.0), only(&b.0, &a.0));
            }
            assert!(a.1 == b.1, "{design} tick {}: the translucent quads updated in place are not the whole build's ({} against {})", p.session.tick, a.1.len(), b.1.len());
            same += 1;
        }
        (changed, same)
    }

    /// Farms in their terrain whose blocks change often (grass dying and spreading, pressure
    /// plates; fire and its light in cross-4-underground): after every change the mesh updated in
    /// place has exactly the triangles of the mesh built whole. `--ignored` (a minute).
    #[test]
    #[ignore]
    fn updated_in_place_is_built_whole() {
        let mut atlas = Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, w, h) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, w, h, &idx, &defs, &models).unwrap();
        for (design, ticks, per) in [("designs/farms/iron/day-1/bed-plate-two-clusters.txt", 2000, 10), ("designs/farms/iron/middle/cross-4-underground.txt", 800, 4), ("designs/farms/iron/day-1/dust-pulse-out-of-sight.txt", 2000, 10)] {
            let (changed, same) = check(&atlas, design, ticks, per);
            eprintln!("{design}: {changed} frames changed the mesh, {same} the same as built whole");
            assert!(changed > 1, "{design}: its blocks never changed");
        }
    }
}
