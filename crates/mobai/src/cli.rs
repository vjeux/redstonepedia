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
    /// (mob-AI 2026-11-11) The seeds whose WanderingTraderSpawner reaches a spawn try: its own
    /// random (the bridge's seeding, `rng::trader_seed`) over the 24000-tick attempts of a new
    /// world (chance 25, +25 up to 75 each attempt; a try needs nextInt(100) <= chance, then 1 in
    /// 10). `--judge`: the judge's builds FROM..=TO, run 0 (seed JUDGE_SEED0 + 1000 K).
    TraderTries {
        #[arg(long, default_value_t = 0)]
        from: i64,
        #[arg(long, default_value_t = 11)]
        to: i64,
        #[arg(long)]
        judge: bool,
        /// Attempts looked at (5: a 5-day judge run's 24000..120000).
        #[arg(long, default_value_t = 5)]
        attempts: u32,
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
        /// Memories printed too that the timeline leaves out by default (walk_target, path, ...)
        #[arg(long, value_delimiter = ',')]
        also: Vec<String>,
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
    /// (tickexact 2026-11-11, ironspec's forced-phase mode) Scene reload-wake (a sleeper's sandbox
    /// unloaded and loaded again, a zombie shown at the load) once per pair of forced start delays of
    /// its NearestLivingEntitySensor and VillagerHostilesSensor (`--living` x `--hostiles`) and per
    /// sandbox seed, game against redsim tick for tick: the wake after the load on both sides
    /// against the sensors' rule.
    ReloadWake {
        #[arg(long, value_delimiter = ',')]
        living: Vec<i64>,
        #[arg(long, value_delimiter = ',')]
        hostiles: Vec<i64>,
        #[arg(long, value_delimiter = ',', default_value = "1")]
        seeds: Vec<i64>,
    },
    /// (tickexact 2026-11-11, ironspec's ledge on R1's drop) The ledge scenes (ironfarm's escR1 take-off,
    /// its gap and its landing slopes, one scene per stored speed 1.5/1.82/1.96/2.2 b/t, each row a lone
    /// cart or two carts touching or 3/4/5/6 gt apart, empty or ridden) in the game and in redsim, and per
    /// row from the game's frames: take-offs, landings (cell and speed), the least spacing (merges) and
    /// derails.
    Ledge {
        #[arg(long, default_value_t = 100)]
        ticks: u32,
        /// Only these scenes (ledge-r1-150/-182/-196/-220, ledge-r1n-182: farmopt's (c′) row,
        /// ledge-r1s-182: the quarter-tick sweep at both edges of the bad band); default all
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
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
        /// Recordings replayed at once (each on its own thread); 0: the machine's cores, at most 8.
        #[arg(long, default_value_t = 0)]
        jobs: usize,
    },
    /// (tickexact 2026-11-11, ironspec's slope merge) A recorded scene's minecarts lane by lane
    /// (the carts sharing a block x): positions, speeds, riders, the gaps between neighbours
    /// (MERGED under 0.01), every `--every` ticks and at every merge
    RecCarts {
        path: std::path::PathBuf,
        #[arg(long, default_value_t = 0)]
        every: u32,
        /// Lanes also split by bands of this many blocks of z (0: by x only)
        #[arg(long, default_value_t = 0)]
        band: i32,
        /// One line a lane (its carts, the first contact, the closest centres, a merge)
        #[arg(long)]
        summary: bool,
        /// Each cart's x and x speed too (carts through curves)
        #[arg(long)]
        xz: bool,
    },
    /// (tickexact 2026-11-11, ironspec's gravity intake bridge) Scenes gq-intake-*, gq-margin-* and
    /// gq-fill-*'s recordings per lane and per cell: the contact speed, merges, the closest
    /// centres, the worst pair
    RecGqintake {
        paths: Vec<std::path::PathBuf>,
        /// One cell per contact speed (the stored speed after a whole number of ticks rolled),
        /// not per 0.02 / 0.05 band
        #[arg(long)]
        by_speed: bool,
    },
    /// (tickexact 2026-11-11, the slide bridge's empties) Scene gq-empties' recording per lane: each
    /// cart's largest stored speed, how far it went back up the run, the merged pairs
    RecGqempties {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-12, ironspec's drop bridge) Scene drop-bridge's recording per lane: the
    /// release, the landing tick and speed, the spread, the speed after, the rider's health, seat and
    /// eye
    RecDrop {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-12, ironspec's pane sight bridge) Scene pane-sight's recording per lane:
    /// whether the rider saw its zombie through or past the panes, and from which tick
    RecPanesight {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-12) Scene pane-water's recording: its block changes in order
    RecPanewater {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-12, ironfarm's lane hand-off) Scene lane-handoff's recording per lane and
    /// cart: the stop, the speed, the gap to the rider ahead, the rider's health and eye cell
    RecHandoff {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-12) Every pane and fence in the designs whose written arms its neighbours
    /// contradict (a raw-placed block keeps them); fails on any
    ArmLint {
        paths: Vec<std::path::PathBuf>,
    },
    /// (tickexact 2026-11-12, G1's connection exceptions) Scene pane-attach's recording: each
    /// lane's pane and whether its east arm is the expected one
    RecPaneattach {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-11, ironspec's sentinel) Scene sentinel-<case>'s recording as events: the
    /// claim, the bed's, observer's and lamp's flips, the lie-downs and wakes, the reload
    RecSentinel {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-11, ironspec's sentinel) Scene sentinel-<case> on the unmodified
    /// dedicated server, a real save, stop and start where the bridge unloads (woken: 3 gt after
    /// the wake the server shows): the same events, every tick around dusk, dawn and the restart
    SentinelVanilla {
        /// night, asleep, dusk, dawn, woken
        #[arg(long, default_value = "night")]
        case: String,
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-11, ironspec's chunk-loader study) On the official server with the
    /// load-window agent: a forced farm chunk's first ticks after a world load, per tick (its
    /// scheduled ticks, block events and entities on or off, and what ran), over --runs restarts
    LoadWindow {
        /// quiet (the farm's chunks forced, a void world), load (a 25 x 25 forced square of real
        /// terrain loading at the same start), pearl (the farm held only by the bot's stand-in
        /// pearls, the bot logging out and in), pearl-busy (pearl in real terrain at view and
        /// simulation distance 12)
        #[arg(long, default_value = "quiet")]
        case: String,
        #[arg(long, default_value_t = 5)]
        runs: u32,
        #[arg(long, default_value = "26.3")]
        version: String,
        /// item-loop-build: the loop's design file (default the build's l1-build-loop.txt)
        #[arg(long)]
        design: Option<std::path::PathBuf>,
        /// item-loop-build: the mob/pickup row (an adult, a baby and a chicken jockey, loot
        /// pickup on, in each portal's bottom row, made to jump every 7 gt): `empty` (empty
        /// hands, the known limit), `armed` (golden swords, as the portal spawns them) or `mixed`
        /// (the babies empty-handed, the rest armed, and a baby piglin in the Nether portal)
        #[arg(long)]
        mobs: Option<String>,
        /// item-loop-*: the loop's item (replacing the design's cobblestone), e.g. minecraft:leather
        #[arg(long, default_value = "minecraft:cobblestone")]
        item: String,
    },
    /// (tickexact 2026-11-12, ironspec's design-space item 3) On the official server: villagers
    /// holding the overworld bed as HOME at night, calm and panicking in the Nether, calm in the
    /// overworld, and one crossing into the Nether in a minecart: whose HOME survives
    NetherHome {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-12, ironspec's T+600 check) On the official server with the golem agent:
    /// a panicking 3-villager batch's summons exactly 600 gt apart, in both member orders, a mixed
    /// batch, and 36 watchers counting the re-marks a golem's 2-tick dwell gives
    T600 {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-12, ironspec's 3-bed check) On the official server: three beds side by
    /// side, three nappers asleep at once, a calm rider within 16 of another bed (occupied) and
    /// beyond 16 of its own, two controls, then a night save, stop and start
    ThreeBeds {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-12, ironspec's daylight bridge) On the official server: a daylight
    /// detector's clear-sky edges to the tick (the first rise, 7/8 at dawn and dusk), rain and thunder
    /// readings, the bot's bed times per weather, real night skips with the readings after, a rain
    /// ending at dusk
    Daylight {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// Only the skip latency, on a running (unfrozen) server
        #[arg(long)]
        latency: bool,
    },
    /// (tickexact 2026-11-11, the parent's shared-bed overload across reloads) Scene bed-reload-<case>'s
    /// recording at its checkpoints: every villager's HOME and LAST_SLEPT, the bed, the naps
    RecBedreload {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-11, the parent's shared-bed overload across reloads, for vjeux) Scene
    /// bed-reload-<case> on the unmodified dedicated server, with a real save, stop and start where
    /// the bridge unloads and loads its sandbox (or --forceload: the site's forced chunks released and
    /// forced again, nobody near): every villager's HOME and LAST_SLEPT, the bed POI's free tickets
    /// (its saved record) and OCCUPIED at the checkpoints, and every nap
    BedReloadVanilla {
        /// day (after the claims), asleep (villager 10 asleep), woken (3 gt after its wake),
        /// broken (the bed broken for good first: the control), none
        #[arg(long, default_value = "day")]
        case: String,
        #[arg(long)]
        forceload: bool,
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-11, ironspec's sweep after the cart-pole fixes) Every element of the
    /// designs in the classes the fixes changed (path blockers, iron bars and chains, piston-moved
    /// table blocks), with what the old redsim did there
    StopperSweep {
        files: Vec<std::path::PathBuf>,
    },
    /// (tickexact 2026-11-11, ironspec's cart-pole bridge) Scene cart-pole's (and -push, -stop,
    /// -climb's) recording: per lane how far the cart got, whether it passed, and the rider's eye
    RecCartpole {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-11, farmopt's and ironspec's H rest-place contacts) Scene h-contact's
    /// recording: per lane the arrival's closest approach, any merge, and the waiting carts pushed back
    RecHcontact {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-11, ironspec's junction-settle) Scene junction-settle's recording: each lane's
    /// settled junction shape and each cart's route through it
    RecJunction {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-11, ironspec's hatch sight lines) redsim's line of sight from the siding's
    /// napper (stand-up cell, sleeper, T rider) to the dorm zombie, the hatch closed and open
    HatchSweep {
        path: std::path::PathBuf,
    },
    /// (tickexact 2026-11-11, ironspec's side push) A recorded scene's carts in pairs of lanes
    /// (`--pairs xa:xb,...`, one cart a lane): each tick their pushing boxes overlap, both carts'
    /// motion and its departure from free running
    RecSide {
        path: std::path::PathBuf,
        #[arg(long, value_delimiter = ',')]
        pairs: Vec<String>,
    },
    /// (tickexact 2026-11-11, 58f770cc's plate question) A recorded scene's pressure plates tick by
    /// tick: their on/off ticks and how far each mob in a plate's column reaches into its cell
    RecPlate {
        path: std::path::PathBuf,
        #[arg(long)]
        scene: String,
    },
    /// (tickexact 2026-11-11, ironspec's raw-placement audit) The scenes (default: every scene
    /// placed raw) whose written block states one update pass would change (a powered rail on its
    /// redstone block written unpowered, a rail shape, a gate, dust...): FAIL on any
    SceneLint {
        scenes: Vec<String>,
    },
    /// The positive control of `mobfarm flee-check`'s 6-8 metric (ironspec Nov 11 11:13): its
    /// static check run on a mob AI scene (default scene flee-distance: riders and standing
    /// villagers at 5.80..8.05 from a zombie in a boat, and 7.0 behind a wall), each villager's
    /// distance as flee-check measures it (the zombie seated in its boat, the rider seated in its
    /// cart) and its class (flee / panic / none) against the scene's recorded distance and the
    /// game's class.
    FleeControl {
        #[arg(default_value = "flee-distance")]
        scene: String,
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
        MobaiCmd::TraderTries { from, to, judge, attempts } => {
            for k in from..=to {
                let seed = if judge { redsim::entity::mob::farm::JUDGE_SEED0 + 1000 * k } else { k };
                let mut r = redsim::rng::LegacyRandom::new(redsim::rng::trader_seed(seed));
                let mut chance = 25;
                let mut first = None;
                for a in 1..=attempts {
                    let roll = r.next_int(100);
                    let passed = roll <= chance;
                    chance = (chance + 25).clamp(25, 75);
                    if passed && r.next_int(10) == 0 {
                        first = Some(a);
                        break;
                    }
                }
                match first {
                    Some(a) => println!("{} {k} (seed {seed}): a spawn try at attempt {a} (~gt {})", if judge { "build" } else { "seed" }, 24000 * a),
                    None => println!("{} {k} (seed {seed}): no try in {attempts} attempts", if judge { "build" } else { "seed" }),
                }
            }
            Ok(())
        }
        MobaiCmd::Fixture { name, seed, ticks, full, probe, trace_at, entity } => crate::fixture::run(version, &name, seed, ticks, full, probe, trace_at, entity),
        MobaiCmd::Record { scenes, seed, ticks, dir } => crate::fixture::record(version, &scenes, seed, ticks, &dir),
        MobaiCmd::Timeline { name, seed, ticks, also } => crate::fixture::timeline(version, &name, seed, ticks, &also),
        MobaiCmd::SensorSweep { name, villager, reveal, dir } => crate::fixture::sensor_sweep(&name, villager, reveal, &dir),
        MobaiCmd::ReloadWake { living, hostiles, seeds } => crate::fixture::reload_wake(version, &living, &hostiles, &seeds),
        MobaiCmd::Ledge { ticks, only } => crate::fixture::ledge(version, ticks, &only),
        MobaiCmd::UnloadRate { name, from, to, cell, state, after, cart, len, check } => {
            let [x, y, z] = cell[..] else { anyhow::bail!("--cell takes x,y,z") };
            crate::fixture::unload_rate(version, &name, from..=to, [x, y, z], &state, after, cart, len, check)
        }
        MobaiCmd::Replay { paths, jobs } => crate::fixture::replay(&paths, jobs),
        MobaiCmd::RecCarts { path, every, band, summary, xz } => crate::fixture::rec_carts(&path, every, band, summary, xz),
        MobaiCmd::SceneLint { scenes } => crate::fixture::scene_lint(&scenes),
        MobaiCmd::FleeControl { scene } => crate::fixture::flee_control(&scene),
        MobaiCmd::RecPlate { path, scene } => crate::fixture::rec_plate(&path, &scene),
        MobaiCmd::RecBedreload { path } => crate::fixture::rec_bedreload(&path),
        MobaiCmd::RecSentinel { path } => crate::fixture::rec_sentinel(&path),
        MobaiCmd::RecGqintake { paths, by_speed } => crate::fixture::rec_gqintake(&paths, by_speed),
        MobaiCmd::RecGqempties { path } => crate::fixture::rec_gqempties(&path),
        MobaiCmd::RecDrop { path } => crate::fixture::rec_drop(&path),
        MobaiCmd::RecPanesight { path } => crate::fixture::rec_panesight(&path),
        MobaiCmd::RecPanewater { path } => crate::fixture::rec_panewater(&path),
        MobaiCmd::RecHandoff { path } => crate::fixture::rec_handoff(&path),
        MobaiCmd::ArmLint { paths } => crate::fixture::arm_lint(&paths),
        MobaiCmd::RecPaneattach { path } => crate::fixture::rec_paneattach(&path),
        MobaiCmd::SentinelVanilla { case, version } => crate::bedreload::sentinel_vanilla(&version, &case),
        MobaiCmd::LoadWindow { case, runs, version, design, mobs, item } => {
            let _ = crate::loadwindow::LOOP_ITEM.set(item);
            crate::loadwindow::load_window(&version, &case, runs, design.as_deref(), mobs.as_deref())
        }
        MobaiCmd::NetherHome { version } => crate::netherhome::nether_home(&version),
        MobaiCmd::T600 { version } => crate::t600::t600(&version),
        MobaiCmd::ThreeBeds { version } => crate::threebeds::three_beds(&version),
        MobaiCmd::Daylight { version, latency } => {
            if latency {
                crate::daylight::skip_latency(&version)
            } else {
                crate::daylight::daylight(&version)
            }
        }
        MobaiCmd::BedReloadVanilla { case, forceload, version } => crate::bedreload::bed_reload_vanilla(&version, &case, forceload),
        MobaiCmd::StopperSweep { files } => crate::fixture::stopper_sweep(&files),
        MobaiCmd::RecCartpole { path } => crate::fixture::rec_cartpole(&path),
        MobaiCmd::RecHcontact { path } => crate::fixture::rec_hcontact(&path),
        MobaiCmd::RecJunction { path } => crate::fixture::rec_junction(&path),
        MobaiCmd::HatchSweep { path } => crate::fixture::hatch_sweep(&path),
        MobaiCmd::RecSide { path, pairs } => {
            let p: Vec<(i64, i64)> = pairs.iter().filter_map(|s| s.split_once(':').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))).collect();
            crate::fixture::rec_side(&path, &p)
        }
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
