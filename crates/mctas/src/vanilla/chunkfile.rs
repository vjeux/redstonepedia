//! Reading saved chunk data back from region files: the terrain (`region/`), entity
//! (`entities/`) and POI (`poi/`) streams, one Anvil file (`r.<rx>.<rz>.mca`) per 32x32
//! chunks per stream, plus external oversized-chunk files (`c.<x>.<z>.mcc`).
//!
//! This mirrors `RegionFile`/`RegionFileVersion` (26.3): an 8 KiB header of big-endian
//! (sector offset | sector count, timestamp) per chunk, a 5-byte chunk header
//! (length including the version byte, version byte), versions 1=gzip, 2=deflate,
//! 3=none, 4=lz4 (reported, not decoded), and version|128 meaning "the bytes are in the
//! external `.mcc` file" (the external threshold is 256 sectors ≈ 1 MiB of stream).
//! Unlike the game's loader this never throws a chunk away silently: every outcome —
//! absent, external missing, truncated, undecodable — is a named class so studies can
//! score what a saved world actually contains.

use super::nbt::{self, Tag};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The three per-chunk persistence streams, by the folder that holds their region files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stream {
    Region,
    Entities,
    Poi,
}

impl Stream {
    pub fn folder(self) -> &'static str {
        match self {
            Stream::Region => "region",
            Stream::Entities => "entities",
            Stream::Poi => "poi",
        }
    }
    pub fn all() -> [Stream; 3] {
        [Stream::Region, Stream::Entities, Stream::Poi]
    }
    pub fn parse(name: &str) -> Result<Stream> {
        Ok(match name {
            "region" => Stream::Region,
            "entities" => Stream::Entities,
            "poi" => Stream::Poi,
            other => bail!("stream must be region, entities or poi, got `{other}`"),
        })
    }
}

/// How one chunk's stream is stored (or fails to be). `regen` in the chunk-regen study is
/// exactly: next load sees Absent / ExternalMissing / Truncated / BadVersion /
/// DecodeFailed / NbtFailed for the terrain stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Absent,
    Internal,
    External,
    ExternalMissing,
    Truncated,
    BadVersion,
    DecodeFailed,
    NbtFailed,
}

impl Presence {
    pub fn name(self) -> &'static str {
        match self {
            Presence::Absent => "absent",
            Presence::Internal => "internal",
            Presence::External => "external",
            Presence::ExternalMissing => "external-missing",
            Presence::Truncated => "truncated",
            Presence::BadVersion => "bad-version",
            Presence::DecodeFailed => "decode-failed",
            Presence::NbtFailed => "nbt-failed",
        }
    }
    /// True when the bytes decoded into an NBT compound.
    pub fn ok(self) -> bool {
        matches!(self, Presence::Internal | Presence::External)
    }
}

/// Counts pulled from a chunk tag (any stream; absent fields count as 0).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagSummary {
    pub status: Option<String>,
    pub data_version: Option<i64>,
    pub x_pos: Option<i64>,
    pub z_pos: Option<i64>,
    pub sections: usize,
    pub block_entities: usize,
    pub block_ticks: usize,
    pub fluid_ticks: usize,
    /// Terrain `entities` (proto chunks) + entity stream `Entities`.
    pub entities: usize,
    /// POI stream: total `Records` over all sections.
    pub poi_records: usize,
    pub inhabited_time: Option<i64>,
    pub last_update: Option<i64>,
}

