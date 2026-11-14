//! THE BUILD INPUTS and THE BUILD TREE, one definition with two consumers (ironspec Nov 14 05:40 (1)):
//! mobfarm's provenance check (the binary rule: a binary stamped with its build tree, refused unless it
//! is the cited commit's and the worktree's) and `mctas test` (a changed include target selects the
//! crates that include it; every binary the runner builds is stamped with the tested tree's build tree).
//!
//! THE BUILD INPUTS: every file the build reads: `crates/`, `Cargo.toml`, `Cargo.lock`, a toolchain file
//! if present, and every `include_str!` / `include_bytes!` target outside `crates/` (scanned from the
//! crates' sources: bridge/ agents, compiled-in designs such as designs/farms/prices.tsv). THE BUILD
//! TREE: git's tree id over those paths of a worktree AS IT STANDS (uncommitted edits and untracked files
//! git doesn't ignore included), through a scratch index (the worktree's own index is never touched); for
//! a commit, the tree of its entries at the same paths.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixed build inputs (paths from the repository root); [`build_inputs`] adds the scanned
/// include_str! / include_bytes! targets.
pub const FIXED_INPUTS: [&str; 5] = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml"];

/// `git -C dir args…` (with `GIT_INDEX_FILE` = `index` and `stdin` when given): its trimmed stdout.
pub fn git_in(dir: &Path, index: Option<&Path>, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    if let Some(i) = index {
        cmd.env("GIT_INDEX_FILE", i);
    }
    cmd.stdin(if stdin.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() });
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if let Some(s) = stdin {
        use std::io::Write;
        child.stdin.take().ok_or("no stdin")?.write_all(s.as_bytes()).map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

/// The worktree root containing `dir`.
pub fn toplevel(dir: &Path) -> Result<PathBuf, String> {
    git_in(dir, None, &["rev-parse", "--show-toplevel"], None).map(PathBuf::from).map_err(|_| format!("{} is not inside a git worktree", dir.display()))
}

fn scratch_index() -> PathBuf {
    std::env::temp_dir().join(format!(
        "mctas-provenance-{}-{}.index",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
    ))
}

/// Normalizes `a/b/../c` to `a/c` (repository-relative, '/'-separated); None if it climbs above the root.
pub fn normalize(p: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
}

/// Every literal include target named in `(file, line)` source lines, with the crates whose files name
/// it (`crates/<crate>/…`), as repository paths.
pub fn include_map<'a>(lines: impl Iterator<Item = (&'a str, &'a str)>) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (file, line) in lines {
        let Some(krate) = file.strip_prefix("crates/").and_then(|r| r.split_once('/')).map(|(k, _)| k).filter(|k| !k.is_empty()) else { continue };
        for mac in ["include_str!(\"", "include_bytes!(\""] {
            let mut rest = line;
            while let Some(i) = rest.find(mac) {
                rest = &rest[i + mac.len()..];
                let Some(j) = rest.find('"') else { break };
                let lit = &rest[..j];
                let dir = file.rsplit_once('/').map_or("", |d| d.0);
                if let Some(p) = normalize(&format!("{dir}/{lit}")) {
                    let ks = m.entry(p).or_default();
                    if !ks.iter().any(|k| k == krate) {
                        ks.push(krate.to_string());
                    }
                }
                rest = &rest[j..];
            }
        }
    }
    m
}

