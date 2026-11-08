//! `door audit`, `door selftest`, `door reuse-check`: known answers and audits of the tools
//! themselves.

use anyhow::Result;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct ProvenanceArgs {
    /// The archive folder.
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    archive: PathBuf,
    /// Community root designs (their mechanisms; archived copies named community* / c79-community*
    /// are added).
    #[arg(long, value_delimiter = ',', default_value = "designs/piston-door-3x3/tracks/c79/c79-closed.txt,designs/piston-door-3x3/tracks/c70b/community-7x8.txt")]
    roots: Vec<PathBuf>,
    #[arg(long, default_value = "designs/piston-door-3x3/archive/provenance.tsv")]
    out: PathBuf,
    #[arg(long, default_value_t = 16)]
    threads: usize,
    /// A mechanism within this many pieces (symmetric difference, mirrors merged) of a root's is
    /// community-derived (2 = one piston moved).
    #[arg(long, default_value_t = 2)]
    near: usize,
    /// No lineage step (by default a design within --near pieces of, or with the paths of, a
    /// community-derived one is community-derived too, to a fixpoint).
    #[arg(long)]
    no_propagate: bool,
}

pub fn cmd_provenance(args: ProvenanceArgs) -> Result<()> {
    let ProvenanceArgs { archive, roots, out, threads, near, no_propagate } = args;
    crate::provenance::run(&archive, &roots, &out, threads, near, !no_propagate)
}

#[derive(clap::Args)]
pub struct AuditArgs {
    /// Design files or folders (default: tracks/c79/c79-closed.txt).
    designs: Vec<PathBuf>,
    /// Comma list of checks, mech, wiring, rules (or all).
    #[arg(long, default_value = "all")]
    sections: String,
    /// Also run the JVM checks (the real judge, props, props-dyn, eligible) in section 1.
    #[arg(long)]
    jvm: bool,
    /// Write the markdown here instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Write every extracted flat mechanism with a macro solution as a families catalog TSV.
    #[arg(long)]
    tsv: Option<PathBuf>,
    #[arg(long, default_value_t = 16)]
    threads: usize,
    /// State cap of the macro solve.
    #[arg(long, default_value_t = 200_000)]
    cap: usize,
    /// Sources scanned by the rules section.
    #[arg(long, default_value = "crates/piston-door/src")]
    src: PathBuf,
    /// Section 2: skip designs with no flat mechanism.
    #[arg(long)]
    only_flat: bool,
    /// Print one design's extracted mechanism (paths, moves, providers, membership) instead.
    #[arg(long)]
    explain: bool,
    /// Generator membership of piece lists instead (ideal-input designs): lines
    /// `W H x0 y0 piece piece ...` (box in door coordinates).
    #[arg(long)]
    member: Option<PathBuf>,
}

