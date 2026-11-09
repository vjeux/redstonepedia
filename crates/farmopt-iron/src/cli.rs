//! `mctas farmopt ...`: the farm optimizer's commands (designs/briefs/farm-optimizer.md,
//! docs/farms-optimizer.md). A spec names its farm (`farm: iron`); each farm adds one arm here.

use anyhow::{Result, bail};
use clap::Subcommand;
use farmopt::spec::Spec;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum FarmoptCmd {
    /// Answer a stage spec: the merged Pareto front per fork, the bounds and the gap, a design file
    /// per member, written to `--out` (default: designs/farms/<farm>/<stage>/).
    Run {
        spec: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        /// The journal (default: target/farmopt/<farm>-<stage>-<space hash>.tsv): every evaluation
        /// kept, so a rerun only evaluates what is new.
        #[arg(long)]
        journal: Option<PathBuf>,
        /// Stop at the quick level (a preview of the fronts: no full judge).
        #[arg(long)]
        no_full: bool,
        #[arg(long, default_value_t = 4000)]
        quick_budget: usize,
        #[arg(long, default_value_t = 40)]
        full_budget: usize,
        #[arg(long, default_value_t = 2)]
        local: usize,
        /// Also evaluate the stage's exact seeds (calibration points, never on a front).
        #[arg(long)]
        seeds: bool,
        #[arg(long)]
        threads: Option<usize>,
    },
    /// The size of a spec's space: candidates, per fork.
    Count { spec: PathBuf },
    /// Write one candidate's design file (by its key).
    Design {
        spec: PathBuf,
        key: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// The part catalogs of a stage (designs/farms/iron/parts/<stage>-<part>.tsv): every cage, pad,
    /// kill, calm part and collection on its own objectives, its Pareto members marked.
    Parts {
        #[arg(long)]
        stage: String,
        #[arg(long, default_value = "designs/farms/iron/parts")]
        out: PathBuf,
    },
    /// Time the evaluation ladder (static, quick, full) on N candidates spread over a spec's space,
    /// each level's every result written to `--out` (tab-separated: key, level, metrics and lines
    /// or the refusal) and summed into one digest, so two builds' results can be compared exactly
    /// (tickexact 2026-11-08: the optimizer's speed work changes no result).
    Bench {
        spec: PathBuf,
        /// Candidates (spread evenly over the space).
        #[arg(long, default_value_t = 16)]
        n: usize,
        /// Of those passing static, how many get the quick level.
        #[arg(long, default_value_t = 8)]
        quick: usize,
        /// Of those passing quick, how many get the full judge.
        #[arg(long, default_value_t = 0)]
        full: usize,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Only candidates whose key contains this.
        #[arg(long)]
        only: Option<String>,
    },
}

fn journal_path(spec: &Spec) -> PathBuf {
    // FNV-1a of what the space depends on (not the objectives)
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in spec.space_key().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    PathBuf::from(format!("target/farmopt/{}-{}-{:016x}.tsv", spec.farm, spec.stage, h))
}

pub fn run(cmd: FarmoptCmd) -> Result<()> {
    match cmd {
        FarmoptCmd::Run { spec, out, journal, no_full, quick_budget, full_budget, local, seeds, threads } => {
            let s = Spec::parse(&std::fs::read_to_string(&spec)?)?;
            if s.farm != "iron" {
                bail!("no farm {:?} (iron)", s.farm);
            }
            let mut farm = crate::IronFarm::new(&s.stage);
            farm.seeds = seeds;
            let threads = threads.unwrap_or_else(farmopt::pool::threads);
            let o = farmopt::search::Opts { journal: journal.unwrap_or_else(|| journal_path(&s)), threads, quick_budget, full_budget, quick_batch: threads, full_batch: 4, local_rounds: local, no_full, verbose: true };
            let res = farmopt::search::run(&farm, &s, &o)?;
            let dir = out.unwrap_or_else(|| PathBuf::from(format!("designs/farms/{}/{}", s.farm, s.stage)));
            let files = farmopt::report::write(&farm, &s, &res, &dir)?;
            println!("{} candidates, {} quick evaluations, {} full judges, {:.0} s", res.cands.len(), res.quick_runs, res.full_runs, res.seconds);
            for f in &res.forks {
                println!("fork {}: {} on the front, {} merged, {} open", f.fork, f.members.len(), f.merged.len(), f.open.len());
            }
            println!("{} files in {}", files.len(), dir.display());
        }
        FarmoptCmd::Count { spec } => {
            let s = Spec::parse(&std::fs::read_to_string(&spec)?)?;
            let farm = crate::IronFarm::new(&s.stage);
            let all = farmopt::Farm::enumerate(&farm, &s)?;
            let mut forks: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for c in &all {
                let f = s.forks.iter().map(|f| format!("{f}={}", crate::cand::param(c, f).unwrap_or_default())).collect::<Vec<_>>().join(" ");
                *forks.entry(f).or_default() += 1;
            }
            println!("{} candidates in {} {}'s space ({} calm parts)", all.len(), s.farm, s.stage, farm.ctx.calm_parts.len());
            for (f, n) in forks {
                println!("  {}: {n}", if f.is_empty() { "all" } else { &f });
            }
        }
        FarmoptCmd::Bench { spec, n, quick, full, threads, out, only } => bench(&spec, n, quick, full, threads, out, only.as_deref())?,
        FarmoptCmd::Parts { stage, out } => {
            if stage == "site" {
                for line in crate::catalogs::write_site(&out)? {
                    println!("{line}");
                }
                return Ok(());
            }
            for line in crate::catalogs::write_all(&stage, &out)? {
                println!("{line}");
            }
        }
        FarmoptCmd::Design { spec, key, out } => {
            let s = Spec::parse(&std::fs::read_to_string(&spec)?)?;
            // the design as judged, when a journal of this spec's space holds it
            let journal = journal_path(&s);
            if let Ok(j) = farmopt::journal::Journal::open(&journal, crate::VERSION)
                && let Some(e) = j.at(&key, farmopt::Level::Full)
                && let Some(t) = e.lines.get("design_text")
            {
                std::fs::write(&out, t)?;
                println!("{} (as judged, from {})", out.display(), journal.display());
                return Ok(());
            }
            let mut farm = crate::IronFarm::new(&s.stage);
            farm.seeds = true;
            let all = farmopt::Farm::enumerate(&farm, &s)?;
            let Some(c) = all.iter().find(|c| crate::cand::key(c) == key) else { bail!("no candidate {key} in the space") };
            std::fs::write(&out, farmopt::Farm::design(&farm, c, &s)?)?;
            println!("{}", out.display());
        }
    }
    Ok(())
}

/// One evaluation as a line: its metrics (value, low, high) and lines, or its refusal.
fn eval_line(r: &std::result::Result<farmopt::Evaluation, String>) -> String {
    match r {
        Ok(e) => {
            let m: Vec<String> = e.metrics.iter().map(|(k, v)| format!("{k}={:?}/{:?}/{:?}", v.v, v.lo, v.hi)).collect();
            let l: Vec<String> = e.lines.iter().filter(|(k, _)| k.as_str() != "design_text").map(|(k, v)| format!("{k}={v}")).collect();
            format!("ok\t{}\t{}", m.join(" "), l.join(" | "))
        }
        Err(e) => format!("refused\t{e}"),
    }
}

/// `farmopt bench`: the ladder's levels on spread candidates, timed, every result kept.
fn bench(spec: &std::path::Path, n: usize, quick: usize, full: usize, threads: Option<usize>, out: Option<PathBuf>, only: Option<&str>) -> Result<()> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let s = Spec::parse(&std::fs::read_to_string(spec)?)?;
    let farm = crate::IronFarm::new(&s.stage);
    let all: Vec<crate::cand::Cand> = farmopt::Farm::enumerate(&farm, &s)?.into_iter().filter(|c| only.is_none_or(|o| crate::cand::key(c).contains(o))).collect();
    if all.is_empty() {
        bail!("no candidate");
    }
    let step = (all.len() / n.max(1)).max(1);
    let picked: Vec<&crate::cand::Cand> = all.iter().step_by(step).take(n).collect();
    let threads = threads.unwrap_or_else(farmopt::pool::threads).max(1);
    // run `f` on every item over `threads` threads, results in item order, with the wall time and
    // the summed per-item time
    fn par<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> (Vec<R>, f64, f64) {
        let next = AtomicUsize::new(0);
        let res: Mutex<Vec<Option<(R, f64)>>> = Mutex::new((0..items.len()).map(|_| None).collect());
        let t0 = std::time::Instant::now();
        std::thread::scope(|sc| {
            for _ in 0..threads.min(items.len().max(1)) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let t = std::time::Instant::now();
                    let r = f(&items[i]);
                    let dt = t.elapsed().as_secs_f64();
                    res.lock().unwrap()[i] = Some((r, dt));
                });
            }
        });
        let wall = t0.elapsed().as_secs_f64();
        let v: Vec<(R, f64)> = res.into_inner().unwrap().into_iter().map(|x| x.unwrap()).collect();
        let cpu: f64 = v.iter().map(|x| x.1).sum();
        (v.into_iter().map(|x| x.0).collect(), wall, cpu)
    }
    let mut lines: Vec<String> = Vec::new();
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    let mut add = |key: &str, level: &str, r: &std::result::Result<farmopt::Evaluation, String>, lines: &mut Vec<String>| {
        let l = format!("{key}\t{level}\t{}", eval_line(r));
        for b in l.bytes() {
            digest = (digest ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
        lines.push(l);
    };
    let (st, wall, cpu) = par(&picked, threads, |c| farmopt::Farm::eval_static(&farm, c, &s));
    let passed: Vec<(&crate::cand::Cand, farmopt::Metrics)> = picked.iter().zip(&st).filter_map(|(c, r)| r.as_ref().ok().map(|e| (*c, e.metrics.clone()))).collect();
    println!("static: {} candidates, {} pass, {wall:.1} s wall, {:.2} s each", picked.len(), passed.len(), cpu / picked.len().max(1) as f64);
    for (c, r) in picked.iter().zip(&st) {
        add(&crate::cand::key(c), "static", r, &mut lines);
    }
    let qs: Vec<&(&crate::cand::Cand, farmopt::Metrics)> = passed.iter().take(quick).collect();
    if !qs.is_empty() {
        let (q, wall, cpu) = par(&qs, threads, |(c, m)| farmopt::Farm::eval_quick(&farm, c, &s, m));
        println!("quick: {} candidates, {} pass, {wall:.1} s wall, {:.2} s each", qs.len(), q.iter().filter(|r| r.is_ok()).count(), cpu / qs.len() as f64);
        for ((c, _), r) in qs.iter().zip(&q) {
            add(&crate::cand::key(c), "quick", r, &mut lines);
        }
        let fs: Vec<&&(&crate::cand::Cand, farmopt::Metrics)> = qs.iter().zip(&q).filter(|(_, r)| r.is_ok()).map(|(c, _)| c).take(full).collect();
        if !fs.is_empty() {
            // the full judge runs its own threads: one candidate at a time
            let (f, wall, _) = par(&fs, 1, |(c, m)| farmopt::Farm::eval_full(&farm, c, &s, m));
            println!("full: {} candidates, {} pass, {wall:.1} s wall, {:.1} s each", fs.len(), f.iter().filter(|r| r.is_ok()).count(), wall / fs.len() as f64);
            for ((c, _), r) in fs.iter().zip(&f) {
                add(&crate::cand::key(c), "full", r, &mut lines);
            }
        }
    }
    println!("digest {digest:016x} ({} results)", lines.len());
    if let Some(o) = out {
        std::fs::write(&o, lines.join("\n") + "\n")?;
        println!("{}", o.display());
    }
    Ok(())
}
