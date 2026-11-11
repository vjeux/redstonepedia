//! A terrain sample as a redsim world (the terrain lane; brief §2-3 and "Placement"): the cells,
//! the game's light (the saved sky and block light, kept current around every change by the
//! engines' rules), the heightmaps, the biomes (`BiomeManager.getBiome`), the world spawn.
//!
//! The light starts from what the game saved, not from a fresh fixed point: the game's own light is
//! not always the fixed point (a tree a neighbour chunk's features grew after this chunk's sky
//! sources were filled left its leaves out of them when the light was made: about 0.04% of a
//! forest's open cells read one or two more than the fixed point, which `mctas terrain probe`
//! confirms at run time). The sky sources are not saved: the game fills them from the blocks when it
//! loads a chunk (leaves stop them), and so does `sky_sources`; the first light check in such a
//! column then lowers the stale cells (`removeSourcesBelow`). Around a change, the sky sources
//! follow `ChunkSkyLightSources.update` step by step and the light settles with the game's
//! incremental engines (`terrain::engine`).

use super::blocks;
use super::format::{Heightmap, Terrain};
use super::light::{self, LightProps};
use crate::block::Block;
use crate::pos::BlockPos;
use crate::world::World;

/// The terrain state of a world.
#[derive(Clone, Debug)]
pub struct Land {
    pub seed: i64,
    pub zoom_seed: i64,
    /// The world spawn: no natural spawn within 24 of it.
    pub spawn: BlockPos,
    pub world_min_y: i32,
    pub world_height: i32,
    pub biome_names: Vec<String>,
    pub biome_origin: [i32; 3],
    pub biome_size: [i32; 3],
    pub biomes: Vec<u16>,
    /// Sky and block light per cell of the world box.
    pub sky: Vec<u8>,
    pub block: Vec<u8>,
    /// Per column (x fastest, then z): the lowest sky source y (`ChunkSkyLightSources`), the
    /// box's top when the column's sources end above the box.
    pub sources: Vec<i32>,
    /// Per heightmap, per column: `getHeight` (the first free y).
    pub heights: Vec<Vec<i32>>,
    /// Cells changed since the light was last settled (settled at the next tick's start: the light
    /// engine's timing, see `World::light_index`), each with the y span its change reaches in its
    /// column: the cell, and the sky source's old and new heights when the change moved it
    /// (tickexact 2026-11-08: the span ran from the cell to its column's source even when the
    /// source stayed, 150 cells tall under a deep change; light moves 15 at most).
    pub dirty: Vec<(BlockPos, i32, i32)>,
    /// The structure starts (their boxes and pieces: the spawn overrides).
    pub structures: Vec<crate::terrain::format::Structure>,
    /// What the light engine approximated (changes within 15 of the box's sides or bottom, whose
    /// light from outside the box is not known), once each.
    pub notes: Vec<String>,
    /// (tickexact 2026-11-10) The stored light of the cells outside the box that the game's engines
    /// light ([`World::stores_light_outside`], `terrain::engine`): the bridge imports whole sections
    /// (`mobfarm::exact::import_terrain`), so the sample's chunks outside the box are air holding
    /// light 0, the sandbox's void chunks around them hold 15, and the section under the box holds
    /// data too. Water pouring out of a cave at the box's side lit the cells beside it, and that
    /// light came back into the box through the cave (d1-bed-plate plains t36936: grass in the
    /// cave stayed dark in redsim). Absent: that default.
    pub outer_sky: crate::fxhash::FxHashMap<BlockPos, u8>,
    pub outer_block: crate::fxhash::FxHashMap<BlockPos, u8>,
}

/// The redsim block of a natural state: air, the fluids redsim models (water, lava, bubble
/// columns: their flow and the mobs' physics in them), a state redsim models whose game string is
/// exactly this one, else `Block::Terrain(row)`.
pub fn block_of(state: &str) -> Result<Block, String> {
    let id = blocks::id_of(state).ok_or_else(|| format!("{state} is not in the natural-state table (mctas terrain blockinfo)"))?;
    let canonical = &blocks::props(id).name;
    if canonical == "minecraft:air" || canonical == "minecraft:cave_air" || canonical == "minecraft:void_air" {
        return Ok(if canonical == "minecraft:air" { Block::Air } else { Block::Terrain(id) });
    }
    let name = canonical.split('[').next().unwrap_or(canonical);
    if matches!(name, "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column") {
        if let Ok(b) = Block::parse(canonical)
            && b.to_string() == *canonical
        {
            return Ok(b);
        }
        // flowing lava and the rest: inert natural states
        return Ok(Block::Terrain(id));
    }
    // (mob-AI) natural sand, red sand and gravel are redsim's falling blocks: an update (a block
    // taken from under them: an enderman's) schedules their fall (FallingBlock.updateShape)
    if matches!(name, "minecraft:sand" | "minecraft:red_sand" | "minecraft:gravel")
        && let Ok(b) = Block::parse(canonical)
        && b.to_string() == *canonical
    {
        return Ok(b);
    }
    Ok(Block::Terrain(id))
}

/// The cells of a terrain box as redsim blocks, its palette mapped once.
fn cells_of(t: &Terrain) -> Result<Vec<Block>, String> {
    let palette: Vec<Block> = t.palette.iter().map(|s| block_of(s)).collect::<Result<_, _>>()?;
    Ok(t.cells.iter().map(|&c| palette[c as usize]).collect())
}

