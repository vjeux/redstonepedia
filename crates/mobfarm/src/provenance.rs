//! Binary provenance (ironspec Nov 14 01:58, the standing binary rule; 02:35 (5)-(6) one shared tool;
//! tick-exact 02:41; G1 02:44b; ironspec 02:43 (4)): every evidence run prints the BUILD-INPUT tree its
//! binary was built from and refuses unless that is the cited commit's build inputs, the worktree's
//! current build inputs, and the worktree is the cited commit (design files read at RUN time are the
//! run's inputs: they must be the cited commit's).
//!
//! THE BUILD INPUTS (one definition, [`build_inputs`]): every file the build reads: `crates/`,
//! `Cargo.toml`, `Cargo.lock`, a toolchain file if present, and every `include_str!` / `include_bytes!`
//! target outside `crates/` (scanned from the crates' sources: bridge/ agents, compiled-in designs such as
//! designs/farms/prices.tsv). THE BUILD TREE: git's tree id over those paths of a worktree AS IT STANDS
//! (uncommitted edits and untracked files git doesn't ignore included), through a scratch index (the
//! worktree's own index is never touched); for a commit, the tree of its entries at the same paths. A
//! build stamps it: `MCTAS_SOURCE_TREE=$(mctas mobfarm source-tree) cargo build --release`; mobfarm's
//! build script declares `rerun-if-env-changed=MCTAS_SOURCE_TREE` and the crate reads it through
//! option_env!, so a rebuild with a new value re-embeds it (tested), and every subcommand of the one mctas
//! binary sees the same stamp through [`BUILT_FROM`].
//!
//! THE CHECK ([`check`]): the cited commit (the argument, else HEAD only when the worktree is clean)
//! passes when (1) the stamp is the worktree's current build tree (a stale or missing stamp refuses),
//! (2) the stamp is the cited commit's build tree, and (3) the worktree's whole tree is the cited
//! commit's (the run's inputs). `--not-evidence` turns a refusal into a NOT EVIDENCE header that the
//! runner stamps on every report line ([`label`]), so such a run can't produce a citable verdict.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The build tree the binary was built from, as stamped at build time (None: an unstamped build).
pub const BUILT_FROM: Option<&str> = option_env!("MCTAS_SOURCE_TREE");

// (THE BUILD INPUTS and THE BUILD TREE live in the shared `mctas::buildinputs`, one definition with
// two consumers: this check and `mctas test`'s selection and stamping, ironspec Nov 14 05:40 (1))
pub use mctas::buildinputs::{FIXED_INPUTS, build_inputs, commit_build_tree, worktree_build_tree, worktree_tree};
use mctas::buildinputs::{git_in, toplevel};

/// The output flags whose values name files the run WRITES (never an input): skipped by
/// [`evidence_inputs`].
pub const OUTPUT_FLAGS: &[&str] = &["--out", "--output"];

/// THE EVIDENCE INPUTS (G1 Nov 14 07:53 (A)(1), ironspec 06:43 / 06:54: one loader, default-deny): every
/// command-line argument of an evidence run that names an existing file (a whole token, a `--flag=VALUE`'s
/// value, or a part of a comma / semicolon list) must be inside the worktree, tracked and not ignored;
/// each is returned with its blob (the header lists them). One outside, untracked or ignored refuses the
/// run by name. (Reads the run makes of repository paths it names itself are inside the worktree by
/// construction and covered by the worktree-tree check; a source scan pins that no evidence crate reads
/// an absolute or home path.)
pub fn evidence_inputs(dir: &Path, args: &[String]) -> Result<Vec<(String, String)>, String> {
    let root = mctas::buildinputs::toplevel(dir)?;
    let croot = root.canonicalize().map_err(|e| format!("{}: {e}", root.display()))?;
    let mut ok: Vec<(String, String)> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    let mut skip_next = false;
    for a in args.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if OUTPUT_FLAGS.contains(&a.as_str()) {
            skip_next = true;
            continue;
        }
        if OUTPUT_FLAGS.iter().any(|f| a.starts_with(&format!("{f}="))) {
            continue;
        }
        let mut cands: Vec<&str> = vec![a.as_str()];
        if let Some((_, v)) = a.strip_prefix("--").and_then(|r| r.split_once('=')) {
            cands.push(v);
        }
        let parts: Vec<&str> = cands.iter().flat_map(|c| c.split([',', ';'])).collect();
        cands.extend(parts);
        for c in cands {
            let p = dir.join(c);
            if c.is_empty() || !p.is_file() {
                continue;
            }
            let cp = match p.canonicalize() {
                Ok(cp) => cp,
                Err(e) => {
                    bad.push(format!("{c} ({e})"));
                    continue;
                }
            };
            let Ok(rel) = cp.strip_prefix(&croot) else {
                bad.push(format!("{c}: outside the worktree {}", croot.display()));
                continue;
            };
            let rel = rel.to_string_lossy().to_string();
            if ok.iter().any(|(r, _)| *r == rel) {
                continue;
            }
            if mctas::buildinputs::git_in(&root, None, &["check-ignore", "-q", "--", &rel], None).is_ok() {
                bad.push(format!("{c}: ignored by git ({rel})"));
                continue;
            }
            if mctas::buildinputs::git_in(&root, None, &["ls-files", "--error-unmatch", "--", &rel], None).is_err() {
                bad.push(format!("{c}: untracked ({rel})"));
                continue;
            }
            match mctas::buildinputs::git_in(&root, None, &["hash-object", "--", &rel], None) {
                Ok(b) => ok.push((rel, b)),
                Err(e) => bad.push(format!("{c}: {e}")),
            }
        }
    }
    if bad.is_empty() { Ok(ok) } else { Err(format!("REFUSED: an evidence input outside the worktree, untracked or ignored (G1 Nov 14 07:53 (A)(1)): {}", bad.join("; "))) }
}

