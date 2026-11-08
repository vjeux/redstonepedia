//! Restore exact traces' deleted source designs as immutable, non-published fixtures.
//! Never substitute a stripped core: even inert removed cells invalidate an exact trace.

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Restore {
    trace: PathBuf,
    old: String,
    destination: PathBuf,
    source: Vec<u8>,
    revision: String,
    text: String,
}

fn git(args: &[&str]) -> Result<Vec<u8>> {
    let result = Command::new("git").args(args).output()?;
    ensure!(result.status.success(), "git {}: {}", args.join(" "), String::from_utf8_lossy(&result.stderr));
    Ok(result.stdout)
}

/// Change precisely the JSON string value of `design`, preserving all other header bytes
/// and EVERY byte after the first newline. Refuse an unexpected representation.
fn repoint(text: &str, path: &Path) -> Result<String> {
    let (header, tail) = text.split_once('\n').context("trace without a header newline")?;
    let v: Value = serde_json::from_str(header)?;
    let old = v["design"].as_str().context("header without design")?;
    let literal = serde_json::to_string(old)?;
    let key = header.find("\"design\"").context("design key spelling")? + "\"design\"".len();
    let colon = header[key..].find(':').context("design colon")? + key + 1;
    let start = colon + header[colon..].len() - header[colon..].trim_start().len();
    ensure!(header[start..].starts_with(&literal), "unexpected design field spelling");
    let replacement = serde_json::to_string(&path.to_string_lossy())?;
    let new = format!("{}{}{}\n{}", &header[..start], replacement, &header[start + literal.len()..], tail);
    ensure!(new.split_once('\n').unwrap().1 == tail, "tick records changed");
    Ok(new)
}

fn plan(roots: &[PathBuf], fixtures: &Path) -> Result<Vec<Restore>> {
    let mut paths = Vec::new();
    for dir in roots {
        // Explicit, flat test directories only. No checkout walk or recursive scan.
        for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "trace") { paths.push(path); }
        }
    }
    paths.sort();
    let mut restores = Vec::new();
    for trace in paths {
        let text = std::fs::read_to_string(&trace)?;
        let header: Value = serde_json::from_str(text.lines().next().context("empty trace")?)?;
        let old = header["design"].as_str().context("missing design")?.to_string();
        if Path::new(&old).is_file() { continue; }
        let deleted = String::from_utf8(git(&["log", "-1", "--diff-filter=D", "--format=%H", "--", &old])?)?.trim().to_string();
        ensure!(!deleted.is_empty(), "{}: no deletion history for {old}", trace.display());
        let revision = format!("{deleted}^:{old}");
        let source = git(&["show", &revision])?;
        let destination = fixtures.join(trace.file_stem().context("trace stem")?).with_extension("txt");
        ensure!(!destination.exists(), "refuse to overwrite {}", destination.display());
        restores.push(Restore { trace, old, destination, source, revision, text });
    }
    Ok(restores)
}

pub fn run(roots: &[PathBuf], fixtures: &Path, binary: &Path, write: bool) -> Result<()> {
    let safe_root = Path::new("designs/clocks/traces").canonicalize()?;
    let destination = if fixtures.exists() {
        fixtures.canonicalize()?
    } else {
        fixtures.parent().context("fixture parent")?.canonicalize()?.join(fixtures.file_name().context("fixture directory name")?)
    };
    ensure!(destination.starts_with(safe_root), "fixtures must stay under designs/clocks/traces, outside all published archives");
    let list = plan(roots, fixtures)?;
    println!("MISSING DESIGNS: {}", list.len());
    if list.is_empty() { return Ok(()); }
    let scratch = std::env::temp_dir().join(format!("mctas-trace-fixtures-{}", std::process::id()));
    std::fs::create_dir_all(&scratch)?;
    let mut traces = Vec::new();
    for (i, r) in list.iter().enumerate() {
        let source = scratch.join(format!("{i}.txt"));
        std::fs::write(&source, &r.source)?;
        let trace = scratch.join(format!("{i}.trace"));
        std::fs::write(&trace, repoint(&r.text, &source)?)?;
        traces.push(trace);
        println!("RESTORE {} from {}", r.destination.display(), r.revision);
    }
    let result = Command::new(binary).arg("trace-compare").args(&traces).output().with_context(|| format!("run {}", binary.display()))?;
    let report = format!("{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    std::fs::write(scratch.join("comparison.txt"), &report)?;
    print!("{report}");
    if !result.status.success() { bail!("historical fixtures did not all match; no kept traces changed. Evidence: {}", scratch.display()); }
    if !write {
        println!("DRY RUN: all originals exact; --write installs fixtures and changes only header paths");
        return Ok(());
    }
    std::fs::create_dir_all(fixtures)?;
    let provenance_path = fixtures.join("sources.tsv");
    let mut provenance = match std::fs::read_to_string(&provenance_path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::from("trace\told_design\tfixture\tgit_source\n"),
        Err(e) => return Err(e.into()),
    };
    ensure!(provenance.ends_with('\n'), "malformed existing provenance table");
    for r in list {
        std::fs::write(&r.destination, &r.source)?;
        ensure!(std::fs::read(&r.destination)? == r.source, "restored bytes changed");
        std::fs::write(&r.trace, repoint(&r.text, &r.destination)?)?;
        provenance.push_str(&format!("{}\t{}\t{}\t{}\n", r.trace.display(), r.old, r.destination.display(), r.revision));
    }
    std::fs::write(fixtures.join("sources.tsv"), provenance)?;
    println!("RESTORED: original bytes, tick data untouched, no archive files written");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_design_string_changes() {
        let old = "{\"z\": 2, \"design\" : \"a/b.txt\", \"name\":\"x\"}\n{\"t\":0,\"motion\":[-0.0,1e-9,2]}\n";
        let new = repoint(old, Path::new("fixtures/b.txt")).unwrap();
        assert_eq!(new, "{\"z\": 2, \"design\" : \"fixtures/b.txt\", \"name\":\"x\"}\n{\"t\":0,\"motion\":[-0.0,1e-9,2]}\n");
    }
}