/// (tickexact 2026-11-11, ironspec's surroundings question) A terrain world of the box `lo`..`lo +
/// size` holding `blocks` (a design with its ground baked in, `mobfarm ground`): its light computed
/// by the light engines from the blocks (as [`build_world`] does for a terrain file without saved
/// light), one biome (plains) and its heightmaps at the box's bottom (a bench: no weather, no
/// natural spawning reads them).
pub fn world_of_blocks(lo: BlockPos, size: [i32; 3], blocks: &[(BlockPos, Block)]) -> Result<World, String> {
    let mut palette: Vec<String> = vec!["minecraft:air".to_string()];
    let mut index: std::collections::HashMap<String, u16> = std::collections::HashMap::new();
    index.insert(palette[0].clone(), 0);
    let n = (size[0] * size[1] * size[2]) as usize;
    let mut cells = vec![0u16; n];
    for (p, b) in blocks {
        let (x, y, z) = (p.x - lo.x, p.y - lo.y, p.z - lo.z);
        if x < 0 || y < 0 || z < 0 || x >= size[0] || y >= size[1] || z >= size[2] {
            continue;
        }
        let s = b.to_string();
        // (only the natural states go in the terrain box: the design's own blocks are placed by the
        // run after it, the light engines settling round them)
        if blocks::id_of(&s).is_none() {
            continue;
        }
        let k = match index.get(&s) {
            Some(k) => *k,
            None => {
                let k = palette.len() as u16;
                palette.push(s.clone());
                index.insert(s, k);
                k
            }
        };
        cells[((y * size[2] + z) * size[0] + x) as usize] = k;
    }
    let bsize = [(size[0] + 3) / 4 + 1, (size[1] + 3) / 4 + 1, (size[2] + 3) / 4 + 1];
    let t = Terrain {
        world_min_y: -64,
        world_height: 384,
        origin: [lo.x, lo.y, lo.z],
        size,
        palette,
        cells,
        biome_palette: vec!["minecraft:plains".to_string()],
        biome_origin: [lo.x.div_euclid(4), lo.y.div_euclid(4), lo.z.div_euclid(4)],
        biome_size: bsize,
        biomes: vec![0; (bsize[0] * bsize[1] * bsize[2]) as usize],
        ..Terrain::default()
    };
    build_world(&t)
}

/// A world holding a terrain file's whole box, its light, heightmaps and biomes.
pub fn build_world(t: &Terrain) -> Result<World, String> {
    if !t.sky.is_empty() && !t.block.is_empty() {
        let mut b = Builder::new(t)?;
        while !b.step(usize::MAX) {}
        return Ok(b.finish());
    }
    let min = BlockPos::new(t.origin[0], t.origin[1], t.origin[2]);
    let mut w = World::new(min, t.size);
    let cells = cells_of(t)?;
    let sources = sky_sources(t, &cells, 0, (t.size[0] * t.size[2]) as usize);
    w.load_cells(cells);
    let g = light::grid_of_world(&w)?;
    let (sky, block) = light::compute(&g);
    w.land = Some(Box::new(land_of(t, sky, block, sources)));
    Ok(w)
}

/// `build_world` in resumable steps, for a file with its saved light (the viewer keeps every
/// main-thread task under 50 ms): `Builder::new(&t)?`, `step(max_cells)` until it returns true,
/// then `finish()` — the very world `build_world(&t)` returns.
pub struct Builder<'a> {
    t: &'a Terrain,
    palette: Vec<Block>,
    cells: Vec<Block>,
    sky: Vec<u8>,
    block: Vec<u8>,
    sources: Vec<i32>,
}