/// One chunk in one stream.
#[derive(Clone, Debug)]
pub struct ChunkEntry {
    pub stream: Stream,
    pub cx: i32,
    pub cz: i32,
    pub presence: Presence,
    /// The stored version byte without the external flag (1..4), when a header existed.
    pub version: Option<u8>,
    pub sectors: u32,
    pub timestamp: u32,
    /// Stored stream bytes (internal: declared length-1; external: `.mcc` size).
    pub stored_len: usize,
    pub decompressed_len: Option<usize>,
    /// Hex sha256 of the decompressed bytes, when they decoded.
    pub sha256: Option<String>,
    /// Hex sha256 of the parsed tag *content* (root `LastUpdate`/`InhabitedTime`
    /// skipped): equal means the same blocks/entities/ticks even if the chunk was
    /// re-saved later. This is the neighbour-intact proof; `sha256` alone is not.
    pub content_sha256: Option<String>,
    pub summary: Option<TagSummary>,
    pub note: Option<String>,
}

/// The overworld dimension folder of a saved level. Accepts a server run directory
/// (holds `world/`), a level folder (holds `dimensions/…` or `region/`), or a dimension
/// folder itself (holds `region/`).
pub fn dimension_dir(input: &Path) -> PathBuf {
    let modern = |base: &Path| base.join("dimensions").join("minecraft").join("overworld");
    if modern(input).join("region").exists() {
        return modern(input);
    }
    if modern(&input.join("world")).join("region").exists() {
        return modern(&input.join("world"));
    }
    if input.join("region").exists() {
        return input.to_path_buf();
    }
    if input.join("world").join("region").exists() {
        return input.join("world");
    }
    // Nothing saved yet: prefer the modern layout so callers still get stable paths.
    if modern(input).exists() {
        return modern(input);
    }
    input.to_path_buf()
}

/// Decode one stored stream (the bytes after the 5-byte chunk header, or the `.mcc` file
/// content) with a RegionFileVersion id.
pub fn decompress_stream(version: u8, bytes: &[u8]) -> Result<Vec<u8>> {
    match version {
        3 => Ok(bytes.to_vec()),
        2 => {
            let mut v = Vec::new();
            flate2::read::ZlibDecoder::new(bytes).read_to_end(&mut v).context("deflate chunk stream")?;
            Ok(v)
        }
        1 => {
            let mut v = Vec::new();
            flate2::read::GzDecoder::new(bytes).read_to_end(&mut v).context("gzip chunk stream")?;
            Ok(v)
        }
        4 => bail!("lz4 chunk stream (version 4) is reported but not decoded"),
        other => bail!("unknown chunk stream version {other}"),
    }
}

fn region_file_path(folder: &Path, rx: i32, rz: i32) -> PathBuf {
    folder.join(format!("r.{rx}.{rz}.mca"))
}

fn load_region_file<'a>(folder: &Path, rx: i32, rz: i32, cache: &'a mut BTreeMap<(i32, i32), Vec<u8>>) -> &'a [u8] {
    cache.entry((rx, rz)).or_insert_with(|| std::fs::read(region_file_path(folder, rx, rz)).unwrap_or_default()).as_slice()
}

/// Inspect one chunk of one stream. `dim` is the dimension folder (see [`dimension_dir`]).
pub fn inspect_chunk(dim: &Path, stream: Stream, cx: i32, cz: i32) -> ChunkEntry {
    let mut cache = BTreeMap::new();
    inspect_chunk_cached(dim, stream, cx, cz, &mut cache)
}

