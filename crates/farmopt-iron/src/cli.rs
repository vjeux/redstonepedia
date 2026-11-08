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