impl<'a> Builder<'a> {
    pub fn new(t: &'a Terrain) -> Result<Builder<'a>, String> {
        if t.sky.is_empty() || t.block.is_empty() {
            return Err("a terrain file without its saved light (build_world computes it in one step)".into());
        }
        let palette: Vec<Block> = t.palette.iter().map(|s| block_of(s)).collect::<Result<_, _>>()?;
        let n = t.cells.len();
        let cols = (t.size[0] * t.size[2]) as usize;
        Ok(Builder { t, palette, cells: Vec::with_capacity(n), sky: Vec::with_capacity(n), block: Vec::with_capacity(n), sources: Vec::with_capacity(cols) })
    }

    /// Up to `max_cells` cells of work (a cell's block; 8 cells of saved light count as one; a
    /// column's sky-source scan counts as its height); true when all is done.
    pub fn step(&mut self, max_cells: usize) -> bool {
        let t = self.t;
        let n = t.cells.len();
        let mut budget = max_cells.max(1);
        if self.cells.len() < n {
            let (a, b) = (self.cells.len(), self.cells.len().saturating_add(budget).min(n));
            let palette = &self.palette;
            self.cells.extend(t.cells[a..b].iter().map(|&c| palette[c as usize]));
            budget -= b - a;
            if budget == 0 {
                return false;
            }
        }
        while self.sky.len() < n || self.block.len() < n {
            let take = budget.saturating_mul(8).max(1);
            let (a, b) = (self.sky.len(), self.sky.len().saturating_add(take).min(n));
            self.sky.extend_from_slice(&t.sky[a..b]);
            self.block.extend_from_slice(&t.block[a..b]);
            budget = budget.saturating_sub((b - a).div_ceil(8));
            if budget == 0 {
                return self.sky.len() == n && self.sources.len() == (t.size[0] * t.size[2]) as usize;
            }
        }
        let cols = (t.size[0] * t.size[2]) as usize;
        while self.sources.len() < cols {
            let per = (t.size[1] as usize).max(1);
            let k = (budget / per).max(1).min(cols - self.sources.len());
            let from = self.sources.len();
            let more = sky_sources(t, &self.cells, from, from + k);
            self.sources.extend(more);
            budget = budget.saturating_sub(k * per);
            if budget == 0 {
                return self.sources.len() == cols;
            }
        }
        true
    }

    /// The world (after `step` returned true).
    pub fn finish(self) -> World {
        let t = self.t;
        assert!(self.cells.len() == t.cells.len() && self.sources.len() == (t.size[0] * t.size[2]) as usize, "Builder::finish before its last step");
        let mut w = World::with_cells(BlockPos::new(t.origin[0], t.origin[1], t.origin[2]), t.size, self.cells);
        w.land = Some(Box::new(land_of(t, self.sky, self.block, self.sources)));
        w
    }
}

/// Columns `from..to` (x fastest, then z): `ChunkSkyLightSources.fillFrom`'s lowest source y, from
/// the blocks as the game computes it when it loads a chunk (`initializeLightSources`): down from
/// the box's top (air above it), the first edge `isEdgeOccluded` closes ends the sources at the cell
/// above it. Not from the saved light (tickexact 2026-11-10, forest's terrain compare t60549): the
/// game saves stale sky light under leaves a neighbour chunk's features added (15 where the fixed
/// point has 14), but its sources at load stop at those leaves, so the first check in the column
/// (`removeSourcesBelow`) lowers them; sources read from that light kept them at 15 for good.
fn sky_sources(t: &Terrain, cells: &[Block], from: usize, to: usize) -> Vec<i32> {
    let [sx, sy, sz] = t.size;
    let props = |b: &Block| match *b {
        Block::Terrain(id) => terrain_lprops(id),
        _ => lprops(b),
    };
    let air = lprops(&Block::Air);
    (from..to)
        .map(|c| {
            let (x, z) = ((c as i32) % sx, (c as i32) / sx);
            let mut top = air;
            for y in (0..sy).rev() {
                let bottom = props(&cells[((y * sz + z) * sx + x) as usize]);
                if light::edge_occluded_pub(&top, &bottom) {
                    return t.origin[1] + y + 1;
                }
                top = bottom;
            }
            t.origin[1]
        })
        .collect()
}

/// The land of a terrain file with its light and sky sources.
fn land_of(t: &Terrain, sky: Vec<u8>, block: Vec<u8>, sources: Vec<i32>) -> Land {
    let cols = (t.size[0] * t.size[2]) as usize;
    let heights = (0..4)
        .map(|m| match t.heightmaps.get(m) {
            Some(h) if h.len() == cols => h.iter().map(|v| *v as i32).collect(),
            _ => vec![t.origin[1]; cols],
        })
        .collect();
    Land {
        seed: t.seed,
        zoom_seed: t.zoom_seed,
        spawn: BlockPos::new(t.spawn[0], t.spawn[1], t.spawn[2]),
        world_min_y: t.world_min_y,
        world_height: t.world_height,
        biome_names: t.biome_palette.clone(),
        biome_origin: t.biome_origin,
        biome_size: t.biome_size,
        biomes: t.biomes.clone(),
        sky,
        block,
        sources,
        heights,
        dirty: Vec::new(),
        structures: t.structures.clone(),
        notes: Vec::new(),
        outer_sky: Default::default(),
        outer_block: Default::default(),
    }
}

/// The one-step body `Builder` replaced (the test compares the two).
#[cfg(test)]
fn build_world_one_step(t: &Terrain) -> Result<World, String> {
    let min = BlockPos::new(t.origin[0], t.origin[1], t.origin[2]);
    let mut w = World::new(min, t.size);
    let cells = cells_of(t)?;
    let cols = (t.size[0] * t.size[2]) as usize;
    // the sky sources from the blocks (`sky_sources`: as the game fills them at load)
    let sources = sky_sources(t, &cells, 0, cols);
    w.load_cells(cells);
    let (sky, block) = if !t.sky.is_empty() && !t.block.is_empty() {
        (t.sky.clone(), t.block.clone())
    } else {
        let g = light::grid_of_world(&w)?;
        light::compute(&g)
    };
    let heights = (0..4)
        .map(|m| match t.heightmaps.get(m) {
            Some(h) if h.len() == cols => h.iter().map(|v| *v as i32).collect(),
            _ => vec![t.origin[1]; cols],
        })
        .collect();
    w.land = Some(Box::new(Land {
        seed: t.seed,
        zoom_seed: t.zoom_seed,
        spawn: BlockPos::new(t.spawn[0], t.spawn[1], t.spawn[2]),
        world_min_y: t.world_min_y,
        world_height: t.world_height,
        biome_names: t.biome_palette.clone(),
        biome_origin: t.biome_origin,
        biome_size: t.biome_size,
        biomes: t.biomes.clone(),
        sky,
        block,
        sources,
        heights,
        dirty: Vec::new(),
        structures: t.structures.clone(),
        notes: Vec::new(),
        outer_sky: Default::default(),
        outer_block: Default::default(),
    }));
    Ok(w)
}

impl Land {
    pub(crate) fn col(w_min: BlockPos, size: [i32; 3], x: i32, z: i32) -> Option<usize> {
        let (lx, lz) = (x - w_min.x, z - w_min.z);
        (lx >= 0 && lz >= 0 && lx < size[0] && lz < size[2]).then(|| (lz * size[0] + lx) as usize)
    }
}

/// `Heightmap.Types.isOpaque` of a block, for the four saved maps.
fn opaque(b: &Block, map: usize) -> bool {
    match *b {
        Block::Air => false,
        Block::Terrain(id) => blocks::props(id).heightmaps[map],
        _ => match blocks::id_of(&b.to_string()) {
            Some(id) => blocks::props(id).heightmaps[map],
            // a modeled block missing from the table (a placed one: redstone dust, a torch, a
            // rail): WORLD_SURFACE takes every block that is not air (`Heightmap.Types` NOT_AIR,
            // tickexact: redsim left dust out of it, so natural spawning's getRandomPosWithin read
            // a column one block low); the others anything with a collision box or a fluid
            None if map == Heightmap::WorldSurface.index() => !b.is_air(),
            None => crate::behavior::collides(b) || !crate::fluid::fluid_state_of(b).is_empty(),
        },
    }
}

/// The light behaviour of a block (a block missing from the table counts as opaque, and the change
/// is recorded as a model error by the caller).
pub(crate) fn lprops(b: &Block) -> LightProps {
    light::props_of(b).unwrap_or(LightProps { dampening: 15, emission: 0, faces: None })
}

/// [`lprops`] of a natural state, per row of the table, computed once (tickexact 2026-11-08: the
/// relight hashed every cell's block into a cache).
pub(crate) fn terrain_lprops(id: u16) -> LightProps {
    static ROWS: std::sync::OnceLock<Vec<LightProps>> = std::sync::OnceLock::new();
    let rows = ROWS.get_or_init(|| (0..crate::terrain::blocks::table().props.len()).map(|i| lprops(&Block::Terrain(i as u16))).collect());
    rows.get(id as usize).copied().unwrap_or_else(|| lprops(&Block::Terrain(id)))
}

/// `ChunkSkyLightSources.isEdgeOccluded` through the light module's rules.
fn edge(top: &Block, bottom: &Block) -> bool {
    light::edge_occluded_pub(&lprops(top), &lprops(bottom))
}

impl World {
    /// Called by `World::put` for every changed cell of a world with land: the heightmaps
    /// (`Heightmap.update`) and the sky sources (`ChunkSkyLightSources.update`) follow the change
    /// at once, the light is settled later (`settle_light`).
    pub(crate) fn land_changed(&mut self, p: BlockPos, old: Block, new: Block) {
        let (min, size) = (self.min, self.size);
        let Some(c) = Land::col(min, size, p.x, p.z) else { return };
        let get = |w: &World, y: i32| w.get(BlockPos::new(p.x, y, p.z));
        // heightmaps
        let mut heights = [0i32; 4];
        for (m, h) in heights.iter_mut().enumerate() {
            let first = self.land.as_ref().map(|l| l.heights[m][c]).unwrap_or(min.y);
            *h = first;
            if p.y <= first - 2 {
                continue;
            }
            if opaque(&new, m) {
                if p.y >= first {
                    *h = p.y + 1;
                }
            } else if first - 1 == p.y {
                let mut y = p.y - 1;
                while y >= min.y && !opaque(&get(self, y), m) {
                    y -= 1;
                }
                *h = if y >= min.y { y + 1 } else { min.y };
            }
        }
        // sky sources
        let old_source = self.land.as_ref().map(|l| l.sources[c]).unwrap_or(min.y);
        let mut source = old_source;
        if p.y + 1 >= old_source {
            let top = get(self, p.y + 1);
            let below = get(self, p.y - 1);
            let mut handled = false;
            for (tp, ts, bs) in [(p.y + 1, top, new), (p.y, new, below)] {
                if edge(&ts, &bs) {
                    if tp > old_source {
                        source = tp;
                        handled = true;
                    }
                } else if tp == old_source {
                    // findLowestSourceBelow
                    let mut t_y = tp - 1;
                    let mut t_s = bs;
                    loop {
                        if t_y - 1 < min.y {
                            source = min.y;
                            break;
                        }
                        let b_s = get(self, t_y - 1);
                        if edge(&t_s, &b_s) {
                            source = t_y;
                            break;
                        }
                        t_s = b_s;
                        t_y -= 1;
                    }
                    handled = true;
                }
                if handled {
                    break;
                }
            }
        }
        let relevant = if super::engine::enabled() {
            // the game's engines check exactly the cells hasDifferentLightProperties names
            light::different_light(&old, &new)
        } else {
            match (light::props_of(&old), light::props_of(&new)) {
                (Ok(a), Ok(b)) => a != b,
                // a state the table does not know: settle_light reports it
                _ => true,
            }
        };
        if let Some(l) = self.land.as_mut() {
            for (m, h) in heights.into_iter().enumerate() {
                l.heights[m][c] = h;
            }
            l.sources[c] = source;
            if source != old_source {
                l.dirty.push((p, p.y.min(old_source).min(source), p.y.max(old_source).max(source)));
            } else if relevant {
                l.dirty.push((p, p.y, p.y));
            }
        }
    }