fn inspect_chunk_cached(dim: &Path, stream: Stream, cx: i32, cz: i32, cache: &mut BTreeMap<(i32, i32), Vec<u8>>) -> ChunkEntry {
    let folder = dim.join(stream.folder());
    let mut e = ChunkEntry {
        stream,
        cx,
        cz,
        presence: Presence::Absent,
        version: None,
        sectors: 0,
        timestamp: 0,
        stored_len: 0,
        decompressed_len: None,
        sha256: None,
        content_sha256: None,
        summary: None,
        note: None,
    };
    let (rx, rz) = (cx.div_euclid(32), cz.div_euclid(32));
    let file_len;
    let (loc, ts);
    {
        let file = load_region_file(&folder, rx, rz, cache);
        file_len = file.len();
        if file_len < 8192 {
            if file_len > 0 {
                e.note = Some(format!("region file is {file_len} bytes (truncated header)"));
                e.presence = Presence::Truncated;
            }
            return e;
        }
        let slot = 4 * ((cx.rem_euclid(32) + cz.rem_euclid(32) * 32) as usize);
        loc = u32::from_be_bytes(file[slot..slot + 4].try_into().unwrap());
        ts = u32::from_be_bytes(file[4096 + slot..4096 + slot + 4].try_into().unwrap());
    }
    e.timestamp = ts;
    let (sector, count) = ((loc >> 8) as usize, (loc & 0xff) as u32);
    e.sectors = count;
    if loc == 0 || count == 0 {
        return e;
    }
    let offset = sector * 4096;
    let (len, version_byte, internal_raw) = {
        let file = load_region_file(&folder, rx, rz, cache);
        let Some(header) = file.get(offset..offset + 5) else {
            e.presence = Presence::Truncated;
            e.note = Some(format!("sector {sector} is past end of region file ({file_len} bytes)"));
            return e;
        };
        let len = u32::from_be_bytes(header[0..4].try_into().unwrap()) as usize;
        let raw = if len >= 1 {
            file.get(offset + 5..offset + 4 + len).map(|s| s.to_vec())
        } else {
            Some(Vec::new())
        };
        (len, header[4], raw)
    };
    let external = version_byte & 0x80 != 0;
    let version = version_byte & 0x7f;
    e.version = Some(version);
    if !(1..=4).contains(&version) && version != 127 {
        e.presence = Presence::BadVersion;
        e.note = Some(format!("version byte {version_byte:#x}"));
        return e;
    }
    let stored: Vec<u8> = if external {
        let mcc = folder.join(format!("c.{cx}.{cz}.mcc"));
        match std::fs::read(&mcc) {
            Ok(bytes) => bytes,
            Err(err) => {
                e.presence = Presence::ExternalMissing;
                e.note = Some(format!("{}: {err}", mcc.display()));
                return e;
            }
        }
    } else {
        if len == 0 {
            e.presence = Presence::Truncated;
            e.note = Some("allocated chunk with zero stream length".into());
            return e;
        }
        match internal_raw {
            Some(raw) => raw,
            None => {
                e.presence = Presence::Truncated;
                e.note = Some(format!("declared stream length {len} runs past end of region file"));
                return e;
            }
        }
    };
    e.stored_len = stored.len();
    let bytes = match decompress_stream(version, &stored) {
        Ok(bytes) => bytes,
        Err(err) => {
            e.presence = Presence::DecodeFailed;
            e.note = Some(format!("{err:#}"));
            return e;
        }
    };
    e.decompressed_len = Some(bytes.len());
    e.sha256 = Some(hex_sha256(&bytes));
    e.presence = if external { Presence::External } else { Presence::Internal };
    match nbt::parse_named(&bytes) {
        Ok(tag) => {
            e.content_sha256 = Some(content_hash(&tag));
            e.summary = Some(summarize(&tag));
        }
        Err(err) => {
            e.presence = Presence::NbtFailed;
            e.note = Some(format!("{err:#}"));
        }
    }
    e
}

/// The decompressed NBT bytes of one chunk (used by `read_box`). `Ok(None)` when the
/// game would also see no chunk (absent, external missing, truncated, bad version);
/// `Err` when bytes exist but do not decode.
pub fn read_chunk_bytes(dim: &Path, stream: Stream, cx: i32, cz: i32) -> Result<Option<Vec<u8>>> {
    let mut entries_cache = BTreeMap::new();
    let entry = inspect_chunk_cached(dim, stream, cx, cz, &mut entries_cache);
    match entry.presence {
        Presence::Internal | Presence::External | Presence::NbtFailed => {
            // Re-read through the raw path: inspect does not keep the bytes.
            raw_decompressed(dim, stream, cx, cz, &entry)
        }
        Presence::DecodeFailed => bail!("chunk {cx},{cz} ({}): {}", stream.folder(), entry.note.unwrap_or_default()),
        _ => Ok(None),
    }
}