pub fn cmd_audit(version: &str, args: AuditArgs) -> Result<()> {
    let AuditArgs { designs, sections, jvm, out, tsv, threads, cap, src, only_flat, explain, member } = args;
    if let Some(m) = member {
        return crate::audit::member_file(&m);
    }
    let designs = if designs.is_empty() { vec![PathBuf::from("designs/piston-door-3x3/tracks/c79/c79-closed.txt")] } else { designs };
    if explain {
        for d in &designs {
            crate::audit::explain(d)?;
        }
    } else {
        crate::audit::run(&crate::audit::Opts { designs, sections, jvm, out, tsv, threads, cap, version: version.to_string(), src, only_flat })?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct SelftestArgs {
    /// Only the checks that start no JVM.
    #[arg(long)]
    quick: bool,
    /// Only checks whose name contains one of these (comma list).
    #[arg(long)]
    only: Option<String>,
    /// List the checks (group, JVM workers, what) and exit.
    #[arg(long)]
    list: bool,
    /// CPU threads at once (0: the machine's cores minus its load).
    #[arg(long, default_value_t = 0)]
    jobs: usize,
    /// JVM workers at once (0: from the free cores and memory). MCTAS_MAX_WORKERS caps each pool.
    #[arg(long, default_value_t = 0)]
    jvm_jobs: usize,
    /// Keep the scratch folder even when everything passes.
    #[arg(long)]
    keep: bool,
}

pub fn cmd_selftest(args: SelftestArgs) -> Result<()> {
    let SelftestArgs { quick, only, list, jobs, jvm_jobs, keep } = args;
    crate::selftest::run(&crate::selftest::Opts { quick, only, list, jobs, jvm_jobs, keep })?;
    Ok(())
}

#[derive(clap::Args)]
pub struct TestArgs {
    /// Every piece: the whole suite, on demand (never a scheduled job).
    #[arg(long)]
    all: bool,
    /// Also run these groups' pieces: sim, door, clock, circuit, rail, autodoor, shulker, tnt,
    /// sorter, smelter, brewer, composter, lift, farm (comma list).
    #[arg(long, value_delimiter = ',')]
    pick: Vec<String>,
    /// Also run these designs' own judges.
    #[arg(long)]
    design: Vec<PathBuf>,
    /// The change is the tree against the merge base of HEAD and this ref.
    #[arg(long, default_value = "origin/main")]
    from: String,
    /// Ignore the tree's changes (only `--pick`, `--design`, `--all`).
    #[arg(long)]
    no_diff: bool,
    /// Print the plan (every piece and the files that picked it) and stop.
    #[arg(long)]
    plan: bool,
    /// Only the pieces whose name contains one of these (comma list).
    #[arg(long)]
    only: Option<String>,
    /// CPU threads at once (0: the machine's cores minus its load).
    #[arg(long, default_value_t = 0)]
    jobs: usize,
    /// JVM workers at once (0: from the free cores and memory). MCTAS_MAX_WORKERS caps each pool.
    #[arg(long, default_value_t = 0)]
    jvms: usize,
    /// Stop at the first FAIL (the running pieces are killed).
    #[arg(long)]
    fail_fast: bool,
    /// Keep the scratch folder even when everything passes.
    #[arg(long)]
    keep: bool,
    /// The mutation test of the pieces instead (crates/piston-door/src/mutate.rs): every mutant
    /// of this file (a one-line redsim bug) applied in `--tree`, built, the suite run there.
    #[arg(long)]
    mutants: Option<PathBuf>,
    /// With --mutants: the git worktree the mutants go into (its own build).
    #[arg(long)]
    tree: Option<PathBuf>,
    /// With --mutants: the suite, as mctas arguments (default `test --from HEAD`: the pieces each
    /// mutant's diff picks; the old suite: `door selftest`).
    #[arg(long)]
    suite: Option<String>,
    /// With --mutants: skip the positive control (the suite without a mutant), when it just ran.
    #[arg(long)]
    no_control: bool,
}

pub fn cmd_test(version: &str, args: TestArgs) -> Result<()> {
    use crate::testplan;
    let TestArgs { all, pick, design, from, no_diff, plan, only, jobs, jvms, fail_fast, keep, mutants, tree, suite, no_control } = args;
    if let Some(m) = mutants {
        let tree = tree.ok_or_else(|| anyhow::anyhow!("--mutants needs --tree <a git worktree>"))?;
        let suite: Vec<String> = suite.unwrap_or_else(|| "test --from HEAD".into()).split_whitespace().map(String::from).collect();
        return crate::mutate::run(&m, &tree, &suite, only.as_deref(), no_control);
    }
    let groups: Vec<testplan::Group> = pick.iter().map(|g| testplan::Group::parse(g).ok_or_else(|| anyhow::anyhow!("--pick {g}: one of {}", testplan::Group::ALL.map(|g| g.name()).join(", ")))).collect::<Result<_>>()?;
    let designs: Vec<String> = design.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let change = if no_diff || all { None } else { testplan::changed_files(&from) };
    let (base, files): (String, Option<Vec<String>>) = match (&change, no_diff || all) {
        (Some((b, f)), _) => (b.clone(), Some(f.clone())),
        (None, true) => (String::new(), Some(Vec::new())),
        (None, false) => (String::new(), None),
    };
    let mut lines = Vec::new();
    let mut pieces = testplan::plan(files.as_deref(), &groups, &designs, all, &mut lines);
    if let Some(o) = &only {
        pieces.retain(|p| o.split(',').any(|q| p.name().contains(q)));
    }
    match &files {
        Some(f) if !base.is_empty() => println!("mctas test: the change is {} file(s) against {} (the merge base of HEAD and {from}):", f.len(), &base[..base.len().min(9)]),
        Some(_) => println!("mctas test: {}", if all { "--all" } else { "--no-diff" }),
        None => println!("mctas test: git cannot say what changed against {from}: every piece"),
    }
    for l in &lines {
        println!("{l}");
    }
    let (cpu, jv, about) = testplan::machine();
    let cpu = if jobs > 0 { jobs } else { cpu };
    let jv = if jvms > 0 { jvms } else { jv };
    println!("plan: {} piece(s) on {about}: {cpu} threads and {jv} JVM workers at once (MCTAS_MAX_WORKERS {} per pool)", pieces.len(), testplan::pool_cap().map(|n| n.to_string()).unwrap_or("unset".into()));
    for p in &pieces {
        let more = if p.why.len() > 2 { format!(" (+{} more)", p.why.len() - 2) } else { String::new() };
        println!("  {:<40} {:<16} {}{more}", p.name(), p.group, p.why.iter().take(2).cloned().collect::<Vec<_>>().join("; "));
    }
    let mut flags: Vec<String> = Vec::new();
    if all {
        flags.push("--all".into());
    }
    if no_diff && !all {
        flags.push("--no-diff".into());
    }
    if !pick.is_empty() {
        flags.push(format!("--pick {}", pick.join(",")));
    }
    for d in &designs {
        flags.push(format!("--design {d}"));
    }
    if let Some(o) = &only {
        flags.push(format!("--only {o}"));
    }
    let line = testplan::commit_line(&pieces, &base, &flags.join(" "));
    if plan {
        println!("{line} (plan only)");
        return Ok(());
    }
    if pieces.is_empty() {
        println!("{line}: nothing to run for this change");
        return Ok(());
    }
    testplan::run(pieces, &testplan::RunOpts { cpu, jvms: jv, keep, fail_fast, version: version.to_string() }, &line)
}

#[derive(clap::Subcommand)]
pub enum RecordedCmd {
    /// Check recording directories against their manifests (`recorded.tsv`): a recording whose
    /// design changed since (STALE), edited, without a row, or a row without its recording fails.
    Check { dirs: Vec<PathBuf> },
    /// Print files' content hashes (the manifests' FNV-1a, for answers recorded in code).
    Hash { files: Vec<PathBuf> },
    /// Write a directory's manifest from its recordings after recording one again: every
    /// recording's hash, its header's design (and `deps`) with their hashes now; a row whose
    /// recording did not change keeps its provenance; a new or changed one takes `--how` (the
    /// command that records it), the game version and now + HEAD (`--fresh`: just recorded) or
    /// the last commit that changed it (a recording made before manifests).
    Write {
        dir: PathBuf,
        #[arg(long)]
        how: Option<String>,
        #[arg(long)]
        fresh: bool,
    },
}

pub fn cmd_recorded(version: &str, cmd: RecordedCmd) -> Result<()> {
    let root = std::path::Path::new(".");
    match cmd {
        RecordedCmd::Hash { files } => {
            for f in &files {
                println!("{}  {}", mctas::recorded::file_hash(f)?, f.display());
            }
        }
        RecordedCmd::Check { dirs } => {
            let mut bad = 0;
            let mut n = 0;
            for d in &dirs {
                let problems = mctas::recorded::check_dir(root, d)?;
                n += mctas::recorded::recordings(d)?.len();
                for p in &problems {
                    println!("{}: {p}", d.display());
                }
                bad += problems.len();
            }
            anyhow::ensure!(bad == 0, "{bad} recording problem(s)");
            println!("RECORDINGS CURRENT: {n} recordings in {} directories, every design as recorded", dirs.len());
        }
        RecordedCmd::Write { dir, how, fresh } => {
            let git = |args: &[&str]| -> String { std::process::Command::new("git").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default() };
            let head = git(&["rev-parse", "--short", "HEAD"]);
            let now = std::process::Command::new("date").args(["-u", "+%Y-%m-%dT%H:%MZ"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
            let how_fresh = how.clone().unwrap_or_else(|| "(not given)".into());
            let how_legacy = how.unwrap_or_else(|| "recorded before manifests: see the recording's header and its commit".into());
            let version = version.to_string();
            let v2 = version.clone();
            // a recording's own header says how, when and with which game it was recorded
            // (`circuit record`); else `--how`, now and HEAD
            let fresh_fn = move |p: &std::path::Path| {
                let field = |k: &str| mctas::recorded::header_field(p, k);
                (field("version").unwrap_or(version.clone()), field("recorded").unwrap_or(format!("{now} on {head} (this tree)")), field("how").unwrap_or(how_fresh.clone()))
            };
            let legacy_fn = move |p: &std::path::Path| {
                let last = git(&["log", "-1", "--format=%cs %h", "--", &p.to_string_lossy()]);
                (v2.clone(), if last.is_empty() { "uncommitted".into() } else { last }, how_legacy.clone())
            };
            let rows = mctas::recorded::rows_for(root, &dir, if fresh { Some(&fresh_fn) } else { None }, &legacy_fn)?;
            mctas::recorded::save(&dir, &rows)?;
            println!("wrote {}: {} recordings", dir.join(mctas::recorded::MANIFEST).display(), rows.len());
        }
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct ReuseCheckArgs {
    design: PathBuf,
    #[arg(long, default_value_t = 30)]
    times: u32,
    #[arg(long, default_value_t = 1)]
    cycles: u32,
    /// Recreate the sandbox before every run instead of reusing it.
    #[arg(long)]
    fresh: bool,
}

pub fn cmd_reuse_check(version: &str, args: ReuseCheckArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let ReuseCheckArgs { design, times, cycles, fresh } = args;
    let d = load_design(&design)?;
    let mut w = Worker::spawn(&cfg)?;
    let opts = VerifyOptions { cycles, offsets: vec![[0, 0, 0]], ..Default::default() };
    let mut sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut line = String::new();
    let mut passes = 0;
    for _ in 0..times {
        if fresh {
            w.drop_sandbox(sb)?;
            sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        }
        let ok = crate::verify_in(&mut w, sb, &d, &opts).is_ok();
        passes += ok as u32;
        line.push(if ok { '+' } else { '.' });
    }
    println!("{line}\n{passes}/{times} passed in {} sandbox(es)", if fresh { "fresh" } else { "one reused" });
    Ok(())
}
