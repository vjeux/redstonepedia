//! A persistent cache of the site build's simulations, by content (the coordinator 2026-10-21
//! 06:07Z: deploys had gone from 1.5 to 4.5 minutes, the clock members simulated at every build).
//!
//! An entry's key hashes what its result depends on (a design's text, the parameters) and the
//! source of the code that computes it, one hash per [`Domain`] (build.rs): an unchanged design
//! costs nothing; a change to a simulator recomputes everything that simulator touches, and only
//! that. Entries are JSON files under `~/.cache/mctas/site/<domain>-<code>/` (`$MCTAS_SITE_CACHE`
//! names another folder; `off` turns the cache off), written atomically (a temporary file, then a
//! rename), so parallel threads and concurrent builds share it safely. A failed computation is
//! never stored (a missing tool, a full disk: the next build tries again).

use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// What computes an entry: its key carries the hash of that code (build.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// A generated clock member (its generator, the clock judge's simulation).
    Members,
    /// The circuit judge's run of a design.
    Circuit,
    /// The 3D view's own session (a circuit's view check, a clock's chart and lamp check).
    View,
    /// A work-in-progress design's verdict for `mctas site preview` (src/simulate/preview.rs).
    Preview,
}

const ALL: [Domain; 4] = [Domain::Members, Domain::Circuit, Domain::View, Domain::Preview];

impl Domain {
    fn name(self) -> &'static str {
        match self {
            Domain::Members => "members",
            Domain::Circuit => "circuit",
            Domain::View => "view",
            Domain::Preview => "preview",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Domain::Members => env!("SITE_CODE_MEMBERS"),
            Domain::Circuit => env!("SITE_CODE_CIRCUIT"),
            Domain::View => env!("SITE_CODE_VIEW"),
            Domain::Preview => env!("SITE_CODE_PREVIEW"),
        }
    }

    fn index(self) -> usize {
        ALL.iter().position(|d| *d == self).unwrap_or(0)
    }

    /// The folder of this domain's entries for this code.
    fn folder(self) -> String {
        format!("{}-{}", self.name(), &self.code()[..16])
    }
}

struct Cache {
    root: Option<PathBuf>,
    hits: [AtomicUsize; 4],
    misses: [AtomicUsize; 4],
    /// Milliseconds spent computing each domain's misses (summed over threads).
    busy_ms: [AtomicU64; 4],
    /// Each domain's slowest computations: (seconds, what).
    slowest: [Mutex<Vec<(f64, String)>>; 4],
    tmp: AtomicU64,
}

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| {
        let root = if cfg!(test) {
            // (unit tests compute everything: nothing shared with the builds)
            None
        } else {
            match std::env::var("MCTAS_SITE_CACHE") {
                Ok(v) if v == "off" => None,
                Ok(v) if !v.is_empty() => Some(PathBuf::from(v)),
                _ => std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))).map(|c| c.join("mctas/site")),
            }
        };
        Cache { root, hits: Default::default(), misses: Default::default(), busy_ms: Default::default(), slowest: Default::default(), tmp: AtomicU64::new(0) }
    })
}

/// What an entry is, for the build's report of the slowest: the first line of the first part that
/// is a design's text (`# CLOCK: …`, `# CIRCUIT: …`), else its first short text part.
fn label(parts: &[&[u8]]) -> String {
    let texts: Vec<&str> = parts.iter().filter_map(|p| std::str::from_utf8(p).ok()).collect();
    let pick = texts.iter().find(|t| t.contains('\n')).map(|t| t.lines().find(|l| l.starts_with("# ")).unwrap_or_else(|| t.lines().next().unwrap_or("")).to_string()).or_else(|| texts.iter().find(|t| !t.is_empty() && t.len() < 80).map(|t| t.to_string())).unwrap_or_default();
    let tag = texts.iter().find(|t| matches!(**t, "still" | "demo")).map(|t| format!("{t}: ")).unwrap_or_default();
    let s: String = pick.chars().take(70).collect();
    format!("{tag}{s}")
}

/// A miss computed: its time counted, and kept if it is among the slowest.
fn computed(domain: Domain, parts: &[&[u8]], t: std::time::Instant) {
    let c = cache();
    let secs = t.elapsed().as_secs_f64();
    c.busy_ms[domain.index()].fetch_add((secs * 1000.0) as u64, Ordering::Relaxed);
    if let Ok(mut v) = c.slowest[domain.index()].lock() {
        if v.len() < 5 || v.last().is_some_and(|l| l.0 < secs) {
            v.push((secs, label(parts)));
            v.sort_by(|a, b| b.0.total_cmp(&a.0));
            v.truncate(5);
        }
    }
}

/// The hash of the code of a member generator's own crate (`mctas clock item gen …` is
/// clock-item's): a key part of its members, beside [`Domain::Members`]'s common code.
pub fn generator_code(generator: &str) -> &'static str {
    let w: Vec<&str> = generator.split_whitespace().collect();
    match w.as_slice() {
        ["mctas", "clock", "item", ..] => env!("SITE_CODE_GEN_ITEM"),
        ["mctas", "clock", "misc", ..] => env!("SITE_CODE_GEN_MISC"),
        ["mctas", "clock", "obs", ..] => env!("SITE_CODE_GEN_OBS"),
        ["mctas", "clock", "rt", ..] => env!("SITE_CODE_GEN_RT"),
        ["mctas", "clock", "sculk", ..] => env!("SITE_CODE_GEN_SCULK"),
        _ => env!("SITE_CODE_GEN_ALL"),
    }
}