    /// Settle the light around the cells changed since the last call: inside the region every
    /// changed cell's light can reach (15 around it, and 15 around every cell whose sky-source
    /// status changed), recompute the engines' fixed point from the sources and emitters inside and
    /// the unchanged cells around it.
    pub fn settle_light(&mut self) {
        let Some(land) = self.land.as_ref() else { return };
        if land.dirty.is_empty() {
            return;
        }
        // (tickexact 2026-11-10: the game's incremental engines, `terrain::engine`; the fixed point
        // below with `MCTAS_LIGHT_ENGINE=0`)
        if super::engine::enabled() {
            super::engine::run(self);
            return;
        }
        let (min, size) = (self.min, self.size);
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for &(p, y0, y1) in &land.dirty {
            let (ylo, yhi) = (y0 - 1, y1 + 1);
            for (a, (l, h)) in [(p.x, p.x), (ylo, yhi), (p.z, p.z)].into_iter().enumerate() {
                lo[a] = lo[a].min(l - 15);
                hi[a] = hi[a].max(h + 15);
            }
        }
        let boxlo = [min.x, min.y, min.z];
        let boxhi = [min.x + size[0] - 1, min.y + size[1] - 1, min.z + size[2] - 1];
        let touches_side = lo[0] < boxlo[0] || lo[2] < boxlo[2] || hi[0] > boxhi[0] || hi[2] > boxhi[2] || lo[1] < boxlo[1];
        for a in 0..3 {
            lo[a] = lo[a].max(boxlo[a]);
            hi[a] = hi[a].min(boxhi[a]);
        }
        // (the first few only: nothing reads them, and a farm at the box's edge made one at every
        // relight)
        if touches_side && self.land.as_ref().is_some_and(|l| l.notes.len() < 8) {
            // the crop's limit: light from outside the box is not known, so cells within 15 of the
            // box's sides or bottom keep their stored values as the border (an approximation there;
            // a farm far from the edges is unaffected); reported, not an error
            let what: Vec<String> = self.land.as_ref().map(|l| l.dirty.iter().take(3).map(|(p, _, _)| format!("{p} now {}", self.get(*p))).collect()).unwrap_or_default();
            let note = format!("a light change within 15 of the terrain box's side or bottom ({lo:?}..{hi:?}; {}): the light outside the box is not known, the edge cells stay as stored", what.join(", "));
            if let Some(l) = self.land.as_mut()
                && !l.notes.contains(&note)
            {
                l.notes.push(note);
            }
        }
        self.relight(lo, hi);
        if let Some(l) = self.land.as_mut() {
            l.dirty.clear();
        }
    }

