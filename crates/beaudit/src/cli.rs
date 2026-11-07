//! `mctas beaudit ...`: the block-entity desync audit's commands
//! (designs/briefs/corrupt-block-entity.md).

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum BeauditCmd {
    /// Run scenario scripts on the UNMODIFIED official server and check their assertions
    /// (block states from the saved region files, block-entity data via `data get block`,
    /// `restart` lines reboot the server on the same world). Script lines are documented
    /// in `crates/beaudit/src/vanilla.rs`; the scenarios live in
    /// `designs/corrupt-block-entity/scenarios/*.vanilla`.
    Vanilla {
        scenarios: Vec<PathBuf>,
        /// Keep the (first) server's world directory after the run.
        #[arg(long)]
        keep: bool,
        /// Heap of the server JVM, in MB (the VanillaServer default is 1536).
        #[arg(long, default_value_t = 1536)]
        heap_mb: u32,
    },
}

pub fn run(version: &str, cmd: BeauditCmd) -> Result<()> {
    match cmd {
        BeauditCmd::Vanilla { scenarios, keep, heap_mb } => crate::vanilla::run_scenarios(version, &scenarios, keep, heap_mb),
    }
}
