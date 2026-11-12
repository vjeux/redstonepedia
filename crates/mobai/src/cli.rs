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
    /// (tickexact 2026-11-12, ironspec: the golem lock only on success) On the official server:
    /// 3 panicking villagers with LAST_SLEPT and no valid golem spot in reach (no golem, no lock),
    /// and the positive control with one spot and a calm 4th villager (a golem, all 4 locked)
    GolemLock {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-12, ironspec: a cell-diff of a design's revision) Two design files'
    /// cells compared, overworld and Nether parts: every cell added, removed or changed, and
    /// whether each added cell was air (absent) before
    DesignDiff {
        old: std::path::PathBuf,
        new: std::path::PathBuf,
    },
    /// (tickexact 2026-11-12, ironspec: decision-critical) The stand's west end on the official
    /// server (dfb89499's station pad, 9fc3155f's two glass scarer posts: a pillager at (-5,-1,-5)
    /// and a zombie at (-5,-4,-2), both open east; the three summoners at their rest points):
    /// golems summoned on the pad's floor cells, each followed until it leaves the overworld (its
    /// positions, health, the pillager's crossbow, every arrow); again with zombified piglins in
    /// the portal cells beside each spawn as push sources; the control with the z -5 portal row
    /// blocked (the golem stays)
    PadGolems {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// The station file whose layers y -9..-4 are the pad (dfb89499's, on
        /// explore/farmopt-m1-restart d1a5f2985)
        #[arg(long, default_value = "designs/farms/iron/carousel-assembly/wip/station/f1q2-station.txt")]
        station: std::path::PathBuf,
    },
    /// (tickexact 2026-11-12, ironspec and farmopt) ironfarm's lane top slope (0,22,6) placed by
    /// the guide's order on the official server (glass on its high side, a temporary helper rail
    /// above it, the slope, the helper removed): its shape after each step, a neighbour update,
    /// glass where the helper stood, a ridden cart down it and a restart; the control with no
    /// helper settles flat
    RailSlope {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-12, ironspec: every rider's bed claim rests on it) A seated rider's
    /// CANT_REACH_WALK_TARGET_SINCE stamp and HOME on the official server, in onechunk's module
    /// (its bed): the brain agent logs every WALK_TARGET, stamp and HOME change with its source;
    /// (1) pocket dwells with the lead posts, (2) a calm lane control, (3) continuous panic with a
    /// pre-aged stamp
    SeatClear {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// The module file (onechunk's a9a418d03, wip/onechunk/module/f1q2-module-first-assembly.txt)
        #[arg(long, default_value = "designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-first-assembly.txt")]
        module: std::path::PathBuf,
        /// Row (1)'s dwells per lane
        #[arg(long, default_value_t = 200)]
        dwells: u32,
        /// Row (1)'s lanes, x:dwell:away (gt) each (dfb89499's recorded station at s 12, REL T+4:
        /// every lane rests 55 gt from landing to departure and is away 45 gt)
        #[arg(long, default_value = "-1:55:45,3:55:45,7:55:45", allow_hyphen_values = true)]
        lanes: String,
        /// The rows to run (any of 1, 2, 3)
        #[arg(long, default_value = "123")]
        rows: String,
        /// Row (1): the real departure's gt (the pocket's support powered, the cart rolling out on
        /// the module's rails with the scarers present; 0: none; dfb89499: 25 gt to the merge row)
        #[arg(long, default_value_t = 0)]
        depart: u32,
        /// Row (1): gt added to every turn (1: the cycle advances 1 gt a dwell against the 20-gt
        /// sensors, every offset sampled)
        #[arg(long, default_value_t = 0)]
        drift: u32,
        /// Row (1): glowstone floors laid first, x,y,z;x,y,z... (9fc3155f's extra stroll cells)
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        floors: String,
        /// Row (1): glass floors laid first (the control: a cell over glass is never a stable
        /// destination, so never drawn)
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        glass: String,
        /// Row (3)'s stamp age when the zombie comes (gt)
        #[arg(long, default_value_t = 1100)]
        preage: u32,
    },
    /// (tickexact 2026-11-12, ironspec's F3 inputs) Seats measured on the official server with
    /// the design's scarers (the lead pair, F3's napper post with `--post`) and glowstone floors:
    /// per seat name:x,y,z (a slope seat of lane x fills its pressed queue from R_1 (x,17,1)), until
    /// `--draws` stroll draws: P_null, q, the landed cells, MoveToTargetSink at the panic onset, the
    /// pillagers' charged ticks; `--rail-glow`: each seat rail's floor glowstone too
    /// (tickexact 2026-11-12, ironspec) A zombified piglin in the sealed pad's portal rows on the
    /// official server: portal-spawned (summoned with the cooldown a portal spawn sets) or crossed
    /// back from the Nether; per player distance (under 32, 32..128, over 128, in the Nether):
    /// where it is every 100 gt (inside a portal cell or not), its PortalCooldown, its despawn tick
    /// (tickexact 2026-11-12, ironspec: the continuous-loop term) One lane's queue as built on the
    /// official server (H plus the pressed slope, F3 laid): every `--every` gt the front cart out of
    /// H and its rider back at the top (ironfarm's traced entry: off C over the drop, stopped by the
    /// wall at z 6.49, falling onto the top slope from a standstill); per rest
    /// position P_null and the landed cells, per rider per lap panic ticks and clears, the sink at
    /// every panic onset, the riders' rest positions
    LaneRun {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-first-assembly.txt")]
        module: std::path::PathBuf,
        #[arg(long, default_value_t = 3, allow_hyphen_values = true)]
        x: i32,
        #[arg(long, default_value_t = 7)]
        riders: usize,
        #[arg(long, default_value_t = 4000)]
        ticks: u32,
        #[arg(long, default_value_t = 100)]
        every: u32,
        #[arg(long)]
        post: bool,
        /// Glowstone floors, x,y,z;...
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        glow: String,
        /// A part file laid over the module (9fc3155f's F3 scarer part: its glass, glowstone and
        /// seated pillager)
        #[arg(long)]
        part: Option<std::path::PathBuf>,
        /// The per-draw log's file (tick, rider cell, nearest hostile, target, outcome, path)
        #[arg(long)]
        draws_out: Option<std::path::PathBuf>,
    },
    /// (tickexact 2026-11-12, ironspec: H = 15 and DAY's thresholds) The controller's daylight
    /// detector at its cell in the composition (fluids flowed): the column over it, then its output
    /// every N gt over a clear, a rain and a thunder day
    DetectorInPlace {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, default_value = "")]
        parts: String,
        #[arg(long, allow_hyphen_values = true, default_value = "-5,15,3")]
        at: String,
        #[arg(long, default_value_t = 20)]
        every: u32,
    },
    /// (tickexact 2026-11-12, ironspec: PANIC's start erases an IDLE rider's entity walk) An IDLE
    /// villager in a cart with an entity walk or interaction target when a zombie comes into sight:
    /// its WALK_TARGET, PATH, LOOK_TARGET and INTERACTION_TARGET lines around PANIC's first tick
    PanicStart {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 3)]
        trials: u32,
        /// The gt each trial waits for an entity walk at most
        #[arg(long, default_value_t = 3000)]
        wait_max: u32,
    },
    /// (tickexact 2026-11-12, ironspec: the hold release) Does a villager seated in a cart trip a
    /// tripwire string over the cart's cell (at rest, rolling)? With an empty cart and a villager
    /// in the string's cell as the controls
    TripwireRider {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// The rolling rows' speed (b/gt)
        #[arg(long, default_value_t = 0.35)]
        speed: f64,
        #[arg(long, default_value_t = 40)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec's mirror rule: day-1's latch rows) A bench piece as stated
    /// and mirrored in z beside it, one drive script applied to both, each read cell of both copies
    /// every tick and the ticks where they differ
    MirrorBench {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        file: std::path::PathBuf,
        /// The mirrored copy's z' = MIRROR_AT - z
        #[arg(long, default_value_t = -40, allow_hyphen_values = true)]
        mirror_at: i32,
        /// t:x,y,z=STATE;... (STATE `air` for minecraft:air), in the stated copy's frame
        #[arg(long, allow_hyphen_values = true)]
        script: String,
        /// name:x,y,z:prop;... (prop a boolean property, or `shape`)
        #[arg(long, allow_hyphen_values = true)]
        reads: String,
        /// The script repeated shifted by each of these gt
        #[arg(long, default_value = "0")]
        phases: String,
        #[arg(long, default_value_t = 120)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, push-scan's bridge) The pairs' neighbourhood hashes of a composition
    /// from its files alone (no server); --list prints each cube's non-air cells
    BridgeHash {
        #[arg(long)]
        module: Option<std::path::PathBuf>,
        #[arg(long, default_value = "")]
        parts: String,
        /// x,y,z~x,y,z;...
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        pairs: String,
        #[arg(long)]
        list: bool,
        /// The hash's own checks (FNV vectors, the cube's order and bytes, ironfarm's lane 3 value)
        #[arg(long)]
        selftest: bool,
    },
    /// (tickexact 2026-11-12, ironspec: the queue post's sight lines) Villagers seated in hovering
    /// carts at given points; after the sensors ran, whether each saw a hostile (NEAREST_HOSTILE)
    SightCheck {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, default_value = "")]
        parts: String,
        /// Entities summoned after the parts, "kind x y z {nbt}" separated by ';;'
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        summon: String,
        /// name:x,y,z;... (a cart's position: the rider sits there)
        #[arg(long, allow_hyphen_values = true)]
        points: String,
        #[arg(long, default_value_t = 60)]
        ticks: u32,
        /// The villagers alone (moved by what moves them there, a bubble lift), not in hovering carts
        #[arg(long)]
        free: bool,
        /// Every tick: each villager's position, its distance to the nearest zombie, its lines
        #[arg(long)]
        trace: bool,
        /// Each point's villager summoned this many times (rides at every sensor phase)
        #[arg(long, default_value_t = 1)]
        repeat: u32,
        /// The gt between two rides of a point
        #[arg(long, default_value_t = 7)]
        every: u32,
        /// The gt each ride lives
        #[arg(long, default_value_t = 40)]
        life: u32,
    },
    /// (tickexact 2026-11-12, the row-draws run's out-of-frame riders) A ridden cart through the
    /// composition: every tick its position and motion, its rider, whether it still rides
    CartTrace {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, default_value = "")]
        parts: String,
        #[arg(long, allow_hyphen_values = true)]
        start: String,
        #[arg(long, allow_hyphen_values = true, default_value = "0,0,0")]
        motion: String,
        #[arg(long, default_value_t = 200)]
        ticks: u32,
        /// Stop after both have stood still this many gt
        #[arg(long, default_value_t = 20)]
        still: u32,
        /// Cells whose composed block is printed and tested in the game, "x,y,z;..."
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        probe: String,
    },
    /// (tickexact 2026-11-12, ironspec: the row fix's napper release) A ridden napper arriving into
    /// a siding from K's cell at each speed, then the siding's switch pulsed; every tick until it
    /// leaves the siding: the rest z, R powered, the cart moving, K reached and its speed there
    NapperRelease {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, default_value = "")]
        parts: String,
        /// K's cell, x,y,z (the napper starts there)
        #[arg(long, allow_hyphen_values = true, default_value = "1,25,9")]
        k: String,
        /// The siding switch's input cell, x,y,z (a redstone block there for --pulse gt)
        #[arg(long, allow_hyphen_values = true, default_value = "1,23,9")]
        switch: String,
        #[arg(long, default_value = "0.10,0.20,0.30,0.44")]
        arrivals: String,
        #[arg(long, default_value_t = 60)]
        wait: u32,
        #[arg(long, default_value_t = 30)]
        pulse: u32,
        #[arg(long, default_value_t = 4)]
        phases: u32,
        #[arg(long, default_value_t = 80)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironfarm via farmopt) The mirrored module's C on the official server:
    /// its placement orders (the helper unpowered; the unmirrored redstone-block recipe), C's shape
    /// after each step, D pressed and released, a restart
    MirrorC {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-12, onechunk via farmopt) Riders at lane 3's S-bend R, rolling on the
    /// split row and dropped onto the lane tops, on the official server; every stroll draw's
    /// target, its first path (complete or partial, its end) and WALK_TARGET's hold
    RowDraws {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, default_value = "")]
        parts: String,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        air: String,
        #[arg(long, default_value = "0.3,0.38")]
        row_speeds: String,
        #[arg(long, default_value_t = 100)]
        every: u32,
        #[arg(long, default_value_t = 2000)]
        ticks: u32,
        #[arg(long)]
        draws_out: Option<std::path::PathBuf>,
    },
    /// (tickexact 2026-11-12, ironspec and G1) The S-bend crossing margin on the official server:
    /// a ridden napper at rest on the siding's R, ridden through-carts entering K from the west at
    /// each speed; every tick the boxes' gaps, the push searches, speeds, the napper's movement
    Crossing {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        /// Part files laid over the module in order, a,b,c
        #[arg(long, default_value = "")]
        parts: String,
        /// Cells cleared to air after the parts, x,y,z;...
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        air: String,
        /// The napper carts at rest, x,y,z;x,y,z (one composition each)
        #[arg(long, allow_hyphen_values = true)]
        napper: String,
        /// The through-carts' start x offsets, a,b,c (one pass per offset and speed)
        #[arg(long, default_value = "0", allow_hyphen_values = true)]
        offsets: String,
        /// The through-carts' start, x,y,z
        #[arg(long, allow_hyphen_values = true)]
        through: String,
        /// The through-carts' speeds east, a,b,c (one pass each)
        #[arg(long, default_value = "0.3")]
        speeds: String,
        #[arg(long, default_value_t = 80)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec: split9's GO) The split junction on the official server: J a
    /// T at (0,25,9) driven by a repeater with a lever, D a detector ascending south under its north
    /// arm; presses and releases of D, the driver toggled, restarts; every tick J's and D's shapes
    JunctionProbe {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// The junction mirrored in z (the N = 4 southern modules)
        #[arg(long)]
        mirror: bool,
    },
    /// (tickexact 2026-11-12, ironspec: the bed stop's merge) A villager walking to its bed at night
    /// on the official server, `/time set 0` when it is `--at` blocks from the head; every tick to
    /// day time `--until`: activity, WALK_TARGET, PATH, position, asleep
    DayJump {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 2.0)]
        at: f64,
        #[arg(long, default_value_t = 40)]
        until: i64,
    },
    /// (tickexact 2026-11-12, ironspec and G1) The halt bridge on the official server: whether an
    /// unpowered powered rail holds a cart against another pressing it (rows hold-empty,
    /// hold-ridden, pair-empty, pair-ridden-V, flat-ridden), every tick both carts, their yaw, the
    /// boxes' vertical gap, the push search and its |cos|
    HaltBridge {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "hold-empty,hold-ridden,pair-empty,pair-ridden-0.106,pair-ridden-0.197,pair-ridden-0.332,pair-ridden-0.392,pair-ridden-0.459,flat-ridden")]
        rows: String,
        #[arg(long, default_value_t = 400)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec's ruling for onechunk's loader) The loader's freeze rows on
    /// the official server: the landed item-loop loader in chunk (1,1) with 1 or 4 farm modules
    /// around it, the player beyond simulation distance + 1 or in the Nether: (a) the loop running,
    /// per farm chunk its best ticket level, entity ticking, frozen block ticks; (b) a restart there
    FreezeRows {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "designs/farms/iron/carousel-assembly/wip/onechunk/loader/l1-item-loop-bench.txt")]
        loader: std::path::PathBuf,
        #[arg(long)]
        module: std::path::PathBuf,
        /// 1 or 4 modules
        #[arg(long, default_value_t = 1)]
        modules: u32,
        /// far (13 chunks off) or nether
        #[arg(long, default_value = "far")]
        place: String,
        /// Row (a)'s run (gt)
        #[arg(long, default_value_t = 2000)]
        ticks: u32,
        /// onechunk's rev 3 recipe: the Nether group file (kill room, storage, shield and the
        /// loader's twin), placed unmoved; the loader's own Nether part is then left out, the
        /// modules' pads kept, everything at `--pocket`
        #[arg(long)]
        group: Option<std::path::PathBuf>,
        /// Rev 3 at N = 4: the mirrored module file (the southern copies)
        #[arg(long)]
        mirrored: Option<std::path::PathBuf>,
        /// Rev 3: the pocket's y (the modules' frame + y; the loader 3 lower)
        #[arg(long, default_value_t = 110)]
        pocket: i32,
    },
    /// (tickexact 2026-11-12, ironspec) df472311b's queue fill on the strict-placed module, printed
    /// every tick on the official server: R_1's power, H's trapdoor, every cart and rider; the
    /// resting pitch
    QueueFill {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        /// Every cart ridden (else empty carts, the last ridden: df472311b's procedure)
        #[arg(long)]
        ridden: bool,
    },
    /// (tickexact 2026-11-12, ironspec) A seated villager's position minus its cart's on the
    /// official server, both read in the same tick, on a flat and an ascending rail, at rest and moving
    SeatOffset {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-12) The module placed both ways on the official server (rails with their
    /// onPlace, and `strict` as the file states them); 20 gt later every block that differs from
    /// the file
    ModuleCheck {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        /// Also every empty cell of the file's bounding box (strict placement): a fluid's flow
        #[arg(long)]
        with_air: bool,
    },
    PadPiglins {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// The sealed pad's station file (dfb89499's, explore/farmopt-m1-restart 4bce82b85)
        #[arg(long, default_value = "designs/farms/iron/carousel-assembly/wip/station/f1q2-station.txt")]
        station: std::path::PathBuf,
        #[arg(long, default_value_t = 3000)]
        ticks: u32,
    },
    SeatF3 {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-first-assembly.txt")]
        module: std::path::PathBuf,
        /// name:x,y,z;name:x,y,z...
        #[arg(long, allow_hyphen_values = true)]
        seats: String,
        #[arg(long)]
        post: bool,
        /// Glowstone floors, x,y,z;...
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        glow: String,
        #[arg(long)]
        rail_glow: bool,
        #[arg(long, default_value_t = 200)]
        draws: u32,
        /// Part files laid over the module in order, a,b,c (blocks as stated, entities summoned);
        /// each file's cells are checked against the world after everything is placed
        #[arg(long, default_value = "")]
        parts: String,
        /// The rider's cart at x,y,z (else the seat cell's centre 0.5 up)
        #[arg(long, allow_hyphen_values = true)]
        cart: Option<String>,
        /// Cells cleared to air after the parts, x,y,z;... (a part re-laid elsewhere)
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        air: String,
        /// Entities summoned after the parts, "kind x y z {nbt}" separated by ';;' (y in the
        /// design's frame)
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        summon: String,
        /// The post: the zombie nearest x,y,z, its distance to the rider printed at the start and
        /// the end, with the rider's flee targets (SetWalkTargetAwayFrom)
        #[arg(long, allow_hyphen_values = true)]
        watch: Option<String>,
        /// The run's tick cap (it also ends at --draws)
        #[arg(long, default_value_t = 6000)]
        max_ticks: u32,
    },
    /// in-chunk branch (a replica): J's shape through carts, a restart and a stone beside it, and
    /// T2 (face-adjacent to the detector) the positive control that re-settles
    RailT {
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
        /// No server: an open detector's level from 26.3's code at every day time, weather, a
        /// skip from a storm and the largest change between readings (day-1's inhibit level)
        #[arg(long)]
        levels: bool,
        /// On the official server: the detector through rain and thunder ramps at held day times
        /// and after one thunder skip, against the --levels model (G1's control)
        #[arg(long)]
        weather_row: bool,
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
        MobaiCmd::GolemLock { version } => crate::golemlock::golem_lock(&version),
        MobaiCmd::DesignDiff { old, new } => crate::designdiff::design_diff(&old, &new),
        MobaiCmd::PadGolems { version, station } => crate::padgolem::pad_golems(&version, &station),
        MobaiCmd::RailSlope { version } => crate::railslope::rail_slope(&version),
        MobaiCmd::SeatClear { version, module, dwells, lanes, rows, preage, depart, drift, floors, glass } => {
            let lanes: Vec<(i32, u32, u32)> = lanes
                .split(',')
                .map(|l| {
                    let v: Vec<&str> = l.split(':').collect();
                    anyhow::ensure!(v.len() == 3, "--lanes: x:dwell:away, got {l}");
                    Ok((v[0].trim().parse()?, v[1].trim().parse()?, v[2].trim().parse()?))
                })
                .collect::<anyhow::Result<_>>()?;
            let cells = |s: &str| -> anyhow::Result<Vec<[i32; 3]>> {
                s.split(';')
                    .filter(|f| !f.trim().is_empty())
                    .map(|f| {
                        let v: Vec<i32> = f.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 3, "x,y,z;..., got {f}");
                        Ok([v[0], v[1], v[2]])
                    })
                    .collect()
            };
            crate::seatclear::seat_clear(&version, &module, &rows, &lanes, dwells, preage, depart, drift, &cells(&floors)?, &cells(&glass)?)
        }
        MobaiCmd::LaneRun { version, module, x, riders, ticks, every, post, glow, part, draws_out } => {
            let glow: Vec<[i32; 3]> = glow
                .split(';')
                .filter(|f| !f.trim().is_empty())
                .map(|f| {
                    let v: Vec<i32> = f.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--glow x,y,z;..., got {f}");
                    Ok([v[0], v[1], v[2]])
                })
                .collect::<anyhow::Result<_>>()?;
            crate::lanerun::lane_run(&version, &module, x, riders, ticks, every, post, &glow, part.as_deref(), draws_out.as_deref())
        }
        MobaiCmd::DetectorInPlace { version, module, parts, at, every } => {
            let v: Vec<i32> = at.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "--at x,y,z, got {at}");
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            crate::detectorinplace::detector_in_place(&version, &module, &parts, [v[0], v[1], v[2]], every)
        }
        MobaiCmd::PanicStart { version, trials, wait_max } => crate::panicstart::panic_start(&version, trials, wait_max),
        MobaiCmd::TripwireRider { version, speed, ticks } => crate::tripwirerider::tripwire_rider(&version, speed, ticks),
        MobaiCmd::MirrorBench { version, file, mirror_at, script, reads, phases, ticks } => {
            let cell = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let script: Vec<crate::mirrorbench::Event> = script
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    let (t, rest) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--script t:x,y,z=STATE, got {s}"))?;
                    let (c, state) = rest.split_once('=').ok_or_else(|| anyhow::anyhow!("--script t:x,y,z=STATE, got {s}"))?;
                    Ok(crate::mirrorbench::Event { t: t.trim().parse()?, p: cell(c)?, state: state.trim().to_string() })
                })
                .collect::<anyhow::Result<_>>()?;
            let reads: Vec<crate::mirrorbench::Read> = reads
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    let f: Vec<&str> = s.split(':').collect();
                    anyhow::ensure!(f.len() == 3, "--reads name:x,y,z:prop, got {s}");
                    Ok(crate::mirrorbench::Read { name: f[0].to_string(), p: cell(f[1])?, prop: f[2].to_string() })
                })
                .collect::<anyhow::Result<_>>()?;
            let phases: Vec<u32> = phases.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::mirrorbench::mirror_bench(&version, &file, mirror_at, &script, &reads, &phases, ticks)
        }
        MobaiCmd::BridgeHash { module, parts, pairs, list, selftest } => {
            if selftest {
                return crate::bridgehash::checks::all();
            }
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            let cell = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let pairs: Vec<([i32; 3], [i32; 3])> = pairs
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    let (a, b) = s.split_once('~').ok_or_else(|| anyhow::anyhow!("--pairs x,y,z~x,y,z;..., got {s}"))?;
                    Ok((cell(a)?, cell(b)?))
                })
                .collect::<anyhow::Result<_>>()?;
            let module = module.ok_or_else(|| anyhow::anyhow!("--module is required (or --selftest)"))?;
            crate::bridgehash::bridge_hash_files(&module, &parts, &pairs, list)
        }
        MobaiCmd::SightCheck { version, module, parts, summon, points, ticks, free, trace, repeat, every, life } => {
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            let summon: Vec<String> = summon.split(";;").map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
            let points: Vec<(String, [f64; 3])> = points
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    let (name, c) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--points name:x,y,z;..., got {s}"))?;
                    let v: Vec<f64> = c.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--points name:x,y,z;..., got {s}");
                    Ok((name.to_string(), [v[0], v[1], v[2]]))
                })
                .collect::<anyhow::Result<_>>()?;
            crate::sightcheck::sight_check(&version, &module, &parts, &summon, &points, &crate::sightcheck::SightOpts { ticks, free, trace, repeat, every, life })
        }
        MobaiCmd::CartTrace { version, module, parts, start, motion, ticks, still, probe } => {
            let f3v = |s: &str, what: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--{what} x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            let probe: Vec<[i32; 3]> = probe
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--probe x,y,z;..., got {s}");
                    Ok([v[0], v[1], v[2]])
                })
                .collect::<anyhow::Result<_>>()?;
            crate::carttrace::cart_trace(&version, &module, &parts, f3v(&start, "start")?, f3v(&motion, "motion")?, ticks, still, &probe)
        }
        MobaiCmd::NapperRelease { version, module, parts, k, switch, arrivals, wait, pulse, phases, ticks } => {
            let i3 = |s: &str, what: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--{what} x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            let arrivals: Vec<f64> = arrivals.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::napperrelease::napper_release(&version, &module, &parts, i3(&k, "k")?, i3(&switch, "switch")?, &arrivals, wait, pulse, phases, ticks)
        }
        MobaiCmd::MirrorC { version } => crate::mirrorc::mirror_c(&version),
        MobaiCmd::RowDraws { version, module, parts, air, row_speeds, every, ticks, draws_out } => {
            let air: Vec<[i32; 3]> = air
                .split(';')
                .filter(|f| !f.trim().is_empty())
                .map(|f| {
                    let v: Vec<i32> = f.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--air x,y,z;..., got {f}");
                    Ok([v[0], v[1], v[2]])
                })
                .collect::<anyhow::Result<_>>()?;
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            let speeds: Vec<f64> = row_speeds.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::rowdraws::row_draws(&version, &module, &parts, &air, &speeds, every, ticks, draws_out.as_deref())
        }
        MobaiCmd::Crossing { version, module, parts, air, napper, offsets, through, speeds, ticks } => {
            let f3 = |s: &str, what: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--{what} x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let air: Vec<[i32; 3]> = air
                .split(';')
                .filter(|f| !f.trim().is_empty())
                .map(|f| {
                    let v: Vec<i32> = f.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--air x,y,z;..., got {f}");
                    Ok([v[0], v[1], v[2]])
                })
                .collect::<anyhow::Result<_>>()?;
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            let speeds: Vec<f64> = speeds.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let nappers: Vec<([f64; 3], f64)> = napper
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3 || v.len() == 4, "--napper x,y,z[,vz];..., got {s}");
                    Ok(([v[0], v[1], v[2]], v.get(3).copied().unwrap_or(0.0)))
                })
                .collect::<anyhow::Result<_>>()?;
            let offsets: Vec<f64> = offsets.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::crossing::crossing(&version, &module, &parts, &air, &nappers, f3(&through, "through")?, &offsets, &speeds, ticks)
        }
        MobaiCmd::JunctionProbe { version, mirror } => crate::junction::junction_probe(&version, mirror),
        MobaiCmd::DayJump { version, at, until } => crate::dayjump::day_jump(&version, at, until),
        MobaiCmd::HaltBridge { version, rows, ticks } => crate::haltbridge::halt_bridge(&version, &rows, ticks),
        MobaiCmd::FreezeRows { version, loader, module, modules, place, ticks, group, mirrored, pocket } => crate::freeze::freeze_rows(&version, &loader, &module, modules, &place, ticks, group.as_deref(), mirrored.as_deref(), pocket),
        MobaiCmd::QueueFill { version, module, ridden } => crate::seatclear::queue_fill(&version, &module, ridden),
        MobaiCmd::SeatOffset { version } => crate::seatoffset::seat_offset(&version),
        MobaiCmd::ModuleCheck { version, module, with_air } => crate::seatclear::module_check(&version, &module, with_air),
        MobaiCmd::PadPiglins { version, station, ticks } => crate::padpiglin::pad_piglins(&version, &station, ticks),
        MobaiCmd::SeatF3 { version, module, seats, post, glow, rail_glow, draws, parts, cart, air, summon, watch, max_ticks } => {
            let cells = |s: &str| -> anyhow::Result<Vec<[i32; 3]>> {
                s.split(';')
                    .filter(|f| !f.trim().is_empty())
                    .map(|f| {
                        let v: Vec<i32> = f.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 3, "x,y,z;..., got {f}");
                        Ok([v[0], v[1], v[2]])
                    })
                    .collect()
            };
            let glow = cells(&glow)?;
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            let cart: Option<[f64; 3]> = match cart {
                Some(c) => {
                    let v: Vec<f64> = c.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--cart x,y,z, got {c}");
                    Some([v[0], v[1], v[2]])
                }
                None => None,
            };
            for s in seats.split(';').filter(|s| !s.trim().is_empty()) {
                let (name, c) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--seats name:x,y,z[@cx,cy,cz], got {s}"))?;
                // a seat may carry its own cart position: name:x,y,z@cx,cy,cz
                let (c, own_cart) = match c.split_once('@') {
                    Some((cell, at)) => {
                        let v: Vec<f64> = at.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 3, "--seats name:x,y,z@cx,cy,cz, got {s}");
                        (cell, Some([v[0], v[1], v[2]]))
                    }
                    None => (c, None),
                };
                let seat = cells(c)?.first().copied().ok_or_else(|| anyhow::anyhow!("--seats name:x,y,z, got {s}"))?;
                // a lane slope seat (x,17+i,1+i): its queue from R_1 up to the cell ahead of it
                let fill: Vec<[i32; 3]> = if (17..=22).contains(&seat[1]) && seat[2] == seat[1] - 16 { (17..seat[1]).map(|y| [seat[0], y, y - 16]).collect() } else { Vec::new() };
                let extra = crate::seatclear::SeatExtra {
                    summon: summon.split(";;").map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect(),
                    watch: match &watch {
                        Some(w) => {
                            let v: Vec<f64> = w.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                            anyhow::ensure!(v.len() == 3, "--watch x,y,z, got {w}");
                            Some([v[0], v[1], v[2]])
                        }
                        None => None,
                    },
                    max_ticks,
                };
                crate::seatclear::f3_seat_parts(&version, &module, name, seat, &fill, post, &glow, rail_glow, draws, &parts, own_cart.or(cart), &cells(&air)?, &extra)?;
            }
            Ok(())
        }
        MobaiCmd::RailT { version } => crate::railt::rail_t(&version),
        MobaiCmd::ThreeBeds { version } => crate::threebeds::three_beds(&version),
        MobaiCmd::Daylight { version, latency, levels, weather_row } => {
            if levels {
                crate::daylight::levels()
            } else if weather_row {
                crate::daylight::weather_row(&version)
            } else if latency {
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
