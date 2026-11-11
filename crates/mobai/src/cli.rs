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
    /// A scene run in the game alone, its mobs' changes printed tick by tick (pose, vehicle,
    /// activities, running behaviors, memories) with the watched blocks' changes: when a villager
    /// lies down, wakes, loses its HOME.
    Timeline {
        name: String,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 2000)]
        ticks: u32,
    },
    /// A recorded scene's panic onset over every phase pair of a sleeping villager's two sensors
    /// (NearestLivingEntitySensor, VillagerHostilesSensor; 400 redsim runs from the scene's state
    /// before its revealing action): the recorded pair's delay first, then the distribution.
    SensorSweep {
        name: String,
        /// The villager's entity number.
        #[arg(long, default_value_t = 0)]
        villager: u32,
        /// The scene action's tick that reveals the hostile.
        #[arg(long)]
        reveal: u32,
        #[arg(long, default_value = "designs/redsim/recorded/mobai")]
        dir: std::path::PathBuf,
    },
    /// (tickexact 2026-11-11) A whole-part unload at an event, in the game alone, over game seeds:
    /// for each seed the scene runs as it is (the event's game tick G: the first change of block
    /// `cell` to a state holding `state` after game time `after`; the cart's rider asleep at G or
    /// not; the rider riding its own cart at G + `check` or not), then again with the whole part
    /// away `len` gt right after the tick to G; per seed, then the totals over the seeds whose
    /// rider was asleep at G with Wilson 95% intervals.
    UnloadRate {
        name: String,
        #[arg(long, default_value_t = 1)]
        from: i64,
        #[arg(long, default_value_t = 20)]
        to: i64,
        /// The event's block, x,y,z (in the scene's coordinates).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        cell: Vec<i32>,
        /// Text the block's new state holds (e.g. `powered=true`).
        #[arg(long)]
        state: String,
        /// The event is the first change after this game time.
        #[arg(long)]
        after: i64,
        /// The cart's drive line (eN's N): its first passenger is the rider.
        #[arg(long)]
        cart: usize,
        #[arg(long, default_value_t = 3)]
        len: u32,
        #[arg(long, default_value_t = 300)]
        check: i64,
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
        /// The day time the farm starts at (as `mobfarm compare --day0`).
        #[arg(long)]
        day0: Option<i64>,
        /// The judge's build K, run `--run` (as `mobfarm compare --build`).
        #[arg(long)]
        build: Option<u32>,
        /// With `--build`: the judge's run I of that build (seed JUDGE_SEED0 + 1000 K + I).
        #[arg(long, default_value_t = 0)]
        run: u32,
        /// No mob spawning on either side (as `mobfarm compare --no-spawning`).
        #[arg(long)]
        no_spawning: bool,
        /// The weather's cycle on both sides (as `mobfarm compare --weather`).
        #[arg(long)]
        weather: bool,
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
    /// Nether portal travel on the official server (the bridge has no Nether): each case's mob
    /// counts per tick in the overworld and the Nether after its summon, beside redsim's model
    PortalProbe {
        #[arg(long, default_value_t = 8)]
        ticks: u32,
        /// The Nether side instead (ironspec's portal pad): a golem's arrival through a linked
        /// Nether portal (its position, the exit cooldown, the PORTAL ticket's window), an item
        /// sent back, a golem in Nether lava.
        #[arg(long)]
        nether: bool,
        /// With --nether: the overworld side as a design file (its blocks at their own coordinates,
        /// its `entity:` lines summoned and followed), instead of the built-in room.
        #[arg(long)]
        overworld: Option<std::path::PathBuf>,
        /// With --nether: the Nether side as a design file (placed at its own coordinates in the
        /// Nether: choose coordinates that link).
        #[arg(long)]
        nether_design: Option<std::path::PathBuf>,
        /// With the design files: print each entity every this many ticks (and at every event).
        #[arg(long, default_value_t = 20)]
        every: u32,
        /// With the design files: run them on redsim (its Nether model) instead of the official
        /// server, printing the same lines.
        #[arg(long)]
        redsim: bool,
        /// Check how a pasted lava source spreads on the official server, in each dimension.
        #[arg(long)]
        fluid_test: bool,
        /// With the design files: summon the overworld design's k-th entity k x this many ticks
        /// in (golems arriving one after another, as a farm sends them).
        #[arg(long, default_value_t = 0)]
        stagger: u32,
        /// Write the judge's portal-pad test farm (day 1's bed-plate with a portal in its pit and a
        /// Nether part) to this file.
        #[arg(long)]
        make_pad: Option<std::path::PathBuf>,
        /// Zombified piglins from the portal's random ticks (official server, random ticks x1000).
        #[arg(long)]
        piglins: bool,
        /// The PORTAL ticket's window on the official server (an item's Age in the Nether).
        #[arg(long)]
        ticket: bool,
    },
}

