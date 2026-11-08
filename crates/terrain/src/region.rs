//! Reading a saved dimension back: every section's block states, biomes and saved light, the
//! chunk's heightmaps, structure starts and status, and the entity chunks (26.3's
//! `SerializableChunkData` and `EntityStorage` layouts).

use anyhow::{Context, Result, bail};
use mctas::vanilla::nbt::{self, Tag};
use mctas::vanilla::read_region_root;
use redsim::terrain::format::{Entity, Heightmap, Structure};
use std::collections::HashMap;
use std::path::Path;

/// One section (16 x 16 x 16) as saved.
pub struct Section {
    pub palette: Vec<String>,
    pub data: Vec<i64>,
    pub biome_palette: Vec<String>,
    pub biome_data: Vec<i64>,
    pub block_light: Option<Vec<u8>>,
    pub sky_light: Option<Vec<u8>>,
}

impl Section {
    /// The block state at section-relative coordinates.
    pub fn state(&self, x: i32, y: i32, z: i32) -> &str {
        if self.palette.len() <= 1 || self.data.is_empty() {
            return self.palette.first().map(|s| s.as_str()).unwrap_or("minecraft:air");
        }
        let bits = (usize::BITS - (self.palette.len() - 1).leading_zeros()).max(4) as usize;
        let idx = ((y * 16 + z) * 16 + x) as usize;
        let v = packed(&self.data, bits, idx);
        self.palette.get(v).map(|s| s.as_str()).unwrap_or("?")
    }

    /// The biome of a 4x4x4 cell at section-relative quart coordinates.
    pub fn biome(&self, qx: i32, qy: i32, qz: i32) -> &str {
        if self.biome_palette.len() <= 1 || self.biome_data.is_empty() {
            return self.biome_palette.first().map(|s| s.as_str()).unwrap_or("?");
        }
        // PalettedContainer.Strategy.SECTION_BIOMES: ceil(log2(n)) bits, no minimum
        let bits = (usize::BITS - (self.biome_palette.len() - 1).leading_zeros()) as usize;
        let idx = ((qy * 4 + qz) * 4 + qx) as usize;
        let v = packed(&self.biome_data, bits, idx);
        self.biome_palette.get(v).map(|s| s.as_str()).unwrap_or("?")
    }
}

/// A value of a packed long array (no value spans two longs, as since 1.16).
fn packed(data: &[i64], bits: usize, idx: usize) -> usize {
    let per_long = 64 / bits;
    let word = data.get(idx / per_long).copied().unwrap_or(0) as u64;
    ((word >> ((idx % per_long) * bits)) & ((1u64 << bits) - 1)) as usize
}

/// A nibble of a saved light layer (`DataLayer.get`).
pub fn nibble(layer: &[u8], x: i32, y: i32, z: i32) -> u8 {
    let idx = ((y * 16 + z) * 16 + x) as usize;
    (layer[idx >> 1] >> ((idx & 1) * 4)) & 0xf
}

/// One saved chunk.
pub struct Chunk {
    pub status: String,
    pub light_on: bool,
    pub sections: HashMap<i32, Section>,
    /// Per heightmap (`Heightmap::index`), 256 values (x + 16 z) of `getHeight`.
    pub heightmaps: Vec<Option<Vec<i32>>>,
    pub structures: Vec<Structure>,
}

impl Chunk {
    pub fn state(&self, x: i32, y: i32, z: i32) -> &str {
        match self.sections.get(&y.div_euclid(16)) {
            Some(s) => s.state(x.rem_euclid(16), y.rem_euclid(16), z.rem_euclid(16)),
            None => "minecraft:air",
        }
    }

    /// The saved block light (an absent layer is 0: `BlockLightSectionStorage`).
    pub fn block_light(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.sections.get(&y.div_euclid(16)).and_then(|s| s.block_light.as_ref()) {
            Some(l) => nibble(l, x.rem_euclid(16), y.rem_euclid(16), z.rem_euclid(16)),
            None => 0,
        }
    }

    /// The saved sky light. A section without a layer reads 15 when no section above it has one
    /// (open sky above the chunk's top layer: `SkyLightSectionStorage.getLightValue`'s
    /// `sectionY >= topSection`), else 0: the game reads 0 there after loading the chunk (checked
    /// with `mctas terrain probe`: `getMaxLocalRawBrightness` 0 in such caves at noon; a layer that
    /// is all 0 is never written, `DataLayer.isEmpty`). Returns (value, read from an absent layer).
    pub fn sky_light(&self, x: i32, y: i32, z: i32, top_section: i32) -> (u8, bool) {
        let (lx, lz) = (x.rem_euclid(16), z.rem_euclid(16));
        let sy = y.div_euclid(16);
        if let Some(l) = self.sections.get(&sy).and_then(|s| s.sky_light.as_ref()) {
            return (nibble(l, lx, y.rem_euclid(16), lz), false);
        }
        let layer_above = (sy + 1..top_section).any(|s| self.sections.get(&s).is_some_and(|s| s.sky_light.is_some()));
        (if layer_above { 0 } else { 15 }, true)
    }
}

fn int_array(t: Option<&Tag>) -> Vec<i32> {
    match t {
        Some(Tag::IntArray(a)) => a.clone(),
        Some(Tag::List(l)) => l.iter().filter_map(|v| v.as_i64()).map(|v| v as i32).collect(),
        _ => Vec::new(),
    }
}

fn byte_array(t: Option<&Tag>) -> Option<Vec<u8>> {
    match t {
        Some(Tag::ByteArray(a)) if a.len() == 2048 => Some(a.iter().map(|b| *b as u8).collect()),
        _ => None,
    }
}