fn raw_decompressed(dim: &Path, stream: Stream, cx: i32, cz: i32, entry: &ChunkEntry) -> Result<Option<Vec<u8>>> {
    let folder = dim.join(stream.folder());
    let version = entry.version.unwrap_or(3);
    if entry.presence == Presence::External || (entry.presence == Presence::NbtFailed && folder.join(format!("c.{cx}.{cz}.mcc")).exists()) {
        let bytes = std::fs::read(folder.join(format!("c.{cx}.{cz}.mcc")))?;
        return Ok(Some(decompress_stream(version, &bytes)?));
    }
    let (rx, rz) = (cx.div_euclid(32), cz.div_euclid(32));
    let file = std::fs::read(region_file_path(&folder, rx, rz)).unwrap_or_default();
    if file.len() < 8192 {
        return Ok(None);
    }
    let slot = 4 * ((cx.rem_euclid(32) + cz.rem_euclid(32) * 32) as usize);
    let loc = u32::from_be_bytes(file[slot..slot + 4].try_into()?);
    let offset = (loc >> 8) as usize * 4096;
    let len = u32::from_be_bytes(file.get(offset..offset + 4).context("chunk header")?.try_into()?) as usize;
    let raw = file.get(offset + 5..offset + 4 + len).context("chunk stream truncated")?;
    Ok(Some(decompress_stream(version, raw)?))
}

/// Every non-absent chunk of the given streams in a dimension, sorted by (stream, x, z).
/// Orphan `.mcc` files (no header entry names them) come back as External entries with a
/// note, because the game would resurrect their bytes if a stub ever pointed at them.
pub fn scan_dimension(dim: &Path, streams: &[Stream]) -> Result<Vec<ChunkEntry>> {
    let mut out = Vec::new();
    for &stream in streams {
        let folder = dim.join(stream.folder());
        let mut cache = BTreeMap::new();
        let mut named_external: std::collections::BTreeSet<(i32, i32)> = Default::default();
        let Ok(read_dir) = std::fs::read_dir(&folder) else { continue };
        let mut regions: Vec<(i32, i32)> = Vec::new();
        let mut externals: Vec<(i32, i32)> = Vec::new();
        for ent in read_dir {
            let name = ent?.file_name().to_string_lossy().into_owned();
            if let Some(rest) = name.strip_prefix("r.").and_then(|s| s.strip_suffix(".mca")) {
                let mut it = rest.split('.');
                if let (Some(Ok(rx)), Some(Ok(rz))) = (it.next().map(|s| s.parse()), it.next().map(|s| s.parse())) {
                    regions.push((rx, rz));
                }
            } else if let Some(rest) = name.strip_prefix("c.").and_then(|s| s.strip_suffix(".mcc")) {
                let mut it = rest.split('.');
                if let (Some(Ok(x)), Some(Ok(z))) = (it.next().map(|s| s.parse()), it.next().map(|s| s.parse())) {
                    externals.push((x, z));
                }
            }
        }
        regions.sort();
        for (rx, rz) in regions {
            for lz in 0..32 {
                for lx in 0..32 {
                    let (cx, cz) = (rx * 32 + lx, rz * 32 + lz);
                    let e = inspect_chunk_cached(dim, stream, cx, cz, &mut cache);
                    if e.presence != Presence::Absent {
                        if e.presence == Presence::External || e.presence == Presence::ExternalMissing {
                            named_external.insert((cx, cz));
                        }
                        out.push(e);
                    }
                }
            }
        }
        externals.sort();
        for (x, z) in externals {
            if !named_external.contains(&(x, z)) {
                let mcc = folder.join(format!("c.{x}.{z}.mcc"));
                let size = std::fs::metadata(&mcc).map(|m| m.len() as usize).unwrap_or(0);
                out.push(ChunkEntry {
                    stream,
                    cx: x,
                    cz: z,
                    presence: Presence::External,
                    version: None,
                    sectors: 0,
                    timestamp: 0,
                    stored_len: size,
                    decompressed_len: None,
                    sha256: None,
                    content_sha256: None,
                    summary: None,
                    note: Some("orphan .mcc (no region header entry)".into()),
                });
            }
        }
    }
    out.sort_by_key(|e| (e.stream, e.cx, e.cz));
    Ok(out)
}

