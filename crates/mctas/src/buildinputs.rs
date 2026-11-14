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
/// include_str! / include_bytes! targets and the declared include roots. `.cargo` (config: rustflags,
/// features) is one (G1 Nov 14 07:53 (A)(3)).
pub const FIXED_INPUTS: [&str; 6] = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml", ".cargo"];

/// THE DECLARED INCLUDES (G1 Nov 14 07:53 (A)(2), the scan DEFAULT-DENY): an include the literal scan can't
/// resolve must be declared here, ONCE, with the repository prefix it can read (a build input) and why,
/// or the scan fails by name. (file, the line's include text, the prefix it reads, why)
pub const DECLARED_INCLUDES: &[(&str, &str, &str, &str)] = &[
    ("crates/mctas/src/bridge.rs", "include_str!(concat!(\"../../../bridge/src/dev/vjeux/mctas/bridge/\", $name))", "bridge/src/dev/vjeux/mctas/bridge", "the bridge's Java sources, one per the macro's literal file-name argument"),
    ("crates/circuit/src/lib.rs", "include!(concat!(env!(\"OUT_DIR\"), \"/studies.rs\"))", "", "the circuit build script's generated module list: a build output, its inputs the script's (crates/circuit/src/studies, inside crates/)"),
    ("crates/redsim/src/redstone/container.rs", "include!(concat!(env!(\"OUT_DIR\"), \"/module_items.rs\"))", "", "redsim's build script's generated item table: a build output, its inputs the script's (inside crates/redsim)"),
];

/// Whether byte `i` of `line` sits inside a string literal (an odd count of unescaped `"` before it).
fn in_string(line: &str, i: usize) -> bool {
    let b = line.as_bytes();
    let mut n = 0usize;
    let mut k = 0usize;
    while k < i && k < b.len() {
        if b[k] == b'\\' {
            k += 2;
            continue;
        }
        if b[k] == b'"' {
            n += 1;
        }
        k += 1;
    }
    n % 2 == 1
}

/// The include-family macros and module paths in `(file, line)` source lines that the literal scan
/// ([`include_map`]) does NOT resolve and [`DECLARED_INCLUDES`] doesn't declare, as `file: text` (G1 Nov 14
/// 07:53 (A)(2): `include!`, a space before `(`, `concat!` / `env!` arguments, `#[path = …]` modules; a raw
/// string `r"…"` / `r#"…"#` resolves). A comment line, and a mention inside a string literal (an escaped
/// quote follows the paren), aren't includes.
pub fn unresolved_includes<'a>(lines: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    let mut bad = Vec::new();
    for (file, line) in lines {
        let t = line.trim_start();
        if t.starts_with("//") || !file.ends_with(".rs") {
            continue;
        }
        if let Some(r) = t.strip_prefix("#[path") {
            // (a literal module path that stays inside crates/ is a build input already)
            let lit = r.trim_start().strip_prefix('=').map(str::trim_start).and_then(literal_at);
            let dir = file.rsplit_once('/').map_or("", |d| d.0);
            let inside = lit.and_then(|l| normalize(&format!("{dir}/{l}"))).is_some_and(|p| p.starts_with("crates/"));
            if !inside {
                bad.push(format!("{file}: {} (a `#[path]` module outside crates/ or not a literal)", t.trim()));
            }
            continue;
        }
        for mac in ["include_str!", "include_bytes!", "include!"] {
            let mut from = 0usize;
            while let Some(off) = line[from..].find(mac) {
                let i = from + off;
                from = i + mac.len();
                let before = &line[..i];
                // (`include!` inside `include_str!` / `include_bytes!`' own text isn't another macro)
                if mac == "include!" && (before.ends_with("_str") || before.ends_with("_bytes")) || before.ends_with("include_") {
                    continue;
                }
                // (a mention inside a string literal, or one not followed by `(`, isn't an invocation)
                let rest = &line[from..];
                if in_string(line, i) || !rest.trim_start().starts_with('(') {
                    continue;
                }
                let resolved = mac != "include!" && (rest.starts_with("(\"") || rest.starts_with("(r\"") || rest.starts_with("(r#\""));
                let declared = DECLARED_INCLUDES.iter().any(|d| d.0 == file && line.contains(d.1));
                if !resolved && !declared {
                    bad.push(format!("{file}: {}", t.trim()));
                }
            }
        }
    }
    bad
}