/// A structure start's bounding box: the union of its pieces' `BB`.
fn structure_of(id: &str, start: &Tag) -> Option<Structure> {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for piece in start.get("Children").map(|c| c.as_list()).unwrap_or(&[]) {
        let bb = int_array(piece.get("BB"));
        if bb.len() == 6 {
            for a in 0..3 {
                lo[a] = lo[a].min(bb[a]);
                hi[a] = hi[a].max(bb[a + 3]);
            }
        }
    }
    (lo[0] <= hi[0]).then(|| Structure { id: id.to_string(), min: lo, max: hi })
}

/// Decode one chunk of a dimension folder (`None`: never saved).
pub fn read_chunk(dim: &Path, cx: i32, cz: i32, cache: &mut HashMap<(i32, i32), Vec<u8>>, world_height: i32) -> Result<Option<Chunk>> {
    let Some(root) = read_region_root(&dim.join("region"), cx, cz, cache)? else { return Ok(None) };
    let status = root.get("Status").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let light_on = root.get("isLightOn").and_then(|t| t.as_i64()).unwrap_or(0) != 0;
    let mut sections = HashMap::new();
    for sec in root.get("sections").map(|t| t.as_list()).unwrap_or(&[]) {
        let y = sec.get("Y").and_then(|t| t.as_i64()).unwrap_or(0) as i32;
        let (palette, data) = match sec.get("block_states") {
            Some(bs) => (
                bs.get("palette").map(|t| t.as_list()).unwrap_or(&[]).iter().map(nbt::block_state_string).collect(),
                bs.get("data").map(|t| t.as_long_array().to_vec()).unwrap_or_default(),
            ),
            None => (vec!["minecraft:air".to_string()], Vec::new()),
        };
        let (biome_palette, biome_data) = match sec.get("biomes") {
            Some(b) => (
                b.get("palette").map(|t| t.as_list()).unwrap_or(&[]).iter().map(|t| t.as_str().unwrap_or("?").to_string()).collect(),
                b.get("data").map(|t| t.as_long_array().to_vec()).unwrap_or_default(),
            ),
            None => (Vec::new(), Vec::new()),
        };
        sections.insert(y, Section { palette, data, biome_palette, biome_data, block_light: byte_array(sec.get("BlockLight")), sky_light: byte_array(sec.get("SkyLight")) });
    }
    let bits = 32 - (world_height as u32).leading_zeros();
    let mut heightmaps = Vec::new();
    for map in Heightmap::ALL {
        let tag = root.get("Heightmaps").and_then(|h| h.get(map.key()));
        heightmaps.push(tag.map(|t| {
            let data = t.as_long_array();
            (0..256).map(|i| packed(data, bits as usize, i) as i32).collect()
        }));
    }
    let mut structures = Vec::new();
    if let Some(Tag::Compound(starts)) = root.get("structures").and_then(|s| s.get("starts")) {
        for (id, start) in starts {
            if start.get("id").and_then(|t| t.as_str()) == Some("INVALID") {
                continue;
            }
            if let Some(s) = structure_of(id, start) {
                structures.push(s);
            }
        }
    }
    Ok(Some(Chunk { status, light_on, sections, heightmaps, structures }))
}

/// Every entity saved in a chunk of the dimension's `entities/` folder.
pub fn read_entities(dim: &Path, cx: i32, cz: i32, cache: &mut HashMap<(i32, i32), Vec<u8>>) -> Result<Vec<Entity>> {
    let Some(root) = read_region_root(&dim.join("entities"), cx, cz, cache)? else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for e in root.get("Entities").map(|t| t.as_list()).unwrap_or(&[]) {
        let id = e.get("id").and_then(|t| t.as_str()).context("an entity without an id")?.to_string();
        let pos: Vec<f64> = e.get("Pos").map(|p| p.as_list().iter().filter_map(|v| v.as_f64()).collect()).unwrap_or_default();
        if pos.len() != 3 {
            bail!("entity {id} without a position");
        }
        out.push(Entity { id, pos: [pos[0], pos[1], pos[2]], snbt: nbt::to_snbt(e) });
    }
    Ok(out)
}

/// The world spawn from `level.dat` (26.x: `Data.spawn.pos` [x, y, z]; older: `SpawnX/Y/Z`).
pub fn read_spawn(world: &Path) -> Result<[i32; 3]> {
    let data = level_data(world)?;
    if let Some(spawn) = data.get("spawn") {
        let pos = int_array(spawn.get("pos"));
        if pos.len() == 3 {
            return Ok([pos[0], pos[1], pos[2]]);
        }
    }
    let get = |k: &str| data.get(k).and_then(|t| t.as_i64()).map(|v| v as i32);
    match (get("SpawnX"), get("SpawnY"), get("SpawnZ")) {
        (Some(x), Some(y), Some(z)) => Ok([x, y, z]),
        _ => bail!("level.dat holds no world spawn I can read: {}", nbt::to_snbt(&data).chars().take(400).collect::<String>()),
    }
}

/// `level.dat`'s `Data` compound.
fn level_data(world: &Path) -> Result<Tag> {
    use std::io::Read;
    let bytes = std::fs::read(world.join("level.dat")).context("reading level.dat")?;
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(&bytes[..]).read_to_end(&mut raw)?;
    let root = nbt::parse_named(&raw)?;
    root.get("Data").cloned().context("level.dat without Data")
}

/// The saved game time and day time (`Time`; the day time: `DayTime`, or the default clock's
/// time in 26.x's world clocks when present).
pub fn read_times(world: &Path) -> Result<(i64, i64)> {
    let data = level_data(world)?;
    let game = data.get("Time").and_then(|t| t.as_i64()).unwrap_or(0);
    let day = data.get("DayTime").and_then(|t| t.as_i64()).unwrap_or(game);
    Ok((game, day))
}
