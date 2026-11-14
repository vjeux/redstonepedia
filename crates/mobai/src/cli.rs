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
    /// (tickexact 2026-11-12, ironspec 18:59 (2b)) A lever-switched rail T: the latest switch that
    /// still routes an arriving cart, the earliest that leaves a departing one undisturbed
    TSwitch {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "0.3,0.35,0.4")]
        speeds: String,
        /// lever | repeater | detector-after | detector-before (the switch's cause and phase)
        #[arg(long, default_value = "lever")]
        cause: String,
        /// Trigger offsets from A (the first tick the arriving cart ends in the T's cell), a,b
        #[arg(long, allow_hyphen_values = true, default_value = "-3,-2,-1,0,1,2")]
        arrive: String,
        /// Trigger offsets from D (the last tick the cart ends in the cell)
        #[arg(long, allow_hyphen_values = true, default_value = "-1,0,1,2,3")]
        depart: String,
        #[arg(long, default_value_t = 60)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec 20:05 (1b)) Two ridden carts through the lift's side slope and
    /// exit top, a gap apart: box contacts and disturbances
    LiftPair {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        /// The cart's rest on the braked side slope
        #[arg(long, allow_hyphen_values = true, default_value = "-3.5,12.7,7.5")]
        slope: String,
        #[arg(long, allow_hyphen_values = true, default_value = "-4,12,6")]
        release_cell: String,
        #[arg(long, default_value_t = 20)]
        release_at: u32,
        #[arg(long, default_value = "10,11,12,13,14,15,16,18")]
        gaps: String,
        #[arg(long, default_value_t = 200)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec 19:20/19:34 PT) Two calm riders in adjacent carts: gossip and
    /// lastGossipTime over a restart
    GossipRow {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// side | line | standing (the positive control: two villagers on foot in a glass pen)
        #[arg(long, default_value = "side")]
        layout: String,
        #[arg(long, default_value_t = 100)]
        day_time: i64,
        #[arg(long, default_value_t = 2400)]
        ticks: u32,
        #[arg(long)]
        restart_after: Option<u32>,
    },
    /// (tickexact 2026-11-12, ironspec 20:16 (4)) The station's official ride: each lane's departure,
    /// merge-row entry speed, column entry and y-12 pass
    StationRide {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        lift: std::path::PathBuf,
        #[arg(long)]
        station: std::path::PathBuf,
        #[arg(long, default_value_t = 60)]
        rel: u32,
        #[arg(long, default_value = "0,12,24")]
        offsets: String,
        #[arg(long, default_value = "")]
        restarts: String,
        #[arg(long, default_value_t = 200)]
        ticks: u32,
        /// Lane 7's side-slope release: a redstone block at (-4,12,6) at REL + a for b gt, a,b
        #[arg(long, default_value = "")]
        lane7_release: String,
    },
    /// (tickexact 2026-11-12, ironspec 20:53 PT (1)) A6's restart trap: the bed stop with its
    /// run-out, a ridden cart from the entry at v, a restart before tick T: the eject, the stall
    /// after it, the lie-down
    BedTrap {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "designs/farms/iron/parts/bed-tier-min/bed-tier-fold-bench.txt")]
        file: std::path::PathBuf,
        /// Cells set over the file after it is pasted (local frame), "x,y,z=STATE;..." (a candidate
        /// such as T5b as cell changes on the bench file)
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        set: String,
        #[arg(long, default_value = "0.13,0.15,0.175")]
        speeds: String,
        /// T: a restart between ticks T-1 and T, a,b,c
        #[arg(long, default_value = "1,2,3,4,5,6")]
        restarts: String,
        #[arg(long, default_value_t = 1)]
        repeats: u32,
        #[arg(long, default_value_t = 500)]
        ticks: u32,
        /// The real approach (ironspec 23:19 PT): the ridden cart at rest here (the file's frame)
        /// instead of the entry summon, x,y,z
        #[arg(long, allow_hyphen_values = true)]
        approach: Option<String>,
        /// With --approach: the release block's cell and its tick (10 gt), x,y,z,t
        #[arg(long, allow_hyphen_values = true, default_value = "8,-14,-4,40")]
        release: String,
        /// With --approach: the napper's HOME (the file's frame)
        #[arg(long, allow_hyphen_values = true, default_value = "-3,-27,3")]
        home: String,
        /// With --approach: a cell (the file's frame) whose block is printed at the napper's first
        /// panic (a trapdoor's open state: day-1's seat hatch (6,-12,-5))
        #[arg(long, allow_hyphen_values = true)]
        watch: Option<String>,
        /// With --approach: the release is the --set lever at the --release cell turned off (record7's
        /// built R1: its NOT-ready lever, ironspec 00:24 PT), not a redstone block
        #[arg(long)]
        release_lever: bool,
        /// Restart rows stop at the first held attempt (no moved one sought)
        #[arg(long)]
        held_only: bool,
        /// With --approach: the file's own ridden cart nearest the start is the napper's (its rider
        /// given HOME and the tags), so the bench's entities stay the record's
        #[arg(long)]
        file_cart: bool,
        /// Day (day time 1000) instead of night
        #[arg(long)]
        day: bool,
        /// With --file-cart: move that cart here (at rest) once tagged, x,y,z
        #[arg(long, allow_hyphen_values = true)]
        cart_pos: Option<String>,
        /// With --approach (a summoned cart): its starting x velocity
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        start_vx: f64,
        /// With --approach: remove the file's cart (and rider) within 1 of x,y,z first (the seat
        /// emptied for an arriving napper)
        #[arg(long, allow_hyphen_values = true)]
        clear: Option<String>,
        /// Another design's cells laid over the file inside --overlay-box (before --set): e.g.
        /// record7's built R1 over a way-in package that lacks it
        #[arg(long)]
        overlay: Option<std::path::PathBuf>,
        /// x0,y0,z0,x1,y1,z1 (the file's frame)
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        overlay_box: String,
    },
    /// (tickexact 2026-11-13, G1 / ironspec 03:36 PT: a cheap confirmation) A seated villager and a
    /// zombie in a boat 4 east; the column the eye-to-eye ray crosses: open, glass, solid, a gap
    /// (tickexact 2026-11-13, ironspec 06:31 real msg-id) The official (A)-geometry re-claim row: a
    /// napper armed with a stamp, stood by day on the bed foot's top, released at dusk; every
    /// AcquirePoi attempt's path and the re-claim
    BedReclaim {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        file: std::path::PathBuf,
        /// Cells set to air first, x,y,z;... (each must change)
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        remove: String,
        #[arg(long, default_value = "-2.5,0.5625,3.5", allow_hyphen_values = true)]
        day_pos: String,
        #[arg(long, default_value = "-3,0,4", allow_hyphen_values = true)]
        home: String,
        #[arg(long, default_value = "-2.5,0,30.5", allow_hyphen_values = true)]
        arm_at: String,
        #[arg(long, default_value_t = 2)]
        runs: u32,
        /// gt after the dusk
        #[arg(long, default_value_t = 2000)]
        after: u32,
        /// (G1 on G #3) the expected outcome: reclaimed | not-reclaimed (each run barred)
        #[arg(long)]
        expect: Option<String>,
        /// with --expect reclaimed: the re-claim within this many gt of the release
        #[arg(long, default_value_t = 100)]
        within: u32,
        /// with --expect reclaimed: the napper asleep too
        #[arg(long)]
        asleep: bool,
    },
    /// (tickexact 2026-11-13, ironspec 06:08 real msg-id: the composed night, live controller) onechunk's
    /// f1q2-run act list (`--drive-out`) replayed open loop over its scene (`--scene-out`)
    DriveReplay {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        scene: std::path::PathBuf,
        #[arg(long)]
        drive: std::path::PathBuf,
        #[arg(long, default_value_t = 14200)]
        ticks: u32,
        /// The day time at the drive's t = 0: the drive's own `# day0`, or this; both disagreeing or neither
        /// given is refused (G1 17:00: no silent default)
        #[arg(long)]
        day_start: Option<i64>,
        /// The bed head, x,y,z (frame)
        #[arg(long, default_value = "-3,-27,3", allow_hyphen_values = true)]
        bed: String,
        /// World opens (save, stop, start) before these ticks, comma-separated
        #[arg(long, default_value = "")]
        loads: String,
    },
    /// (farmopt 13:13, day-1's x4 re-form) a glass block one cell above a row rail: ridden carts passing
    /// under it straight and turning under it (east -> north), beside no-glass controls
    RowGlass {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "0.35,0.60,0.05")]
        speeds: String,
        #[arg(long, value_delimiter = ',', default_value = "0.00,0.11,0.23,0.37")]
        phases: Vec<f64>,
        #[arg(long, value_delimiter = ',', default_value = "0.45,0.60")]
        empties: Vec<f64>,
    },
    /// (G1 12:55) the row strings: carts rolling east under a north-south string one cell (A) and two
    /// cells (B) above a flat rail line, at stored speeds x phases, ridden and empty
    RowString {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// ridden stored speeds: from,to,step
        #[arg(long, default_value = "0.35,0.60,0.01")]
        speeds: String,
        #[arg(long, value_delimiter = ',', default_value = "0.00,0.11,0.23,0.37")]
        phases: Vec<f64>,
        /// empty-cart stored speeds
        #[arg(long, value_delimiter = ',', default_value = "0.30,0.45,0.60")]
        empties: Vec<f64>,
    },
    /// (tickexact 2026-11-13, ironspec 12:38 (2)) the launch dense sweep: nappers on F at stored speeds x
    /// entry phases, braked on R, released, their entry into the drop and the outcome (fall or stall)
    LaunchSweep {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        scene: std::path::PathBuf,
        /// stored speeds: from,to,step
        #[arg(long, default_value = "0.25,0.45,0.01")]
        speeds: String,
        /// entry phases: z positions on F
        #[arg(long, value_delimiter = ',', default_value = "8.50,8.62,8.74,8.86")]
        phases: Vec<f64>,
        #[arg(long)]
        sbend_straight: bool,
        /// the launch bar (rest z) the margins are quoted against
        #[arg(long, default_value_t = 7.29)]
        bar: f64,
        /// print every lane's position and motion each tick of the approach
        #[arg(long)]
        trace: bool,
    },
    /// (cc767b78 14:47 / 15:20, ironspec 14:49) a lie-down on the stop's own cells (58f770cc's bench file):
    /// `eject` (set down by E from a southbound cart) or `hurt` (hurt on P, lying down when its panic ends);
    /// the lie-down tick's move, the settled position, then the park released with the sleeper asleep
    SleepStop {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        bench: std::path::PathBuf,
        #[arg(long, default_value = "eject")]
        mode: String,
        /// the napper's cart's stored speed heading south (eject)
        #[arg(long, default_value_t = 0.3)]
        v: f64,
        #[arg(long, default_value_t = 200)]
        release_after: u32,
        #[arg(long, default_value_t = 200)]
        watch: u32,
        /// (G1 15:47) wake the sleeper with this yaw (deg) set and print its stand-up cell, then end
        #[arg(long, allow_hyphen_values = true)]
        wake_yaw: Option<f32>,
    },
    /// (G1 13:37) does a minecart passing P beside the bed seat a sleeping villager? (the stop's local cells)
    SleepPickup {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// the cart's stored speed heading north on the approach
        #[arg(long, default_value_t = 0.3)]
        v: f64,
        #[arg(long, default_value_t = 40)]
        settle: u32,
        #[arg(long, default_value_t = 80)]
        watch: u32,
        /// the stop's own approach: the napper walks in from the eject north of P, the cart comes west into
        /// P and turns south (58f770cc 13:46)
        #[arg(long)]
        stop: bool,
        #[arg(long, default_value_t = 1200)]
        max_wait: u32,
    },
    /// (tickexact 2026-11-13, day-1 12:27) the reverse merge: a villager lies down in a bed watched by an
    /// observer; at the lie-down + k it is hurt or a hostile appears in view (or nothing); the observer's
    /// pulses, the wake, the villager's path; `--guard` adds a string in the cell above the bed head
    SleepMerge {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// none | damage | hostile
        #[arg(long, default_value = "none")]
        mode: String,
        #[arg(long, default_value_t = 0)]
        k: u32,
        #[arg(long, default_value_t = 150)]
        watch: u32,
        #[arg(long)]
        guard: bool,
        #[arg(long, default_value_t = 1200)]
        max_wait: u32,
    },
    /// (tickexact 2026-11-13, ironspec 10:52) the presence-string row: a tripwire string two blocks above
    /// a resting rail (the y-26 form over R), (A) a cart with a seated villager resting under it, (B)
    /// an empty cart under its own string; the strings per tick, a world open, the departure
    PresenceString {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 300)]
        ticks: u32,
        #[arg(long, default_value_t = 100)]
        open_at: u32,
        #[arg(long, default_value_t = 200)]
        depart_at: u32,
        /// The string's height above R (2 = the y-26 form over R at y 24)
        #[arg(long, default_value_t = 2)]
        rise: i32,
        /// The design's R (an unpowered powered rail ascending south on slime, the napper rolled in
        /// from F or placed at its measured rests) instead of a flat R
        #[arg(long)]
        slope: bool,
    },
    /// (tickexact 2026-11-13, G1 10:36) the drop-entry envelope from drive-replay outputs with `+move`
    /// rows (MCTAS_REPLAY_MOVE over the drop region, unclipped): per lane (x0 = -1 / 3 / 7), every
    /// cart pass through the drop with its off-rail ticks on C's support's top (box at y 24.000) and its
    /// largest lateral displacement from the lane's centre, then N, min / median / max of each
    DropEnvelope {
        /// drive-replay outputs (one per sample)
        files: Vec<std::path::PathBuf>,
    },
    /// (tickexact 2026-11-13, ironspec 10:38 (9): the BUILT chunk phase) a drive-replay scene and drive
    /// translated by --by dx,dy,dz (blocks, block entities, ports, entities; the drive's `home` and
    /// `set` acts); `# expect` lines are dropped (their states are in the old frame: the shifted drive
    /// runs open loop, MCTAS_REPLAY_FIDELITY_ONLY); pass the bed shifted too
    ShiftScene {
        #[arg(long)]
        scene: std::path::PathBuf,
        #[arg(long)]
        drive: std::path::PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        by: String,
        #[arg(long)]
        scene_out: std::path::PathBuf,
        #[arg(long)]
        drive_out: std::path::PathBuf,
    },
    SightControl {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// open, glass, solid, gap, comma-separated
        #[arg(long, default_value = "open,glass,solid,gap")]
        cases: String,
        #[arg(long, default_value_t = 60)]
        ticks: u32,
    },
    /// (tickexact 2026-11-13, ironspec 02:27 PT: B3) A bot in bed at the save, the world closed
    /// (a restart with the bot connected, save-and-quit, a crash) and opened again: whether a player
    /// is asleep at its first tick, and the night skip against the first post-load lie-down
    PlayerSleep {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// restart, quit, crash, comma-separated
        #[arg(long, default_value = "restart,quit,crash")]
        how: String,
    },
    /// (tickexact 2026-11-13, ironspec 01:46 PT (a)) Official replays of the rail-shape builder
    /// recipes (C = (x,24,7), D = (x,24,8), c3 = (x-1,24,7)): the test recipe and the two controls
    RailSteps {
        #[arg(long, default_value = "26.3")]
        version: String,
        file: std::path::PathBuf,
        /// Lanes x, comma-separated
        #[arg(long, default_value = "-1,3,7", allow_hyphen_values = true)]
        lanes: String,
        /// test, ctl-i, ctl-ii or all
        #[arg(long, default_value = "all")]
        case: String,
        /// The power cell as dx,y,z (x relative to the lane); default (x,25,7) above C
        #[arg(long, allow_hyphen_values = true)]
        power: Option<String>,
        /// A free-form replay instead: "empty x,y,z; set x,y,z=STATE; air x,y,z; wait N; expect
        /// x,y,z=SHAPE; count x,y,z; …"
        #[arg(long, allow_hyphen_values = true)]
        script: Option<String>,
        /// With --script: more cells to print after every step, "x,y,z;…"
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        watch: String,
    },
    /// (tickexact 2026-11-13, ironspec 00:27 PT: rules B, C, E) A design pasted with its fluid
    /// cells' ticks scheduled, run, saved and read back: every cell whose water left its stored
    /// state (new water fails the run)
    WaterSettle {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// The design (omit with --controls)
        file: Option<std::path::PathBuf>,
        /// Bench cells laid after the paste, "x,y,z=STATE;…"
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        set: String,
        #[arg(long, default_value_t = 200)]
        ticks: u32,
        /// The two built-in controls (a sourceless flowing cell must fail LOST, a bubble column
        /// stored with the wrong drag must fail CHANGED), through the same code path
        #[arg(long)]
        controls: bool,
    },
    /// (tickexact 2026-11-13, ironspec 00:00 PT) GOLEM_DETECTED_RECENTLY's ttl across a world open:
    /// the first golem at T, a restart at T + `--restart-after`, the next golem's tick from T
    GolemTtl {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 300)]
        restart_after: u32,
        #[arg(long, default_value_t = 2)]
        runs: u32,
        #[arg(long, default_value_t = 2400)]
        ticks: u32,
        /// The boundary rows (ironspec 00:30 PT): the trio's memories set at T + 100 to clear this
        /// many ticks before the slot T + 400, the restart (if any) at T + 200
        #[arg(long)]
        boundary: Option<u32>,
    },
    /// (tickexact 2026-11-12, ironspec 22:29 PT (2)) Entity, block-entity and block ticks counted by
    /// in-game command blocks at 20 TPS across world opens and logins with load
    LoadCount {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 8)]
        cases: u32,
        /// Real milliseconds the server runs after each load before the counters are read
        #[arg(long, default_value_t = 4000)]
        run_ms: u64,
        /// (ironspec 13:37) count on this scene's own chunks with its entities (the farm-chunk k / j)
        #[arg(long)]
        scene: Option<std::path::PathBuf>,
    },
    /// (tickexact 2026-11-12, ironspec 19:51 PT) VillagerPanicTrigger's start/tick calls every tick
    /// on continuously panicking villagers: the 60-gt behaviour's skip against the summon ticks
    PanicTicks {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 3)]
        villagers: u32,
        #[arg(long, default_value_t = 700)]
        ticks: u32,
        #[arg(long)]
        restart_at: Option<u32>,
    },
    /// (tickexact 2026-11-12, ironspec 19:52 (2)) The S-bend curve bar's boundary row (lane 3)
    SbendBoundary {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long)]
        lanes: std::path::PathBuf,
        #[arg(long, allow_hyphen_values = true, default_value = "1.5,24.63,7.57")]
        napper: String,
        #[arg(long, default_value_t = 0.36)]
        leader_v: f64,
        #[arg(long, allow_hyphen_values = true, default_value = "-6,-5,-4,-3,-2,-1")]
        offsets: String,
        #[arg(long, default_value_t = 120)]
        ticks: u32,
        /// The leader first in the entity list (parked, teleported in at its start)
        #[arg(long)]
        leader_first: bool,
        /// The napper rolling instead of at rest: summoned at P (the leader's summon tick, t10),
        /// before the leader, with this state (after P - 1): x,y,z,vx,vy,vz
        #[arg(long, allow_hyphen_values = true)]
        napper_roll: Option<String>,
    },
    /// (tickexact 2026-11-12, ironspec 18:46) A ridden cart down dry vs waterlogged slopes and flats
    WaterRails {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 6)]
        slopes: i32,
        #[arg(long, default_value_t = 12)]
        flat: i32,
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        #[arg(long, default_value_t = 200)]
        ticks: u32,
        /// Every tick's position and motion (t, x, y, z, vx, vy, vz, cell) for both rides
        #[arg(long)]
        every_tick: bool,
        /// Restart rows instead (dry slopes, wet flats): one save/stop/start at each listed tick
        #[arg(long, default_value = "")]
        restarts: String,
        /// With --restarts: the speed where the cart leaves this many flats
        #[arg(long, default_value_t = 5)]
        measure: i32,
        /// With --restarts: attempts continue (up to 16) until an attempt whose cart moved across
        /// the load (uncounted harness ticks, a harness artefact) is caught beside the held row, and
        /// the two are compared by world tick
        #[arg(long)]
        both_branches: bool,
    },
    /// (tickexact 2026-11-13, ironspec 21:42 / 22:07 / 22:58) (g)'s latch alone from 58f770cc's block list (the control)
    LatchRig {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// (day-1 23:37) rig day-1's form (b) block list (the positive) instead of the as-listed control
        #[arg(long)]
        form_b: Option<std::path::PathBuf>,
    },
    /// (tickexact 2026-11-13, ironspec 22:12 / G1 22:15) A villager seated in a minecart dropped onto rails, slime, stone, water
    CartFall {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-13, ironspec 21:42) A real P_STRING's release on a test rig: press vs the observer's pulse
    StringRelease {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-13, G1 21:25 (2)) (g)'s w = 2 boundary: the abutting handover into a torch's read
    AbutLatch {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-13, ironspec 20:41) An empty cart from the napper lift over E under (E)'s string
    EString {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// the rev 3 record (record7f13b)
        #[arg(long)]
        record: std::path::PathBuf,
    },
    /// (tickexact 2026-11-13, ironspec 20:31) An empty cart at rest on R_1's slope under the slot-1 string
    SlopeString {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-13, ironspec 19:03 (2)) The hopper / dropper / comparator timings on the game
    HopperPin {
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// (tickexact 2026-11-12, ironspec 16:29 (c)) Watched cells' states every tick over restarts
    BlockWatch {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        set: String,
        /// "x,y,z=STATE;..."
        #[arg(long, allow_hyphen_values = true)]
        watch: String,
        #[arg(long, default_value = "")]
        restarts: String,
        #[arg(long, default_value_t = 200)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec 16:29 (b)) A villager on A6's second landing chest, its walk
    /// target forced north of the trapdoor's plate: does it cross?
    OnewayCross {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        design: std::path::PathBuf,
        /// Part files laid over the design, "file@dx,dy,dz;..."
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        parts: String,
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        set: String,
        #[arg(long, allow_hyphen_values = true, default_value = "3.5,-24.125,-3.5")]
        at: String,
        #[arg(long, allow_hyphen_values = true, default_value = "3,-24,-6;3,-25,-6;2,-24,-7;4,-24,-7;3,-23,-6;3,-25,-5")]
        targets: String,
        #[arg(long, default_value_t = 120)]
        tries: u32,
        #[arg(long, default_value_t = 40)]
        ticks: u32,
        #[arg(long, allow_hyphen_values = true, default_value_t = -4.0)]
        north_z: f64,
        #[arg(long, default_value_t = 0.6)]
        speed: f64,
        #[arg(long, default_value_t = 1000)]
        day_time: i64,
    },
    /// (tickexact 2026-11-12, ironspec 16:04: path (a)) The napper from the hold's fired pulse
    /// through the T, the merge row's ramp and the lift to its y-12 pass
    HoldPulse {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, allow_hyphen_values = true, default_value = "9.5,0.0625,8.51")]
        rest: String,
        #[arg(long, allow_hyphen_values = true, default_value = "7,-1,8")]
        pulse_cell: String,
        /// The pulse's ticks, one start phase each, a,b,c
        #[arg(long, default_value = "40,41,42,43")]
        pulse_at: String,
        #[arg(long, default_value_t = 12)]
        pulse_len: u32,
        #[arg(long, default_value_t = 12.0)]
        pass_y: f64,
        /// Restart rows at these ticks after the rise, a,b
        #[arg(long, default_value = "")]
        restarts: String,
        #[arg(long, default_value_t = 300)]
        max_ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec 16:36) Golems per summon chance on a pad of `--pad` (on glass)
    GolemPad {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "minecraft:cobblestone")]
        pad: String,
        /// The layer under the pad (the glass floor's cells there)
        #[arg(long, default_value = "minecraft:glass")]
        under: String,
        #[arg(long, default_value_t = 16)]
        stands: u32,
        #[arg(long, default_value_t = 3000)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec 16:17: the rail-node rule) The game's createPath for a
    /// villager riding a cart held at each position, toward each target (the bridge's path_nodes)
    PathProbeGame {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        design: std::path::PathBuf,
        /// The carts' positions "x,y,z;..."
        #[arg(long, allow_hyphen_values = true)]
        at: String,
        /// "x,y,z;..."
        #[arg(long, allow_hyphen_values = true)]
        targets: String,
        /// Cells laid over the design, "x,y,z=STATE;..."
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        set: String,
        /// Part files laid over the design (their non-air cells) before `--set`, "file@dx,dy,dz;..."
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        parts: String,
        #[arg(long, default_value_t = 0)]
        reach: i64,
        /// Also every node the evaluator made
        #[arg(long)]
        nodes: bool,
        /// Tick once after the summon (the rider placed at its seat) before the searches
        #[arg(long)]
        tick: bool,
    },
    /// (tickexact 2026-11-12, ironspec: dfb89499's lane 7 hold) The braked side slope into the
    /// shared lift under a wall sign (dry or waterlogged): water states, rest, air, entry, exit
    SideSlope {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, default_value = "0.2,0.3,0.4")]
        speeds: String,
        #[arg(long)]
        waterlogged: bool,
        #[arg(long, default_value_t = 100)]
        release_at: u32,
        /// Restart rows at these ticks, a,b
        #[arg(long, default_value = "")]
        restarts: String,
        #[arg(long, default_value_t = 260)]
        max_ticks: u32,
        /// (ironspec 15:11 / 15:23) the frame: world = the record's local cell + this offset (P1 6,80,6;
        /// P3 6,240,6; P4 16390,80,16390); the default is the record's own frame at +80
        #[arg(long, default_value = "0,80,0", allow_hyphen_values = true)]
        at: String,
    },
    /// (tickexact 2026-11-12, ironspec: the pocket backs) A cart at rest on a north_south powered
    /// rail powered with a block at its north end: the first tick's velocity, whether it leaves
    RailKick {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "slime_block,glass,stone")]
        blocks: String,
        #[arg(long, default_value_t = 60)]
        power_at: u32,
        #[arg(long, default_value_t = 20)]
        watch: u32,
        /// Restart rows at these ticks (before the power), a,b
        #[arg(long, default_value = "")]
        restarts: String,
        /// (ironspec 15:11 / 15:23) the frame: world = the bench's local cell + this offset (its copies at
        /// local x 0, 8, 16, ..., y 0, z 0; P4 16390,80,16390); the default is the bench's own (0,64,0)
        #[arg(long, default_value = "0,64,0", allow_hyphen_values = true)]
        at: String,
    },
    /// (tickexact 2026-11-12, G1 on b8949c96's way-in) The queue's release, the second fall and the
    /// roll into A6's entry: the entry tick and speed, the rider's health, restarts mid-ride
    QueueRoll {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, allow_hyphen_values = true, default_value = "9.5,-13.0508,-3.8867")]
        start: String,
        #[arg(long, allow_hyphen_values = true, default_value = "8,-14,-4")]
        release_cell: String,
        #[arg(long, default_value_t = 20)]
        release_at: u32,
        #[arg(long, default_value_t = 10)]
        release_len: u32,
        /// The release block stays until this departure detector powers (onechunk's emulation of
        /// the gate) instead of `--release-len` gt
        #[arg(long, allow_hyphen_values = true)]
        until_departure: Option<String>,
        #[arg(long, allow_hyphen_values = true, default_value = "3,-25,-5")]
        entry: String,
        /// Restart rows at these ticks after the release, a,b,c
        #[arg(long, default_value = "")]
        restarts: String,
        #[arg(long, default_value_t = 300)]
        max_ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec 13:27: a pick waiting at the queue all day) A seated
    /// villager at the queue seat with HOME, time running from REST: its activity every tick, HOME
    AllDayWait {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        seat: String,
        #[arg(long, allow_hyphen_values = true)]
        home: String,
        #[arg(long, default_value_t = 12000)]
        from: i64,
        #[arg(long, default_value_t = 14000)]
        ticks: u32,
        /// The cart's start x,y,z (it rides to the seat); unset: held at the seat
        #[arg(long, allow_hyphen_values = true)]
        start: Option<String>,
        /// `--start`'s motion vx,vy,vz (else `--speeds` along +x, one per trial in turn)
        #[arg(long, allow_hyphen_values = true)]
        motion: Option<String>,
        #[arg(long, default_value = "0.2")]
        speeds: String,
        #[arg(long, default_value_t = 1)]
        trials: u32,
        /// Cells laid over the module, "x,y,z=STATE;..."
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        set: String,
        /// Release the cart this many gt after its arrival (to A6's entry, the eject, the ledge)
        #[arg(long)]
        release_after: Option<u32>,
        #[arg(long, allow_hyphen_values = true, default_value = "8,-14,-4")]
        release_cell: String,
        #[arg(long, allow_hyphen_values = true, default_value = "9,-15,-3")]
        departure: String,
        #[arg(long, allow_hyphen_values = true, default_value = "3,-25,-5")]
        entry: String,
    },
    /// (tickexact 2026-11-12, ironspec 12:44: A6's restart re-board) A ridden cart onto A6's
    /// activator, a restart at every tick from the eject to eject + W (chest landing or `--glass`)
    Reboard {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long, default_value = "0.2,0.3,0.4")]
        speeds: String,
        #[arg(long, default_value_t = 10)]
        window: u32,
        #[arg(long)]
        glass: bool,
    },
    /// (tickexact 2026-11-12, ironspec: A6's napper ride) The napper's cart from R up its lift and
    /// the staircase to rest on the hold, on the official server: the detector's rise, the T entry
    /// speed, the rest tick; rows at several start speeds and with a restart along the climb
    NapperRide {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, default_value_t = 9)]
        depth: i32,
        #[arg(long, default_value = "0.2")]
        speeds: String,
        #[arg(long, allow_hyphen_values = true, default_value = "7,-3,4")]
        detector: String,
        #[arg(long, allow_hyphen_values = true, default_value = "9,0,7")]
        t: String,
        #[arg(long, allow_hyphen_values = true, default_value = "9,0,8")]
        hold: String,
        /// Cart y thresholds: one row per threshold restarts the server when the cart first reaches it
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        restart_at: String,
        /// A restart row at every tick from each speed's detector rise to its rest, with the maximum
        #[arg(long)]
        every_tick: bool,
        /// `--every-tick` narrowed to start this many gt before the detector's rise (default: from
        /// the summon at R, the whole climb)
        #[arg(long)]
        before: Option<u32>,
        /// `--every-tick --before N`: only the ticks before the rise
        #[arg(long)]
        only_before: bool,
        /// FROM,TO: at each speed's worst restart tick, the player joining FROM..=TO gt after the start
        #[arg(long)]
        join: Option<String>,
        /// Cells laid over the module, "x,y,z=STATE;..." (the hold's slime stop as built)
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        set: String,
        #[arg(long, default_value_t = 400)]
        max_ticks: u32,
    },
    /// (tickexact 2026-11-12, farmopt/ironspec: A6's wake) A villager asleep in A6's bed, the hatch
    /// revealed N gt after the lie-down: NEAREST_HOSTILE, the wake and the stand-up cell per trial
    NapWake {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long, default_value_t = 30)]
        trials: u32,
        #[arg(long, default_value_t = 10)]
        flip_after: u32,
        #[arg(long, default_value_t = 300)]
        ticks: u32,
        /// The wake at REST's end with the levers on (the day time run up from 23950 at lie-down +
        /// N) instead of F
        #[arg(long)]
        rest_end: bool,
        /// Each trial's yaw at the summon, alternating, a,b (findStandUpPosition tries the yaw's side first)
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        yaws: String,
        /// A restart at F + each offset, one per trial in turn (negative: before F), a,b
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        restarts: String,
    },
    /// (tickexact 2026-11-12, ironspec: does a summoner's GOLEM_DETECTED_RECENTLY get renewed after
    /// its own summon?) Stands of three panicking seated villagers summoning a golem that leaves
    /// after N ticks: the renewals after T and the summon at T + 600, over fresh rounds
    GolemRenew {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 16)]
        stands: u32,
        #[arg(long, default_value_t = 20)]
        rounds: u32,
        #[arg(long, default_value_t = 2)]
        golem_ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec: the villager's golem detection range on 26.3) A seated
    /// villager and a held iron golem at box gaps, horizontal and vertical: GOLEM_DETECTED_RECENTLY
    /// and whether NEAREST_LIVING_ENTITIES holds the golem
    GolemRange {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value = "15,17,30,47")]
        gaps: String,
        #[arg(long, default_value_t = 400)]
        ticks: u32,
    },
    /// (tickexact 2026-11-12, ironspec: the queue post's sky) The post's zombie in the composition:
    /// the light and the sky at its eye at noon, its Fire and Health over a clear day
    PostSky {
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long)]
        module: std::path::PathBuf,
        #[arg(long, default_value = "")]
        parts: String,
        /// The zombie's boat, "kind x y z {nbt}" (y in the design's frame)
        #[arg(long, allow_hyphen_values = true)]
        summon: String,
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
        /// Cells laid after the parts, "x,y,z=STATE;..."
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        set: String,
        /// The day time set before the rides
        #[arg(long)]
        day_time: Option<i64>,
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
        /// A real save, stop and start before each listed tick (the cart as the moving probe)
        #[arg(long, default_value = "")]
        restart_at: String,
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
    /// (G1 15:31, ironspec 14:33 / 14:35: the loader check) the composed farm from world-frame files, the
    /// player at the farm, in the Nether, logged out for --ticks with a restart inside, then logging in:
    /// per route chunk entity ticking, per Nether chunk its level, both PORTAL tickets' least ticks left
    LoaderLong {
        #[arg(long, default_value = "26.3")]
        version: String,
        /// the world-frame design: overworld cells at their world coordinates, its Nether part at the Nether's
        #[arg(long)]
        world: Option<std::path::PathBuf>,
        /// a second world-frame file whose cells are the Nether's (when the Nether part is a file of its own)
        #[arg(long)]
        nether: Option<std::path::PathBuf>,
        /// (onechunk 16:18, instead of --world) the farm module laid at each --module-offsets
        #[arg(long)]
        module: Option<std::path::PathBuf>,
        /// the module copies' offsets, "x,y,z;x,y,z" (world = cell + offset; its entities' HOME too)
        #[arg(long, allow_hyphen_values = true, default_value = "")]
        module_offsets: String,
        /// the module's Nether part's y base (its cells at the Nether image of its anchor, y less this)
        #[arg(long, default_value_t = 64)]
        module_nether_ybase: i32,
        /// (onechunk 16:42) the module copies' overworld pad portal blocks left out (freeze_rows' `no_pads`)
        #[arg(long)]
        module_no_pads: bool,
        /// (onechunk 16:18) the loader, laid at --loader-offset
        #[arg(long)]
        loader: Option<std::path::PathBuf>,
        #[arg(long, allow_hyphen_values = true, default_value = "6,80,-10")]
        loader_offset: String,
        #[arg(long, default_value_t = 0)]
        loader_nether_ybase: i32,
        /// the overworld route chunks the bar holds, "cx,cz;cx,cz"
        #[arg(long, allow_hyphen_values = true)]
        chunks: String,
        /// the Nether chunks the bar holds, "cx,cz;cx,cz"
        #[arg(long, allow_hyphen_values = true)]
        nchunks: String,
        /// the overworld chunk holding the loader's portal
        #[arg(long, allow_hyphen_values = true)]
        portal: String,
        /// the Nether chunk holding the twin's portal
        #[arg(long, allow_hyphen_values = true)]
        nportal: String,
        #[arg(long, default_value_t = 48000)]
        ticks: u32,
        #[arg(long, default_value_t = 24000)]
        restart_at: u32,
        /// where the player stands at the farm (overworld), x,y,z
        #[arg(long, allow_hyphen_values = true)]
        farm_afk: String,
        /// where the player stands in the Nether, x,y,z
        #[arg(long, allow_hyphen_values = true)]
        nether_afk: String,
        /// the load agent's watched chunks (both levels), x0,z0,x1,z1
        #[arg(long, allow_hyphen_values = true, default_value = "-1,-1,3,3")]
        watch: String,
        /// (G1 on Y #1) keep the loader verdict when the record isn't update-stable (SHAPE differences after
        /// placement), labelling it; without it a SHAPE difference fails the run
        #[arg(long)]
        loader_only: bool,
    },
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
        /// Rev 3 at N = 4: the mirrored module file (the southern copies); `self`: the module
        /// itself mirrored in z there (z' = 3 - z, as onechunk's mirrored file)
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
        /// On the official server at 20 tps: the detector's dawn edge, the update tick's day time
        /// set to the model's threshold - 1 .. + 2 (ironspec 00:36 PT)
        #[arg(long)]
        dawn: bool,
        /// (tickexact 2026-11-13) On the official server: the DAY line's off-edge (the first dusk
        /// update reading <= 5) in clear, rain and thunder, each at its end levels
        #[arg(long)]
        edges: bool,
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
        /// print every conflicting cell (default: the first 6 per scene)
        #[arg(long)]
        all: bool,
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
        MobaiCmd::SceneLint { scenes, all } => crate::fixture::scene_lint(&scenes, all),
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
            // (G1 on G #10) no 0-run row (the pending control would be its only restart)
            anyhow::ensure!(runs >= 1, "--runs 0: no sample (NOT JUDGED)");
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
        MobaiCmd::TSwitch { version, speeds, cause, arrive, depart, ticks } => {
            let speeds: Vec<f64> = speeds.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let arrive: Vec<i32> = arrive.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let depart: Vec<i32> = depart.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(["lever", "repeater", "detector-after", "detector-before"].contains(&cause.as_str()), "--cause: lever, repeater, detector-after or detector-before");
            crate::tswitch::t_switch(&version, &speeds, &cause, &arrive, &depart, ticks)
        }
        MobaiCmd::LiftPair { version, module, slope, release_cell, release_at, gaps, ticks } => {
            let s: Vec<f64> = slope.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let r: Vec<i32> = release_cell.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(s.len() == 3 && r.len() == 3, "--slope x,y,z and --release-cell x,y,z");
            let gaps: Vec<u32> = gaps.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::liftpair::lift_pair(&version, &module, [s[0], s[1], s[2]], [r[0], r[1], r[2]], release_at, &gaps, ticks)
        }
        MobaiCmd::StationRide { version, lift, station, rel, offsets, restarts, ticks, lane7_release } => {
            let offsets: Vec<u32> = offsets.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(offsets.len() == 3 && offsets.iter().all(|&o| o + rel >= 1), "--offsets: three, with REL + offset >= 1");
            let restarts: Vec<u32> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let l7: Vec<u32> = lane7_release.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(l7.is_empty() || l7.len() == 2, "--lane7-release a,b");
            crate::stationride::station_ride(&version, &lift, &station, rel, &offsets, &restarts, ticks, if l7.len() == 2 { Some((l7[0], l7[1])) } else { None })
        }
        MobaiCmd::BedTrap { version, file, set, speeds, restarts, repeats, ticks, approach, release, home, watch, release_lever, held_only, file_cart, day, cart_pos, start_vx, clear, overlay, overlay_box } => {
            let mut cells = Vec::new();
            // --overlay: the other design's cells inside the box first (the --set cells override)
            if let Some(o) = &overlay {
                let b: Vec<i32> = overlay_box.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(b.len() == 6, "--overlay-box x0,y0,z0,x1,y1,z1");
                let d = mctas::design::Design::load(o)?;
                // (ironspec 04:07) an overlay cell equal to the file's own cell isn't an override: left out
                let base = mctas::design::Design::load(&file)?;
                let mut same = 0usize;
                for (p, s) in d.blocks.iter() {
                    if (b[0]..=b[3]).contains(&p[0]) && (b[1]..=b[4]).contains(&p[1]) && (b[2]..=b[5]).contains(&p[2]) {
                        if base.blocks.get(p) == Some(s) {
                            same += 1;
                            continue;
                        }
                        cells.push(([p[0], p[1], p[2]], s.clone()));
                    }
                }
                println!("--overlay {}: {} cells inside {overlay_box} laid before --set ({same} more equal to the file's own, left out)", o.display(), cells.len());
                // (G1 on 6ec22ffd8 #9) an overlay that lays nothing is no overlay
                anyhow::ensure!(!cells.is_empty(), "--overlay {}: no cell inside {overlay_box} differs from the file's own: the overlay lays nothing, the run fails", o.display());
            }
            for e in set.split(';').filter(|e| !e.trim().is_empty()) {
                let (p, st) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("x,y,z=STATE, got {e}"))?;
                let v: Vec<i32> = p.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z=STATE, got {e}");
                cells.push(([v[0], v[1], v[2]], st.trim().to_string()));
            }
            let speeds: Vec<f64> = speeds.split(',').map(|v| v.trim().parse()).collect::<Result<_, _>>()?;
            let restarts: Vec<u32> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            // (G1 on 6ec22ffd8 #3, #5, #10) no 0-sample run, no restart past the run's end, no flag
            // that does nothing silently
            anyhow::ensure!(repeats >= 1, "--repeats 0: no sample (NOT JUDGED); the run fails");
            anyhow::ensure!(!speeds.is_empty(), "--speeds: none given");
            if let Some(bad) = restarts.iter().find(|&&r| r == 0 || r > ticks) {
                anyhow::bail!("--restarts {bad}: outside the run's 1..={ticks} ticks, so the restart row would never restart");
            }
            if approach.is_none() {
                let silent: Vec<&str> = [
                    ("--day", day),
                    ("--watch", watch.is_some()),
                    ("--clear", clear.is_some()),
                    ("--release-lever", release_lever),
                    ("--cart-pos", cart_pos.is_some()),
                    ("--file-cart", file_cart),
                    ("--start-vx", start_vx != 0.0),
                ]
                .iter()
                .filter(|(_, on)| *on)
                .map(|(n, _)| *n)
                .collect();
                anyhow::ensure!(silent.is_empty(), "{} only act(s) with --approach: refused rather than ignored", silent.join(", "));
            }
            let approach = match approach {
                Some(s) => {
                    let p: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    // the release tick may be a range T1..T2: one bed-trap run per release phase
                    let parts: Vec<&str> = release.split(',').collect();
                    anyhow::ensure!(parts.len() == 4, "--release x,y,z,t (t or t1..t2)");
                    let r: Vec<i64> = parts[..3].iter().map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    let (t1, t2): (u32, u32) = match parts[3].split_once("..") {
                        Some((a, b)) => (a.trim().parse()?, b.trim().parse()?),
                        None => {
                            let t = parts[3].trim().parse()?;
                            (t, t)
                        }
                    };
                    let h: Vec<i32> = home.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(p.len() == 3 && h.len() == 3 && t1 <= t2, "--approach x,y,z --release x,y,z,t --home x,y,z");
                    // (G1 #10) the release must fall inside the run (9999 = "never" is allowed as the
                    // explicit no-release probe)
                    anyhow::ensure!(t2 < ticks || t1 == 9999, "--release tick {t2} is not inside the run's {ticks} ticks (use 9999 for an explicit no-release probe)");
                    let watch = match &watch {
                        Some(w) => {
                            let v: Vec<i32> = w.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                            anyhow::ensure!(v.len() == 3, "--watch x,y,z");
                            Some([v[0], v[1], v[2]])
                        }
                        None => None,
                    };
                    let cart_pos = match &cart_pos {
                        Some(c) => {
                            let v: Vec<f64> = c.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                            anyhow::ensure!(v.len() == 3, "--cart-pos x,y,z");
                            Some([v[0], v[1], v[2]])
                        }
                        None => None,
                    };
                    let clear = match &clear {
                        Some(c) => {
                            let v: Vec<f64> = c.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                            anyhow::ensure!(v.len() == 3, "--clear x,y,z");
                            Some([v[0], v[1], v[2]])
                        }
                        None => None,
                    };
                    for t in t1..=t2 {
                        let a = crate::bedtrap::Approach { start: [p[0], p[1], p[2]], release: [r[0] as i32, r[1] as i32, r[2] as i32], release_at: t, home: [h[0], h[1], h[2]], watch, lever: release_lever, file_cart, day, cart_pos, start_vx, clear };
                        crate::bedtrap::bed_trap(&version, &file, &cells, &speeds, Some(a), &restarts, repeats, ticks, held_only)?;
                    }
                    return Ok(());
                }
                None => None,
            };
            crate::bedtrap::bed_trap(&version, &file, &cells, &speeds, approach, &restarts, repeats, ticks, held_only)
        }
        MobaiCmd::BedReclaim { version, file, remove, day_pos, home, arm_at, runs, after, expect, within, asleep } => {
            let f3 = |s: &str, n: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|x| x.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "{n} x,y,z");
                Ok([v[0], v[1], v[2]])
            };
            let rm: Vec<[i32; 3]> = remove
                .split(';')
                .filter(|e| !e.trim().is_empty())
                .map(|e| -> anyhow::Result<[i32; 3]> {
                    let v: Vec<i32> = e.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--remove x,y,z;…");
                    Ok([v[0], v[1], v[2]])
                })
                .collect::<anyhow::Result<_>>()?;
            let h = f3(&home, "--home")?;
            crate::reclaim::bed_reclaim(&version, &file, &rm, f3(&day_pos, "--day-pos")?, [h[0] as i32, h[1] as i32, h[2] as i32], f3(&arm_at, "--arm-at")?, runs, after, expect, within, asleep)
        }
        MobaiCmd::RowGlass { version, speeds, phases, empties } => {
            let s: Vec<f64> = speeds.split(',').map(|v| v.trim().parse::<f64>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(s.len() == 3 && s[2] > 0.0 && s[1] >= s[0], "--speeds from,to,step, got {speeds}");
            let n = ((s[1] - s[0]) / s[2]).round() as usize;
            let list: Vec<f64> = (0..=n).map(|i| s[0] + s[2] * i as f64).collect();
            crate::presencestring::row_glass(&version, &list, &phases, &empties)
        }
        MobaiCmd::RowString { version, speeds, phases, empties } => {
            let s: Vec<f64> = speeds.split(',').map(|v| v.trim().parse::<f64>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(s.len() == 3 && s[2] > 0.0 && s[1] >= s[0], "--speeds from,to,step, got {speeds}");
            let n = ((s[1] - s[0]) / s[2]).round() as usize;
            let list: Vec<f64> = (0..=n).map(|i| s[0] + s[2] * i as f64).collect();
            crate::presencestring::row_string(&version, &list, &phases, &empties)
        }
        MobaiCmd::LaunchSweep { version, scene, speeds, phases, sbend_straight, bar, trace } => {
            let s: Vec<f64> = speeds.split(',').map(|v| v.trim().parse::<f64>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(s.len() == 3 && s[2] > 0.0 && s[1] >= s[0], "--speeds from,to,step, got {speeds}");
            let n = ((s[1] - s[0]) / s[2]).round() as usize;
            let list: Vec<f64> = (0..=n).map(|i| s[0] + s[2] * i as f64).collect();
            crate::launchsweep::launch_sweep(&version, &scene, &list, &phases, sbend_straight, bar, trace)
        }
        MobaiCmd::SleepStop { version, bench, mode, v, release_after, watch, wake_yaw } => crate::sleepmerge::sleep_stop(&version, &bench, &mode, v, release_after, watch, wake_yaw),
        MobaiCmd::SleepPickup { version, v, settle, watch, stop, max_wait } => crate::sleepmerge::sleep_pickup(&version, v, settle, watch, max_wait, stop),
        MobaiCmd::SleepMerge { version, mode, k, watch, guard, max_wait } => crate::sleepmerge::sleep_merge(&version, &mode, k, watch, guard, max_wait),
        MobaiCmd::PresenceString { version, ticks, open_at, depart_at, rise, slope } => {
            if slope {
                crate::presencestring::presence_slope(&version, ticks, open_at, depart_at)
            } else {
                crate::presencestring::presence_string_at(&version, ticks, open_at, depart_at, rise)
            }
        }
        MobaiCmd::DropEnvelope { files } => crate::drivereplay::drop_envelope(&files),
        MobaiCmd::ShiftScene { scene, drive, by, scene_out, drive_out } => {
            let b: Vec<i32> = by.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(b.len() == 3, "--by dx,dy,dz");
            crate::drivereplay::shift_scene(&scene, &drive, [b[0], b[1], b[2]], &scene_out, &drive_out)
        }
        MobaiCmd::DriveReplay { version, scene, drive, ticks, day_start, bed, loads } => {
            let b: Vec<i32> = bed.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(b.len() == 3, "--bed x,y,z");
            let loads: Vec<u32> = loads.split(',').filter(|s| !s.trim().is_empty()).map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(loads.iter().all(|&l| l > 0 && l <= ticks), "--loads: each inside 1..={ticks}");
            crate::drivereplay::drive_replay(&version, &scene, &drive, ticks, day_start, [b[0], b[1], b[2]], &loads)
        }
        MobaiCmd::SightControl { version, cases, ticks } => crate::sightctl::sight_control(&version, &cases.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>(), ticks),
        MobaiCmd::PlayerSleep { version, how } => crate::playersleep::player_sleep(&version, &how.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>()),
        MobaiCmd::RailSteps { version, file, lanes, case, power, script, watch } => {
            if let Some(s) = script {
                let extra: Vec<[i32; 3]> = watch
                    .split(';')
                    .filter(|e| !e.trim().is_empty())
                    .map(|e| -> anyhow::Result<[i32; 3]> {
                        let v: Vec<i32> = e.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 3, "--watch x,y,z;…");
                        Ok([v[0], v[1], v[2]])
                    })
                    .collect::<anyhow::Result<_>>()?;
                return crate::railsteps::rail_script(&version, &file, &s, &extra);
            }
            let lanes: Vec<i32> = lanes.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let power = match power {
                Some(p) => {
                    let v: Vec<i32> = p.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--power dx,y,z");
                    Some([v[0], v[1], v[2]])
                }
                None => None,
            };
            crate::railsteps::rail_steps(&version, &file, &lanes, &case, power)
        }
        MobaiCmd::WaterSettle { version, file, set, ticks, controls } => {
            if controls && file.is_none() {
                anyhow::ensure!(ticks >= 1, "--ticks 0: no settling judged (NOT JUDGED)");
                return crate::watersettle::controls(&version, ticks);
            }
            let file = file.ok_or_else(|| anyhow::anyhow!("water-settle: a design file (or --controls)"))?;
            let mut cells = Vec::new();
            for e in set.split(';').filter(|e| !e.trim().is_empty()) {
                let (p, st) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("x,y,z=STATE, got {e}"))?;
                let v: Vec<i32> = p.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z=STATE, got {e}");
                cells.push(([v[0], v[1], v[2]], st.trim().to_string()));
            }
            crate::watersettle::water_settle(&version, &file, &cells, ticks)
        }
        MobaiCmd::GolemTtl { version, restart_after, runs, ticks, boundary } => crate::golemttl::golem_ttl(&version, restart_after, runs, ticks, boundary),
        MobaiCmd::LoadCount { version, cases, run_ms, scene } => match scene {
            Some(s) => crate::loadcount::load_count_farm(&version, &s, cases, run_ms),
            None => crate::loadcount::load_count(&version, cases, run_ms),
        },
        MobaiCmd::PanicTicks { version, villagers, ticks, restart_at } => crate::panicticks::panic_ticks(&version, villagers, ticks, restart_at),
        MobaiCmd::GossipRow { version, layout, day_time, ticks, restart_after } => crate::gossiprow::gossip_row(&version, &layout, day_time, ticks, restart_after),
        MobaiCmd::SbendBoundary { version, module, lanes, napper, leader_v, offsets, ticks, leader_first, napper_roll } => {
            let n: Vec<f64> = napper.split(',').map(|v| v.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(n.len() == 3, "--napper x,y,z");
            let offsets: Vec<i32> = offsets.split(',').map(|v| v.trim().parse()).collect::<Result<_, _>>()?;
            let roll = match napper_roll {
                Some(s) => {
                    let v: Vec<f64> = s.split(',').map(|x| x.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 6, "--napper-roll x,y,z,vx,vy,vz");
                    Some([v[0], v[1], v[2], v[3], v[4], v[5]])
                }
                None => None,
            };
            crate::sbendrow::sbend_boundary(&version, &module, &lanes, [n[0], n[1], n[2]], leader_v, &offsets, ticks, leader_first, roll)
        }
        MobaiCmd::WaterRails { version, slopes, flat, start, ticks, every_tick, restarts, measure, both_branches } => {
            let restarts: Vec<u32> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            if restarts.is_empty() {
                crate::waterrails::water_rails(&version, slopes, flat, start, ticks, every_tick)
            } else {
                crate::waterrails::water_restarts(&version, slopes, flat, measure, &restarts, ticks, every_tick, both_branches)
            }
        }
        MobaiCmd::LatchRig { version, form_b } => match form_b {
            Some(p) => crate::latchformb::latch_rig_form_b(&version, &p),
            None => crate::latchrig::latch_rig(&version),
        },
        MobaiCmd::CartFall { version } => crate::cartfall::cart_fall(&version),
        MobaiCmd::StringRelease { version } => crate::stringrelease::string_release(&version),
        MobaiCmd::AbutLatch { version } => crate::abutlatch::abut_latch(&version),
        MobaiCmd::EString { version, record } => crate::estring::e_string(&version, &record),
        MobaiCmd::SlopeString { version } => crate::slopestring::slope_string(&version),
        MobaiCmd::HopperPin { version } => crate::hopperpin::hopper_pin(&version),
        MobaiCmd::BlockWatch { version, module, set, watch, restarts, ticks } => {
            let cells_of = |s: &str| -> anyhow::Result<Vec<([i32; 3], String)>> {
                let mut out = Vec::new();
                for e in s.split(';').filter(|e| !e.trim().is_empty()) {
                    let (p, st) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("x,y,z=STATE, got {e}"))?;
                    let v: Vec<i32> = p.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "x,y,z=STATE, got {e}");
                    out.push(([v[0], v[1], v[2]], st.trim().to_string()));
                }
                Ok(out)
            };
            let restarts: Vec<u32> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::blockwatch::block_watch(&version, &module, &cells_of(&set)?, &cells_of(&watch)?, &restarts, ticks)
        }
        MobaiCmd::OnewayCross { version, design, parts, set, at, targets, tries, ticks, north_z, speed, day_time } => {
            let f3 = |s: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let i3 = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let mut part_list = Vec::new();
            for e in parts.split(';').filter(|e| !e.trim().is_empty()) {
                let (f, off) = e.rsplit_once('@').ok_or_else(|| anyhow::anyhow!("--parts file@dx,dy,dz, got {e}"))?;
                part_list.push((std::path::PathBuf::from(f.trim()), i3(off)?));
            }
            let mut cells = Vec::new();
            for e in set.split(';').filter(|e| !e.trim().is_empty()) {
                let (p, st) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("--set x,y,z=STATE, got {e}"))?;
                cells.push((i3(p)?, st.trim().to_string()));
            }
            let targets: Vec<[i32; 3]> = targets.split(';').filter(|e| !e.trim().is_empty()).map(i3).collect::<Result<_, _>>()?;
            crate::onewaycross::oneway_cross(&version, &design, &part_list, &cells, f3(&at)?, &targets, tries, ticks, north_z, speed, day_time)
        }
        MobaiCmd::HoldPulse { version, module, rest, pulse_cell, pulse_at, pulse_len, pass_y, restarts, max_ticks } => {
            let f3 = |s: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let i3 = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let pulse_at: Vec<u32> = pulse_at.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let restarts: Vec<u32> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::holdpulse::hold_pulse(&version, &module, f3(&rest)?, i3(&pulse_cell)?, &pulse_at, pulse_len, pass_y, &restarts, max_ticks)
        }
        MobaiCmd::GolemPad { version, pad, under, stands, ticks } => crate::golempad::golem_pad(&version, &pad, &under, stands, ticks),
        MobaiCmd::PathProbeGame { version, design, at, targets, set, parts, reach, nodes, tick } => {
            let f3 = |s: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let i3 = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let at: Vec<[f64; 3]> = at.split(';').filter(|e| !e.trim().is_empty()).map(f3).collect::<Result<_, _>>()?;
            let targets: Vec<[i32; 3]> = targets.split(';').filter(|e| !e.trim().is_empty()).map(i3).collect::<Result<_, _>>()?;
            let mut cells = Vec::new();
            for e in set.split(';').filter(|e| !e.trim().is_empty()) {
                let (p, st) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("--set x,y,z=STATE, got {e}"))?;
                cells.push((i3(p)?, st.trim().to_string()));
            }
            let mut part_list = Vec::new();
            for e in parts.split(';').filter(|e| !e.trim().is_empty()) {
                let (f, off) = e.rsplit_once('@').ok_or_else(|| anyhow::anyhow!("--parts file@dx,dy,dz, got {e}"))?;
                part_list.push((std::path::PathBuf::from(f.trim()), i3(off)?));
            }
            crate::pathprobegame::path_probe_game(&version, &design, &part_list, &at, &targets, &cells, reach, nodes, tick)
        }
        MobaiCmd::SideSlope { version, module, speeds, waterlogged, release_at, restarts, max_ticks, at } => {
            let speeds: Vec<f64> = speeds.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let restarts: Vec<u32> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::sideslope::side_slope(&version, &module, &speeds, waterlogged, release_at, &restarts, max_ticks, crate::railkick::cell3(&at)?)
        }
        MobaiCmd::RailKick { version, blocks, power_at, watch, restarts, at } => {
            let blocks: Vec<String> = blocks.split(',').map(|b| b.trim().to_string()).filter(|b| !b.is_empty()).collect();
            let restarts: Vec<u32> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::railkick::rail_kick(&version, &blocks, power_at, watch, &restarts, crate::railkick::cell3(&at)?)
        }
        MobaiCmd::QueueRoll { version, module, start, release_cell, release_at, release_len, until_departure, entry, restarts, max_ticks } => {
            let f3 = |s: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let i3 = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let restarts: Vec<u32> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::queueroll::queue_roll(&version, &module, f3(&start)?, i3(&release_cell)?, release_at, release_len, until_departure.as_deref().map(i3).transpose()?, i3(&entry)?, &restarts, max_ticks)
        }
        MobaiCmd::AllDayWait { version, module, seat, home, from, ticks, start, motion, speeds, trials, set, release_after, release_cell, departure, entry } => {
            let f3 = |s: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let i3 = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let mut cells = Vec::new();
            for e in set.split(';').filter(|e| !e.trim().is_empty()) {
                let (p, st) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("--set x,y,z=STATE, got {e}"))?;
                cells.push((i3(p)?, st.trim().to_string()));
            }
            let w = crate::alldaywait::Wait {
                seat: f3(&seat)?,
                home: i3(&home)?,
                from,
                ticks,
                start: start.as_deref().map(f3).transpose()?,
                motion: motion.as_deref().map(f3).transpose()?,
                speeds: speeds.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?,
                trials,
                set: cells,
                release: match release_after {
                    Some(n) => Some((n, i3(&release_cell)?, i3(&departure)?, i3(&entry)?)),
                    None => None,
                },
            };
            crate::alldaywait::all_day_wait(&version, &module, &w)
        }
        MobaiCmd::Reboard { version, file, speeds, window, glass } => {
            let speeds: Vec<f64> = speeds.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::reboard::reboard(&version, &file, &speeds, window, glass)
        }
        MobaiCmd::NapperRide { version, module, depth, speeds, detector, t, hold, restart_at, every_tick, before, only_before, join, set, max_ticks } => {
            let p3 = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let speeds: Vec<f64> = speeds.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let restarts: Vec<f64> = restart_at.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let join = match join.as_deref() {
                Some(s) => {
                    let v: Vec<u32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 2, "--join FROM,TO, got {s}");
                    Some((v[0], v[1]))
                }
                None => None,
            };
            let mut cells_set = Vec::new();
            for e in set.split(';').filter(|e| !e.trim().is_empty()) {
                let (p, st) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("--set x,y,z=STATE, got {e}"))?;
                cells_set.push((p3(p)?, st.trim().to_string()));
            }
            let cells = crate::napperride::Cells { detector: p3(&detector)?, t_cell: p3(&t)?, hold: p3(&hold)?, set: cells_set };
            crate::napperride::napper_ride(&version, &module, depth, &speeds, &cells, &restarts, every_tick, before, only_before, join, max_ticks)
        }
        MobaiCmd::NapWake { version, file, trials, flip_after, ticks, rest_end, yaws, restarts } => {
            let yaws: Vec<f32> = yaws.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            let restarts: Vec<i64> = restarts.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::napwake::nap_wake(&version, &file, trials, flip_after, ticks, rest_end, &yaws, &restarts)
        }
        MobaiCmd::GolemRenew { version, stands, rounds, golem_ticks } => crate::golemrenew::golem_renew(&version, stands, rounds, golem_ticks),
        MobaiCmd::GolemRange { version, gaps, ticks } => {
            let gaps: Vec<f64> = gaps.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::golemrange::golem_range(&version, &gaps, ticks)
        }
        MobaiCmd::PostSky { version, module, parts, summon } => {
            let parts: Vec<std::path::PathBuf> = parts.split(',').filter(|p| !p.trim().is_empty()).map(std::path::PathBuf::from).collect();
            crate::postsky::post_sky(&version, &module, &parts, &summon)
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
        MobaiCmd::SightCheck { version, module, parts, summon, points, ticks, free, trace, repeat, every, life, set, day_time } => {
            let mut cells = Vec::new();
            for e in set.split(';').filter(|e| !e.trim().is_empty()) {
                let (p, st) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("--set x,y,z=STATE, got {e}"))?;
                let v: Vec<i32> = p.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--set x,y,z=STATE, got {e}");
                cells.push(([v[0], v[1], v[2]], st.trim().to_string()));
            }
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
            crate::sightcheck::sight_check(&version, &module, &parts, &summon, &points, &crate::sightcheck::SightOpts { ticks, free, trace, repeat, every, life, set: cells, day_time })
        }
        MobaiCmd::CartTrace { version, module, parts, start, motion, ticks, still, probe, restart_at } => {
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
            let restart_at: Vec<u32> = restart_at.split(',').filter(|n| !n.trim().is_empty()).map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            crate::carttrace::cart_trace(&version, &module, &parts, f3v(&start, "start")?, f3v(&motion, "motion")?, ticks, still, &probe, &restart_at)
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
        MobaiCmd::LoaderLong { version, world, nether, module, module_offsets, module_nether_ybase, module_no_pads, loader, loader_offset, loader_nether_ybase, chunks, nchunks, portal, nportal, ticks, restart_at, farm_afk, nether_afk, watch, loader_only } => {
            let pairs = |s: &str| -> anyhow::Result<Vec<(i32, i32)>> {
                s.split(';')
                    .filter(|p| !p.trim().is_empty())
                    .map(|p| {
                        let v: Vec<i32> = p.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 2, "cx,cz, got {p}");
                        Ok((v[0], v[1]))
                    })
                    .collect()
            };
            let f3 = |s: &str| -> anyhow::Result<[f64; 3]> {
                let v: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let i3 = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
                Ok([v[0], v[1], v[2]])
            };
            let w: Vec<i32> = watch.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(w.len() == 4, "--watch x0,z0,x1,z1");
            let one = |s: &str| -> anyhow::Result<(i32, i32)> { pairs(s)?.first().copied().ok_or_else(|| anyhow::anyhow!("cx,cz, got {s}")) };
            let parts = match (world, module, loader) {
                (Some(w), None, None) => vec![crate::freeze::world_part(&w, nether.as_deref())?],
                (None, Some(m), Some(l)) => {
                    anyhow::ensure!(nether.is_none(), "--nether goes with --world");
                    let mut v = Vec::new();
                    for o in module_offsets.split(';').filter(|s| !s.trim().is_empty()) {
                        v.push(crate::freeze::recipe_part(&m, i3(o)?, module_nether_ybase, module_no_pads)?);
                    }
                    anyhow::ensure!(!v.is_empty(), "--module-offsets names no copy");
                    v.push(crate::freeze::recipe_part(&l, i3(&loader_offset)?, loader_nether_ybase, false)?);
                    v
                }
                _ => anyhow::bail!("give either --world (world-frame files) or --module with --module-offsets and --loader"),
            };
            crate::freeze::loader_long(&version, &parts, &pairs(&chunks)?, &pairs(&nchunks)?, one(&portal)?, one(&nportal)?, ticks, restart_at, f3(&farm_afk)?, f3(&nether_afk)?, [w[0], w[1], w[2], w[3]], loader_only)
        }
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
        MobaiCmd::Daylight { version, latency, levels, weather_row, dawn, edges } => {
            if edges {
                crate::daylight::edges(&version)
            } else if dawn {
                crate::daylight::dawn_edge(&version)
            } else if levels {
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