/// The literal of a raw or plain string at the start of `rest` (`"…"`, `r"…"`, `r#"…"#`).
fn literal_at(rest: &str) -> Option<&str> {
    if let Some(r) = rest.strip_prefix("r#\"") {
        return r.find("\"#").map(|j| &r[..j]);
    }
    let r = rest.strip_prefix("r\"").or_else(|| rest.strip_prefix('"'))?;
    r.find('"').map(|j| &r[..j])
}

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
        for mac in ["include_str!(", "include_bytes!("] {
            let mut rest = line;
            while let Some(i) = rest.find(mac) {
                rest = &rest[i + mac.len()..];
                // (a plain or raw string literal: `"…"`, `r"…"`, `r#"…"#`; anything else is
                // `unresolved_includes`' to refuse)
                let Some(lit) = literal_at(rest) else { continue };
                let dir = file.rsplit_once('/').map_or("", |d| d.0);
                if let Some(p) = normalize(&format!("{dir}/{lit}")) {
                    let ks = m.entry(p).or_default();
                    if !ks.iter().any(|k| k == krate) {
                        ks.push(krate.to_string());
                    }
                }
            }
        }
    }
    m
}

/// The build scripts' file reads, DECLARED or denied (G1 Nov 14 07:53 (A)(3)): from `(file, line)` lines of
/// `build.rs` files, each script that reads files (`fs::read*`, `read_dir`, `File::open`) must declare them
/// (`cargo:rerun-if-changed`), and none may name an absolute or home path literal (every build input is
/// under the repository's build inputs: today all under crates/); the violations, by file.
pub fn build_script_violations<'a>(lines: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    let mut reads: BTreeMap<&str, bool> = BTreeMap::new();
    let mut declares: BTreeMap<&str, bool> = BTreeMap::new();
    let mut bad = Vec::new();
    for (file, line) in lines {
        let t = line.trim_start();
        if t.starts_with("//") {
            continue;
        }
        for call in ["fs::read(", "fs::read_to_string(", "read_dir(", "File::open("] {
            if let Some(j) = line.find(call) {
                reads.insert(file, true);
                let rest = &line[j + call.len()..];
                if rest.starts_with("\"/") || rest.starts_with("\"~") {
                    bad.push(format!("{file}: {} (an absolute or home path)", t.trim()));
                }
            }
        }
        if line.contains("cargo:rerun-if-changed") {
            declares.insert(file, true);
        }
    }
    for f in reads.keys() {
        if !declares.contains_key(f) {
            bad.push(format!("{f}: reads files but declares none (`cargo:rerun-if-changed`)"));
        }
    }
    bad
}