/// The provenance header for a run in `dir` citing commit `cite` (None: HEAD, only when clean), or the
/// refusal naming every tree. `Ok((header, citable))`: `citable` false under `--not-evidence`.
pub fn check(dir: &Path, cite: Option<&str>, not_evidence: bool) -> Result<(String, bool), String> {
    check_with(BUILT_FROM, dir, cite, not_evidence)
}

/// [`check`] with the stamp given (the stamp is fixed at build time; tests inject one).
pub fn check_with(built: Option<&str>, dir: &Path, cite: Option<&str>, not_evidence: bool) -> Result<(String, bool), String> {
    let root = toplevel(dir)?;
    let here_full = worktree_tree(dir)?;
    let here_build = worktree_build_tree(dir)?;
    let inputs = build_inputs(dir, None)?;
    let head_full = git_in(dir, None, &["rev-parse", "HEAD^{tree}"], None).ok();
    let cited: Result<String, String> = match cite {
        Some(c) => git_in(dir, None, &["rev-parse", &format!("{c}^{{commit}}")], None),
        None if head_full.as_deref() == Some(here_full.as_str()) => git_in(dir, None, &["rev-parse", "HEAD"], None),
        None => Err(format!(
            "the worktree at {} is DIRTY (its tree {here_full}, HEAD's {}) and no commit is cited",
            root.display(),
            head_full.clone().unwrap_or_else(|| "-".to_string())
        )),
    };
    let stamp = built.map_or("an UNSTAMPED binary".to_string(), |b| format!("build tree {b}"));
    let verdict: Result<String, String> = cited.and_then(|cc| {
        let c_build = commit_build_tree(dir, &cc)?;
        let c_full = git_in(dir, None, &["rev-parse", &format!("{cc}^{{tree}}")], None)?;
        if built.is_none() {
            return Err(format!("{stamp}; the worktree's build tree is {here_build}"));
        }
        if built != Some(here_build.as_str()) {
            return Err(format!("the binary was built from {stamp}, but the worktree's current build tree is {here_build} (a STALE stamp or dirty build inputs)"));
        }
        if built != Some(c_build.as_str()) {
            return Err(format!("the binary was built from {stamp}, not the cited commit {cc}'s build tree {c_build}"));
        }
        if here_full != c_full {
            return Err(format!("the binary's build inputs are the cited commit {cc}'s (build tree {c_build}), but the worktree at {} (the run's inputs) is tree {here_full}, not the commit's {c_full}", root.display()));
        }
        Ok(format!("the binary was built from build tree {c_build}, the cited commit {cc}'s; the worktree at {} is that commit's tree {c_full}", root.display()))
    });
    let paths = format!("the build inputs: {} paths ({}{})", inputs.len(), inputs.iter().take(6).cloned().collect::<Vec<_>>().join(", "), if inputs.len() > 6 { ", ..." } else { "" });
    match verdict {
        Ok(line) => Ok((format!("provenance (ironspec Nov 14 01:58 / 02:35 / 02:43): {line}; {paths}"), true)),
        Err(why) if not_evidence => Ok((format!("NOT EVIDENCE (--not-evidence): provenance (ironspec Nov 14 01:58 / 02:35 / 02:43): {why}; {paths}"), false)),
        Err(why) => Err(format!(
            "provenance (ironspec Nov 14 01:58 / 02:35 / 02:43): REFUSED: {why}; {paths}; rebuild from the cited commit's clean worktree (MCTAS_SOURCE_TREE=$(mctas mobfarm source-tree) cargo build --release), or pass --not-evidence"
        )),
    }
}

