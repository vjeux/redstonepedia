//! Recorded game frames (`mctas test`'s simulator pieces, docs/tools.md#mctas-test): a judge's
//! run recorded ONCE in the real game (every cell of the watched box every gt, the frames the
//! exact gates compare: block state, container contents, furnace / brew timers, comparator
//! outputs) and replayed on redsim for every simulator change, without the game.
//!
//! The file: a JSON header (`kind`, `design`: the frozen copy of the design it was recorded from,
//! next to the recording; `design_fnv`; `run`, `margin`, `frames`, `version`, `recorded`, `how`),
//! then one line per frame: the cells that changed since the previous frame, `[x,y,z,"state"]`,
//! `null` for a cell that became empty (frame 0 lists every cell). A recording whose design
//! changed is STALE and fails ([`replay_check`], and `mctas recorded check` on its directory).

use crate::Pos;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One frame: the watched box's non-empty cells and their full states.
pub type Frame = Vec<(Pos, String)>;

/// How an exact gate runs: live (the game now), recording the game's frames, or replaying a
/// recording (no game).
pub enum Mode {
    Live,
    /// Record into this file (its design frozen next to it beforehand: [`freeze`]).
    Record { file: PathBuf, how: String },
    /// Compare redsim with this recording.
    Replay { file: PathBuf },
}

/// Copy a design next to the recordings (`dir/<file name>`), once: recordings are made from the
/// frozen copy, so design work on the original never makes them stale. An existing different copy
/// is refused (a recording may depend on it).
pub fn freeze(design: &Path, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let copy = dir.join(design.file_name().context("design file name")?);
    let bytes = std::fs::read(design).with_context(|| format!("read {}", design.display()))?;
    if copy.exists() {
        if std::fs::read(&copy)? != bytes {
            bail!("{} exists and differs from {}: a recording may depend on it (remove its recordings first)", copy.display(), design.display());
        }
    } else {
        std::fs::write(&copy, &bytes)?;
    }
    Ok(copy)
}