/// Hash the tag tree deterministically (compound keys are BTreeMap-sorted), skipping the
/// root bookkeeping fields that change on every re-save.
fn content_hash(tag: &Tag) -> String {
    let mut hasher = Sha256::new();
    hash_tag(tag, true, &mut hasher);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest.iter() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hash_tag(tag: &Tag, is_root: bool, hasher: &mut Sha256) {
    match tag {
        Tag::Byte(v) => {
            hasher.update([1]);
            hasher.update(v.to_be_bytes());
        }
        Tag::Short(v) => {
            hasher.update([2]);
            hasher.update(v.to_be_bytes());
        }
        Tag::Int(v) => {
            hasher.update([3]);
            hasher.update(v.to_be_bytes());
        }
        Tag::Long(v) => {
            hasher.update([4]);
            hasher.update(v.to_be_bytes());
        }
        Tag::Float(v) => {
            hasher.update([5]);
            hasher.update(v.to_bits().to_be_bytes());
        }
        Tag::Double(v) => {
            hasher.update([6]);
            hasher.update(v.to_bits().to_be_bytes());
        }
        Tag::ByteArray(v) => {
            hasher.update([7]);
            hasher.update((v.len() as u64).to_be_bytes());
            hasher.update(v.iter().map(|b| *b as u8).collect::<Vec<u8>>());
        }
        Tag::String(v) => {
            hasher.update([8]);
            hasher.update((v.len() as u64).to_be_bytes());
            hasher.update(v.as_bytes());
        }
        Tag::List(v) => {
            hasher.update([9]);
            hasher.update((v.len() as u64).to_be_bytes());
            for item in v {
                hash_tag(item, false, hasher);
            }
        }
        Tag::Compound(m) => {
            hasher.update([10]);
            let kept: Vec<(&String, &Tag)> = m
                .iter()
                .filter(|(k, _)| !(is_root && (k.as_str() == "LastUpdate" || k.as_str() == "InhabitedTime")))
                .collect();
            hasher.update((kept.len() as u64).to_be_bytes());
            for (k, v) in kept {
                hasher.update((k.len() as u64).to_be_bytes());
                hasher.update(k.as_bytes());
                hash_tag(v, false, hasher);
            }
        }
        Tag::IntArray(v) => {
            hasher.update([11]);
            hasher.update((v.len() as u64).to_be_bytes());
            for i in v {
                hasher.update(i.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            hasher.update([12]);
            hasher.update((v.len() as u64).to_be_bytes());
            for i in v {
                hasher.update(i.to_be_bytes());
            }
        }
    }
}

fn summarize(tag: &Tag) -> TagSummary {
    let list_len = |key: &str| tag.get(key).map(|t| t.as_list().len()).unwrap_or(0);
    let mut poi_records = 0;
    if let Some(sections) = tag.get("Sections").and_then(|t| t.as_compound()) {
        for (_, section) in sections {
            poi_records += section.get("Records").map(|t| t.as_list().len()).unwrap_or(0);
        }
    }
    TagSummary {
        status: tag.get("Status").and_then(|t| t.as_str()).map(|s| s.to_string()),
        data_version: tag.get("DataVersion").and_then(|t| t.as_i64()),
        x_pos: tag.get("xPos").and_then(|t| t.as_i64()),
        z_pos: tag.get("zPos").and_then(|t| t.as_i64()),
        sections: list_len("sections"),
        block_entities: list_len("block_entities"),
        block_ticks: list_len("block_ticks"),
        fluid_ticks: list_len("fluid_ticks"),
        entities: list_len("entities") + list_len("Entities"),
        poi_records,
        inhabited_time: tag.get("InhabitedTime").and_then(|t| t.as_i64()),
        last_update: tag.get("LastUpdate").and_then(|t| t.as_i64()),
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest.iter() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Zero the header entry of one chunk in a region file (positive control; the caller must
/// pass a *copy* of a disposable world). Returns false when there was nothing to clear.
pub fn header_zero(dim: &Path, stream: Stream, cx: i32, cz: i32) -> Result<bool> {
    let folder = dim.join(stream.folder());
    let path = region_file_path(&folder, cx.div_euclid(32), cz.div_euclid(32));
    let mut file = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    if file.len() < 8192 {
        bail!("{} is {} bytes: no complete header to edit", path.display(), file.len());
    }
    let slot = 4 * ((cx.rem_euclid(32) + cz.rem_euclid(32) * 32) as usize);
    let had = file[slot..slot + 4] != [0, 0, 0, 0];
    file[slot..slot + 4].copy_from_slice(&[0, 0, 0, 0]);
    std::fs::write(&path, &file)?;
    let mcc = folder.join(format!("c.{cx}.{cz}.mcc"));
    let _ = std::fs::remove_file(mcc);
    Ok(had)
}

/// Scribble over the first bytes of a chunk's stored stream (positive control: the game
/// will fail to decode it). Returns the number of bytes written.
pub fn garbage_stream(dim: &Path, stream: Stream, cx: i32, cz: i32) -> Result<usize> {
    let folder = dim.join(stream.folder());
    let e = inspect_chunk(dim, stream, cx, cz);
    if e.presence == Presence::External {
        let mcc = folder.join(format!("c.{cx}.{cz}.mcc"));
        let mut bytes = std::fs::read(&mcc)?;
        let n = bytes.len().min(64);
        for b in &mut bytes[..n] {
            *b = 0xff;
        }
        std::fs::write(&mcc, &bytes)?;
        return Ok(n);
    }
    if e.presence != Presence::Internal {
        bail!("chunk {cx},{cz} has no internal/external stream to garble (presence {})", e.presence.name());
    }
    let path = region_file_path(&folder, cx.div_euclid(32), cz.div_euclid(32));
    let mut file = std::fs::read(&path)?;
    let slot = 4 * ((cx.rem_euclid(32) + cz.rem_euclid(32) * 32) as usize);
    let loc = u32::from_be_bytes(file[slot..slot + 4].try_into()?);
    let start = (loc >> 8) as usize * 4096 + 5;
    let n = (file.len() - start).min(64);
    for b in &mut file[start..start + n] {
        *b = 0xff;
    }
    std::fs::write(&path, &file)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-built named compound: {Status:"minecraft:full", DataVersion:4325, xPos:cx, zPos:cz}.
    fn chunk_tag_bytes(cx: i32, cz: i32) -> Vec<u8> {
        let mut b = Vec::new();
        let named_string = |b: &mut Vec<u8>, name: &str, value: &str| {
            b.push(8);
            b.extend_from_slice(&(name.len() as u16).to_be_bytes());
            b.extend_from_slice(name.as_bytes());
            b.extend_from_slice(&(value.len() as u16).to_be_bytes());
            b.extend_from_slice(value.as_bytes());
        };
        let named_int = |b: &mut Vec<u8>, name: &str, value: i32| {
            b.push(3);
            b.extend_from_slice(&(name.len() as u16).to_be_bytes());
            b.extend_from_slice(name.as_bytes());
            b.extend_from_slice(&value.to_be_bytes());
        };
        b.push(10);
        b.extend_from_slice(&0u16.to_be_bytes());
        named_string(&mut b, "Status", "minecraft:full");
        named_int(&mut b, "DataVersion", 4325);
        named_int(&mut b, "xPos", cx);
        named_int(&mut b, "zPos", cz);
        b.push(0);
        b
    }

    fn temp_dim(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mctas-chunkfile-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("region")).unwrap();
        dir
    }

    /// Write one region file with two version-3 chunks: (0,0) internal, (1,0) external.
    fn write_synthetic(dim: &Path) {
        let mut file = vec![0u8; 4096 * 4];
        let put_entry = |file: &mut Vec<u8>, lx: usize, sector: u32, count: u32| {
            let slot = 4 * lx;
            file[slot..slot + 4].copy_from_slice(&(sector << 8 | count).to_be_bytes());
            file[4096 + slot..4096 + slot + 4].copy_from_slice(&12345u32.to_be_bytes());
        };
        // internal at sector 2
        let tag = chunk_tag_bytes(0, 0);
        let off = 2 * 4096;
        file[off..off + 4].copy_from_slice(&((tag.len() + 1) as u32).to_be_bytes());
        file[off + 4] = 3;
        file[off + 5..off + 5 + tag.len()].copy_from_slice(&tag);
        put_entry(&mut file, 0, 2, 1);
        // external at sector 3 (stub), bytes in c.1.0.mcc
        let off = 3 * 4096;
        file[off..off + 4].copy_from_slice(&1u32.to_be_bytes());
        file[off + 4] = 3 | 128;
        put_entry(&mut file, 1, 3, 1);
        std::fs::write(dim.join("region").join("r.0.0.mca"), &file).unwrap();
        std::fs::write(dim.join("region").join("c.1.0.mcc"), chunk_tag_bytes(1, 0)).unwrap();
    }

    #[test]
    fn internal_and_external_round_trip() {
        let dim = temp_dim("roundtrip");
        write_synthetic(&dim);
        let a = inspect_chunk(&dim, Stream::Region, 0, 0);
        assert_eq!(a.presence, Presence::Internal);
        assert_eq!(a.version, Some(3));
        let s = a.summary.unwrap();
        assert_eq!(s.status.as_deref(), Some("minecraft:full"));
        assert_eq!(s.data_version, Some(4325));
        let b = inspect_chunk(&dim, Stream::Region, 1, 0);
        assert_eq!(b.presence, Presence::External);
        assert_eq!(b.summary.unwrap().x_pos, Some(1));
        assert_eq!(read_chunk_bytes(&dim, Stream::Region, 1, 0).unwrap().as_deref(), Some(chunk_tag_bytes(1, 0).as_slice()));
        let absent = inspect_chunk(&dim, Stream::Region, 5, 5);
        assert_eq!(absent.presence, Presence::Absent);
        let scan = scan_dimension(&dim, &[Stream::Region]).unwrap();
        assert_eq!(scan.len(), 2);
    }

    #[test]
    fn missing_external_and_controls() {
        let dim = temp_dim("controls");
        write_synthetic(&dim);
        std::fs::remove_file(dim.join("region").join("c.1.0.mcc")).unwrap();
        assert_eq!(inspect_chunk(&dim, Stream::Region, 1, 0).presence, Presence::ExternalMissing);
        assert!(header_zero(&dim, Stream::Region, 0, 0).unwrap());
        assert_eq!(inspect_chunk(&dim, Stream::Region, 0, 0).presence, Presence::Absent);
    }

    #[test]
    fn garbage_stream_breaks_decode_or_nbt() {
        let dim = temp_dim("garbage");
        write_synthetic(&dim);
        assert!(garbage_stream(&dim, Stream::Region, 0, 0).unwrap() > 0);
        let e = inspect_chunk(&dim, Stream::Region, 0, 0);
        // Version none: the bytes are the NBT itself, so the failure is NBT-level.
        assert_eq!(e.presence, Presence::NbtFailed);
    }
}