/// The cache's folder, if it is on.
pub fn root() -> Option<PathBuf> {
    cache().root.clone()
}

fn key(domain: Domain, parts: &[&[u8]]) -> String {
    let mut h = Sha256::new();
    h.update(domain.name().as_bytes());
    h.update(domain.code().as_bytes());
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p);
    }
    h.finalize().iter().take(16).map(|b| format!("{b:02x}")).collect()
}

fn read<T: DeserializeOwned>(domain: Domain, k: &str) -> Option<T> {
    let root = cache().root.as_ref()?;
    let bytes = std::fs::read(root.join(domain.folder()).join(format!("{k}.json"))).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write<T: Serialize>(domain: Domain, k: &str, v: &T) {
    let c = cache();
    let Some(root) = c.root.as_ref() else { return };
    let dir = root.join(domain.folder());
    let Ok(bytes) = serde_json::to_vec(v) else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let tmp = dir.join(format!(".{k}.{}.{}", std::process::id(), c.tmp.fetch_add(1, Ordering::Relaxed)));
    if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, dir.join(format!("{k}.json"))).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// The cached result for `parts` (what it depends on, besides the domain's code), else `f()`,
/// stored for the next build.
pub fn get_or<T: Serialize + DeserializeOwned>(domain: Domain, parts: &[&[u8]], f: impl FnOnce() -> T) -> T {
    let c = cache();
    if c.root.is_none() {
        return f();
    }
    let k = key(domain, parts);
    if let Some(v) = read(domain, &k) {
        c.hits[domain.index()].fetch_add(1, Ordering::Relaxed);
        return v;
    }
    c.misses[domain.index()].fetch_add(1, Ordering::Relaxed);
    let t = std::time::Instant::now();
    let v = f();
    computed(domain, parts, t);
    write(domain, &k, &v);
    v
}

/// [`get_or`] for a computation that can fail: only a success is stored.
pub fn get_or_try<T: Serialize + DeserializeOwned, E>(domain: Domain, parts: &[&[u8]], f: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    let c = cache();
    if c.root.is_none() {
        return f();
    }
    let k = key(domain, parts);
    if let Some(v) = read(domain, &k) {
        c.hits[domain.index()].fetch_add(1, Ordering::Relaxed);
        return Ok(v);
    }
    c.misses[domain.index()].fetch_add(1, Ordering::Relaxed);
    let t = std::time::Instant::now();
    let v = f();
    computed(domain, parts, t);
    let v = v?;
    write(domain, &k, &v);
    Ok(v)
}

/// This run's hits and misses per domain, for the build's last lines, with the time its misses
/// took (thread-seconds) and the slowest of them: what a cold build spent its time on.
pub fn report() -> String {
    let c = cache();
    match &c.root {
        None => "cache: off".to_string(),
        Some(root) => {
            let mut s = format!(
                "cache ({}): {}",
                root.display(),
                ALL.iter()
                    // (a domain this run never asked for: no line, the previews' in a site build)
                    .filter(|d| c.hits[d.index()].load(Ordering::Relaxed) + c.misses[d.index()].load(Ordering::Relaxed) > 0 || **d != Domain::Preview)
                    .map(|d| format!("{} {} reused, {} computed ({:.0} thread-s)", d.name(), c.hits[d.index()].load(Ordering::Relaxed), c.misses[d.index()].load(Ordering::Relaxed), c.busy_ms[d.index()].load(Ordering::Relaxed) as f64 / 1000.0))
                    .collect::<Vec<_>>()
                    .join(" · ")
            );
            for d in ALL {
                let v = c.slowest[d.index()].lock().map(|v| v.clone()).unwrap_or_default();
                if v.first().is_some_and(|x| x.0 >= 5.0) {
                    s.push_str(&format!("\n  note: slowest {} computations: {}", d.name(), v.iter().map(|(t, w)| format!("{w} ({t:.0} s)")).collect::<Vec<_>>().join(", ")));
                }
            }
            s
        }
    }
}

/// Keep the cache from growing without end: mark this code's folders used, and remove the folders
/// of each domain beyond the 4 most recently used (older code: another clone on this box may still
/// run one of the newest few).
pub fn prune() {
    let Some(root) = cache().root.clone() else { return };
    for d in ALL {
        let mine = root.join(d.folder());
        if mine.is_dir() {
            let _ = std::fs::write(mine.join(".used"), b"");
        }
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        let prefix = format!("{}-", d.name());
        let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = rd
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix) && e.path().is_dir())
            .map(|e| {
                let used = std::fs::metadata(e.path().join(".used")).and_then(|m| m.modified()).or_else(|_| e.metadata().and_then(|m| m.modified())).unwrap_or(std::time::UNIX_EPOCH);
                (used, e.path())
            })
            .collect();
        dirs.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, p) in dirs.into_iter().skip(4) {
            if p != mine {
                let _ = std::fs::remove_dir_all(&p);
            }
        }
    }
}