/// The label for a report line (G1 02:44b): a line of a run under `--not-evidence` carries the mark.
pub fn label(citable: bool, line: &str) -> String {
    if citable { line.to_string() } else { format!("[NOT EVIDENCE] {line}") }
}

/// THE EXIT-CODE TABLE for every evidence subcommand (ironspec Nov 14 02:57 (4); owned here, reviewed by
/// G1): each lane maps onto it, and the code prints WITH ITS NAME on the last line ([`finish`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Every judged bar passed.
    Pass = 0,
    /// A judged bar failed (named in the output).
    Fail = 1,
    /// No bar failed, but a run couldn't be modeled.
    NotModeled = 2,
    /// `--not-evidence`: verdict lines print, nothing cites.
    NotEvidence = 3,
    /// The provenance check failed before any run (UNSTAMPED, STALE, another commit, DIRTY, run inputs
    /// off the commit).
    Refused = 4,
}

impl Exit {
    pub fn name(self) -> &'static str {
        match self {
            Exit::Pass => "PASS",
            Exit::Fail => "FAIL",
            Exit::NotModeled => "NOT MODELED",
            Exit::NotEvidence => "NOT EVIDENCE",
            Exit::Refused => "REFUSED",
        }
    }
    /// The last line an evidence run prints.
    pub fn last_line(self) -> String {
        format!("exit {} ({}) (the evidence exit-code table, ironspec Nov 14 02:57)", self as i32, self.name())
    }
}

/// Prints the exit code's last line and exits with it.
pub fn finish(code: Exit) -> ! {
    println!("{}", code.last_line());
    std::process::exit(code as i32)
}

/// THE EVIDENCE SUBCOMMANDS (the binary rule's one list; G1 02:44b / 02:59: DEFAULT-DENY): every
/// subcommand of the evidence families (mobfarm, mobai) runs the provenance check at mctas's dispatch
/// unless it is on [`NON_EVIDENCE`] (the tool's own printer and pure printers); mctas-cli's unit test
/// enumerates every subcommand of both families and asserts each is checked or listed.
pub const NON_EVIDENCE: [(&str, &str); 1] = [("mobfarm", "source-tree")];