/// The crates' include lines of the worktree containing `dir` (`at` None) or of commit `at`, as
/// `(file, text)` (git grep over `crates/*.rs`): every line naming an include-family macro or starting a
/// `#[path]` attribute (the literal ones for [`include_map`], the rest for [`unresolved_includes`]).
pub fn include_lines(dir: &Path, at: Option<&str>) -> Result<Vec<(String, String)>, String> {
    let root = toplevel(dir)?;
    let re = r#"include(_str|_bytes)?!|^[[:space:]]*#\[path"#;
    let grep = match at {
        None => git_in(&root, None, &["grep", "-n", "--untracked", "-E", re, "--", "crates/*.rs"], None),
        Some(c) => git_in(&root, None, &["grep", "-n", "-E", re, c, "--", "crates/*.rs"], None),
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
    // (G1 Nov 14 07:53 (A)(2): DEFAULT-DENY: an include the scan can't resolve and nobody declared fails
    // the build inputs by name, so the stamp and the check refuse rather than miss an input)
    let bad = unresolved_includes(lines.iter().map(|(f, t)| (f.as_str(), t.as_str())));
    if !bad.is_empty() {
        return Err(format!("the build inputs' include scan can't resolve {} include(s): {} (write a literal path, or declare it once in mctas::buildinputs::DECLARED_INCLUDES)", bad.len(), bad.join("; ")));
    }
    // ((A)(3): the build scripts' reads declared or denied)
    let re = r#"fs::read|read_dir\(|File::open\(|cargo:rerun-if-changed"#;
    let scripts = match at {
        None => git_in(&root, None, &["grep", "-n", "--untracked", "-E", re, "--", "crates/*/build.rs", "crates/*/src/build.rs"], None),
        Some(c) => git_in(&root, None, &["grep", "-n", "-E", re, c, "--", "crates/*/build.rs", "crates/*/src/build.rs"], None),
    }
    .or_else(|e| if e.contains("failed: ") && !e.contains("fatal") { Ok(String::new()) } else { Err(e) })?;
    let script_lines: Vec<(String, String)> = scripts
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
        .collect();
    let sbad = build_script_violations(script_lines.iter().map(|(f, t)| (f.as_str(), t.as_str())));
    if !sbad.is_empty() {
        return Err(format!("the build scripts' reads aren't declared (G1 Nov 14 07:53 (A)(3)): {}", sbad.join("; ")));
    }
    let mut outside: Vec<String> = include_map(lines.iter().map(|(f, t)| (f.as_str(), t.as_str()))).into_keys().filter(|p| !p.starts_with("crates/")).collect();
    outside.extend(DECLARED_INCLUDES.iter().filter(|d| !d.2.is_empty() && !d.2.starts_with("crates/") && lines.iter().any(|(f, t)| f == d.0 && t.contains(d.1))).map(|d| d.2.to_string()));
    outside.sort();
    outside.dedup();
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

    /// G1 Nov 14 07:53 (A)(2): the scan is DEFAULT-DENY: each include form the literal scan can't resolve
    /// fails by name (a test per form); raw strings resolve; a comment, a mention inside a string literal
    /// and a declared include don't fail.
    #[test]
    fn the_include_scan_refuses_every_form_it_cannot_resolve() {
        let one = |line: &str| unresolved_includes(std::iter::once(("crates/x/src/a.rs", line)));
        assert_eq!(one("const A: &str = include!(\"gen.rs\");").len(), 1, "include!");
        assert_eq!(one("const A: &str = include_str! (\"../x.txt\");").len(), 1, "a space before (");
        assert_eq!(one("const A: &str = include_str!(concat!(\"../\", \"x.txt\"));").len(), 1, "concat!");
        assert_eq!(one("const A: &str = include_str!(env!(\"X\"));").len(), 1, "env!");
        assert_eq!(one("#[path = \"../../../tools/other.rs\"] mod other;").len(), 1, "a #[path] outside crates/");
        assert!(one("#[path = \"simulate/preview.rs\"] mod preview;").is_empty(), "a #[path] inside crates/ is a build input already");
        assert!(one("const A: &str = include_str!(\"../x.txt\");").is_empty(), "a literal resolves");
        assert!(one("const A: &str = include_str!(r\"../x.txt\");").is_empty(), "a raw string resolves");
        assert!(one("const A: &str = include_bytes!(r#\"../x.bin\"#);").is_empty(), "a raw string with hashes resolves");
        assert!(one("    // include!(\"anything\") in a comment").is_empty(), "a comment");
        assert!(one("let s = \"include_str!(\\\"x\\\")\";").is_empty(), "a mention inside a string literal");
        let m = include_map([("crates/x/src/a.rs", "const A: &[u8] = include_bytes!(r#\"../../../data/y.bin\"#);")].into_iter());
        assert!(m.contains_key("data/y.bin"), "the raw literal is a build input: {m:?}");
        for d in DECLARED_INCLUDES {
            assert!(unresolved_includes(std::iter::once((d.0, d.1))).is_empty(), "the declared include {} {} resolves", d.0, d.1);
        }
        assert!(!unresolved_includes(std::iter::once(("crates/other/src/b.rs", DECLARED_INCLUDES[0].1))).is_empty(), "a declaration holds for its own file only");
    }

    /// G1 Nov 14 07:53 (A)(3): a build script that reads files must declare them; an absolute or home path
    /// literal is denied; the repository's own build scripts pass.
    #[test]
    fn a_build_scripts_reads_are_declared_or_denied() {
        let v = |ls: &[(&str, &str)]| build_script_violations(ls.iter().map(|(f, t)| (*f, *t)));
        assert!(v(&[("crates/a/build.rs", "let t = std::fs::read_to_string(p);"), ("crates/a/build.rs", "println!(\"cargo:rerun-if-changed={}\", p.display());")]).is_empty());
        assert_eq!(v(&[("crates/a/build.rs", "let t = std::fs::read_to_string(p);")]).len(), 1, "a read with no declaration");
        assert_eq!(v(&[("crates/a/build.rs", "let t = std::fs::read_to_string(\"/etc/x\"); println!(\"cargo:rerun-if-changed=/etc/x\");")]).len(), 1, "an absolute path");
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(build_inputs(here, None).is_ok(), "the repository's own build inputs resolve: {:?}", build_inputs(here, None));
    }
}
