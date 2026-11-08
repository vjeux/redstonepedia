use crate::{probe, scenario::{self, Scenario}};
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use mctas::sim::{Worker, WorkerConfig};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum BoatLeadsCmd {
    /// Invalid-target and destruction behavior against the game, with live positive controls.
    ApiCheck,
    /// Fixed source-branch fixtures, recorded in the game then compared exactly with redsim.
    Calibrate {
        #[arg(long, default_value = "target/boat-leads-calibration")]
        out_dir: PathBuf,
        /// Run only a named fixed fixture.
        #[arg(long)]
        case: Option<String>,
        /// Saved pre-port redsim-fuzz binary; persist its real missing-support report too.
        #[arg(long)]
        baseline: Option<PathBuf>,
        /// Record without comparing (for before/after tests).
        #[arg(long)]
        record_only: bool,
    },
    /// Reproduce a JSON mechanics scenario, with every tick in an exact trace.
    Record {
        scenario: PathBuf,
        #[arg(long, default_value = "target/boat-leads-record")]
        out_dir: PathBuf,
    },
    /// Replay every kept legacy trace and re-verify ALL archived boat/raft clocks.
    Legacy {
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long, default_value = "target/boat-leads-legacy")]
        out_dir: PathBuf,
        /// Debugging only: skip official-game clock re-verification, not a full ship gate.
        #[arg(long)]
        skip_boat_verify: bool,
        /// At most two official-game workers, per the shared-node budget.
        #[arg(long, default_value_t = 2)]
        workers: usize,
    },
    /// Validate a saved legacy report including already-hidden negatives; does not rerun it.
    LegacyAudit {
        #[arg(long, default_value = "target/boat-leads-legacy")]
        out_dir: PathBuf,
    },
    /// Recheck existing exact fixture traces (no JVM).
    Compare { traces: Vec<PathBuf> },
}

pub fn run(version: &str, command: BoatLeadsCmd) -> Result<()> {
    match command {
        BoatLeadsCmd::ApiCheck => crate::api::run(version)?,
        BoatLeadsCmd::Calibrate { out_dir, case, baseline, record_only } => {
            let cases: Vec<_> = scenario::calibration_cases().into_iter().filter(|c| case.as_ref().is_none_or(|name| c.name == *name)).collect();
            ensure!(!cases.is_empty(), "unknown calibration case");
            let mut w = Worker::spawn(&WorkerConfig { version: version.into(), ..Default::default() })?;
            let mut paths = Vec::new();
            for scene in &cases {
                let path = probe::record(&mut w, scene, &out_dir)?;
                println!("RECORDED {}: {} game ticks", scene.name, scene.ticks);
                paths.push(path);
            }
            drop(w);
            if let Some(binary) = baseline { probe::baseline(&binary, &paths, &out_dir)?; }
            if !record_only { probe::require_all(&paths)?; }
        }
        BoatLeadsCmd::Record { scenario, out_dir } => {
            let text = std::fs::read_to_string(&scenario).with_context(|| format!("read {}", scenario.display()))?;
            let scene: Scenario = serde_json::from_str(&text)?;
            let mut w = Worker::spawn(&WorkerConfig { version: version.into(), ..Default::default() })?;
            let path = probe::record(&mut w, &scene, &out_dir)?;
            println!("RECORDED {}: {} game ticks ({})", scene.name, scene.ticks, path.display());
            drop(w);
            probe::require_all(&[path])?;
        }
        BoatLeadsCmd::Legacy { baseline, out_dir, skip_boat_verify, workers } => crate::legacy::run(&baseline, &out_dir, skip_boat_verify, workers)?,
        BoatLeadsCmd::LegacyAudit { out_dir } => crate::legacy::audit(&out_dir)?,
        BoatLeadsCmd::Compare { traces } => {
            ensure!(!traces.is_empty(), "at least one trace required");
            probe::require_all(&traces)?;
        }
    }
    Ok(())
}
