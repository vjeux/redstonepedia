//! Chunk-regeneration study tooling (designs/chunk-regen/NOTES.md): snapshot manifests of
//! the terrain/entity/POI streams of a saved world, diffs between them, and the offline
//! positive controls (header-zero, drop the external `.mcc`, garble a stream) that a
//! harness applies to a *copy* of a disposable world to prove it can tell regeneration
//! from a round-trip. Reading uses `mctas::vanilla::chunkfile`.

pub mod cli;
pub mod exp;

use anyhow::{Context, Result, bail};
use mctas::vanilla::chunkfile::{self, ChunkEntry, Presence, Stream};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const HEADER: &str = "stream\tcx\tcz\tpresence\tversion\tsectors\tstored\tdecompressed\tsha256\tstatus\tdata_version\tsections\tblock_entities\tblock_ticks\tfluid_ticks\tentities\tpoi_records\tinhabited\tlast_update\ttimestamp\tnote";

fn entry_row(e: &ChunkEntry) -> String {
    let s = e.summary.as_ref();
    let opt = |v: Option<&dyn std::fmt::Display>| v.map(|d| d.to_string()).unwrap_or_else(|| "-".into());
    let fields: Vec<String> = vec![
        e.stream.folder().into(),
        e.cx.to_string(),
        e.cz.to_string(),
        e.presence.name().into(),
        opt(e.version.as_ref().map(|v| v as &dyn std::fmt::Display)),
        e.sectors.to_string(),
        e.stored_len.to_string(),
        opt(e.decompressed_len.as_ref().map(|v| v as &dyn std::fmt::Display)),
        e.sha256.clone().unwrap_or_else(|| "-".into()),
        s.and_then(|s| s.status.clone()).unwrap_or_else(|| "-".into()),
        opt(s.and_then(|s| s.data_version).as_ref().map(|v| v as &dyn std::fmt::Display)),
        s.map(|s| s.sections).unwrap_or(0).to_string(),
        s.map(|s| s.block_entities).unwrap_or(0).to_string(),
        s.map(|s| s.block_ticks).unwrap_or(0).to_string(),
        s.map(|s| s.fluid_ticks).unwrap_or(0).to_string(),
        s.map(|s| s.entities).unwrap_or(0).to_string(),
        s.map(|s| s.poi_records).unwrap_or(0).to_string(),
        opt(s.and_then(|s| s.inhabited_time).as_ref().map(|v| v as &dyn std::fmt::Display)),
        opt(s.and_then(|s| s.last_update).as_ref().map(|v| v as &dyn std::fmt::Display)),
        e.timestamp.to_string(),
        e.note.clone().unwrap_or_else(|| "-".into()).replace(['\t', '\n'], " "),
    ];
    fields.join("\t")
}

/// One line per chunk per stream, for a human or a diff.
pub fn manifest_text(entries: &[ChunkEntry]) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for e in entries {
        out.push_str(&entry_row(e));
        out.push('\n');
    }
    out
}

pub fn write_manifest(path: &Path, entries: &[ChunkEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, manifest_text(entries)).with_context(|| format!("writing {}", path.display()))
}

/// Scan a saved world (server dir, level folder or dimension folder) into entries.
pub fn snap(input: &Path, streams: &[Stream]) -> Result<Vec<ChunkEntry>> {
    let dim = chunkfile::dimension_dir(input);
    chunkfile::scan_dimension(&dim, streams)
}

/// Inspect specific chunks (including absent ones — that is the signal for regen).
pub fn inspect(input: &Path, streams: &[Stream], chunks: &[(i32, i32)]) -> Vec<ChunkEntry> {
    let dim = chunkfile::dimension_dir(input);
    let mut out = Vec::new();
    for &stream in streams {
        for &(cx, cz) in chunks {
            out.push(chunkfile::inspect_chunk(&dim, stream, cx, cz));
        }
    }
    out.sort_by_key(|e| (e.stream, e.cx, e.cz));
    out
}

type Key = (String, i32, i32);

fn parse_manifest(text: &str) -> Result<BTreeMap<Key, Vec<String>>> {
    let mut map = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 && line == HEADER {
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let f: Vec<String> = line.split('\t').map(|s| s.to_string()).collect();
        if f.len() < 9 {
            bail!("manifest line {} has {} fields (want at least 9)", i + 1, f.len());
        }
        let cx: i32 = f[1].parse().with_context(|| format!("manifest line {}: cx", i + 1))?;
        let cz: i32 = f[2].parse().with_context(|| format!("manifest line {}: cz", i + 1))?;
        map.insert((f[0].clone(), cx, cz), f);
    }
    Ok(map)
}