pub fn run(version: &str, cmd: MobaiCmd) -> Result<()> {
    match cmd {
        MobaiCmd::BenchGame { mob, count, ticks, half, seed } => crate::bench::bench_game(version, &mob, count, ticks, half, seed),
        MobaiCmd::BenchRedsim { mob, count, ticks, half, seed } => crate::bench::bench_redsim(&mob, count, ticks, half, seed),
        MobaiCmd::Pathfuzz { cases, seed, density, verbose } => crate::pathfuzz::pathfuzz(version, cases, seed, density, verbose),
        MobaiCmd::Fixture { name, seed, ticks, full, probe, trace_at, entity } => crate::fixture::run(version, &name, seed, ticks, full, probe, trace_at, entity),
        MobaiCmd::Record { scenes, seed, ticks, dir } => crate::fixture::record(version, &scenes, seed, ticks, &dir),
        MobaiCmd::Timeline { name, seed, ticks } => crate::fixture::timeline(version, &name, seed, ticks),
        MobaiCmd::SensorSweep { name, villager, reveal, dir } => crate::fixture::sensor_sweep(&name, villager, reveal, &dir),
        MobaiCmd::UnloadRate { name, from, to, cell, state, after, cart, len, check } => {
            let [x, y, z] = cell[..] else { anyhow::bail!("--cell takes x,y,z") };
            crate::fixture::unload_rate(version, &name, from..=to, [x, y, z], &state, after, cart, len, check)
        }
        MobaiCmd::Replay { paths } => crate::fixture::replay(&paths),
        MobaiCmd::FarmTrace { design, seed, at, entity, terrain, day0, build, run, no_spawning, weather } => crate::farmtrace::run(version, &design, seed, at, entity, terrain, day0, build.map(|k| (k, run)), no_spawning, weather),
        MobaiCmd::Mathfuzz { n, seed, inputs } => crate::mathfuzz::run(version, n, seed, &inputs),
        MobaiCmd::PortalProbe { ticks, nether, overworld, nether_design, every, redsim, fluid_test, stagger, make_pad, piglins, ticket } => match (nether, overworld, nether_design) {
            _ if ticket => crate::portal::ticket(version),
            _ if piglins => crate::portal::piglins(version, ticks),
            _ if make_pad.is_some() => crate::portal::make_pad(make_pad.as_deref().expect("a path")),
            _ if fluid_test => crate::portal::fluid_test(version),
            (true, Some(o), Some(n)) if redsim => crate::portal::nether_designs_redsim(&o, &n, ticks, every, stagger),
            (true, Some(o), Some(n)) => crate::portal::nether_designs(version, &o, &n, ticks, every, stagger),
            (true, None, None) => crate::portal::nether(version, ticks),
            (true, _, _) => anyhow::bail!("--overworld and --nether-design go together"),
            (false, _, _) => crate::portal::run(version, ticks),
        },
    }
}
