//! `mctas test --mutants <tsv> --tree <worktree> [--suite '<mctas args>']`: the mutation test of
//! the test pieces (2026-11-08: "introduce known bugs in redsim and show the simulator tests catch
//! each one, and how fast"). Each mutant is a one-line bug (a find/replace in one file of the
//! worktree); for each: the tree as it is must pass the suite first (the positive control, once),
//! then the mutant is applied, the tree built (`cargo build --release`), the suite run (by
//! default `mctas test --from HEAD`: the pieces the mutant's own diff picks), the FAIL lines and
//! the time of the first one recorded, and the file restored. The report is one line per mutant.
//!
//! The mutants file: `name<TAB>file<TAB>find<TAB>replace<TAB>what`, `\n` and `\t` escaped in
//! find/replace; several rows with the same name are one mutant (every edit applied). `find` must
//! occur exactly once in its file.

use anyhow::{Context, Result, bail, ensure};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

/// One mutant: its edits and what bug it is.
pub struct Mutant {
    pub name: String,
    pub edits: Vec<(String, String, String)>,
    pub what: String,
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

pub fn load(path: &Path) -> Result<Vec<Mutant>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut out: Vec<Mutant> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') || line.starts_with("name\t") {
            continue;
        }
        let t: Vec<&str> = line.split('\t').collect();
        ensure!(t.len() == 5, "{} line {}: {} columns, want name, file, find, replace, what", path.display(), i + 1, t.len());
        let edit = (t[1].to_string(), unescape(t[2]), unescape(t[3]));
        match out.iter_mut().find(|m| m.name == t[0]) {
            Some(m) => m.edits.push(edit),
            None => out.push(Mutant { name: t[0].into(), edits: vec![edit], what: t[4].into() }),
        }
    }
    Ok(out)
}

/// The outcome of one suite run.
pub struct Run {
    pub passed: bool,
    pub fails: Vec<String>,
    pub first_fail: Option<f64>,
    pub secs: f64,
    pub tail: String,
}

/// Run `mctas <suite>` in the tree, every stdout line timestamped.
fn suite(tree: &Path, args: &[String], log: &Path) -> Result<Run> {
    let exe = tree.join("target/release/mctas");
    let t0 = Instant::now();
    let mut child = Command::new(&exe).args(args).current_dir(tree).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().with_context(|| format!("run {}", exe.display()))?;
    let mut fails = Vec::new();
    let mut first_fail = None;
    let mut lines = Vec::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line?;
        let t = t0.elapsed().as_secs_f64();
        if line.starts_with("FAIL") {
            first_fail.get_or_insert(t);
            fails.push(line.split_whitespace().nth(1).unwrap_or("?").to_string());
        }
        lines.push(format!("[{t:7.1}] {line}"));
    }
    let status = child.wait()?;
    std::fs::write(log, lines.join("\n"))?;
    let tail = lines.iter().rev().take(2).rev().cloned().collect::<Vec<_>>().join(" | ");
    Ok(Run { passed: status.success(), fails, first_fail, secs: t0.elapsed().as_secs_f64(), tail })
}

fn build(tree: &Path) -> Result<f64> {
    let t0 = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let o = Command::new(cargo).args(["build", "--release"]).current_dir(tree).output().context("cargo build")?;
    if !o.status.success() {
        bail!("does not build: {}", String::from_utf8_lossy(&o.stderr).lines().filter(|l| l.starts_with("error")).take(2).collect::<Vec<_>>().join(" | "));
    }
    Ok(t0.elapsed().as_secs_f64())
}

fn apply(tree: &Path, m: &Mutant) -> Result<Vec<(PathBuf, String)>> {
    let mut saved: Vec<(PathBuf, String)> = Vec::new();
    for (file, find, replace) in &m.edits {
        let p = tree.join(file);
        let text = match saved.iter().find(|(q, _)| *q == p) {
            Some(_) => std::fs::read_to_string(&p)?,
            None => {
                let t = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
                saved.push((p.clone(), t.clone()));
                t
            }
        };
        let n = text.matches(find.as_str()).count();
        if n != 1 {
            restore(&saved)?;
            bail!("{}: `{}` occurs {n} times in {file} (want exactly once)", m.name, find.lines().next().unwrap_or(""));
        }
        std::fs::write(&p, text.replacen(find.as_str(), replace, 1))?;
    }
    Ok(saved)
}

