//! The `.mcterrain` file (version 1): ground the official 26.3 server generated, read back from
//! its region files by `mctas terrain gen` (crates/terrain). Pure Rust, no dependencies, so the
//! site's viewer decodes it in wasm.
//!
//! Little-endian; integers are LEB128 varints (signed ones zigzag); a file is the magic, then tagged
//! sections (`[u8; 4]` tag, varint byte length, payload), unknown tags skipped:
//!
//! * `HEAD`: seed, the biome zoom seed (`BiomeManager.obfuscateSeed`), the world spawn, the world's
//!   min y and height, the game and day time at the save, origin and size of the box;
//! * `PALT` + `CELL`: block states (the game's strings) and the box's cells as palette ids, x
//!   fastest then z then y, in runs (the bridge `get` order);
//! * `BPAL` + `BIOM`: biome ids and the 4x4x4 cells (quart coordinates) covering every cell
//!   `BiomeManager.getBiome` reads for the box, in runs;
//! * `HMAP`: the chunks' saved heightmaps per column (`getHeight`: the first free y);
//! * `SKYL`, `BLKL`: the saved sky and block light per cell (0-15), in runs;
//! * `STRU`: structures (id, box) from the chunks' structure starts;
//! * `ENTS`: entities (id, position, SNBT) from the entity region files;
//! * `BENT`: block entities (cell, SNBT: beds, bells, chests...) from the chunks' `block_entities`;
//! * `INHB`: per chunk (x, z), its `InhabitedTime` (the regional difficulty's local part);
//! * `NOTE`: provenance (key, value) pairs: the command, version, centre, radius.

/// The file's first 8 bytes.
pub const MAGIC: &[u8; 8] = b"MCTERRv1";

/// The saved heightmaps, in the file's order (`Heightmap.Types`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Heightmap {
    WorldSurface,
    MotionBlocking,
    MotionBlockingNoLeaves,
    OceanFloor,
}

impl Heightmap {
    pub const ALL: [Heightmap; 4] = [Heightmap::WorldSurface, Heightmap::MotionBlocking, Heightmap::MotionBlockingNoLeaves, Heightmap::OceanFloor];

    /// The NBT key (`Heightmap.Types.getSerializationKey`).
    pub fn key(self) -> &'static str {
        match self {
            Heightmap::WorldSurface => "WORLD_SURFACE",
            Heightmap::MotionBlocking => "MOTION_BLOCKING",
            Heightmap::MotionBlockingNoLeaves => "MOTION_BLOCKING_NO_LEAVES",
            Heightmap::OceanFloor => "OCEAN_FLOOR",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// A saved entity: its type id, position and whole saved data (SNBT, as the game wrote it).
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    pub id: String,
    pub pos: [f64; 3],
    pub snbt: String,
}

/// A structure start: its id and bounding box (inclusive).
#[derive(Clone, Debug, PartialEq)]
pub struct Structure {
    pub id: String,
    pub min: [i32; 3],
    pub max: [i32; 3],
    /// Its pieces' boxes (inclusive): `structureHasPieceAt` (empty in files written before them).
    pub pieces: Vec<[i32; 6]>,
}

/// A terrain sample in memory.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Terrain {
    pub seed: i64,
    /// `BiomeManager.obfuscateSeed(seed)`: the first 8 bytes (little-endian) of SHA-256 of the
    /// seed's 8 little-endian bytes (Guava `hashLong(seed).asLong()`).
    pub zoom_seed: i64,
    /// The world spawn (`LevelData.RespawnData.pos`): no natural spawn within 24 of it.
    pub spawn: [i32; 3],
    pub world_min_y: i32,
    pub world_height: i32,
    pub game_time: i64,
    pub day_time: i64,
    /// The box: its lowest corner and its size.
    pub origin: [i32; 3],
    pub size: [i32; 3],
    pub palette: Vec<String>,
    /// Palette ids, x fastest, then z, then y.
    pub cells: Vec<u16>,
    pub biome_palette: Vec<String>,
    /// The biome cells' box in quart coordinates (block >> 2).
    pub biome_origin: [i32; 3],
    pub biome_size: [i32; 3],
    pub biomes: Vec<u16>,
    /// Per heightmap (`Heightmap::index`), per column (x fastest, then z): `getHeight`.
    pub heightmaps: Vec<Vec<i16>>,
    /// The saved light per cell (empty: none saved).
    pub sky: Vec<u8>,
    pub block: Vec<u8>,
    pub structures: Vec<Structure>,
    pub entities: Vec<Entity>,
    /// The block entities (beds, bells, chests...: their saved NBT, `BENT`).
    pub block_entities: Vec<BlockEntity>,
    /// Per chunk (x, z): its `InhabitedTime` (the regional difficulty's local part, `INHB`).
    pub inhabited: Vec<([i32; 2], i64)>,
    pub notes: Vec<(String, String)>,
}