/// The crates' include lines of the worktree containing `dir` (`at` None) or of commit `at`, as
/// `(file, text)` (git grep over `crates/*.rs`).
pub fn include_lines(dir: &Path, at: Option<&str>) -> Result<Vec<(String, String)>, String> {
    let root = toplevel(dir)?;
    let grep = match at {
        None => git_in(&root, None, &["grep", "-n", "--untracked", "-E", r#"include_(str|bytes)!\(""#, "--", "crates/*.rs"], None),
        Some(c) => git_in(&root, None, &["grep", "-n", "-E", r#"include_(str|bytes)!\(""#, c, "--", "crates/*.rs"], None),
    }
    .or_else(|e| if e.contains("failed: ") && !e.contains("fatal") { Ok(String::new()) } else { Err(e) })?;
    Ok(grep
        .lines()
        .filter_map(|l| {
            let l = match at {
                Some(c) => l.strip_prefix(&format!("{c}:"))?,
                None => l,
            };
            let (file, rest) = l.split_once(':')?;
            let (_, text) = rest.split_once(':')?;
            Some((file.to_string(), text.to_string()))
        })
        .collect())
}

/// THE BUILD INPUTS of the worktree containing `dir` (`at` None) or of commit `at`: [`FIXED_INPUTS`]
/// present there plus every include target outside crates/ that its crates' sources name.
pub fn build_inputs(dir: &Path, at: Option<&str>) -> Result<Vec<String>, String> {
    let root = toplevel(dir)?;
    let lines = include_lines(dir, at)?;
    let mut inputs: Vec<String> = FIXED_INPUTS
        .iter()
        .filter(|p| match at {
            None => root.join(p).exists(),
            Some(c) => git_in(&root, None, &["cat-file", "-e", &format!("{c}:{p}")], None).is_ok(),
        })
        .map(|p| p.to_string())
        .collect();
    let mut outside: Vec<String> = include_map(lines.iter().map(|(f, t)| (f.as_str(), t.as_str()))).into_keys().filter(|p| !p.starts_with("crates/")).collect();
    outside.sort();
    inputs.extend(outside);
    Ok(inputs)
}

/// The BUILD TREE of the worktree containing `dir`, as it stands: `mctas mobfarm source-tree`'s value.
pub fn worktree_build_tree(dir: &Path) -> Result<String, String> {
    let root = toplevel(dir)?;
    let inputs = build_inputs(dir, None)?;
    let idx = scratch_index();
    let r = (|| {
        git_in(&root, Some(&idx), &["read-tree", "--empty"], None)?;
        let present: Vec<&str> = inputs.iter().map(|s| s.as_str()).filter(|q| root.join(q).exists()).collect();
        if !present.is_empty() {
            let mut args = vec!["add", "-A", "-f", "--"];
            args.extend(present);
            git_in(&root, Some(&idx), &args, None)?;
        }
        git_in(&root, Some(&idx), &["write-tree"], None)
    })();
    let _ = std::fs::remove_file(&idx);
    r
}

/// The BUILD TREE of commit `c`: the tree of its entries at its build inputs.
pub fn commit_build_tree(dir: &Path, c: &str) -> Result<String, String> {
    let root = toplevel(dir)?;
    let inputs = build_inputs(dir, Some(c))?;
    let mut args = vec!["ls-tree", "-r", c, "--"];
    args.extend(inputs.iter().map(|s| s.as_str()));
    let entries = git_in(&root, None, &args, None)?;
    let idx = scratch_index();
    let r = (|| {
        git_in(&root, Some(&idx), &["read-tree", "--empty"], None)?;
        if !entries.is_empty() {
            git_in(&root, Some(&idx), &["update-index", "--index-info"], Some(&format!("{entries}\n")))?;
        }
        git_in(&root, Some(&idx), &["write-tree"], None)
    })();
    let _ = std::fs::remove_file(&idx);
    r
}

/// Git's tree id of the whole worktree containing `dir`, as it stands (the run's inputs).
pub fn worktree_tree(dir: &Path) -> Result<String, String> {
    let root = toplevel(dir)?;
    let idx = scratch_index();
    let r = git_in(&root, Some(&idx), &["read-tree", "HEAD"], None).and_then(|_| git_in(&root, Some(&idx), &["add", "-A"], None)).and_then(|_| git_in(&root, Some(&idx), &["write-tree"], None));
    let _ = std::fs::remove_file(&idx);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_include_map_names_every_target_and_its_crates() {
        let lines = [
            ("crates/mobfarm/src/returnbench.rs", "let c = include_str!(\"../../../designs/farms/x.txt\");"),
            ("crates/mobfarm/src/a.rs", "const G: &[u8] = include_bytes!(\"./data/g.bin\"); let t = include_str!(\"../x.txt\");"),
            ("crates/circuit/src/b.rs", "include_str!(\"../../../designs/farms/x.txt\")"),
            ("README.md", "include_str!(\"nope\")"),
        ];
        let m = include_map(lines.iter().map(|(f, t)| (*f, *t)));
        assert_eq!(m.get("designs/farms/x.txt"), Some(&vec!["mobfarm".to_string(), "circuit".to_string()]));
        assert_eq!(m.get("crates/mobfarm/src/data/g.bin"), Some(&vec!["mobfarm".to_string()]));
        assert_eq!(m.get("crates/mobfarm/x.txt"), Some(&vec!["mobfarm".to_string()]));
        assert_eq!(m.len(), 3, "a line outside crates/ names no crate's include: {m:?}");
        assert_eq!(normalize("a/b/../c"), Some("a/c".to_string()));
        assert_eq!(normalize("../x"), None);
    }
}
