//! A terrain sample as a redsim world (the terrain lane; brief §2-3 and "Placement"): the cells,
//! the game's light (the saved sky and block light, kept current around every change by the
//! engines' rules), the heightmaps, the biomes (`BiomeManager.getBiome`), the world spawn.
//!
//! The light starts from what the game saved, not from a fresh fixed point: the game's own light is
//! not always the fixed point (a tree a neighbour chunk's features grew after this chunk's sky
//! sources were filled leaves its leaves out of `ChunkSkyLightSources`: about 0.04% of a forest's
//! open cells read one or two more than the fixed point, which `mctas terrain probe` confirms at
//! run time). Around a change, the sky sources follow `ChunkSkyLightSources.update` step by step and
//! the light settles to the engines' fixed point inside the changed region, the unchanged cells
//! around it as its border.

use super::blocks;
use super::format::{Heightmap, Terrain};
use super::light::{self, LightProps};
use crate::block::Block;
use crate::pos::BlockPos;
use crate::world::World;
use std::collections::HashMap;

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
    /// Cells changed since the light was last settled.
    pub dirty: Vec<BlockPos>,
    /// Light-relevant changes: (game time, cell) of the last two ticks (the light engine's timing).
    pub light_changes: Vec<(i64, BlockPos)>,
    /// The structure starts (their boxes and pieces: the spawn overrides).
    pub structures: Vec<crate::terrain::format::Structure>,
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
    Ok(Block::Terrain(id))
}

/// The cells of a terrain box as redsim blocks, its palette mapped once.
fn cells_of(t: &Terrain) -> Result<Vec<Block>, String> {
    let palette: Vec<Block> = t.palette.iter().map(|s| block_of(s)).collect::<Result<_, _>>()?;
    Ok(t.cells.iter().map(|&c| palette[c as usize]).collect())
}

/// A world holding a terrain file's whole box, its light, heightmaps and biomes.
pub fn build_world(t: &Terrain) -> Result<World, String> {
    let min = BlockPos::new(t.origin[0], t.origin[1], t.origin[2]);
    let mut w = World::new(min, t.size);
    let cells = cells_of(t)?;
    w.load_cells(cells);
    let cols = (t.size[0] * t.size[2]) as usize;
    let (sky, block) = if !t.sky.is_empty() && !t.block.is_empty() {
        (t.sky.clone(), t.block.clone())
    } else {
        let g = light::grid_of_world(&w)?;
        light::compute(&g)
    };
    let mut sources = vec![0i32; cols];
    let [sx, sy, sz] = t.size;
    for z in 0..sz {
        for x in 0..sx {
            let mut y = sy - 1;
            while y >= 0 && sky[((y * sz + z) * sx + x) as usize] == 15 {
                y -= 1;
            }
            sources[(z * sx + x) as usize] = t.origin[1] + y + 1;
        }
    }
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
        light_changes: Vec::new(),
        structures: t.structures.clone(),
    }));
    Ok(w)
}

impl Land {
    fn col(w_min: BlockPos, size: [i32; 3], x: i32, z: i32) -> Option<usize> {
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
            // a modeled block missing from the table: anything with a collision box or a fluid
            None => crate::behavior::collides(b) || !crate::fluid::fluid_state_of(b).is_empty(),
        },
    }
}