/// What changed between two manifests. Identity is (presence, sha256): a re-save of an
/// untouched chunk keeps both; InhabitedTime growth changes the hash, so a CHANGED line
/// must be read with its counts (printed) before claiming a block changed.
pub fn diff_manifests(before: &str, after: &str) -> Result<String> {
    let b = parse_manifest(before)?;
    let a = parse_manifest(after)?;
    let (mut added, mut removed, mut changed, mut same) = (0usize, 0usize, 0usize, 0usize);
    let mut lines = Vec::new();
    for (k, av) in &a {
        match b.get(k) {
            None => {
                added += 1;
                lines.push(format!("ADDED {} {} {} presence={} sha={}", k.0, k.1, k.2, av[3], short(&av[8])));
            }
            Some(bv) if bv[3] != av[3] || bv[8] != av[8] => {
                changed += 1;
                lines.push(format!(
                    "CHANGED {} {} {} {}->{} sha {}->{} be {}->{} ent {}->{} poi {}->{}",
                    k.0, k.1, k.2, bv[3], av[3], short(&bv[8]), short(&av[8]), bv[12], av[12], bv[15], av[15], bv[16], av[16]
                ));
            }
            Some(_) => same += 1,
        }
    }
    for k in b.keys() {
        if !a.contains_key(k) {
            removed += 1;
            lines.push(format!("REMOVED {} {} {}", k.0, k.1, k.2));
        }
    }
    lines.sort();
    let mut out = format!("added {added} removed {removed} changed {changed} same {same}\n");
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    Ok(out)
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

/// The offline positive controls. Each operates in place — callers must pass a copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Zero the region header entry (and remove the `.mcc`): the chunk reads as absent.
    HeaderZero,
    /// Delete only the external `.mcc` file: the state a kill between `RegionFile`'s
    /// header commit and its external rename would leave.
    DropExternal,
    /// Scribble over the stored stream: the chunk fails to decode on load.
    Garbage,
}

impl Control {
    pub fn parse(name: &str) -> Result<Control> {
        Ok(match name {
            "header-zero" => Control::HeaderZero,
            "drop-external" => Control::DropExternal,
            "garbage" => Control::Garbage,
            other => bail!("control must be header-zero, drop-external or garbage, got `{other}`"),
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Control::HeaderZero => "header-zero",
            Control::DropExternal => "drop-external",
            Control::Garbage => "garbage",
        }
    }
}

pub fn apply_control(input: &Path, control: Control, stream: Stream, cx: i32, cz: i32) -> Result<String> {
    let dim = chunkfile::dimension_dir(input);
    let before = chunkfile::inspect_chunk(&dim, stream, cx, cz);
    let msg = match control {
        Control::HeaderZero => {
            let had = chunkfile::header_zero(&dim, stream, cx, cz)?;
            format!("header entry zeroed (had content: {had})")
        }
        Control::DropExternal => {
            if before.presence != Presence::External {
                bail!("chunk {cx},{cz} ({}) is {}, not external: nothing to drop", stream.folder(), before.presence.name());
            }
            let mcc: PathBuf = dim.join(stream.folder()).join(format!("c.{cx}.{cz}.mcc"));
            std::fs::remove_file(&mcc).with_context(|| format!("removing {}", mcc.display()))?;
            format!("removed {}", mcc.display())
        }
        Control::Garbage => {
            let n = chunkfile::garbage_stream(&dim, stream, cx, cz)?;
            format!("scribbled {n} stream bytes")
        }
    };
    let after = chunkfile::inspect_chunk(&dim, stream, cx, cz);
    Ok(format!("control {} on {} {cx},{cz}: {} -> {}; {msg}", control.name(), stream.folder(), before.presence.name(), after.presence.name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(presence: Presence, sha: &str) -> ChunkEntry {
        ChunkEntry {
            stream: Stream::Region,
            cx: 4,
            cz: 4,
            presence,
            version: Some(2),
            sectors: 3,
            timestamp: 7,
            stored_len: 10,
            decompressed_len: Some(20),
            sha256: Some(sha.into()),
            summary: None,
            note: None,
        }
    }

    #[test]
    fn diff_names_every_kind() {
        let before = manifest_text(&[entry(Presence::Internal, "aaaaaaaaaaaaaaaa")]);
        let after_same = manifest_text(&[entry(Presence::Internal, "aaaaaaaaaaaaaaaa")]);
        assert!(diff_manifests(&before, &after_same).unwrap().starts_with("added 0 removed 0 changed 0 same 1"));
        let after_gone = manifest_text(&[]);
        assert!(diff_manifests(&before, &after_gone).unwrap().contains("REMOVED region 4 4"));
        let after_ext = manifest_text(&[entry(Presence::External, "bbbbbbbbbbbbbbbb")]);
        let d = diff_manifests(&before, &after_ext).unwrap();
        assert!(d.contains("CHANGED region 4 4 internal->external"), "{d}");
    }
}
