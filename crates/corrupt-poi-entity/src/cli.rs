//! `mctas corrupt-poi-entity ...`: POI/entity-store desync scenarios on the official server.

use crate::scenarios;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum CorruptPoiEntityCmd {
    /// Run one scenario by name on a fresh official-server world (bed-double, job-double,
    /// piston-push, phantom-workstation) and print PASS/FAIL with the marker values seen.
    Scenario {
        name: String,
        /// Keep the server world directories after the run (for post-mortems).
        #[arg(long)]
        keep_dir: bool,
    },
    /// Run every scenario, each on its own fresh world.
    All {
        #[arg(long)]
        keep_dir: bool,
    },
}

pub fn run(version: &str, command: CorruptPoiEntityCmd) -> Result<()> {
    match command {
        CorruptPoiEntityCmd::Scenario { name, keep_dir } => scenarios::run_one(version, &name, keep_dir),
        CorruptPoiEntityCmd::All { keep_dir } => scenarios::run_all(version, keep_dir),
    }
}
