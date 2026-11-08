//! Recorded game answers (`mctas test`'s simulator tier, docs/tools.md#mctas-test): a run of the
//! real game recorded ONCE (an exact trace, a judge's frames) and replayed on redsim for every
//! change of the simulator, in milliseconds. The game never changes (26.3), so the recording is
//! the game's answer for as long as the files it was recorded from stay the same.
//!
//! Every recording directory holds a manifest, [`MANIFEST`] (`recorded.tsv`): one row per
//! recording with its content hash, the files it was recorded from (its design, from the
//! recording's JSON header: `design` and the optional `deps` list) with their hashes, the game
//! version, when and from which commit it was recorded, and how (the command). [`check_dir`]
//! fails on a STALE recording (a file it was recorded from changed since: the recording no
//! longer says what the game does with it), on a recording edited without its row, on a row
//! whose recording is gone and on a recording without a row: re-record it (the row names the
//! command), then `mctas recorded write <dir>`. A stale recording never passes silently.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// The manifest file of a recording directory.
pub const MANIFEST: &str = "recorded.tsv";

/// The extensions of the recordings a directory holds (exact traces, judge frame recordings).
pub const EXTENSIONS: [&str; 2] = ["trace", "rec"];

const HEADER: &str = "file\thash\tdeps\tversion\trecorded\thow";

/// FNV-1a, 64 bits (the verdict caches' hash).
pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// A file's content hash, 16 hex digits.
pub fn file_hash(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:016x}", fnv64(&bytes)))
}

/// One manifest row.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The recording's file name (in the manifest's directory).
    pub file: String,
    /// Its content hash.
    pub hash: String,
    /// The files it was recorded from (repository paths) and their hashes then.
    pub deps: Vec<(String, String)>,
    /// The game version it was recorded with.
    pub version: String,
    /// When and from which commit (`2026-11-08T17:10Z 4ab5af6ed`).
    pub recorded: String,
    /// How: the command that records it again.
    pub how: String,
}

impl Row {
    fn line(&self) -> String {
        let deps: Vec<String> = self.deps.iter().map(|(p, h)| format!("{p}={h}")).collect();
        let clean = |s: &str| s.replace(['\t', '\n'], " ");
        format!("{}\t{}\t{}\t{}\t{}\t{}", clean(&self.file), self.hash, clean(&deps.join(";")), clean(&self.version), clean(&self.recorded), clean(&self.how))
    }

    fn parse(line: &str) -> Result<Row> {
        let t: Vec<&str> = line.split('\t').collect();
        if t.len() != 6 {
            bail!("{} columns, want 6: {line}", t.len());
        }
        let deps = t[2]
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.rsplit_once('=').map(|(p, h)| (p.to_string(), h.to_string())).with_context(|| format!("dep without a hash: {s}")))
            .collect::<Result<Vec<_>>>()?;
        Ok(Row { file: t[0].into(), hash: t[1].into(), deps, version: t[3].into(), recorded: t[4].into(), how: t[5].into() })
    }
}

/// Read a directory's manifest (an empty list when it has none).
pub fn load(dir: &Path) -> Result<Vec<Row>> {
    let path = dir.join(MANIFEST);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.starts_with("file\t") || line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        rows.push(Row::parse(line).with_context(|| format!("{} line {}", path.display(), i + 1))?);
    }
    Ok(rows)
}

/// Write a manifest (rows sorted by file).
pub fn save(dir: &Path, rows: &[Row]) -> Result<()> {
    let mut rows = rows.to_vec();
    rows.sort_by(|a, b| a.file.cmp(&b.file));
    let mut text = format!("# recordings of the real game replayed on redsim (mctas recorded check; crates/mctas/src/recorded.rs)\n{HEADER}\n");
    for r in &rows {
        text.push_str(&r.line());
        text.push('\n');
    }
    std::fs::write(dir.join(MANIFEST), text).with_context(|| format!("write {}", dir.join(MANIFEST).display()))
}

/// The recordings of a directory (its own files, not its subdirectories), sorted.
pub fn recordings(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| EXTENSIONS.iter().any(|x| e == *x)))
        .collect();
    out.sort();
    Ok(out)
}

/// A recording's text: plain, or gzip-compressed (the judges' frame recordings, `.rec`: their
/// per-gt container and timer states compress ~25x).
pub fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.starts_with(&[0x1f, 0x8b]) {
        use std::io::Read;
        let mut s = String::new();
        flate2::read::GzDecoder::new(&bytes[..]).read_to_string(&mut s).with_context(|| format!("gunzip {}", path.display()))?;
        Ok(s)
    } else {
        String::from_utf8(bytes).with_context(|| format!("{}: not UTF-8", path.display()))
    }
}

/// Write a recording's text gzip-compressed (read back by [`read_text`]).
pub fn write_gz(path: &Path, text: &str) -> Result<()> {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(text.as_bytes())?;
    std::fs::write(path, e.finish()?).with_context(|| format!("write {}", path.display()))
}