    /// The fixed point inside `lo..=hi` with the cells around it as its border (their light is
    /// unchanged and reaches in through the faces the engines let it cross).
    fn relight(&mut self, lo: [i32; 3], hi: [i32; 3]) {
        // the region and a one-cell border (inside the box), with every cell's light behaviour
        let (min, size) = (self.min, self.size);
        let elo = [(lo[0] - 1).max(min.x), (lo[1] - 1).max(min.y), (lo[2] - 1).max(min.z)];
        let ehi = [(hi[0] + 1).min(min.x + size[0] - 1), (hi[1] + 1).min(min.y + size[1] - 1), (hi[2] + 1).min(min.z + size[2] - 1)];
        let n = [ehi[0] - elo[0] + 1, ehi[1] - elo[1] + 1, ehi[2] - elo[2] + 1];
        let idx = |x: i32, y: i32, z: i32| (((y - elo[1]) * n[2] + (z - elo[2])) * n[0] + (x - elo[0])) as usize;
        let mut cache: crate::fxhash::FxHashMap<Block, LightProps> = Default::default();
        let mut props = Vec::with_capacity((n[0] * n[1] * n[2]) as usize);
        // (the region's rows read from the box's array, each row one index stepped along x: the
        // extended region lies in the box)
        for y in elo[1]..=ehi[1] {
            for z in elo[2]..=ehi[2] {
                let base = self.index(BlockPos::new(elo[0], y, z)).expect("the relit region lies in the box");
                for k in 0..n[0] as usize {
                    let b = self.cells[base + k];
                    props.push(match b {
                        // a natural state's from the per-row table, without hashing the block
                        Block::Terrain(id) => terrain_lprops(id),
                        _ => *cache.entry(b).or_insert_with(|| lprops(&b)),
                    });
                }
            }
        }
        // each direction's step in the region's array
        let stride = light::DIRS_PUB.map(|(_, [dx, dy, dz])| dx as isize + (dz * n[0]) as isize + (dy * n[0] * n[2]) as isize);
        let land = self.land.as_mut().unwrap();
        let gidx = |x: i32, y: i32, z: i32| (((y - min.y) * size[2] + (z - min.z)) * size[0] + (x - min.x)) as usize;
        let inside = |x: i32, y: i32, z: i32| x >= lo[0] && x <= hi[0] && y >= lo[1] && y <= hi[1] && z >= lo[2] && z <= hi[2];
        // a cell on the box's side: light may reach it from outside the box, where redsim knows
        // nothing (the sample's neighbours in the world it was cut from); the game keeps such light
        // until an update of its own reaches it (tickexact 2026-11-08: d1u-deep's west edge kept the
        // sample's block light 5 through the settle, where this fixed point had made it 0), so a
        // side cell keeps at least its stored light
        let side = |x: i32, z: i32| x == min.x || z == min.z || x == min.x + size[0] - 1 || z == min.z + size[2] - 1;
        for sky in [true, false] {
            let mut level = vec![0u8; props.len()];
            let mut queue: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); 16];
            for y in elo[1]..=ehi[1] {
                for z in elo[2]..=ehi[2] {
                    for x in elo[0]..=ehi[0] {
                        let i = idx(x, y, z);
                        let stored = if sky { land.sky[gidx(x, y, z)] } else { land.block[gidx(x, y, z)] };
                        let v = if !inside(x, y, z) {
                            // the border keeps its light
                            stored
                        } else if sky {
                            let c = Land::col(min, size, x, z).unwrap();
                            let v = if y >= land.sources[c] { 15 } else { 0 };
                            if side(x, z) { v.max(stored) } else { v }
                        } else {
                            let v = props[i].emission.min(15);
                            if side(x, z) { v.max(stored) } else { v }
                        };
                        level[i] = v;
                        if v > 1 {
                            queue[v as usize].push((x, y, z));
                        }
                    }
                }
            }
            for l in (2..=15usize).rev() {
                while let Some((x, y, z)) = queue[l].pop() {
                    let i = idx(x, y, z);
                    if level[i] as usize != l {
                        continue;
                    }
                    for (k, (d, [dx, dy, dz])) in light::DIRS_PUB.into_iter().enumerate() {
                        let (qx, qy, qz) = (x + dx, y + dy, z + dz);
                        if !inside(qx, qy, qz) {
                            continue;
                        }
                        let j = (i as isize + stride[k]) as usize;
                        let new = l as i32 - props[j].dampening.max(1) as i32;
                        if new > level[j] as i32 && !light::shape_occludes_pub(&props[i], &props[j], d) {
                            level[j] = new as u8;
                            if new > 1 {
                                queue[new as usize].push((qx, qy, qz));
                            }
                        }
                    }
                }
            }
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    // (a row: one index stepped along x on each side)
                    let (li, gi) = (idx(lo[0], y, z), gidx(lo[0], y, z));
                    let len = (hi[0] - lo[0] + 1) as usize;
                    let dst = if sky { &mut land.sky[gi..gi + len] } else { &mut land.block[gi..gi + len] };
                    dst.copy_from_slice(&level[li..li + len]);
                }
            }
        }
    }
}