fn restore(saved: &[(PathBuf, String)]) -> Result<()> {
    for (p, t) in saved {
        std::fs::write(p, t)?;
    }
    Ok(())
}

/// The whole experiment; the report goes to stdout and to `<tree>/target/mutants/report.txt`, the
/// suite's output per mutant beside it (under target/: untracked files would join the next
/// mutant's diff).
pub fn run(mutants_file: &Path, tree: &Path, suite_args: &[String], only: Option<&str>, skip_control: bool) -> Result<()> {
    let mutants: Vec<Mutant> = load(mutants_file)?.into_iter().filter(|m| only.is_none_or(|o| o.split(',').any(|q| m.name.contains(q)))).collect();
    ensure!(tree.join(".git").exists(), "--tree {}: not a git worktree", tree.display());
    let logs = tree.join("target/mutants");
    std::fs::create_dir_all(&logs)?;
    let mut report = vec![format!("# mutation test: suite `mctas {}` in {}", suite_args.join(" "), tree.display())];
    let say = |report: &mut Vec<String>, s: String| {
        println!("{s}");
        report.push(s);
    };
    let b = build(tree)?;
    if skip_control {
        say(&mut report, format!("control skipped (--no-control; build {b:.0} s)"));
    } else {
        // the positive control: the suite passes without a mutant. The default suite picks its
        // pieces from the mutant's diff (nothing without one): its control is every simulator piece
        let control_args: Vec<String> = if suite_args.join(" ") == "test --from HEAD" { ["test", "--pick", "sim", "--no-diff"].map(String::from).to_vec() } else { suite_args.to_vec() };
        let control = suite(tree, &control_args, &logs.join("control.log"))?;
        say(&mut report, format!("control (no mutant, `mctas {}`): {} in {:.0} s (build {b:.0} s){}", control_args.join(" "), if control.passed { "PASS" } else { "FAIL" }, control.secs, if control.passed { String::new() } else { format!(": {}", control.tail) }));
        ensure!(control.passed, "the suite fails without a mutant: a caught mutant would mean nothing (see {})", logs.join("control.log").display());
    }
    for m in &mutants {
        let saved = match apply(tree, m) {
            Ok(s) => s,
            Err(e) => {
                say(&mut report, format!("{}\tNOT APPLIED\t{e:#}", m.name));
                continue;
            }
        };
        let line = match build(tree) {
            Err(e) => format!("{}\tNOT BUILT\t{e:#}", m.name),
            Ok(bs) => {
                let r = suite(tree, suite_args, &logs.join(format!("{}.log", m.name)))?;
                let mut fails = r.fails.clone();
                fails.dedup();
                if r.passed {
                    format!("{}\tMISSED\tbuild {bs:.0} s, suite {:.0} s\t{}", m.name, r.secs, m.what)
                } else {
                    format!("{}\tCAUGHT\tfirst FAIL at {:.1} s (build {bs:.0} s, suite {:.0} s) by {} piece(s): {}\t{}", m.name, r.first_fail.unwrap_or(r.secs), r.secs, fails.len(), fails.join(", "), m.what)
                }
            }
        };
        restore(&saved)?;
        say(&mut report, line);
    }
    let clean = Command::new("git").args(["diff", "--quiet", "--", "crates"]).current_dir(tree).status()?.success();
    say(&mut report, format!("tree restored: {}", if clean { "yes (git diff clean)" } else { "NO: git diff shows changes" }));
    std::fs::write(logs.join("report.txt"), report.join("\n") + "\n")?;
    // the tree's binary back to the unmutated code
    let _ = build(tree);
    Ok(())
}