/// Save the game's frames.
pub fn save(file: &Path, kind: &str, design: &Path, run: &str, margin: i32, how: &str, frames: &[Frame]) -> Result<()> {
    let head = std::process::Command::new("git").args(["rev-parse", "--short", "HEAD"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let header = serde_json::json!({
        "kind": kind,
        "design": design.to_string_lossy(),
        "design_fnv": mctas::recorded::file_hash(design)?,
        "run": run,
        "margin": margin,
        "frames": frames.len(),
        "version": mctas::DEFAULT_VERSION,
        "recorded": format!("{} on {head}", clock::catalog::now_utc()),
        "how": how,
    });
    let mut out = serde_json::to_string(&header)?;
    out.push('\n');
    let mut prev: BTreeMap<Pos, &String> = BTreeMap::new();
    for f in frames {
        let now: BTreeMap<Pos, &String> = f.iter().map(|(p, s)| (*p, s)).collect();
        let mut line: Vec<serde_json::Value> = Vec::new();
        for (p, s) in &now {
            if prev.get(p) != Some(s) {
                line.push(serde_json::json!([p[0], p[1], p[2], s]));
            }
        }
        for p in prev.keys() {
            if !now.contains_key(p) {
                line.push(serde_json::json!([p[0], p[1], p[2], null]));
            }
        }
        out.push_str(&serde_json::to_string(&line)?);
        out.push('\n');
        prev = now;
    }
    mctas::recorded::write_gz(file, &out)
}

/// A recording: its header and its frames.
pub fn load(file: &Path) -> Result<(serde_json::Value, Vec<Frame>)> {
    let text = mctas::recorded::read_text(file)?;
    let mut lines = text.lines();
    let header: serde_json::Value = serde_json::from_str(lines.next().context("empty recording")?)?;
    let mut cur: BTreeMap<Pos, String> = BTreeMap::new();
    let mut frames = Vec::new();
    for (i, l) in lines.enumerate() {
        let v: Vec<serde_json::Value> = serde_json::from_str(l).with_context(|| format!("{} frame {i}", file.display()))?;
        for c in v {
            let a = c.as_array().context("cell")?;
            let p = [a[0].as_i64().context("x")? as i32, a[1].as_i64().context("y")? as i32, a[2].as_i64().context("z")? as i32];
            match a[3].as_str() {
                Some(s) => {
                    cur.insert(p, s.to_string());
                }
                None => {
                    cur.remove(&p);
                }
            }
        }
        frames.push(cur.iter().map(|(p, s)| (*p, s.clone())).collect());
    }
    if header["frames"].as_u64().is_some_and(|n| n as usize != frames.len()) {
        bail!("{}: {} frames, the header says {}", file.display(), frames.len(), header["frames"]);
    }
    Ok((header, frames))
}

/// A recording's design, checked: the design it was recorded from must be byte for byte the
/// same, and the game version the same; else it is stale (Err: re-record it).
pub fn replay_check(file: &Path, header: &serde_json::Value) -> Result<PathBuf> {
    let design = PathBuf::from(header["design"].as_str().context("recording without a design")?);
    let want = header["design_fnv"].as_str().unwrap_or("");
    let now = mctas::recorded::file_hash(&design).unwrap_or_else(|_| "gone".into());
    if now != want {
        bail!("STALE {}: {} changed since it was recorded ({}); record it again: {}", file.display(), design.display(), header["recorded"].as_str().unwrap_or("?"), header["how"].as_str().unwrap_or("?"));
    }
    if header["version"].as_str() != Some(mctas::DEFAULT_VERSION) {
        bail!("STALE {}: recorded with the game {}, the tools run {}", file.display(), header["version"], mctas::DEFAULT_VERSION);
    }
    Ok(design)
}

/// The first difference between the game's frames and redsim's: `Ok(frames)` when identical
/// (the exact gates' wording otherwise: the frame and up to 4 cells).
pub fn first_diff(game: &[Frame], sim: &[Frame]) -> std::result::Result<usize, String> {
    let n = game.len().min(sim.len());
    for f in 0..n {
        let a: BTreeMap<Pos, &String> = game[f].iter().map(|(p, s)| (*p, s)).collect();
        let b: BTreeMap<Pos, &String> = sim[f].iter().map(|(p, s)| (*p, s)).collect();
        if a != b {
            let keys: std::collections::BTreeSet<&Pos> = a.keys().chain(b.keys()).collect();
            let diffs: Vec<String> = keys
                .into_iter()
                .filter(|p| a.get(*p) != b.get(*p))
                .take(4)
                .map(|p| format!("{p:?} game {} / redsim {}", a.get(p).map(|s| s.as_str()).unwrap_or("air"), b.get(p).map(|s| s.as_str()).unwrap_or("air")))
                .collect();
            return Err(format!("frame {f} of {n}: {}", diffs.join("; ")));
        }
    }
    if game.len() != sim.len() {
        return Err(format!("{} frames in the game, {} in redsim", game.len(), sim.len()));
    }
    Ok(n)
}

/// One exact gate: redsim's frames against the game's (live, recorded now, or from a recording).
/// `game` runs the game (not called when replaying).
pub fn gate(kind: &str, design: &Path, run: &str, margin: i32, sim: std::result::Result<Vec<Frame>, String>, game: impl FnOnce() -> Result<std::result::Result<Vec<Frame>, String>>, mode: &Mode) -> Result<std::result::Result<usize, String>> {
    let sim = match sim {
        Ok(f) => f,
        Err(why) => return Ok(Err(format!("redsim does not run it: {why}"))),
    };
    let game = match mode {
        Mode::Replay { file } => load(file)?.1,
        _ => match game()? {
            Ok(f) => f,
            Err(why) => return Ok(Err(format!("the game run fails: {why}"))),
        },
    };
    let r = first_diff(&game, &sim);
    if let (Mode::Record { file, how }, Ok(_)) = (mode, &r) {
        save(file, kind, design, run, margin, how, &game)?;
    }
    Ok(r)
}

/// The kinds of recorded runs: a study's exact gate (its `--compare`) or the circuit judge's
/// (`circuit trace-compare`: the kind's stimulus on the base build).
pub const KINDS: [&str; 5] = ["sorter-run", "smelter-run", "brewer-run", "composter-run", "circuit-trace"];

fn dispatch(kind: &str, pool: Option<&mctas::sim::Pool>, c: &crate::Circuit, margin: i32, run: &str, design: &Path, mode: &Mode) -> Result<std::result::Result<usize, String>> {
    let bad_run = || anyhow::anyhow!("{kind}: no run {run:?}");
    match kind {
        "sorter-run" => crate::sorter::compare_with(pool, c, margin, crate::sorter::Run::parse(run).ok_or_else(bad_run)?, design, mode),
        "smelter-run" => crate::smelter::compare_with(pool, c, margin, crate::smelter::Run::parse(run).ok_or_else(bad_run)?, design, mode),
        "brewer-run" => crate::brewer::compare_with(pool, c, margin, crate::brewer::Run::parse(run).ok_or_else(bad_run)?, design, mode),
        "composter-run" => crate::composter::compare_with(pool, c, margin, crate::composter::Run::parse(run).ok_or_else(bad_run)?, design, mode),
        "circuit-trace" => crate::verify::trace_compare_with(c, &crate::verify::Options { margin, ..Default::default() }, design, mode),
        k => bail!("unknown recording kind {k:?} (one of {})", KINDS.join(", ")),
    }
}

/// Record one design's run: the design frozen into `dir`, the run in the game and on redsim
/// (they must be identical: a recording is only ever made of an exact match), the game's frames
/// saved as `dir/<stem>-<run>.rec`. `pool`: the game (one worker is enough).
pub fn record(kind: &str, run: &str, design: &Path, dir: &Path, margin: i32, pool: &mctas::sim::Pool) -> Result<(PathBuf, std::result::Result<usize, String>)> {
    let copy = freeze(design, dir)?;
    let c = crate::Circuit::load(&copy)?;
    let stem = copy.file_stem().context("design stem")?.to_string_lossy().to_string();
    let file = dir.join(format!("{stem}-{run}.rec"));
    let how = format!("mctas circuit record --kind {kind} --run {run} --margin {margin} --dir {} {}", dir.display(), design.display());
    let r = dispatch(kind, Some(pool), &c, margin, run, &copy, &Mode::Record { file: file.clone(), how })?;
    Ok((file, r))
}

/// Replay one recording on redsim (no game): stale (its design or the game version changed)
/// is an error; otherwise the first difference or the frame count.
pub fn replay_one(file: &Path) -> Result<std::result::Result<usize, String>> {
    let (h, _) = load(file)?;
    let design = replay_check(file, &h)?;
    let c = crate::Circuit::load(&design)?;
    let margin = h["margin"].as_i64().unwrap_or(3) as i32;
    let run = h["run"].as_str().unwrap_or("base").to_string();
    dispatch(h["kind"].as_str().unwrap_or(""), None, &c, margin, &run, &design, &Mode::Replay { file: file.to_path_buf() })
}

/// Replay recordings (`.rec` files, or directories of them) on `jobs` threads: one line each in
/// order (`IDENTICAL: <file>, <frames> frames`, `DIVERGES: <file>: ...`, `STALE ...`), then
/// `N of M recordings identical`; Err unless all are.
pub fn replay_all(paths: &[PathBuf], jobs: usize) -> Result<()> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(p)?.flatten().map(|e| e.path()).filter(|q| q.extension().is_some_and(|e| e == "rec")).collect();
            v.sort();
            files.extend(v);
        } else {
            files.push(p.clone());
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: std::sync::Mutex<Vec<Option<(bool, String)>>> = std::sync::Mutex::new(files.iter().map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..jobs.clamp(1, files.len().max(1)) {
            s.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(f) = files.get(k) else { break };
                let line = match replay_one(f) {
                    Ok(Ok(n)) => (true, format!("IDENTICAL: {}, {n} frames", f.display())),
                    Ok(Err(why)) => (false, format!("DIVERGES: {}: {why}", f.display())),
                    Err(e) => (false, format!("{e:#}")),
                };
                out.lock().unwrap()[k] = Some(line);
            });
        }
    });
    let mut same = 0;
    for (ok, line) in out.into_inner().unwrap().into_iter().flatten() {
        same += ok as usize;
        println!("{line}");
    }
    println!("{same} of {} recordings identical", files.len());
    if same < files.len() {
        bail!("{} recordings differ or are stale", files.len() - same);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let dir = std::env::temp_dir().join(format!("circuit-recorded-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.join("d.txt");
        std::fs::write(&d, "name: x\n").unwrap();
        let frames: Vec<Frame> = vec![vec![([0, 0, 0], "a".into()), ([1, 0, 0], "b".into())], vec![([0, 0, 0], "a".into())], vec![([0, 0, 0], "c".into()), ([2, 0, 0], "d".into())]];
        let f = dir.join("r.rec");
        save(&f, "test", &d, "base", 3, "how", &frames).unwrap();
        let (h, back) = load(&f).unwrap();
        assert_eq!(back, frames);
        assert!(replay_check(&f, &h).is_ok());
        assert_eq!(first_diff(&frames, &back), Ok(3));
        std::fs::write(&d, "name: y\n").unwrap();
        assert!(replay_check(&f, &h).unwrap_err().to_string().starts_with("STALE"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