impl World {
    /// The light behind a read: the light as the previous tick (and any edit between ticks) left
    /// the blocks. The game's light engine (`ThreadedLevelLightEngine`) works off the main thread: a
    /// block change's `checkBlock` is queued (its task dispatcher, then the "light" executor's task
    /// list), and an update runs only when the main thread's `ServerChunkCache.MainThreadExecutor
    /// .pollTask` calls `tryScheduleUpdate` — between ticks (`waitUntilNextTick`'s managed block), so
    /// the light thread publishes (`swapSectionMap`) the changes of tick T in the gap before tick
    /// T + 1; reads during tick T see the light of the end of tick T - 1. redsim settles the changes
    /// at the start of the next tick (`World::tick` -> `settle_light`), the same timing (the bridge's
    /// sandbox drains the light engine before and after every tick: the tick-exact compare's
    /// reference).
    pub(crate) fn light_index(&self, p: BlockPos) -> Option<usize> {
        self.index(p)
    }

    /// `getBrightness(LightLayer.SKY, pos)`: the raw sky light (no darkening). A world without land
    /// reads daylight.rs's per-query fixed point (`Err` on unclassified blocks, as before); outside
    /// the box: [`World::sky_light_outside`].
    pub fn sky_light(&self, p: BlockPos) -> Result<i32, String> {
        match self.land.as_ref() {
            Some(l) => Ok(match self.light_index(p) {
                Some(i) => l.sky[i] as i32,
                None => self.sky_light_outside(l, p),
            }),
            None => crate::daylight::sky_light(self, p),
        }
    }

    /// (tickexact 2026-11-10) The chunks the sample's sections fill, `[min cx, min cz]` and
    /// `[max cx, max cz]`: the bridge imports them whole, so the game stores light for every cell
    /// of them in the box's y range, outside the box too ([`Land::outer_sky`]).
    pub fn stored_chunks(&self) -> ([i32; 2], [i32; 2]) {
        let (x1, z1) = (self.min.x + self.size[0] - 1, self.min.z + self.size[2] - 1);
        ([self.min.x.div_euclid(16), self.min.z.div_euclid(16)], [x1.div_euclid(16), z1.div_euclid(16)])
    }

    /// Whether a cell outside the box is one the game's light engines may store: in the sample's
    /// chunks ([`World::stored_chunks`]) or the sandbox's void chunks around them, within the box's
    /// y range. False inside the box.
    pub fn stores_light_outside(&self, p: BlockPos) -> bool {
        self.outside_chunk(p).is_some()
    }

    /// For a cell outside the box within its y range: `Some(true)` in one of the sample's chunks,
    /// `Some(false)` in a void chunk around them, `None` past those.
    pub(crate) fn outside_chunk(&self, p: BlockPos) -> Option<bool> {
        if self.land.is_none() || self.contains(p) || p.y < self.min.y || p.y >= self.min.y + self.size[1] {
            return None;
        }
        let (lo, hi) = self.stored_chunks();
        let (cx, cz) = (p.x.div_euclid(16), p.z.div_euclid(16));
        if cx >= lo[0] && cx <= hi[0] && cz >= lo[1] && cz <= hi[1] {
            Some(true)
        } else if cx >= lo[0] - 1 && cx <= hi[0] + 1 && cz >= lo[1] - 1 && cz <= hi[1] + 1 {
            Some(false)
        } else {
            None
        }
    }

    /// The highest non-empty section of a chunk (its cells in the box: the highest non-air cell
    /// of its columns, `Heightmap.WORLD_SURFACE`; and what the natural updates wrote outside the
    /// box), `None` when it has none.
    fn highest_nonempty_section(&self, l: &Land, cx: i32, cz: i32) -> Option<i32> {
        let mut top = self.min.y - 1;
        let m = Heightmap::WorldSurface.index();
        for z in (cz * 16).max(self.min.z)..=(cz * 16 + 15).min(self.min.z + self.size[2] - 1) {
            for x in (cx * 16).max(self.min.x)..=(cx * 16 + 15).min(self.min.x + self.size[0] - 1) {
                if let Some(c) = Land::col(self.min, self.size, x, z) {
                    top = top.max(l.heights[m][c] - 1);
                }
            }
        }
        for (p, b) in self.outer.iter() {
            if !b.is_air() && p.x.div_euclid(16) == cx && p.z.div_euclid(16) == cz {
                top = top.max(p.y);
            }
        }
        (top >= self.min.y).then_some(top >> 4)
    }