/// A string field of a recording's JSON header (its first line), when it has one.
pub fn header_field(path: &Path, key: &str) -> Option<String> {
    let text = read_text(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(text.lines().next()?).ok()?;
    v[key].as_str().map(String::from)
}

/// The files a recording was recorded from, from its first line (a JSON header): `design` and
/// every path of `deps` (repository paths).
pub fn header_deps(path: &Path) -> Result<Vec<String>> {
    let text = read_text(path)?;
    let first = text.lines().next().unwrap_or("");
    let v: serde_json::Value = serde_json::from_str(first).with_context(|| format!("{}: the first line is not a JSON header", path.display()))?;
    let mut deps = Vec::new();
    if let Some(d) = v["design"].as_str() {
        deps.push(d.to_string());
    }
    if let Some(a) = v["deps"].as_array() {
        deps.extend(a.iter().filter_map(|s| s.as_str().map(String::from)));
    }
    deps.dedup();
    Ok(deps)
}

/// One problem of a recording directory.
#[derive(Debug, PartialEq)]
pub enum Problem {
    /// A file the recording was recorded from changed (or is gone).
    Stale { file: String, dep: String, why: String, how: String },
    /// The recording itself changed since its row was written.
    Edited { file: String },
    /// A recording with no row.
    Unlisted { file: String },
    /// A row whose recording is gone.
    Missing { file: String },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::Stale { file, dep, why, how } => write!(f, "STALE {file}: {dep} {why} since it was recorded; record it again (`{how}`), then `mctas recorded write`"),
            Problem::Edited { file } => write!(f, "EDITED {file}: the recording changed without its manifest row (`mctas recorded write` after recording it again)"),
            Problem::Unlisted { file } => write!(f, "UNLISTED {file}: a recording without a manifest row (`mctas recorded write`)"),
            Problem::Missing { file } => write!(f, "MISSING {file}: the manifest lists a recording that is gone (`mctas recorded write`)"),
        }
    }
}

/// Check a recording directory against its manifest (paths of `deps` are relative to `root`,
/// the repository root). Every problem is listed; none means every recording is current.
pub fn check_dir(root: &Path, dir: &Path) -> Result<Vec<Problem>> {
    let rows = load(dir)?;
    let files = recordings(dir)?;
    let mut out = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let Some(r) = rows.iter().find(|r| r.file == name) else {
            out.push(Problem::Unlisted { file: name });
            continue;
        };
        if file_hash(f)? != r.hash {
            out.push(Problem::Edited { file: name.clone() });
        }
        for (dep, h) in &r.deps {
            let p = root.join(dep);
            let why = match file_hash(&p) {
                Ok(now) if &now == h => continue,
                Ok(_) => "changed",
                Err(_) => "is gone",
            };
            out.push(Problem::Stale { file: name.clone(), dep: dep.clone(), why: why.into(), how: r.how.clone() });
        }
    }
    for r in &rows {
        if !files.iter().any(|f| f.file_name().is_some_and(|n| n.to_string_lossy() == r.file)) {
            out.push(Problem::Missing { file: r.file.clone() });
        }
    }
    Ok(out)
}

/// The rows of a directory written again from its recordings: every recording's hash and its
/// header's files and their hashes now. A row that already exists keeps its version, date and
/// command unless the recording changed (`fresh` gives them: the recording was just made);
/// a new row takes them from `fresh`, else from `legacy` (a recording made before manifests:
/// the file's provenance from git).
pub fn rows_for(root: &Path, dir: &Path, fresh: Option<&dyn Fn(&Path) -> (String, String, String)>, legacy: &dyn Fn(&Path) -> (String, String, String)) -> Result<Vec<Row>> {
    let old = load(dir)?;
    let mut rows = Vec::new();
    for f in recordings(dir)? {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let hash = file_hash(&f)?;
        let mut deps = Vec::new();
        for d in header_deps(&f)? {
            let h = file_hash(&root.join(&d)).with_context(|| format!("{name}: its design {d}"))?;
            deps.push((d, h));
        }
        let keep = old.iter().find(|r| r.file == name && r.hash == hash);
        let (version, recorded, how) = match (keep, fresh) {
            (Some(r), _) => (r.version.clone(), r.recorded.clone(), r.how.clone()),
            (None, Some(fr)) => fr(&f),
            (None, None) => legacy(&f),
        };
        rows.push(Row { file: name, hash, deps, version, recorded, how });
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_round_trip_and_stale_recordings_fail() {
        let root = std::env::temp_dir().join(format!("mctas-recorded-test-{}", std::process::id()));
        let dir = root.join("rec");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(root.join("d.txt"), "design v1\n").unwrap();
        std::fs::write(dir.join("a.trace"), "{\"design\":\"d.txt\",\"deps\":[\"d.txt\"]}\n{}\n").unwrap();
        let fresh = |_: &Path| ("26.3".to_string(), "2026-11-08T17:00Z abc".to_string(), "mctas x".to_string());
        let rows = rows_for(&root, &dir, Some(&fresh), &fresh).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].deps.len(), 1);
        save(&dir, &rows).unwrap();
        assert_eq!(load(&dir).unwrap(), rows);
        assert!(check_dir(&root, &dir).unwrap().is_empty());
        // the design changes: the recording is stale
        std::fs::write(root.join("d.txt"), "design v2\n").unwrap();
        let p = check_dir(&root, &dir).unwrap();
        assert!(matches!(&p[..], [Problem::Stale { .. }]), "{p:?}");
        // a recording without a row, and a row without its recording
        std::fs::write(dir.join("b.rec"), "{\"design\":\"d.txt\"}\n").unwrap();
        std::fs::remove_file(dir.join("a.trace")).unwrap();
        let p = check_dir(&root, &dir).unwrap();
        assert!(p.contains(&Problem::Unlisted { file: "b.rec".into() }) && p.contains(&Problem::Missing { file: "a.trace".into() }), "{p:?}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
