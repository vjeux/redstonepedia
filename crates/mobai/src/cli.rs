//! `mctas mobai ...`: the mob-AI lane's commands.

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum MobaiCmd {
    /// The game's own cost of mob AI (the bar): N mobs of a type on a closed stone floor in a
    /// bridge sandbox (natural spawning off, persistent mobs), stepped T ticks, timed against the
    /// same sandbox without mobs. Prints microseconds per mob per tick.
    BenchGame {
        /// The mob type (`zombie`, `iron_golem`, `villager`, `creeper`, `skeleton`, `spider`).
        #[arg(long, default_value = "zombie")]
        mob: String,
        /// How many mobs.
        #[arg(long, default_value_t = 50)]
        count: u32,
        /// Ticks timed.
        #[arg(long, default_value_t = 2000)]
        ticks: u32,
        /// The floor's half side (the sandbox covers it).
        #[arg(long, default_value_t = 32)]
        half: i32,
        #[arg(long, default_value_t = 1)]
        seed: i64,
    },
    /// The same arena in redsim: the mob layer's cost per mob per tick (today: physics and the
    /// villager brain, no walking; with the AI: the whole model).
    BenchRedsim {
        #[arg(long, default_value = "zombie")]
        mob: String,
        #[arg(long, default_value_t = 50)]
        count: u32,
        #[arg(long, default_value_t = 2000)]
        ticks: u32,
        #[arg(long, default_value_t = 32)]
        half: i32,
        #[arg(long, default_value_t = 1)]
        seed: i64,
    },
    /// The path probe fuzzer: random block worlds, the game's `createPath` against redsim's, node
    /// by node, for zombies, villagers, golems, creepers, spiders and skeletons.
    Pathfuzz {
        #[arg(long, default_value_t = 100)]
        cases: u32,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// The share of columns with a palette block on top.
        #[arg(long, default_value_t = 0.25)]
        density: f64,
        #[arg(long)]
        verbose: bool,
    },
    /// A mob-AI scene in the game and in redsim, compared tick for tick (redsim::exact).
    Fixture {
        /// The scene (`zombie-stroll`, `zombie-obstacles`, ...).
        name: String,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 2000)]
        ticks: u32,
        /// Keep comparing after the first difference.
        #[arg(long)]
        full: bool,
        /// The game's draw log with each frame (the first difference's context).
        #[arg(long)]
        probe: bool,
        /// Both sides' draws of this game time (the game's log, redsim's call sites).
        #[arg(long)]
        trace_at: Option<i64>,
        /// Trace this entity's own random instead of the level random.
        #[arg(long)]
        entity: Option<u32>,
    },
    /// Record scenes in the game for `mctas test` (`<dir>/<scene>.rec`: the bridge's frames,
    /// gzipped, with the scene's hash in the header), then `mctas recorded write <dir>`.
    Record {
        /// The scenes (default: every scene).
        scenes: Vec<String>,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 3000)]
        ticks: u32,
        #[arg(long, default_value = "designs/redsim/recorded/mobai")]
        dir: std::path::PathBuf,
    },
    /// Replay recorded scenes on redsim (no game): every frame identical, or FAIL.
    Replay {
        /// Recording files or directories of them.
        paths: Vec<std::path::PathBuf>,
    },
    /// A farm design's redsim run as the exact compare runs it, the level random's (or an
    /// entity's) draws of one game tick printed by call site.
    FarmTrace {
        design: std::path::PathBuf,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// The game time of the traced tick.
        #[arg(long)]
        at: i64,
        /// Trace this entity's own random instead of the level random.
        #[arg(long)]
        entity: Option<u32>,
        /// Build it into a terrain sample (as `mobfarm compare --terrain`).
        #[arg(long)]
        terrain: Option<String>,
    },
    /// redsim's math ports (Math.sin/cos, Mth.atan2, Mth.sin/cos) against the game JVM, bit for
    /// bit, on random inputs (and the given ones: decimals or 0x raw bits).
    Mathfuzz {
        #[arg(long, default_value_t = 20000)]
        n: usize,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        inputs: Vec<String>,
    },
}

pub fn run(version: &str, cmd: MobaiCmd) -> Result<()> {
    match cmd {
        MobaiCmd::BenchGame { mob, count, ticks, half, seed } => crate::bench::bench_game(version, &mob, count, ticks, half, seed),
        MobaiCmd::BenchRedsim { mob, count, ticks, half, seed } => crate::bench::bench_redsim(&mob, count, ticks, half, seed),
        MobaiCmd::Pathfuzz { cases, seed, density, verbose } => crate::pathfuzz::pathfuzz(version, cases, seed, density, verbose),
        MobaiCmd::Fixture { name, seed, ticks, full, probe, trace_at, entity } => crate::fixture::run(version, &name, seed, ticks, full, probe, trace_at, entity),
        MobaiCmd::Record { scenes, seed, ticks, dir } => crate::fixture::record(version, &scenes, seed, ticks, &dir),
        MobaiCmd::Replay { paths } => crate::fixture::replay(&paths),
        MobaiCmd::FarmTrace { design, seed, at, entity, terrain } => crate::farmtrace::run(version, &design, seed, at, entity, terrain),
        MobaiCmd::Mathfuzz { n, seed, inputs } => crate::mathfuzz::run(version, n, seed, &inputs),
    }
}