    /// The section from which a stored chunk's sky reads 15 (`SkyLightSectionStorage.topSections`):
    /// a section holds light data while it or one of its 26 neighbours is non-empty
    /// (`LayerLightSectionStorage.updateSectionStatus`), so the sky reads 15 two sections above the
    /// highest non-empty one of the chunk and the 8 around it.
    pub(crate) fn chunk_top_section(&self, l: &Land, cx: i32, cz: i32) -> i32 {
        let mut top = None;
        for dz in -1..=1 {
            for dx in -1..=1 {
                top = top.max(self.highest_nonempty_section(l, cx + dx, cz + dz));
            }
        }
        top.map(|s| s + 2).unwrap_or(self.min.y >> 4)
    }

    /// The sky light of a cell outside the box as the game's sandbox reads it
    /// (`SkyLightSectionStorage.getLightValue`): above the box open sky, below it dark; below a
    /// chunk's top section ([`World::chunk_top_section`]) its stored light ([`Land::outer_sky`]:
    /// the import's 0 in the sample's chunks, the lit void's 15 in the chunks around them, until an
    /// engine writes it), 15 from there up; past those chunks, 15 at every height.
    fn sky_light_outside(&self, l: &Land, p: BlockPos) -> i32 {
        if p.y >= self.min.y + self.size[1] {
            return 15;
        }
        if p.y < self.min.y {
            // (mob-AI 2026-11-11, tickexact's reading of SkyLightSectionStorage.getLightValue) the
            // storage keeps one extra section under the world (minSection - 1): a cell in it reads
            // its own value, a deeper one that section's bottom row (getFlatIndex, y = minY - 16).
            // That section is not in the sample: the light engine fills it from the column's sky
            // sources (propagateLightSources), and the import's saved light then overwrites only
            // the sample's own sections. So a column with no block at all (its sources at the
            // bottom: the sample's air ring past the box) has 15 under the world even where its
            // imported light above is dark (module-underground plains b0 t42022: an enderman
            // falling past the box's edge saw the sky at y -66 and -68, not at -64). A covered
            // column's light falls by 1 a block from the world's bottom.
            let y = p.y.max(self.min.y - 16);
            // past the box every column was air when the light engine filled that section (the
            // import's air ring, the sandbox's void chunks); a block formed there later darkens
            // only cells still lit above it, so the section keeps its 15
            let open = match Land::col(self.min, self.size, p.x, p.z) {
                Some(c) => l.sources[c] <= self.min.y,
                None => true,
            };
            if open {
                return 15;
            }
            let bottom = self.sky_light(BlockPos::new(p.x, self.min.y, p.z)).unwrap_or(0);
            return (bottom - (self.min.y - y)).max(0);
        }
        let Some(imported) = self.outside_chunk(p) else { return 15 };
        if p.y >> 4 >= self.chunk_top_section(l, p.x.div_euclid(16), p.z.div_euclid(16)) {
            return 15;
        }
        l.outer_sky.get(&p).copied().unwrap_or(if imported { 0 } else { 15 }) as i32
    }

    /// `getBrightness(LightLayer.BLOCK, pos)` (a world without land: no block light is modeled
    /// there; `Err` when a light source is in it). Outside the box: the stored light of the
    /// sample's chunks ([`Land::outer_block`]), else 0.
    pub fn block_light(&self, p: BlockPos) -> Result<i32, String> {
        match self.land.as_ref() {
            Some(l) => Ok(match self.light_index(p) {
                Some(i) => l.block[i] as i32,
                None => l.outer_block.get(&p).copied().unwrap_or(0) as i32,
            }),
            None => Err("block light is modeled in terrain worlds only".into()),
        }
    }

    /// `getRawBrightness(pos, darken)` = `max(block, sky - darken)`.
    pub fn raw_brightness(&self, p: BlockPos, darken: i32) -> Result<i32, String> {
        let sky = self.sky_light(p)? - darken;
        Ok(sky.max(self.block_light(p)?))
    }

    /// `BlockAndLightGetter.canSeeSky(pos)`: sky light 15.
    pub fn can_see_sky(&self, p: BlockPos) -> Result<bool, String> {
        crate::terrain::land::can_see_sky(self, p)
    }

    /// `getHeight(map, x, z)`: the first free y of a column. A world without land: the map's
    /// predicate over the box's column (air above the box).
    pub fn height(&self, map: Heightmap, x: i32, z: i32) -> i32 {
        if let Some(l) = self.land.as_ref()
            && let Some(c) = Land::col(self.min, self.size, x, z)
        {
            return l.heights[map.index()][c];
        }
        let mut y = self.min.y + self.size[1] - 1;
        while y >= self.min.y && !opaque(&self.get(BlockPos::new(x, y, z)), map.index()) {
            y -= 1;
        }
        y + 1
    }

