//! `mctas corrupt ...`: derived/saved world-data staleness probes
//! (designs/briefs/corrupt-derived.md, designs/corrupt-derived/NOTES.md).

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum CorruptCmd {
    /// P1: a mass light edit followed by immediate departure — unload snapshots the
    /// pending light, the light-data wipe discards the rest, and the saved layers are
    /// trusted on reload. Control = same edit, light allowed to settle first.
    Light {
        /// Fill toggles of the test volume (each block queues one light task).
        #[arg(long, default_value_t = 6)]
        fills: usize,
        /// Test volume extends chunk_radius chunks around chunk 0,0 in x/z.
        #[arg(long, default_value_t = 1)]
        chunks: i32,
        /// Ticks to step after teleporting away (unload processing).
        #[arg(long, default_value_t = 30)]
        leave_ticks: u32,
        /// Keep the scenario world directories (for post-mortems / `corrupt audit`).
        #[arg(long)]
        keep: bool,
    },
    /// P2: inhabited time is incremented every spawning tick but never marks its chunk
    /// unsaved, so an inert chunk's growth rolls back on unload while a once-dirtied
    /// control chunk banks it.
    Inhabited {
        /// Spawning ticks to accumulate before leaving.
        #[arg(long, default_value_t = 1200)]
        ticks: u32,
        #[arg(long)]
        keep: bool,
    },
    /// P4 (negative control): recompute WORLD_SURFACE heightmaps from a saved world's
    /// blocks and diff them against the saved `Heightmaps` (expects 0 mismatches on a
    /// vanilla-played world — load trusts them blindly, so any mismatch is forever).
    Audit {
        /// A dimension, level, or server directory.
        world: PathBuf,
        /// World min Y (overworld -64).
        #[arg(long, default_value_t = -64)]
        min_y: i32,
        /// World height in blocks (overworld 384).
        #[arg(long, default_value_t = 384)]
        height: i32,
    },
}

pub fn run(version: &str, cmd: CorruptCmd) -> Result<()> {
    match cmd {
        CorruptCmd::Light { fills, chunks, leave_ticks, keep } => crate::probe_light(version, fills, chunks, leave_ticks, keep),
        CorruptCmd::Inhabited { ticks, keep } => crate::probe_inhabited(version, ticks, keep),
        CorruptCmd::Audit { world, min_y, height } => crate::audit_heightmaps(&world, min_y, height),
    }
}