/// The light behaviour of a block (a block missing from the table counts as opaque, and the change
/// is recorded as a model error by the caller).
fn lprops(b: &Block) -> LightProps {
    light::props_of(b).unwrap_or(LightProps { dampening: 15, emission: 0, faces: None })
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
        let relevant = light::props_of(&old) != light::props_of(&new);
        let now = self.time;
        if let Some(l) = self.land.as_mut() {
            for (m, h) in heights.into_iter().enumerate() {
                l.heights[m][c] = h;
            }
            l.sources[c] = source;
            if relevant || source != old_source {
                l.dirty.push(p);
                l.light_changes.retain(|&(t, _)| t >= now - 1);
                l.light_changes.push((now, p));
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
        let (min, size) = (self.min, self.size);
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for p in &land.dirty {
            let c = Land::col(min, size, p.x, p.z).unwrap_or(0);
            let s = land.sources[c];
            let (ylo, yhi) = (p.y.min(s) - 1, p.y.max(s) + 1);
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
        if touches_side {
            self.errors.push(format!("a light change within 15 of the terrain box's side or bottom ({lo:?}..{hi:?}): the light outside the box is not known"));
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
        let mut cache: HashMap<Block, LightProps> = HashMap::new();
        let mut props = Vec::with_capacity((n[0] * n[1] * n[2]) as usize);
        for y in elo[1]..=ehi[1] {
            for z in elo[2]..=ehi[2] {
                for x in elo[0]..=ehi[0] {
                    let b = self.get(BlockPos::new(x, y, z));
                    props.push(*cache.entry(b).or_insert_with(|| lprops(&b)));
                }
            }
        }
        let land = self.land.as_mut().unwrap();
        let gidx = |x: i32, y: i32, z: i32| (((y - min.y) * size[2] + (z - min.z)) * size[0] + (x - min.x)) as usize;
        let inside = |x: i32, y: i32, z: i32| x >= lo[0] && x <= hi[0] && y >= lo[1] && y <= hi[1] && z >= lo[2] && z <= hi[2];
        for sky in [true, false] {
            let mut level = vec![0u8; props.len()];
            let mut queue: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); 16];
            for y in elo[1]..=ehi[1] {
                for z in elo[2]..=ehi[2] {
                    for x in elo[0]..=ehi[0] {
                        let i = idx(x, y, z);
                        let v = if !inside(x, y, z) {
                            // the border keeps its light
                            if sky { land.sky[gidx(x, y, z)] } else { land.block[gidx(x, y, z)] }
                        } else if sky {
                            let c = Land::col(min, size, x, z).unwrap();
                            if y >= land.sources[c] { 15 } else { 0 }
                        } else {
                            props[i].emission.min(15)
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
                    for (d, [dx, dy, dz]) in light::DIRS_PUB {
                        let (qx, qy, qz) = (x + dx, y + dy, z + dz);
                        if !inside(qx, qy, qz) {
                            continue;
                        }
                        let j = idx(qx, qy, qz);
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
                    for x in lo[0]..=hi[0] {
                        let v = level[idx(x, y, z)];
                        if sky {
                            land.sky[gidx(x, y, z)] = v;
                        } else {
                            land.block[gidx(x, y, z)] = v;
                        }
                    }
                }
            }
        }
    }
}

impl World {
    /// The settled light of the cell behind a read (every read settles pending changes first,
    /// through `settle_light` at the tick's start; a read near a change of this tick or the last is
    /// a model error, the light engine's timing).
    fn light_index(&self, p: BlockPos) -> Option<usize> {
        self.index(p)
    }

    fn check_light_timing(&self, p: BlockPos) -> Result<(), String> {
        if let Some(l) = self.land.as_ref()
            && let Some((t, q)) = l.light_changes.iter().find(|(t, q)| *t >= self.time - 1 && (q.x - p.x).abs().max((q.y - p.y).abs()).max((q.z - p.z).abs()) <= 15)
        {
            return Err(format!("light read at {p} right after a light change at {q} (tick {t}): the light engine's timing is not modeled"));
        }
        Ok(())
    }

    /// `getBrightness(LightLayer.SKY, pos)`: the raw sky light (no darkening). A world without land
    /// reads daylight.rs's per-query fixed point (`Err` on unclassified blocks, as before); outside
    /// the box: the open sky above, darkness below.
    pub fn sky_light(&self, p: BlockPos) -> Result<i32, String> {
        match self.land.as_ref() {
            Some(l) => {
                self.check_light_timing(p)?;
                Ok(match self.light_index(p) {
                    Some(i) => l.sky[i] as i32,
                    None => {
                        if p.y >= self.min.y + self.size[1] {
                            15
                        } else {
                            0
                        }
                    }
                })
            }
            None => crate::daylight::sky_light(self, p),
        }
    }

    /// `getBrightness(LightLayer.BLOCK, pos)` (a world without land: no block light is modeled
    /// there; `Err` when a light source is in it).
    pub fn block_light(&self, p: BlockPos) -> Result<i32, String> {
        match self.land.as_ref() {
            Some(l) => {
                self.check_light_timing(p)?;
                Ok(self.light_index(p).map(|i| l.block[i] as i32).unwrap_or(0))
            }
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

    /// `BiomeManager.getBiome(pos)` (a world without land: `None`).
    pub fn biome_at(&self, p: BlockPos) -> Option<&str> {
        let l = self.land.as_ref()?;
        let (qx, qy, qz) = zoom(l.zoom_seed, p.x, p.y, p.z);
        let [ox, oy, oz] = l.biome_origin;
        let [sx, sy, sz] = l.biome_size;
        let qy = qy.clamp(l.world_min_y >> 2, ((l.world_min_y + l.world_height) >> 2) - 1);
        if qx < ox || qy < oy || qz < oz || qx >= ox + sx || qy >= oy + sy || qz >= oz + sz {
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