    /// `BiomeManager.getBiome(pos)` (a world without land: `None`). The zoomed quart cell reads
    /// the sample's biome cells only inside the chunks the sample's box covers (the bridge's
    /// terrain sandbox imports the cells chunk by chunk; a cell of a chunk past the box has the
    /// level's own biome: `None` here, `Natural::void_biome` for the spawner).
    pub fn biome_at(&self, p: BlockPos) -> Option<&str> {
        let l = self.land.as_ref()?;
        let (qx, qy, qz) = zoom(l.zoom_seed, p.x, p.y, p.z);
        let [ox, oy, oz] = l.biome_origin;
        let [sx, sy, sz] = l.biome_size;
        let qy = qy.clamp(l.world_min_y >> 2, ((l.world_min_y + l.world_height) >> 2) - 1);
        if qx < ox || qy < oy || qz < oz || qx >= ox + sx || qy >= oy + sy || qz >= oz + sz {
            return None;
        }
        let (cx, cz) = ((qx * 4).div_euclid(16), (qz * 4).div_euclid(16));
        let (lo, hi) = (self.min, self.min.offset(self.size[0] - 1, 0, self.size[2] - 1));
        if cx < lo.x.div_euclid(16) || cx > hi.x.div_euclid(16) || cz < lo.z.div_euclid(16) || cz > hi.z.div_euclid(16) {
            return None;
        }
        let i = (((qy - oy) * sz + (qz - oz)) * sx + (qx - ox)) as usize;
        Some(l.biome_names[l.biomes[i] as usize].as_str())
    }
}

/// `LinearCongruentialGenerator.next`.
fn lcg(r: i64, c: i64) -> i64 {
    r.wrapping_mul(r.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)).wrapping_add(c)
}

/// `BiomeManager.getFiddle`.
fn fiddle(r: i64) -> f64 {
    let uniform = ((r >> 24).rem_euclid(1024)) as f64 / 1024.0;
    (uniform - 0.5) * 0.9
}

/// `BiomeManager.getFiddledDistance`.
fn fiddled(seed: i64, x: i32, y: i32, z: i32, dx: f64, dy: f64, dz: f64) -> f64 {
    let mut r = seed;
    for c in [x, y, z, x, y, z] {
        r = lcg(r, c as i64);
    }
    let fx = fiddle(r);
    r = lcg(r, seed);
    let fy = fiddle(r);
    r = lcg(r, seed);
    let fz = fiddle(r);
    (dz + fz) * (dz + fz) + (dy + fy) * (dy + fy) + (dx + fx) * (dx + fx)
}

/// `BiomeManager.getBiome(x, y, z)`'s choice of a quart cell (its noise biome is then read,
/// clamped in y by the chunk).
pub fn zoom(seed: i64, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    let (ax, ay, az) = (x - 2, y - 2, z - 2);
    let (px, py, pz) = (ax >> 2, ay >> 2, az >> 2);
    let (fx, fy, fz) = ((ax & 3) as f64 / 4.0, (ay & 3) as f64 / 4.0, (az & 3) as f64 / 4.0);
    let mut best = 0;
    let mut best_d = f64::INFINITY;
    for i in 0..8 {
        let (xe, ye, ze) = (i & 4 == 0, i & 2 == 0, i & 1 == 0);
        let (cx, cy, cz) = (if xe { px } else { px + 1 }, if ye { py } else { py + 1 }, if ze { pz } else { pz + 1 });
        let (dx, dy, dz) = (if xe { fx } else { fx - 1.0 }, if ye { fy } else { fy - 1.0 }, if ze { fz } else { fz - 1.0 });
        let d = fiddled(seed, cx, cy, cz, dx, dy, dz);
        if best_d > d {
            best = i;
            best_d = d;
        }
    }
    (if best & 4 == 0 { px } else { px + 1 }, if best & 2 == 0 { py } else { py + 1 }, if best & 1 == 0 { pz } else { pz + 1 })
}

/// `BlockAndLightGetter.canSeeSky(pos)`: sky light 15.
pub fn can_see_sky(w: &World, p: BlockPos) -> Result<bool, String> {
    Ok(w.sky_light(p)? >= 15)
}

/// `LevelReader.canSeeSkyFromBelowWater(pos)`: at or above the sea level, `canSeeSky`; below,
/// the sea-level cell of its column sees the sky and every block between them has no dampening
/// or is a liquid.
pub fn can_see_sky_from_below_water(w: &World, p: BlockPos) -> Result<bool, String> {
    const SEA_LEVEL: i32 = 63;
    if p.y >= SEA_LEVEL {
        return can_see_sky(w, p);
    }
    let scan = BlockPos::new(p.x, SEA_LEVEL, p.z);
    if !can_see_sky(w, scan)? {
        return Ok(false);
    }
    let mut q = scan.below();
    while q.y > p.y {
        let b = w.get(q);
        let lp = lprops(&b);
        let liquid = !crate::fluid::fluid_state_of(&b).is_empty() || matches!(b, Block::Lava { .. }) || matches!(b, Block::Terrain(id) if blocks::props(id).liquid);
        if lp.dampening > 0 && !liquid {
            return Ok(false);
        }
        q = q.below();
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Builder` in small steps builds the very world the one-step body built.
    #[test]
    fn the_builder_in_steps_equals_the_one_step_world() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/terrain/forest.mcterrain");
        let t = Terrain::decode(&std::fs::read(path).unwrap()).unwrap();
        let mut b = Builder::new(&t).unwrap();
        let mut steps = 0;
        while !b.step(97_000) {
            steps += 1;
        }
        assert!(steps > 10, "{steps} steps");
        let a = b.finish();
        let o = build_world_one_step(&t).unwrap();
        assert_eq!((a.min, a.size), (o.min, o.size));
        let (la, lo) = (a.land.as_ref().unwrap(), o.land.as_ref().unwrap());
        assert!(la.sky == lo.sky && la.block == lo.block && la.sources == lo.sources && la.heights == lo.heights);
        for y in 0..t.size[1] {
            for z in 0..t.size[2] {
                for x in 0..t.size[0] {
                    let p = BlockPos::new(t.origin[0] + x, t.origin[1] + y, t.origin[2] + z);
                    assert_eq!(a.get(p), o.get(p), "at {p}");
                }
            }
        }
    }
}
