//! The boat foundation's complete legacy gate, without shell loops or filename guesses.
use anyhow::{Context, Result, ensure};
use mctas::design::Design;
use std::path::{Path, PathBuf};
use std::process::Command;

fn flat_files(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for (n, entry) in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))?.enumerate() {
        ensure!(n < 10000, "refuse oversized flat directory {}", dir.display());
        let path = entry?.path();
        if path.extension().is_some_and(|x| x == extension) {
            ensure!(std::fs::metadata(&path)?.is_file(), "expected a file at {}", path.display());
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn archived_boats() -> Result<Vec<PathBuf>> {
    let paths = flat_files(Path::new("designs/clocks/archive"), "txt")?;
    let bytes: u64 = paths.iter().map(|p| std::fs::metadata(p).map(|m| m.len())).collect::<std::io::Result<Vec<_>>>()?.into_iter().sum();
    ensure!(bytes <= 32 * 1024 * 1024, "archive exceeds this tool's 32MiB scoped-read bound");
    println!("ARCHIVE INVENTORY: {} flat files, {bytes} bytes", paths.len());
    let mut boats = Vec::new();
    for path in paths {
        let d = Design::load(&path)?;
        if d.entities.iter().any(|e| e.kind.ends_with("_boat") || e.kind.ends_with("_raft")) { boats.push(path); }
    }
    println!("ARCHIVED BOAT/RAFT DESIGNS: {}", boats.len());
    ensure!(!boats.is_empty(), "no boat designs selected");
    Ok(boats)
}

fn run_to(binary: &Path, args: &[&str], paths: &[PathBuf], log: &Path) -> Result<String> {
    let output = Command::new(binary).args(args).args(paths).env("MCTAS_MAX_WORKERS", "1").output()?;
    let text = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    std::fs::write(log, &text)?;
    ensure!(output.status.success(), "{} {} failed; {}\n{}", binary.display(), args.join(" "), log.display(), text);
    println!("PASS {} {} (evidence {})", binary.display(), args.join(" "), log.display());
    Ok(text)
}

/// Both models replay all kept exact traces; inventory selects boats by entity content,
/// not a filename substring; every selected archived boat is re-judged in the game.
pub fn run(baseline: &Path, out: &Path, skip_boat_verify: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut traces = flat_files(Path::new("designs/clocks/traces/exact"), "trace")?;
    traces.extend(flat_files(Path::new("designs/clocks/tracks/clk-misc/redsim-tests/traces"), "trace")?);
    println!("KEPT EXACT TRACES: {}", traces.len());
    let current = std::env::current_exe()?;
    let fuzz = current.with_file_name("redsim-fuzz");
    let before = run_to(baseline, &["trace-compare"], &traces, &out.join("traces-before.txt"))?;
    let after = run_to(&fuzz, &["trace-compare"], &traces, &out.join("traces-after.txt"))?;
    ensure!(before == after, "legacy trace reports changed; compare before/after evidence");
    let boats = archived_boats()?;
    let list = boats.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::write(out.join("archived-boats.txt"), list)?;
    let before = run_to(baseline, &["archive-check", "--problems"], &boats, &out.join("archive-before.txt"))?;
    let after = run_to(&fuzz, &["archive-check", "--problems"], &boats, &out.join("archive-after.txt"))?;
    ensure!(before == after, "archived boat verdicts changed; compare before/after evidence");
    if skip_boat_verify {
        println!("GAME RE-VERIFICATION SKIPPED explicitly; this is NOT the complete legacy gate");
    } else {
        run_to(&current, &["clock", "verify", "--workers", "1"], &boats, &out.join("boat-game-verification.txt"))?;
        println!("LEGACY GATE: {} exact traces unchanged; {} archived boat/raft designs re-verified", traces.len(), boats.len());
    }
    Ok(())
}