pub fn is_evidence(family: &str, name: &str) -> bool {
    matches!(family, "mobfarm" | "mobai") && !NON_EVIDENCE.contains(&(family, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch repository: crates/x/src/a.rs including ../../../bridge/agent.txt, designs/d.txt;
    /// committed; returns its path.
    fn scratch_repo(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mctas-prov-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for sub in ["crates/x/src", "designs", "bridge"] {
            std::fs::create_dir_all(d.join(sub)).unwrap();
        }
        std::fs::write(d.join("crates/x/src/a.rs"), "const A: &str = include_str!(\"../../../bridge/agent.txt\");\n").unwrap();
        std::fs::write(d.join("bridge/agent.txt"), "an agent\n").unwrap();
        std::fs::write(d.join("Cargo.toml"), "[workspace]\n").unwrap();
        std::fs::write(d.join("designs/d.txt"), "a design\n").unwrap();
        let g = |args: &[&str]| git_in(&d, None, args, None).unwrap();
        g(&["init", "-q"]);
        g(&["add", "-A"]);
        g(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "c"]);
        d
    }

    fn commit_all(d: &Path, msg: &str) {
        git_in(d, None, &["add", "-A"], None).unwrap();
        git_in(d, None, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", msg], None).unwrap();
    }

    /// G1 Nov 14 07:53 (A)(1): the evidence inputs: a tracked file passes with its blob; an out-of-tree, an
    /// untracked and an ignored one each refuse by name; an output flag's value is skipped.
    #[test]
    fn an_evidence_input_outside_untracked_or_ignored_is_refused_by_name() {
        let d = scratch_repo("evin");
        std::fs::write(d.join(".gitignore"), "*.log\n").unwrap();
        commit_all(&d, "ignore logs");
        let a = |v: &[&str]| -> Vec<String> { std::iter::once("mctas").chain(v.iter().copied()).map(|s| s.to_string()).collect() };
        let ok = evidence_inputs(&d, &a(&["mobfarm", "return-bench", "--record", "designs/d.txt", "--offsets", "0,17,27"])).unwrap();
        assert_eq!(ok.len(), 1, "{ok:?}");
        assert_eq!(ok[0].0, "designs/d.txt");
        assert_eq!(ok[0].1, git_in(&d, None, &["rev-parse", "HEAD:designs/d.txt"], None).unwrap(), "the blob is the tracked one");
        let outside = std::env::temp_dir().join(format!("mctas-prov-outside-{}.txt", std::process::id()));
        std::fs::write(&outside, "out of tree\n").unwrap();
        let e = evidence_inputs(&d, &a(&["--record", outside.to_str().unwrap()])).unwrap_err();
        assert!(e.contains("outside the worktree"), "{e}");
        let e = evidence_inputs(&d, &a(&[&format!("--record={}", outside.display())])).unwrap_err();
        assert!(e.contains("outside the worktree"), "a --flag=VALUE too: {e}");
        std::fs::write(d.join("designs/new.txt"), "untracked\n").unwrap();
        let e = evidence_inputs(&d, &a(&["--record", "designs/new.txt"])).unwrap_err();
        assert!(e.contains("untracked"), "{e}");
        std::fs::write(d.join("designs/run.log"), "ignored\n").unwrap();
        let e = evidence_inputs(&d, &a(&["--push-grid", "designs/run.log"])).unwrap_err();
        assert!(e.contains("ignored"), "{e}");
        assert!(evidence_inputs(&d, &a(&["--out", outside.to_str().unwrap()])).is_ok(), "an output flag's value is never an input");
        let _ = std::fs::remove_file(&outside);
    }

    /// The source scan behind the loader (G1 07:53 (A)(1)): no evidence crate (mobfarm, mobai) reads an
    /// absolute or home path it names itself; the planted line is caught.
    #[test]
    fn no_evidence_crate_reads_an_absolute_or_home_path() {
        let bad = |text: &str| -> Vec<String> {
            let mut v = Vec::new();
            for (i, l) in text.lines().enumerate() {
                for call in ["read_to_string(\"", "fs::read(\"", "File::open(\"", "read_dir(\""] {
                    if let Some(j) = l.find(call) {
                        let rest = &l[j + call.len()..];
                        if rest.starts_with('/') || rest.starts_with('~') {
                            v.push(format!("line {}: {}", i + 1, l.trim()));
                        }
                    }
                }
            }
            v
        };
        assert_eq!(bad("let t = std::fs::read_to_string(\"/home/x/rec.txt\")?;").len(), 1, "the planted line");
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory");
        let mut found = Vec::new();
        for c in ["mobfarm", "mobai"] {
            let src = crates.join(c).join("src");
            for e in std::fs::read_dir(&src).unwrap().flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "rs") && p.file_name().is_some_and(|n| n != "provenance.rs") {
                    for b in bad(&std::fs::read_to_string(&p).unwrap()) {
                        found.push(format!("{}: {b}", p.display()));
                    }
                }
            }
        }
        assert!(found.is_empty(), "an evidence crate reads an absolute or home path: {found:?}");
    }

    #[test]
    fn the_build_inputs_are_the_code_and_every_include_target() {
        let d = scratch_repo("inputs");
        assert_eq!(build_inputs(&d, None).unwrap(), vec!["crates".to_string(), "Cargo.toml".to_string(), "bridge/agent.txt".to_string()]);
        assert_eq!(build_inputs(&d, Some("HEAD")).unwrap(), build_inputs(&d, None).unwrap(), "the commit's set is the worktree's when clean");
        let b0 = worktree_build_tree(&d).unwrap();
        assert_eq!(b0, commit_build_tree(&d, "HEAD").unwrap(), "a clean worktree's build tree is the commit's");
        std::fs::write(d.join("designs/d.txt"), "an edited design\n").unwrap();
        assert_eq!(worktree_build_tree(&d).unwrap(), b0, "a design edit (a run input) leaves the build tree");
        std::fs::write(d.join("bridge/agent.txt"), "an edited agent\n").unwrap();
        assert_ne!(worktree_build_tree(&d).unwrap(), b0, "an include target's edit changes the build tree");
        std::fs::write(d.join("bridge/agent.txt"), "an agent\n").unwrap();
        std::fs::write(d.join("crates/x/src/b.rs"), "fn b() {}\n").unwrap();
        assert_ne!(worktree_build_tree(&d).unwrap(), b0, "an untracked code file changes the build tree");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn refusals_by_name_unstamped_stale_foreign_dirty() {
        let d = scratch_repo("refuse");
        let b = worktree_build_tree(&d).unwrap();
        let (h, citable) = check_with(Some(&b), &d, None, false).expect("the clean commit's own stamp passes");
        assert!(citable && h.contains(&b), "{h}");
        let e = check_with(None, &d, None, false).expect_err("unstamped");
        assert!(e.contains("REFUSED") && e.contains("UNSTAMPED"), "{e}");
        let e = check_with(Some("0000000000000000000000000000000000000000"), &d, None, false).expect_err("foreign");
        assert!(e.contains("REFUSED") && e.contains("STALE"), "{e}");
        // (a stale stamp: built before a code edit, the edit committed, the stamp not renewed)
        std::fs::write(d.join("crates/x/src/a.rs"), "const A: &str = include_str!(\"../../../bridge/agent.txt\"); // edited\n").unwrap();
        commit_all(&d, "c2");
        let e = check_with(Some(&b), &d, None, false).expect_err("stale");
        assert!(e.contains("STALE"), "{e}");
        // (the stamp is the cited commit's code, but not the commit cited: HEAD~1's stamp citing HEAD)
        let b2 = worktree_build_tree(&d).unwrap();
        let e = check_with(Some(&b2), &d, Some("HEAD~1"), false).expect_err("another commit");
        assert!(e.contains("not the cited commit"), "{e}");
        // (dirty run inputs: an uncommitted design edit, with no cite and with HEAD cited)
        std::fs::write(d.join("designs/d.txt"), "uncommitted\n").unwrap();
        let e = check_with(Some(&b2), &d, None, false).expect_err("dirty, no cite");
        assert!(e.contains("DIRTY"), "{e}");
        let e = check_with(Some(&b2), &d, Some("HEAD"), false).expect_err("dirty inputs");
        assert!(e.contains("the run's inputs"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn not_evidence_labels_every_line_and_is_never_citable() {
        let d = scratch_repo("label");
        let (h, citable) = check_with(None, &d, None, true).expect("labelled, not refused");
        assert!(!citable && h.starts_with("NOT EVIDENCE"), "{h}");
        assert_eq!(label(citable, "the row's verdict: PASS"), "[NOT EVIDENCE] the row's verdict: PASS");
        assert_eq!(label(true, "x"), "x");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_evidence_subcommands_default_deny() {
        assert!(is_evidence("mobfarm", "return-bench") && is_evidence("mobfarm", "gen") && is_evidence("mobai", "anything-new"));
        assert!(!is_evidence("mobfarm", "source-tree") && !is_evidence("door", "verify"));
    }

    #[test]
    fn the_exit_code_table() {
        let t: Vec<(i32, &str)> = [Exit::Pass, Exit::Fail, Exit::NotModeled, Exit::NotEvidence, Exit::Refused].iter().map(|e| (*e as i32, e.name())).collect();
        assert_eq!(t, vec![(0, "PASS"), (1, "FAIL"), (2, "NOT MODELED"), (3, "NOT EVIDENCE"), (4, "REFUSED")]);
        assert!(Exit::Refused.last_line().starts_with("exit 4 (REFUSED)"));
    }

    /// G1 02:44b (b): cargo's env tracking is the claim, verified: a tiny crate reading the stamp through
    /// option_env! (with this crate's build-script declaration) built three times reports each new stamp:
    /// stamp A, stamp B, then stamp C after a one-byte source change.
    #[test]
    fn a_rebuild_re_embeds_the_stamp() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let d = std::env::temp_dir().join(format!("mctas-prov-embed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("Cargo.toml"), "[package]\nname = \"stampcheck\"\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = \"build.rs\"\n[workspace]\n").unwrap();
        std::fs::write(d.join("build.rs"), "fn main() { println!(\"cargo:rerun-if-env-changed=MCTAS_SOURCE_TREE\"); }\n").unwrap();
        let src = |extra: &str| format!("fn main() {{ println!(\"{{}}\", option_env!(\"MCTAS_SOURCE_TREE\").unwrap_or(\"none\")); }}{extra}\n");
        std::fs::write(d.join("src/main.rs"), src("")).unwrap();
        let run = |stamp: &str| -> String {
            let st = Command::new(&cargo).args(["build", "-q", "--offline"]).current_dir(&d).env("MCTAS_SOURCE_TREE", stamp).env("CARGO_TARGET_DIR", d.join("target")).status().expect("cargo build");
            assert!(st.success(), "the scratch build");
            let out = Command::new(d.join("target/debug/stampcheck")).output().expect("run");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        assert_eq!(run("aaaa"), "aaaa");
        assert_eq!(run("bbbb"), "bbbb", "a new stamp re-embeds");
        std::fs::write(d.join("src/main.rs"), src(" ")).unwrap();
        assert_eq!(run("cccc"), "cccc", "after a one-byte source change, the new stamp");
        let _ = std::fs::remove_dir_all(&d);
    }
}