/// A block entity: its cell and its saved NBT (SNBT).
#[derive(Clone, Debug, PartialEq)]
pub struct BlockEntity {
    pub pos: [i32; 3],
    pub snbt: String,
}

impl Terrain {
    /// Cells in the box.
    pub fn volume(&self) -> usize {
        (self.size[0] as usize) * (self.size[1] as usize) * (self.size[2] as usize)
    }

    pub fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        let [ox, oy, oz] = self.origin;
        let [sx, sy, sz] = self.size;
        x >= ox && y >= oy && z >= oz && x < ox + sx && y < oy + sy && z < oz + sz
    }

    /// The index of a cell of the box.
    pub fn index(&self, x: i32, y: i32, z: i32) -> usize {
        let [ox, oy, oz] = self.origin;
        let [sx, _, sz] = self.size;
        (((y - oy) * sz + (z - oz)) * sx + (x - ox)) as usize
    }

    /// The block state at a cell of the box (`None` outside).
    pub fn state(&self, x: i32, y: i32, z: i32) -> Option<&str> {
        if !self.contains(x, y, z) {
            return None;
        }
        Some(self.palette[self.cells[self.index(x, y, z)] as usize].as_str())
    }

    /// A column's saved heightmap (`None` outside the box's columns).
    pub fn height(&self, map: Heightmap, x: i32, z: i32) -> Option<i32> {
        let [ox, _, oz] = self.origin;
        let [sx, _, sz] = self.size;
        if x < ox || z < oz || x >= ox + sx || z >= oz + sz {
            return None;
        }
        self.heightmaps.get(map.index()).map(|h| h[((z - oz) * sx + (x - ox)) as usize] as i32)
    }

    /// The biome cell (quart coordinates) id, `None` outside the stored cells.
    pub fn biome_cell(&self, qx: i32, qy: i32, qz: i32) -> Option<&str> {
        let [ox, oy, oz] = self.biome_origin;
        let [sx, sy, sz] = self.biome_size;
        if qx < ox || qy < oy || qz < oz || qx >= ox + sx || qy >= oy + sy || qz >= oz + sz {
            return None;
        }
        let i = (((qy - oy) * sz + (qz - oz)) * sx + (qx - ox)) as usize;
        Some(self.biome_palette[self.biomes[i] as usize].as_str())
    }

    /// The saved sky light at a cell (`None` outside or not saved).
    pub fn saved_sky(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        if self.sky.is_empty() || !self.contains(x, y, z) {
            return None;
        }
        Some(self.sky[self.index(x, y, z)])
    }

    /// The saved block light at a cell (`None` outside or not saved).
    pub fn saved_block(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        if self.block.is_empty() || !self.contains(x, y, z) {
            return None;
        }
        Some(self.block[self.index(x, y, z)])
    }

    /// The quart box `BiomeManager.getBiome` reads for a block box: each block reads the cells
    /// floor((b - 2) / 4) and the next one on each axis.
    pub fn biome_box_for(origin: [i32; 3], size: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let mut o = [0; 3];
        let mut s = [0; 3];
        for a in 0..3 {
            let lo = (origin[a] - 2) >> 2;
            let hi = ((origin[a] + size[a] - 1 - 2) >> 2) + 1;
            o[a] = lo;
            s[a] = hi - lo + 1;
        }
        (o, s)
    }

    /// The sub-box `lo..=hi` (clamped to this box): cells, light, heightmaps, the biome cells it
    /// reads, the structures meeting it and the entities in it.
    pub fn crop(&self, lo: [i32; 3], hi: [i32; 3]) -> Result<Terrain, String> {
        let mut nlo = [0; 3];
        let mut nhi = [0; 3];
        for a in 0..3 {
            nlo[a] = lo[a].max(self.origin[a]);
            nhi[a] = hi[a].min(self.origin[a] + self.size[a] - 1);
            if nlo[a] > nhi[a] {
                return Err(format!("crop {lo:?}..{hi:?} misses the box {:?} + {:?}", self.origin, self.size));
            }
        }
        let size = [nhi[0] - nlo[0] + 1, nhi[1] - nlo[1] + 1, nhi[2] - nlo[2] + 1];
        let mut out = Terrain { origin: nlo, size, palette: Vec::new(), cells: Vec::with_capacity((size[0] * size[1] * size[2]) as usize), ..self.clone() };
        out.sky = Vec::new();
        out.block = Vec::new();
        let mut remap: Vec<Option<u16>> = vec![None; self.palette.len()];
        for y in nlo[1]..=nhi[1] {
            for z in nlo[2]..=nhi[2] {
                for x in nlo[0]..=nhi[0] {
                    let i = self.index(x, y, z);
                    let old = self.cells[i] as usize;
                    let id = match remap[old] {
                        Some(id) => id,
                        None => {
                            let id = out.palette.len() as u16;
                            out.palette.push(self.palette[old].clone());
                            remap[old] = Some(id);
                            id
                        }
                    };
                    out.cells.push(id);
                    if !self.sky.is_empty() {
                        out.sky.push(self.sky[i]);
                    }
                    if !self.block.is_empty() {
                        out.block.push(self.block[i]);
                    }
                }
            }
        }
        out.heightmaps = self
            .heightmaps
            .iter()
            .map(|h| {
                let mut v = Vec::with_capacity((size[0] * size[2]) as usize);
                for z in nlo[2]..=nhi[2] {
                    for x in nlo[0]..=nhi[0] {
                        v.push(h[((z - self.origin[2]) * self.size[0] + (x - self.origin[0])) as usize]);
                    }
                }
                v
            })
            .collect();
        let (bo, bs) = Terrain::biome_box_for(nlo, size);
        out.biome_origin = bo;
        out.biome_size = bs;
        out.biome_palette = Vec::new();
        out.biomes = Vec::with_capacity((bs[0] * bs[1] * bs[2]) as usize);
        let mut bremap: Vec<Option<u16>> = vec![None; self.biome_palette.len()];
        for qy in bo[1]..bo[1] + bs[1] {
            for qz in bo[2]..bo[2] + bs[2] {
                for qx in bo[0]..bo[0] + bs[0] {
                    let [ox, oy, oz] = self.biome_origin;
                    let [sx, sy, sz] = self.biome_size;
                    if qx < ox || qy < oy || qz < oz || qx >= ox + sx || qy >= oy + sy || qz >= oz + sz {
                        return Err(format!("crop needs biome cell {qx},{qy},{qz} outside the stored cells"));
                    }
                    let old = self.biomes[(((qy - oy) * sz + (qz - oz)) * sx + (qx - ox)) as usize] as usize;
                    let id = match bremap[old] {
                        Some(id) => id,
                        None => {
                            let id = out.biome_palette.len() as u16;
                            out.biome_palette.push(self.biome_palette[old].clone());
                            bremap[old] = Some(id);
                            id
                        }
                    };
                    out.biomes.push(id);
                }
            }
        }
        out.structures = self.structures.iter().filter(|s| (0..3).all(|a| s.min[a] <= nhi[a] && s.max[a] >= nlo[a])).cloned().collect();
        out.entities = self
            .entities
            .iter()
            .filter(|e| (0..3).all(|a| e.pos[a] >= nlo[a] as f64 && e.pos[a] < (nhi[a] + 1) as f64))
            .cloned()
            .collect();
        out.block_entities = self.block_entities.iter().filter(|b| (0..3).all(|a| b.pos[a] >= nlo[a] && b.pos[a] <= nhi[a])).cloned().collect();
        out.inhabited = self
            .inhabited
            .iter()
            .filter(|(c, _)| c[0] * 16 + 15 >= nlo[0] && c[0] * 16 <= nhi[0] && c[1] * 16 + 15 >= nlo[2] && c[1] * 16 <= nhi[2])
            .copied()
            .collect();
        Ok(out)
    }

    /// The file bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        let mut head = Writer::default();
        head.i64(self.seed);
        head.i64(self.zoom_seed);
        for v in self.spawn {
            head.i64(v as i64);
        }
        head.i64(self.world_min_y as i64);
        head.i64(self.world_height as i64);
        head.i64(self.game_time);
        head.i64(self.day_time);
        for v in self.origin.iter().chain(self.size.iter()) {
            head.i64(*v as i64);
        }
        section(&mut out, b"HEAD", head);
        section(&mut out, b"PALT", strings(&self.palette));
        section(&mut out, b"CELL", runs16(&self.cells));
        section(&mut out, b"BPAL", strings(&self.biome_palette));
        let mut b = Writer::default();
        for v in self.biome_origin.iter().chain(self.biome_size.iter()) {
            b.i64(*v as i64);
        }
        b.bytes(&runs16(&self.biomes).buf);
        section(&mut out, b"BIOM", b);
        let mut h = Writer::default();
        h.u64(self.heightmaps.len() as u64);
        for m in &self.heightmaps {
            let as_u16: Vec<u16> = m.iter().map(|v| *v as u16).collect();
            h.bytes(&runs16(&as_u16).buf);
        }
        section(&mut out, b"HMAP", h);
        if !self.sky.is_empty() {
            section(&mut out, b"SKYL", runs8(&self.sky));
        }
        if !self.block.is_empty() {
            section(&mut out, b"BLKL", runs8(&self.block));
        }
        let mut s = Writer::default();
        s.u64(self.structures.len() as u64);
        for st in &self.structures {
            s.str(&st.id);
            for v in st.min.iter().chain(st.max.iter()) {
                s.i64(*v as i64);
            }
            s.u64(st.pieces.len() as u64);
            for p in &st.pieces {
                for v in p {
                    s.i64(*v as i64);
                }
            }
        }
        section(&mut out, b"STR2", s);
        let mut e = Writer::default();
        e.u64(self.entities.len() as u64);
        for en in &self.entities {
            e.str(&en.id);
            for v in en.pos {
                e.buf.extend_from_slice(&v.to_le_bytes());
            }
            e.str(&en.snbt);
        }
        section(&mut out, b"ENTS", e);
        if !self.block_entities.is_empty() {
            let mut b = Writer::default();
            b.u64(self.block_entities.len() as u64);
            for be in &self.block_entities {
                for v in be.pos {
                    b.i64(v as i64);
                }
                b.str(&be.snbt);
            }
            section(&mut out, b"BENT", b);
        }
        if !self.inhabited.is_empty() {
            let mut h = Writer::default();
            h.u64(self.inhabited.len() as u64);
            for (c, t) in &self.inhabited {
                h.i64(c[0] as i64);
                h.i64(c[1] as i64);
                h.i64(*t);
            }
            section(&mut out, b"INHB", h);
        }
        let mut n = Writer::default();
        n.u64(self.notes.len() as u64);
        for (k, v) in &self.notes {
            n.str(k);
            n.str(v);
        }
        section(&mut out, b"NOTE", n);
        out
    }

    /// Parse a file.
    pub fn decode(bytes: &[u8]) -> Result<Terrain, String> {
        if bytes.len() < 8 || &bytes[..8] != MAGIC {
            return Err("not an .mcterrain v1 file (bad magic)".into());
        }
        let mut t = Terrain::default();
        let mut r = Reader { b: bytes, i: 8 };
        let mut seen_cells = false;
        while r.i < bytes.len() {
            let tag: [u8; 4] = r.take(4)?.try_into().map_err(|_| "truncated tag")?;
            let len = r.u64()? as usize;
            let body = r.take(len)?;
            let mut s = Reader { b: body, i: 0 };
            match &tag {
                b"HEAD" => {
                    t.seed = s.i64()?;
                    t.zoom_seed = s.i64()?;
                    for a in 0..3 {
                        t.spawn[a] = s.i64()? as i32;
                    }
                    t.world_min_y = s.i64()? as i32;
                    t.world_height = s.i64()? as i32;
                    t.game_time = s.i64()?;
                    t.day_time = s.i64()?;
                    for a in 0..3 {
                        t.origin[a] = s.i64()? as i32;
                    }
                    for a in 0..3 {
                        t.size[a] = s.i64()? as i32;
                    }
                }
                b"PALT" => t.palette = s.strings()?,
                b"CELL" => {
                    t.cells = s.runs16(t.volume())?;
                    seen_cells = true;
                }
                b"BPAL" => t.biome_palette = s.strings()?,
                b"BIOM" => {
                    for a in 0..3 {
                        t.biome_origin[a] = s.i64()? as i32;
                    }
                    for a in 0..3 {
                        t.biome_size[a] = s.i64()? as i32;
                    }
                    let n = (t.biome_size[0] * t.biome_size[1] * t.biome_size[2]) as usize;
                    t.biomes = s.runs16(n)?;
                }
                b"HMAP" => {
                    let maps = s.u64()? as usize;
                    let cols = (t.size[0] * t.size[2]) as usize;
                    for _ in 0..maps {
                        t.heightmaps.push(s.runs16(cols)?.into_iter().map(|v| v as i16).collect());
                    }
                }
                b"SKYL" => t.sky = s.runs8(t.volume())?,
                b"BLKL" => t.block = s.runs8(t.volume())?,
                b"STRU" | b"STR2" => {
                    let with_pieces = &tag == b"STR2";
                    let n = s.u64()? as usize;
                    for _ in 0..n {
                        let id = s.str()?;
                        let mut v = [0i32; 6];
                        for x in v.iter_mut() {
                            *x = s.i64()? as i32;
                        }
                        let mut pieces = Vec::new();
                        if with_pieces {
                            let np = s.u64()? as usize;
                            for _ in 0..np {
                                let mut p = [0i32; 6];
                                for x in p.iter_mut() {
                                    *x = s.i64()? as i32;
                                }
                                pieces.push(p);
                            }
                        }
                        t.structures.push(Structure { id, min: [v[0], v[1], v[2]], max: [v[3], v[4], v[5]], pieces });
                    }
                }
                b"ENTS" => {
                    let n = s.u64()? as usize;
                    for _ in 0..n {
                        let id = s.str()?;
                        let mut pos = [0f64; 3];
                        for p in pos.iter_mut() {
                            *p = f64::from_le_bytes(s.take(8)?.try_into().map_err(|_| "truncated f64")?);
                        }
                        let snbt = s.str()?;
                        t.entities.push(Entity { id, pos, snbt });
                    }
                }
                b"BENT" => {
                    let n = s.u64()? as usize;
                    for _ in 0..n {
                        let pos = [s.i64()? as i32, s.i64()? as i32, s.i64()? as i32];
                        let snbt = s.str()?;
                        t.block_entities.push(BlockEntity { pos, snbt });
                    }
                }
                b"INHB" => {
                    let n = s.u64()? as usize;
                    for _ in 0..n {
                        let c = [s.i64()? as i32, s.i64()? as i32];
                        let v = s.i64()?;
                        t.inhabited.push((c, v));
                    }
                }
                b"NOTE" => {
                    let n = s.u64()? as usize;
                    for _ in 0..n {
                        let k = s.str()?;
                        let v = s.str()?;
                        t.notes.push((k, v));
                    }
                }
                _ => {}
            }
        }
        if !seen_cells {
            return Err("an .mcterrain file without cells".into());
        }
        if t.cells.iter().any(|&c| c as usize >= t.palette.len()) {
            return Err("a cell outside the palette".into());
        }
        if t.biomes.iter().any(|&c| c as usize >= t.biome_palette.len()) {
            return Err("a biome cell outside the biome palette".into());
        }
        Ok(t)
    }

    /// A provenance note's value.
    pub fn note(&self, key: &str) -> Option<&str> {
        self.notes.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

#[derive(Default)]
struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn u64(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(byte);
                return;
            }
            self.buf.push(byte | 0x80);
        }
    }
    fn i64(&mut self, v: i64) {
        self.u64(((v << 1) ^ (v >> 63)) as u64);
    }
    fn str(&mut self, s: &str) {
        self.u64(s.len() as u64);
        self.buf.extend_from_slice(s.as_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
}

fn section(out: &mut Vec<u8>, tag: &[u8; 4], w: Writer) {
    out.extend_from_slice(tag);
    let mut len = Writer::default();
    len.u64(w.buf.len() as u64);
    out.extend_from_slice(&len.buf);
    out.extend_from_slice(&w.buf);
}

fn strings(v: &[String]) -> Writer {
    let mut w = Writer::default();
    w.u64(v.len() as u64);
    for s in v {
        w.str(s);
    }
    w
}

fn runs16(v: &[u16]) -> Writer {
    let mut w = Writer::default();
    let mut i = 0;
    while i < v.len() {
        let mut j = i + 1;
        while j < v.len() && v[j] == v[i] {
            j += 1;
        }
        w.u64((j - i) as u64);
        w.u64(v[i] as u64);
        i = j;
    }
    w
}

fn runs8(v: &[u8]) -> Writer {
    let as16: Vec<u16> = v.iter().map(|&b| b as u16).collect();
    runs16(&as16)
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.i + n > self.b.len() {
            return Err(format!("truncated at byte {}", self.i));
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn u64(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let byte = *self.take(1)?.first().unwrap();
            if shift >= 64 {
                return Err("varint too long".into());
            }
            v |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
        }
    }
    fn i64(&mut self) -> Result<i64, String> {
        let u = self.u64()?;
        Ok(((u >> 1) as i64) ^ -((u & 1) as i64))
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.u64()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| "a string that is not UTF-8".to_string())
    }
    fn strings(&mut self) -> Result<Vec<String>, String> {
        let n = self.u64()? as usize;
        (0..n).map(|_| self.str()).collect()
    }
    fn runs16(&mut self, n: usize) -> Result<Vec<u16>, String> {
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let run = self.u64()? as usize;
            let v = self.u64()?;
            if run == 0 || out.len() + run > n || v > u16::MAX as u64 {
                return Err(format!("bad run ({run} x {v}) at {} of {n}", out.len()));
            }
            out.extend(std::iter::repeat_n(v as u16, run));
        }
        Ok(out)
    }
    fn runs8(&mut self, n: usize) -> Result<Vec<u8>, String> {
        Ok(self.runs16(n)?.into_iter().map(|v| v as u8).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Terrain {
        let size = [5, 4, 6];
        let n = (size[0] * size[1] * size[2]) as usize;
        let (bo, bs) = Terrain::biome_box_for([-3, 60, 7], size);
        Terrain {
            seed: -42,
            zoom_seed: 0x1234_5678_9abc_def0,
            spawn: [0, 64, -8],
            world_min_y: -64,
            world_height: 384,
            game_time: 1200,
            day_time: 1200,
            origin: [-3, 60, 7],
            size,
            palette: vec!["minecraft:air".into(), "minecraft:stone".into(), "minecraft:grass_block[snowy=false]".into()],
            cells: (0..n).map(|i| (i % 7 % 3) as u16).collect(),
            biome_palette: vec!["minecraft:plains".into(), "minecraft:forest".into()],
            biome_origin: bo,
            biome_size: bs,
            biomes: (0..(bs[0] * bs[1] * bs[2]) as usize).map(|i| (i % 2) as u16).collect(),
            heightmaps: vec![vec![63; 30], vec![64; 30], vec![64; 30], vec![62; 30]],
            sky: (0..n).map(|i| (i % 16) as u8).collect(),
            block: vec![0; n],
            structures: vec![Structure { id: "minecraft:village_plains".into(), min: [-10, 60, 0], max: [40, 80, 50], pieces: vec![[-10, 60, 0, 5, 70, 9]] }],
            entities: vec![Entity { id: "minecraft:villager".into(), pos: [-1.5, 61.0, 9.25], snbt: "{Health:20.0f}".into() }],
            block_entities: vec![BlockEntity { pos: [-1, 61, 9], snbt: "{id:\"minecraft:bed\"}".into() }],
            inhabited: vec![([-1, 0], 1234)],
            notes: vec![("command".into(), "mctas terrain gen".into())],
        }
    }

    #[test]
    fn round_trip() {
        let t = sample();
        let back = Terrain::decode(&t.encode()).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn crop_keeps_cells_and_reads() {
        let t = sample();
        let c = t.crop([-2, 61, 8], [0, 62, 10]).unwrap();
        assert_eq!(c.size, [3, 2, 3]);
        for y in 61..=62 {
            for z in 8..=10 {
                for x in -2..=0 {
                    assert_eq!(c.state(x, y, z), t.state(x, y, z));
                    assert_eq!(c.saved_sky(x, y, z), t.saved_sky(x, y, z));
                }
            }
        }
        assert_eq!(c.entities.len(), 1);
        let back = Terrain::decode(&c.encode()).unwrap();
        assert_eq!(c, back);
    }
}
