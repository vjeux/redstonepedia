//! `mctas mobfarm ...`: the generators and the judge's tools.

use anyhow::{Result, bail};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum MobfarmCmd {
    /// Write a layout's design file: `l1` (the obvious bar), `l2` (a pad around a wet hole, a drop
    /// out of range), `l2d` (the dry-hole module: middle and fastest tiers), `l3` (day 1: spawn
    /// pits that burn the golems in place), `l4` (two banks of fence-gated platforms), `l5` (a
    /// grid of l2d modules at a 27-block pitch).
    Gen {
        layout: String,
        /// Spawn pits (l3).
        #[arg(long, default_value_t = 8)]
        pits: i32,
        /// The pad's side, even (l2, l2d, l5).
        #[arg(long, default_value_t = 4)]
        pad: i32,
        /// The pad's floor height, the villagers' beds at 0 (l2; spawn cells reach -6).
        #[arg(long, default_value_t = -4, allow_hyphen_values = true)]
        floor: i32,
        /// Villagers, 3 or 4 (l2, l2d, l5; 4 uses L4's front: three trapdoors before the zombie).
        #[arg(long, default_value_t = 3)]
        villagers: i32,
        /// Modules per side of the grid (l5).
        #[arg(long, default_value_t = 2)]
        grid: i32,
        /// Modules stacked (stack: M2 modules in one chunk column).
        #[arg(long, default_value_t = 2)]
        modules: i32,
        /// The stack's vertical pitch (stack).
        #[arg(long, default_value_t = 43)]
        pitch: i32,
        /// The regular cage (cross: blind U1 in the watchers' row in a pod like theirs; brief
        /// iteration 17).
        #[arg(long)]
        regular: bool,
        /// The stack's step in x and z (stack: side by side).
        #[arg(long, default_value_t = 0)]
        dx: i32,
        #[arg(long, default_value_t = 0)]
        dz: i32,
        /// LP: the gap from a lower cluster up to its upper one, and the pairs' period.
        #[arg(long, default_value_t = 15)]
        gap: i32,
        #[arg(long, default_value_t = 62)]
        period: i32,
        /// m2open: the hide's length in gt (the bed observer's delay line, 12..=48).
        #[arg(long, default_value_t = 48)]
        hide: i32,
        /// d1u: its kill pit's floor, the villagers' beds at 0 (default -13; at -19 or deeper a
        /// burning golem is out of every villager's 16-block golem sensor).
        #[arg(long, default_value_t = -13, allow_hyphen_values = true)]
        kill: i32,
        /// d1u, d1u-side, d1u-drop-side: build its access in (a spiral staircase dug from the
        /// chest to the surface, a fence-gate hatch: `day1::dug_access`).
        #[arg(long)]
        access: bool,
        /// The tier the design is published in (`day-1`, `middle`, `fastest`: the kind's last
        /// word, `circuit: mob-farm iron_ingot middle`); none: a farm in no tier (catalog only).
        #[arg(long)]
        tier: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// The golem exit flood of a design (every position a golem can stand on, still and 8
    /// inputs), with its spawn spots. A file with no bed floods as a part by default (ironspec
    /// Nov 11 14:00, `exit::golem_exit_part`): no villager gets a HOME, every declared villager
    /// position is a summoner (its villagers, the design's `positions:` line and --positions,
    /// moved by --offset), and a golem nothing kills or that walks out of the part is reported,
    /// not refused (a part has no kill); a golem that reaches a zombie or touches a vehicle still
    /// fails. It says when it ran as a part.
    Exit {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// A part's further summoner positions: files of x y z per line (`queue-loop-run
        /// --record-positions` over a full bench night, or a stand's pocket cells).
        #[arg(long, value_delimiter = ',')]
        positions: Vec<PathBuf>,
        /// Moves the --positions and `positions:` cells by dx,dy,dz (into the design's frame).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        offset: Vec<i32>,
    },
    /// The built farm (water settled) along a row: each cell's block from x0 to x1 at y, z.
    /// `--terrain`: built into its terrain sample at the judge's cheapest spot (design
    /// coordinates). `--flammable`: instead of a row, every flammable block inside the farm's box
    /// (`weather::lightning::flammable`: the fuel the fire rules' spread can pass through, natural
    /// terrain included), with whether it sees the sky.
    Probe {
        design: PathBuf,
        #[arg(long, allow_hyphen_values = true, num_args = 4)]
        row: Vec<i32>,
        #[arg(long)]
        terrain: Option<String>,
        #[arg(long)]
        flammable: bool,
    },
    /// Edit a design in place, its caption and every other line kept: `--chest` puts the farm's
    /// single chest at its `port output` (the download then holds it), `--how` sets its `how:`
    /// line (how it works in a few words: the end of its site title).
    Edit {
        design: PathBuf,
        #[arg(long)]
        chest: bool,
        #[arg(long)]
        how: Option<String>,
        /// A meta line to set, `key=value` (repeatable; an empty value removes it): `--meta
        /// sample=village-edge` judges a void design in that terrain sample.
        #[arg(long)]
        meta: Vec<String>,
        /// A block to set, `x,y,z=state` in the design's own coordinates (repeatable; `air`
        /// removes the cell): a fault copy of a farm (the bed removed, say).
        #[arg(long)]
        set: Vec<String>,
    },
    /// The built farm's lines of sight: each villager's eye to the zombie's eye (the hostile
    /// sensor's test), standing and asleep on its bed (bed + 0.8875), as built and with every
    /// trapdoor forced shut and forced open. `--terrain S`: built into that sample at the judge's
    /// cheapest spot (the design's `terrain:` meta: natural rock for its walls).
    Sight {
        design: PathBuf,
        #[arg(long)]
        terrain: Option<String>,
        /// Instead: every golem spawn cell (`SpawnUtil.trySpawnMob`'s LEGACY_IRON_GOLEM search,
        /// 8 across, from 6 up to 6 down) from each villager standing at its cell and 1, 2 and 3
        /// above it, in design coordinates, with the block it stands on and whose that block is
        /// (the design's or natural).
        #[arg(long)]
        spawns: bool,
    },
    /// The late stage's coupling, exactly: the villager clusters (villagers within each other's
    /// 10-box), and every golem spawn spot of a cluster against every foreign villager's follow
    /// cube at the largest follow range (16 x 1.11485), with the smallest margin (negative:
    /// a foreign villager lists that golem).
    Cubes { design: PathBuf },
    /// A design's build bill at a stage's prices, as the judge's `build minutes` reads it
    /// (circuit::mobfarm::site_bill, the void build): every item, the travel and height rows
    /// (brief farm-iron rev 9 iteration 16) and the collecting trip.
    Bill {
        design: PathBuf,
        /// day-1, middle or late.
        #[arg(long, default_value = "middle")]
        stage: String,
        /// Price it built into this terrain sample at its cheapest spot (village-edge, plains,
        /// forest): the spot's digging, seals and caps, its access stair (a terrain design's
        /// card prices it so).
        #[arg(long)]
        terrain: Option<String>,
    },
    /// A void farm dug into rock (vjeux 18:18: the underground version): its containment blocks
    /// give way to natural rock, every cell its golems (any input in the exit flood, the runs),
    /// items, water, villagers, zombie, sight lines and redstone need is dug open (`terrain:
    /// rock`), and the natural ground lies `--cover` layers above its top.
    Bury {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Rock layers between the farm's top and the natural ground layer.
        #[arg(long, default_value_t = 1)]
        cover: i32,
        /// Runs (each `--ticks`) whose entity boxes join the open cells.
        #[arg(long, default_value_t = 4)]
        seeds: u32,
        #[arg(long, default_value_t = 24000)]
        ticks: u32,
    },
    /// A local optimizer for any farm design (vjeux 2026-11-08 20:59; brief farm-iron rev 9
    /// iteration 20): at the design's own spot, removes each placed block and refills each dug
    /// cell that no static check needs, swaps blocks for cheaper ones of the same role, outer
    /// shell first, every check as on the design (`mobfarm::trim`); then short AI-on runs against
    /// the design's (within 3%, else the edits are bisected); prints before -> after.
    Trim {
        design: PathBuf,
        /// Where the trimmed design goes (its caption kept).
        #[arg(long)]
        out: PathBuf,
        /// Threads screening the edits (0: the machine's cores less 16).
        #[arg(long, default_value_t = 0)]
        threads: usize,
        /// The short AI-on runs: game days each, and how many.
        #[arg(long, default_value_t = 1.0)]
        days: f64,
        #[arg(long, default_value_t = 192)]
        runs: u32,
    },
    /// The player's way up from the chest at the design's spot (trim's climb flood: a 0.6 x 1.8
    /// box walking, jumping 1.25, dropping up to 3; fence gates open): the highest floor reached
    /// against the ground layer (a built-in access must reach it).
    Climb { design: PathBuf },
    /// The farm built into a terrain sample, then the terrain track's worst-case checks for
    /// natural mobs (`hostile::check`): each channel's exposure (brief revision 8: a card line,
    /// never a refusal).
    Exposures {
        design: PathBuf,
        #[arg(long, default_value = "village-edge")]
        terrain: String,
        /// Every cell of each exposure, in design coordinates (where to light or seal).
        #[arg(long)]
        cells: bool,
        /// Instead of the list: this design's exposures against another's (the version before a
        /// change, say), one line per class (channel, mob and what), unchanged ones included,
        /// `BEFORE -> AFTER` with the cells added and removed (design coordinates), then the golem
        /// and interior cells and a count of the classes that changed (ironspec 2026-11-11: a cut
        /// diff hid C8 and C9 once; every class is printed, so none can be missed).
        #[arg(long)]
        against: Option<PathBuf>,
    },
    /// The hostile checks' C1 against the game: at every interior cell of the farm built in a
    /// terrain sample, for every type listed there, C1's verdict (placement, spawn rule at its
    /// best draws and worst light, no collision) beside the game's own `SpawnPlacements`
    /// (`isSpawnPositionOk`, `noCollision`, `checkSpawnRules` over `--tries` randoms) in the
    /// compare's terrain sandbox.
    SpawnRules {
        design: PathBuf,
        #[arg(long, default_value = "village-edge")]
        terrain: String,
        #[arg(long, default_value_t = 200)]
        tries: u32,
        /// The game's clock for the check (the overworld's total ticks; default midnight, the
        /// darkest sky, what C1 assumes for a darkness rule).
        #[arg(long, default_value_t = 18000)]
        day_time: i64,
    },
    /// The design priced at each of the spots a player would pick in a terrain sample (the 12
    /// nearest valid ones, `site::price_all`): blocks placed, glass covers, natural blocks dug and
    /// their build seconds, cheapest first, with the time the cheap part (`price_spots`) and the
    /// covers (`covers_at`) take (`--repeat`: the cheap part timed over N calls).
    Spots {
        design: PathBuf,
        #[arg(long, default_value = "village-edge")]
        terrain: String,
        #[arg(long, default_value_t = 1)]
        repeat: u32,
        /// The caps for the panicking villagers' golem searches only (the judge caps every
        /// villager's, `site::covers_at`), to compare.
        #[arg(long)]
        panicking_only: bool,
        /// List the cheapest spot's capped cells in design coordinates.
        #[arg(long)]
        caps: bool,
    },
    /// mobinfra: the real-game gate (what `circuit verify` and `circuit archive` run for a mob
    /// farm): each build in the 26.3 server for `--days` of natural weather, a forced day of rain
    /// and one of thunder, on Hard, every must-have checked, the rate against redsim's.
    Gate {
        design: PathBuf,
        #[arg(long, default_value_t = 3)]
        days: u32,
        /// Only these builds (rotation.offset index, e.g. `0.0 1.1`); default: the judge's 8.
        #[arg(long, num_args = 0..)]
        builds: Vec<String>,
        #[arg(long)]
        no_rain_day: bool,
        #[arg(long)]
        no_thunder_day: bool,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Servers at once.
        #[arg(long, default_value_t = 8)]
        parallel: usize,
        /// The weather's warm-up range (gt), `MIN MAX`.
        #[arg(long, num_args = 2, default_values_t = [150000u32, 400000u32])]
        warmup: Vec<u32>,
        #[arg(long)]
        keep: bool,
        #[arg(long)]
        trace: bool,
        /// A terrain sample (`village-edge`...): the farm at its Site in the sample's kept world,
        /// `--parallel` runs of the one placement.
        #[arg(long)]
        terrain: Option<String>,
        /// The kept world folder (default `$MCTAS_HOME/terrain/worlds/<the sample's world>`).
        #[arg(long)]
        world: Option<PathBuf>,
        /// Natural monsters off in a terrain run (`spawn_monsters false`): an experiment that
        /// separates the farm from the natural mobs around it (mob-AI's).
        #[arg(long)]
        no_monsters: bool,
        /// A terrain sample's number of runs (default: `--parallel`), `--parallel` servers at
        /// once (each its own seed): enough runs for a narrow interval on the mean.
        #[arg(long, default_value_t = 0)]
        runs: usize,
        /// Only redsim's 64 reference runs of the gate's schedule (no game): each failed run named
        /// (a scan of the listed farms for runs whose world went wrong).
        #[arg(long)]
        sim_only: bool,
        /// A test of the gate's event log only (ironspec 2026-11-11): at gt 2000 of each build, a
        /// skeleton trap horse summoned on the farm and the first watched flammable block set to
        /// air; the log prints both with their position and time, and the gate fails on both.
        #[arg(long)]
        inject_events: bool,
    },
    /// redsim's over the same length.
    WeatherStats {
        #[arg(long, default_value_t = 200)]
        days: u32,
        #[arg(long, default_value_t = 8)]
        servers: usize,
        /// Sample every this many gt.
        #[arg(long, default_value_t = 100)]
        dt: u32,
        #[arg(long, default_value_t = 64)]
        sim_runs: u32,
    },
    /// mobinfra: lava's fire in a farm, the game's time to the first burned block beside redsim's.
    FireBurn {
        design: PathBuf,
        #[arg(long, default_value_t = 8)]
        servers: usize,
        #[arg(long, default_value_t = 48000)]
        max: u32,
        #[arg(long, default_value_t = 200)]
        seeds: u32,
    },
    /// mobinfra: bolts summoned in fixed scenarios (hits through roofs, the box's edge, a boat, a
    /// drop, a golem, the bolt's fire), the game's outcome beside redsim's.
    BoltCheck,
    /// (tickexact 2026-11-11, ironspec's surroundings question) A design's terrain baked into a new
    /// design file in the design's own coordinates (the judge's site in a sample: build K, the ground
    /// line the design y at the ground layer, `margin` cells of terrain round the box, the box as the
    /// site digs it), so every void-world runner runs it in its built surroundings unchanged
    Ground {
        design: PathBuf,
        /// The sample (flat: the price tool's ground, no sample; village-edge, plains, forest; default
        /// the design's `sample:`, else flat)
        #[arg(long)]
        sample: Option<String>,
        /// The design y at the ground layer (default the design's `ground:`, else -1: the price tool's)
        #[arg(long, allow_hyphen_values = true)]
        ground: Option<i32>,
        #[arg(long, default_value_t = 0)]
        build: usize,
        #[arg(long, default_value_t = 16)]
        margin: i32,
        #[arg(long)]
        out: PathBuf,
    },
    /// (tickexact 2026-11-11, ironspec's weather gap) A design's golems a game day, day by day, at the
    /// judge's site under the card's windows probe, the headline's natural cycle and no weather
    WeatherDays {
        design: PathBuf,
        #[arg(long, default_value_t = 16)]
        runs: u32,
        #[arg(long, default_value_t = 5)]
        days: u32,
        /// The judge's build (site) K (0: the cheapest, the card's)
        #[arg(long, default_value_t = 0)]
        build: u32,
        /// Also each day's golems in this many equal bins (a day's 24000 gt split), mean a run
        #[arg(long, default_value_t = 0)]
        bins: u32,
    },
    /// (ironspec 2026-11-11 12:50) The rail-junction check: every plain rail joined to three or four
    /// rails, its stored shape against the game's settle order for its signal at rest (both power
    /// states printed); exit status 1 when a stored shape would re-settle at its first update.
    JunctionCheck {
        design: PathBuf,
    },
    /// (tickexact 2026-11-11, ironspec's HOME rule) The walk-target static check: from every rail cell
    /// (a rider seated in a cart there), every cell its brain can take as a walk target (PANIC's flee
    /// 16/7 and stroll 2/2, IDLE's stroll 10/7 and walks to villagers within 8, job-site / bell walks,
    /// REST's HOME walk), the chance an event picks each, and whether a complete path reaches it
    FleeCheck {
        design: PathBuf,
        /// The bed (HOME) for REST's walk: `x,y,z` (default: the design's `port home:` line)
        #[arg(long, value_delimiter = ',')]
        home: Vec<i32>,
        /// Unreached cells printed a place
        #[arg(long, default_value_t = 6)]
        show: usize,
        /// More places a villager stands on foot: `x,y,z,x,y,z,...` (the rail cells within 1 of the
        /// bed are tried on foot already: a dropped napper)
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        foot: Vec<i32>,
        /// Print every place's clearing line (what can clear a stamp there: the panic stroll's
        /// reachable cells, or the calm stroll's reachable share), not only the places with none.
        #[arg(long)]
        clearing: bool,
        /// The eye check's positive control: a stone set at the first rail cell's rider eye (the
        /// suffocation gate must fail).
        #[arg(long)]
        eye_control: bool,
        /// The pickup gate's positive control: a boated zombie 0.1 beside the first rail cell's
        /// cart box, inside its pickup reach (the pickup gate must fail).
        #[arg(long)]
        pickup_control: bool,
        /// Each place's least zombie distance at the three sampled points only (default: also
        /// the exact least over the cart's whole path through the cell).
        #[arg(long)]
        sampled: bool,
    },
    /// One judge run of a design on redsim: its table.
    Run {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 1)]
        days: u32,
        /// Every golem stands still (default: a random policy each).
        #[arg(long)]
        still: bool,
        /// Build it into a standard terrain sample (village-edge, plains, forest) with natural
        /// spawning on Hard around its AFK spot (the design's `ground:`, `afk:`, `terrain:` meta).
        #[arg(long)]
        terrain: Option<String>,
        /// Only tick the built farm for `--days` (no checks, no report): the time a game day takes.
        #[arg(long)]
        ticks_only: bool,
        /// Print every change of these cells' blocks (`x,y,z`, repeatable) with its game time and
        /// day time, and each villager's sleeping, instead of the report.
        #[arg(long)]
        watch: Vec<String>,
        /// After the report, the N cells where living golems spent the most ticks (design
        /// coordinates; the run's occupancy).
        #[arg(long, default_value_t = 0)]
        cells: usize,
        /// Instead of the report: tick the run for `--days` and print the mobs by type (the most
        /// alive at once, and alive every 1000 gt summed), and the time the entity ticks took.
        #[arg(long)]
        census: bool,
        /// The judge's terrain build K instead of the cheapest spot: the K-th site of the design's
        /// sample (`circuit::mobfarm::terrain_sites`) with the judge's run options there (its
        /// weather cycle; `--seed` is then the run's: JUDGE_SEED0 + 1000 K + I for its run I).
        #[arg(long)]
        build: Option<usize>,
        /// Instead of the report: follow the entities of these kinds tick by tick (`villager,
        /// zombie_villager,iron_golem,item` or `item:iron_ingot`, `boat`, `all`): each as it
        /// appears and goes (a villager discarded and a zombie villager added at its spot in one
        /// tick is a conversion) and their mob events (deaths, hurts, drops, pickups, burns).
        #[arg(long)]
        log: Option<String>,
        /// With `--log`: only entities whose block cell lies in this world box `x0,y0,z0,x1,y1,z1`.
        #[arg(long)]
        log_box: Option<String>,
        /// With `--log`: the game-time window printed (the run ticks from its start regardless).
        #[arg(long, default_value_t = 0)]
        log_from: i64,
        #[arg(long, default_value_t = i64::MAX)]
        log_to: i64,
        /// With `--log`: each followed entity's line again every time it changes block cell.
        #[arg(long)]
        log_moves: bool,
        /// In terrain: first the site's seals (each cell's natural block, the sample's world
        /// before the build: a cave's air, or a gravity block the dig would set falling), and with
        /// `--watch` each watched cell's natural block and what the site does to it (sealed,
        /// capped, cleared); alone, only that.
        #[arg(long)]
        natural: bool,
    },
    /// A mob farm design's Nether part as a design of its own (its `nether layer` blocks and
    /// `nether nbt:` data as the blocks, its `port nether-<name>` ports as ports): to picture the
    /// Nether side (the kill, the collection) with `web shot`, whose parser reads the overworld only.
    NetherPart {
        design: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
    },
    /// A run's tick-level timing view (the carousel's parts and assembly: the parent and ironspec
    /// 2026-11-09): one judge run (`run`'s options: the same run), sampled after every tick, written
    /// as a page (`<out>/<stem>-seed<S>.timeline.html`: zoom, pan and pinch; a tap reads every lane
    /// at a tick) and a text dump (`.timeline.txt`: one line per change, `VIOLATION <rule>` lines):
    /// each farm villager's position class, activity, panic trigger, GOLEM_DETECTED_RECENTLY and its
    /// expiry, wanting a golem, cart, HOME, LAST_SLEPT and CANT_REACH; each cart's position, speed
    /// and rider; each bed's OCCUPIED; the design's tagged blocks and regions (`timeline: signal
    /// NAME x y z`, `timeline: region NAME x0 y0 z0 x1 y1 z1`; a region `station...` is the
    /// summoners' group); the gameTime % 100 slots, each summoner's 10-box and result, each spawning
    /// batch's arrival margin; the golems' spawns and exits; and the violations: the 10.6 rule at a
    /// spawn, the 16 rule, CANT_REACH over 1200 in REST, a golem sensor's sighting and any follow-box
    /// exposure away from the summoning batch, an arrival under 39 gt before the slot, no HOME at a
    /// dawn or midday, not exactly one sleep a night, a summon off the grid. `--from-dump FILE`
    /// instead draws a part bench's own trace in the dump's format (`t=<tick> gt=<gt> signal
    /// NAME=VALUE`, `mark NAME text`, `VIOLATION <rule> <lane>: text` lines) as `<out>/<stem>.timeline.html`.
    Timeline {
        /// The design (none with `--from-dump`).
        design: Option<PathBuf>,
        /// Draw this dump instead of running a design.
        #[arg(long)]
        from_dump: Option<PathBuf>,
        /// Read this bridge recording (a `.rec` of `mctas mobai record`: the game's own frames)
        /// instead of running a design; `--tag` coordinates are then the recording's world's.
        #[arg(long)]
        recording: Option<PathBuf>,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 1)]
        days: u32,
        /// Every golem stands still (default: a random policy each).
        #[arg(long)]
        still: bool,
        /// Build it into a standard terrain sample (as `run --terrain`).
        #[arg(long)]
        terrain: Option<String>,
        /// The judge's terrain build K (as `run --build`).
        #[arg(long)]
        build: Option<usize>,
        /// More tags, in the design's syntax (`signal NAME x y z`, `region NAME x0 y0 z0 x1 y1 z1`,
        /// design coordinates); repeatable.
        #[arg(long)]
        tag: Vec<String>,
        /// The window written: ticks after the arrival (default: the whole run).
        #[arg(long, default_value_t = 0)]
        from: u32,
        #[arg(long)]
        to: Option<u32>,
        #[arg(long, short, default_value = ".")]
        out: PathBuf,
    },
    /// Paste a part design into a base design (the station's pad and Nether part under a loop's
    /// stand, say): the part's blocks, block entities and entities moved by `--at`, its ports and
    /// Nether part (its anchor moved too), its `build:` steps joined to the base's; a cell both
    /// hold with different blocks is refused, and so is a port both name.
    Paste {
        base: PathBuf,
        part: PathBuf,
        /// The part's origin in the base (x,y,z).
        #[arg(long, default_value = "0,0,0", allow_hyphen_values = true)]
        at: String,
        #[arg(long)]
        out: PathBuf,
        /// A contested cell (both designs hold it, with different blocks), a port both name, a cell
        /// both declare differently or a second Nether part keeps this side (`base` or `part`)
        /// instead of refusing the paste; each conflict is printed and written into the file's
        /// header as a `# conflict:` line (an assembly that lists what it couldn't join).
        #[arg(long)]
        keep: Option<String>,
        /// Boxes of the base cleared before the paste (`;`-separated X0,Y0,Z0,X1,Y1,Z1, the base's
        /// frame, inclusive): their blocks, block entities and entities (by the cell they stand in)
        /// removed, each box's counts printed and written into the header (the part a pasted
        /// design replaces).
        #[arg(long, allow_hyphen_values = true)]
        clear: Option<String>,
        /// Leave every rail junction as stored (by default each plain-rail junction the game
        /// wouldn't keep is set to the shape it settles to at its first update for its power as
        /// placed, ironspec Nov 11 15:56, each one listed in the header; `junction-check` stays the
        /// gate).
        #[arg(long)]
        no_settle: bool,
    },
    /// The single-lane carousel's queue: a physics scene (carts back to back on one track, a
    /// release of the front ones down braked slopes) for `queue-run`.
    QueueLoop {
        #[arg(long, default_value_t = 18)]
        slots: i32,
        #[arg(long, default_value_t = 20)]
        run: i32,
        #[arg(long, default_value_t = 3)]
        depart: i32,
        #[arg(long, default_value_t = 0)]
        boost_every: i32,
        #[arg(long, default_value_t = 3)]
        boost_first: i32,
        /// The queue in one straight row (else folded in two rows with a U-turn).
        #[arg(long)]
        straight: bool,
        /// Ledges before the stand and the queue's tail (each cart's speed clamped to 0.4 off its
        /// rail; slower arrivals).
        #[arg(long)]
        drop: bool,
        /// V stand slots (slope down, valley, its own braked slope up) instead of catch slots.
        #[arg(long)]
        vstand: bool,
        /// The queue's 3 tail slots as plain sawtooth slots (else V slots with their own
        /// slope-up line).
        #[arg(long)]
        no_tail_v: bool,
        /// The earlier folded layout (the stand at the far end of the release run, the return
        /// back along it) instead of the bridge layout.
        #[arg(long)]
        folded: bool,
        /// The bridge layout's queue rows this far apart: 1 (a U-turn of two curves) or 2 (a
        /// curve, a booster, a curve).
        #[arg(long, default_value_t = 3)]
        row_gap: i32,
        /// A booster in the middle of the queue's U-turn (rows 2 apart).
        #[arg(long)]
        uturn_boost: bool,
        /// The line layout: the queue one straight row (15 slots), the return the long way round
        /// it (two batches on the return at a time).
        #[arg(long)]
        line: bool,
        /// The queue's 3 tail slots as V slots (or plain with `--plain`) instead of deep slots (a
        /// braked slope down linked to the slot's own, two plain slopes up).
        #[arg(long)]
        no_tail_deep: bool,
        /// Plain loop: plain sawtooth slots everywhere (stand and queue tail too), slow trains,
        /// each slot braked as its own cart comes, the stand's batch out like a queue advance.
        #[arg(long)]
        plain: bool,
        /// A descending stand: each slot a braked slope linked to its own and a plain slope up,
        /// a block lower each slot (the catch is a slope wherever the brake comes).
        #[arg(long)]
        stand_desc: bool,
        /// A detector rail on the release run, 4 cells before the stand's curve, to time the
        /// stand's brake (`queue-loop-run --stand-detect D`).
        #[arg(long)]
        detector: bool,
        /// Zombie posts (a zombie in a boat on glass, leaves over it) every this many cells
        /// along x `--zombie-x` from z 5 north (PANIC for the riders); 0: none.
        #[arg(long, default_value_t = 0)]
        zombie_every: i32,
        #[arg(long, default_value_t = 3, allow_hyphen_values = true)]
        zombie_x: i32,
        /// A deep stand: each slot a braked slope linked to its own, two plain slopes up and a
        /// peak (5 cells); a cart that climbs out of a braked slot rolls back into it.
        #[arg(long)]
        stand_deep: bool,
        /// The queue's 3 tail slots as descending deep slots (each a block lower, one plain slope
        /// up out of it), the return coming in at y 4.
        #[arg(long)]
        tail_desc: bool,
        /// With --tail-desc: the 3 spare cells as second peak cells on slots 3-5, not flat cells
        /// before the tail.
        #[arg(long)]
        peak_pad: bool,
        /// The departure turns north right after S1's peak.
        #[arg(long)]
        short_depart: bool,
        /// farmopt's pocket stand: each cart of the batch in its own gate pocket (counting gates,
        /// no brake timing, no contact), all three released at once.
        #[arg(long)]
        stand_stop: bool,
        /// With --stand-stop: the pockets' gates wired (a detector rail on pocket 1's flat, a
        /// two-torch latch, one input cell REL that opens all three and resets the latch), no
        /// bench triggers; `queue-loop-run` then drives REL only (`--pocket-release`, `--rel-for`).
        #[arg(long)]
        stand_wired: bool,
        /// With --stand-wired: the stand's fault guard: the booster-ups plain rails, each gate
        /// slope powered from its own gate's driver dust (boosting only while its gate is open),
        /// so no cart drifting back from a crowded pocket is boosted back through G1/G2; and C, the
        /// D1 fallback: the latch set at REL's fall + 37 gt if D1 has not set it (a timer on REL).
        #[arg(long)]
        stand_guard: bool,
        /// The whole design moved by X,Z (and the bench's frame with it, through the `queue:`
        /// line): the game's chunk lines fall where a build at that offset puts them (ironspec's
        /// alignment X & 15 = 6, Z & 15 = 6: `--shift 6,6`).
        #[arg(long)]
        shift: Option<String>,
        /// The re-packing tail: the returning carts roll into a FIFO of N gate pockets in place of
        /// the 3 tail slots (a pocket's gate open while the next pocket is empty; the last
        /// pocket's gate, the intake, opened once after each advance), the return coming in at
        /// y 1 + N; needs --stand-stop and --slots 15 (21 carts: 15 + the stand's 3 + the FIFO's 3).
        #[arg(long, default_value_t = 0)]
        tail_fifo: u32,
        /// The escapement tail: the returning carts roll down a gravity queue of Q plain slopes onto
        /// B (braked, holds the next cart), X (plain), P (braked, against a gate, pulsed with it),
        /// R (plain), then slot 0 a sawtooth slot (one cart into slot 0 per advance; queue-loop-run
        /// --fifo-intake and --esc-b time it); the return comes in at y Q + 5; needs --stand-stop
        /// and --slots 14 (21 carts: 14 + the stand's 3 + the escapement's 4, one left waiting on P
        /// between cycles).
        #[arg(long, default_value_t = 0)]
        tail_esc: u32,
        /// The departure climb's boosted cells from its foot, the rest plain slopes (the returning
        /// carts' speed into the tail); default all boosted.
        #[arg(long)]
        climb_boost: Option<u32>,
        /// C' wired (with --approach 5 --stand-guard): the approach detector's edge circuit in
        /// place of C's timer; its six repeater delays R_in,R1,R2,R3,R4,R5 in redstone ticks.
        #[arg(long, value_delimiter = ',')]
        cprime: Vec<i32>,
        /// With --approach: the climb's last slope (the cell before T) an always-on booster.
        #[arg(long)]
        approach_boost: bool,
        /// With --approach: every climb slope a booster but the detector D (with --boost-first 0:
        /// the release run plain).
        #[arg(long)]
        approach_climb_boost: bool,
        /// With --depart-crest: the dip past the line as plain flat rails (the boosted rise
        /// climbs alone).
        #[arg(long)]
        crest_plain_dip: bool,
        /// With --depart-crest: this many plain slopes down from the crest (default: as many as
        /// --depart-crest), the dip at the level they reach.
        #[arg(long, default_value_t = 0)]
        crest_descent: u32,
        /// With --part-departure: the part's column after the crest's descent built for a
        /// hand-off corner at the departure's column, height Y, z Z (ironfarm's R1: 13,24): the
        /// dip at the descent's level, then boosted slopes up to Y ending at Z - 1; the loop's
        /// own departure is not changed.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        part_handoff: Vec<i32>,
        /// With --part-departure: the `border:` header's allowed cells, the dip's cells with z in
        /// Z0..=Z1, as push-in rows show them clean (instead of the 1.2 rule's distance).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        part_allowed: Vec<i32>,
        /// Also write the stand's departure alone as a part design (G0 and its slope, the climb,
        /// the crest, the dip, the rise, the U-turn; `border:` and the hand-off cell).
        #[arg(long)]
        part_departure: Option<PathBuf>,
        /// The escapement tail's entry: one never-powered braked slope instead of the ledge.
        #[arg(long)]
        tail_brake: bool,
        /// With --tail-brake: that slope plain (no speed reducer; for slow returns).
        #[arg(long)]
        tail_plain: bool,
        /// The departure climb: a booster every Nth cell from its foot, plain slopes between.
        #[arg(long, default_value_t = 0)]
        climb_every: u32,
        /// The ledge's landing as a booster slope then a plain slope (not two flats).
        #[arg(long)]
        ledge_slope: bool,
        /// The escapement's B door: an iron door at B's exit, opened with B's pulse.
        #[arg(long)]
        esc_door: bool,
        /// The flat escapement: B a flat plain rail held by a fence gate, X an always-on booster.
        #[arg(long)]
        esc_flat: bool,
        /// The ledge's landing: both landing flats boosters.
        #[arg(long)]
        landing_boost2: bool,
        /// The escapement's G0: a fence gate at B's entry, shut through B's pulse.
        #[arg(long)]
        esc_g0: bool,
        /// Admission control (with --tail-esc Q, Q queue slopes): the gravity queue, G0 (latched:
        /// opened once per advance, shut when a cart is on P), P/G_P, R.
        #[arg(long)]
        esc_adm: bool,
        /// Admission control's entry: escL3's ledge inside the tail (else the queue's slopes start
        /// at the curve).
        #[arg(long)]
        adm_ledge: bool,
        /// Admission control: a mid-queue gate G1 over the K-th queue slope (0: none).
        #[arg(long, default_value_t = 0)]
        adm_g1: u32,
        /// Admission control's short entry (with --adm-ledge): the entry curve as the pre-ledge,
        /// a one-cell booster landing (two more queue slopes fit).
        #[arg(long)]
        adm_short: bool,
        /// Admission control: the last queue slope (the front's rest against G0) an always-on
        /// booster.
        #[arg(long)]
        adm_qboost: bool,
        /// Admission control: B, a flat always-on booster at the queue's foot (the front pressed
        /// against G0 on it); P always on with it.
        #[arg(long)]
        adm_flatb: bool,
        /// Admission control: R (the slope under G_P into slot 0) a powered rail pulsed at each
        /// launch (tail-bench / queue-loop-run `--adm-rpulse`).
        #[arg(long)]
        adm_rboost: bool,
        /// Admission control: P a sloped detector rail (senses its cart; the launch on gravity).
        #[arg(long)]
        adm_pdet: bool,
        /// Admission control's entry-hold sensors: the plain landing flat and the third queue
        /// slope as detector rails (the stand's REL waits while either is on).
        #[arg(long)]
        adm_sensors: bool,
        /// The room detector's queue slope (0 = the top one, by the landing).
        #[arg(long, default_value_t = 2)]
        adm_room_k: u32,
        /// The top queue slope a detector too: the landing latch's reset (the cart moved on).
        #[arg(long)]
        adm_latch_reset: bool,
        /// Admission control's x-queue (ironspec's R1): N plain slopes down west along z 24, the
        /// curve north and one flat transition cell, all upstream of the forced chunk border z
        /// 25|26, then the --tail-esc slopes north into G0 and P.
        #[arg(long, default_value_t = 0)]
        adm_xq: u32,
        /// The zombie post beside the escapement (loop z 29) moved to this x, clear of the
        /// drivers east of G0 and G_P (day-1: x 6).
        #[arg(long, allow_hyphen_values = true)]
        tail_post_x: Option<i32>,
        /// The x-queue's ledge: the gap's width in cells.
        #[arg(long, default_value_t = 1)]
        xq_gap: u32,
        /// The x-queue's ledge: the landing's drop below the take-off's level.
        #[arg(long, default_value_t = 2)]
        xq_drop: u32,
        /// The x-queue's row (default 24): 25 lays it on farmopt's (c′) departure's corner row,
        /// the curve (0,y,25) then the border's flat cell.
        #[arg(long)]
        xq_z: Option<i32>,
        /// Row A's slot count (default by the tail's formula; the rest are row B's): the S3
        /// bench's C register behind R1's tail is --slots 16 --row-a 5.
        #[arg(long)]
        row_a: Option<i32>,
        /// The 6-8 rule's zombie posts beside R1's tail (each the boat's cell X,Y,Z, glass under,
        /// leaves 3 over; separated by `;`), replacing the strip's posts at z 21 and 29.
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        tail_posts: String,
        /// farmopt's approach lab (with --stand-stop): the release run climbs to a flat top, then
        /// its last N cells before pocket 3's rest slope come down with no flat and no curve (N - 2
        /// slopes down, a snap turn onto an east-west slope, pocket 3's cell a slope); N >= 2.
        #[arg(long, default_value_t = 0)]
        approach: u32,
        /// The departure's crest (with --stand-stop): the climb ends in a flat crest D cells before
        /// the chunk line z 9|10, D slopes down to the line, boosters past it and a boosted climb
        /// back to the tail's level.
        #[arg(long, default_value_t = 0)]
        depart_crest: u32,
        #[arg(long)]
        out: PathBuf,
    },
    /// The carousel brief's "The site (the placed farm)" (ironfarm's 7b8d9a006 and its amend;
    /// ironspec and G1 2026-11-11): the five site conditions against the placed farm (lightning,
    /// natural mobs, empty-handed zombies, no village POI within 8 sections, the farm's zombies
    /// stay), one verdict each with its findings, then each condition's positive controls (a copy
    /// of the placed farm with one change, which must fail). Exits 1 on a FAIL or on a control
    /// that does not bite.
    SiteCheck {
        design: PathBuf,
        /// Built into this terrain sample at the judge's cheapest spot (village-edge, plains,
        /// forest); without it, the void.
        #[arg(long)]
        terrain: Option<String>,
        /// More villager positions: files of `x y z` lines in design cells (`queue-loop-run
        /// --record-positions` over a full bench night), as queue-loop-spots reads them.
        #[arg(long, value_delimiter = ',')]
        positions: Vec<PathBuf>,
        /// Every rail cell of the design is a villager position too.
        #[arg(long)]
        rails: bool,
        /// A lightning rod the site guide adds, at design column X,Z: a stone pillar from the
        /// column's top to 1 above the farm's top, the rod on it (repeatable).
        #[arg(long, allow_hyphen_values = true)]
        rod: Vec<String>,
        /// The verdicts alone, no controls.
        #[arg(long)]
        no_controls: bool,
        /// The AFK player's feet, design coordinates x,y,z (the site guide's spot; default the
        /// nearest spot to the site's default that lies 2 sections from the shared bed's, printed).
        #[arg(long, allow_hyphen_values = true)]
        afk: Option<String>,
        /// An empty cart's per-cell speeds from a runner trace: `x y z v` lines (design cells,
        /// blocks a tick, the fastest crossing); pickup then counts only the cells a cart passes
        /// at 0.1 or faster (without it, every track; the line says which).
        #[arg(long)]
        speeds: Option<PathBuf>,
    },
    /// Two designs compared cell by cell (a lab change's cost): the cells only B has (added), only
    /// A has (removed) and those both hold with different blocks (changed), each list counted by
    /// block kind; positions moved by --shift X,Z first (B's frame back to A's).
    CellDiff {
        a: PathBuf,
        b: PathBuf,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        shift: Vec<i32>,
    },
    /// The golem-spots check (the brief's "no golem spots off the pad"): the game's summon search
    /// from every position a villager can occupy: a loop scene's rail cells, plus the positions
    /// in --positions files (x y z per line: `queue-loop-run --record-positions` over a full bench
    /// night) and the design's own `positions:` line (x,y,z ...), moved by --offset. Block-only
    /// (a cell only an entity covers counts as a spot) unless --with-entities.
    QueueLoopSpots {
        design: PathBuf,
        #[arg(long, value_delimiter = ',')]
        positions: Vec<PathBuf>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        offset: Vec<i32>,
        /// Only the positions inside this box (in the positions' own frame, before --offset):
        /// x0,y0,z0,x1,y1,z1 (a part's own positions out of a whole assembly's recorded night).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        within: Vec<i32>,
        /// Every rail cell of the design is a position too (a rider sits in its cart's cell: every
        /// branch, a give-up path included).
        #[arg(long)]
        rails: bool,
        #[arg(long)]
        with_entities: bool,
        /// How many cells to list (default 40).
        #[arg(long, default_value_t = 40)]
        list: usize,
        /// The pad, a box x0,y0,z0,x1,y1,z1 (or the design's `pad:` line): spots inside it are
        /// allowed only when every position that sees them is a stand position; a pad cell seen
        /// from another position is an out-of-turn summon, listed on its own line.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        pad: Vec<i32>,
        /// The stand positions, "x,y,z;x,y,z;..." (or the design's `stand:` line), besides a loop
        /// scene's own stand cells and pockets.
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        stand: String,
    },
    /// The stop-based stand's lab (farmopt): a run, the stand's cells from a spec (`mobfarm::
    /// standlab`), three ridden carts moving +x at each speed of a sweep: where they rest, when,
    /// whether they stay put; then the departure (the spec's `c` cells powered) and how they leave.
    StandLab {
        /// The stand's cells from x 0 east (e.g. "rv rv b. c^ c^").
        #[arg(long)]
        spec: String,
        /// Stored speeds on the run: "a,b,step".
        #[arg(long, default_value = "0.4,2.0,0.2")]
        speeds: String,
        /// Added to the 2nd and 3rd carts' speeds: "a,b".
        #[arg(long, default_value = "0,0")]
        skew: String,
        /// The mob AI on (the loop bench's setting).
        #[arg(long)]
        ai: bool,
        /// The back cart first in the entity list (the loop's order).
        #[arg(long)]
        reverse: bool,
        /// The train's two gaps (cells, front to back).
        #[arg(long, default_value = "3,3")]
        gaps: String,
        #[arg(long, default_value_t = -2.5)]
        start_x: f64,
        #[arg(long)]
        depart_at: Option<i64>,
        /// The `k` cells (powered for the catch) braked at this tick, powered again at
        /// `--release-at`.
        #[arg(long)]
        hold_at: Option<i64>,
        #[arg(long)]
        release_at: Option<i64>,
        /// Metering: 3 departure pulses this many gt apart, one cart each.
        #[arg(long)]
        meter: Option<i64>,
        /// Metering: the gate shuts once a leaving cart's centre is this far past the gate cell's
        /// west edge.
        #[arg(long, default_value_t = 1.29)]
        close_at: f64,
        /// Pockets: every gate its own stop (shut by the falling-edge logic), released gate 0,
        /// 1, 2 at `--depart-at` + k `--meter`.
        #[arg(long)]
        pockets: bool,
        /// Pockets: the stand's gate `S` opened (its `c` cells powered) at this tick, shut again
        /// at `--stand-close-at`.
        #[arg(long)]
        stand_open_at: Option<i64>,
        #[arg(long)]
        stand_close_at: Option<i64>,
        /// Pockets: gate k opened (with every gate east of it) at `--depart-at` + the k-th offset.
        #[arg(long, default_value = "0,12,24")]
        releases: String,
        #[arg(long, default_value_t = 300)]
        ticks: i64,
        /// Write the lab's design here too.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        trace: bool,
    },
    /// Slot feed from above: a ridden cart rolled off a ledge H above a register slot, west of it,
    /// at a cycle's tick, onto the slot's cell; the register advancing at the loop's times; prints
    /// the landing, the rest, the slot at each advance, falls, doubles, order, merges, the rider.
    SlotDrop {
        design: PathBuf,
        /// The target slot (an index into the register's slots, 0 = the tail's end).
        #[arg(long)]
        slot: usize,
        /// The ledge's rail height above the slot rail.
        #[arg(long, default_value_t = 2)]
        h: i32,
        /// The cart's speed east on the ledge (b/t).
        #[arg(long, default_value_t = 0.1)]
        v: f64,
        /// The tick within the cycle at which the cart starts on the ledge (cycle 1).
        #[arg(long)]
        dt: i64,
        /// Leave the slot's own cart in place (a cart already in the slot).
        #[arg(long)]
        occupied: bool,
        /// Cycles run after the drop's.
        #[arg(long, default_value_t = 2)]
        after: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Run seeds 1..=N in parallel and print each one's verdict, then the clean count.
        #[arg(long)]
        seeds: Option<i64>,
        #[arg(long, default_value_t = 38)]
        advance_at: i64,
        #[arg(long, default_value_t = 20)]
        advance_every: i64,
        #[arg(long, default_value_t = 9)]
        pulse: i64,
        /// The dropped cart's path for 60 gt in the report.
        #[arg(long)]
        trace: bool,
        /// The ledge's last rail a powered rail kept on (the floor booster).
        #[arg(long)]
        boost: bool,
    },
    /// The re-packing tail's arrival injector: a `queue-loop --tail-esc` design's tail benched
    /// alone, ridden carts spawned on the return run per `--pattern` (per cycle
    /// `N[/GAP][@DT][:SPEED][*REPEAT]`, defaults 3/12@76:1.2; 0 = nothing), the register's first
    /// slots advancing at the loop's times; prints the escapement's contract lines.
    TailBench {
        design: PathBuf,
        #[arg(long)]
        pattern: String,
        /// Cycles run after the pattern's.
        #[arg(long, default_value_t = 4)]
        extra: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Seeds 1..=N in parallel (the summary line per seed).
        #[arg(long)]
        seeds: Option<i64>,
        #[arg(long, default_value_t = 38)]
        advance_at: i64,
        #[arg(long, default_value_t = 20)]
        advance_every: i64,
        #[arg(long, default_value_t = 9)]
        pulse: i64,
        /// The gate and P: opened D gt after each advance's start, for W gt.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        fifo_intake: Vec<i64>,
        /// B: powered K gt after the gate opens, for W gt.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        esc_b: Vec<i64>,
        /// G0 (a design with --esc-g0): shut from LEAD gt before B's pulse to LAG gt after it.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        esc_g0: Vec<i64>,
        /// Admission control (a design with --esc-adm): G0 opened DELAY gt after the gate (G_P)
        /// opens, shut by a cart crossing A' or after MAX gt.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        esc_adm: Vec<i64>,
        /// Admission control's latch: G0 also shuts once a cart is on P and stays shut while P
        /// holds one (else G0 is a plain timed pulse).
        #[arg(long)]
        adm_latch: bool,
        /// Admission control: P always powered (held by G_P alone) from the first gate on.
        #[arg(long)]
        p_always: bool,
        /// Admission control: G1's window in gt (default: G0's).
        #[arg(long, default_value_t = 0)]
        adm_g1_w: i64,
        /// Admission control: G0's delay after the gate opens for the third advance of each cycle
        /// (−1: as the others).
        #[arg(long, default_value_t = -1, allow_hyphen_values = true)]
        adm_k2: i64,
        /// Admission control's R booster (`--adm-rboost` designs): R's control on for this many gt
        /// from each G_P opening.
        #[arg(long, default_value_t = 0)]
        adm_rpulse: i64,
        /// The rider-space check's positive control: stones at X,Y,Z;... (the design's frame)
        /// placed before the check.
        #[arg(long)]
        rider_space_control: Option<String>,
        /// Declared sight stretches added to the design's `sight:` (X0,Y0,Z0..X1,Y1,Z1;...): their
        /// riders keep every zombie within 8 in sight.
        #[arg(long)]
        sight: Option<String>,
        /// Every cart's position each tick in the log.
        #[arg(long)]
        trace: bool,
        /// The design placed shifted by DX,DZ (its chunk alignment: `chunk: 0 0 0 6-6 6-6` is 6,6).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        shift: Vec<i32>,
        /// Carts spawned at the departure curve (they ride the climb and the return run).
        #[arg(long)]
        from_climb: bool,
        /// A partial unload: cycle C, tick DT, the chunk box X0,Z0,X1,Z1 (shifted world), back N
        /// ticks later (repeatable).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, num_args = 7)]
        unload: Vec<i64>,
        /// Write the bench's block changes as a drive for tick-exact's bridge.
        #[arg(long)]
        drive_out: Option<PathBuf>,
        /// Replay a loop's arrival stream (queue-loop-run `--arrivals-out`): each cart spawned on
        /// the return run where and when the loop's crossed x 8.5, at its speed (with `--pattern
        /// 0*N` for N cycles).
        #[arg(long)]
        arrivals: Option<PathBuf>,
        /// With `--scene-out`/`--drive-out`: the scene holds the tail's carts as at cycle C's
        /// start (position, motion, facing, a villager riding) and the drive starts there.
        #[arg(long)]
        scene_at: Option<i64>,
        /// Write the design reduced to what the bench keeps (the tail's carts and the other
        /// entities) as a scene file for the bridge.
        #[arg(long)]
        scene_out: Option<PathBuf>,
        /// Replace the design's declared flights (a positive control of the off-the-rails
        /// check): `X,Y,Z>X,Y,Z|X,Y,Z@AIR; ...`.
        #[arg(long)]
        flights: Option<String>,
    },
    /// The summon-frame defaults' test (ironspec Nov 11 13:13): queue-loop-run's default bench
    /// timing (release group, register advances) against day-1's wired C24 core, by per-tick
    /// trace-diff on its build (default: the c24u-pad fixture): the wired reference, the bench
    /// driving the release group on the defaults (IDENTICAL required), and the 1-gt controls
    /// (release start 3 and 5, advance-at 31, offs one earlier: each must differ).
    FrameCheck {
        /// The wired build (day-1's c24u-pad: release-gen at explore/day1-sched-wip 37fe1c656 with
        /// main's pad wip/pad-a247e2fc2.txt).
        #[arg(default_value = "crates/mobfarm/fixtures/c24u-pad.txt")]
        design: PathBuf,
        #[arg(long, default_value_t = 10)]
        cycles: i64,
        /// The wired input (the golem pulse's cell) of that build.
        #[arg(long, default_value = "-2,-5,20", allow_hyphen_values = true)]
        wired: String,
    },
    /// Two release traces (queue-loop-run with MCTAS_RELEASE_TRACE=path) compared tick by tick,
    /// the bench-against-wired check: the release group's 3 rails' power and every cart on the
    /// release run, the first difference of each, their counts and the largest position gap.
    /// The hold bench (ironspec Nov 11 15:44, the metered hold ahead of R1's queue): a straight
    /// line built in code (a descent of plain slopes, a brake slope, flats, the hold gate H_R over
    /// the queue's first slope); carts launched at the descent's top at the given stored speeds,
    /// `--gap` gt apart: each one's speed leaving the brake, its stored speed at any contact with a
    /// cart ahead (the brief's 0.2 bar), where it rests, any left on the brake.
    BrakeBench {
        #[arg(long, value_delimiter = ',', default_values_t = [0.4, 0.8, 1.2])]
        speeds: Vec<f64>,
        /// Empty carts (default ridden).
        #[arg(long)]
        empty: bool,
        /// Per cart, 1 ridden or 0 empty (the last repeats), over --empty.
        #[arg(long, value_delimiter = ',')]
        riders: Vec<u8>,
        #[arg(long, default_value_t = 3)]
        descent: i32,
        /// Plain slopes down after the brake, before the flats (a bounced cart can't climb back).
        #[arg(long, default_value_t = 0)]
        post: i32,
        #[arg(long, default_value_t = 3)]
        flats: i32,
        #[arg(long, default_value_t = 12)]
        gap: i64,
        #[arg(long, default_value_t = 300)]
        ticks: i64,
        /// No brake (a plain slope in its place): the control.
        #[arg(long)]
        no_brake: bool,
        /// The brake window: each speed alone in its own world (bar 1).
        #[arg(long)]
        window: bool,
    },
    /// The pocket-module bench (farmopt Nov 11 16:26, the shared module bench): N pockets of
    /// 9fc3155f's 3-cell module (P detector flat, R rest slope, G gate column) after a descent,
    /// then a slot-0 stand-in; a latch read from the P detectors' real powered state, the
    /// admission spacer, front-first releases at slot 0's beats; faults (missed:S:N, double:S:N,
    /// freeze:T:D, ghost:S:T).
    PocketBench {
        #[arg(long, default_value_t = 3)]
        pockets: usize,
        #[arg(long, value_delimiter = ',', default_values_t = [1.0, 1.0, 1.0])]
        speeds: Vec<f64>,
        /// Per cart, 1 ridden or 0 empty (the last repeats).
        #[arg(long, value_delimiter = ',', default_values_t = [1u8])]
        riders: Vec<u8>,
        /// Launch spacing, gt (the last repeats).
        #[arg(long, value_delimiter = ',', default_values_t = [24i64])]
        gaps: Vec<i64>,
        #[arg(long, default_value_t = 2)]
        descent: i32,
        #[arg(long, default_value_t = 400)]
        ticks: i64,
        #[arg(long, default_value_t = 24)]
        beat: i64,
        #[arg(long, default_value_t = 40)]
        beat_at: i64,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        corner: i64,
        #[arg(long, default_value_t = 16)]
        lead: i64,
        #[arg(long, default_value_t = 8)]
        pulse: i64,
        #[arg(long, default_value_t = 2)]
        wire: i64,
        #[arg(long, default_value_t = 24)]
        spacer: i64,
        #[arg(long)]
        fault: Vec<String>,
        /// Cells per pocket: 3 (P, R, G) or 5 (P, R, G, B, S).
        #[arg(long, default_value_t = 5)]
        pocket_cells: usize,
        /// Front-first releases (default: every pocket gate together, the latch reset).
        #[arg(long)]
        front_first: bool,
        /// The normally-shut entry (7b90c3f1, ironspec Nov 11 17:19): the entry pocket's gate G_0
        /// is shut except to admit, opening when pocket 0 is latched and pocket 1 free, and
        /// shutting at P_1's rising edge (the admitted cart in pocket 1; the spacer then holds it)
        /// or after MAX gt; 0 = off (G_0 shut only while pocket 1 is latched or in the spacer).
        #[arg(long, default_value_t = 0)]
        entry_admit: i64,
        /// Feed-gated launches (7b90c3f1): a cart launches only while pocket 0 is latched free and
        /// no launched cart is short of P_0 (the hold's feed, driven by its entry pocket); the
        /// together release leaves pocket 0's latch to P_1's edge.
        #[arg(long)]
        feed_gated: bool,
    },
    /// The gravity-queue bench (ironspec Nov 11 16:38; paper v0 2098083320824660): a tail chain of
    /// the stand's pockets, a straight pressed run fed by count, R1's escapement (G0 over a powered
    /// slope, G_P over a plain one), a 3-pocket hold released together at T+3, a sink and the climb
    /// back; carts per opening, gate edges on carts, arrival contacts, the run's fill.
    GqBench {
        #[arg(long, default_value_t = 21)]
        carts: usize,
        /// Per cart, 1 ridden or 0 empty (repeating).
        #[arg(long, value_delimiter = ',', default_values_t = [1u8])]
        riders: Vec<u8>,
        #[arg(long, default_value_t = 5)]
        tail: usize,
        #[arg(long, default_value_t = 12)]
        run: i32,
        #[arg(long, default_value_t = 12)]
        cap: usize,
        #[arg(long, default_value_t = 8)]
        cycles: i64,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        corner: i64,
        #[arg(long, default_value_t = 8)]
        pulse: i64,
        #[arg(long, default_value_t = 2)]
        wire: i64,
        #[arg(long, default_value_t = 24)]
        spacer: i64,
        #[arg(long, default_value_t = 170)]
        ret: i64,
        #[arg(long, default_value_t = 6)]
        ret_gap: i64,
        #[arg(long, default_value_t = 0.4)]
        ret_speed: f64,
        /// T,D: every cart frozen from tick T for D gt (repeatable).
        #[arg(long)]
        freeze: Vec<String>,
        #[arg(long, default_value_t = 32)]
        hold_pulse: i64,
        /// The hold's release as a front-first ripple of D gt (0: together).
        #[arg(long, default_value_t = 0)]
        hold_ripple: i64,
        /// Live gt after the last tail pocket's pass edge before its release.
        #[arg(long, default_value_t = 16)]
        settle: i64,
        /// The overfill junction instead of the top-cell-free feed.
        #[arg(long)]
        overfill: bool,
        /// Overfill: live gt after a G0 admission edge before the last tail gate shuts.
        #[arg(long, default_value_t = 14)]
        adm_settle: i64,
        /// The intake keyed on the escapement's admission: one intake per G0 opening edge.
        #[arg(long)]
        intake_admit: bool,
        /// The ideal tail: no tail chain, each returning cart placed at rest at the last gate.
        #[arg(long)]
        ideal_tail: bool,
        /// No flat between the intake's G5 and the run.
        #[arg(long)]
        no_intake_flat: bool,
        /// The run filled pressed (0.79 apart) from G0 up to this far past the last tail gate (0: a
        /// cart a cell).
        #[arg(long, default_value_t = 0.0)]
        back_at: f64,
        /// (cc767b78) Day-1's live-time cycle clock and liveness gate (crate::liveness): the cycle
        /// restarts at each golem event (the column live at a summon boundary), every pulse (the
        /// hold at T+3, G_P and G0's openings, the tail chain's release) ANDed with ALIVE at its
        /// rise, a risen pulse completing, the cycle's unfired remainder dropped when ALIVE falls.
        #[arg(long)]
        alive_gate: bool,
        /// (cc767b78) T,D: a real freeze: the whole bench at level 32 inside a forced ring of
        /// chunks (redstone running, entities frozen) from tick T for D gt (repeatable).
        #[arg(long)]
        freeze32: Vec<String>,
        /// (cc767b78) The freeze matrix: "T0,T1,STEP;D1,D2,..[;real]": one run per freeze start
        /// (T0..T1 every STEP gt) and length, in parallel (--jobs), with --freeze (or, with
        /// `real`, --freeze32); one line per run with the bars.
        #[arg(long, default_value = "")]
        freeze_matrix: String,
        #[arg(long, default_value_t = 1)]
        jobs: usize,
        /// (7b90c3f1) The batch receiver: the tail chain's first three pockets in timed mode (the
        /// front Tail(2) shut except to feed, the middle Tail(1) and rear Tail(0) open until their
        /// committed timers T_MID,T_REAR, live gt from the hold's release (paused while ALIVE is
        /// low, never dropped)); after the batch settles, the front feeds one cart into Tail(3)
        /// when it is latched free and the spacer since its last P edge has run, and the pockets
        /// behind advance on each vacated pocket's own P edge (a live timeout ends an empty one).
        #[arg(long, value_delimiter = ',')]
        receiver: Vec<i64>,
        /// (7b90c3f1) The batch's return offsets: the j-th cart sunk after a hold release comes back
        /// at the release + ret + OFFS[j] (the climb table's per-pair spacing; default: ret_gap).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        ret_offs: Vec<i64>,
        /// (7b90c3f1) The receiver's feed: live gt after the rear timer before the first feed (the
        /// batch at rest), the front's feed pulse (shut early at Tail(3)'s P edge), the advance
        /// timeout (live gt waiting for a vacated pocket's refill edge).
        #[arg(long, default_value_t = 6)]
        recv_settle: i64,
        #[arg(long, default_value_t = 30)]
        recv_feed: i64,
        #[arg(long, default_value_t = 40)]
        recv_timeout: i64,
        /// (7b90c3f1) The receiver alone: a cart fed into Tail(3) leaves at once and comes back at
        /// the next hold release + ret + OFFS[j] (an ideal downstream; use with --carts 3).
        #[arg(long)]
        recv_ideal: bool,
        /// (7b90c3f1) The receiver's flying feed: the front stays open through the advances, each
        /// cart behind flying through from the middle's face (its feed spacing printed).
        #[arg(long)]
        recv_fly: bool,
        /// (7b90c3f1) The receiver's middle and rear shuts keyed on the front pocket's P (Tail(2)'s)
        /// rising edge (crossed only by the batch's first cart while the receiver is idle) plus
        /// D_MID,D_REAR live gt, instead of --receiver's timers from the hold release.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        receiver_p: Vec<i64>,
        /// (7b90c3f1) Flats between the receiver's front gate and the U into the next pocket
        /// (tail-v5: G3, flat, curve, four flats, U into P4: about 6).
        #[arg(long, default_value_t = 0)]
        recv_run: i32,
    },
    TraceDiff {
        a: PathBuf,
        b: PathBuf,
        /// Positions closer than this (blocks, on every axis) count as equal.
        #[arg(long, default_value_t = 0.0001)]
        tol: f64,
    },
    /// The static job-site check (ironspec Nov 11 08:53): every job-site block (composter, barrel,
    /// lectern, smoker, blast furnace, grindstone, loom, stonecutter, cartography table,
    /// fletching table, brewing stand, cauldron, smithing table) or bell within `--range` of a
    /// place a rider can be (a rail cell or the cell above it, a bed), per design; any hit fails.
    JobsiteCheck {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 48.0)]
        range: f64,
        /// A block added before the check (`X,Y,Z,BLOCK`, e.g. a composter: the positive control).
        #[arg(long)]
        add: Vec<String>,
    },
    /// The ledge's follower-gap sweep on redsim: the design's first declared flight, two carts off
    /// its take-off, the follower GAP gt behind (fractional), at each stored speed, empty and
    /// ridden; each pair clean, stuck, merged or derailed, printed as bands.
    LedgeSweep {
        design: PathBuf,
        #[arg(long, value_delimiter = ',', default_values_t = [1.5, 1.82, 1.96, 2.2])]
        speeds: Vec<f64>,
        #[arg(long, default_value_t = 0.0)]
        gap_from: f64,
        #[arg(long, default_value_t = 10.0)]
        gap_to: f64,
        #[arg(long, default_value_t = 0.25)]
        gap_step: f64,
        /// Which riders: `both` (default), `ridden` or `empty`.
        #[arg(long, default_value = "both")]
        riders: String,
        #[arg(long, default_value_t = 60)]
        ticks: u32,
    },
    /// S3's bench file with R1's tail (farmopt Nov 11 10:47): the base loop's old departure,
    /// return run and register head taken out (--remove, the loop frame), the stand departure part
    /// laid ((c′), the loop frame) and R1's tail from a queue-loop design that has it (its cells in
    /// --tail-box plus its tail posts' columns); the header joined (declared, flights, border,
    /// handoff, the queue line with R1's options and the tail file's slots and row A).
    R1Combine {
        /// The base loop (its frame the loop frame moved by its queue line's shift).
        base: PathBuf,
        /// The stand departure part, in the loop frame.
        #[arg(long)]
        departure: PathBuf,
        /// R1's source: a queue-loop design with R1's tail and the base's register, in the loop
        /// frame.
        #[arg(long)]
        tail: PathBuf,
        /// The base's boxes taken out (the loop frame; `;`-separated X0,Y0,Z0,X1,Y1,Z1): by default
        /// the old departure past the climb's foot and the return run, register head and tail
        /// posts it replaces.
        #[arg(long, default_value = "9,-6,1,10,16,20;0,-2,19,8,8,32", allow_hyphen_values = true)]
        remove: String,
        /// R1's cells in the tail file (the loop frame, X0,Y0,Z0,X1,Y1,Z1).
        #[arg(long, default_value = "0,-3,21,8,16,32", allow_hyphen_values = true)]
        tail_box: String,
        #[arg(long, default_value = "carousel-loop-s3-r1")]
        name: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// R1's queue-contact scenes for the merge bridge: the escapement column of a queue-loop
    /// design in 4 lanes (RR, RE, ER, EE: the waiting cart's rider, then the arrival's), case g0 (an
    /// arrival into the front pressed at G0) or p (the admitted cart meeting P's cart), one scene
    /// file and drive per arrival speed in --out-dir, each also run on redsim and scored as `mctas
    /// mobai rec-carts` scores a recording.
    ContactScenes {
        design: PathBuf,
        /// g0 or p.
        #[arg(long, default_value = "g0")]
        case: String,
        #[arg(long, value_delimiter = ',', default_values_t = [0.5, 0.55, 0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95, 1.0])]
        speeds: Vec<f64>,
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 100)]
        ticks: u32,
        /// Where the waiting cart starts along its cell: 0.885 at rest pressed on the gate (as the
        /// design's tail carts rest), 0.5 the cell's middle (rolling in, as slope-merge's lanes).
        #[arg(long, default_value_t = 0.885)]
        front_frac: f64,
        /// Case p: the arrival starts at G0's rest (the admitted cart's start when G0 opens)
        /// instead of the middle of the slope 2 cells before G0.
        #[arg(long)]
        from_rest: bool,
    },
    /// The zombie-post search for the 6-8 rule: the posts to re-lay removed (each `X,Z`, the
    /// boat's column: its boat and zombie, the glass under it and the leaves over it), then posts
    /// picked greedily from --area so every rail cell in --panic sees a zombie within 8 while every
    /// rider point of the design stays 6 or more from every post (both less --margin).
    PostSearch {
        design: PathBuf,
        /// The posts to re-lay, `X,Z` each (separated by `;`).
        #[arg(long, default_value = "")]
        remove: String,
        /// The rail cells that need panic: a box X0,Y0,Z0,X1,Y1,Z1.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        panic: Vec<i32>,
        /// Where posts may go (the boat's cell): a box X0,Y0,Z0,X1,Y1,Z1.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        area: Vec<i32>,
        #[arg(long, default_value_t = 0.3)]
        margin: f64,
        #[arg(long, default_value_t = 4)]
        max_posts: usize,
        /// Keep every post (glass, boat, zombie, leaves) inside the design's box.
        #[arg(long)]
        in_box: bool,
    },
    /// Runs a loop scene (`queue-loop`) for `--cycles` slots with ideal controls and checks the
    /// loop order.
    QueueLoopRun {
        design: PathBuf,
        #[arg(long, default_value_t = 14)]
        cycles: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Run this many seeds from `--seed` on and print one line each plus the total.
        #[arg(long, default_value_t = 1)]
        seeds: i64,
        /// The release group's slopes powered this long from each one's own opening (day-1's
        /// release controller: each slope 40 gt, front first at +4/+8/+12).
        #[arg(long, default_value_t = 40)]
        release_for: i64,
        /// The queue's 3 advance pulses: the first this many gt after the slot's start, then
        /// every `--advance-every`, each `--pulse` long. Default 32, 24, 10: day-1's wired C24
        /// core in the farm's summon frame (A1 +32 = S+39, A2 S+63, A3 S+87; the c24u-pad
        /// trace-diff, Nov 11 13:05, where 31 differs). Main's old 20-gt controller was 41, 20,
        /// 10 with `--skew 2,2,2`.
        #[arg(long, default_value_t = 32)]
        advance_at: i64,
        #[arg(long, default_value_t = 24)]
        advance_every: i64,
        #[arg(long, default_value_t = 10)]
        pulse: i64,
        /// The stand's slots powered for the departure front first, this many gt apart.
        #[arg(long, default_value_t = 12)]
        depart_every: i64,
        /// Brake the stand at this gt after its slot's start (a timer) instead of the bench's
        /// position trigger.
        #[arg(long)]
        stand_brake: Option<i64>,
        /// Brake the stand this many gt after the release train's front cart first stands on S2's
        /// slope up (what a detector rail there gives) instead of the bench's position trigger.
        #[arg(long)]
        stand_detect: Option<i64>,
        /// Brake each stand slot this many gt after its own cart first stands on the cell two
        /// before its slope (one detector rail per slot).
        #[arg(long)]
        stand_detect_each: Option<i64>,
        /// Brake the queue's tail slots 1-2, then slot 0, at these gt after the returning train's
        /// departure (timers) instead of the bench's position triggers.
        #[arg(long, value_delimiter = ',')]
        intake_brake: Vec<i64>,
        /// The line loop's startup: when (gt into slot 0) the bench places the batch that is on
        /// the return at the start into the queue's tail (default 110 for the line loop).
        #[arg(long)]
        inject_at: Option<i64>,
        /// Plain loop: the stand's 3 departure pulses start this many gt after the slot's start
        /// and this many gt apart; the stand opens this long after the last one ends.
        #[arg(long, default_value_t = 4)]
        depart_at: i64,
        #[arg(long, default_value_t = 16)]
        depart_period: i64,
        #[arg(long, default_value_t = 8)]
        open_after: i64,
        /// The design's own redstone drives the controls: the bench touches none of them and
        /// only powers this input cell (x,y,z: a redstone block there) for `--golem-for` gt at
        /// each slot's start, the golem pulse; the checks stay.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        wired: Vec<i32>,
        #[arg(long, default_value_t = 10)]
        golem_for: i64,
        /// With `--wired`: each slot's golem pulse up to this many gt late (0..=J, from the seed
        /// and the slot), so the gaps between pulses vary over 100 - J ..= 100 + J.
        #[arg(long, default_value_t = 0)]
        golem_jitter: i64,
        /// With `--wired`: the next golem up to this many gt late (0..=J more gt at each slot's
        /// end, from the seed and the slot), so the gaps between pulses vary over 100 ..= 100 + J.
        #[arg(long, default_value_t = 0)]
        golem_late: i64,
        /// Hold the day time in the day: no night, so a file's night window (its siding visits)
        /// never opens: the loop and its stand alone.
        #[arg(long)]
        day_only: bool,
        /// Every rider's LAST_SLEPT set at the start and each night from day time 13,000
        /// (bench-driven: the station bench's stand-in for the siding's naps; a design with a pad
        /// and no siding summons only with it).
        #[arg(long)]
        slept: bool,
        /// The slots' phase: the bench starts its slots so that slot +K falls on the villagers'
        /// summon tick (gameTime % 100 == 0). Default 93, the farm frame: the controller's slot
        /// starts on input.golem, which day-1's wired c24pad measured at summon +7 (the pad's pulse
        /// +1, its 243-cell line +6; ironspec Nov 11 12:12). 99 is a controller on the pad's pulse
        /// itself. Every run prints its frame.
        #[arg(long, allow_hyphen_values = true)]
        summon_at: Option<i64>,
        /// No summon alignment (the slots start wherever the settle leaves them): every margin
        /// against a summon is then in a bench frame, and the run says "summon frame unaligned".
        #[arg(long)]
        summon_unaligned: bool,
        /// Skip the cross-track push count (`crate::crosspush`, ironspec Nov 11 12:40: by default
        /// an acting push between carts on different tracks fails the run); the run says so.
        #[arg(long)]
        no_cross_push_check: bool,
        /// Start every cart at yaw 0 (facing +x), as a player-placed or summoned cart starts
        /// whatever its rail (tick-exact), instead of along its rail as a cart that arrived there
        /// faces (ironspec Nov 11 12:48 and 12:50: the default; this for the assembly's start-up
        /// row, once per assembly).
        #[arg(long)]
        placed_yaw: bool,
        /// Fault injection: SLOT,DT,X,Y,Z,BLOCK[,FOR]: the block set at slot SLOT + DT gt; with
        /// FOR, the block it replaced put back FOR gt later (meanwhile the bench's own drive of
        /// that cell, the wired stand's REL, leaves it). SLOT,DT,unload,N[,ORDER] or
        /// SLOT,DT,unload,X0,Z0,X1,Z1,N: the whole part (or a chunk box) away N gt. SLOT,DT,dismount,R
        /// or SLOT,DT,swap,R1,R2: a rider off its cart, or two riders exchanging carts (R:
        /// visit:SLOT, that visit's napper by its current number; visit:SLOT/J, the rider of that
        /// visit's batch's J-th cart; or an entity number). SLOT,DT,place,K,X,Y,Z[,VZ] or
        /// SLOT,DT,place,K,X,Y,Z,VX,VZ: cart K (or `head`, the cart nearest the queue's head slot;
        /// `s1`..`s3`, nearest that pocket's rest cell) and its rider set at (X,Y,Z) in the loop's
        /// frame, at rest or moving (VX,VZ) b/t; DT `rN`: N gt after the stand's REL rises in that
        /// slot. Repeat for more (up to 8).
        #[arg(long, allow_hyphen_values = true)]
        act: Vec<String>,
        /// A diagnostic for the pocket stand: at this slot's start, a hand reset of the queue only
        /// (every cart outside the stand's region put at rest in the queue's slots, in loop order,
        /// the release group first; the stand's region (its strip and the release run), its carts
        /// and every block left as they are). A clean run after it means the stand was back by
        /// itself at that slot.
        #[arg(long)]
        reset_queue_at: Option<i64>,
        /// With --reset-queue-at: the stand's stragglers (a cart in its region not at rest in a
        /// pocket) moved to the queue too.
        #[arg(long)]
        reset_stragglers: bool,
        /// With --reset-queue-at: every cart placed by hand, the stand's three pockets first (the
        /// design's rest positions), then the queue; no block touched (the stand's redstone kept).
        #[arg(long)]
        reset_stand_carts: bool,
        /// With --reset-queue-at: the queue reset again at every later slot's start, an emulated
        /// tail that always re-packs (the stand's own recovery, the bench's tail out of it).
        #[arg(long)]
        reset_queue_every: bool,
        /// With `--wired`: the stand latch's reset input (x,y,z), pulsed by the bench (a
        /// redstone block for 2 gt) at its own stand-brake trigger, until that brake is wired.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        stand_reset: Vec<i32>,
        /// The stand's batch leaves like a queue advance: 3 pulses on all 3 stand slopes at
        /// `--depart-at` + k·`--depart-period`, `--pulse` long, then the stand opens.
        #[arg(long)]
        shift_depart: bool,
        /// S1's departure slope on at this gt (S2, S3 at `--depart-every` and twice that).
        #[arg(long, default_value_t = 0)]
        s1_at: i64,
        /// The tail's slot 0 opens this many gt after slots 1-2.
        #[arg(long, default_value_t = 0)]
        tail0_late: i64,
        /// With `--wired`: the bench still drives the stand's 3 controls (its departure times
        /// and the brake), the design's redstone the queue's 18.
        #[arg(long)]
        bench_stand: bool,
        /// With `--wired`: the bench still drives the release group's 3 controls (the last 3
        /// queue slots'): a design without its release controller, for the bench-against-wired
        /// check (`trace-diff` on MCTAS_RELEASE_TRACE files).
        #[arg(long)]
        bench_release: bool,
        /// Each release slope's off time, q15,q16,q17 (gt into the slot), overriding the computed
        /// windows. Default 42,48,44: day-1's C24 core in the farm's summon frame (q15 closes
        /// with A1's pulse at +42; the c24u-pad trace-diff, Nov 11 13:05). `--release-computed`
        /// drops it for the windows `--release-for` / `--stagger-off` compute.
        #[arg(long, value_delimiter = ',', default_values_t = [42, 48, 44])]
        release_offs: Vec<i64>,
        /// The release slopes' off times computed from `--release-for` (and `--stagger-off`)
        /// instead of `--release-offs` (main's old controllers).
        #[arg(long)]
        release_computed: bool,
        /// With `--bench-release`: the first slot whose release group the bench drives (1 for a
        /// design whose pad gives its first pulse in slot 1: day-1's wired reference).
        #[arg(long, default_value_t = 0)]
        release_from: i64,
        /// Fail the run on the walk-target static check's flee verdict (`mobfarm flee-check`,
        /// printed by default; ironspec Nov 11 09:29: a void bench's verdict neither passes nor
        /// fails a design, so it gates only when asked until runs are grounded).
        #[arg(long)]
        flee_gate: bool,
        /// Skip the walk-target static check altogether.
        #[arg(long)]
        no_flee_check: bool,
        /// Skip the rider-eye check (by default a suffocating block at any seated rider's eye
        /// along the track fails the run; the blind places are printed).
        #[arg(long)]
        no_eye_check: bool,
        /// Skip the rail-junction check (every three-arm rail settled as the game would for its
        /// signal at rest; a stored shape it would not keep fails the run); the run says so.
        #[arg(long)]
        no_junction_check: bool,
        /// The release group's first slope (the head) on this many gt into the slot. Default 4:
        /// day-1's wired C24 core in the farm's summon frame (q17's rail powered at slot +4 =
        /// S+11), found by the per-tick trace-diff on day-1's c24u-pad (Nov 11 13:05, IDENTICAL
        /// with --release-stagger 4 --release-offs 42,48,44 --advance-at 32 --advance-every 24
        /// --pulse 10; 3 and 5 each differ). Main's old 20-gt controller was 3 (with
        /// --advance-at 41 --advance-every 20 --skew 2,2,2 --summon-unaligned).
        #[arg(long, default_value_t = 4)]
        release_start: i64,
        /// Added to the stand's departure times (with `--wired`, day-1's release starts 4 gt
        /// after the golem pulse: pass 4).
        #[arg(long, default_value_t = 0)]
        stand_offset: i64,
        /// The release group's 3 slopes on front first, this many gt apart (0: together). Default
        /// 4, day-1's C24 core (q17 +4, q16 +8, q15 +12; the c24u-pad trace-diff).
        #[arg(long, default_value_t = 4)]
        release_stagger: i64,
        /// When (gt into a slot) to check that every earlier returning batch rests in the
        /// queue's tail (default: the first advance pulse; wired with day-1's tail: 49).
        #[arg(long)]
        tail_check_at: Option<i64>,
        /// S1, S2, S3 braked these many gt after the detector rail's rise (one brake line each).
        #[arg(long, value_delimiter = ',')]
        stand_detect_slots: Vec<i64>,
        /// Tolerance lines: shift these controls one gt at a time, early and late, to the first
        /// failure (`--seeds` x `--cycles` each): release-start, release-end, adv1-time ..
        /// adv3-time, adv1-width .. adv3-width, s1, s2, s3, stand-brake, tail12, tail0, speed
        /// (the release group's entry speed kick), or `all`; the pocket stand's pocket-release,
        /// pocket-reset, gate-close; the wired pocket stand's rel-start (REL shifted whole),
        /// rel-for (its length), rel-on (its start alone, the end kept).
        #[arg(long, value_delimiter = ',')]
        sweep: Vec<String>,
        /// How far a sweep goes each way (gt).
        #[arg(long, default_value_t = 30)]
        sweep_range: i64,
        /// After the sweep, the corner runs: every swept control at its early limit together, then
        /// at its late limit, each with the entry speed kick at both of its extremes.
        #[arg(long)]
        corners: bool,
        /// The escapement's window bands (ironspec Nov 11 13:32): at each of the 27 combinations
        /// of G_P's opening, G0's opening and the third admission at ±2, the G0 and G_P windows'
        /// passing widths (swept to `--sweep-range`), then each band over all corners, its centre
        /// and whether it is at least 5 gt wide; `--jobs` widths at once.
        #[arg(long)]
        esc_bands: bool,
        /// Don't fail the run on blind riders at night (a seated rider's eye inside a collision
        /// shape at a night tick; the brief's sight rule); the blind line still prints.
        #[arg(long)]
        no_blind_check: bool,
        /// Fail the run on a blind rider at any time of day, not only at night (a transit blind
        /// tick recurs every slot).
        #[arg(long)]
        blind_all: bool,
        /// The freeze rows: SLOT,DT0,DT1,CX0,CZ0,CX1,CZ1: that chunk box (chunk coordinates) away
        /// for each `--freeze-for` N from slot SLOT + every dt in DT0..DT1, each run
        /// `--freeze-after` slots past its thaw; a row fails on any problem or blind rider-tick.
        #[arg(long, allow_hyphen_values = true)]
        freeze_rows: Option<String>,
        #[arg(long, value_delimiter = ',', default_values_t = [40, 200, 1500, 5000])]
        freeze_for: Vec<i64>,
        #[arg(long, default_value_t = 8)]
        freeze_after: i64,
        /// A speed kick on the release group's carts at the release, b/t along their slopes.
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        entry_speed: f64,
        /// Each advance pulse's time shift and width delta (gt), for a single run.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        adv_shift: Vec<i64>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        adv_width: Vec<i64>,
        /// Each queue slot's advance pulses this many gt late, slot 0 first (day-1's wiring:
        /// 10,8,8,6,6,6,6,4,2,2,0,...).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        skew: Vec<i64>,
        /// A diagnostic: at this gt of each slot, the batch (if near the stand) is put at rest at
        /// the stand's nominal offsets, a stand that always delivers.
        #[arg(long)]
        ideal_stand_at: Option<i64>,
        /// The at-the-advance check requires the returning train at rest in the tail (as the
        /// wired run's flag), not only braked into it; pair with --tail-check-at.
        #[arg(long)]
        tail_check_rest: bool,
        /// Slot 2's tail brake on its own, at this gt after the train's departure (else braked
        /// with slot 1's, the first --intake-brake value).
        #[arg(long)]
        tail2_at: Option<i64>,
        /// Place the design as written: blocks raw, block entities (containers) merged, no shape
        /// updates and no settle; the loop must still run clean ("works as placed").
        #[arg(long)]
        placed_raw: bool,
        /// Run both placements (the judge's, then the raw paste) and report each: the brief's
        /// "works as placed" check.
        #[arg(long)]
        placed_both: bool,
        /// Build the design in order instead (the brief's "built in order"): the world empty, each
        /// cell placed with the game's updates in the default build order (supports, containers
        /// filled as placed, rails, redstone from its sources outward), each cell's priced gap
        /// between placements (prices.tsv: place 1 s = 20 gt; 40 gt at a cell the bill charges
        /// place:edge), or GAP gt for every cell.
        #[arg(long, num_args = 0..=1, default_missing_value = "0")]
        placed_in_order: Option<i64>,
        /// All three placements, each reported: the judge's, the raw paste, built in order (each
        /// cell's priced gap, or --placed-in-order's GAP).
        #[arg(long)]
        placed_all: bool,
        /// Built in order at every gap from 1 gt up to the longest priced one (40 gt, or
        /// --placed-in-order's GAP), to the first gap that fails.
        #[arg(long)]
        build_gap_sweep: bool,
        /// Paste a part into the loop before the run (farmopt's bed siding): its blocks, block
        /// entities and entities inside --siding-box (minecarts left out), turned --siding-turn
        /// quarter turns clockwise seen from above (+x to +z), moved to --siding-at.
        #[arg(long)]
        siding: Option<PathBuf>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        siding_at: Vec<i32>,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        siding_turn: i32,
        /// The part's box in its own frame, x0,y0,z0,x1,y1,z1 (default: the siding's documented
        /// box, -7,-4,-3,6,4,9).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        siding_box: Vec<i32>,
        /// With --release-stagger: each staggered slot's off delayed like its on (a repeater in
        /// front of its control delays both edges), not all three off at --release-for. The
        /// default now (day-1's controller); --stagger-common-off restores the old common off.
        #[arg(long)]
        stagger_off: bool,
        /// With --release-stagger: all three release slopes off together at --release-for (the
        /// bench's old model; at stagger 5 and more it cuts the last cart off mid-run, which then
        /// leaks into the release run a slot late).
        #[arg(long)]
        stagger_common_off: bool,
        /// Day-1's merge (b), the side drop: C,SLOT,D,V[,PARK]: at slot C, +4 (the queue at rest),
        /// the cart in queue slot PARK (default SLOT) leaves the loop, its hole then moving one
        /// slot per advance; at +D it rolls off a side rail (3 plain rails west of SLOT's slope at
        /// the peaks' height, a stone stop east of it, placed with their updates) onto SLOT's
        /// braked slope at V b/t.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        side_drop: Vec<f64>,
        /// The side drop staged (day-1's design): two rails one block higher, then a braked powered
        /// rail ascending west on the end cell; the cart arrives at V 20 gt before D, stops on it,
        /// and the bench powers it for 10 gt at D.
        #[arg(long)]
        side_drop_staged: bool,
        /// The staged side drop with two braked cells: the upper rail's east cell a braked
        /// powered rail too (powered through the slope at the kick).
        #[arg(long)]
        side_drop_staged2: bool,
        /// Controls shifted as --sweep shifts them, name=gt (release-start, adv2-width, s3,
        /// tail0, ...), comma-separated: a corner rerun without one of its controls, say.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        shift: Vec<String>,
        /// The stop stand (`--stand-stop` scenes): its gate open and its braked slopes powered at
        /// this gt of the slot (after `--stand-offset`), the gate shut at `--stop-close` (the
        /// slopes stay powered for the next batch, braked `--stop-brake-delay` gt after its third
        /// cart is on them); the re-spacer's pockets all opened at `--pocket-release`, reset
        /// `--pocket-reset` gt later.
        #[arg(long, default_value_t = 0)]
        stop_open: i64,
        #[arg(long, default_value_t = 40)]
        stop_close: i64,
        /// The pocket stand's release (its REL at the stand's input, a redstone block from this gt
        /// of the slot). Default 14: day-1's wired C24 core in the farm's summon frame (the stand
        /// departs at slot +14 = S+21; the bench's departure equal to the wired one to 4
        /// decimals, Nov 11 13:10; 13 and 15 one tick early / late). Main's old controller was 13
        /// at its stand's input repeater (51b2aacd's trace, Nov 11 07:32).
        #[arg(long, default_value_t = 14, allow_hyphen_values = true)]
        pocket_release: i64,
        /// The pocket stand's gates reset (the last shut, the other two open) at this gt of the slot.
        #[arg(long, default_value_t = 52)]
        pocket_reset: i64,
        #[arg(long, default_value_t = 0)]
        stop_brake_delay: i64,
        /// The stop stand's braked slope powered at this gt of the slot (after the gate opened).
        #[arg(long, default_value_t = 8)]
        stop_cv: i64,
        /// The pocket stand's counting gates shut this many gt after their count.
        #[arg(long, default_value_t = 0)]
        gate_close_delay: i64,
        /// A nap visit to the bed siding: S,K,REL: during slot S the stand's cart K (0 = S1, the
        /// front, first across the corner) is diverted, T1/T2 set for its drop slot (q-4-K), the
        /// siding released (10 gt) at +REL from slot S's start; checked: it sleeps, rolls off its
        /// drop rail, rests in its own slot, its batch in order before slot S+5's first advance.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        visit: Vec<i64>,
        /// The nightly visit schedule: EVERY,REL[,FROM]: from the first slot whose day time is
        /// FROM (default 12500) or later, a visit every EVERY slots, 21 a night, the j-th taking
        /// the stand's cart j div 7 (EVERY prime to 7: every batch, every place once), the siding
        /// released at +REL of its slot; checked besides: every rider asleep once a night, every
        /// rider's HOME at each dawn and midday as at the start.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        visits_nightly: Vec<i64>,
        /// The siding's release per stand cart, R0,R1,R2 (its drop slot's route: slots 14, 13,
        /// 12), in place of the REL of --visit / --visits-nightly.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        visit_release_k: Vec<i64>,
        /// The release control per stand cart, x,y,z three times (carts 0, 1, 2: a hold on each
        /// drop line), in place of --siding-release-cell.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        visit_release_cells: Vec<i32>,
        /// T1/T2 set the tick before the visit's release (one release on the route past them,
        /// day-1's earlier layouts) instead of as its cart is diverted (a hold on each line).
        #[arg(long)]
        visit_t_at_release: bool,
        /// The nightly schedule's j-th visit takes the stand's cart j mod 3 (day-1's scheduler)
        /// instead of j div 7.
        #[arg(long)]
        visit_k_mod3: bool,
        /// Wired visits, observed: the file drives its own divert, junctions and holds; the bench
        /// writes no control cell, takes a visit from each cart on the divert's branch rail
        /// (--divert-branch, default one above and one north of --divert-cell), and runs the
        /// same checks (asleep once a night, HOME at dawn and midday, back in its own slot, its
        /// batch in order); --visit-release-cells names the holds for the ready times.
        #[arg(long)]
        visits_observe: bool,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        divert_branch: Vec<i32>,
        /// The divert's 10 gt window start in a visit's slot for the stand's cart 0, +10 per
        /// cart (default 16: golem-relative in a wired file, day-1's measured corner crossings at
        /// golem +21+10K).
        #[arg(long)]
        divert_at: Option<i64>,
        /// The visit's controls (glass = off, a redstone block = on): the divert (the departure
        /// corner T's support), T1, T2, and the siding's release (day-1's layout's cells).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "9,-4,0")]
        divert_cell: Vec<i32>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "-8,0,28")]
        t1_cell: Vec<i32>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "-10,0,31")]
        t2_cell: Vec<i32>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "-16,0,10")]
        siding_release_cell: Vec<i32>,
        /// The wired pocket stand (`queue-loop --stand-wired`): its input cell REL a redstone
        /// block from `--pocket-release` for this many gt (then glass).
        #[arg(long, default_value_t = 40)]
        rel_for: i64,
        /// The bench-driven pocket stand (an ideal-input lab): G1 and G2 shut on a trigger
        /// position, X1,D1,X2,D2: gate k D_k gt after the first cart's centre passes X_k (a
        /// detector rail at cell x0 fires at x0 - 0.29; the bench's D is a circuit's delay - 1).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        pocket_trigger: Vec<f64>,
        /// A diagnostic, never a design check: every rider remembers a golem, so none summons one
        /// (how a loop runs once its off-pad golem cells are gone; the run says so).
        #[arg(long)]
        no_summons: bool,
        /// A diagnostic, never a design check: every zombie killed at the start, so the riders
        /// stay calm (the `cant-reach:` line's positive control).
        #[arg(long)]
        calm: bool,
        /// A diagnostic: each exposed rider-tick also checks, on a copy of its navigation, for a
        /// path from its cart to within 1 of its bed (the open bed top's counterfactual).
        #[arg(long)]
        reach_probe: bool,
        /// Every exposed rider-tick written to this file (game time, slot, offset, rider, x y z,
        /// streak, panicked within 20 gt, its PATH): the freeze-exposure map's raw data.
        #[arg(long)]
        exposure_out: Option<PathBuf>,
        /// Every villager's block position each tick written to this file (x y z per line), for
        /// the golem-spots check (`queue-loop-spots --positions`).
        #[arg(long)]
        record_positions: Option<PathBuf>,
        /// Slots whose golem pulse never comes (a missed golem; up to 8 entries, each a slot or a
        /// range A-B): the design's own expiry must release them (a wired file), or the bench's
        /// launch model does (--launch-model).
        #[arg(long, value_delimiter = ',')]
        golem_miss: Vec<String>,
        /// With --wired: no golem pulse from the bench at all, the controls left to the design (a
        /// design whose pad gives the pulse).
        #[arg(long)]
        no_inject: bool,
        /// Every take-off crossing listed (time, cart, gap, stored and visible speed): a
        /// departure's arrivals for a trace-diff.
        #[arg(long)]
        takeoff_list: bool,
        /// Every cell's calm rider-ticks at night (the PANIC line shows the top 6).
        #[arg(long)]
        calm_list: bool,
        /// Start each run at a day time drawn from its seed (0..24,000): the power-on sweep. With
        /// a pad, each run prints the first golem, the first release and the releases before the
        /// first one locked to the golem grid (G0 open 1..20 gt after the pad's pulse).
        #[arg(long)]
        start_random: bool,
        /// The re-packing tail's intake (a `tail_fifo` design): its gate opened D gt after each
        /// advance's start, for W gt.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        fifo_intake: Vec<i64>,
        /// The escapement tail's B: powered K gt after the gate opens, for W gt.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        esc_b: Vec<i64>,
        /// Admission control (`esc_adm` designs): G0 opened DELAY gt after the gate opens, for W gt.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        esc_adm: Vec<i64>,
        /// Admission control: G0's delay for the third advance of each cycle (−1: as the others).
        #[arg(long, default_value_t = -1, allow_hyphen_values = true)]
        adm_k2: i64,
        /// Lever (b′): G0 keyed to arrival evidence, opening once the queue's front has been
        /// pressed at G0 this many gt (armed from G_P's opening plus the G0 delay to the next G_P
        /// opening, once per arm; the timed windows and --adm-k2 unused).
        #[arg(long)]
        adm_evidence: Option<i64>,
        /// Admission control's R booster (`--adm-rboost` designs): R's control on for this many gt
        /// from each G_P opening.
        #[arg(long, default_value_t = 0)]
        adm_rpulse: i64,
        /// The rider-space check's positive control: stones at X,Y,Z;... placed before the check.
        #[arg(long)]
        rider_space_control: Option<String>,
        /// Declared sight stretches added to the design's `sight:` (X0,Y0,Z0..X1,Y1,Z1;...).
        #[arg(long)]
        sight: Option<String>,
        /// Admission control: P always on from the first gate opening.
        #[arg(long)]
        p_always: bool,
        /// Admission faults, SLOT,ADV,KIND[,OFF] (up to 8): KIND skip (G_P and G0 skipped: the
        /// divert's hole), miss-g0, miss-gp, extra-g0 or extra-gp (a pulse OFF gt after G_P opens).
        #[arg(long)]
        adm_fault: Vec<String>,
        /// The entry hold (`--adm-sensors` designs): the stand's release waits while either tail
        /// detector (landing, room for 3) is on.
        #[arg(long)]
        entry_hold: bool,
        /// The entry hold's landing input counts only after this many gt on without a break.
        #[arg(long, default_value_t = 0)]
        hold_landing_rest: i64,
        /// The cart-evidence landing latch's strand threshold (gt set).
        #[arg(long, default_value_t = 40)]
        hold_strand_t: i64,
        /// The entry hold's progress input: REL only if a cart left slot 0 forward since the
        /// last release.
        #[arg(long)]
        hold_progress: bool,
        /// The register's cart interlock, ideal sensors (bench-driven): an advance moves a cart
        /// only into an empty slot, the head releases only into an empty stand, the tail launches
        /// only into an empty slot 0 (the advance spacing is --advance-every).
        #[arg(long)]
        reg_interlock: bool,
        /// A level-33 freeze of the tail's chunk: its controls delayed by the freeze (a pure
        /// delay) instead of snapping to their schedule on the resume tick (the burst, default).
        #[arg(long)]
        l33_delay: bool,
        /// Every tail arrival printed: the landed cart's free roll, the closing speed at its first
        /// impact, the met cart's state and the queue's occupancy.
        #[arg(long)]
        arrival_log: bool,
        /// The tail front metric's positive control: from slot C +DT for N gt the queue's second
        /// cart behind G0 held where it is (C,DT,N).
        #[arg(long, value_delimiter = ',')]
        hold_back: Vec<i64>,
        /// Admission control's skip requests, SLOT,DT (up to 8): each sets the latch; the next
        /// G0 opening after it is skipped.
        #[arg(long)]
        adm_skip_req: Vec<String>,
        /// The queue's first slots' carts at each slot's start (a re-packing tail diagnostic).
        #[arg(long)]
        verbose_tail: bool,
        /// Every cart in the tail's cells each tick over cycles C0,C1 (a re-packing tail diagnostic).
        #[arg(long, value_delimiter = ',')]
        tail_trace: Vec<i64>,
        /// `--seeds` runs on this many threads (one process).
        #[arg(long, default_value_t = 1)]
        jobs: usize,
        /// (ironspec's reload-order rule, farmopt 2026-11-11) The run again under each of these
        /// load orders for every whole-part `--act ...,unload,N`: `all:N` = N orders (save,
        /// reverse, then seed:1 .. seed:N-2); `all` = `all:40` (ironspec 2026-11-11 05:12:
        /// wherever a sensor phase cannot be forced, a reload row runs at least 40 samples with 0
        /// failures; each order is one sample of the game's random reload, tick-exact), or a list
        /// (`save,reverse,seed:3`, ranges `seed:1-30`); on `--jobs` threads: one line each (its
        /// problems and first problem, the visits, night 1, the lap order, and the visits spanning
        /// the unload: each one's rest slot and tick), then the row's verdict (a failure under any
        /// order fails it) with its miss rate and Wilson 95% interval.
        #[arg(long, value_delimiter = ',')]
        reload_orders: Vec<String>,
        /// (tickexact 2026-11-11, ironspec's forced-phase mode) After each whole-part load
        /// (`--act ...,unload,N`), these sensor delays forced on one villager:
        /// `WHO@SENSOR=DELAY[,SENSOR=DELAY]` (WHO `visit:SLOT`, `visit:SLOT/J` or an entity number;
        /// SENSOR an index or name of the villager's sensors, `living` and `hostiles` the two that
        /// time a sleeper's wake); the load's own draws are still made, every other draw as sampled
        #[arg(long)]
        force_phase: Option<String>,
        /// The run again for every delay of these sensors, `WHO@SENSOR[=VALUES][,SENSOR[=VALUES]]`
        /// (VALUES `A..B`, `A` or `A/B/C`; all of 0 up to the scan rate when omitted), forced on WHO
        /// after every whole-part load, on `--jobs` threads; with `--reload-orders`, under each of
        /// those load orders too: per combination its problems and the villager's wake after the
        /// load, then per order the table of outcomes and the failing share
        #[arg(long)]
        phase_sweep: Option<String>,
        #[arg(long)]
        verbose: bool,
        /// Print these carts (design order) every gt.
        #[arg(long, value_delimiter = ',')]
        watch: Vec<usize>,
        /// With --watch: these cells' block states too ("x,y,z;x,y,z", up to 8): a wiring join's
        /// line, say.
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        watch_cells: String,
        /// Every cart entering each of these cells printed (`x,y,z;x,y,z`, up to 8): its time,
        /// ridden or empty, its speed and the gap since the cell's last entry.
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        cross_list: String,
        /// A mob printed every gt over `--watch-mob-slots`: its place, pose, vehicle, activities,
        /// behaviours, memories and target, and the hostiles within 16 blocks of it (distance, line
        /// of sight from its eyes, target). R: visit:SLOT (that visit's napper, by its current
        /// number), visit:SLOT/J or an entity number. Repeat for more (up to 4).
        #[arg(long)]
        watch_mob: Vec<String>,
        /// The slots --watch-mob prints: S0,S1 (inclusive).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        watch_mob_slots: Vec<i64>,
        /// Write the run's drive log (every control change as `act <t> set x y z <state>`, each
        /// slot's cart positions as `expect <t> pos eN x y z`) for mobviewer's replay.
        #[arg(long)]
        drive_out: Option<PathBuf>,
        /// Write the arrival stream (each cart crossing x 8.5 westward on the escapement tail's
        /// return run: `arrival c<C> +<DT> x<X> v<V>`) for tail-bench `--arrivals`.
        #[arg(long)]
        arrivals_out: Option<PathBuf>,
        /// Write every cell a cart enters with its fastest entry, `x y z v` per line (design cells,
        /// blocks per tick): site-check's `--speeds` input.
        #[arg(long)]
        speeds_out: Option<PathBuf>,
        /// Print every powered rail switching on under a cart: the cart's speed and its motion
        /// against the slope (down, rest, up = backward, flat). A rise under a cart moving up its
        /// slope fails the run either way.
        #[arg(long)]
        rise_list: bool,
        /// The approach's rest check instead of a run: a lone ridden cart set on each cell
        /// ("x,y,z;x,y,z", the loop's frame; `approach`: every cell from the queue head's slope up
        /// to pocket 3's rest slope) at each --rest-speeds speed; where it comes to rest.
        #[arg(long, allow_hyphen_values = true)]
        place_rest: Option<String>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "0,-0.1,0.1")]
        rest_speeds: Vec<f64>,
        /// C', the train-synchronous D1 fallback, emulated: X,Y,Z,DELAY,FOR: each rising edge of the
        /// detector rail at X,Y,Z (the loop's frame) puts a redstone block on F's stone (5,-2,2)
        /// DELAY gt later for FOR gt (remove T3 with --act 0,0,5,-3,2,glass).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        c_prime: Vec<i64>,
        /// With --c-prime: REL held on to C''s edge + E gt when it is on at the edge (a
        /// train-synchronous end for the stand's release window).
        #[arg(long)]
        rel_ext: Option<i64>,
        /// The stand's REL held at least L gt from its rise (a pulse extender on REL: its end at
        /// max(the controller's end, rise + L)); no rise, no extension.
        #[arg(long, default_value_t = 0)]
        rel_min: i64,
        /// The pocket stand's fault table instead of a run (ironspec's faults at slot 5, the
        /// partial unloads too when the design is shifted), on --seeds seeds from --seed, --jobs
        /// threads; a value keeps only the cases whose name contains it ("D1|REL").
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        fault_table: Option<String>,
        /// With --fault-table: the perfect tail (the queue reset at every slot from 6).
        #[arg(long)]
        perfect_tail: bool,
        /// With --fault-table: every case at REL nominal, early and late x staggers 2, 3, 4, 6.
        #[arg(long)]
        all_corners: bool,
        /// With --fault-table (a --cprime design): the table for each set of C''s delays
        /// "R1,R2,R3,R4,RIN,RS1;..." (redstone ticks), the design generated again with each.
        #[arg(long, default_value = "")]
        cprime_variants: String,
        /// The shared bed's head block X,Y,Z (loop frame): print the riders' closest approach to
        /// it, over the run and on the approach (farmopt's approach lab).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        bed_distance: Vec<i32>,
        /// Slots C0,C1[,X]: print every cart on the approach's column each tick (position, speed,
        /// the gap to the cart ahead; farmopt's approach lab); X: the column x = X instead (the
        /// departure's, its carts travelling +z).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        approach_trace: Vec<i64>,
        /// farmopt's departure-clear input, emulated: X,Y,Z (loop frame): the stand's REL_eff
        /// rises only while no cart is in that cell (a detector on the departure's dip); prints
        /// the holds and the margin from the cell's last cart to each REL rise.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        rel_clear: Vec<i32>,
        /// With --rel-clear: the queue head's release waits on the same input too (the release
        /// group held off while a cart is in the cell, released when it clears).
        #[arg(long)]
        head_wait: bool,
        /// The "train arrived since launch" latch, emulated: K,X,Y,Z: the queue head launches only
        /// once the K-th cart since its last launch has entered the cell X,Y,Z (loop frame; K 1 =
        /// the first cart's edge, 3 = the last's).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        head_latch: Vec<i32>,
        /// The tolerance corners instead of a run: REL nominal, early and late x staggers 2, 3, 4, 6.
        #[arg(long)]
        corner_table: bool,
    },
    /// The single-lane carousel's queue: a physics scene (carts back to back on one track, a
    /// release of the front ones down braked slopes) for `queue-run`.
    Queue {
        #[arg(long, value_enum, default_value = "flat")]
        scene: crate::queue::Scene,
        #[arg(long, default_value_t = 21)]
        carts: i32,
        /// The queue's spacing at rest along the track (the flat scene).
        #[arg(long, default_value_t = 1.0)]
        spacing: f64,
        #[arg(long, default_value_t = 3)]
        release: i32,
        /// Empty carts.
        #[arg(long)]
        no_riders: bool,
        /// The saw scene: carts at offsets 0.1 .. 0.95 down their slopes.
        #[arg(long)]
        sweep: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Runs a queue scene: the release slopes powered at `--release-at` for `--release-for` gt,
    /// the queue's rails at `--queue-at` for `--queue-for`; prints every cart's x and speed every
    /// `--every` gt, then which carts left and each cart's largest move, speed and bounces.
    QueueRun {
        design: PathBuf,
        #[arg(long, default_value_t = 600)]
        ticks: i64,
        #[arg(long)]
        release_at: Option<i64>,
        #[arg(long, default_value_t = 40)]
        release_for: i64,
        #[arg(long)]
        queue_at: Option<i64>,
        #[arg(long, default_value_t = 40)]
        queue_for: i64,
        #[arg(long, default_value_t = 0)]
        every: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Lockstep cycles: the first at this tick, then every `--cycle-every`, `--cycles` in all
        /// (every rail powered, braked when the cycle's first held cart passes `--trigger-x`).
        #[arg(long)]
        cycle_start: Option<i64>,
        #[arg(long, default_value_t = 100)]
        cycle_every: i64,
        #[arg(long, default_value_t = 1)]
        cycles: i64,
        #[arg(long, default_value_t = 20.0)]
        trigger_x: f64,
        /// Each cycle braked again this many gt after it starts (0: at the trigger).
        #[arg(long, default_value_t = 0)]
        pulse: i64,
    },
    /// A design's plane x = X as text, for laying out wiring in a stacked part: `.` air, `-` and
    /// `/` a rail (flat, sloped), `P`/`p` a powered rail (flat, sloped), `A` activator, `D`
    /// detector, `R` redstone block, `T` target, `g` glass, `s` stone or cobblestone, `B` bed, `h`
    /// trapdoor, `L` leaves, `O` observer, `r` repeater, `t` torch, `*` dust, `#` any other block;
    /// `c` a minecart's cell (its rider's eye cell `e`, the eye 1.62 over the cart's position), `Z`
    /// a boat's cell.
    Slice {
        design: PathBuf,
        /// The plane.
        #[arg(long)]
        x: i32,
        /// The rows shown (default: the design's extent).
        #[arg(long, allow_hyphen_values = true)]
        y_from: Option<i32>,
        #[arg(long, allow_hyphen_values = true)]
        y_to: Option<i32>,
        /// The columns shown (default: the design's extent).
        #[arg(long, allow_hyphen_values = true)]
        z_from: Option<i32>,
        #[arg(long, allow_hyphen_values = true)]
        z_to: Option<i32>,
    },
    /// The carousel's release + hold part (brief farm-iron-carousel.md, "The parts" 2): write its
    /// bench design, 7 lanes of stacked single-cart tracks, one villager a track, a batch a lane
    /// (`mobfarm hold-bench` runs it).
    Hold {
        /// Lane spacing in x.
        #[arg(long, default_value_t = 1)]
        pitch: i32,
        /// Tracks a lane (a batch's villagers).
        #[arg(long, default_value_t = 3)]
        levels: i32,
        /// Height between levels (3: a rider's eyes under the level above's supports).
        #[arg(long, default_value_t = 3)]
        level_gap: i32,
        /// The bottom level's hold slope (z); level k's is run - k.
        #[arg(long, default_value_t = 21)]
        run: i32,
        /// The bottom level's stand slope (z); level k's is stand-z0 + k (1: the stand ends'
        /// torch chain fits in each lane's column, clear of the pad's nearest row at z -1).
        #[arg(long, default_value_t = 1)]
        stand_z0: i32,
        /// A booster every this many flat rails (0: none; 2 keeps the leaving batch past the
        /// follow box at the next slot with the switch up to 45 gt late).
        #[arg(long, default_value_t = 2)]
        boost_every: i32,
        /// Flat rails at each end without a booster (1 or more).
        #[arg(long, default_value_t = 1)]
        margin: i32,
        /// No hold zombies (a calm hold).
        #[arg(long)]
        no_zombies: bool,
        /// The sleep routine's yard past the hold ends (the shared bed reached by each napper in
        /// its own cart through switched rails; the bench sends each villager there once a night).
        #[arg(long)]
        yard: bool,
        /// The pitch-1 stack (lanes at x = b * pitch) instead of paired lanes (x 0,1 | 3,4 | 6,7 |
        /// 9 with wiring columns, every other pair's hold ends one nearer: ironspec's ruling).
        #[arg(long)]
        unpaired: bool,
        /// With `--yard` on paired lanes: each track's hold end driven through its wired feed
        /// from an input under the stack (a torch under level 0's control; dust lines up the
        /// feed columns for levels 1 and 2) instead of its control switched directly.
        #[arg(long)]
        feeds: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// The release + hold bench (brief farm-iron-carousel.md, "The parts" 2): the 7 batches
    /// cycling on the 100-gt slot grid with the counter's lines ideal (each switch 1 + 0..=jitter gt
    /// after a slot), every villager checked every tick: the selected batch at its stand end at
    /// every slot, every other rider held past the golems' follow box and outside the summoner's
    /// 10-box, never two batches at the station, every rider on its cart and in its lane, REST
    /// ticks and CANT_REACH, and every cell any rider took scanned for a golem spawn spot.
    HoldBench {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        from: i64,
        #[arg(long, default_value_t = 100)]
        to: i64,
        #[arg(long, default_value_t = 6)]
        days: u32,
        #[arg(long, default_value_t = 20)]
        jitter: u32,
        #[arg(long)]
        jobs: Option<usize>,
        /// Print each slot and the first calm villagers at night with the zombies they see.
        #[arg(long)]
        verbose: bool,
        /// Write the first seed's tick-level timing view (`mobfarm timeline`'s page and dump:
        /// the stand zone and the holds as regions, each lane's hold-end control as a signal;
        /// with the yard, every junction, merge, the activator, the park slope and the hatch too)
        /// into this directory.
        #[arg(long)]
        timeline: Option<PathBuf>,
        /// Trace this villager (its entity id) to stderr: its position and its cart's every
        /// other tick, its sleep visit's stage every tick.
        #[arg(long)]
        trace: Option<u32>,
    },
    /// The carousel's station + portal pad (brief farm-iron-carousel.md, "The parts" 3): write its
    /// bench design, the pad (portal planes with a trench row on both sides, a tripwire line on
    /// each frame) on the release + hold part's bench (`mobfarm hold`).
    Station {
        /// The trench rows' x range (the portals' interior one in from each end; 16 wide for the
        /// paired lanes at x 0..9: each rider needs 12 pad columns in its +-8 window).
        #[arg(long, default_value_t = -3, allow_hyphen_values = true)]
        x0: i32,
        #[arg(long, default_value_t = 12)]
        x1: i32,
        /// The portal planes' z, comma-separated (3 or more apart).
        #[arg(long, default_value = "-5,-2", allow_hyphen_values = true)]
        planes: String,
        /// The frames' bottom row (y).
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        by: i32,
        /// The portals' height.
        #[arg(long, default_value_t = 3)]
        h: i32,
        /// Trench rows to leave out (z, comma-separated).
        #[arg(long, allow_hyphen_values = true)]
        skip: Option<String>,
        /// The station part alone (the pad and the Nether part, no release + hold): the catalog
        /// member that `mobfarm bill` prices.
        #[arg(long)]
        part: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// The station's static check: each of the 21 stand riders' golem search, cell by cell (the
    /// judge's function), every spawn spot classed by its portal contact; the batch's own parked
    /// carts against each spot; every entity a cell clear of the portals; each batch's success per
    /// slot for 1, 2 and 3 willing callers; the pad's counts.
    StationCheck {
        design: PathBuf,
        /// Print every stray spot.
        #[arg(long)]
        verbose: bool,
        /// Instead: a golem placed on this trench cell (x,y,z: its feet) and the pulse's path
        /// printed each tick.
        #[arg(long, allow_hyphen_values = true)]
        probe: Option<String>,
        /// Instead: the pad and its Nether part on the official server, a golem summoned on each
        /// of these trench cells (x,y,z;x,y,z;...: feet) 60 ticks apart, its world and the
        /// pulse printed each tick for 14 ticks.
        #[arg(long, allow_hyphen_values = true)]
        official: Option<String>,
    },
    /// The station's bench: the seven batches through the slots on the release + hold part's lines,
    /// measuring each slot's willing riders, tries and golem, each golem's stay in the overworld and
    /// contact, the counter's pulse (its delay from the spawn and its length), the zombified
    /// piglins.
    StationBench {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        from: i64,
        #[arg(long, default_value_t = 20)]
        to: i64,
        #[arg(long, default_value_t = 1)]
        days: u32,
        #[arg(long, default_value_t = 20)]
        jitter: u32,
        #[arg(long)]
        jobs: Option<usize>,
        /// Write the first seed's tick-level timing view (`mobfarm timeline`'s page and dump: the
        /// stand cells, the pad and the holds as regions, the counter's pulse as a signal) into
        /// this directory.
        #[arg(long)]
        timeline: Option<PathBuf>,
        /// With an assembly's counter in the loop: each level's delay (gt) from a line's change to
        /// its hold control's change (levels 0,1,2), as a physical wiring would add.
        #[arg(long, default_value = "0,0,0")]
        hold_delay: String,
        /// The same for the stand controls.
        #[arg(long, default_value = "0,0,0")]
        stand_delay: String,
        /// The first seed's counter line changes (the first 2,000 gt) on stderr.
        #[arg(long)]
        verbose: bool,
        /// Write the first seed's drive as data (mobfarm::drive: every action the bench takes on
        /// the world, and the golems, pulses, crossings and Nether deaths it saw) to this file.
        #[arg(long)]
        drive_out: Option<PathBuf>,
    },
    /// Assembly v0 (step a): the station design (release + hold, pad, Nether part) with day-1's
    /// counter v7 under the stand and the pad's pulse wired to its input; station-bench runs it with
    /// the counter in the loop (its lines drive the slope controls).
    Assembly {
        /// The trench rows' x range (the portals' interior one in from each end; 16 wide for the
        /// paired lanes at x 0..9: each rider needs 12 pad columns in its +-8 window).
        #[arg(long, default_value_t = -3, allow_hyphen_values = true)]
        x0: i32,
        #[arg(long, default_value_t = 12)]
        x1: i32,
        /// The counter frame's shift (x,y,z).
        #[arg(long, default_value = "3,-10,15", allow_hyphen_values = true)]
        counter_at: String,
        /// The wired v0 on the paired lanes (the counter in the yard space, every line wired:
        /// stand chains, pulse wire, delivery cells at z 20); --counter-at is then ignored.
        #[arg(long)]
        wired: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Assembly v0's wiring check: the design's redstone alone on redsim (no villagers), a golem
    /// pulse at every slot but a `miss` share, for `slots` slots: every lane's slope rails against
    /// its counter line (stand slopes powered while the line is low, hold slopes while it is high):
    /// the delays per end and level, crosstalk, rails stuck over `tolerance` gt, two lines high.
    AssemblyCheck {
        design: PathBuf,
        #[arg(long, default_value_t = 2000)]
        slots: u32,
        #[arg(long, default_value_t = 0.05)]
        miss: f64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 40)]
        tolerance: i64,
    },
    /// The station on the linear stand (vjeux 05:58: one lane, all 21 in carts back to back,
    /// released 3 by 3): write its bench design (the pad, the Nether part, the stand's 3 carts
    /// back to back on one track, the stand zombies, the other 18 villagers in a far calm hold).
    Linear {
        #[arg(long, default_value_t = -2, allow_hyphen_values = true)]
        x0: i32,
        #[arg(long, default_value_t = 8)]
        x1: i32,
        /// The stand's first cart cell (x,y,z).
        #[arg(long, default_value = "2,0,0", allow_hyphen_values = true)]
        stand: String,
        /// The stand's track along z (default: along x, parallel to the pad).
        #[arg(long)]
        along_z: bool,
        /// The stand carts' spacing along the track (1: back to back) and each next cart's drop.
        #[arg(long, default_value_t = 1)]
        pitch: i32,
        #[arg(long, default_value_t = 0)]
        drop: i32,
        #[arg(long)]
        out: PathBuf,
        /// Write the cells as the generator writes them, not settled (the default writes every
        /// cell as the bench finds it after the settle, the Nether part's too, comparator outputs
        /// as block entities: "settled as placed", mobfarm::placing::settle_design).
        #[arg(long)]
        unsettled: bool,
        /// The pad and its Nether part alone (no stand track, zombies, hold or bed): the part
        /// another design pastes, with its ports (the golem pulse `output.pulse`, the Nether's
        /// anchor, axis and output) and its build steps.
        #[arg(long)]
        alone: bool,
        /// Trench floor cells written as top slabs ("x,y,z;x,y,z"): a golem floor that does not
        /// suffocate a rider whose eye passes through it (the pocket stand's: "-2,2,-1;-1,2,-1").
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        slab_floor: String,
    },
    /// The linear stand's static check: each of the 3 stand riders' golem search (the judge's
    /// function), the held riders' searches, the boxes near the portals, success per slot.
    LinearCheck { design: PathBuf },
    /// The linear stand's bench: the 7 batches through the stand (the release stubbed: each
    /// batch's carts moved to the stand `lead` gt before its slot), every slot, golem, pulse.
    LinearBench {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        from: i64,
        #[arg(long, default_value_t = 20)]
        to: i64,
        #[arg(long, default_value_t = 1)]
        days: u32,
        #[arg(long, default_value_t = 60)]
        lead: i64,
        #[arg(long)]
        jobs: Option<usize>,
        /// Write the first seed's drive as data (mobfarm::drive) to this file.
        #[arg(long)]
        drive_out: Option<PathBuf>,
        /// Place the design as written: blocks raw, block entities merged, no shape updates and no
        /// settle, the Nether part too ("settled as placed"); the bench must run as clean.
        #[arg(long)]
        placed_raw: bool,
        /// Build the design in order instead ("built in order"): the worlds empty, the Nether part
        /// then the overworld's, each cell placed with the game's updates in the build order
        /// (mobfarm::placing::build_steps_of: ironfarm's order, each portal lit after its frame,
        /// fluids poured last), containers filled as placed, each cell's priced gap (20 gt; 40 gt at
        /// a place:edge cell), or GAP gt for every cell; then the entities.
        #[arg(long, num_args = 0..=1, default_missing_value = "0")]
        placed_in_order: Option<i64>,
        /// All three placements, each reported: the judge's, the raw paste, built in order.
        #[arg(long)]
        placed_all: bool,
        /// Built in order at every gap from 1 gt up to 40 (or --placed-in-order's GAP), to the first
        /// gap that fails.
        #[arg(long)]
        build_gap_sweep: bool,
    },
    /// The carousel's bed siding (ironfarm's hand-over, vjeux's single lane): write its bench design
    /// (the siding, the bench's approach and run-out, 21 villagers in carts in a far hold).
    Siding {
        #[arg(long)]
        out: PathBuf,
        /// The circuit in place of the bench-driven controls (mobfarm::siding::wire).
        #[arg(long)]
        wired: bool,
        /// The siding by itself: no launcher, approach, run-out, hold or carts, only its own
        /// blocks and its dorm zombie (for pasting into ironfarm's loop bench).
        #[arg(long)]
        alone: bool,
        /// A braked powered rail just past the exit cell (day-1's side-drop merge wants at most
        /// 0.36 b/t).
        #[arg(long)]
        exit_brake: bool,
        /// Close the bed's open north front: the bed reachable by path only from the cage.
        #[arg(long)]
        cage_only: bool,
        /// Which give-up the wired siding has: `scheduler` (default: day-1's scheduler pulses the
        /// force input (-5,-3,-4) 150-170 gt after each arrival; the siding gates it with a copper
        /// bulb lit from the arrival to the return), `timer` (its own arrival latch and 220 gt
        /// comparator timer) or `none`.
        #[arg(long, default_value = "scheduler")]
        give_up: String,
        /// Write the cells as the judge's placement leaves them (the generator's own states), not
        /// settled (the default writes every cell as the bench finds it after the settle, comparator
        /// outputs as block entities: "settled as placed", mobfarm::placing::settle_design).
        #[arg(long)]
        unsettled: bool,
    },
    /// The bed siding's static check: every rider and napper cell's golem search (any spot is a
    /// stray).
    SidingCheck { design: PathBuf },
    /// The bed siding's bench: visits through the nights (the villager napped longest ago, its cart
    /// teleported to the approach moving +x at a drawn speed), the controls driven ideally, the
    /// release a drawn time after the arrival: re-board rate, ready times, naps, panic while
    /// waiting, exit speeds.
    SidingBench {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        from: i64,
        #[arg(long, default_value_t = 20)]
        to: i64,
        #[arg(long, default_value_t = 2)]
        nights: u32,
        #[arg(long, default_value = "400,500")]
        gap: String,
        #[arg(long, default_value = "0.15,1.1")]
        speed: String,
        #[arg(long, default_value = "560,610")]
        release: String,
        #[arg(long)]
        jobs: Option<usize>,
        /// At most this many visits a seed (the first visit alone: no failure carried into the next).
        #[arg(long)]
        visits: Option<usize>,
        /// The design is wired: touch no control (only the release and the arrivals).
        #[arg(long)]
        wired: bool,
        /// The traces' window, gt after the start: "a,b".
        #[arg(long, default_value = "0,4000")]
        trace_window: String,
        /// Arrivals launched from rest on the bench's launcher (as a drive can replay them).
        #[arg(long)]
        launch: bool,
        /// Write the first seed's run as a drive (the viewer replays it; use with --launch).
        #[arg(long)]
        drive_out: Option<PathBuf>,
        /// The first seed's first visits' ticks on stderr.
        #[arg(long)]
        trace: bool,
        /// Place the design as written: blocks raw, block entities merged, no shape updates and no
        /// settle ("settled as placed"); the bench must run as clean (mobfarm::placing).
        #[arg(long)]
        placed_raw: bool,
        /// Build the design in order instead ("built in order"): the world empty, each cell placed
        /// with the game's updates in the build order (mobfarm::placing::build_steps_of: ironfarm's
        /// order), containers filled as placed, each cell's priced gap between placements (20 gt;
        /// 40 gt at a place:edge cell), or GAP gt for every cell; then the entities.
        #[arg(long, num_args = 0..=1, default_missing_value = "0")]
        placed_in_order: Option<i64>,
        /// All three placements, each reported: the judge's, the raw paste, built in order (each
        /// cell's priced gap, or --placed-in-order's GAP).
        #[arg(long)]
        placed_all: bool,
        /// Built in order at every gap from 1 gt up to 40 (or --placed-in-order's GAP), to the first
        /// gap that fails.
        #[arg(long)]
        build_gap_sweep: bool,
        /// The release input (4,-2,0) held powered the whole run, no release pulses: each napped
        /// cart leaves its slot as soon as it reaches it (day-1's case); ready = on the slot with
        /// its villager.
        #[arg(long)]
        release_held: bool,
        /// Forced failures (the give-up's test): the first N villagers cannot sleep (their HOME
        /// erased just before each of their visits; the bed's ticket stays taken); each of their
        /// visits must still end with the cart on the slot and released, within 500 gt of the
        /// arrival; the others' checks are unchanged.
        #[arg(long, default_value_t = 0)]
        fail_no_home: u32,
        /// Forced failures, the other kind: every visit by day (day time 1000 to 12000), when no
        /// napper sleeps: every cart must still be on the slot and released within 500 gt of its
        /// arrival, every villager still holding HOME at each check.
        #[arg(long)]
        day_visits: bool,
        /// The force input's pulse (day-1's scheduler: its visit register's D3 gating REL+20, a 12
        /// gt pulse on at visit +328..+340, 118-150 gt after the arrival), gt after each arrival,
        /// spread over the range; driven on every visit when the design has the scheduler give-up
        /// (`--no-force`: never).
        #[arg(long, default_value = "118,150")]
        force_at: String,
        #[arg(long)]
        no_force: bool,
        /// Fault injection (ironspec): on this visit (its index in each run, from 0) the napper
        /// cannot sleep and F is not pulsed; the next visits are sent on schedule all the same.
        /// Prints what the dropped visit's cart does and what the next visits do.
        #[arg(long)]
        drop_force: Option<usize>,
        /// Fault injection at a stateful element, KIND:K (K the visit index): arr-miss, arr-extra,
        /// r-miss, r-extra, f-double (siding::BenchOpts::fault). Prints the visits after it and
        /// how many until one is clean again.
        #[arg(long)]
        fault: Option<String>,
        /// A partial unload (tick-exact's act, farmopt and ironspec 2026-11-10), the world shifted
        /// 2 east so the loop's chunk lines fall on the siding as day-1 pastes it (x −3|−2 and
        /// z −1|0 in the siding's frame): LINE,SIDE,LEVEL,STATE,N[,K] (x or z; lo or hi, the side
        /// dropped; 32 or 33, the dropped side's level, at 33 the other side at 32; arrival, nap,
        /// fpending, kick, exit, or none: the forced box only; N gt; K the visit, default 3), or
        /// `matrix[:K]`: every line, side, level and state at N = 3, 40, 240 and 700, one line each.
        #[arg(long)]
        unload: Option<String>,
        /// The bench world's shift DX,DZ for `--unload` (default: 2,0 for the split placement's
        /// x/z lines, 8,3 for the one-chunk placement's w/e lines, 0,0 for the whole part); 0,0
        /// lets a partial case write a drive in the design's own coordinates.
        #[arg(long)]
        unload_shift: Option<String>,
        /// V as day-1's divert gate, emulated (ironspec 2026-11-11): no visit is sent while the
        /// siding's visit latch V is set (the loop's divert holds the cart back); prints the
        /// visits held back.
        #[arg(long)]
        divert_gate: bool,
        /// The gate's deadlock guard, emulated (day-1's scheduler's reset pulse VR): every
        /// scheduled visit slot, sent or held back, pulses the R line (as R does) at its arrival
        /// + this many gt.
        #[arg(long)]
        v_reset: Option<i64>,
    },
    /// The station's storage alone: the Nether part's pit floor and collection (sorter, crafter,
    /// composter, the double chest) in a test world, a golem's drops on a random pit cell each 100
    /// gt at the given success rate, for `hours`: the iron where it ends each hour and when the
    /// collection first falls behind (a drop lying 300 gt).
    StationStorage {
        #[arg(long, default_value_t = 8.0)]
        hours: f64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 0.99)]
        success: f64,
        /// On the official server instead of redsim.
        #[arg(long)]
        official: bool,
    },
    /// The judge's run of a design on many seeds at once (the carousel's robustness: "21 of 100
    /// seeds hold 3 days"): each seed's verdict (its first failure, or PASS) with its golems, iron
    /// and riders at each midday, then how many hold and the first failures counted by kind.
    Seeds {
        design: PathBuf,
        /// The first and last seed (inclusive).
        #[arg(long, default_value_t = 1)]
        from: i64,
        #[arg(long, default_value_t = 100)]
        to: i64,
        #[arg(long, default_value_t = 3)]
        days: u32,
        /// Runs at once (default: the machine's cores).
        #[arg(long)]
        jobs: Option<usize>,
        /// Also record the count in `<stem>.seeds.tsv` beside the design (page copy's card
        /// fields, `{holds_days}`: header `days held of seeds`, one row per run in run order; a
        /// run of the same days and seeds replaces its row).
        #[arg(long)]
        write: bool,
    },
    /// lane owning its first difference); `--record` writes it to the ratchet's table
    /// (designs/circuits/mobfarm/exact.tsv: a design recorded exact fails a later verify that
    /// differs).
    Exact {
        designs: Vec<PathBuf>,
        #[arg(long)]
        record: bool,
        /// Instead: the table's rows whose design changed since it was recorded (its file's content
        /// hash is not the row's: a trimmed or edited design the ratchet no longer holds), to
        /// record again.
        #[arg(long)]
        stale: bool,
    },
    /// The brief's game check: the design run in the real server for `--days`, its golems (time
    /// and cell) and the iron in the chest, against redsim's runs of the same design.
    Validate {
        design: PathBuf,
        #[arg(long, default_value_t = 3)]
        days: u32,
        /// Kill each golem at once (the spawn-spot statistics).
        #[arg(long)]
        kill: bool,
        /// Redsim seeds to compare with.
        #[arg(long, default_value_t = 32)]
        seeds: u32,
        /// Print each villager's brain memories every 1000 gt.
        #[arg(long)]
        trace: bool,
        /// Game runs (each its own sandbox seed; the game's entity randoms are unseeded).
        #[arg(long, default_value_t = 1)]
        repeat: u32,
    },
    /// The exact compare (track tickexact): the design run in the game (the bridge, every random
    /// seeded from the run's seed) and in redsim side by side, every tick compared (blocks, every
    /// entity, the level random, randValue, each entity's random); prints the first difference
    /// with its context and the fields redsim does not model yet. `--twin`: the bridge against
    /// itself (the determinism check). `--probe`: on a difference, run again and print the game's
    /// draws of that tick by call site.
    Compare {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Game days after the settle and the entities' arrival.
        #[arg(long, default_value_t = 3.0)]
        days: f64,
        /// Game ticks (instead of --days).
        #[arg(long)]
        ticks: Option<u32>,
        /// Weather on (rain and thunder cycle).
        #[arg(long)]
        weather: bool,
        #[arg(long)]
        probe: bool,
        #[arg(long)]
        twin: bool,
        /// Run to the end after the first difference.
        #[arg(long)]
        full: bool,
        /// The bare scenario: no AFK player, the void's biome, spawn_mobs off (the mob layer
        /// without natural spawning, fire spread or thunder). Default: the real-game gate's.
        #[arg(long)]
        bare: bool,
        /// Print the game's draw log of this game time (every stream: "level", "e<n>" = entity
        /// n's own random; each call site with its count, in draw order).
        #[arg(long)]
        draws_at: Option<i64>,
        /// Build it into a terrain sample (village-edge, plains, forest) at its site, natural
        /// spawning on Hard around its AFK spot.
        #[arg(long)]
        terrain: Option<String>,
        /// The day time the farm starts at (default: the steady state's).
        #[arg(long)]
        day0: Option<i64>,
        /// Scan seeds FROM..=TO on the game's side alone for entities of this type entering (a
        /// siege's zombie, a patrol's pillager): the fixtures' seeds.
        #[arg(long, num_args = 3, value_names = ["TYPE", "FROM", "TO"])]
        scan: Option<Vec<String>>,
        /// Ticks per traced step.
        #[arg(long, default_value_t = 100)]
        batch: u32,
        /// The judge's build K (the card's: rotation K / 2 at the (K % 2)-th position in the void,
        /// the K-th site in a terrain sample), with `--run`: its seed, walking and weather.
        #[arg(long)]
        build: Option<u32>,
        /// With `--build`: the judge's run I of that build (seed JUDGE_SEED0 + 1000 K + I).
        #[arg(long, default_value_t = 0)]
        run: u32,
        /// No mob spawning on either side (natural and the custom spawners; the chunk shuffle
        /// still drawn): the random ticks, weather and fire alone.
        #[arg(long)]
        no_spawning: bool,
        /// Only these kinds of difference count, comma separated: `world` (times, the AFK player,
        /// the level random, randValue, weather), `blocks` (blocks, containers), `entities`, or
        /// a category's own name ("level random", "villager pos").
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
    },
}

fn load(_version: &str, path: &PathBuf) -> Result<(circuit::Circuit, redsim::entity::mob::farm::FarmSpec)> {
    let c = circuit::Circuit::load(path)?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow::anyhow!(e))?;
    Ok((c, spec))
}

/// A built run's cells unlike the file, as a problem line (None: the build ended as written).
fn built_problem(diff: &[String]) -> Option<String> {
    (!diff.is_empty()).then(|| format!("built in order: {} cells unlike the file after the build, e.g. {}", diff.len(), diff.iter().take(4).cloned().collect::<Vec<_>>().join("; ")))
}

/// A siding bench run's first problem (None: clean, the bench's criteria): its error, a build that
/// left cells unlike the file, a world error, no visit, a visit with a problem or not through every
/// stage (dropped, napped, woken, back in its own cart, ready on the slot, released).
fn siding_problem(r: &std::result::Result<crate::siding::BenchReport, String>) -> Option<String> {
    let r = match r {
        Err(e) => return Some(format!("ERROR {e}")),
        Ok(r) => r,
    };
    if let Some(p) = built_problem(&r.built_diff) {
        return Some(p);
    }
    if !r.errors.is_empty() {
        return Some(format!("errors: {}", r.errors.join("; ")));
    }
    // ironspec's hard line (2026-11-11): no merged carts (ironfarm's shared check)
    if !r.merged.is_empty() {
        return Some(format!("{} merged cart pairs (the game's push never separates them): {}", r.merged.len(), r.merged_first.join("; ")));
    }
    // ironspec's check: all 21 hold HOME = the bed at every dawn and midday (forced failures: the
    // others)
    let ok_n = 21 - r.fail_no_home;
    if let Some((t, day, n)) = r.home_checks.iter().find(|c| c.2 < ok_n) {
        return Some(format!("{n} of 21 hold HOME = the bed at time {t} (day time {day}); HOME changes: {}", r.home_events.iter().take(4).cloned().collect::<Vec<_>>().join("; ")));
    }
    // and every villager naps once each night it is scheduled
    let mut per_night: std::collections::BTreeMap<i64, std::collections::BTreeSet<u32>> = Default::default();
    for v in r.visits.iter().filter(|v| v.slept.is_some()) {
        per_night.entry(v.night).or_default().insert(v.villager);
    }
    if let Some((n, s)) = per_night.iter().find(|(_, s)| (s.len() as u32) < ok_n && !r.day_visits) {
        return Some(format!("night {n}: {} of 21 villagers napped", s.len()));
    }
    // (a visit the run's end cut short is not counted)
    let counted: Vec<&crate::siding::VisitRec> = r.visits.iter().filter(|v| v.released.is_some() || v.problem.is_some()).collect();
    if counted.is_empty() {
        return Some("no visit".into());
    }
    for v in counted {
        if let Some(p) = &v.problem {
            return Some(format!("villager {} arrived {}: {p}", v.villager, v.arrived));
        }
        // (a forced failure: the cart still on the slot and released, ready within 500 gt)
        if v.failed {
            if !(v.ready.is_some_and(|t| t - v.arrived <= 500) && v.released.is_some()) {
                return Some(format!("forced failure, villager {} arrived {}: dropped {:?} re-boarded {:?} ready {:?} released {:?}", v.villager, v.arrived, v.dropped, v.reboarded, v.ready, v.released));
            }
            continue;
        }
        if !(v.dropped.is_some() && v.slept.is_some() && v.woken.is_some() && v.reboarded.is_some() && v.own_cart && v.ready.is_some() && v.released.is_some()) {
            return Some(format!("villager {} arrived {}: dropped {:?} slept {:?} woken {:?} re-boarded {:?} (own cart {}) ready {:?} released {:?}", v.villager, v.arrived, v.dropped, v.slept, v.woken, v.reboarded, v.own_cart, v.ready, v.released));
        }
    }
    None
}

/// A linear bench run's first problem (None: clean, the bench's criteria): its error, a build that
/// left cells unlike the file, a world error, a violation (a golem not the stand batch's, one back
/// in the overworld or still there 20 gt after its spawn), a golem touching no portal, a pulse
/// without a golem, a golem not 2 gt in the overworld or not pulsed at +1 gt for 10 gt, more
/// golems alive in the Nether at the end than the last 300 gt's slots make.
/// A design's `declared:` line, parsed (none: nothing declared).
fn mctas_decl(v: Option<&String>) -> anyhow::Result<Vec<crate::declared::Decl>> {
    crate::declared::parse(v.map(String::as_str).unwrap_or("")).map_err(|e| anyhow::anyhow!(e))
}

fn linear_problem(r: &std::result::Result<crate::linear::BenchReport, String>) -> Option<String> {
    let r = match r {
        Err(e) => return Some(format!("ERROR {e}")),
        Ok(r) => r,
    };
    if let Some(p) = built_problem(&r.built_diff) {
        return Some(p);
    }
    if !r.errors.is_empty() {
        return Some(format!("errors: {}", r.errors.join("; ")));
    }
    if r.violation_count > 0 {
        return Some(format!("{} violation(s): {}", r.violation_count, r.violations.join("; ")));
    }
    if !r.stray.is_empty() {
        return Some(format!("{} golem(s) touching no portal: {}", r.stray.len(), r.stray.iter().take(3).cloned().collect::<Vec<_>>().join("; ")));
    }
    if r.spurious_pulses > 0 || r.returned > 0 || r.slots == 0 {
        return Some(format!("{} pulse(s) without a golem, {} golem(s) back in the overworld, {} slots", r.spurious_pulses, r.returned, r.slots));
    }
    if let Some(d) = r.dwell.iter().find(|d| **d != 2) {
        return Some(format!("a golem {d} gt in the overworld (the pad's 2)"));
    }
    if let Some(p) = r.pulse.iter().find(|p| **p != (Some(1), 10)) {
        return Some(format!("a pulse {p:?} (delay, length; the pad's +1 gt for 10 gt)"));
    }
    if r.nether_alive > 3 {
        return Some(format!("{} golems alive in the Nether at the end", r.nether_alive));
    }
    None
}

/// `mobfarm slice`: the plane x = `x` of a design as text (rows y from the top, columns z).
fn slice_text(d: &mctas::design::Design, x: i32, y_from: Option<i32>, y_to: Option<i32>, z_from: Option<i32>, z_to: Option<i32>) -> String {
    let ys = d.blocks.keys().map(|p| p[1]);
    let zs = d.blocks.keys().map(|p| p[2]);
    let (y0, y1) = (y_from.unwrap_or_else(|| ys.clone().min().unwrap_or(0)), y_to.unwrap_or_else(|| ys.max().unwrap_or(0)));
    let (z0, z1) = (z_from.unwrap_or_else(|| zs.clone().min().unwrap_or(0)), z_to.unwrap_or_else(|| zs.max().unwrap_or(0)));
    let mut marks: std::collections::BTreeMap<(i32, i32), char> = Default::default();
    for e in &d.entities {
        if e.pos[0].floor() as i32 != x {
            continue;
        }
        let (y, z) = (e.pos[1].floor() as i32, e.pos[2].floor() as i32);
        if e.kind.ends_with("minecart") {
            marks.insert((y, z), 'c');
            marks.insert(((e.pos[1] + 1.62).floor() as i32, z), 'e');
        } else if e.kind.ends_with("boat") {
            marks.insert((y, z), 'Z');
        }
    }
    let glyph = |s: &str| -> char {
        let n = s.strip_prefix("minecraft:").unwrap_or(s);
        let base = n.split('[').next().unwrap_or(n);
        let slope = n.contains("shape=ascending");
        match base {
            "rail" => if slope { '/' } else { '-' },
            "powered_rail" => if slope { 'p' } else { 'P' },
            "activator_rail" => 'A',
            "detector_rail" => 'D',
            "redstone_block" => 'R',
            "target" => 'T',
            "glass" => 'g',
            "stone" | "cobblestone" => 's',
            "iron_trapdoor" => 'h',
            "observer" => 'O',
            "repeater" => 'r',
            "redstone_torch" | "redstone_wall_torch" => 't',
            "redstone_wire" => '*',
            _ if base.ends_with("_bed") => 'B',
            _ if base.ends_with("_leaves") => 'L',
            _ => '#',
        }
    };
    let mut out = String::new();
    out.push_str(&format!("x = {x}: rows y {y1}..{y0}, columns z {z0}..{z1}\n"));
    let mut head = String::from("     ");
    for z in z0..=z1 {
        head.push(char::from_digit(z.rem_euclid(10) as u32, 10).unwrap_or('?'));
    }
    out.push_str(&head);
    out.push('\n');
    for y in (y0..=y1).rev() {
        let mut row = format!("{y:>4} ");
        for z in z0..=z1 {
            let c = match d.blocks.get(&[x, y, z]) {
                Some(s) => glyph(s),
                None => *marks.get(&(y, z)).unwrap_or(&'.'),
            };
            row.push(c);
        }
        out.push_str(&row);
        out.push('\n');
    }
    out
}

pub fn run(version: &str, cmd: MobfarmCmd) -> Result<()> {
    match cmd {
        MobfarmCmd::Gen { layout, pits, pad, floor, villagers, grid, modules, pitch, regular, dx, dz, gap, period, hide, kill, access, tier, out } => {
            let (text, how) = match layout.as_str() {
                "l3" => (crate::layouts::l3(pits), "golems burn in lava pits".to_string()),
                "l1" => (crate::layouts::l1(), "water carries golems to a lava blade in the villagers' sight".to_string()),
                "l2" => (crate::layouts::l2(pad, floor, villagers), "golems sink through water out of sight to a lava blade".to_string()),
                "l2d" => (crate::layouts::l2d(villagers, pad), "golems fall out of sight to a lava blade".to_string()),
                "l5" => (crate::layouts::l5(villagers, pad, grid), format!("{} copies of the fastest module", grid * grid)),
                "l4" => (crate::layouts::l4(), "golems fall from fence-gated platforms to a lava blade".to_string()),
                "l6p-dry" => (crate::day1::l6p_dry(pad), "the bed-plate calm over a dry drop".to_string()),
                "l6p-pad" => (crate::day1::l6p_pad(pad), "golems burn in a corner of their pad".to_string()),
                "d4" => (crate::day1::d4_two_scared(pad), "golems burn in a corner of their pit".to_string()),
                "d1u" => (crate::day1::d1u_with_access(pad, kill, access), if kill <= -19 { "golems burn in a pit too deep for the villagers to sense".to_string() } else { "golems burn in a corner of their pit".to_string() }),
                "d1u-side" => (crate::day1::d1u_side_with(pad, 0, access), "water carries golems sideways to a pit beyond the villagers' reach".to_string()),
                "d1u-drop-side" => (crate::day1::d1u_side_with(pad, 13, access), "golems drop below the villagers' reach, then water carries them sideways to a pit".to_string()),
                "m3" => (crate::day1::m3_bedplate(pad), "golems fall out of sight to a lava blade".to_string()),
                "m2" => (crate::day1::m2_observer(pad), "golems fall out of sight to a lava blade".to_string()),
                "m2open" => (crate::day1::m2_open_hide(pad, hide), "golems fall out of sight to a lava blade".to_string()),
                "m2open-rep" => (crate::day1::m2_open_rep(pad, hide), "golems fall out of sight to a lava blade".to_string()),
                "m2open-loop" => (crate::day1::m2_open_loop(pad, hide / 10, hide % 10), "golems fall out of sight to a lava blade".to_string()),
                "m2open-timer" => (crate::day1::m2_open_timer_cage(pad, hide as u32, crate::parts::Stage::Nether, regular)?, "golems fall out of sight to a lava blade".to_string()),
                "row" => (crate::day1::row(modules, pitch, pad, hide as u32, crate::parts::Stage::Nether)?, format!("{modules} modules in a row share one collection")),
                "row-shared" => (crate::day1::row_shared(modules, pitch, pad, hide as u32, crate::parts::Stage::Nether)?, format!("{modules} modules in a row share one kill chamber")),
                "row-channel" => (crate::day1::row_shared_form(modules, pitch, pad, hide as u32, crate::parts::Stage::Nether, crate::day1::KillForm::Channel)?, format!("{modules} modules in a row send their golems to one kill chamber")),
                "tstack" => (crate::day1::stack(modules, pitch, pad, hide as u32, crate::parts::Stage::Nether)?, format!("{modules} modules stacked")),
                "cross" => (crate::day1::cross_channel_cage(pitch, if modules == 2 { 2 } else { 4 }, regular, pad, hide as u32, crate::parts::Stage::Nether)?, "farms around one kill chamber".to_string()),
                "carousel" => (crate::carousel::carousel(pitch)?, "the villager carousel (L1 prototype)".to_string()),
                "carousel-nolatch" => (crate::carousel::carousel_with(pitch, false, false)?, "the villager carousel (L1 prototype), its curtain shut from the evening threshold".to_string()),
                "carousel-free" => (crate::carousel::carousel_with(pitch, true, true)?, "the villager carousel (L1 prototype), its night running free (boosters every 6 rails)".to_string()),
                "stack" => (crate::day1::m2_stack(modules, [dx, pitch, dz], pad), format!("{modules} middle modules")),
                "lp" => (crate::day1::lp(modules / 2, gap, period, pad), format!("{modules} clusters in up/down pairs")),
                "twin" => (crate::day1::twin(pad, villagers - 4), "two clusters share a pad, a drop and a chamber".to_string()),
                "tz" => (crate::day1::tz(pad), "two clusters share one zombie and one calm".to_string()),
                "shared" => (crate::day1::stack_collect(modules, pitch, pad), format!("{modules} modules share one collection")),
                other => bail!("no layout {other:?} (l1, l2, l2d, l3, l4, l5, l6p-dry, l6p-pad, m3, m2, stack)"),
            };
            // (every farm holds its chest, so the download is the whole farm; its `how:` line ends
            // its site title)
            let text = crate::layouts::edit(&text, true, Some(&how))?;
            let text = match tier {
                Some(t) => crate::layouts::set_tier(&text, &t)?,
                None => text,
            };
            std::fs::write(&out, text)?;
            println!("{}", out.display());
        }
        MobfarmCmd::Exit { design, seed, positions, offset } => {
            let (c, spec) = load(version, &design)?;
            let part = !spec.has_bed();
            if part {
                // (a part's riders and posts checked on a homeless build: a check that can't run
                // never passes silently, ironspec Nov 11 16:03)
                for r in &redsim::entity::mob::refuse::static_checks_homeless(&spec) {
                    println!("refused (a part, built homeless): {r}");
                }
            } else {
                anyhow::ensure!(positions.is_empty(), "--positions adds summoners to a part's flood (a file with no bed); this file holds a bed");
                let refused = redsim::entity::mob::refuse::static_checks(&spec);
                for r in &refused {
                    println!("refused: {r}");
                }
            }
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32);
            let t0 = std::time::Instant::now();
            let rep = if part {
                let off = match offset.as_slice() { [x, y, z] => [*x, *y, *z], [] => [0, 0, 0], _ => anyhow::bail!("--offset takes dx,dy,dz") };
                let mut extra: Vec<redsim::pos::BlockPos> = Vec::new();
                let mut take = |t: &str| -> Result<()> {
                    let v: Vec<i32> = t.split([',', ' ']).filter(|s| !s.is_empty()).map(|n| n.parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "a position takes x y z: {t}");
                    extra.push(redsim::pos::BlockPos::new(v[0] + off[0], v[1] + off[1], v[2] + off[2]));
                    Ok(())
                };
                for f in &positions {
                    for line in std::fs::read_to_string(f)?.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                        take(line)?;
                    }
                }
                if let Some(list) = c.clock.design.meta.get("positions") {
                    for t in list.split_whitespace() {
                        take(t)?;
                    }
                }
                println!("a part's flood (no bed in the file): every villager and {} declared position(s) summon; a golem nothing kills, that walks out of the part or that goes through a nether portal is reported, not refused (a part has no kill)", extra.len());
                redsim::entity::mob::exit::golem_exit_part(&spec, &extra, seed, threads)
            } else {
                redsim::entity::mob::exit::golem_exit(&spec, seed, threads)
            }
            .map_err(|e| anyhow::anyhow!(e))?;
            println!("{} spawn spots, {} positions, {} runs, worst life {} gt, {:.1} s", rep.spawn_spots, rep.starts, rep.runs, rep.worst_life, t0.elapsed().as_secs_f64());
            if rep.part {
                println!("part: {} summoners; {} runs ended with the golem alive (it stopped, found no new cell, or reached the limit); {} walked out of the part; {} went through a nether portal", rep.summoners, rep.rested, rep.left, rep.portal);
                for l in rep.left_at.iter().chain(&rep.portal_at) {
                    println!("  {l}");
                }
            }
            if let Some((lo, hi)) = rep.stood {
                println!("golems stood on cells within {} {} {} .. {} {} {}", lo.x, lo.y, lo.z, hi.x, hi.y, hi.z);
            }
            for f in &rep.failures {
                println!("FAIL {f}");
            }
        }
        MobfarmCmd::NetherPart { design, out } => {
            let d = mctas::design::Design::load(&design)?;
            anyhow::ensure!(!d.nether_blocks.is_empty(), "{}: no Nether part (`nether layer Y:` sections)", design.display());
            let n = mctas::design::Design {
                name: format!("{} nether part", d.name),
                meta: d.meta.iter().filter(|(k, _)| k.as_str() == "circuit" || k.as_str() == "origin").map(|(k, v)| (k.clone(), v.clone())).collect(),
                ports: d.nether_ports.clone(),
                blocks: d.nether_blocks.clone(),
                block_entities: d.nether_block_entities.clone(),
                ..Default::default()
            };
            n.save(&out)?;
            println!("{} ({} blocks)", out.display(), n.blocks.len());
        }
        MobfarmCmd::Timeline { design, from_dump, recording, seed, days, still, terrain, build, tag, from, to, out } => {
            if let Some(rec) = recording {
                let text = mctas::recorded::read_text(&rec)?;
                let stem = rec.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "recording".into());
                let tags: Vec<crate::timeline::Tag> = tag.iter().map(|t| crate::timeline::Tag::parse(t)).collect::<Result<_>>()?;
                let tl = crate::timeline::Timeline::from_recording(&stem, &rec.display().to_string(), &text, &tags)?;
                let to = to.unwrap_or(tl.ticks).min(tl.ticks);
                let (html, txt) = crate::timeline::write(&tl, &out, &stem, from.min(to), to)?;
                println!("{}", tl.summary(from.min(to), to));
                println!("{} ({} KB)", html.display(), std::fs::metadata(&html)?.len() / 1024);
                println!("{} ({} KB)", txt.display(), std::fs::metadata(&txt)?.len() / 1024);
                return Ok(());
            }
            if let Some(dump) = from_dump {
                let text = std::fs::read_to_string(&dump)?;
                let stem = dump.file_stem().map(|s| s.to_string_lossy().trim_end_matches(".timeline").to_string()).unwrap_or_else(|| "dump".into());
                let tl = crate::timeline::Timeline::from_dump(&stem, &dump.display().to_string(), &text)?;
                let to = to.unwrap_or(tl.ticks).min(tl.ticks);
                let (html, _) = crate::timeline::write(&tl, &out, &stem, from.min(to), to)?;
                println!("{}", tl.summary(from.min(to), to));
                println!("{} ({} KB)", html.display(), std::fs::metadata(&html)?.len() / 1024);
                return Ok(());
            }
            let design = design.ok_or_else(|| anyhow::anyhow!("timeline: a design, or --from-dump FILE"))?;
            let (c, spec) = load(version, &design)?;
            use redsim::entity::mob::farm::{RunOpts, WalkMode};
            let walk = if still { WalkMode::Fixed(redsim::entity::mob::walk::Walk::Still) } else { WalkMode::Random };
            let mut o = RunOpts::steady(seed, days * 24000, walk);
            // (the run `run` makes: the same site, the same options)
            let (spec, offset) = match (&terrain, build) {
                (_, Some(k)) => {
                    let sample = circuit::mobfarm::terrain_sample(&c).map_err(|e| anyhow::anyhow!(e))?.ok_or_else(|| anyhow::anyhow!("--build: the design is not judged in terrain"))?;
                    let sites = circuit::mobfarm::terrain_sites(&c, &spec, sample).map_err(|e| anyhow::anyhow!(e))?;
                    let site = *sites.get(k).ok_or_else(|| anyhow::anyhow!("--build {k}: the judge has {} builds", sites.len()))?;
                    o = redsim::entity::mob::farm::judge_opts_in(seed, days * 24000, walk, Some(site));
                    (site.spec(&spec), site.offset)
                }
                (Some(name), None) => {
                    let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow::anyhow!(e))?));
                    o.site = Some(site);
                    (site.spec(&spec), site.offset)
                }
                (None, None) => (spec, [0, 0, 0]),
            };
            let text = std::fs::read_to_string(&design)?;
            let mut tags = crate::timeline::tags_in(&text)?;
            for t in &tag {
                tags.push(crate::timeline::Tag::parse(t)?);
            }
            let t0 = std::time::Instant::now();
            let ticks = days * 24000;
            let stem = design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "design".into());
            let tl = crate::timeline::record(&stem, &spec, &o, offset, &tags, ticks)?;
            let to = to.unwrap_or(ticks).min(ticks);
            let (html, txt) = crate::timeline::write(&tl, &out, &stem, from.min(to), to)?;
            println!("{}", tl.summary(from.min(to), to));
            println!("{}", tl.golem_lock_line(from.min(to), to));
            println!("{} ({} KB)", html.display(), std::fs::metadata(&html)?.len() / 1024);
            println!("{} ({} KB)", txt.display(), std::fs::metadata(&txt)?.len() / 1024);
            println!("{ticks} ticks recorded in {:.1} s", t0.elapsed().as_secs_f64());
        }
        MobfarmCmd::Paste { base, part, at, out, keep, clear, no_settle } => {
            let o: Vec<i32> = at.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(o.len() == 3, "--at x,y,z");
            let keep_part = match keep.as_deref() {
                None => None,
                Some("base") => Some(false),
                Some("part") => Some(true),
                Some(k) => anyhow::bail!("--keep {k}: base or part"),
            };
            let mut d = mctas::design::Design::parse(&std::fs::read_to_string(&base)?)?;
            let p = mctas::design::Design::parse(&std::fs::read_to_string(&part)?)?;
            // (`--clear`: the base's boxes emptied first, the cells the part replaces)
            let mut cleared: Vec<String> = Vec::new();
            for bx in clear.as_deref().unwrap_or("").split(';').map(str::trim).filter(|s| !s.is_empty()) {
                let v: Vec<i32> = bx.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 6, "--clear takes X0,Y0,Z0,X1,Y1,Z1 boxes: {bx}");
                let (lo, hi) = ([v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5])], [v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])]);
                let inside = |c: &[i32; 3]| (0..3).all(|i| c[i] >= lo[i] && c[i] <= hi[i]);
                let nb = d.blocks.len();
                d.blocks.retain(|c, _| !inside(c));
                d.block_entities.retain(|c, _| !inside(c));
                let ne = d.entities.len();
                d.entities.retain(|e| !inside(&[e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]));
                cleared.push(format!("{},{},{}..{},{},{}: {} blocks, {} entities", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], nb - d.blocks.len(), ne - d.entities.len()));
            }
            let mv = |c: &[i32; 3]| [c[0] + o[0], c[1] + o[1], c[2] + o[2]];
            let mut clashes = Vec::new();
            let mut conflicts: Vec<String> = Vec::new();
            let side = |part: bool| if part { "the part's" } else { "the base's" };
            for (c, s) in &p.blocks {
                let at = mv(c);
                match d.blocks.get(&at) {
                    Some(t) if t != s => {
                        clashes.push(format!("{at:?}: the base's {t}, the part's {s}"));
                        if let Some(kp) = keep_part {
                            conflicts.push(format!("{},{},{}: the base's {t}, the part's {s}; kept {}", at[0], at[1], at[2], side(kp)));
                            if kp {
                                d.blocks.insert(at, s.clone());
                            }
                        }
                    }
                    _ => {
                        d.blocks.insert(at, s.clone());
                    }
                }
            }
            anyhow::ensure!(clashes.is_empty() || keep_part.is_some(), "{} cells both designs hold: {} (`--keep base|part` keeps one side and lists each)", clashes.len(), clashes.iter().take(12).cloned().collect::<Vec<_>>().join("; "));
            for (c, s) in &p.block_entities {
                d.block_entities.insert(mv(c), s.clone());
            }
            let mut merged_entities = 0;
            for e in &p.entities {
                let mut e = e.clone();
                e.pos = [e.pos[0] + o[0] as f64, e.pos[1] + o[1] as f64, e.pos[2] + o[2] as f64];
                // (an entity line both designs hold, the same kind, position and data: one entity,
                // as two pasted copies of a shared register would otherwise double its carts)
                if d.entities.iter().any(|x| x.kind == e.kind && x.nbt == e.nbt && (0..3).all(|i| (x.pos[i] - e.pos[i]).abs() < 1.0e-6)) {
                    merged_entities += 1;
                    continue;
                }
                d.entities.push(e);
            }
            for (k, c) in &p.ports {
                if let Some(&b) = d.ports.get(k) {
                    if b == mv(c) {
                        continue;
                    }
                    let Some(kp) = keep_part else { anyhow::bail!("the port {k} is in both designs") };
                    conflicts.push(format!("the port {k}: the base's {:?}, the part's {:?}; kept {}", b, mv(c), side(kp)));
                    if !kp {
                        continue;
                    }
                }
                d.ports.insert(k.clone(), mv(c));
            }
            if !p.nether_blocks.is_empty() {
                if !d.nether_blocks.is_empty() {
                    let Some(kp) = keep_part else { anyhow::bail!("both designs have a Nether part") };
                    conflicts.push(format!("both designs have a Nether part ({} and {} blocks); kept {}", d.nether_blocks.len(), p.nether_blocks.len(), side(kp)));
                    if kp {
                        d.nether_blocks = p.nether_blocks.clone();
                        d.nether_block_entities = p.nether_block_entities.clone();
                        d.nether_ports = p.nether_ports.clone();
                    }
                } else {
                    d.nether_blocks = p.nether_blocks.clone();
                    d.nether_block_entities = p.nether_block_entities.clone();
                    d.nether_ports = p.nether_ports.clone();
                }
            }
            // (the build steps: both designs' tokens, each once; the part's cells moved)
            if let Some(b) = p.meta.get("build") {
                let moved: Vec<String> = b.split_whitespace().map(|t| match t.split(',').map(|v| v.parse::<i32>()).collect::<Result<Vec<_>, _>>() {
                    Ok(v) if v.len() == 3 => format!("{},{},{}", v[0] + o[0], v[1] + o[1], v[2] + o[2]),
                    _ => t.to_string(),
                }).collect();
                let mut toks: Vec<String> = d.meta.get("build").map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default();
                for t in moved {
                    if !toks.contains(&t) {
                        toks.push(t);
                    }
                }
                d.meta.insert("build".into(), toks.join(" "));
            }
            if let Some(s) = p.meta.get("station") {
                d.meta.insert("station".into(), s.clone());
            }
            // (the declared states and flights, `crate::declared` / `crate::flight`: both designs'
            // lines, the part's moved by --at; a cell both declare differently is refused, and a
            // loop base's states derived from its `queue:` options are the runner's own, not here)
            let mut decl = mctas_decl(d.meta.get("declared"))?;
            for x in mctas_decl(p.meta.get("declared"))? {
                let moved = crate::declared::Decl { cell: mv(&x.cell), ..x };
                match decl.iter().find(|e| e.cell == moved.cell) {
                    Some(e) if *e != moved => {
                        let what = format!("the cell {:?} is declared by both designs differently: the base's {}, the part's {}", moved.cell, crate::declared::format(std::slice::from_ref(e)), crate::declared::format(std::slice::from_ref(&moved)));
                        let Some(kp) = keep_part else { anyhow::bail!("{what}") };
                        conflicts.push(format!("{what}; kept {}", side(kp)));
                        if kp {
                            decl.retain(|x| x.cell != moved.cell);
                            decl.push(moved);
                        }
                    }
                    Some(_) => {}
                    None => decl.push(moved),
                }
            }
            let mut fl = crate::flight::parse(d.meta.get("flights").map(String::as_str).unwrap_or("")).map_err(|e| anyhow::anyhow!(e))?;
            for f in crate::flight::parse(p.meta.get("flights").map(String::as_str).unwrap_or("")).map_err(|e| anyhow::anyhow!(e))? {
                let moved = crate::flight::Flight { take_off: mv(&f.take_off), landing: f.landing.iter().map(|c| mv(c)).collect(), max_air: f.max_air };
                if !fl.contains(&moved) {
                    fl.push(moved);
                }
            }
            if d.meta.contains_key("declared") || p.meta.contains_key("declared") {
                d.meta.insert("declared".into(), crate::declared::format(&decl));
            }
            if d.meta.contains_key("flights") || p.meta.contains_key("flights") {
                d.meta.insert("flights".into(), crate::flight::format(&fl));
            }
            let head: String = std::fs::read_to_string(&base)?.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
            let conflict_lines: String = cleared.iter().map(|c| format!("# - cleared before {} at {},{},{}: {c}\n", p.name, o[0], o[1], o[2])).chain(conflicts.iter().map(|c| format!("# conflict ({} at {},{},{}): {c}\n", p.name, o[0], o[1], o[2]))).collect();
            std::fs::write(&out, format!("{head}# + {} pasted at {},{},{} ({} blocks{}{}{})\n{conflict_lines}{}", p.name, o[0], o[1], o[2], p.blocks.len(), if p.nether_blocks.is_empty() { String::new() } else { format!(", its Nether part's {}", p.nether_blocks.len()) }, if merged_entities == 0 { String::new() } else { format!("; {merged_entities} entities the base already held") }, if conflicts.is_empty() { String::new() } else { format!("; {} conflicts", conflicts.len()) }, d.to_text()))?;
            println!("{}: {} blocks after {} blocks of {} at {:?}{}{}{}", out.display(), d.blocks.len(), p.blocks.len(), p.name, o, if p.nether_blocks.is_empty() { String::new() } else { format!(", the Nether part ({} blocks)", p.nether_blocks.len()) }, if merged_entities == 0 { String::new() } else { format!("; {merged_entities} entities the base already held") }, if conflicts.is_empty() { String::new() } else { format!("; {} conflicts (kept {})", conflicts.len(), side(keep_part == Some(true))) });
            for c in &cleared {
                println!("  cleared: {c}");
            }
            for c in &conflicts {
                println!("  conflict: {c}");
            }
            // (ironspec Nov 11 15:56: every junction set to the shape its as-placed power settles
            // it to, each listed; `--no-settle` leaves them as stored)
            if !no_settle {
                let (_c, spec) = load(version, &out)?;
                let settled = crate::queue::settled_junctions(&spec).map_err(|e| anyhow::anyhow!(e))?;
                if !settled.is_empty() {
                    let mut lines = String::new();
                    for (c, from, to, sig) in &settled {
                        let line = format!("{},{},{}: stored {}, settled to {} (its signal {})", c[0], c[1], c[2], from.name(), to.name(), if *sig { "on" } else { "off" });
                        if let Some(s) = d.blocks.get_mut(c) {
                            *s = s.replace(&format!("shape={}", from.name()), &format!("shape={}", to.name()));
                        }
                        println!("  settled: {line}");
                        lines.push_str(&format!("# settled junction ({} at {},{},{}): {line}\n", p.name, o[0], o[1], o[2]));
                    }
                    let text = std::fs::read_to_string(&out)?;
                    let head_n = text.lines().take_while(|l| l.starts_with('#')).count();
                    let head: String = text.lines().take(head_n).map(|l| format!("{l}\n")).collect();
                    std::fs::write(&out, format!("{head}{lines}{}", d.to_text()))?;
                }
            }
        }
        MobfarmCmd::QueueLoop { slots, run, depart, boost_every, boost_first, straight, drop, vstand, no_tail_v, folded, row_gap, uturn_boost, line, no_tail_deep, plain, stand_desc, detector, zombie_every, zombie_x, stand_deep, tail_desc, peak_pad, short_depart, stand_stop, stand_wired, stand_guard, shift, tail_fifo, tail_esc, climb_boost, tail_brake, tail_plain, climb_every, ledge_slope, esc_door, esc_flat, landing_boost2, esc_g0, esc_adm, adm_ledge, adm_g1, adm_short, adm_qboost, adm_flatb, adm_rboost, adm_pdet, adm_sensors, adm_room_k, adm_latch_reset, adm_xq, tail_post_x, xq_gap, xq_drop, xq_z, row_a, tail_posts, approach, depart_crest, cprime, approach_boost, part_departure, crest_plain_dip, crest_descent, part_handoff, approach_climb_boost, part_allowed, out } => {
            let tail_deep = !no_tail_deep && !plain && !tail_desc && tail_fifo == 0 && tail_esc == 0;
            let slots = if line && slots == 18 { 15 } else { slots };
            let o = crate::queue::LoopOpts { slots, run, depart, boost_every, boost_first, straight, drop, vstand, tail_v: !no_tail_v && !tail_deep && !plain && !tail_desc && tail_fifo == 0 && tail_esc == 0, bridge: !folded && !straight && !line, row_gap, uturn_boost, line, tail_deep, plain, stand_desc, detector, zombie_every, zombie_x, stand_deep, tail_desc, peak_pad, short_depart, stand_stop, stand_wired: stand_wired && stand_stop, stand_guard: stand_guard && stand_wired && stand_stop, shift: match shift.as_deref() {
                None => [0, 0],
                Some(s) => {
                    let (x, z) = s.split_once(',').ok_or_else(|| anyhow::anyhow!("--shift {s}: give X,Z"))?;
                    [x.trim().parse()?, z.trim().parse()?]
                }
            }, tail_fifo, tail_esc, climb_boost, tail_brake: tail_brake || tail_plain, tail_plain, climb_every, ledge_slope, esc_door, esc_flat, landing_boost2, esc_g0, esc_adm, adm_ledge, adm_g1, adm_short, adm_qboost, adm_flatb, adm_rboost, adm_pdet, adm_sensors, adm_room_k, adm_latch_reset, adm_xq, tail_post_x, xq_gap, xq_drop, xq_z, row_a, tail_posts: {
                let mut a = [None; 4];
                for (i, s) in tail_posts.split(';').filter(|s| !s.trim().is_empty()).enumerate() {
                    anyhow::ensure!(i < 4, "--tail-posts takes up to 4 posts");
                    let n: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(n.len() == 3, "--tail-posts takes X,Y,Z per post");
                    a[i] = Some([n[0], n[1], n[2]]);
                }
                a
            }, approach, depart_crest, cprime: match cprime.as_slice() { [a, b, c, d, e, f] => Some([*a, *b, *c, *d, *e, *f]), [] => None, _ => anyhow::bail!("--cprime takes six delays") }, approach_boost, crest_plain_dip, crest_descent, approach_climb_boost };
            std::fs::write(&out, crate::queue::loop_design(&o)?)?;
            if let Some(pd) = &part_departure {
                let ho = match part_handoff.as_slice() { [y, z] => Some((*y, *z)), [] => None, _ => anyhow::bail!("--part-handoff takes Y,Z") };
                let az = match part_allowed.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--part-allowed takes Z0,Z1") };
                std::fs::write(pd, crate::queue::departure_part(&o, &std::fs::read_to_string(&out)?, ho, az)?)?;
                println!("{}", pd.display());
            }
            println!("{}", out.display());
            // the control cells (each a block under a powered slope: glass = braked, a redstone
            // block = powered, in the bench)
            let lay = crate::queue::loop_layout(&o);
            let fmt = |ps: &[redsim::pos::BlockPos]| ps.iter().map(|p| format!("({},{},{})", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ");
            println!("queue slots 0..{} (tail first): {}", lay.slots.len() - 1, fmt(&lay.slots));
            println!("release group (the last 3): {}", fmt(&lay.slots[lay.slots.len() - 3..]));
            println!("stand S1 S2 S3: {}", fmt(&lay.stand));
            if let Some(r) = lay.stand_rel {
                println!("wired pocket stand: REL input cell ({},{},{}): a redstone block opens the three gates and resets the latch, glass lets it count", r.x, r.y, r.z);
            }
        }
        MobfarmCmd::SiteCheck { design, terrain, positions, rails, rod, no_controls, afk, speeds } => {
            let (c, spec) = load(version, &design)?;
            let mut extra: Vec<redsim::pos::BlockPos> = Vec::new();
            for f in &positions {
                for line in std::fs::read_to_string(f)?.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                    let v: Vec<i32> = line.split([',', ' ']).filter(|s| !s.is_empty()).map(|n| n.parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "{}: a position takes x y z: {line}", f.display());
                    extra.push(redsim::pos::BlockPos::new(v[0], v[1], v[2]));
                }
            }
            let mut rods: Vec<(i32, i32)> = Vec::new();
            for t in &rod {
                let v: Vec<i32> = t.split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 2, "--rod takes X,Z (a design column): {t}");
                rods.push((v[0], v[1]));
            }
            let afk = match afk.as_deref() {
                Some(t) => {
                    let v: Vec<f64> = t.split(',').map(|n| n.trim().parse::<f64>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--afk takes x,y,z (design coordinates): {t}");
                    Some([v[0], v[1], v[2]])
                }
                None => None,
            };
            let speeds: Option<Vec<(redsim::pos::BlockPos, f64)>> = match &speeds {
                Some(f) => {
                    let mut v = Vec::new();
                    for line in std::fs::read_to_string(f)?.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                        let n: Vec<f64> = line.split([',', ' ', '\t']).filter(|s| !s.is_empty()).map(|n| n.parse::<f64>()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(n.len() == 4, "{}: a speed line takes x y z v (design cell, blocks a tick): {line}", f.display());
                        v.push((redsim::pos::BlockPos::new(n[0] as i32, n[1] as i32, n[2] as i32), n[3]));
                    }
                    Some(v)
                }
                None => None,
            };
            let placed = crate::sitecheck::place(&c, &spec, terrain.as_deref(), &extra, rails, &rods, afk, speeds.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
            let (text, ok) = crate::sitecheck::report(&placed, !no_controls);
            print!("{text}");
            if !ok {
                std::process::exit(1);
            }
        }
        MobfarmCmd::CellDiff { a, b, shift } => {
            let (_, sa) = load(version, &a)?;
            let (_, sb) = load(version, &b)?;
            let (dx, dz) = match shift.as_slice() { [x, z] => (*x, *z), [] => (0, 0), _ => anyhow::bail!("--shift takes X,Z") };
            let ma: std::collections::BTreeMap<[i32; 3], String> = sa.blocks.iter().map(|(p, k)| ([p.x, p.y, p.z], k.to_string().replace("minecraft:", ""))).collect();
            let mb: std::collections::BTreeMap<[i32; 3], String> = sb.blocks.iter().map(|(p, k)| ([p.x - dx, p.y, p.z - dz], k.to_string().replace("minecraft:", ""))).collect();
            let kind = |s: &str| s.split('[').next().unwrap_or("").to_string();
            let count = |v: &[String]| { let mut m: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new(); for s in v { *m.entry(kind(s)).or_default() += 1; } m.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ") };
            let added: Vec<String> = mb.iter().filter(|(p, _)| !ma.contains_key(*p)).map(|(p, s)| format!("{},{},{} {s}", p[0], p[1], p[2])).collect();
            let removed: Vec<String> = ma.iter().filter(|(p, _)| !mb.contains_key(*p)).map(|(p, s)| format!("{},{},{} {s}", p[0], p[1], p[2])).collect();
            let changed: Vec<String> = ma.iter().filter_map(|(p, s)| mb.get(p).filter(|t| *t != s).map(|t| format!("{},{},{} {s} -> {t}", p[0], p[1], p[2]))).collect();
            let strip = |v: &[String]| v.iter().map(|s| s.split(' ').nth(1).unwrap_or("").to_string()).collect::<Vec<_>>();
            println!("ADDED ({}): {}", added.len(), count(&strip(&added)));
            for s in &added {
                println!("  {s}");
            }
            println!("REMOVED ({}): {}", removed.len(), count(&strip(&removed)));
            for s in &removed {
                println!("  {s}");
            }
            println!("CHANGED ({})", changed.len());
            for s in &changed {
                println!("  {s}");
            }
        }
        MobfarmCmd::QueueLoopSpots { design, positions, offset, within, rails, with_entities, list, pad, stand } => {
            let (c, spec) = load(version, &design)?;
            let o = crate::queue::loop_opts_of(&c.clock.design.meta).ok();
            let off = match offset.as_slice() { [x, y, z] => [*x, *y, *z], [] => [0, 0, 0], _ => anyhow::bail!("--offset takes dx,dy,dz") };
            let within_box = match within.as_slice() { [a, b, c, d, e, f] => Some(([*a, *b, *c], [*d, *e, *f])), [] => None, _ => anyhow::bail!("--within takes x0,y0,z0,x1,y1,z1") };
            let mut extra: Vec<redsim::pos::BlockPos> = Vec::new();
            let mut take = |t: &str| -> Result<()> {
                let v: Vec<i32> = t.split([',', ' ']).filter(|s| !s.is_empty()).map(|n| n.parse::<i32>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "a position takes x y z: {t}");
                // (`--within`: positions outside the box, in their own frame, are not this part's)
                if let Some((lo, hi)) = within_box {
                    if (0..3).any(|i| v[i] < lo[i] || v[i] > hi[i]) {
                        return Ok(());
                    }
                }
                extra.push(redsim::pos::BlockPos::new(v[0] + off[0], v[1] + off[1], v[2] + off[2]));
                Ok(())
            };
            for f in &positions {
                for line in std::fs::read_to_string(f)?.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                    take(line)?;
                }
            }
            if let Some(list) = c.clock.design.meta.get("positions") {
                for t in list.split_whitespace() {
                    take(t)?;
                }
            }
            anyhow::ensure!(o.is_some() || !extra.is_empty() || rails, "not a loop scene and no positions: give --positions, --rails or a design `positions:` line");
            // (the pad, a box: --pad or the design's `pad:` line; the stand positions: --stand or
            // the design's `stand:` line, plus a loop scene's stand cells and pockets)
            let six = |v: &[i32]| -> Result<Option<(redsim::pos::BlockPos, redsim::pos::BlockPos)>> { match v { [a, b, c, d, e, f] => Ok(Some((redsim::pos::BlockPos::new(*a.min(d), *b.min(e), *c.min(f)), redsim::pos::BlockPos::new(*a.max(d), *b.max(e), *c.max(f))))), [] => Ok(None), _ => anyhow::bail!("a pad takes x0,y0,z0,x1,y1,z1") } };
            let meta_pad: Vec<i32> = c.clock.design.meta.get("pad").map(|s| s.split([',', ' ']).filter(|t| !t.is_empty()).map(|n| n.parse::<i32>()).collect::<Result<Vec<_>, _>>()).transpose()?.unwrap_or_default();
            let pad_box = match six(&pad)? { Some(b) => Some(b), None => six(&meta_pad)? };
            let mut stand_cells: Vec<redsim::pos::BlockPos> = Vec::new();
            for t in stand.split(';').chain(c.clock.design.meta.get("stand").map(|s| s.as_str()).unwrap_or("").split_whitespace()).map(str::trim).filter(|t| !t.is_empty()) {
                let v: Vec<i32> = t.split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "a stand position takes x,y,z: {t}");
                stand_cells.push(redsim::pos::BlockPos::new(v[0], v[1], v[2]));
            }
            // (`--rails`: every rail cell of the design is a position: a rider sits in its cart's
            // cell, on any branch, the give-up path included)
            if rails {
                for (p, b) in &spec.blocks {
                    if matches!(b, redsim::block::Block::Rail { .. } | redsim::block::Block::PlainRail { .. } | redsim::block::Block::DetectorRail { .. }) {
                        extra.push(*p);
                    }
                }
            }
            println!("{}", crate::queue::golem_spots(&spec, o.as_ref(), &extra, with_entities, list, pad_box, &stand_cells).map_err(|e| anyhow::anyhow!(e))?);
        }
        MobfarmCmd::StandLab { spec, speeds, skew, ai, reverse, gaps, start_x, depart_at, hold_at, release_at, meter, close_at, pockets, stand_open_at, stand_close_at, releases, ticks, out, trace } => {
            let text = crate::standlab::design(&spec)?;
            let path = out.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("stand-lab-{}.txt", std::process::id())));
            std::fs::write(&path, &text)?;
            let (_c, fspec) = load(version, &path)?;
            let cells = crate::standlab::parse(&spec)?;
            let nums = |s: &str| -> Result<Vec<f64>> { Ok(s.split(',').map(|p| p.trim().parse::<f64>()).collect::<Result<Vec<_>, _>>()?) };
            let sp = nums(&speeds)?;
            anyhow::ensure!(sp.len() == 3 && sp[2] > 0.0, "--speeds a,b,step");
            let g = nums(&gaps)?;
            anyhow::ensure!(g.len() == 2, "--gaps a,b");
            println!("cells: {}", cells.iter().map(|c| format!("{}{}@({},{})", c.kind, c.slope, c.x, c.y)).collect::<Vec<_>>().join(" "));
            let mut v = sp[0];
            while v <= sp[1] + 1e-9 {
                let o = crate::standlab::LabOpts { speed: v, ai, reverse, skew: { let s = nums(&skew)?; [s.first().copied().unwrap_or(0.0), s.get(1).copied().unwrap_or(0.0)] }, gaps: [g[0], g[1]], start_x, depart_at, hold_at, release_at, meter, close_at, pockets, stand_open_at, stand_close_at, releases: releases.split(',').filter(|s| !s.is_empty()).map(|s| s.trim().parse::<i64>()).collect::<Result<Vec<_>, _>>()?, ticks, trace };
                let r = crate::standlab::bench(&fspec, &cells, &o).map_err(|e| anyhow::anyhow!(e))?;
                let rest: Vec<String> = r.rest.iter().map(|(x, y, s)| format!("{x:.2},{y:.2}{}", if *s > 1e-3 { format!(" v{s:.3}") } else { String::new() })).collect();
                let exits: Vec<String> = r.exits.iter().map(|e| e.map(|(t, s)| format!("{t}@{s:.2}")).unwrap_or("-".into())).collect();
                println!("speed {v:.2}: rest [{}] frozen at {:?} wobble {:.3} ordered {}{}", rest.join(" | "), r.frozen_at, r.wobble, r.ordered, if depart_at.is_some() { format!(" exits [{}]", exits.join(" ")) } else { String::new() });
                v += sp[2];
            }
            if out.is_none() {
                let _ = std::fs::remove_file(&path);
            }
        }
        MobfarmCmd::SlotDrop { design, slot, h, v, dt, occupied, after, seed, seeds, advance_at, advance_every, pulse, trace, boost } => {
            let (c, spec) = load(version, &design)?;
            let o = crate::queue::loop_opts_of(&c.clock.design.meta)?;
            let tm = crate::queue::LoopTiming { advance_at, advance_every, pulse, ..Default::default() };
            let case = crate::slot_drop::DropCase { slot, h, v, cycle: 1, dt, occupied, after, trace, boost };
            match seeds {
                None => println!("{}", crate::slot_drop::run_slot_drop(&spec, &o, &tm, &case, seed).map_err(|e| anyhow::anyhow!(e))?),
                Some(n) => {
                    let out: Vec<(i64, Result<String, String>)> = std::thread::scope(|sc| {
                        let hs: Vec<_> = (1..=n).map(|s| {
                            let (spec, o, tm, case) = (&spec, &o, &tm, &case);
                            (s, sc.spawn(move || crate::slot_drop::run_slot_drop(spec, o, tm, case, s)))
                        }).collect();
                        hs.into_iter().map(|(s, h)| (s, h.join().unwrap_or_else(|_| Err("panicked".into())))).collect()
                    });
                    let mut bad = Vec::new();
                    for (s, r) in &out {
                        match r {
                            Ok(t) => {
                                let verdict = t.lines().last().unwrap_or("");
                                if !verdict.ends_with(": clean") {
                                    bad.push(*s);
                                    let why: Vec<&str> = t.lines().filter(|l| l.starts_with("landed") || l.starts_with("at rest") || (l.starts_with("fell") && !l.starts_with("fell: 0")) || (l.starts_with("doubles") && !l.contains(": 0 ")) || (l.starts_with("out of order") && !l.contains(": 0 ")) || (l.starts_with("merged") && !l.contains("them): 0 ")) || l.starts_with("rider seated: false") || l.contains("seated at the end: false")).collect();
                                    if bad.len() <= 3 {
                                        println!("seed {s}: {}", why.join(" | "));
                                    }
                                }
                            }
                            Err(e) => {
                                bad.push(*s);
                                println!("seed {s}: error {e}");
                            }
                        }
                    }
                    println!("slot drop: {} of {n} seeds clean{}", n as usize - bad.len(), if bad.is_empty() { String::new() } else { format!("; not clean: {bad:?}") });
                }
            }
        }
        MobfarmCmd::TailBench { design, pattern, extra, seed, seeds, advance_at, advance_every, pulse, fifo_intake, esc_b, esc_g0, esc_adm, adm_latch, p_always, adm_g1_w, adm_k2, adm_rpulse, rider_space_control, sight, trace, shift, from_climb, unload, drive_out, scene_out, flights, arrivals, scene_at } => {
            let (c, spec) = load(version, &design)?;
            let o = crate::queue::loop_opts_of(&c.clock.design.meta)?;
            let pat = crate::tail_bench::parse_pattern(&pattern).map_err(|e| anyhow::anyhow!(e))?;
            anyhow::ensure!(unload.len() % 7 == 0, "--unload takes C,DT,X0,Z0,X1,Z1,N");
            let bo = crate::tail_bench::BenchOpts {
                shift: match shift.as_slice() { [x, z] => (*x, *z), [] => (0, 0), _ => anyhow::bail!("--shift takes DX,DZ") },
                from_climb,
                unloads: unload.chunks(7).map(|u| (u[0], u[1], [u[2] as i32, u[3] as i32, u[4] as i32, u[5] as i32], u[6])).collect(),
                drive_out: drive_out.clone(),
                flights: match &flights { Some(s) => Some(crate::flight::parse(s).map_err(|e| anyhow::anyhow!(e))?), None => None },
                hdr: {
                    let mut h = crate::queue::headers_of(&c.clock.design.meta).map_err(|e| anyhow::anyhow!(e))?;
                    if let Some(s) = &rider_space_control {
                        h.rider_space_control = crate::declared::parse_cells(s).map_err(|e| anyhow::anyhow!("--rider-space-control: {e}"))?;
                    }
                    if let Some(s) = &sight {
                        h.sight.extend(crate::declared::parse_boxes(s).map_err(|e| anyhow::anyhow!("--sight: {e}"))?);
                    }
                    h
                },
                arrivals: match &arrivals {
                    Some(f) => std::fs::read_to_string(f)?
                        .lines()
                        .filter_map(|l| l.strip_prefix("arrival c"))
                        .map(|l| {
                            let t: Vec<&str> = l.split_whitespace().collect();
                            let num = |s: &str, p: &str| s.strip_prefix(p).ok_or_else(|| anyhow::anyhow!("--arrivals: {l}")).and_then(|v| v.parse::<f64>().map_err(|e| anyhow::anyhow!("--arrivals: {l}: {e}")));
                            match t.as_slice() {
                                [c, dt, x, v] => Ok((c.parse::<i64>()?, num(dt, "+")? as i64, num(x, "x")?, num(v, "v")?)),
                                _ => anyhow::bail!("--arrivals: {l}"),
                            }
                        })
                        .collect::<Result<Vec<_>>>()?,
                    None => Vec::new(),
                },
                scene_at,
            };
            let tm = crate::queue::LoopTiming {
                advance_at,
                advance_every,
                pulse,
                fifo_intake: match fifo_intake.as_slice() { [d, w] => (*d, *w), [] => (6, 10), _ => anyhow::bail!("--fifo-intake takes D,W") },
                esc_b: match esc_b.as_slice() { [k, w] => (*k, *w), [] => (6, 8), _ => anyhow::bail!("--esc-b takes K,W") },
                esc_g0: match esc_g0.as_slice() { [a, b] => (*a, *b), [] => (2, 0), _ => anyhow::bail!("--esc-g0 takes LEAD,LAG") },
                esc_adm: match esc_adm.as_slice() { [a, b] => (*a, *b), [] => (8, 3), _ => anyhow::bail!("--esc-adm takes DELAY,MAX") },
                adm_latch,
                p_always,
                adm_g1_w,
                adm_k2,
                adm_rpulse,
                ..Default::default()
            };
            // (`scene_out`: the design reduced to what the bench keeps: every entity line but the
            // minecarts outside the tail (x 0, from the entry curve to slot 0), shifted with the
            // design when `--shift` is given)
            if let Some(sp) = &scene_out {
                let lay = crate::queue::loop_layout(&o);
                let z_slot0 = lay.slot_cells[0][2] as f64;
                let text = std::fs::read_to_string(&design)?;
                let mut out = String::new();
                let mut kept = 0;
                for l in text.lines() {
                    if let Some(rest) = l.strip_prefix("entity: minecraft:minecart ") {
                        let v: Vec<f64> = rest.split_whitespace().take(3).filter_map(|t| t.parse().ok()).collect();
                        let in_tail = v.len() == 3 && v[0] > -0.5 && v[0] < 1.5 && v[2] > 20.5 && v[2] < z_slot0;
                        if !in_tail {
                            continue;
                        }
                        kept += 1;
                    }
                    out.push_str(l);
                    out.push('\n');
                }
                anyhow::ensure!(bo.shift == (0, 0), "--scene-out writes the unshifted design; drop --shift");
                if bo.scene_at.is_none() {
                    std::fs::write(sp, out)?;
                    println!("scene: {} ({kept} tail carts kept)", sp.display());
                }
            }
            match seeds {
                None => {
                    let r = crate::tail_bench::run_tail_bench(&spec, &o, &tm, &pat, extra, seed, trace, &bo).map_err(|e| anyhow::anyhow!(e))?;
                    // (`--scene-at C`: the scene holds every entity line of the design but its
                    // minecarts, and the tail's carts as the bench had them at cycle C's start)
                    if let (Some(sp), Some(c0)) = (&scene_out, bo.scene_at) {
                        let text = std::fs::read_to_string(&design)?;
                        let mut out: String = text.lines().filter(|l| !l.starts_with("entity: minecraft:minecart ")).map(|l| format!("{l}\n")).collect();
                        let lines: Vec<&str> = r.lines().filter_map(|l| l.strip_prefix("scene line: ")).collect();
                        for l in &lines {
                            out.push_str(l);
                            out.push('\n');
                        }
                        std::fs::write(sp, out)?;
                        println!("scene: {} ({} tail carts as at cycle {c0}'s start)", sp.display(), lines.len());
                    }
                    print!("{}", r.lines().filter(|l| !l.starts_with("scene line: ")).map(|l| format!("{l}\n")).collect::<String>());
                }
                Some(n) => {
                    // (the seeds in parallel, each its summary: every contract line on one row)
                    let out: Vec<(i64, Result<String, String>)> = std::thread::scope(|sc| {
                        let hs: Vec<_> = (1..=n).map(|s| { let (spec, o, tm, pat, bo) = (&spec, &o, &tm, &pat, &bo); (s, sc.spawn(move || crate::tail_bench::run_tail_bench(spec, o, tm, pat, extra, s, false, bo))) }).collect();
                        hs.into_iter().map(|(s, h)| (s, h.join().unwrap_or_else(|_| Err("panicked".into())))).collect()
                    });
                    let mut bad = Vec::new();
                    for (s, r) in &out {
                        match r {
                            Ok(t) => {
                                let line = |pre: &str| t.lines().find(|l| l.starts_with(pre)).unwrap_or("").to_string();
                                let num = |pre: &str| line(pre).trim_start_matches(pre).split_whitespace().next().unwrap_or("?").to_string();
                                let lost = num("lost intakes (P loaded, slot 0 empty at the next advance): ");
                                let starv = num("starved intakes (carts waiting in the tail, P empty): ");
                                let dbl = num("doubles (two carts in slot 0 at an advance): ");
                                let ooo = num("out of arrival order: ");
                                let fell = num("fell or left the rails: ");
                                let gate = num("through the shut gate: ");
                                let back = num("backward on a powered rail (B, P, slot 0) while it is powered: ");
                                let rolled = num("pushed back down the return (onto the climb): ");
                                let count = line("count: ");
                                let lost_carts = count.split(", ").last().and_then(|t| t.split_whitespace().next()).unwrap_or("?").to_string();
                                let hurt = num("villagers: ");
                                let merged = num("merged carts (horizontal distance under 0.01, boxes paired by the push; the game never separates them): ");
                                // (starved intakes are the pipeline refilling after an empty tail: a
                                // line of their own, not a fault)
                                let clean = [&lost, &dbl, &ooo, &fell, &rolled, &gate, &back, &hurt, &lost_carts, &merged].iter().all(|v| v.as_str() == "0");
                                if !clean {
                                    bad.push(*s);
                                }
                                println!("seed {s}: lost {lost} starved {starv} doubles {dbl} out-of-order {ooo} fell {fell} rolled-back {rolled} gate {gate} backward-powered {back} villagers-hurt {hurt} carts-lost {lost_carts} merged {merged} | {} | {} | {}{}", line("at rest in the tail: "), line("creep while quiet"), line("the tail's most carts at once: "), {
                                    // (admission control's lines, when the design has G0)
                                    let a = line("admission (G0): ");
                                    if a.is_empty() {
                                        String::new()
                                    } else {
                                        let grab = |pre: &str| a.split(pre).nth(1).and_then(|t| t.split(|c: char| c == ' ' || c == ';' || c == '(').next()).unwrap_or("?").to_string();
                                        format!(" | adm-missed {} front-missed {} miss-pressed {} miss-rolling {} miss-far {} front-far {} fol-near {} fol-fast {} adm-doubles {} follow {} gp-run {} p-margin {}", grab("missed "), grab("front missed "), grab("P empty: pressed "), grab(" rolling "), grab(" far "), grab("the front at most "), grab("the follower at least "), grab("short and at most "), grab("doubles at G0's shut "), grab("front face at least "), grab("behind the launched one "), {
                                            let b = line("admission: ");
                                            b.split("at rest on P ").nth(1).and_then(|t| t.split_whitespace().next()).unwrap_or("?").to_string()
                                        })
                                    }
                                });
                            }
                            Err(e) => {
                                bad.push(*s);
                                println!("seed {s}: error {e}");
                            }
                        }
                    }
                    println!("tail bench: {} of {n} seeds clean{}", n as usize - bad.len(), if bad.is_empty() { String::new() } else { format!("; not clean: {bad:?}") });
                }
            }
        }
        MobfarmCmd::FrameCheck { design, cycles, wired } => {
            // (each run a queue-loop-run of this binary, its release trace through the stand)
            let exe = std::env::current_exe()?;
            let dir = std::env::temp_dir().join(format!("frame-check-{}", std::process::id()));
            std::fs::create_dir_all(&dir)?;
            let base: Vec<String> = vec!["mobfarm".into(), "queue-loop-run".into(), design.display().to_string(), "--wired".into(), wired.clone(), "--no-inject".into(), "--slept".into(), "--seed".into(), "1".into(), "--cycles".into(), cycles.to_string(), "--no-flee-check".into(), "--no-eye-check".into()];
            let run = |name: &str, extra: &[&str]| -> anyhow::Result<String> {
                let path = dir.join(format!("{name}.trace"));
                let st = std::process::Command::new(&exe).args(&base).args(extra).env("MCTAS_RELEASE_TRACE", &path).env("MCTAS_RELEASE_TRACE_CELLS", "90").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()?;
                if !st.success() {
                    anyhow::bail!("frame-check: the {name} run failed ({st})");
                }
                Ok(std::fs::read_to_string(&path)?)
            };
            let reference = run("wired", &[])?;
            let bench = ["--bench-release", "--release-from", "1"];
            let mut ok = true;
            let cases: [(&str, &[&str], bool); 5] = [
                ("the defaults", &[], true),
                ("control: --release-start 3", &["--release-start", "3"], false),
                ("control: --release-start 5", &["--release-start", "5"], false),
                ("control: --advance-at 31", &["--advance-at", "31"], false),
                ("control: --release-offs 41,47,43", &["--release-offs", "41,47,43"], false),
            ];
            for (k, (name, extra, want_same)) in cases.iter().enumerate() {
                let args: Vec<&str> = bench.iter().copied().chain(extra.iter().copied()).collect();
                let t = run(&format!("bench{k}"), &args)?;
                let line = crate::queue::trace_diff(&reference, &t, 0.0001);
                let same = line.ends_with("IDENTICAL");
                let pass = same == *want_same;
                ok &= pass;
                println!("{} {name}: {}", if pass { "PASS" } else { "FAIL" }, line);
            }
            let _ = std::fs::remove_dir_all(&dir);
            println!("frame-check {}: queue-loop-run's default bench timing against the wired build {} ({} slots)", if ok { "PASS" } else { "FAIL" }, design.display(), cycles);
            if !ok {
                anyhow::bail!("frame-check failed");
            }
        }
        MobfarmCmd::BrakeBench { speeds, empty, riders, descent, post, flats, gap, ticks, no_brake, window } => {
            if window {
                println!("{}", crate::holdbench::brake_window(&speeds, !empty, descent, post, flats, ticks).map_err(|e| anyhow::anyhow!(e))?);
                return Ok(());
            }
            let rid: Vec<bool> = if riders.is_empty() { vec![!empty] } else { riders.iter().map(|r| *r != 0).collect() };
            let (_, text) = crate::holdbench::hold_bench(&speeds, &rid, descent, post, flats, gap, ticks, !no_brake).map_err(|e| anyhow::anyhow!(e))?;
            println!("{text}");
        }
        MobfarmCmd::PocketBench { pockets, speeds, riders, gaps, descent, ticks, beat, beat_at, corner, lead, pulse, wire, spacer, fault, pocket_cells, front_first, entry_admit, feed_gated } => {
            let faults = fault.iter().map(|f| crate::pocketbench::parse_fault(f)).collect::<Result<Vec<_>, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let o = crate::pocketbench::PocketOpts { pockets, speeds, riders: riders.iter().map(|r| *r != 0).collect(), gaps, descent, ticks, beat, beat_at, corner, lead, pulse, wire, spacer, faults, cells: pocket_cells, together: !front_first, entry_admit, feed_gated };
            let rep = crate::pocketbench::pocket_bench(&o).map_err(|e| anyhow::anyhow!(e))?;
            println!("{}", rep.text);
        }
        MobfarmCmd::GqBench { carts, riders, tail, run, cap, cycles, corner, pulse, wire, spacer, ret, ret_gap, ret_speed, freeze, hold_pulse, hold_ripple, settle, overfill, adm_settle, intake_admit, ideal_tail, no_intake_flat, back_at, alive_gate, freeze32, freeze_matrix, jobs, receiver, ret_offs, recv_settle, recv_feed, recv_timeout, recv_ideal, recv_fly, receiver_p, recv_run } => {
            let pairs = |v: &[String], what: &str| -> anyhow::Result<Vec<(i64, i64)>> {
                v.iter()
                    .map(|s| {
                        let p: Vec<i64> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                        if p.len() == 2 { Ok((p[0], p[1])) } else { Err(anyhow::anyhow!("--{what} T,D")) }
                    })
                    .collect()
            };
            let fr = pairs(&freeze, "freeze")?;
            let fr32 = pairs(&freeze32, "freeze32")?;
            let o = crate::gqbench::GqOpts { carts, riders: riders.iter().map(|r| *r != 0).collect(), tail, run, cap, cycles, corner, pulse, wire, spacer, ret, ret_gap, ret_speed, freeze: fr, hold_pulse, hold_ripple, settle, overfill, adm_settle, intake_admit, ideal_tail, no_intake_flat, back_at, alive: alive_gate, freeze32: fr32, receiver: match receiver.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--receiver takes T_MID,T_REAR") }, ret_offs, recv_settle, recv_feed, recv_timeout, recv_ideal, recv_fly, receiver_p: match receiver_p.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--receiver-p takes D_MID,D_REAR") }, recv_run };
            if !freeze_matrix.is_empty() {
                let parts: Vec<&str> = freeze_matrix.split(';').collect();
                let sv: Vec<i64> = parts.first().unwrap_or(&"").split(',').filter_map(|x| x.trim().parse().ok()).collect();
                let [t0, t1, step] = sv.as_slice() else { anyhow::bail!("--freeze-matrix takes T0,T1,STEP;D1,D2,..[;real]") };
                let ds: Vec<i64> = parts.get(1).unwrap_or(&"").split(',').filter_map(|x| x.trim().parse().ok()).collect();
                let real = parts.get(2).is_some_and(|x| x.trim() == "real");
                let runs: Vec<(i64, i64)> = (*t0..=*t1).step_by((*step).max(1) as usize).flat_map(|t| ds.iter().map(move |d| (t, *d))).collect();
                println!("gq-bench freeze matrix: {} freezes from t {t0}..{t1} every {step} gt for {:?} gt ({} runs; {}{})", if real { "real (the whole bench at level 32, redstone running)" } else { "emulated (every cart's position and motion restored each tick)" }, ds, runs.len(), if alive_gate { "day-1's live-time clock and liveness gate" } else { "ironfarm's instant gate (openings skip while frozen)" }, if corner != 0 { format!(", corner {corner}") } else { String::new() });
                for chunk in runs.chunks(jobs.max(1)) {
                    let got: Vec<((i64, i64), Result<String, String>)> = std::thread::scope(|sc| {
                        let hs: Vec<_> = chunk
                            .iter()
                            .map(|&(ft, d)| {
                                let mut o2 = o.clone();
                                if real {
                                    o2.freeze32 = vec![(ft, d)];
                                } else {
                                    o2.freeze = vec![(ft, d)];
                                }
                                sc.spawn(move || ((ft, d), crate::gqbench::gq_bench(&o2)))
                            })
                            .collect();
                        hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
                    });
                    for ((ft, d), r) in got {
                        let r = r.map_err(|e| anyhow::anyhow!(e))?;
                        let after = |key: &str| r.lines().find_map(|l| l.split(key).nth(1)).map(|x| x.to_string());
                        let num = |s: &str| s.trim().split(|c: char| !c.is_ascii_digit()).next().unwrap_or("").to_string();
                        let per_open = r.lines().find_map(|l| l.strip_prefix("carts per opening (exactly 1): ")).unwrap_or("-").to_string();
                        let shuts = after("the bar: 0): ").map(|x| num(&x)).unwrap_or("-".to_string());
                        let arr = r.lines().find(|l| l.starts_with("arrivals into the pressed run")).map(|l| {
                            let hard = l.split("the hardest ").nth(1).map(|x| x.split(',').next().unwrap_or("").to_string()).unwrap_or_default();
                            let above = l.split(", ").find(|x| x.ends_with("above 0.2") || x.contains(" above 0.2")).map(|x| num(x)).unwrap_or_default();
                            format!("{above} over 0.2 (hardest {hard})")
                        }).unwrap_or("-".to_string());
                        let fill = after("at its emptiest after the first cycle: ").map(|x| x.split(';').next().unwrap_or("").to_string()).unwrap_or("-".to_string());
                        let tail_max = after("the tail chain's most carts: ").map(|x| num(&x)).unwrap_or("-".to_string());
                        let held = r.lines().find_map(|l| l.strip_prefix("the hold at each release (latched carts): ")).map(|x| {
                            let ns: Vec<usize> = x.split(';').next().unwrap_or("").split(", ").filter_map(|y| y.split(": ").nth(1)).filter_map(|y| y.trim().parse().ok()).collect();
                            format!("{} releases, {} not 3 carts", ns.len(), ns.iter().filter(|n| **n != 3).count())
                        }).unwrap_or("-".to_string());
                        let clock = r.lines().find(|l| l.starts_with("live-time cycle clock")).map(|l| {
                            let gol = l.split("): ").nth(1).map(|x| x.split(';').next().unwrap_or("").to_string()).unwrap_or_default();
                            let off = l.split("(out of phase) ").nth(1).map(|x| num(x)).unwrap_or_default();
                            let two = l.split("cycles with two releases ").nth(1).map(|x| num(x)).unwrap_or_default();
                            let zero = l.split("releases per cycle ").nth(1).map(|x| x.split("; releases off").next().unwrap_or("").split(", ").filter(|c| c.split(": ").nth(1).is_some_and(|n| n.starts_with('0'))).count()).unwrap_or(0);
                            format!("{gol}, cycles with no release {zero}, two {two}, off T+3 {off}")
                        }).unwrap_or("-".to_string());
                        let lat = after("detection latency ").map(|x| x.to_string()).unwrap_or("-".to_string());
                        let into = after("must be 0): ").map(|x| num(&x)).unwrap_or("-".to_string());
                        let dropped = after("sequences dropped ").map(|x| num(&x)).unwrap_or("-".to_string());
                        println!("freeze t {ft} for {d}: per opening {per_open} | shuts onto a cart {shuts} | arrivals {arr} | run fill min {fill}, tail max {tail_max} | hold {held} | clock {clock} | latency {lat} | fired into a frozen cart {into} | sequences dropped {dropped}");
                    }
                }
                return Ok(());
            }
            println!("{}", crate::gqbench::gq_bench(&o).map_err(|e| anyhow::anyhow!(e))?);
        }
        MobfarmCmd::TraceDiff { a, b, tol } => {
            println!("{}", crate::queue::trace_diff(&std::fs::read_to_string(&a)?, &std::fs::read_to_string(&b)?, tol));
        }
        MobfarmCmd::JobsiteCheck { designs, range, add } => {
            let mut total = 0usize;
            for d in &designs {
                let c = match circuit::Circuit::load(d) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("{}: not a design ({e})", d.display());
                        continue;
                    }
                };
                let mut blocks: Vec<([i32; 3], String)> = c.clock.design.block_list().into_iter().map(|(q, s)| ([q[0], q[1], q[2]], s)).collect();
                for a in &add {
                    let f: Vec<&str> = a.splitn(4, ',').collect();
                    let [x, y, z, blk] = f.as_slice() else { anyhow::bail!("--add takes X,Y,Z,BLOCK") };
                    blocks.push(([x.parse()?, y.parse()?, z.parse()?], blk.to_string()));
                }
                let (line, n) = crate::queue::jobsite_check_blocks(&blocks, range);
                total += n;
                println!("{}: {line}", d.display());
            }
            println!("job-site check: {total} hits in {} designs", designs.len());
            if total > 0 {
                std::process::exit(1);
            }
        }
        MobfarmCmd::LedgeSweep { design, speeds, gap_from, gap_to, gap_step, riders, ticks } => {
            let (c, spec) = load(version, &design)?;
            let hdr = crate::queue::headers_of(&c.clock.design.meta).map_err(|e| anyhow::anyhow!(e))?;
            let fl = hdr.flights.first().cloned().ok_or_else(|| anyhow::anyhow!("no `flights:` line in the design"))?;
            let ridden: Vec<bool> = match riders.as_str() {
                "both" => vec![false, true],
                "ridden" => vec![true],
                "empty" => vec![false],
                _ => anyhow::bail!("--riders: both, ridden or empty"),
            };
            anyhow::ensure!(gap_step > 0.0, "--gap-step must be positive");
            for l in crate::ledge_sweep::sweep(&spec, &fl, &speeds, gap_from, gap_to, gap_step, &ridden, ticks).map_err(|e| anyhow::anyhow!(e))? {
                println!("{l}");
            }
        }
        MobfarmCmd::PostSearch { design, remove, panic, area, margin, max_posts, in_box } => {
            let (_c, mut spec) = load(version, &design)?;
            anyhow::ensure!(panic.len() == 6 && area.len() == 6, "--panic and --area take X0,Y0,Z0,X1,Y1,Z1");
            let cols: Vec<(i32, i32)> = remove.split(';').filter(|s| !s.trim().is_empty()).map(|s| {
                let v: Vec<i32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                (v.first().copied().unwrap_or(0), v.get(1).copied().unwrap_or(0))
            }).collect();
            // (a post: a boat (with its zombie) whose column is (X, Z), the glass under it and the
            // leaves over it in that column)
            let mut removed = 0;
            spec.entities.retain(|(ty, p, _)| {
                let hit = ty.ends_with("boat") && cols.contains(&(p[0].floor() as i32, p[2].floor() as i32));
                removed += hit as usize;
                !hit
            });
            spec.blocks.retain(|(p, b)| {
                let s = b.to_string();
                !(cols.contains(&(p.x, p.z)) && (s.contains("glass") || s.contains("leaves")) && (1..=7).contains(&p.y))
            });
            let rails: Vec<[i32; 3]> = spec.blocks.iter().filter(|(p, b)| b.to_string().contains("rail") && (panic[0]..=panic[3]).contains(&p.x) && (panic[1]..=panic[4]).contains(&p.y) && (panic[2]..=panic[5]).contains(&p.z)).map(|(p, _)| [p.x, p.y, p.z]).collect();
            let (blo, bhi) = crate::postsearch::design_box(&spec);
            println!("post search: {removed} posts removed, {} panic cells, area {:?}, margin {margin}; the design's box ({},{},{})..({},{},{}){}", rails.len(), area, blo[0], blo[1], blo[2], bhi[0], bhi[1], bhi[2], if in_box { ", posts kept inside it" } else { "" });
            let f = crate::postsearch::search_in(&spec, &rails, [area[0], area[1], area[2], area[3], area[4], area[5]], margin, max_posts, in_box).map_err(|e| anyhow::anyhow!(e))?;
            let grows: Vec<String> = f.posts.iter().filter(|p| p[0] < blo[0] || p[0] > bhi[0] || p[2] < blo[2] || p[2] > bhi[2] || p[1] - 1 < blo[1] || p[1] + 3 > bhi[1]).map(|p| format!("({},{},{})", p[0], p[1], p[2])).collect();
            println!("posts that grow the box: {}", if grows.is_empty() { "none".to_string() } else { grows.join(" ") });
            println!("posts (the boat's cell; glass under, leaves 3 over): {}", f.posts.iter().map(|p| format!("({},{},{})", p[0], p[1], p[2])).collect::<Vec<_>>().join(" "));
            println!("the least distance from any rider point to a post: {:.2}", f.least);
            let bad: Vec<String> = f.cover.iter().filter(|(_, d, seen)| *d > 8.0 - margin || !seen).map(|(c, d, seen)| format!("({},{},{}) {d:.2}{}", c[0], c[1], c[2], if *seen { "" } else { " unseen" })).collect();
            println!("panic cells: {} of {} covered (a seen post within 8 - margin from every point){}", f.cover.len() - bad.len(), f.cover.len(), if bad.is_empty() { String::new() } else { format!("; not: {}", bad.join(", ")) });
        }
        MobfarmCmd::R1Combine { base, departure, tail, remove, tail_box, name, out } => {
            let rd = |p: &PathBuf| -> Result<mctas::design::Design> { Ok(mctas::design::Design::parse(&std::fs::read_to_string(p)?)?) };
            let bx = |s: &str| -> Result<([i32; 3], [i32; 3])> {
                let n: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(n.len() == 6, "a box is X0,Y0,Z0,X1,Y1,Z1");
                Ok(([n[0], n[1], n[2]], [n[3], n[4], n[5]]))
            };
            let rm: Vec<([i32; 3], [i32; 3])> = remove.split(';').filter(|s| !s.trim().is_empty()).map(bx).collect::<Result<_>>()?;
            let c = crate::combine::combine(&rd(&base)?, &rd(&departure)?, &rd(&tail)?, &rm, bx(&tail_box)?, &name).map_err(|e| anyhow::anyhow!(e))?;
            for l in &c.lines {
                println!("{l}");
            }
            std::fs::write(&out, c.design.to_text())?;
            println!("{}", out.display());
        }
        MobfarmCmd::ContactScenes { design, case, speeds, out_dir, ticks, front_frac, from_rest } => {
            let src = mctas::design::Design::parse(&std::fs::read_to_string(&design)?)?;
            let case = match case.as_str() {
                "g0" => crate::contact_scene::Case::G0,
                "p" => crate::contact_scene::Case::P,
                _ => anyhow::bail!("--case: g0 or p"),
            };
            std::fs::create_dir_all(&out_dir)?;
            let src_name = design.file_name().map_or("the design".to_string(), |n| n.to_string_lossy().to_string());
            for v in &speeds {
                let d = crate::contact_scene::scene(&src, case, *v, front_frac, from_rest).map_err(|e| anyhow::anyhow!(e))?;
                let path = out_dir.join(format!("{}.txt", d.name));
                std::fs::write(&path, crate::contact_scene::caption(case, *v, &src_name) + &d.to_text())?;
                std::fs::write(out_dir.join(format!("{}.drive", d.name)), crate::contact_scene::drive(case, *v, ticks))?;
                let (_c, spec) = load(version, &path)?;
                let scores = crate::contact_scene::score(&spec, ticks).map_err(|e| anyhow::anyhow!(e))?;
                for l in crate::contact_scene::score_lines(&d.name, &scores) {
                    println!("{l}");
                }
            }
        }
        MobfarmCmd::QueueLoopRun { design, cycles, seed, seeds, release_for, advance_at, advance_every, pulse, depart_every, stand_brake, stand_detect, stand_detect_each, intake_brake, inject_at, depart_at, depart_period, open_after, wired, golem_for, stand_reset, shift_depart, s1_at, tail0_late, bench_stand, stand_offset, release_stagger, tail_check_at, stand_detect_slots, sweep, sweep_range, corners, entry_speed, adv_shift, adv_width, skew, ideal_stand_at, tail_check_rest, tail2_at, placed_raw, placed_both, placed_in_order, placed_all, build_gap_sweep, siding, siding_at, siding_turn, siding_box, stagger_off, stagger_common_off, side_drop, side_drop_staged, side_drop_staged2, shift, stop_open, stop_close, pocket_release, pocket_reset, stop_brake_delay, stop_cv, jobs, reload_orders, force_phase, phase_sweep, gate_close_delay, visit, visits_nightly, visit_release_k, visit_release_cells, visit_t_at_release, visit_k_mod3, visits_observe, divert_branch, divert_at, divert_cell, t1_cell, t2_cell, siding_release_cell, rel_for, pocket_trigger, no_summons, calm, reach_probe, exposure_out, record_positions, golem_miss, no_inject, start_random, fifo_intake, esc_b, verbose_tail, tail_trace, verbose, watch, watch_cells, cross_list, watch_mob, watch_mob_slots, golem_jitter, golem_late, day_only, slept, summon_at, summon_unaligned, no_cross_push_check, placed_yaw, act, reset_queue_at, reset_stragglers, reset_stand_carts, reset_queue_every, drive_out, esc_adm, adm_k2, adm_evidence, adm_rpulse, rider_space_control, sight, p_always, adm_fault, entry_hold, hold_landing_rest, hold_strand_t, hold_progress, reg_interlock, l33_delay, adm_skip_req, arrivals_out, arrival_log, hold_back, bench_release, release_start, release_offs, release_computed, release_from, flee_gate, no_flee_check, no_eye_check, takeoff_list, calm_list, no_junction_check, esc_bands, no_blind_check, blind_all, freeze_rows, freeze_for, freeze_after, place_rest, rest_speeds, c_prime, rel_ext, rel_min, fault_table, perfect_tail, all_corners, cprime_variants, bed_distance, approach_trace, rel_clear, head_wait, head_latch, corner_table, speeds_out, rise_list } => {
            let (c, mut spec) = load(version, &design)?;
            let o = crate::queue::loop_opts_of(&c.clock.design.meta)?;
            // (the design's own declared states and flights; a pasted part's join them, moved and
            // turned as its blocks)
            let mut hdr = crate::queue::headers_of(&c.clock.design.meta).map_err(|e| anyhow::anyhow!(e))?;
            if let Some(s) = &rider_space_control {
                hdr.rider_space_control = crate::declared::parse_cells(s).map_err(|e| anyhow::anyhow!("--rider-space-control: {e}"))?;
            }
            if let Some(s) = &sight {
                hdr.sight.extend(crate::declared::parse_boxes(s).map_err(|e| anyhow::anyhow!("--sight: {e}"))?);
            }
            if let Some(sf) = &siding {
                // (a part pasted into the loop: farmopt's bed siding, inside its box)
                let (pc, part) = load(version, sf)?;
                let at = match siding_at.as_slice() { [x, y, z] => [*x, *y, *z], _ => anyhow::bail!("--siding needs --siding-at x,y,z") };
                let ph = crate::queue::headers_of(&pc.clock.design.meta).map_err(|e| anyhow::anyhow!(e))?;
                let mv = |c: [i32; 3]| {
                    let (mut x, mut z) = (c[0], c[2]);
                    for _ in 0..siding_turn.rem_euclid(4) {
                        (x, z) = (-z, x);
                    }
                    [x + at[0], c[1] + at[1], z + at[2]]
                };
                for d in ph.declared {
                    hdr.declared.push(crate::declared::Decl { cell: mv(d.cell), ..d });
                }
                for f in ph.flights {
                    hdr.flights.push(crate::flight::Flight { take_off: mv(f.take_off), landing: f.landing.iter().map(|l| mv(*l)).collect(), max_air: f.max_air });
                }
                for c in ph.rider_space_ok {
                    hdr.rider_space_ok.push(mv(c));
                }
                for c in ph.cross_push_ok {
                    hdr.cross_push_ok.push(mv(c));
                }
                let bx = match siding_box.as_slice() { [a, b, c, d, e, f] => ([*a, *b, *c], [*d, *e, *f]), [] => ([-7, -4, -3], [6, 4, 9]), _ => anyhow::bail!("--siding-box takes x0,y0,z0,x1,y1,z1 in the part's own frame") };
                let n = crate::queue::paste_part(&mut spec, &part, at, siding_turn, bx).map_err(|e| anyhow::anyhow!(e))?;
                eprintln!("pasted {n} blocks of {} at {:?}, turned {siding_turn}", sf.display(), at);
            }
            let intake_brake = match intake_brake.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--intake-brake takes two values: slots 1-2, then slot 0") };
            let inject_at = inject_at.or(if o.line { Some(110) } else { None });
            let wired = match wired.as_slice() { [x, y, z] => Some(redsim::pos::BlockPos::new(*x, *y, *z)), [] => None, _ => anyhow::bail!("--wired takes the input cell: x,y,z") };
            let stand_reset = match stand_reset.as_slice() { [x, y, z] => Some(redsim::pos::BlockPos::new(*x, *y, *z)), [] => None, _ => anyhow::bail!("--stand-reset takes a cell: x,y,z") };
            let stand_detect_slots = match stand_detect_slots.as_slice() { [a, b, c] => Some([*a, *b, *c]), [] => None, _ => anyhow::bail!("--stand-detect-slots takes 3 delays: S1,S2,S3") };
            let cell3 = |v: &[i32]| -> Result<redsim::pos::BlockPos> { match v { [x, y, z] => Ok(redsim::pos::BlockPos::new(*x, *y, *z)), _ => anyhow::bail!("a cell takes x,y,z") } };
            let rel_k: [Option<i64>; 3] = match visit_release_k.as_slice() { [a, b, c] => [Some(*a), Some(*b), Some(*c)], [] => [None; 3], _ => anyhow::bail!("--visit-release-k takes 3 ticks: carts 0, 1, 2") };
            let rel_cells: [Option<redsim::pos::BlockPos>; 3] = match visit_release_cells.as_slice() {
                [a, b, c, d, e, f, g, h, i] => [Some(redsim::pos::BlockPos::new(*a, *b, *c)), Some(redsim::pos::BlockPos::new(*d, *e, *f)), Some(redsim::pos::BlockPos::new(*g, *h, *i))],
                [] => [None; 3],
                _ => anyhow::bail!("--visit-release-cells takes 3 cells: carts 0, 1, 2 (9 numbers)"),
            };
            let tm = crate::queue::LoopTiming { release_for, advance_at, advance_every, pulse, depart_every, stand_brake, stand_detect, stand_detect_each, intake_brake, inject_at, depart_at, depart_period, open_after, wired, golem_for, stand_reset, shift_depart, s1_at, tail0_late, bench_stand, stand_offset, release_stagger, tail_check_at, stand_detect_slots, entry_speed, adv_shift: match adv_shift.as_slice() { [a, b, c] => [*a, *b, *c], [] => [0; 3], _ => anyhow::bail!("--adv-shift takes 3 shifts") }, adv_width: match adv_width.as_slice() { [a, b, c] => [*a, *b, *c], [] => [0; 3], _ => anyhow::bail!("--adv-width takes 3 deltas") }, skew: { let mut s = [0i64; 24]; for (i, v) in skew.iter().enumerate().take(24) { s[i] = *v; } s }, ideal_stand_at, tail_check_rest, tail2_at, placed_raw, build_gap: placed_in_order.filter(|_| !placed_all && !build_gap_sweep).map(|g| if g == 0 { 20 } else { g }), build_priced: placed_in_order == Some(0), stagger_off: stagger_off || !stagger_common_off, side_drop: match side_drop.as_slice() { [c, s, d, v] => Some(crate::queue::SideDrop { cycle: *c as i64, slot: *s as usize, at: *d as i64, speed: *v, park: *s as usize, staged: side_drop_staged || side_drop_staged2, staged2: side_drop_staged2 }), [c, s, d, v, p] => Some(crate::queue::SideDrop { cycle: *c as i64, slot: *s as usize, at: *d as i64, speed: *v, park: *p as usize, staged: side_drop_staged || side_drop_staged2, staged2: side_drop_staged2 }), [] => None, _ => anyhow::bail!("--side-drop takes C,SLOT,D,V[,PARK]") }, stop_open, stop_close, pocket_release, pocket_reset, stop_brake_delay, stop_cv, pocket_close_delay: gate_close_delay, rel_for, pocket_trigger: match pocket_trigger.as_slice() { [a, b, c, d] => Some([(*a, *b as i64), (*c, *d as i64)]), [] => None, _ => anyhow::bail!("--pocket-trigger takes X1,D1,X2,D2") }, no_summons, calm, reach_probe, visit: match (visit.as_slice(), visits_nightly.as_slice()) {
                ([s, k, r], []) => Some(crate::queue::Visit { slot: *s, k: *k as usize, divert_at: divert_at.unwrap_or(16), release_at: *r, divert: cell3(&divert_cell)?, t1: cell3(&t1_cell)?, t2: cell3(&t2_cell)?, release: cell3(&siding_release_cell)?, every: 0, from: 0, release_k: rel_k, release_cells: rel_cells, t_at_release: visit_t_at_release, t_at: 0, k_mod3: visit_k_mod3, observe: false, branch: redsim::pos::BlockPos::new(0, 0, 0) }),
                ([], [e, r]) | ([], [e, r, _]) => Some(crate::queue::Visit { slot: -1, k: 0, divert_at: divert_at.unwrap_or(16), release_at: *r, divert: cell3(&divert_cell)?, t1: cell3(&t1_cell)?, t2: cell3(&t2_cell)?, release: cell3(&siding_release_cell)?, every: *e, from: visits_nightly.get(2).copied().unwrap_or(12500), release_k: rel_k, release_cells: rel_cells, t_at_release: visit_t_at_release, t_at: 0, k_mod3: visit_k_mod3, observe: false, branch: redsim::pos::BlockPos::new(0, 0, 0) }),
                ([], []) if visits_observe => Some(crate::queue::Visit { slot: -1, k: 0, divert_at: divert_at.unwrap_or(16), release_at: 0, divert: cell3(&divert_cell)?, t1: cell3(&t1_cell)?, t2: cell3(&t2_cell)?, release: cell3(&siding_release_cell)?, every: 0, from: 0, release_k: rel_k, release_cells: rel_cells, t_at_release: false, t_at: 0, k_mod3: visit_k_mod3, observe: true, branch: match divert_branch.as_slice() { [x, y, z] => redsim::pos::BlockPos::new(*x, *y, *z), [] => { let d = cell3(&divert_cell)?; redsim::pos::BlockPos::new(d.x, d.y + 1, d.z - 1) } _ => anyhow::bail!("--divert-branch takes x,y,z") } }),
                ([], []) => None,
                _ => anyhow::bail!("--visit takes S,K,REL; --visits-nightly EVERY,REL[,FROM]; not both"),
            }, build_fills_last: c.clock.design.meta.get("build").is_some_and(|s| s.split_whitespace().any(|t| t == "fills-last")), build_last: match c.clock.design.meta.get("build") {
                Some(s) => {
                    // (`last x,y,z ...`: those cells placed after every other; `fills-last`: the
                    // containers' stocks after every block, main's mobfarm::placing build plan)
                    let mut cells = Vec::new();
                    let mut in_last = false;
                    for t in s.split_whitespace() {
                        match t {
                            "last" => in_last = true,
                            "fills-last" => in_last = false,
                            t => {
                                anyhow::ensure!(in_last, "build: takes 'last x,y,z ...' and 'fills-last' (not {t:?})");
                                let v: Vec<i32> = t.split(',').map(|n| n.parse::<i32>()).collect::<Result<_, _>>()?;
                                anyhow::ensure!(v.len() == 3, "build: last takes x,y,z cells");
                                cells.push(redsim::pos::BlockPos::new(v[0], v[1], v[2]));
                            }
                        }
                    }
                    anyhow::ensure!(cells.len() <= 64, "build: last takes up to 64 cells");
                    let mut a = [None; 64];
                    for (i, p) in cells.into_iter().enumerate() {
                        a[i] = Some(p);
                    }
                    a
                }
                None => [None; 64],
            }, watch_cells: {
                let mut v = [None; 8];
                for (i, c) in watch_cells.split(';').filter(|c| !c.trim().is_empty()).take(8).enumerate() {
                    let n: Vec<i32> = c.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(n.len() == 3, "--watch-cells takes x,y,z;x,y,z");
                    v[i] = Some([n[0], n[1], n[2]]);
                }
                v
            }, cross_cells: {
                let mut v = [None; 8];
                for (i, c) in cross_list.split(';').filter(|c| !c.trim().is_empty()).take(8).enumerate() {
                    let n: Vec<i32> = c.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(n.len() == 3, "--cross-list takes x,y,z;x,y,z");
                    v[i] = Some([n[0], n[1], n[2]]);
                }
                v
            }, watch_mob: {
                anyhow::ensure!(watch_mob.len() <= 4, "--watch-mob: at most 4");
                let mut v = [None; 4];
                for (i, s) in watch_mob.iter().enumerate() {
                    v[i] = Some(crate::queue::parse_rider("--watch-mob", s).map_err(|e| anyhow::anyhow!(e))?);
                }
                v
            }, watch_mob_slots: match watch_mob_slots.as_slice() { [a, b] => (*a, *b), [] => (0, -1), _ => anyhow::bail!("--watch-mob-slots takes S0,S1") }, golem_jitter, golem_late, day_only, slept, summon_at: crate::queue::summon_frame(summon_at, summon_unaligned).map_err(|e| anyhow::anyhow!(e))?, no_cross_push: no_cross_push_check, placed_yaw, acts: { anyhow::ensure!(act.len() <= 8, "--act: at most 8"); let mut a = [None; 8]; for (i, s) in act.iter().enumerate() { a[i] = Some(crate::queue::parse_act(s).map_err(|e| anyhow::anyhow!(e))?); } a }, reset_queue_at, reset_stragglers, reset_stand_carts, reset_queue_every, no_inject, takeoff_list, calm_list, no_blind_check, blind_all, start_random, fifo_intake: match fifo_intake.as_slice() { [d, w] => (*d, *w), [] => (2, 6), _ => anyhow::bail!("--fifo-intake takes D,W") }, esc_b: match esc_b.as_slice() { [k, w] => (*k, *w), [] => (6, 6), _ => anyhow::bail!("--esc-b takes K,W") }, esc_adm: match esc_adm.as_slice() { [d, w] => (*d, *w), [] => (2, 4), _ => anyhow::bail!("--esc-adm takes DELAY,W") }, adm_k2, adm_evidence, adm_rpulse, entry_hold, hold_landing_rest, hold_strand_t, hold_progress, reg_interlock, l33_delay, arrival_log, hold_back: match hold_back.as_slice() { [c, d, n] => Some([*c, *d, *n]), [] => None, _ => anyhow::bail!("--hold-back takes C,DT,N") }, p_always, adm_faults: {
                anyhow::ensure!(adm_fault.len() <= 8, "--adm-fault: at most 8");
                let mut a = [None; 8];
                for (i, s) in adm_fault.iter().enumerate() {
                    let f: Vec<&str> = s.split(',').collect();
                    let (slot, adv) = match (f.first().and_then(|v| v.parse::<i64>().ok()), f.get(1).and_then(|v| v.parse::<i64>().ok())) { (Some(s), Some(k)) if (0..3).contains(&k) => (s, k), _ => anyhow::bail!("--adm-fault {s}: SLOT,ADV(0..2),KIND[,OFF]") };
                    let kind = match f.get(2).copied() { Some("skip") => 0u8, Some("miss-g0") => 1, Some("miss-gp") => 2, Some("extra-g0") => 3, Some("extra-gp") => 4, _ => anyhow::bail!("--adm-fault {s}: KIND is skip, miss-g0, miss-gp, extra-g0 or extra-gp") };
                    let off = f.get(3).map(|v| v.parse::<i64>()).transpose()?.unwrap_or(10);
                    a[i] = Some((slot, adv, kind, off));
                }
                a
            }, adm_skip_req: {
                anyhow::ensure!(adm_skip_req.len() <= 8, "--adm-skip-req: at most 8");
                let mut a = [None; 8];
                for (i, s) in adm_skip_req.iter().enumerate() {
                    a[i] = match s.split(',').map(|v| v.parse::<i64>()).collect::<Result<Vec<_>, _>>()?.as_slice() { [slot, dt] => Some((*slot, *dt)), _ => anyhow::bail!("--adm-skip-req {s}: SLOT,DT") };
                }
                a
            }, arrivals: arrivals_out.is_some(), speeds: speeds_out.is_some(), rise_list, verbose_tail, tail_trace: match tail_trace.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--tail-trace takes C0,C1") }, pulse_at: c.clock.design.ports.get("output.pulse").map(|p| redsim::pos::BlockPos::new(p[0], p[1], p[2])), release_start, bench_release, release_offs: match (release_computed, release_offs.as_slice()) { (true, _) | (false, []) => None, (false, [a, b, c]) => Some([*a, *b, *c]), _ => anyhow::bail!("--release-offs takes Q15,Q16,Q17") }, release_from, c_prime: match c_prime.as_slice() { [x, y, z, d, f] => Some(([*x as i32, *y as i32, *z as i32], *d, *f, 40)), [x, y, z, d, f, l] => Some(([*x as i32, *y as i32, *z as i32], *d, *f, *l)), [] => None, _ => anyhow::bail!("--c-prime takes X,Y,Z,DELAY,FOR[,LOCK]") }, rel_ext, rel_min, bed_at: match bed_distance.as_slice() { [x, y, z] => Some([*x, *y, *z]), [] => None, _ => anyhow::bail!("--bed-distance takes X,Y,Z") }, approach_trace: match approach_trace.as_slice() { [a, b] => Some((*a, *b, None)), [a, b, x] => Some((*a, *b, Some(*x as i32))), [] => None, _ => anyhow::bail!("--approach-trace takes C0,C1[,X]") }, rel_clear: match rel_clear.as_slice() { [x, y, z] => Some([*x, *y, *z]), [] => None, _ => anyhow::bail!("--rel-clear takes X,Y,Z") }, head_wait, head_latch: match head_latch.as_slice() { [k, x, y, z] => Some((*k as usize, [*x, *y, *z])), [] => None, _ => anyhow::bail!("--head-latch takes K,X,Y,Z") }, ..Default::default() };
            // (`--place-rest`: the approach's rest check instead of a run)
            if let Some(pr) = &place_rest {
                let cells: Vec<[i32; 3]> = if pr == "approach" {
                    Vec::new()
                } else {
                    pr.split(';').filter(|s| !s.trim().is_empty()).map(|s| {
                        let v: Vec<i32> = s.split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 3, "--place-rest takes x,y,z;x,y,z or approach");
                        Ok([v[0], v[1], v[2]])
                    }).collect::<Result<_>>()?
                };
                println!("{}", crate::queue::rest_check(&spec, &o, &cells, &rest_speeds, cycles.max(3) * 100).map_err(|e| anyhow::anyhow!(e))?);
                return Ok(());
            }
            // (`--fault-table`, `--corner-table`: farmopt's approach lab tables)
            let seed_list: Vec<i64> = (seed..seed + seeds).collect();
            if let Some(f) = &fault_table {
                let mut t = tm;
                for s in &shift {
                    let (n, v) = s.split_once('=').ok_or_else(|| anyhow::anyhow!("--shift takes name=gt"))?;
                    t = crate::queue::loop_shifted(&t, n, v.parse()?).map_err(|e| anyhow::anyhow!(e))?;
                }
                // (`--cprime-variants`: the table for each set of C''s delays, the design generated
                // again from its queue: line with that set)
                let variants: Vec<Option<[i32; 6]>> = if cprime_variants.is_empty() {
                    vec![None]
                } else {
                    cprime_variants.split(';').map(|v| {
                        let d: Vec<i32> = v.split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(d.len() == 6, "--cprime-variants takes R1,R2,R3,R4,RIN,RS1;...");
                        Ok(Some([d[0], d[1], d[2], d[3], d[4], d[5]]))
                    }).collect::<Result<_>>()?
                };
                for v in variants {
                    let (o2, spec2) = match v {
                        None => (o, spec.clone()),
                        Some(d) => {
                            let o2 = crate::queue::LoopOpts { cprime: Some(d), ..o };
                            let p = std::env::temp_dir().join(format!("cprime-variant-{}-{}.txt", std::process::id(), d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("_")));
                            std::fs::write(&p, crate::queue::loop_design(&o2)?)?;
                            let (_, s2) = load(version, &p)?;
                            let _ = std::fs::remove_file(&p);
                            // (C''s delay from D's rise to T3 lit: 2 gt a repeater tick, and K, T_n
                            // and T3 one tick each)
                            println!("== C' delays {:?}: D's rise to T3 lit {} gt", d, 2 * (d[0] + d[1] + d[2] + d[3] + d[4]) + 6);
                            (o2, s2)
                        }
                    };
                    println!("{}", crate::approach::fault_table(&spec2, &o2, &t, &seed_list, cycles, jobs, perfect_tail, if f.is_empty() { None } else { Some(f.as_str()) }, all_corners).map_err(|e| anyhow::anyhow!(e))?);
                }
                return Ok(());
            }
            if corner_table {
                println!("{}", crate::approach::corner_table(&spec, &o, &tm, &seed_list, cycles, jobs).map_err(|e| anyhow::anyhow!(e))?);
                return Ok(());
            }
            // (`--shift name=gt,...`: controls shifted as the sweep shifts them, e.g. to rerun a
            // corner without one of its controls)
            let mut tm = tm;
            for s in &shift {
                let (n, v) = s.split_once('=').ok_or_else(|| anyhow::anyhow!("--shift takes name=gt"))?;
                tm = crate::queue::loop_shifted(&tm, n, v.parse()?).map_err(|e| anyhow::anyhow!(e))?;
            }
            // (the walk-target static check, printed by default; its flee verdict fails the run with
            // --flee-gate (ironspec Nov 11 09:29: (ii) until runs are grounded); printed once here
            // for a sweep, whose runs don't carry the headers)
            if !no_flee_check {
                let g = crate::queue::flee_gate(&spec, &std::fs::read_to_string(&design)?);
                let g = (format!("{}{}", g.0, if flee_gate { "" } else { " (printed only: a void bench's verdict; --flee-gate fails the run on it)" }), g.1 && flee_gate);
                if !sweep.is_empty() || phase_sweep.is_some() {
                    println!("{}{}", g.0, if g.1 { " -- the static gate FAILS" } else { "" });
                }
                hdr.static_checks.push(g);
            }
            // (the census's rider-eye check, ironspec Nov 11 11:53: a suffocating block at any
            // seated rider's eye along the track fails the run by default; the blind places
            // printed)
            if !no_eye_check {
                let (n, hits) = crate::fleecheck::eye_check(&spec, false).map_err(|e| anyhow::anyhow!(e))?;
                let (lines, fails) = crate::fleecheck::eye_lines(n, &hits, 0);
                let g = (lines.first().cloned().unwrap_or_default(), fails);
                if !sweep.is_empty() || phase_sweep.is_some() {
                    println!("{}{}", g.0, if g.1 { " -- the static gate FAILS" } else { "" });
                }
                hdr.static_checks.push(g);
            }
            // (the pickup reach, a default gate (ironspec Nov 11 14:32): a zombie's or boat's box
            // inside an empty cart's pickup box anywhere along the track fails the run)
            if !no_eye_check {
                let (plines, pfails) = crate::fleecheck::pickup_check(&spec, false);
                let g = (plines.first().cloned().unwrap_or_default(), pfails);
                if !sweep.is_empty() || phase_sweep.is_some() {
                    println!("{}{}", g.0, if g.1 { " -- the static gate FAILS" } else { "" });
                }
                hdr.static_checks.push(g);
            }
            // (the rail-junction check, a default gate (ironspec Nov 11 12:50): a stored junction
            // shape the game would not keep fails the run; the opt-out says so)
            if !no_junction_check {
                let g = crate::queue::junction_gate(&spec, &std::fs::read_to_string(&design)?);
                if !sweep.is_empty() || phase_sweep.is_some() {
                    println!("{}{}", g.0, if g.1 { " -- the static gate FAILS" } else { "" });
                }
                hdr.static_checks.push(g);
            } else {
                hdr.static_checks.push(("rail junctions: NOT CHECKED (--no-junction-check)".to_string(), false));
            }
            if esc_bands {
                println!("{}", crate::queue::esc_bands(&spec, &o, &tm, sweep_range, seed, seeds, cycles, jobs).map_err(|e| anyhow::anyhow!(e))?);
                return Ok(());
            }
            if let Some(f) = &freeze_rows {
                let v: Vec<i64> = f.split(',').map(|s| s.trim().parse::<i64>()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("--freeze-rows: {e}"))?;
                let [slot, dt0, dt1, x0, z0, x1, z1] = v[..] else { anyhow::bail!("--freeze-rows takes SLOT,DT0,DT1,CX0,CZ0,CX1,CZ1") };
                println!("{}", crate::queue::freeze_rows(&spec, &o, &tm, slot, dt0, dt1, [x0 as i32, z0 as i32, x1 as i32, z1 as i32], &freeze_for, freeze_after, seed, jobs).map_err(|e| anyhow::anyhow!(e))?);
                return Ok(());
            }
            if !sweep.is_empty() {
                // the tolerance lines: one control at a time, to the first failure each way
                let names: Vec<String> = if sweep == ["all"] { ["release-start", "release-end", "adv1-time", "adv1-width", "adv2-time", "adv2-width", "adv3-time", "adv3-width", "s1", "s2", "s3", "stand-brake", "tail12", "tail0", "speed"].iter().map(|s| s.to_string()).collect() } else { sweep.clone() };
                let mut lines = Vec::new();
                for name in &names {
                    let l = crate::queue::loop_sweep_jobs(&spec, &o, &tm, name, sweep_range, seed, seeds, cycles, jobs).map_err(|e| anyhow::anyhow!(e))?;
                    println!("{}", l.text);
                    lines.push(l);
                }
                if corners {
                    println!("{}", crate::queue::loop_corners(&spec, &o, &tm, &lines, seed, seeds, cycles).map_err(|e| anyhow::anyhow!(e))?);
                }
                return Ok(());
            }
            if let Some(f) = &force_phase {
                tm.phase_force = Some(crate::queue::parse_phase_force(f).map_err(|e| anyhow::anyhow!(e))?);
            }
            if let Some(sw) = &phase_sweep {
                return phase_sweep_cmd(&spec, &o, &tm, &hdr, sw, &reload_orders, cycles, seed, jobs);
            }
            if !reload_orders.is_empty() {
                return reload_orders_cmd(&spec, &o, &tm, &hdr, &reload_orders, cycles, seed, jobs);
            }
            if seeds > 1 || placed_both || placed_all || build_gap_sweep {
                // one line per seed, then the total (the drive log, watch and verbose apply to
                // single runs); `placed_both`: the judge's placement, then the raw paste;
                // `placed_all`: and the build in order at the priced gap; `build_gap_sweep`: the
                // build in order at every gap from 1 gt up to the priced one, to the first failure
                let priced_in_order = placed_in_order.is_none_or(|g| g == 0);
                let modes: Vec<(bool, Option<i64>, bool)> = if build_gap_sweep {
                    (1..=placed_in_order.filter(|g| *g > 0).unwrap_or(40)).map(|g| (false, Some(g), false)).collect()
                } else if placed_all {
                    vec![(false, None, false), (true, None, false), (false, Some(placed_in_order.filter(|g| *g > 0).unwrap_or(20)), priced_in_order)]
                } else if placed_both {
                    vec![(false, None, false), (true, None, false)]
                } else {
                    vec![(tm.placed_raw, tm.build_gap, tm.build_priced)]
                };
                let mut all_clean = true;
                for (raw, gap, priced) in modes {
                    let tm = crate::queue::LoopTiming { placed_raw: raw, build_gap: gap, build_priced: priced, ..tm };
                    let label = match (raw, gap, priced) {
                        (_, Some(_), true) => "built in order, each cell's priced gap (20 gt, 40 gt at a place:edge cell)".to_string(),
                        (_, Some(g), false) => format!("built in order, {g} gt between placements"),
                        (true, None, _) => "settled as placed (raw paste)".to_string(),
                        (false, None, _) => "the judge's placement".to_string(),
                    };
                    let (mut clean, mut bad) = (0, Vec::new());
                    // (`--jobs`: the seeds run on that many threads at once, results in seed order,
                    // each chunk's seeds printed as soon as it ends: a long run's partial results
                    // survive it)
                    let mut results: Vec<(i64, String)> = Vec::new();
                    let all: Vec<i64> = (seed..seed + seeds).collect();
                    for chunk in all.chunks(jobs.max(1)) {
                        let got: Vec<(i64, Result<String, String>)> = std::thread::scope(|sc| {
                            let hs: Vec<_> = chunk.iter().map(|&s| { let (spec, o, tm, hdr) = (&spec, &o, &tm, &hdr); sc.spawn(move || (s, crate::queue::run_loop_hdr(spec, o, tm, hdr, cycles, s, false, None, None))) }).collect();
                            hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
                        });
                        for (s, r) in got {
                            let r = r.map_err(|e| anyhow::anyhow!(e))?;
                            let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("?").to_string();
                            let first = r.lines().skip_while(|l| !l.ends_with("problems:")).nth(1).unwrap_or("").trim().to_string();
                            if seeds > 1 {
                                // (and how often a rider took damage, a zombie in reach say)
                                let hurt = r.matches(" hurt at slot ").count();
                                // (the faults' reach: which slots had a problem, and the lap order)
                                let reach: String = r.lines().filter(|l| l.starts_with("problem slots: ") || l.starts_with("lap order: CHANGED")).map(|l| format!(" | {}", l.split(" (").next().unwrap_or(l))).collect();
                                // (admission control's margins, compact: the fastest front at a G0
                                // opening, the closest rider gap, the fewest rider ticks)
                                let margins: String = r.lines().find_map(|l| l.strip_prefix("admission margins: ")).map(|l| {
                                    let num_after = |key: &str| l.split(key).nth(1).and_then(|t| t.split_whitespace().next()).unwrap_or("-").to_string();
                                    format!(" | G0 front vmax {} rider gap {} rider gt {}", num_after("opening "), num_after("at least "), l.split("reached P's end at least ").nth(1).and_then(|t| t.split_whitespace().next()).unwrap_or("-"))
                                }).unwrap_or_default();
                                println!("{label}, seed {s}: {probs} {first}{}{reach}{margins}", if hurt > 0 { format!(" (riders hurt {hurt} times)") } else { String::new() });
                                use std::io::Write;
                                let _ = std::io::stdout().flush();
                            }
                            results.push((s, r));
                        }
                    }
                    for (s, r) in &results {
                        let s = *s;
                        let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("?").to_string();
                        if probs.starts_with("0 problems") { clean += 1 } else { bad.push(s) }
                    }
                    all_clean &= bad.is_empty();
                    println!("{label}: {clean} of {seeds} seeds clean over {cycles} slots each{}", if bad.is_empty() { String::new() } else { format!("; problems on seeds {bad:?}") });
                    // (the nap visits over every seed: per stand cart (its drop slot), how many,
                    // how many came back, the roll-off speeds; and the nights with every rider
                    // asleep)
                    let mut per_k: std::collections::BTreeMap<String, (usize, usize, f64, f64, Vec<f64>)> = std::collections::BTreeMap::new();
                    let (mut nights, mut full_nights) = (0usize, 0usize);
                    for (_, r) in &results {
                        for l in r.lines() {
                            if let Some(rest) = l.strip_prefix("visit slot ") {
                                let k = rest.split(" cart ").nth(1).and_then(|t| t.split(':').next()).unwrap_or("?").to_string();
                                let e = per_k.entry(k).or_insert((0, 0, f64::MAX, 0.0, Vec::new()));
                                e.0 += 1;
                                if l.contains("back at rest in slot") {
                                    e.1 += 1;
                                }
                                if let Some(v) = l.split("rolled off its drop rail at +").nth(1).and_then(|t| t.split(" at ").nth(1)).and_then(|t| t.split(' ').next()).and_then(|t| t.parse::<f64>().ok()) {
                                    e.2 = e.2.min(v);
                                    e.3 = e.3.max(v);
                                    e.4.push(v);
                                }
                            }
                            if let Some(rest) = l.strip_prefix("visits: ") {
                                for n in rest.split("; ").filter(|t| t.starts_with("night ")) {
                                    nights += 1;
                                    if n.split(": ").nth(1).is_some_and(|t| { let w: Vec<&str> = t.split_whitespace().collect(); w.len() >= 3 && w[0] == w[2] }) {
                                        full_nights += 1;
                                    }
                                }
                            }
                        }
                    }
                    if !per_k.is_empty() {
                        let parts: Vec<String> = per_k.iter().map(|(k, (n, back, lo, hi, vs))| {
                            let mut vs = vs.clone();
                            vs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            let pct = |p: f64| vs.get(((vs.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0.0);
                            format!("cart {k}: {back} of {n} back, roll-off {lo:.2}..{hi:.2} b/t (median {:.2}, 99th pct {:.2})", pct(0.5), pct(0.99))
                        }).collect();
                        println!("{label}: nap visits over every seed: {}; nights with every rider asleep: {full_nights} of {nights}", parts.join("; "));
                    }
                    // (the bed's nearest awake villager over every seed: each run's `bed:` line, the
                    // least (ironspec 2026-11-11: the minimum across runs decides the margin))
                    let mut bed_min: Option<(f64, String)> = None;
                    for (seed, r) in &results {
                        for l in r.lines() {
                            let Some(rest) = l.strip_prefix("bed: the nearest awake villager while a bed was taken ") else { continue };
                            let Some(d) = rest.split_whitespace().next().and_then(|t| t.parse::<f64>().ok()) else { continue };
                            if bed_min.as_ref().is_none_or(|b| d < b.0) {
                                bed_min = Some((d, format!("seed {seed}: {rest}")));
                            }
                        }
                    }
                    if let Some((d, at)) = bed_min {
                        println!("{label}: bed over every seed: least {d:.2} ({at})");
                    }
                    // (the pocket stand's margins over every seed: each run's `stand margins:` line)
                    let mut agg: Option<[i64; 9]> = None;
                    for (_, r) in &results {
                        let Some(l) = r.lines().find(|l| l.starts_with("stand margins: ")) else { continue };
                        let v: Vec<i64> = l.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                        if v.len() != 9 || v[8] == 0 {
                            continue;
                        }
                        agg = Some(match agg {
                            None => [v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]],
                            Some(a) => [a[0].min(v[0]), a[1].max(v[1]), a[2].min(v[2]), a[3].max(v[3]), a[4].min(v[4]), a[5].max(v[5]), a[6].min(v[6]), a[7].max(v[7]), a[8] + v[8]],
                        });
                    }
                    if let Some(a) = agg {
                        println!("{label}: the pocket stand over {} slots: cart 1 to cart 2 at D2 {}..{} gt apart; G2 shut {}..{} gt after cart 2's front was in it ({}..{} after its tail left), {}..{} gt before cart 3 reached it", a[8], a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
                    }
                    // (the pad over every seed: each run's `pad stats:` line, its numbers in order:
                    // golems, slots with one, slots, crossed, died, alive, pulses, spurious, iron,
                    // out of turn)
                    let mut pad: Option<[i64; 10]> = None;
                    for (_, r) in &results {
                        let Some(l) = r.lines().find(|l| l.starts_with("pad stats: ")) else { continue };
                        let v: Vec<i64> = l.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                        if v.len() != 10 {
                            continue;
                        }
                        let mut a = pad.unwrap_or([0; 10]);
                        for i in 0..10 {
                            a[i] += v[i];
                        }
                        pad = Some(a);
                    }
                    if let Some(a) = pad {
                        // (an hour is 72,000 gt: 720 slots)
                        let hours = a[2] as f64 * 100.0 / 72000.0;
                        println!("{label}: the pad over {} slots ({hours:.1} h): {} golems ({:.0}/h), {} slots with a golem ({:.2}%); {} through the portal, {} killed in the Nether, {} alive at a run's end; pulses {} with a golem, {} without; iron into storage {} ingots ({:.0}/h, {:.2} a golem); summons out of turn {}", a[2], a[0], a[0] as f64 / hours, a[1], 100.0 * a[1] as f64 / a[2].max(1) as f64, a[3], a[4], a[5], a[6], a[7], a[8], a[8] as f64 / hours, a[8] as f64 / a[4].max(1) as f64, a[9]);
                    }
                    // (the summon ticks over every seed: each run's `summon stats:` line, 15
                    // counts: willing 0..3, panicking 0..3, at the stand 0..3, the unwilling's
                    // reasons (never slept, slept 24,000 gt ago or more, a golem seen lately))
                    let mut st: Option<[i64; 15]> = None;
                    for (_, r) in &results {
                        let Some(l) = r.lines().find(|l| l.starts_with("summon stats: ")) else { continue };
                        let v: Vec<i64> = l.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                        if v.len() != 15 {
                            continue;
                        }
                        let mut a = st.unwrap_or([0; 15]);
                        for i in 0..15 {
                            a[i] += v[i];
                        }
                        st = Some(a);
                    }
                    if let Some(a) = st {
                        let n = (a[0] + a[1] + a[2] + a[3]).max(1) as f64;
                        println!("{label}: at the summon ticks ({} of them): stand riders willing 3: {:.2}%, 2: {:.2}%, 1: {:.2}%, 0: {:.2}%; panicking 3: {:.2}%; riders at the stand 3: {:.2}%; the unwilling: never slept {}, slept 24,000 gt ago or more {}, a golem seen lately {}", n as i64, 100.0 * a[3] as f64 / n, 100.0 * a[2] as f64 / n, 100.0 * a[1] as f64 / n, 100.0 * a[0] as f64 / n, 100.0 * a[7] as f64 / n, 100.0 * a[11] as f64 / n, a[12], a[13], a[14]);
                    }
                    // (power-on over every seed: each run's `power stats:` line, its start day
                    // time, first golem, first release, releases before the first locked one)
                    let pw: Vec<Vec<i64>> = results.iter().filter_map(|(_, r)| r.lines().find(|l| l.starts_with("power stats: ")).map(|l| l.split_whitespace().filter_map(|w| w.parse().ok()).collect::<Vec<i64>>())).filter(|v| v.len() == 6).collect();
                    if !pw.is_empty() {
                        let spread = |k: usize| -> String {
                            let mut v: Vec<i64> = pw.iter().map(|r| r[k]).filter(|x| *x >= 0).collect();
                            v.sort();
                            let never = pw.len() - v.len();
                            if v.is_empty() {
                                return format!("never in {never}");
                            }
                            format!("{}..{} (median {}){}", v[0], v[v.len() - 1], v[v.len() / 2], if never > 0 { format!(", never in {never}") } else { String::new() })
                        };
                        println!("{label}: power-on over {} seeds{}: the first golem at +{} gt; the first release at +{} gt; releases before the first locked one {}", pw.len(), if pw.iter().any(|r| r[0] >= 0) { " (each from its own day time)" } else { "" }, spread(1), spread(2), spread(3));
                    }
                    if build_gap_sweep && !bad.is_empty() {
                        println!("built in order: the first gap that fails is {} gt", gap.unwrap_or(0));
                        break;
                    }
                }
                if placed_both || placed_all {
                    println!("{}", if all_clean { "works every way placed" } else { "FAILS in at least one placement" });
                }
                if build_gap_sweep && all_clean {
                    println!("built in order: every gap from 1 to {} gt passes", placed_in_order.filter(|g| *g > 0).unwrap_or(40));
                }
                return Ok(());
            }
            let tm = crate::queue::LoopTiming { record_positions: record_positions.is_some(), exposure_log: exposure_out.is_some(), golem_miss: { anyhow::ensure!(golem_miss.len() <= 8, "--golem-miss takes up to 8 entries"); let mut a = [(-1i64, -1i64); 8]; for (i, s) in golem_miss.iter().enumerate() { a[i] = match s.split_once('-') { Some((x, y)) => (x.trim().parse()?, y.trim().parse()?), None => { let n: i64 = s.trim().parse()?; (n, n) } }; } a }, ..tm };
            let s = crate::queue::run_loop_hdr(&spec, &o, &tm, &hdr, cycles, seed, verbose, if watch.is_empty() { None } else { Some(&watch) }, drive_out.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
            // (`--record-positions F`: the villager positions line written to F, x y z per line)
            if let Some(f) = &record_positions {
                let line = s.lines().find_map(|l| l.strip_prefix("villager positions (")).ok_or_else(|| anyhow::anyhow!("no positions recorded"))?;
                let cells = line.split_once("): ").map(|(_, c)| c).unwrap_or("");
                let text: String = cells.split_whitespace().map(|c| format!("{}\n", c.replace(',', " "))).collect();
                std::fs::write(f, format!("# villager block positions over {} slots of queue-loop-run {} (seed {seed})\n{text}", cycles, design.display()))?;
                eprintln!("wrote {} positions to {}", text.lines().count(), f.display());
            }
            println!("{}", s.lines().filter(|l| !l.starts_with("villager positions (") && !l.starts_with("arrival c") && !l.starts_with("exposure ")).collect::<Vec<_>>().join("\n"));
            // (`--exposure-out F`: every exposed rider-tick)
            if let Some(f) = &exposure_out {
                let lines: Vec<&str> = s.lines().filter_map(|l| l.strip_prefix("exposure ")).collect();
                std::fs::write(f, format!("# exposed rider-ticks of queue-loop-run {} (seed {seed}, {cycles} slots): game time, slot, offset, rider, x y z, streak, panicked within 20 gt, PATH\n{}\n", design.display(), lines.join("\n")))?;
                eprintln!("wrote {} exposed rider-ticks to {}", lines.len(), f.display());
            }
            if let Some(f) = &arrivals_out {
                let lines: Vec<&str> = s.lines().filter(|l| l.starts_with("arrival c")).collect();
                std::fs::write(f, format!("# arrival stream of queue-loop-run {} (seed {seed}, {cycles} slots): each cart crossing x 8.5 westward on the return run\n{}\n", design.display(), lines.join("\n")))?;
                eprintln!("wrote {} arrivals to {}", lines.len(), f.display());
            }
            if let Some(f) = &speeds_out {
                let lines: Vec<&str> = s.lines().filter_map(|l| l.strip_prefix("speed ")).collect();
                std::fs::write(f, format!("# fastest entry per cell of queue-loop-run {} (seed {seed}, {cycles} slots): x y z v (design cells, blocks per tick)\n{}\n", design.display(), lines.join("\n")))?;
                eprintln!("wrote {} cells' speeds to {}", lines.len(), f.display());
            }
        }
        MobfarmCmd::Queue { scene, carts, spacing, release, no_riders, sweep, out } => {
            let o = crate::queue::QueueOpts { scene, carts, spacing, release, riders: !no_riders, sweep };
            let text = crate::queue::design(&o)?;
            std::fs::write(&out, text)?;
            println!("{}", out.display());
        }
        MobfarmCmd::QueueRun { design, ticks, release_at, release_for, queue_at, queue_for, every, seed, cycle_start, cycle_every, cycles, trigger_x, pulse } => {
            let (c, spec) = load(version, &design)?;
            let o = crate::queue::opts_of(&c.clock.design.meta)?;
            let cyc: Vec<i64> = cycle_start.map(|s| (0..cycles).map(|k| s + k * cycle_every).collect()).unwrap_or_default();
            let mut plan = crate::queue::RunPlan { ticks, release: Vec::new(), queue: Vec::new(), every, cycles: cyc, trigger_x, pulse };
            if let Some(t) = release_at {
                plan.release = vec![(t, true), (t + release_for, false)];
            }
            if let Some(t) = queue_at {
                plan.queue = vec![(t, true), (t + queue_for, false)];
            }
            let s = crate::queue::run(&spec, &o, &plan, seed).map_err(|e| anyhow::anyhow!(e))?;
            println!("{s}");
        }
        MobfarmCmd::Slice { design, x, y_from, y_to, z_from, z_to } => {
            let d = mctas::design::Design::parse(&std::fs::read_to_string(&design)?)?;
            print!("{}", slice_text(&d, x, y_from, y_to, z_from, z_to));
        }
        MobfarmCmd::Hold { pitch, levels, level_gap, run, stand_z0, boost_every, margin, no_zombies, yard, unpaired, feeds, out } => {
            let o = crate::hold::LaneOpts { pitch, levels, level_gap, run, stand_z0, boost_every, margin, zombies: !no_zombies, yard, paired: !unpaired, feeds, ..Default::default() };
            let text = crate::hold::design(&o)?;
            std::fs::write(&out, text)?;
            println!("{}", out.display());
        }
        MobfarmCmd::Station { x0, x1, planes, by, h, skip, part, out } => {
            let planes: Vec<i32> = planes.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>()?;
            let skip: Vec<i32> = match skip {
                Some(s) => s.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>()?,
                None => Vec::new(),
            };
            let o = crate::station::PadOpts { x0, x1, planes, by, h, skip, slabs: Vec::new() };
            let text = if part { crate::station::part(&o, &crate::hold::LaneOpts::default())? } else { crate::station::design(&o, &crate::hold::LaneOpts::default())? };
            std::fs::write(&out, text)?;
            println!("{}", out.display());
        }
        MobfarmCmd::StationCheck { design, verbose, probe, official } => {
            let (c, spec) = load(version, &design)?;
            let lanes = crate::hold::lanes_of(&c.clock.design.meta)?;
            let pad = crate::station::PadOpts::of(&c.clock.design.meta)?;
            if let Some(list) = official {
                let mut cells = Vec::new();
                for p in list.split(';') {
                    let v: Vec<i32> = p.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--official x,y,z;x,y,z;...");
                    cells.push([v[0], v[1], v[2]]);
                }
                for l in crate::station::probe_official(version, &pad, &cells, 14, 60)? {
                    println!("{l}");
                }
                return Ok(());
            }
            if let Some(p) = probe {
                let v: Vec<i32> = p.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--probe x,y,z");
                for l in crate::station::probe_pulse(&spec, &pad, [v[0], v[1], v[2]], 14).map_err(|e| anyhow::anyhow!(e))? {
                    println!("{l}");
                }
                return Ok(());
            }
            let r = crate::station::check(&spec, &lanes, &pad).map_err(|e| anyhow::anyhow!(e))?;
            let ks: Vec<usize> = r.riders.iter().map(|s| s.valid.len() - s.obstructed.len().min(s.valid.len())).collect();
            let strays: usize = r.riders.iter().map(|s| s.stray.len()).sum();
            let mut union: std::collections::BTreeSet<[i32; 3]> = Default::default();
            let mut inter: Option<std::collections::BTreeSet<[i32; 3]>> = None;
            for s in &r.riders {
                let set: std::collections::BTreeSet<[i32; 3]> = s.valid.iter().copied().collect();
                union.extend(set.iter().copied());
                inter = Some(match inter {
                    None => set,
                    Some(i) => i.intersection(&set).copied().collect(),
                });
            }
            println!("pad: {} blocks ({} obsidian, {} portal cells), box x {}..{} y {}..{} z {}..{}", r.pad_blocks, r.obsidian, r.portal_cells, r.pad_box.0[0], r.pad_box.1[0], r.pad_box.0[1], r.pad_box.1[1], r.pad_box.0[2], r.pad_box.1[2]);
            {
                // the Nether part (kill, collection, the player's way): its blocks by kind
                let (nb, nbes, _) = crate::station::nether_part();
                let mut kinds: std::collections::BTreeMap<String, u32> = Default::default();
                let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
                for (p, s) in &nb {
                    let name = s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
                    *kinds.entry(name).or_insert(0) += 1;
                    for i in 0..3 {
                        lo[i] = lo[i].min(p[i]);
                        hi[i] = hi[i].max(p[i]);
                    }
                }
                let mut v: Vec<(String, u32)> = kinds.into_iter().collect();
                v.sort_by(|a, b| b.1.cmp(&a.1));
                println!("Nether part: {} blocks, box x {}..{} y {}..{} z {}..{} ({} with block-entity data): {}", nb.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], nbes.len(), v.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", "));
                // the whole design (the farm optimizer's volume: its blocks' and ports' box)
                let (clo, chi) = c.bounds();
                let vol = (chi[0] - clo[0] + 1) as i64 * (chi[1] - clo[1] + 1) as i64 * (chi[2] - clo[2] + 1) as i64;
                let nvol = (hi[0] - lo[0] + 1) as i64 * (hi[1] - lo[1] + 1) as i64 * (hi[2] - lo[2] + 1) as i64;
                println!("the whole design: {} overworld blocks + {} Nether, overworld box x {}..{} y {}..{} z {}..{} = {} (+ the Nether part's {}), {} entities", c.clock.design.blocks.len(), nb.len(), clo[0], chi[0], clo[1], chi[1], clo[2], chi[2], vol, nvol, c.clock.design.entities.len());
            }
            println!("valid spots per rider (portal contact, not under the batch's own carts): min {} max {}; union {}, in every search {}", ks.iter().min().unwrap_or(&0), ks.iter().max().unwrap_or(&0), union.len(), inter.map_or(0, |i| i.len()));
            for b in 0..crate::hold::BATCHES {
                let row: Vec<String> = r.riders.iter().filter(|s| s.lane == b).map(|s| format!("L{} {}{}", s.level, s.valid.len() - s.obstructed.len().min(s.valid.len()), if s.obstructed.is_empty() { String::new() } else { format!(" ({} under its carts)", s.obstructed.len()) })).collect();
                println!("lane {b}: {} | success per slot: 3 callers {:.3}%, 2 {:.3}%, 1 {:.3}%", row.join(", "), 100.0 * r.success(b, 3), 100.0 * r.success(b, 2), 100.0 * r.success(b, 1));
            }
            println!("stray spots (spawnable, no portal contact): {strays}");
            if verbose || strays <= 12 {
                for s in &r.riders {
                    for c in &s.stray {
                        println!("  lane {} level {}: {c:?}", s.lane, s.level);
                    }
                }
            }
            let touching = r.near_portal.iter().filter(|(_, g)| *g <= 0.0).count();
            println!("boxes within a cell of a portal: {} ({touching} touching one)", r.near_portal.len());
            for (n, g) in r.near_portal.iter().take(12) {
                println!("  {n}: {g:.3} from the nearest portal cell");
            }
            println!("held riders' searches (every batch at its hold): {} spawnable spots{}", r.held_strays.len(), if r.held_strays.is_empty() { String::new() } else { format!(" {:?}", r.held_strays.iter().take(12).collect::<Vec<_>>()) });
            let ok = strays == 0 && touching == 0 && r.held_strays.is_empty();
            println!("{}{}", if ok { "PASS" } else { "FAIL" }, if ok && !r.near_portal.is_empty() { " (with boxes under a cell from a portal: the contract's margin)" } else { "" });
            if !ok {
                std::process::exit(1);
            }
        }
        MobfarmCmd::Assembly { x0, x1, counter_at, wired, out } => {
            let p: Vec<i32> = counter_at.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(p.len() == 3, "--counter-at x,y,z");
            let pad = crate::station::PadOpts { x0, x1, ..Default::default() };
            let o = crate::assembly::AsmOpts { counter_at: [p[0], p[1], p[2]], wired: false };
            let text = if wired { crate::assembly::v0b(&pad, &crate::hold::LaneOpts::default())? } else { crate::assembly::v0a(&pad, &crate::hold::LaneOpts::default(), &o)? };
            std::fs::write(&out, text)?;
            println!("{}", out.display());
        }
        MobfarmCmd::AssemblyCheck { design, slots, miss, seed, tolerance } => {
            let (c, spec) = load(version, &design)?;
            let lanes = crate::hold::lanes_of(&c.clock.design.meta)?;
            let pad = crate::station::PadOpts::of(&c.clock.design.meta)?;
            let asm = crate::assembly::AsmOpts::of(&c.clock.design.meta)?.ok_or_else(|| anyhow::anyhow!("not an assembly design (no `assembly:` line)"))?;
            let r = crate::assembly::wiring_check(&spec, &lanes, &pad, &asm, slots, miss, seed, tolerance).map_err(|e| anyhow::anyhow!(e))?;
            println!("{} slots, {} golem pulses ({:.1}% missed), {} advances ({} out of the cycle); ticks with 2+ lines high: {}", r.slots, r.pulses, 100.0 * (1.0 - r.pulses as f64 / r.slots.max(1) as f64), r.advances, r.order_faults, r.two_lines);
            for (k, (lo, hi, n)) in &r.delays {
                println!("  {k}: delay {lo}..{hi} gt over {n} changes");
            }
            println!("crosstalk: {} {:?}", r.crosstalk_count, r.crosstalk);
            println!("stuck: {} {:?}", r.stuck_count, r.stuck);
            let ends = if asm.wired { lanes.levels as usize + 1 } else { 2 * lanes.levels as usize };
            let ok = r.crosstalk_count == 0 && r.stuck_count == 0 && r.two_lines == 0 && r.order_faults == 0 && r.delays.len() == ends;
            println!("{}", if ok { "PASS" } else { "FAIL" });
        }
        MobfarmCmd::Linear { x0, x1, stand, along_z, pitch, drop, out, unsettled, alone, slab_floor } => {
            let p: Vec<i32> = stand.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(p.len() == 3, "--stand x,y,z");
            let pad = crate::station::PadOpts { x0, x1, slabs: crate::station::parse_cells(&slab_floor)?, ..Default::default() };
            let o = crate::linear::LinearOpts { x0: p[0], y: p[1], z: p[2], along_x: !along_z, pitch, drop, ..Default::default() };
            std::fs::write(&out, crate::linear::design(&pad, &o)?)?;
            if !unsettled {
                let changes = crate::placing::settle_file(&out)?;
                println!("settled: {} cells written as the settle leaves them", changes.len());
                for c in &changes {
                    println!("  {c}");
                }
            }
            if alone {
                let text = std::fs::read_to_string(&out)?;
                std::fs::write(&out, crate::linear::alone(&text, &pad)?)?;
                let d = mctas::design::Design::parse(&std::fs::read_to_string(&out)?)?;
                println!("alone: {} pad blocks, {} Nether blocks, the golem pulse at {:?}", d.blocks.len(), d.nether_blocks.len(), d.ports.get("output.pulse"));
            }
            println!("{}", out.display());
        }
        MobfarmCmd::LinearCheck { design } => {
            let (c, spec) = load(version, &design)?;
            let pad = crate::station::PadOpts::of(&c.clock.design.meta)?;
            let o = crate::linear::LinearOpts::of(&c.clock.design.meta)?;
            let r = crate::linear::check(&spec, &pad, &o).map_err(|e| anyhow::anyhow!(e))?;
            println!("pad: {} blocks ({} obsidian, {} portal cells), box x {}..{} y {}..{} z {}..{}", r.pad_blocks, r.obsidian, r.portal_cells, r.pad_box.0[0], r.pad_box.1[0], r.pad_box.0[1], r.pad_box.1[1], r.pad_box.0[2], r.pad_box.1[2]);
            let (clo, chi) = c.bounds();
            println!("the whole bench design: {} overworld blocks, box x {}..{} y {}..{} z {}..{} (with its far hold)", c.clock.design.blocks.len(), clo[0], chi[0], clo[1], chi[1], clo[2], chi[2]);
            let mut strays = 0;
            for s in &r.riders {
                let k = s.valid.len() - s.obstructed.len().min(s.valid.len());
                strays += s.stray.len();
                println!("rider {} at {:?}: {k} valid spots{}, {} strays", s.level, s.cell, if s.obstructed.is_empty() { String::new() } else { format!(" ({} under the batch's carts)", s.obstructed.len()) }, s.stray.len());
                for c in s.stray.iter().take(8) {
                    println!("  stray {c:?}");
                }
            }
            println!("success per slot: 3 callers {:.3}%, 2 {:.3}%, 1 {:.3}%", 100.0 * r.success(0, 3), 100.0 * r.success(0, 2), 100.0 * r.success(0, 1));
            let touching = r.near_portal.iter().filter(|(_, g)| *g <= 0.0).count();
            println!("boxes within a cell of a portal: {} ({touching} touching one)", r.near_portal.len());
            for (n, g) in r.near_portal.iter().take(8) {
                println!("  {n}: {g:.3}");
            }
            println!("held riders' searches: {} spawnable spots", r.held_strays.len());
            let ok = strays == 0 && touching == 0 && r.held_strays.is_empty();
            println!("{}", if ok { "PASS" } else { "FAIL" });
        }
        MobfarmCmd::LinearBench { design, from, to, days, lead, jobs, drive_out, placed_raw, placed_in_order, placed_all, build_gap_sweep } => {
            let (c, spec) = load(version, &design)?;
            let pad = crate::station::PadOpts::of(&c.clock.design.meta)?;
            let o = crate::linear::LinearOpts::of(&c.clock.design.meta)?;
            let plan = crate::placing::BuildPlan::of(&c.clock.design.meta)?;
            // (its declared states and flights, `crate::declared` / `crate::flight`: the brief's
            // "No derails"; a design without the lines declares nothing)
            let declared = crate::declared::parse(c.clock.design.meta.get("declared").map(String::as_str).unwrap_or("")).map_err(|e| anyhow::anyhow!(e))?;
            let flights = crate::flight::parse(c.clock.design.meta.get("flights").map(String::as_str).unwrap_or("")).map_err(|e| anyhow::anyhow!(e))?;
            println!("declared: {} states, {} flights{}", declared.len(), flights.len(), if c.clock.design.meta.contains_key("declared") { "" } else { " (the design has no `declared:` line)" });
            let st = crate::linear::check(&spec, &pad, &o).map_err(|e| anyhow::anyhow!(e))?;
            let seeds: Vec<i64> = (from..=to).collect();
            let jobs = jobs.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8)).max(1);
            let t0 = std::time::Instant::now();
            let run_seeds = |placement: crate::placing::Placement, drive: bool| -> Vec<(i64, Result<crate::linear::BenchReport, String>)> {
                let next = std::sync::atomic::AtomicUsize::new(0);
                let out: std::sync::Mutex<Vec<(i64, Result<crate::linear::BenchReport, String>)>> = std::sync::Mutex::new(Vec::new());
                std::thread::scope(|sc| {
                    for _ in 0..jobs.min(seeds.len()) {
                        sc.spawn(|| loop {
                            let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(&seed) = seeds.get(k) else { break };
                            let r = crate::linear::bench(&spec, &pad, &o, &crate::linear::BenchOpts { seed, days, lead, drive: drive && seed == from, placement, plan: plan.clone(), declared: declared.clone(), flights: flights.clone() });
                            out.lock().unwrap().push((seed, r));
                        });
                    }
                });
                let mut rows = out.into_inner().unwrap();
                rows.sort_by_key(|r| r.0);
                rows
            };
            let modes = crate::placing::Placement::modes(placed_raw, placed_in_order, placed_all, build_gap_sweep);
            if modes.len() > 1 {
                // one line per placement (and per seed with a problem); the sweep stops at the first
                // gap that fails
                let mut all_clean = true;
                for m in &modes {
                    let rows = run_seeds(*m, false);
                    let bad: Vec<(i64, String)> = rows.iter().filter_map(|(s, r)| linear_problem(r).map(|p| (*s, p))).collect();
                    let (slots, summoned) = rows.iter().filter_map(|r| r.1.as_ref().ok()).fold((0u64, 0u64), |a, r| (a.0 + r.slots as u64, a.1 + r.summoned as u64));
                    let build = rows.iter().find_map(|r| r.1.as_ref().ok().and_then(|r| r.build)).map(|(stp, cl, t)| format!(" (build: {cl} cells in {stp} steps, {t} gt)")).unwrap_or_default();
                    println!("{}: {} of {} seeds clean over {days} day(s) each; a golem in {summoned} of {slots} slots ({:.2}%){build}{}", m.label(), rows.len() - bad.len(), rows.len(), 100.0 * summoned as f64 / slots.max(1) as f64, if bad.is_empty() { String::new() } else { format!("; problems on seeds {:?}", bad.iter().map(|b| b.0).collect::<Vec<_>>()) });
                    for (s, p) in bad.iter().take(4) {
                        println!("  seed {s}: {p}");
                    }
                    all_clean &= bad.is_empty();
                    if build_gap_sweep && !bad.is_empty() {
                        if let crate::placing::Placement::Built { gap: Some(g) } = m {
                            println!("built in order: the first gap that fails is {g} gt");
                        }
                        break;
                    }
                }
                if placed_all {
                    println!("{}", if all_clean { "works every way placed" } else { "FAILS in at least one placement" });
                }
                if build_gap_sweep && all_clean {
                    println!("built in order: every gap from 1 to {} gt passes", placed_in_order.filter(|g| *g > 0).unwrap_or(40));
                }
                println!("{:.1} s", t0.elapsed().as_secs_f64());
                return Ok(());
            }
            let rows = run_seeds(modes[0], drive_out.is_some());
            let bad: Vec<(i64, String)> = rows.iter().filter_map(|(s, r)| linear_problem(r).map(|p| (*s, p))).collect();
            println!("{}: {} of {} seeds clean{}", modes[0].label(), rows.len() - bad.len(), rows.len(), rows.iter().find_map(|r| r.1.as_ref().ok().and_then(|r| r.build)).map(|(stp, cl, t)| format!(" (build: {cl} cells in {stp} steps, {t} gt)")).unwrap_or_default());
            for (s, p) in bad.iter().take(6) {
                println!("  seed {s}: {p}");
            }
            // (the first seed's build: every cell it left unlike the file)
            for l in rows.first().and_then(|r| r.1.as_ref().ok()).map(|r| r.built_diff.clone()).unwrap_or_default().iter().take(100) {
                println!("  built unlike the file: {l}");
            }
            let (mut slots, mut summoned, mut viol, mut spurious, mut deaths, mut alive, mut back) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let (mut willing, mut panicking, mut tries, mut failed) = ([0u64; 4], [0u64; 4], [0u64; 4], [0u64; 4]);
            let mut dwell: std::collections::BTreeMap<i64, u64> = Default::default();
            let mut pdelay: std::collections::BTreeMap<Option<i64>, u64> = Default::default();
            let mut plen: std::collections::BTreeMap<i64, u64> = Default::default();
            let (mut predicted, mut strays) = (0.0, 0usize);
            for (seed, r) in &rows {
                if let (Ok(crate::linear::BenchReport { drive: Some(text), .. }), Some(f)) = (r, &drive_out) {
                    std::fs::write(f, text)?;
                    println!("seed {seed}'s drive: {} ({} lines)", f.display(), text.lines().count());
                }
                match r {
                    Err(e) => println!("seed {seed}: ERROR {e}"),
                    Ok(r) => {
                        slots += r.slots as u64;
                        summoned += r.summoned as u64;
                        viol += r.violation_count as u64;
                        spurious += r.spurious_pulses as u64;
                        deaths += r.nether_deaths as u64;
                        alive += r.nether_alive as u64;
                        back += r.returned as u64;
                        strays += r.stray.len();
                        for &w in &r.willing {
                            willing[(w as usize).min(3)] += 1;
                            predicted += st.success(0, w as usize);
                        }
                        for &p in &r.panicking {
                            panicking[(p as usize).min(3)] += 1;
                        }
                        for (j, &t) in r.tries.iter().enumerate() {
                            tries[(t as usize).min(3)] += 1;
                            if !r.ok.get(j).copied().unwrap_or(false) {
                                failed[(t as usize).min(3)] += 1;
                            }
                        }
                        for &d in &r.dwell {
                            *dwell.entry(d).or_insert(0) += 1;
                        }
                        for &(d, l) in &r.pulse {
                            *pdelay.entry(d).or_insert(0) += 1;
                            if d.is_some() {
                                *plen.entry(l).or_insert(0) += 1;
                            }
                        }
                        if !r.violations.is_empty() || !r.errors.is_empty() {
                            println!("seed {seed}: {} violation(s): {}; errors: {}", r.violation_count, r.violations.join("; "), r.errors.join("; "));
                        }
                    }
                }
            }
            println!("{} seeds x {days} day(s): {slots} slots, a golem in {summoned} ({:.3}%; the static check's odds at each slot's willing count: {:.3}%)", rows.len(), 100.0 * summoned as f64 / slots.max(1) as f64, 100.0 * predicted / slots.max(1) as f64);
            println!("riders wanting a golem at the slot (0/1/2/3): {willing:?}; panicking (0/1/2/3): {panicking:?}; tries per slot (0/1/2/3): {tries:?}, of which no golem: {failed:?}");
            println!("golems' overworld stay (gt: count): {dwell:?}; pulse delay from the spawn (gt: count; None = no pulse): {pdelay:?}; length {plen:?}; pulses without a golem: {spurious}");
            println!("golems touching no portal: {strays}; Nether: {deaths} dead, {alive} alive at the end, {back} back in the overworld");
            println!("violations: {viol}; {:.1} s", t0.elapsed().as_secs_f64());
        }
        MobfarmCmd::Siding { out, wired, alone, exit_brake, cage_only, give_up, unsettled } => {
            let g = match (wired, give_up.as_str()) {
                (false, _) | (_, "none") => crate::siding::GiveUp::None,
                (true, "timer") => crate::siding::GiveUp::Timer,
                (true, "scheduler") => crate::siding::GiveUp::Scheduler,
                (_, other) => anyhow::bail!("--give-up {other}: scheduler, timer or none"),
            };
            std::fs::write(&out, crate::siding::design_full(wired, alone, exit_brake, cage_only, g)?)?;
            if !unsettled {
                let changes = crate::placing::settle_file(&out)?;
                println!("settled: {} cells written as the settle leaves them", changes.len());
                for c in &changes {
                    println!("  {c}");
                }
            }
            println!("{}", out.display());
        }
        MobfarmCmd::SidingCheck { design } => {
            let (_c, spec) = load(version, &design)?;
            for l in crate::siding::check(&spec).map_err(|e| anyhow::anyhow!(e))? {
                println!("{l}");
            }
        }
        MobfarmCmd::JunctionCheck { design } => {
            let (_c, spec) = load(version, &design)?;
            let (text, failed) = crate::queue::junction_gate(&spec, &std::fs::read_to_string(&design)?);
            println!("{text}");
            if failed {
                std::process::exit(1);
            }
        }
        MobfarmCmd::FleeCheck { design, home, show, foot, clearing, eye_control, pickup_control, sampled } => {
            let (_c, spec) = load(version, &design)?;
            // (the bed: --home, else the design's `port home:` line)
            let text = std::fs::read_to_string(&design)?;
            let port = text.lines().find_map(|l| l.strip_prefix("port home:")).map(|r| r.split_whitespace().filter_map(|t| t.parse::<i32>().ok()).collect::<Vec<_>>());
            let h = if home.len() == 3 { Some(home) } else { port.filter(|v| v.len() == 3) };
            let bed = h.map(|v| redsim::pos::BlockPos::new(v[0], v[1], v[2]));
            let foot: Vec<redsim::pos::BlockPos> = foot.chunks(3).filter(|c| c.len() == 3).map(|c| redsim::pos::BlockPos::new(c[0], c[1], c[2])).collect();
            // (the design's `panic:` boxes: where its parts rely on panic, ironspec Nov 11 10:54)
            let panic_boxes = match text.lines().find_map(|l| l.strip_prefix("panic:")) {
                Some(s) => crate::declared::parse_boxes(s).map_err(|e| anyhow::anyhow!("panic: {e}"))?,
                None => Vec::new(),
            };
            let places = crate::fleecheck::check_with(&spec, bed, &foot, &panic_boxes, !sampled).map_err(|e| anyhow::anyhow!(e))?;
            let jobs = crate::fleecheck::job_and_bell_blocks(&spec);
            for l in crate::fleecheck::report_with(&places, &jobs, show, clearing) {
                println!("{l}");
            }
            // (the rider eyes: the suffocation gate and the blind places, ironspec Nov 11 11:53)
            let (n, hits) = crate::fleecheck::eye_check(&spec, eye_control).map_err(|e| anyhow::anyhow!(e))?;
            let (lines, fails) = crate::fleecheck::eye_lines(n, &hits, show);
            for l in &lines {
                println!("{l}");
            }
            println!("the suffocation gate{}: {}", if eye_control { " (with the positive control's stone)" } else { "" }, if fails { "FAIL" } else { "PASS" });
            // (the pickup reach, ironspec Nov 11 14:32: zombies and boats out of empty carts'
            // pickup boxes; a default gate)
            let (plines, pfails) = crate::fleecheck::pickup_check(&spec, pickup_control);
            for l in plines.iter().take(1 + if show > 0 { plines.len() } else { 0 }) {
                println!("{l}");
            }
            println!("the pickup gate{}: {}", if pickup_control { " (with the positive control's post)" } else { "" }, if pfails { "FAIL" } else { "PASS" });
        }
        MobfarmCmd::SidingBench { design, from, to, nights, gap, speed, release, jobs, visits, wired, trace, trace_window, launch, drive_out, placed_raw, placed_in_order, placed_all, build_gap_sweep, release_held, fail_no_home, day_visits, force_at, no_force, drop_force, fault, unload, unload_shift, divert_gate, v_reset } => {
            let (c, spec) = load(version, &design)?;
            let plan = crate::placing::BuildPlan::of(&c.clock.design.meta)?;
            let two_i = |s: &str| -> Result<(i64, i64)> {
                let v: Vec<i64> = s.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 2, "two numbers a,b");
                Ok((v[0], v[1]))
            };
            let force = if no_force { None } else { Some(two_i(&force_at)?) };
            let fault_opt: Option<(String, usize)> = match &fault {
                Some(f) => {
                    let (k, n) = f.split_once(':').ok_or_else(|| anyhow::anyhow!("--fault KIND:K"))?;
                    anyhow::ensure!(["arr-miss", "arr-extra", "r-miss", "r-extra", "f-double"].contains(&k), "--fault: arr-miss, arr-extra, r-miss, r-extra or f-double");
                    Some((k.to_string(), n.parse()?))
                }
                None => None,
            };
            // (the partial unload: one, or every line, side, level, state and N; `whole,STATE,N[,K]`:
            // the whole part saved and unloaded)
            const STATES: [&str; 12] = ["sent", "arrival", "arriving", "drop", "nap", "asleep", "waking", "fpending", "kick", "reboard", "exit", "none"];
            let unload_one: Option<crate::siding::Unload> = match unload.as_deref() {
                Some(u) if u.starts_with("whole,") => {
                    let p: Vec<&str> = u.split(',').map(|x| x.trim()).collect();
                    anyhow::ensure!(p.len() == 3 || p.len() == 4, "--unload whole,STATE,N[,K]");
                    anyhow::ensure!(STATES.contains(&p[1]), "--unload STATE: one of {STATES:?}");
                    Some(crate::siding::Unload { forced: [0, 0, -1, -1], drop: [0, 0, -1, -1], state: p[1].to_string(), n: p[2].parse()?, visit: p.get(3).map(|k| k.parse()).transpose()?.unwrap_or(3), whole: true })
                }
                Some(u) if !u.contains("matrix") => {
                    let p: Vec<&str> = u.split(',').map(|x| x.trim()).collect();
                    anyhow::ensure!(p.len() == 5 || p.len() == 6, "--unload LINE,SIDE,LEVEL,STATE,N[,K]");
                    let (forced, drop) = crate::siding::unload_boxes(p[0], p[1], p[2].parse()?).map_err(|e| anyhow::anyhow!(e))?;
                    anyhow::ensure!(STATES.contains(&p[3]), "--unload STATE: one of {STATES:?}");
                    Some(crate::siding::Unload { forced, drop, state: p[3].to_string(), n: p[4].parse()?, visit: p.get(5).map(|k| k.parse()).transpose()?.unwrap_or(3), whole: false })
                }
                _ => None,
            };
            // (the world's shift: the split placement's lines x and z at (2, 0), the one-chunk
            // placement's w and e at (8, 3), none for the whole part; `--unload-shift` overrides)
            let shift_of = |u: &str| -> (i32, i32) {
                if u.starts_with("whole") {
                    (0, 0)
                } else if u.starts_with("chunk-matrix") || u.starts_with("w,") || u.starts_with("e,") {
                    (8, 3)
                } else {
                    (2, 0)
                }
            };
            let (shift_x, shift_z) = match (&unload_shift, unload.as_deref()) {
                (Some(s), _) => two_i(s).map(|(a, b)| (a as i32, b as i32))?,
                (None, Some(u)) => shift_of(u),
                (None, None) => (0, 0),
            };
            let sp: Vec<f64> = speed.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(sp.len() == 2, "--speed a,b");
            let (gap, release) = (two_i(&gap)?, two_i(&release)?);
            let seeds: Vec<i64> = (from..=to).collect();
            let jobs = jobs.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8)).max(1);
            let t0 = std::time::Instant::now();
            let tw = two_i(&trace_window).unwrap_or((0, 4000));
            let run_seeds = |placement: crate::placing::Placement, drive: bool| -> Vec<(i64, Result<crate::siding::BenchReport, String>)> {
                let next = std::sync::atomic::AtomicUsize::new(0);
                let out: std::sync::Mutex<Vec<(i64, Result<crate::siding::BenchReport, String>)>> = std::sync::Mutex::new(Vec::new());
                std::thread::scope(|sc| {
                    for _ in 0..jobs.min(seeds.len()) {
                        sc.spawn(|| loop {
                            let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(&seed) = seeds.get(k) else { break };
                            let r = crate::siding::bench(&spec, &crate::siding::BenchOpts { seed, nights, gap, speed: (sp[0], sp[1]), release, trace: trace && seed == from, max_visits: visits, wired, trace_window: tw, launch, drive: drive && seed == from, placement, plan: plan.clone(), release_held, fail_no_home, day_visits, force, drop_force, fault: fault_opt.clone(), shift_x, shift_z, unload: unload_one.clone(), divert_gate, v_reset });
                            out.lock().unwrap().push((seed, r));
                        });
                    }
                });
                let mut rows = out.into_inner().unwrap();
                rows.sort_by_key(|r| r.0);
                rows
            };
            // (the partial unload matrix: every line, side, level, state and N on visit K, each
            // config over the seeds; one line each: the visits until a clean one, naps lost, jams)
            let matrix_mode = unload.as_deref().and_then(|u| {
                for (p, m) in [("whole-matrix", 1u8), ("chunk-matrix", 2), ("matrix", 0)] {
                    if let Some(rest) = u.strip_prefix(p) {
                        return Some((m, rest.to_string()));
                    }
                }
                None
            });
            if let Some((mode, m)) = matrix_mode {
                let whole = mode == 1;
                let k: usize = m.strip_prefix(':').map(|x| x.parse()).transpose()?.unwrap_or(3);
                let ns: Vec<i64> = if mode == 2 { vec![3, 40, 240] } else { vec![3, 40, 240, 700] };
                let control_forced = if mode == 2 { [-1, 0, 1, 0] } else { [-1, -1, 0, 0] };
                let mut cfgs: Vec<(String, crate::siding::Unload)> = vec![(if whole { "control (nothing unloaded)" } else { "control (the forced box only)" }.into(), crate::siding::Unload { forced: control_forced, drop: [0, 0, -1, -1], state: "none".into(), n: 0, visit: k, whole })];
                if whole {
                    for state in ["arriving", "drop", "asleep", "waking", "fpending", "kick", "reboard", "exit"] {
                        for &n in &ns {
                            cfgs.push((format!("whole {state} N={n}"), crate::siding::Unload { forced: [0, 0, -1, -1], drop: [0, 0, -1, -1], state: state.into(), n, visit: k, whole }));
                        }
                    }
                } else {
                    let (lines, states): (&[&str], &[&str]) = if mode == 2 { (&["w", "e"], &["sent", "arrival", "nap", "waking", "fpending", "kick", "reboard", "exit"]) } else { (&["x", "z"], &["arrival", "nap", "waking", "fpending", "kick", "reboard", "exit"]) };
                    for &line in lines {
                        for side in ["lo", "hi"] {
                            for level in [32u8, 33] {
                                for &state in states {
                                    for &n in &ns {
                                        let Ok((forced, drop)) = crate::siding::unload_boxes(line, side, level) else { continue };
                                        cfgs.push((format!("{line} {side} {level} {state} N={n}"), crate::siding::Unload { forced, drop, state: state.into(), n, visit: k, whole }));
                                    }
                                }
                            }
                        }
                    }
                }
                let (mx, mz) = match &unload_shift {
                    Some(s) => two_i(s).map(|(a, b)| (a as i32, b as i32))?,
                    None => [(2, 0), (0, 0), (8, 3)][mode as usize],
                };
                let work: Vec<(usize, i64)> = (0..cfgs.len()).flat_map(|c| seeds.iter().map(move |&s| (c, s))).collect();
                let next = std::sync::atomic::AtomicUsize::new(0);
                let out: std::sync::Mutex<Vec<(usize, i64, Result<crate::siding::BenchReport, String>)>> = std::sync::Mutex::new(Vec::new());
                std::thread::scope(|sc| {
                    for _ in 0..jobs.min(work.len()) {
                        sc.spawn(|| loop {
                            let j = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(&(c, seed)) = work.get(j) else { break };
                            let r = crate::siding::bench(&spec, &crate::siding::BenchOpts { seed, nights, gap, speed: (sp[0], sp[1]), release, trace: false, max_visits: visits, wired, trace_window: tw, launch, drive: false, placement: crate::placing::Placement::Judge, plan: plan.clone(), release_held, fail_no_home, day_visits, force, drop_force, fault: None, shift_x: mx, shift_z: mz, unload: Some(cfgs[c].1.clone()), divert_gate, v_reset });
                            out.lock().unwrap().push((c, seed, r));
                        });
                    }
                });
                let out = out.into_inner().unwrap();
                let clean = |v: &crate::siding::VisitRec| v.slept.is_some() && v.own_cart && v.released.is_some() && v.problem.is_none();
                println!("{} on visit {k}, {} seeds each; per config: the visits until a clean one (the worst seed; 0 = the visit itself clean), naps lost in the window (the visit and the 5 after it), carts out without their napper (napped, out past the exit, never re-boarded), jams (a cart still inside 2000 gt or not out 400 gt after its release), unclean visits after the window, when the unload hit (gt after the arrival), model errors", if whole { "whole-part unload (every chunk saved and unloaded, back as new entities)".to_string() } else if mode == 2 { format!("partial unload at the one-chunk placement's junction lines (the world shifted {mx},{mz}: the siding in chunk 0,0; w the approach's line at the siding's x -9|-8, e the run-out's at 7|8)") } else { format!("partial unload, the world shifted {mx},{mz} (the loop's chunk lines at the siding's x -3|-2 and z -1|0)") }, seeds.len());
                for (c, (name, u)) in cfgs.iter().enumerate() {
                    let (mut worst, mut lost, mut jams, mut later, mut missed, mut left_without) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
                    let mut merged = 0usize;
                    let mut hit: Vec<i64> = Vec::new();
                    let mut errs: Vec<String> = Vec::new();
                    for (_, seed, r) in out.iter().filter(|x| x.0 == c) {
                        match r {
                            Err(e) => errs.push(format!("seed {seed}: {e}")),
                            Ok(r) => {
                                if let Some(e) = r.errors.first() {
                                    errs.push(format!("seed {seed}: {e}"));
                                }
                                match (r.unload, r.visits.get(k)) {
                                    (Some((at, _)), Some(v)) => hit.push(at - v.arrived),
                                    _ if u.state != "none" => missed += 1,
                                    _ => {}
                                }
                                let after = &r.visits[k.min(r.visits.len())..];
                                let window = &after[..after.len().min(6)];
                                worst = worst.max(window.iter().rposition(|v| !clean(v)).map_or(0, |i| i + 1));
                                lost += window.iter().filter(|v| v.slept.is_none()).count();
                                jams += r.visits.iter().filter(|v| v.problem.as_ref().is_some_and(|p| p.contains("still inside") || p.contains("not out"))).count();
                                later += after.iter().skip(6).filter(|v| !clean(v)).count();
                                left_without += window.iter().filter(|v| v.slept.is_some() && v.exited.is_some() && v.reboarded.is_none()).count();
                                merged += r.merged.len();
                            }
                        }
                    }
                    hit.sort();
                    let hit_s = if hit.is_empty() { "never".to_string() } else { format!("{}..{}", hit[0], hit[hit.len() - 1]) };
                    println!("  {name}: until clean {worst}; naps lost {lost}; out without the napper {left_without}; jams {jams}; merged carts {merged}; unclean after {later}; hit +{hit_s}{}{}", if missed > 0 { format!(" (not hit on {missed} seeds)") } else { String::new() }, if errs.is_empty() { String::new() } else { format!("; errors: {}", errs[0]) });
                }
                return Ok(());
            }
            let modes = crate::placing::Placement::modes(placed_raw, placed_in_order, placed_all, build_gap_sweep);
            if modes.len() > 1 {
                // one line per placement (and per seed with a problem); the sweep stops at the first
                // gap that fails
                let mut all_clean = true;
                for m in &modes {
                    let rows = run_seeds(*m, false);
                    let bad: Vec<(i64, String)> = rows.iter().filter_map(|(s, r)| siding_problem(r).map(|p| (*s, p))).collect();
                    let n_visits: usize = rows.iter().filter_map(|r| r.1.as_ref().ok()).map(|r| r.visits.iter().filter(|v| v.released.is_some() || v.problem.is_some()).count()).sum();
                    let build = rows.iter().find_map(|r| r.1.as_ref().ok().and_then(|r| r.build)).map(|(st, c, t)| format!(" (build: {c} cells in {st} steps, {t} gt)")).unwrap_or_default();
                    println!("{}: {} of {} seeds clean over {nights} night(s) each, {n_visits} visits{build}{}", m.label(), rows.len() - bad.len(), rows.len(), if bad.is_empty() { String::new() } else { format!("; problems on seeds {:?}", bad.iter().map(|b| b.0).collect::<Vec<_>>()) });
                    for (s, p) in bad.iter().take(4) {
                        println!("  seed {s}: {p}");
                    }
                    all_clean &= bad.is_empty();
                    if build_gap_sweep && !bad.is_empty() {
                        if let crate::placing::Placement::Built { gap: Some(g) } = m {
                            println!("built in order: the first gap that fails is {g} gt");
                        }
                        break;
                    }
                }
                if placed_all {
                    println!("{}", if all_clean { "works every way placed" } else { "FAILS in at least one placement" });
                }
                if build_gap_sweep && all_clean {
                    println!("built in order: every gap from 1 to {} gt passes", placed_in_order.filter(|g| *g > 0).unwrap_or(40));
                }
                println!("{:.1} s", t0.elapsed().as_secs_f64());
                return Ok(());
            }
            let rows = run_seeds(modes[0], drive_out.is_some());
            let bad: Vec<(i64, String)> = rows.iter().filter_map(|(s, r)| siding_problem(r).map(|p| (*s, p))).collect();
            println!("{}: {} of {} seeds clean{}", modes[0].label(), rows.len() - bad.len(), rows.len(), rows.iter().find_map(|r| r.1.as_ref().ok().and_then(|r| r.build)).map(|(st, c, t)| format!(" (build: {c} cells in {st} steps, {t} gt)")).unwrap_or_default());
            for (s, p) in bad.iter().take(6) {
                println!("  seed {s}: {p}");
            }
            // (the first seed's build: every cell it left unlike the file)
            for l in rows.first().and_then(|r| r.1.as_ref().ok()).map(|r| r.built_diff.clone()).unwrap_or_default().iter().take(100) {
                println!("  built unlike the file: {l}");
            }
            let (mut visits, mut dropped, mut slept, mut woken, mut reboarded, mut own, mut ready, mut released) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let (mut wait_ticks, mut wait_panic) = (0u64, 0u64);
            let mut ready_t: Vec<i64> = Vec::new();
            let mut stages: Vec<(i64, i64, i64, i64)> = Vec::new();
            // (each napped visit's segments: arrival to drop, drop to sleep, the nap, wake to
            // re-board, re-board to ready on the slot, and the whole ready time)
            let mut segs: Vec<[i64; 6]> = Vec::new();
            let mut seg_who: Vec<(i64, u32, i64)> = Vec::new();
            let mut speeds: Vec<f64> = Vec::new();
            let mut leaves: Vec<f64> = Vec::new();
            let mut problems: std::collections::BTreeMap<String, u64> = Default::default();
            let mut examples: Vec<String> = Vec::new();
            for (seed, r) in &rows {
                match r {
                    Err(e) => println!("seed {seed}: ERROR {e}"),
                    Ok(r) => {
                        for v in &r.visits {
                            // (a visit the run's end cut short is not counted)
                            if v.released.is_none() && v.problem.is_none() {
                                continue;
                            }
                            visits += 1;
                            dropped += v.dropped.is_some() as u64;
                            slept += v.slept.is_some() as u64;
                            woken += v.woken.is_some() as u64;
                            reboarded += v.reboarded.is_some() as u64;
                            own += v.own_cart as u64;
                            ready += v.ready.is_some() as u64;
                            released += v.released.is_some() as u64;
                            wait_ticks += v.wait_ticks as u64;
                            wait_panic += v.wait_panic as u64;
                            if let Some(rt) = v.ready {
                                ready_t.push(rt - v.arrived);
                            }
                            if let (Some(d), Some(s), Some(w), Some(b)) = (v.dropped, v.slept, v.woken, v.reboarded) {
                                stages.push((d - v.arrived, s - v.arrived, w - v.arrived, b - v.arrived));
                                if let Some(r) = v.ready {
                                    segs.push([d - v.arrived, s - d, w - s, b - w, r - b, r - v.arrived]);
                                    seg_who.push((*seed, v.villager, v.arrived));
                                }
                            }
                            if let Some(e) = v.exit_speed {
                                speeds.push(e);
                            }
                            if let Some(e) = v.leave_speed {
                                leaves.push(e);
                            }
                            if let Some(p) = &v.problem {
                                let key: String = p.split(" (").next().unwrap_or(p).to_string();
                                *problems.entry(key).or_insert(0) += 1;
                                if examples.len() < 6 {
                                    examples.push(format!("seed {seed} villager {} arrived {}: {p}; dropped {:?} slept {:?} woken {:?} reboarded {:?} ready {:?}", v.villager, v.arrived, v.dropped.map(|x| x - v.arrived), v.slept.map(|x| x - v.arrived), v.woken.map(|x| x - v.arrived), v.reboarded.map(|x| x - v.arrived), v.ready.map(|x| x - v.arrived)));
                                }
                            }
                        }
                        if !r.errors.is_empty() {
                            println!("seed {seed}: errors {}", r.errors.join("; "));
                        }
                    }
                }
            }
            ready_t.sort();
            speeds.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let pct = |v: &[i64], q: f64| v.get(((v.len() as f64 - 1.0) * q).round() as usize).copied();
            println!("{} seeds x {nights} nights: {visits} visits; dropped {dropped}, slept {slept}, woken {woken}, re-boarded {reboarded} (own cart {own}), ready on the slot {ready}, released {released}", rows.len());
            println!("ready after the arrival (gt): min {:?} median {:?} p99 {:?} max {:?}; over 500: {}", ready_t.first(), pct(&ready_t, 0.5), pct(&ready_t, 0.99), ready_t.last(), ready_t.iter().filter(|&&r| r > 500).count());
            if !segs.is_empty() {
                // (what the ready time is made of: each segment's p50/p90/p99/max, and the mean
                // segments of the slowest 1% of visits against all)
                let names = ["arrival to drop", "drop to asleep", "nap", "woken to re-boarded", "re-boarded to ready", "ready (all)"];
                let mut cols: Vec<String> = Vec::new();
                for (i, n) in names.iter().enumerate() {
                    let mut v: Vec<i64> = segs.iter().map(|s| s[i]).collect();
                    v.sort();
                    cols.push(format!("{n} {:?}/{:?}/{:?}/{:?}", pct(&v, 0.5).unwrap_or(0), pct(&v, 0.9).unwrap_or(0), pct(&v, 0.99).unwrap_or(0), v.last().copied().unwrap_or(0)));
                }
                println!("segments, p50/p90/p99/max gt ({} napped visits): {}", segs.len(), cols.join("; "));
                let mut by_ready = segs.clone();
                by_ready.sort_by_key(|s| s[5]);
                let tail = &by_ready[by_ready.len() - (by_ready.len() / 100).max(1)..];
                let mean = |v: &[[i64; 6]], i: usize| v.iter().map(|s| s[i] as f64).sum::<f64>() / v.len().max(1) as f64;
                println!("the slowest {} visits (ready {}..{} gt), mean segments vs all: {}", tail.len(), tail[0][5], tail[tail.len() - 1][5], (0..5).map(|i| format!("{} {:.1} vs {:.1}", names[i], mean(tail, i), mean(&segs, i))).collect::<Vec<_>>().join("; "));
                let mut order: Vec<usize> = (0..segs.len()).collect();
                order.sort_by_key(|&i| std::cmp::Reverse(segs[i][5]));
                for &i in order.iter().take(3) {
                    println!("  slow: seed {} villager {} arrived at time {}: segments {:?}", seg_who[i].0, seg_who[i].1, seg_who[i].2, segs[i]);
                }
            }
            if !stages.is_empty() {
                let n = stages.len() as f64;
                let mean = |f: &dyn Fn(&(i64, i64, i64, i64)) -> i64| stages.iter().map(|s| f(s) as f64).sum::<f64>() / n;
                println!("mean stage ticks after the arrival: dropped {:.1}, asleep {:.1}, woken {:.1}, re-boarded {:.1}", mean(&|s| s.0), mean(&|s| s.1), mean(&|s| s.2), mean(&|s| s.3));
                let mut naps: Vec<i64> = stages.iter().map(|s| s.2 - s.1).collect();
                naps.sort();
                let mut drop_to_board: Vec<i64> = stages.iter().map(|s| s.3 - s.0).collect();
                drop_to_board.sort();
                println!("nap length (asleep to woken, gt): min {:?} median {:?} p99 {:?} max {:?}; drop to re-board: min {:?} (the boarding cooldown is 60)", naps.first(), pct(&naps, 0.5), pct(&naps, 0.99), naps.last(), drop_to_board.first());
            }
            println!("panicking while waiting on the slot at night: {wait_panic} of {wait_ticks} ticks ({:.2}%)", 100.0 * wait_panic as f64 / wait_ticks.max(1) as f64);
            {
                let all: Vec<&crate::siding::VisitRec> = rows.iter().filter_map(|r| r.1.as_ref().ok()).flat_map(|r| r.visits.iter()).collect();
                let calm: Vec<(i64, i64)> = all.iter().filter_map(|v| v.calm).collect();
                let mut firsts: Vec<i64> = calm.iter().map(|c| c.0).collect();
                firsts.sort();
                let mut lasts: Vec<i64> = calm.iter().map(|c| c.1).collect();
                lasts.sort();
                println!("visits with calm waiting ticks: {} of {}; first calm tick after ready: min {:?} median {:?} max {:?}; last: median {:?} max {:?}", calm.len(), all.len(), firsts.first(), firsts.get(firsts.len() / 2), firsts.last(), lasts.get(lasts.len() / 2), lasts.last());
                let mut xs: std::collections::BTreeMap<String, u32> = Default::default();
                for v in &all {
                    if let Some(x) = v.rest_x {
                        *xs.entry(format!("{:.1}", x)).or_insert(0) += 1;
                    }
                }
                println!("rest x on the slot: {xs:?}");
                let long: Vec<String> = rows.iter().filter_map(|r| r.1.as_ref().ok().map(|x| (r.0, x))).flat_map(|(seed, r)| r.visits.iter().map(move |v| (seed, v))).filter(|(_, v)| v.calm.is_some_and(|c| c.1 > 20)).take(8).map(|(seed, v)| format!("seed {seed} villager {} arrived {} rest x {:.3} sees {:?} calm {:?} ({} of {} waiting ticks panicking) {:?}", v.villager, v.arrived, v.rest_x.unwrap_or(f64::NAN), v.sees, v.calm, v.wait_panic, v.wait_ticks, v.calm_acts)).collect();
                for l in long {
                    println!("  long calm: {l}");
                }
                let mut by_x: std::collections::BTreeMap<String, (u32, u32)> = Default::default();
                for v in &all {
                    if let (Some(x), Some((s, _, _))) = (v.rest_x, v.sees) {
                        let e = by_x.entry(format!("{:.1}", x)).or_insert((0, 0));
                        e.0 += s as u32;
                        e.1 += 1;
                    }
                }
                println!("rest x: (sees the dorm zombie, of): {by_x:?}");
            }
            if !speeds.is_empty() {
                println!("exit speed at the exit cell (b/t, stored): min {:.3} median {:.3} max {:.3}", speeds[0], speeds[speeds.len() / 2], speeds[speeds.len() - 1]);
            }
            if !leaves.is_empty() {
                leaves.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let over = leaves.iter().filter(|&&v| v > 0.36).count();
                println!("speed leaving the exit cell (b/t, stored): min {:.3} median {:.3} max {:.3}; over 0.36: {over} of {}", leaves[0], leaves[leaves.len() / 2], leaves[leaves.len() - 1], leaves.len());
            }
            println!("problems: {problems:?}");
            // (ironspec's hard line: merged carts, over every seed: ironfarm's shared check)
            {
                let oks: Vec<&crate::siding::BenchReport> = rows.iter().filter_map(|r| r.1.as_ref().ok()).collect();
                let n: usize = oks.iter().map(|r| r.merged.len()).sum();
                let first: Vec<String> = rows.iter().filter_map(|(s, r)| r.as_ref().ok().and_then(|r| r.merged_first.first().map(|f| format!("seed {s}: {f}")))).take(3).collect();
                println!("merged carts (horizontal distance under 0.01, boxes paired by the push; the game never separates them): {n} {}", first.join("; "));
                // (the divert gate, emulated: the visits it held back and the resets VR fired)
                if divert_gate {
                    let held: usize = oks.iter().map(|r| r.held_back.len()).sum();
                    let vr: usize = oks.iter().map(|r| r.v_resets.len()).sum();
                    let longest = oks.iter().map(|r| {
                        // (the longest run of consecutive held-back slots, in slots)
                        let mut best = 0usize;
                        let mut cur = 0usize;
                        let mut last = i64::MIN;
                        for &h in &r.held_back {
                            cur = if h - last <= 650 { cur + 1 } else { 1 };
                            best = best.max(cur);
                            last = h;
                        }
                        best
                    }).max().unwrap_or(0);
                    println!("the divert gate (V or the bed occupied, emulated): {held} scheduled visits held back over the seeds, the longest run {longest} in a row; reset pulses VR fired: {vr}");
                }
            }
            // (the force input: when the bench pulsed it, and for the visits whose villager had
            // not slept by then, the pulse to ready on the slot)
            {
                let oks: Vec<&crate::siding::BenchReport> = rows.iter().filter_map(|r| r.1.as_ref().ok()).collect();
                let forced: Vec<&crate::siding::VisitRec> = oks.iter().flat_map(|r| r.visits.iter()).filter(|v| v.forced.is_some()).collect();
                if !forced.is_empty() {
                    let mut after: Vec<i64> = forced.iter().map(|v| v.forced.unwrap() - v.arrived).collect();
                    after.sort();
                    let mut to_ready: Vec<i64> = forced.iter().filter(|v| v.slept.is_none_or(|s| s > v.forced.unwrap())).filter_map(|v| v.ready.map(|r| r - v.forced.unwrap())).collect();
                    to_ready.sort();
                    let cut = forced.iter().filter(|v| !v.failed && v.slept.is_none()).count();
                    println!("force input (day-1's scheduler, driven by the bench): {} pulses, {}..{} gt after the arrival; visits not asleep by it: {} (pulse to ready on the slot: min {:?} median {:?} max {:?} gt); normal visits it cut short: {cut}", forced.len(), after[0], after[after.len() - 1], to_ready.len(), to_ready.first(), to_ready.get(to_ready.len() / 2), to_ready.last());
                    // (F's tolerance, ironspec: the napped visits' sleep onset after the arrival,
                    // the earliest F's bound; and the visits F sent out, the pulse to leaving the
                    // exit, the latest F's)
                    let mut onset: Vec<i64> = oks.iter().flat_map(|r| r.visits.iter()).filter_map(|v| v.slept.map(|s| s - v.arrived)).collect();
                    onset.sort();
                    if !onset.is_empty() {
                        let q = |p: f64| onset[((onset.len() as f64 - 1.0) * p) as usize];
                        let over = |x: i64| onset.iter().filter(|&&o| o > x).count();
                        println!("  sleep onset after the arrival ({} napped visits): p50 {} p99 {} p99.9 {} p99.99 {} max {} gt; over 60: {}, over 80: {}, over 100: {}; the 8 slowest: {:?}", onset.len(), q(0.5), q(0.99), q(0.999), q(0.9999), onset[onset.len() - 1], over(60), over(80), over(100), &onset[onset.len().saturating_sub(8)..]);
                    }
                    let mut to_exit: Vec<i64> = forced.iter().filter(|v| v.slept.is_none_or(|s| s > v.forced.unwrap())).filter_map(|v| v.exited.map(|x| x - v.forced.unwrap())).collect();
                    to_exit.sort();
                    if !to_exit.is_empty() {
                        println!("  sent out by F: the pulse to leaving the exit cell: min {} median {} max {} gt ({} visits)", to_exit[0], to_exit[to_exit.len() / 2], to_exit[to_exit.len() - 1], to_exit.len());
                    }
                    // (the siding's T: back to entry-bed after each visit, against the arrival or
                    // F, and against the cart coming onto the exit branch; the cart's x then)
                    let all: Vec<&crate::siding::VisitRec> = oks.iter().flat_map(|r| r.visits.iter()).collect();
                    let span = |mut v: Vec<i64>| -> String {
                        v.sort();
                        if v.is_empty() { "none".into() } else { format!("{}..{} ({})", v[0], v[v.len() - 1], v.len()) }
                    };
                    let by_f = |v: &crate::siding::VisitRec| v.forced.is_some_and(|f| v.slept.is_none_or(|s| s > f));
                    let nap_back: Vec<i64> = all.iter().filter(|v| !by_f(v) && v.slept.is_some()).filter_map(|v| v.tee_back.map(|(t, _)| t - v.arrived)).collect();
                    let f_back: Vec<i64> = all.iter().filter(|v| by_f(v)).filter_map(|v| v.tee_back.map(|(t, _)| t - v.forced.unwrap())).collect();
                    let clear: Vec<i64> = all.iter().filter_map(|v| Some(v.tee_back?.0 - v.branch?)).collect();
                    let xmin = all.iter().filter_map(|v| v.tee_back.map(|b| b.1)).filter(|x| x.is_finite()).fold(f64::INFINITY, f64::min);
                    println!("  the siding's T back to entry-bed: {} gt after a nap visit's arrival, {} gt after F on a visit sent out by F; {} gt after its cart came onto the exit branch, the cart then at x {:.2} or more (the T's cell x 1..2)", span(nap_back), span(f_back), span(clear), xmin);
                    // (design (b)'s napper-aboard sensor, when the design has it)
                    let seats: Vec<&crate::siding::SeatLog> = oks.iter().filter_map(|r| r.seat.as_ref()).collect();
                    if !seats.is_empty() {
                        let sum = |f: fn(&crate::siding::SeatLog) -> u32| seats.iter().map(|s| f(s)).sum::<u32>();
                        println!("  the napper-aboard sensor at (1,2,1) (a tripwire, or a stone plate on the booster's glass): on {} times with a seated villager in its band, {} times without one; seated villagers in its band with it off: {} ticks; on {} ticks; ticks with a villager standing under it or on the free cell beside it: {}", sum(|s| s.on_seated), sum(|s| s.on_other), sum(|s| s.missed), sum(|s| s.on_ticks), sum(|s| s.seen_standing));
                        for e in seats.iter().flat_map(|s| s.examples.iter()).take(8) {
                            println!("    {e}");
                        }
                        let tops: Vec<(f64, f64)> = seats.iter().filter_map(|s| s.seated_top).collect();
                        let feet: Vec<(f64, f64)> = seats.iter().filter_map(|s| s.seated_feet).collect();
                        if !tops.is_empty() {
                            println!("    seated villagers anywhere: box top {:.3}..{:.3}, feet {:.3}..{:.3} above their cart's position", tops.iter().map(|x| x.0).fold(f64::INFINITY, f64::min), tops.iter().map(|x| x.1).fold(f64::NEG_INFINITY, f64::max), feet.iter().map(|x| x.0).fold(f64::INFINITY, f64::min), feet.iter().map(|x| x.1).fold(f64::NEG_INFINITY, f64::max));
                        }
                    }
                }
                // (the fault injection at a stateful element: the faulted visit, the next ones,
                // and the visits until one is clean again, per seed)
                if let Some((kind, fk)) = &fault_opt {
                    let clean = |v: &crate::siding::VisitRec| v.slept.is_some() && v.own_cart && v.released.is_some() && v.problem.is_none();
                    let mut worst = 0usize;
                    let mut lost_naps = 0usize;
                    for (_seed, r) in rows.iter().filter_map(|(s, r)| r.as_ref().ok().map(|r| (s, r))) {
                        // (the window: the faulted visit and the 5 after it; recovered = the
                        // first clean visit after the last unclean one in the window)
                        let after = &r.visits[(*fk).min(r.visits.len())..];
                        let window = &after[..after.len().min(6)];
                        let last_bad = window.iter().rposition(|v| !clean(v)).map_or(0, |i| i + 1);
                        worst = worst.max(last_bad);
                        lost_naps += window.iter().filter(|v| v.slept.is_none()).count();
                    }
                    println!("fault {kind} on visit {fk}: visits until a clean one (napped, own cart, released), worst over the seeds: {worst} (0 = the faulted visit itself clean); naps lost in those: {lost_naps}");
                    for (seed, r) in rows.iter().filter_map(|(s, r)| r.as_ref().ok().map(|r| (s, r))).take(4) {
                        for (j, v) in r.visits.iter().enumerate().skip(*fk).take(3) {
                            println!("  seed {seed} visit {j} (+{}): V before {:?}; slept {:?} re-boarded {:?} own cart {} ready {:?} released {:?} F {:?}{}", v.arrived - r.visits[*fk].arrived, v.v_before, v.slept.map(|x| x - v.arrived), v.reboarded.map(|x| x - v.arrived), v.own_cart, v.ready.map(|x| x - v.arrived), v.released.map(|x| x - v.arrived), v.forced.map(|x| x - v.arrived), v.problem.as_ref().map(|p| format!("; problem: {p}")).unwrap_or_default());
                        }
                    }
                }
                // (the fault injection: the dropped visit and the next ones, per seed; first a
                // one-line summary over every seed)
                {
                    let clean = |v: &crate::siding::VisitRec| v.slept.is_some() && v.own_cart && v.released.is_some() && v.problem.is_none();
                    let mut lines = Vec::new();
                    for (seed, r) in rows.iter().filter_map(|(s, r)| r.as_ref().ok().map(|r| (s, r))) {
                        let Some(k) = r.visits.iter().position(|v| v.force_dropped) else { continue };
                        let d = &r.visits[k];
                        let out = d.exited.map(|x| x - d.arrived);
                        let after = &r.visits[k + 1..];
                        let window = &after[..after.len().min(5)];
                        let last_bad = window.iter().rposition(|v| !clean(v)).map_or(0, |i| i + 1);
                        let later_bad = after.iter().skip(5).filter(|v| !clean(v)).count();
                        lines.push(format!("seed {seed}: stuck cart out {}; unclean visits after it: the next {last_bad} (later: {later_bad})", out.map_or("never".to_string(), |x| format!("{x} gt after its arrival"))));
                    }
                    if !lines.is_empty() {
                        println!("drop-force summary:\n  {}", lines.join("\n  "));
                    }
                }
                for (seed, r) in rows.iter().filter_map(|(s, r)| r.as_ref().ok().map(|r| (s, r))).take(8) {
                    let Some(k) = r.visits.iter().position(|v| v.force_dropped) else { continue };
                    let d = &r.visits[k];
                    let fate = if d.slept.is_some() {
                        format!("the napper slept anyway (the fault did not take); ready {:?} released {:?}", d.ready.map(|x| x - d.arrived), d.released.map(|x| x - d.arrived))
                    } else {
                        match (d.unstuck, d.exited) {
                            (Some(u), Some(x)) => format!("its cart came onto the exit branch {} gt after its arrival, left past the exit {} gt after (ready on the slot: {:?}, released {:?}; with its villager: {})", u - d.arrived, x - d.arrived, d.ready.map(|x| x - d.arrived), d.released.map(|x| x - d.arrived), d.ready.is_some()),
                            (Some(u), None) => format!("its cart came onto the exit branch {} gt after its arrival and never left past the exit (ready on the slot: {:?}, released {:?})", u - d.arrived, d.ready.map(|x| x - d.arrived), d.released.map(|x| x - d.arrived)),
                            (None, _) => format!("its cart stayed in the siding to the end of the run ({} gt)", r.visits.last().map_or(0, |v| v.arrived - d.arrived)),
                        }
                    };
                    println!("drop-force seed {seed}: visit {k} (villager {}, arrived {}): napper dropped {:?} slept {:?}; {fate}", d.villager, d.arrived, d.dropped.map(|x| x - d.arrived), d.slept.map(|x| x - d.arrived));
                    for (j, v) in r.visits.iter().enumerate().skip(k + 1).take(4) {
                        println!("  next visit {j} (villager {}, +{} gt): V lit before it {:?}; dropped {:?} slept {:?} re-boarded {:?} own cart {} ready {:?} released {:?} F {:?}{}", v.villager, v.arrived - d.arrived, v.v_before, v.dropped.map(|x| x - v.arrived), v.slept.map(|x| x - v.arrived), v.reboarded.map(|x| x - v.arrived), v.own_cart, v.ready.map(|x| x - v.arrived), v.released.map(|x| x - v.arrived), v.forced.map(|x| x - v.arrived), v.problem.as_ref().map(|p| format!("; problem: {p}")).unwrap_or_default());
                    }
                    let later = &r.visits[(k + 5).min(r.visits.len())..];
                    let ok = later.iter().filter(|v| v.slept.is_some() && v.own_cart && v.released.is_some()).count();
                    let inverted = later.iter().filter(|v| v.v_before == Some(true)).count();
                    println!("  the {} visits after those: {ok} napped, back in their own cart and released; V lit before {inverted} of them (the bulb's phase inverted)", later.len());
                }
            }
            // ironspec's check, every seed: HOME at every dawn and midday, naps per villager-night
            {
                let oks: Vec<&crate::siding::BenchReport> = rows.iter().filter_map(|r| r.1.as_ref().ok()).collect();
                let checks: usize = oks.iter().map(|r| r.home_checks.len()).sum();
                let min_held = oks.iter().flat_map(|r| r.home_checks.iter().map(|c| c.2)).min();
                let short: usize = oks.iter().map(|r| r.home_checks.iter().filter(|c| c.2 < 21).count()).sum();
                let changes: usize = oks.iter().map(|r| r.home_events.len()).sum();
                let mut naps: std::collections::BTreeMap<usize, u64> = Default::default();
                for r in &oks {
                    let mut per: std::collections::BTreeMap<(i64, u32), usize> = Default::default();
                    for v in r.visits.iter().filter(|v| v.slept.is_some()) {
                        *per.entry((v.night, v.villager)).or_insert(0) += 1;
                    }
                    for n in per.values() {
                        *naps.entry(*n).or_insert(0) += 1;
                    }
                }
                let nights: usize = oks.iter().map(|r| r.visits.iter().map(|v| v.night).collect::<std::collections::BTreeSet<_>>().len()).sum();
                println!("ironspec's check: {checks} dawn/midday checks over {} seeds, HOME = the bed held by min {:?} of 21 (checks short of 21: {short}); HOME changes (losses and regains): {changes}; villager-nights by naps (naps: count): {naps:?} over {nights} seed-nights (21 each: {})", oks.len(), min_held, 21 * nights);
                let mut gaps: Vec<i64> = oks.iter().flat_map(|r| r.home_gaps.iter().copied()).collect();
                gaps.sort();
                let unregained: usize = oks.iter().map(|r| r.home_events.iter().filter(|e| e.contains("loses")).count()).sum::<usize>().saturating_sub(gaps.len());
                println!("HOME lost and taken back (the napper's release at its first REST tick and its AcquirePoi reclaim): {} times, without HOME p50 {:?} p99 {:?} max {:?} gt; losses not taken back by the end: {unregained}", gaps.len(), gaps.get(gaps.len() / 2), gaps.get(((gaps.len() as f64 - 1.0) * 0.99) as usize), gaps.last());
                for e in oks.iter().flat_map(|r| r.home_events.iter()).take(8) {
                    println!("  HOME change: {e}");
                }
            }
            for e in &examples {
                println!("  {e}");
            }
            if let Some(path) = &drive_out {
                let text = rows.iter().find(|r| r.0 == from).and_then(|r| r.1.as_ref().ok()).and_then(|r| r.drive.clone()).ok_or_else(|| anyhow::anyhow!("no drive from seed {from}"))?;
                std::fs::write(path, &text)?;
                println!("drive: {} ({} acts, {} expectations)", path.display(), text.lines().filter(|l| l.starts_with("act ")).count(), text.lines().filter(|l| l.starts_with("expect")).count());
            }
            println!("{:.1} s", t0.elapsed().as_secs_f64());
        }
        MobfarmCmd::StationStorage { hours, seed, success, official } => {
            let lines = if official { crate::station::storage_official(version, hours, seed, success)? } else { crate::station::storage_bench(hours, seed, success).map_err(|e| anyhow::anyhow!(e))? };
            for l in lines {
                println!("{l}");
            }
        }
        MobfarmCmd::StationBench { design, from, to, days, jitter, jobs, timeline, hold_delay, stand_delay, verbose, drive_out } => {
            let (c, spec) = load(version, &design)?;
            let lanes = crate::hold::lanes_of(&c.clock.design.meta)?;
            let pad = crate::station::PadOpts::of(&c.clock.design.meta)?;
            let static_check = crate::station::check(&spec, &lanes, &pad).map_err(|e| anyhow::anyhow!(e))?;
            let three = |s: &str| -> Result<[i64; 3]> {
                let v: Vec<i64> = s.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "three delays (levels 0,1,2)");
                Ok([v[0], v[1], v[2]])
            };
            let (hold_delay, stand_delay) = (three(&hold_delay)?, three(&stand_delay)?);
            // an assembly design: its counter in the loop
            let asm = crate::assembly::AsmOpts::of(&c.clock.design.meta)?;
            let wired = asm.is_some_and(|a| a.wired);
            let counter = match asm {
                Some(a) => Some(crate::assembly::counter_taps(&a)?),
                None => None,
            };
            let seeds: Vec<i64> = (from..=to).collect();
            let jobs = jobs.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8)).max(1);
            let next = std::sync::atomic::AtomicUsize::new(0);
            let out: std::sync::Mutex<Vec<(i64, Result<crate::station::BenchReport, String>)>> = std::sync::Mutex::new(Vec::new());
            let t0 = std::time::Instant::now();
            std::thread::scope(|sc| {
                for _ in 0..jobs.min(seeds.len()) {
                    sc.spawn(|| loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&seed) = seeds.get(k) else { break };
                        let r = crate::station::bench(&spec, &lanes, &pad, &crate::station::BenchOpts { seed, days, jitter, verbose: verbose && seed == from, timeline: if seed == from { timeline.clone() } else { None }, counter: counter.clone(), hold_delay, stand_delay, wired, drive: drive_out.is_some() && seed == from });
                        out.lock().unwrap().push((seed, r));
                    });
                }
            });
            let mut rows = out.into_inner().unwrap();
            rows.sort_by_key(|r| r.0);
            let (mut slots, mut summoned, mut lingering, mut viol, mut spurious, mut piglins, mut crossed) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let (mut n_alive, mut back, mut items_back, mut iron_chest, mut iron_hoppers, mut iron_loose) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let (mut iron_dropped, mut full_at): (u64, Vec<i64>) = (0, Vec::new());
            let (mut iron_burnt, mut poppies) = (0u64, 0u64);
            let (mut piglins_out, mut piglins_alive) = (0u64, 0u64);
            let mut n_life: std::collections::BTreeMap<i64, u64> = Default::default();
            let mut adv_per_slot = [0u64; 4];
            let mut adv_delay: std::collections::BTreeMap<i64, u64> = Default::default();
            let mut line_faults = 0u64;
            let mut startup = 0u64;
            let mut gaps: std::collections::BTreeMap<i64, u64> = Default::default();
            let mut margins: std::collections::BTreeMap<Option<i64>, u64> = Default::default();
            let mut willing = [0u64; 4];
            let mut tries = [0u64; 4];
            let mut failed_at = [0u64; 4];
            let mut panicking = [0u64; 4];
            let mut dwell: std::collections::BTreeMap<i64, u64> = Default::default();
            let mut pulse_delay: std::collections::BTreeMap<Option<i64>, u64> = Default::default();
            let mut pulse_len: std::collections::BTreeMap<i64, u64> = Default::default();
            let mut predicted = 0.0;
            let mut strays: Vec<String> = Vec::new();
            for (seed, r) in &rows {
                if let (Ok(crate::station::BenchReport { drive: Some(text), .. }), Some(f)) = (r, &drive_out) {
                    std::fs::write(f, text)?;
                    println!("seed {seed}'s drive: {} ({} lines)", f.display(), text.lines().count());
                }
                if let Ok(crate::station::BenchReport { timeline: Some((page, summary)), .. }) = r {
                    println!("seed {seed}'s timing view: {page}\n  {summary}");
                }
                match r {
                    Err(e) => println!("seed {seed}: ERROR {e}"),
                    Ok(r) => {
                        slots += r.slots as u64;
                        summoned += r.summoned as u64;
                        lingering += r.lingering as u64;
                        viol += r.violation_count as u64;
                        spurious += r.spurious_pulses as u64;
                        piglins += r.piglins as u64;
                        crossed += r.piglins_crossed as u64;
                        piglins_out += r.piglins_out as u64;
                        piglins_alive += r.piglins_alive as u64;
                        n_alive += r.nether_alive as u64;
                        back += r.returned as u64;
                        items_back += r.items_back as u64;
                        iron_chest += r.iron_chest as u64;
                        iron_hoppers += r.iron_hoppers as u64;
                        iron_loose += r.iron_loose as u64;
                        iron_dropped += r.iron_dropped as u64;
                        iron_burnt += r.iron_burnt as u64;
                        poppies += r.poppies_dropped as u64;
                        if let Some(f) = r.chest_full_at {
                            full_at.push(f);
                        }
                        for &l in &r.nether_life {
                            *n_life.entry(l / 10 * 10).or_insert(0) += 1;
                        }
                        for (n, &w) in r.willing.iter().enumerate() {
                            willing[(w as usize).min(3)] += 1;
                            let lane = r.slot_lanes.get(n).map_or((n % crate::hold::BATCHES as usize) as i32, |&l| l as i32);
                            predicted += static_check.success(lane, w as usize);
                        }
                        // the counter in the loop: advances between consecutive slots, and each
                        // advance's delay after the slot before it
                        if !r.advances.is_empty() {
                            line_faults += r.line_faults as u64;
                            startup += r.startup_golems as u64;
                            for &g in &r.gaps {
                                *gaps.entry(g).or_insert(0) += 1;
                            }
                            for m in &r.margins {
                                *margins.entry(*m).or_insert(0) += 1;
                            }
                            let slot_ticks: Vec<i64> = r.slot_ticks.clone();
                            for w in slot_ticks.windows(2) {
                                let n = r.advances.iter().filter(|&&a| a > w[0] && a <= w[1]).count();
                                adv_per_slot[n.min(3)] += 1;
                            }
                            for &a in &r.advances {
                                if let Some(&s) = slot_ticks.iter().rev().find(|&&s| s < a) {
                                    *adv_delay.entry(a - s).or_insert(0) += 1;
                                }
                            }
                        }
                        for (j, &t) in r.tries.iter().enumerate() {
                            tries[(t as usize).min(3)] += 1;
                            if !r.ok.get(j).copied().unwrap_or(false) {
                                failed_at[(t as usize).min(3)] += 1;
                            }
                        }
                        for &p in &r.panicking {
                            panicking[(p as usize).min(3)] += 1;
                        }
                        for &d in &r.dwell {
                            *dwell.entry(d).or_insert(0) += 1;
                        }
                        for &(d, l) in &r.pulse {
                            *pulse_delay.entry(d).or_insert(0) += 1;
                            if d.is_some() {
                                *pulse_len.entry(l).or_insert(0) += 1;
                            }
                        }
                        strays.extend(r.stray.iter().cloned());
                        if !r.violations.is_empty() || !r.errors.is_empty() {
                            println!("seed {seed}: {} violation(s): {}; errors: {}", r.violation_count, r.violations.join("; "), r.errors.join("; "));
                        }
                    }
                }
            }
            let pct = |a: u64, b: u64| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
            println!("{} seeds x {days} day(s): {slots} slots, a golem in {summoned} ({:.3}%; the static check's odds at each slot's willing count: {:.3}%)", rows.len(), pct(summoned, slots), if slots == 0 { 0.0 } else { 100.0 * predicted / slots as f64 });
            println!("riders wanting a golem at the slot (0/1/2/3): {:?}; tries per slot (0/1/2/3): {:?}, of which no golem: {:?}; riders panicking at the slot (0/1/2/3): {:?}", willing, tries, failed_at, panicking);
            println!("golems' overworld stay (gt: count): {dwell:?}; still there at the end: {lingering}");
            println!("pulse delay from the spawn tick (gt: count; None = no pulse): {pulse_delay:?}; length (gt: count): {pulse_len:?}; pulses without a golem: {spurious}");
            println!("golems touching no portal: {}", strays.len());
            if counter.is_some() {
                let mut delays: Vec<(i64, u64)> = adv_delay.iter().map(|(k, v)| (*k, *v)).collect();
                delays.sort();
                let short: Vec<String> = delays.iter().take(6).map(|(d, n)| format!("{d}: {n}")).collect();
                let long: Vec<String> = delays.iter().rev().take(4).map(|(d, n)| format!("{d}: {n}")).collect();
                println!("counter in the loop: advances between consecutive slots (0/1/2/3+): {adv_per_slot:?}; advance delay after the slot before it (gt: count) shortest {short:?}, longest {long:?}; ticks with 2+ lines high: {line_faults}; no-line gaps (gt: count) {gaps:?}; golems at the counter's power-on (before its rotation settles): {startup}");
                let late = margins.get(&None).copied().unwrap_or(0);
                let ms: Vec<(i64, u64)> = margins.iter().filter_map(|(k, v)| k.map(|k| (k, *v))).collect();
                let min = ms.first().map(|m| m.0);
                let n: u64 = ms.iter().map(|m| m.1).sum();
                let mean = if n == 0 { 0.0 } else { ms.iter().map(|(k, v)| *k as f64 * *v as f64).sum::<f64>() / n as f64 };
                println!("raised batch's arrival margin before its slot: min {min:?} gt, mean {mean:.1}; slots it was not all there: {late}; the 5 smallest (gt: count) {:?}", ms.iter().take(5).collect::<Vec<_>>());
            }
            for s in strays.iter().take(8) {
                println!("  {s}");
            }
            println!("zombified piglins: {piglins} spawned ({:.2} a day), {crossed} crossed, {piglins_out} ever out of the portal cells, {piglins_alive} alive at the end (the rest despawned)", piglins as f64 / (rows.len() as f64 * days as f64));
            let deaths: u64 = n_life.values().sum();
            println!("Nether: {deaths} golems died there, life from arrival (gt, by tens: count) {n_life:?}; {n_alive} alive at the end; golems back in the overworld {back}; items back {items_back}");
            println!("Nether iron: {iron_dropped} dropped ({:.2} a golem that died; {poppies} poppies); {iron_chest} in the chest, {iron_hoppers} in the hoppers, {iron_loose} loose, {iron_burnt} burnt; unaccounted (despawned) {}", if deaths == 0 { 0.0 } else { iron_dropped as f64 / deaths as f64 }, iron_dropped as i64 - (iron_chest + iron_hoppers + iron_loose + iron_burnt) as i64);
            if !full_at.is_empty() {
                full_at.sort();
                println!("the output chest full in {} of {} runs, first after {} gt ({:.1} game days), median {} gt", full_at.len(), rows.len(), full_at[0], full_at[0] as f64 / 24000.0, full_at[full_at.len() / 2]);
            }
            println!("violations: {viol}; {:.1} s", t0.elapsed().as_secs_f64());
        }
        MobfarmCmd::HoldBench { design, from, to, days, jitter, jobs, verbose, timeline, trace } => {
            let (c, spec) = load(version, &design)?;
            let lanes = crate::hold::lanes_of(&c.clock.design.meta)?;
            let seeds: Vec<i64> = (from..=to).collect();
            let jobs = jobs.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8)).max(1);
            let next = std::sync::atomic::AtomicUsize::new(0);
            let out: std::sync::Mutex<Vec<(i64, Result<crate::hold::BenchReport, String>)>> = std::sync::Mutex::new(Vec::new());
            let t0 = std::time::Instant::now();
            std::thread::scope(|sc| {
                for _ in 0..jobs.min(seeds.len()) {
                    sc.spawn(|| loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&seed) = seeds.get(k) else { break };
                        let r = crate::hold::bench(&spec, &lanes, &crate::hold::BenchOpts { seed, days, jitter, verbose, timeline: if seed == from { timeline.clone() } else { None }, trace });
                        out.lock().unwrap().push((seed, r));
                    });
                }
            });
            let mut rows = out.into_inner().unwrap();
            rows.sort_by_key(|r| r.0);
            let (mut clean, mut min_margin, mut margins, mut rest, mut calm, mut off) = (0, i64::MAX, 0.0, 0u64, 0u64, 0u64);
            let mut calm_at: std::collections::BTreeMap<(i32, i32, i32, bool), u64> = Default::default();
            let mut spots: Vec<redsim::pos::BlockPos> = Vec::new();
            let (mut stand_lo, mut stand_hi, mut hold_lo, mut cant) = (f64::MAX, f64::MIN, f64::MAX, 0i64);
            for (seed, r) in &rows {
                if let Ok(crate::hold::BenchReport { timeline: Some((page, summary)), .. }) = r {
                    println!("seed {seed}'s timing view: {page}\n  {summary}");
                }
                match r {
                    Err(e) => println!("seed {seed}: ERROR {e}"),
                    Ok(r) => {
                        if r.violation_count == 0 && r.errors.is_empty() {
                            clean += 1;
                        }
                        if r.violation_count > 0 || !r.errors.is_empty() || verbose {
                            println!("seed {seed}: {} violations in {} slots, margin min {} mean {:.1}, stand z {:.2}..{:.2}, hold z >= {:.2}, rest {} calm {} off-cart {}, errors {:?}", r.violation_count, r.slots, r.min_margin, r.mean_margin, r.stand_z.0, r.stand_z.1, r.hold_min_z, r.rest_ticks, r.calm_ticks, r.off_cart, r.errors);
                            for v in r.violations.iter().take(4) {
                                println!("    {v}");
                            }
                        }
                        min_margin = min_margin.min(r.min_margin);
                        margins += r.mean_margin;
                        rest += r.rest_ticks;
                        calm += r.calm_ticks;
                        off += r.off_cart;
                        cant = cant.max(r.max_cant_reach_at_rest);
                        stand_lo = stand_lo.min(r.stand_z.0);
                        stand_hi = stand_hi.max(r.stand_z.1);
                        hold_lo = hold_lo.min(r.hold_min_z);
                        for s in &r.spawn_spots {
                            if !spots.contains(s) {
                                spots.push(*s);
                            }
                        }
                        for (k, v) in &r.calm_at {
                            *calm_at.entry(*k).or_insert(0u64) += v;
                        }
                    }
                }
            }
            if verbose {
                let mut top: Vec<_> = calm_at.iter().filter(|(k, _)| k.3).collect();
                top.sort_by(|a, b| b.1.cmp(a.1));
                println!("calm at night by cell (x y z: villager-ticks), the worst 24:");
                for ((x, y, z, _), v) in top.iter().take(24) {
                    println!("  {x} {y} {z}: {v}");
                }
                let mut day: Vec<_> = calm_at.iter().filter(|(k, _)| !k.3 && k.2 < 8).collect();
                day.sort_by(|a, b| b.1.cmp(a.1));
                println!("calm by day within 8 of the stand end by cell (x y z: villager-ticks), the worst 24:");
                for ((x, y, z, _), v) in day.iter().take(24) {
                    println!("  {x} {y} {z}: {v}");
                }
            }
            let n = rows.len().max(1) as f64;
            let (lo, hi) = c.bounds();
            println!("{clean} of {} seeds clean over {days} days (jitter 0..={jitter}); arrival margin min {min_margin} gt, mean {:.1}; stand z {stand_lo:.2}..{stand_hi:.2}, hold z >= {hold_lo:.2}; villager-ticks: REST {rest} (oldest CANT_REACH {cant}), calm {calm}, off-cart {off}; golem spawn spots from rider positions: {} {:?}; {} blocks, box {}x{}x{} = {}; {:.0} s on {jobs} threads", rows.len(), margins / n, spots.len(), spots.iter().take(6).collect::<Vec<_>>(), c.clock.design.blocks.len(), hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1, (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1), t0.elapsed().as_secs_f64());
            if lanes.yard {
                let ok: Vec<&crate::hold::BenchReport> = rows.iter().filter_map(|(_, r)| r.as_ref().ok()).collect();
                let mut visits: Vec<i64> = ok.iter().flat_map(|r| r.visits.iter().copied()).collect();
                visits.sort();
                let pct = |p: f64| visits.get(((visits.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0);
                let (nights, misses, home, r16): (u32, u32, u32, u32) = ok.iter().fold((0, 0, 0, 0), |a, r| (a.0 + r.nights, a.1 + r.sleep_misses, a.2 + r.home_misses, a.3 + r.rule16));
                let second: u32 = ok.iter().map(|r| r.second_naps).sum();
                let worst_gap = ok.iter().map(|r| r.worst_gap).max().unwrap_or(0);
                let mut clear: Vec<i64> = ok.iter().flat_map(|r| r.clear.iter().copied()).collect();
                clear.sort();
                let cpct = |p: f64| clear.get(((clear.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0);
                let problems: usize = ok.iter().map(|r| r.nap_problems.len()).sum();
                let holds: u32 = ok.iter().map(|r| r.holds).sum();
                let lost: u32 = ok.iter().map(|r| r.lost_homes).sum();
                let mut retakes: Vec<(i64, bool)> = ok.iter().flat_map(|r| r.retakes.iter().copied()).collect();
                retakes.sort();
                let riding = retakes.iter().filter(|r| r.1).count();
                let rpct = |p: f64| retakes.get(((retakes.len() as f64 - 1.0) * p).round() as usize).map(|r| r.0).unwrap_or(0);
                println!("HOME in the yard: {} released and re-taken ({riding} released riding), re-take min {} / median {} / p95 {} / max {} gt after the release; {lost} never re-taken during the visit; routes the scheduler would hold (another return still in the junctions at the route's time): {holds}", retakes.len(), rpct(0.0), rpct(0.5), rpct(0.95), rpct(1.0));
                println!("sleep routine: {} visits, length min {} / median {} / p95 {} / max {} gt (the junctions clear of the return: median {} / p95 {} / max {} gt after the dispatch); {problems} visit problems; {nights} whole nights, villager-nights without a nap {misses}, second naps {second}; worst interval between a villager's naps {worst_gap} gt; HOME missing {home} times at dawn/midday; REST within 16 of the occupied bed {r16} ticks", visits.len(), pct(0.0), pct(0.5), pct(0.95), pct(1.0), cpct(0.5), cpct(0.95), cpct(1.0));
                for (seed, r) in &rows {
                    let Ok(r) = r else { continue };
                    for p in r.nap_problems.iter().take(3) {
                        println!("  seed {seed}: {p}");
                    }
                    if verbose {
                        for l in &r.nap_log {
                            println!("  seed {seed}: {l}");
                        }
                    }
                }
            }
        }
        MobfarmCmd::Seeds { design, from, to, days, jobs, write } => {
            let (_c, spec) = load(version, &design)?;
            use redsim::entity::mob::farm::{RunOpts, WalkMode, run};
            let seeds: Vec<i64> = (from..=to).collect();
            let jobs = jobs.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8)).max(1);
            let next = std::sync::atomic::AtomicUsize::new(0);
            let out: std::sync::Mutex<Vec<(i64, Option<String>, String)>> = std::sync::Mutex::new(Vec::new());
            let t0 = std::time::Instant::now();
            std::thread::scope(|sc| {
                for _ in 0..jobs.min(seeds.len()) {
                    sc.spawn(|| {
                        loop {
                            let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(&seed) = seeds.get(k) else { break };
                            let o = RunOpts::steady(seed, days * 24000, WalkMode::Random);
                            let row = match run(&spec, &o) {
                                Ok(r) => {
                                    let first = r.failures.iter().chain(r.errors.iter()).next().cloned();
                                    let riding = r.riding_middays.iter().map(|(riding, rode)| format!("{riding}/{rode}")).collect::<Vec<_>>().join(" ");
                                    (seed, first, format!("{} golems, {} iron ({:.0}/h), riding at middays {}", r.golems, r.iron_dropped, r.iron_per_hour(), if riding.is_empty() { "-".to_string() } else { riding }))
                                }
                                Err(e) => (seed, Some(format!("the run went wrong: {e}")), String::new()),
                            };
                            out.lock().unwrap().push(row);
                        }
                    });
                }
            });
            let mut rows = out.into_inner().unwrap();
            rows.sort_by_key(|r| r.0);
            let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            let mut held = Vec::new();
            for (seed, first, line) in &rows {
                match first {
                    None => {
                        held.push(seed.to_string());
                        println!("seed {seed}: PASS, {line}");
                    }
                    Some(f) => {
                        // the kind: the failure's words without its numbers
                        let kind: String = f.split_once(": ").map(|(h, r)| if h.starts_with("gt ") { r } else { f.as_str() }).unwrap_or(f).chars().filter(|c| !c.is_ascii_digit()).collect::<String>().split_whitespace().take(8).collect::<Vec<_>>().join(" ");
                        *kinds.entry(kind).or_insert(0) += 1;
                        println!("seed {seed}: FAIL {f}; {line}");
                    }
                }
            }
            println!("{} of {} seeds hold {days} days ({}); {:.0} s on {jobs} threads", held.len(), rows.len(), held.join(" "), t0.elapsed().as_secs_f64());
            let mut kv: Vec<_> = kinds.into_iter().collect();
            kv.sort_by(|a, b| b.1.cmp(&a.1));
            for (k, n) in kv {
                println!("  first failure {n}x: {k}");
            }
            if write {
                let path = seeds_file(&design);
                write_seeds_row(&path, days, held.len(), rows.len(), &format!("{from}-{to}"))?;
                println!("recorded in {}", path.display());
            }
        }
        MobfarmCmd::Run { design, seed, days, still, terrain, ticks_only, watch, cells, census, build, log, log_box, log_from, log_to, log_moves, natural } => {
            let (c, spec) = load(version, &design)?;
            use redsim::entity::mob::farm::{RunOpts, WalkMode, run};
            let walk = if still { WalkMode::Fixed(redsim::entity::mob::walk::Walk::Still) } else { WalkMode::Random };
            let mut o = RunOpts::steady(seed, days * 24000, walk);
            let spec = match (&terrain, build) {
                (_, Some(k)) => {
                    let sample = circuit::mobfarm::terrain_sample(&c).map_err(|e| anyhow::anyhow!(e))?.ok_or_else(|| anyhow::anyhow!("--build: the design is not judged in terrain"))?;
                    let sites = circuit::mobfarm::terrain_sites(&c, &spec, sample).map_err(|e| anyhow::anyhow!(e))?;
                    let site = *sites.get(k).ok_or_else(|| anyhow::anyhow!("--build {k}: the judge has {} builds", sites.len()))?;
                    println!("terrain {} build {k}: offset {:?}, {} blocks placed + {} filled, {} dug, AFK at {:.1} {:.1} {:.1}", site.sample, site.offset, site.placed, site.fills.len(), site.dug, site.afk.x, site.afk.y, site.afk.z);
                    o = redsim::entity::mob::farm::judge_opts_in(seed, days * 24000, walk, Some(site));
                    site.spec(&spec)
                }
                (Some(name), None) => {
                    let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow::anyhow!(e))?));
                    println!("terrain {}: offset {:?}, {} blocks placed + {} filled, {} dug, AFK at {:.1} {:.1} {:.1}; the cheapest of {} spots (block time {:.0}-{:.0} s)", site.sample, site.offset, site.placed, site.fills.len(), site.dug, site.afk.x, site.afk.y, site.afk.z, site.spots, site.build_range.0, site.build_range.1);
                    o.site = Some(site);
                    site.spec(&spec)
                }
                (None, None) => spec,
            };
            let t0 = std::time::Instant::now();
            if natural && let Some(site) = o.site {
                let w = site.world.read().unwrap_or_else(|e| e.into_inner());
                for (p, dig) in &site.seals {
                    println!("seal {} {} {}: natural {}{}", p.x, p.y, p.z, w.get(*p), if *dig { " (dug first)" } else { "" });
                }
                if log.is_none() && watch.is_empty() {
                    return Ok(());
                }
            }
            if let Some(types) = log {
                let bx = log_box.as_deref().map(crate::entlog::parse_box).transpose().map_err(|e| anyhow::anyhow!(e))?;
                let lo = crate::entlog::LogOpts { types: types.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(), bx, from: log_from, to: log_to, moves: log_moves };
                return crate::entlog::log_run(&spec, &o, days * 24000, &lo).map_err(|e| anyhow::anyhow!(e));
            }
            if !watch.is_empty() {
                let cells: Vec<redsim::BlockPos> = watch
                    .iter()
                    .map(|s| {
                        let v: Vec<i32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                        if v.len() == 3 { Ok(redsim::BlockPos::new(v[0], v[1], v[2])) } else { Err(anyhow::anyhow!("--watch {s}: expected x,y,z")) }
                    })
                    .collect::<Result<_>>()?;
                if natural && let Some(site) = o.site {
                    let w = site.world.read().unwrap_or_else(|e| e.into_inner());
                    for c in &cells {
                        let what = if site.seals.iter().any(|(p, _)| p == c) {
                            " (the site seals it: cobblestone)"
                        } else if site.fills.contains(c) {
                            " (the site caps it)"
                        } else if site.clears.iter().any(|(p, _)| p == c) {
                            " (the site clears it)"
                        } else {
                            ""
                        };
                        println!("natural {} {} {}: {}{what}", c.x, c.y, c.z, w.get(*c));
                    }
                }
                let mut r = redsim::entity::mob::farm::Run::new(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
                let mut last: Vec<String> = cells.iter().map(|c| r.world.get(*c).to_string()).collect();
                let mut sleeping: Vec<(u32, bool)> = Vec::new();
                for (c, s) in cells.iter().zip(&last) {
                    println!("t {} day {}: {} {} {}: {s}", r.world.time, r.world.day_time().rem_euclid(24000), c.x, c.y, c.z);
                }
                for _ in 0..days * 24000 {
                    r.tick();
                    let (t, day) = (r.world.time, r.world.day_time().rem_euclid(24000));
                    for (k, c) in cells.iter().enumerate() {
                        let s = r.world.get(*c).to_string();
                        if s != last[k] {
                            println!("t {t} day {day}: {} {} {}: {s}", c.x, c.y, c.z);
                            last[k] = s;
                        }
                    }
                    for e in &r.world.entities.list {
                        if let redsim::entity::EntityKind::Mob(m) = &e.kind
                            && m.ty == redsim::entity::mob::types::MobType::Villager
                        {
                            let asleep = m.sleeping_pos.is_some();
                            match sleeping.iter_mut().find(|(id, _)| *id == e.id) {
                                Some((_, was)) if *was != asleep => {
                                    println!("t {t} day {day}: villager {} at {:.1} {:.1} {} ", e.id, e.pos.x, e.pos.z, if asleep { "lies down" } else { "wakes" });
                                    *was = asleep;
                                }
                                Some(_) => {}
                                None => sleeping.push((e.id, asleep)),
                            }
                        }
                    }
                }
                return Ok(());
            }
            if ticks_only {
                let mut r = redsim::entity::mob::farm::Run::new(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
                let built = t0.elapsed().as_secs_f64();
                for _ in 0..days * 24000 {
                    r.tick();
                }
                println!("built and settled in {built:.1} s; {days} game day(s) ticked in {:.1} s", t0.elapsed().as_secs_f64() - built);
                return Ok(());
            }
            if census {
                let mut r = redsim::entity::mob::farm::Run::new(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
                let built = t0.elapsed().as_secs_f64();
                let mut peak: std::collections::BTreeMap<String, usize> = Default::default();
                let mut sum: std::collections::BTreeMap<String, usize> = Default::default();
                for t in 0..days * 24000 {
                    r.tick();
                    let mut now: std::collections::BTreeMap<String, usize> = Default::default();
                    for e in &r.world.entities.list {
                        if let redsim::entity::EntityKind::Mob(m) = &e.kind
                            && !e.removed
                            && !m.dead
                        {
                            *now.entry(m.ty.def().id.to_string()).or_insert(0) += 1;
                        }
                    }
                    for (k, n) in &now {
                        let p = peak.entry(k.clone()).or_insert(0);
                        *p = (*p).max(*n);
                        if t % 1000 == 0 {
                            *sum.entry(k.clone()).or_insert(0) += n;
                        }
                    }
                }
                let mut v: Vec<_> = sum.iter().collect();
                v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                for (k, s) in v {
                    println!("{k}: peak {} alive, {s} summed over the 1000-gt samples", peak[k]);
                }
                println!("built and settled in {built:.1} s; {days} game day(s) ticked in {:.1} s; model errors: {}", t0.elapsed().as_secs_f64() - built, r.world.errors.join("; "));
                return Ok(());
            }
            let r = run(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
            // (the day time of a spawn: the mob layer starts at day0 - SETTLE_TICKS at game time 0)
            let night = r.spawn_times.iter().filter(|t| (o.day0 - redsim::entity::mob::farm::SETTLE_TICKS + **t).rem_euclid(24000) >= 12000).count();
            println!("golems by day (day time 0..12000) {}, by night {night}", r.spawn_times.len() - night);
            println!("containers at the end: {}", r.contents.join("; "));
            println!(
                "day0 {}: {} golems ({:.1}/h), {} iron ({:.0}/h), {} in the chest, kill {:?} gt, {} tries without a spot, {} sightings, {} sleeps, longest without sleep {} gt, {:.1} s",
                o.day0,
                r.golems,
                r.golems_per_hour(),
                r.iron_dropped,
                r.iron_per_hour(),
                r.iron_in_chest,
                r.lifetimes.iter().take(8).collect::<Vec<_>>(),
                r.losses.no_spot,
                r.losses.seen,
                r.sleeps,
                r.max_sleep_gap,
                t0.elapsed().as_secs_f64()
            );
            for f in r.failures.iter().chain(r.errors.iter()) {
                println!("FAIL {f}");
            }
            // (the portal pad: golems through the Nether portal, the iron burnt on the way, and
            // the zombified piglins its portal spawned)
            if r.crossings > 0 || spec.nether.is_some() {
                println!("the Nether: {} golems through the portal, {} iron burnt", r.crossings, r.iron_burnt);
            }
            if r.piglins_spawned > 0 || spec.nether.is_some() {
                let days = r.ticks as f64 / 24000.0;
                println!("zombified piglins: {} spawned, {} killed, {} through a portal ({:.2} spawned per day)", r.piglins_spawned, r.piglins_killed, r.piglins_crossed, r.piglins_spawned as f64 / days.max(1e-9));
            }
            if !r.riding_middays.is_empty() {
                println!(
                    "riding at each midday (of the villagers that rode): {}; golems summoned by riders {} of {}",
                    r.riding_middays.iter().map(|(riding, rode)| format!("{riding}/{rode}")).collect::<Vec<_>>().join(" "),
                    r.golems_by_riders,
                    r.golems
                );
            }
            if !r.releases.is_empty() {
                println!("POI releases (a villager's bed given up after 1200 gt unreachable) at gt {:?}", r.releases);
            }
            if !r.legacy_ai.is_empty() {
                println!("natural mobs on the legacy walk model (their AI not modeled yet): {}", r.legacy_ai.join(", "));
            }
            if cells > 0 {
                let total: u64 = r.golem_cells.iter().map(|(_, n)| *n as u64).sum();
                let mut v = r.golem_cells.clone();
                v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                println!("golem ticks by cell ({total} in all; the top {cells}):");
                for (c, n) in v.iter().take(cells) {
                    println!("  {} {} {}: {n} ({:.1}%)", c[0], c[1], c[2], 100.0 * *n as f64 / total.max(1) as f64);
                }
            }
        }
        MobfarmCmd::Validate { design, days, kill, seeds, trace, repeat } => validate_cmd(version, &design, days, kill, seeds, trace, repeat)?,
        MobfarmCmd::Exact { designs, record, stale } => {
            if stale {
                let root = std::env::current_dir()?;
                let rows = circuit::mobfarm_exact::read_record(&root);
                let (mut n, mut changed) = (0, 0);
                for r in &rows {
                    n += 1;
                    let now = match circuit::Circuit::load(std::path::Path::new(&r.file)) {
                        Ok(c) => clock::catalog::content_hash(&c.clock),
                        Err(e) => {
                            changed += 1;
                            println!("stale: {} (exact {}): unreadable now: {e:#}", r.file, if r.exact { "yes" } else { "no" });
                            continue;
                        }
                    };
                    if now != r.hash {
                        changed += 1;
                        println!("stale: {} (exact {}, recorded {} as {}, now {now})", r.file, if r.exact { "yes" } else { "no" }, r.recorded, r.hash);
                    }
                }
                println!("{changed} of {n} rows stale ({})", circuit::mobfarm_exact::RECORD);
                return Ok(());
            }
            let o = circuit::verify::Options { version: version.to_string(), ..Default::default() };
            for d in &designs {
                let c = circuit::Circuit::load(d)?;
                let g = circuit::mobfarm_exact::gate(&c, &o, 1, circuit::mobfarm::JUDGE_DAYS)?;
                println!("{}: {} ({:.0} s)", d.display(), g.summary(), g.seconds);
                for r in &g.runs {
                    println!("{}", r.line());
                }
                if record {
                    let r = circuit::mobfarm_exact::write_record(&std::env::current_dir()?, &c, &d.to_string_lossy(), &g)?;
                    println!("  recorded in {}: exact {}", circuit::mobfarm_exact::RECORD, if r.exact { "yes" } else { "no" });
                }
            }
        }
        MobfarmCmd::Probe { design, row, terrain, flammable } => {
            let (c, spec) = load(version, &design)?;
            let mut o = redsim::entity::mob::farm::RunOpts::steady(1, 0, redsim::entity::mob::farm::WalkMode::Random);
            // (`--terrain`: the farm in its sample at the judge's cheapest spot, as `run
            // --terrain` builds it; coordinates stay the design's)
            let (moved, off) = match &terrain {
                Some(name) => {
                    let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow::anyhow!(e))?));
                    o.site = Some(site);
                    (site.spec(&spec), site.offset)
                }
                None => (spec.clone(), [0, 0, 0]),
            };
            let w = redsim::entity::mob::farm::build(&moved, &o).map_err(|e| anyhow::anyhow!(e))?;
            let at = |x: i32, y: i32, z: i32| redsim::BlockPos::new(x + off[0], y + off[1], z + off[2]);
            if flammable {
                // (farmopt 2026-11-11: the flammable blocks in and around the farm, natural or
                // the design's, with their sky: the fire rules' fuel (`lightning::FireReach`))
                let mut n = 0;
                for y in spec.lo.y..=spec.hi.y {
                    for z in spec.lo.z..=spec.hi.z {
                        for x in spec.lo.x..=spec.hi.x {
                            let p = at(x, y, z);
                            let b = w.get(p);
                            if let Some((ig, bu)) = redsim::weather::lightning::flammable(&b) {
                                let sky = redsim::weather::can_see_sky(&w, p.above()).map(|s| if s { "sees the sky" } else { "under cover" }).unwrap_or("sky unknown");
                                let built = spec.blocks.iter().any(|(q, _)| *q == redsim::BlockPos::new(x, y, z));
                                println!("{x} {y} {z}: {b} (ignite {ig}, burn {bu}; {}; {sky})", if built { "the design's" } else { "natural" });
                                n += 1;
                            }
                        }
                    }
                }
                println!("{n} flammable blocks in the farm's box {:?}..{:?}", spec.lo, spec.hi);
                return Ok(());
            }
            anyhow::ensure!(row.len() == 4, "--row takes X0 X1 Y Z (or --flammable)");
            let [x0, x1, y, z] = [row[0], row[1], row[2], row[3]];
            for x in x0..=x1 {
                println!("{x} {y} {z}: {}", w.get(at(x, y, z)));
            }
        }
        MobfarmCmd::Edit { design, chest, how, meta, set } => {
            if !set.is_empty() {
                let mut d = mctas::design::Design::parse(&std::fs::read_to_string(&design)?)?;
                for s in &set {
                    let (c, state) = s.split_once('=').ok_or_else(|| anyhow::anyhow!("--set x,y,z=state, not {s:?}"))?;
                    let v: Vec<i32> = c.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--set x,y,z=state, not {s:?}");
                    let state = state.trim();
                    let full = if state.contains(':') { state.to_string() } else { format!("minecraft:{state}") };
                    if full == "minecraft:air" {
                        d.blocks.remove(&[v[0], v[1], v[2]]);
                    } else {
                        d.blocks.insert([v[0], v[1], v[2]], full);
                    }
                }
                std::fs::write(&design, d.to_text())?;
            }
            if chest || how.is_some() || !meta.is_empty() {
                let text = std::fs::read_to_string(&design)?;
                let out = crate::layouts::edit_meta(&text, chest, how.as_deref(), &meta)?;
                std::fs::write(&design, out)?;
            }
            println!("{}", design.display());
        }
        MobfarmCmd::Sight { design, terrain, spawns } => {
            if spawns {
                spawns_cmd(version, &design, terrain.as_deref())?
            } else {
                sight_cmd(version, &design, terrain.as_deref())?
            }
        }
        MobfarmCmd::Cubes { design } => cubes_cmd(version, &design)?,
        MobfarmCmd::Trim { design, out, threads, days, runs } => crate::trim::trim_cmd(&crate::trim::TrimArgs { design, out, threads, days, runs })?,
        MobfarmCmd::Climb { design } => crate::trim::climb_cmd(&design)?,
        MobfarmCmd::Bury { design, out, cover, seeds, ticks } => {
            let (c, spec) = load(version, &design)?;
            let (cells, fluids) = crate::bury::envelope(&spec, &c.clock.design, seeds, ticks)?;
            let (d, removed, opened) = crate::bury::bury(&c.clock.design, &cells, &fluids, cover);
            std::fs::write(&out, d.to_text())?;
            println!("{}: {} open cells ({} dug open beyond its blocks), {} containment blocks left to the rock; ground layer {}", out.display(), cells.len(), opened, removed, d.meta.get("ground").cloned().unwrap_or_default());
        }
        MobfarmCmd::Bill { design, stage, terrain } => {
            let (c, spec) = load(version, &design)?;
            let ix = match stage.as_str() {
                "day-1" | "day1" => 0,
                "middle" => 1,
                "late" | "fastest" => 2,
                s => anyhow::bail!("--stage {s}: day-1, middle or late"),
            };
            let site = match &terrain {
                Some(t) => Some(circuit::mobfarm::site_of(&c, &spec, t).map_err(|e| anyhow::anyhow!(e))?),
                None => None,
            };
            let bill = circuit::mobfarm::site_bill(&c, site.as_ref(), ix).map_err(|e| anyhow::anyhow!(e))?;
            let mut v: Vec<(&String, &(u32, f64))> = bill.items.iter().collect();
            v.sort_by(|a, b| b.1.1.partial_cmp(&a.1.1).unwrap_or(std::cmp::Ordering::Equal));
            for (k, (n, s)) in v {
                println!("{k:<24} {n:>6} {s:>9.1} s");
            }
            let travel: f64 = bill.items.iter().filter(|(k, _)| k.starts_with("travel:") || k.starts_with("villager:") || k.as_str() == "place:edge").map(|(_, (_, s))| s).sum();
            println!("{}: {:.1} build minutes at {stage} prices ({:.1} of them travel and height); collecting {:.0} s per trip ({} layers each way)", c.name(), bill.seconds / 60.0, travel / 60.0, bill.collect_s, bill.collect_layers);
        }
        MobfarmCmd::SpawnRules { design, terrain, tries, day_time } => crate::spawnrules::run(version, &design, &terrain, tries, day_time)?,
        MobfarmCmd::Exposures { design, terrain, cells, against } => {
            if let Some(before) = against {
                return exposures_against(version, &before, &design, &terrain);
            }
            let (c, spec) = load(version, &design)?;
            let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, &terrain).map_err(|e| anyhow::anyhow!(e))?));
            let moved = site.spec(&spec);
            // the farm after its settle (its water flowed), as the card's exposures line reads it
            let w = circuit::mobfarm::settled_world(site, &moved);
            let summoners = {
                let mut with = w.clone();
                for (k, p, n) in &moved.entities {
                    let _ = redsim::entity::spawn(&mut with, k, *p, n.as_deref());
                }
                redsim::entity::mob::site::villager_cells(&with)
            };
            let farm = circuit::mobfarm::hostile_farm(&w, &moved, site.afk, &summoners);
            let ex = redsim::entity::mob::hostile::check(&w, &farm, true);
            println!("{} in {} at offset {:?}: {} placed + {} filled + {} sealed, {} dug (the cheapest of {} spots, block time {:.0}-{:.0} s); {} golem cells, {} interior cells", design.display(), site.sample, site.offset, site.placed, site.fills.len(), site.seals.len(), site.dug, site.spots, site.build_range.0, site.build_range.1, farm.golem_cells.len(), farm.interior.len());
            // the village's saved mobs against the farm's villagers: a village villager within the
            // 10-box of a farm summoner (`spawnGolemIfNeeded`: inflate 10, 10, 10) counts toward
            // its 3; any golem within 16 of a farm villager stalls it (GolemSensor)
            if let Some(sample) = redsim::terrain::samples::sample(site.sample) {
                let ents = redsim::entity::mob::site::sample_entities(sample).map_err(|e| anyhow::anyhow!(e))?;
                let farm_v: Vec<[f64; 3]> = moved.entities.iter().filter(|(k, _, _)| k.trim_start_matches("minecraft:") == "villager").map(|(_, p, _)| *p).collect();
                let near = |p: &[f64; 3], r: f64, ry: f64| farm_v.iter().any(|v| (v[0] - p[0]).abs() < r && (v[2] - p[2]).abs() < r && (v[1] - p[1]).abs() < ry);
                for kind in ["villager", "iron_golem", "cat"] {
                    let of: Vec<&[f64; 3]> = ents.iter().filter(|(k, _)| k == kind).map(|(_, p)| p).collect();
                    let nearest = of.iter().map(|p| farm_v.iter().map(|v| ((v[0] - p[0]).powi(2) + (v[1] - p[1]).powi(2) + (v[2] - p[2]).powi(2)).sqrt()).fold(f64::INFINITY, f64::min)).fold(f64::INFINITY, f64::min);
                    println!("village {kind}s as saved: {} (nearest {:.1} blocks from a farm villager; {} within the 10-box of one, {} within 16)", of.len(), nearest, of.iter().filter(|p| near(p, 10.6, 11.95)).count(), of.iter().filter(|p| near(p, 16.0, 16.0)).count());
                }
            }
            if ex.is_empty() {
                println!("no exposure");
            }
            for e in ex {
                println!("{}", e.line());
                if cells {
                    let [ox, oy, oz] = site.offset;
                    println!("    design cells: {}", e.cells.iter().map(|c| format!("({},{},{})", c.x - ox, c.y - oy, c.z - oz)).collect::<Vec<_>>().join(" "));
                }
            }
        }
        MobfarmCmd::Spots { design, terrain, repeat, panicking_only, caps } => spots_cmd(version, &design, &terrain, repeat, !panicking_only, caps)?,
        MobfarmCmd::Gate { design, days, builds, no_rain_day, no_thunder_day, seed, parallel, warmup, keep, trace, terrain, world, no_monsters, runs, sim_only, inject_events } => {
            crate::gate::gate_cmd(version, &design, crate::gate::GateArgs { days, builds, rain_day: !no_rain_day, thunder_day: !no_thunder_day, seed, parallel, warmup: (warmup[0], warmup[1]), keep, trace, terrain, world, no_monsters, runs, sim_only, inject_events })?
        }
        MobfarmCmd::WeatherStats { days, servers, dt, sim_runs } => crate::weathercheck::weather_stats_cmd(version, days, servers, dt, sim_runs)?,
        MobfarmCmd::FireBurn { design, servers, max, seeds } => crate::weathercheck::fire_burn_cmd(version, &design, servers, max, seeds)?,
        MobfarmCmd::BoltCheck => crate::weathercheck::bolt_check_cmd(version)?,
        MobfarmCmd::Ground { design, sample, ground, build, margin, out } => {
            let (c, spec) = load(version, &design)?;
            let meta = &c.clock.design.meta;
            let sample = sample.or_else(|| meta.get("sample").cloned()).unwrap_or_else(|| "flat".to_string());
            let g = ground.or_else(|| meta.get("ground").and_then(|v| v.trim().parse().ok())).unwrap_or(-1);
            let r = crate::ground::ground(&design, &spec, &sample, g, build, margin, &out)?;
            println!(
                "{}: {} grounded in {} build {build} (offset {:?}), ground line y {g}, box {:?}..{:?}, {} terrain blocks added within {margin}",
                out.display(),
                design.display(),
                r.sample,
                r.offset,
                [r.box_lo.x, r.box_lo.y, r.box_lo.z],
                [r.box_hi.x, r.box_hi.y, r.box_hi.z],
                r.added
            );
        }
        MobfarmCmd::WeatherDays { design, runs, days, build, bins } => {
            let (c, spec) = load(version, &design)?;
            let sample = circuit::mobfarm::terrain_sample(&c).map_err(|e| anyhow::anyhow!(e))?.ok_or_else(|| anyhow::anyhow!("weather-days: the design is not judged in terrain"))?;
            let sites = circuit::mobfarm::terrain_sites(&c, &spec, sample).map_err(|e| anyhow::anyhow!(e))?;
            let site = *sites.get(build as usize).ok_or_else(|| anyhow::anyhow!("--build {build}: the judge has {} builds", sites.len()))?;
            crate::weathercheck::weather_days_cmd(&spec, site, build, runs, days, bins)?;
        }
        MobfarmCmd::Compare { design, seed, days, ticks, weather, probe, twin, full, bare, draws_at, terrain, day0, scan, batch, build, run, no_spawning, only } => {
            let ticks = ticks.unwrap_or((days * 24000.0).round() as u32);
            let opts = crate::exact::Opts { seed, ticks, weather, probe, batch: batch.max(1), full, bare, draws_at, terrain, day0, build: build.map(|k| (k, run)), no_spawning, only };
            if let Some(s) = scan {
                let (from, to): (i64, i64) = (s[1].parse()?, s[2].parse()?);
                let hits = crate::exact::scan(version, &design, &opts, from..=to, &s[0])?;
                println!("{} of {} seeds: {:?}", hits.len(), to - from + 1, hits);
                return Ok(());
            }
            compare_cmd(version, &design, &opts, twin)?;
        }
    }
    Ok(())
}

/// `queue-loop-run --phase-sweep WHO@S1[,S2]` (ironspec's forced-phase mode, tickexact
/// 2026-11-11): the same run once per combination of the listed sensors' start delays (each over
/// 0 up to its scan rate), forced on WHO after every whole-part load, on `jobs` threads; per
/// combination one line (its problems, first problem, WHO's wake after the load), then the table
/// (rows S1, columns S2: `.` clean, `X` a problem) and the failing share of the phase space.
#[allow(clippy::too_many_arguments)]
fn phase_sweep_cmd(spec: &redsim::entity::mob::farm::FarmSpec, o: &crate::queue::LoopOpts, tm: &crate::queue::LoopTiming, hdr: &crate::queue::Headers, sweep: &str, orders: &[String], cycles: i64, seed: i64, jobs: usize) -> Result<()> {
    let (who, rest) = sweep.split_once('@').ok_or_else(|| anyhow::anyhow!("--phase-sweep {sweep}: WHO@SENSOR[=VALUES][,SENSOR[=VALUES]]"))?;
    let who = crate::queue::parse_rider(sweep, who).map_err(|e| anyhow::anyhow!(e))?;
    let rate = |k: usize| redsim::entity::mob::villager::SENSORS[k].1;
    // (each sensor's values: all of 0 up to its scan rate, or `=A..B` / `=A` / `=A/B/C`)
    let mut ks: Vec<usize> = Vec::new();
    let mut vals: Vec<Vec<i64>> = Vec::new();
    for t in rest.split(',') {
        let (name, v) = t.split_once('=').map_or((t, None), |(n, v)| (n, Some(v)));
        let k = crate::queue::sensor_index(name).ok_or_else(|| anyhow::anyhow!("--phase-sweep {sweep}: no sensor {name}"))?;
        let list: Vec<i64> = match v {
            None => (0..rate(k)).collect(),
            Some(v) => match v.split_once("..") {
                Some((a, b)) => (a.parse::<i64>()?..=b.parse::<i64>()?).collect(),
                None => v.split('/').map(|x| x.parse::<i64>()).collect::<Result<_, _>>()?,
            },
        };
        anyhow::ensure!(!list.is_empty() && list.iter().all(|x| (0..rate(k)).contains(x)), "--phase-sweep {sweep}: {name}'s delays are 0..{}", rate(k) - 1);
        ks.push(k);
        vals.push(list);
    }
    anyhow::ensure!((1..=2).contains(&ks.len()), "--phase-sweep {sweep}: one or two sensors");
    anyhow::ensure!(tm.acts.iter().flatten().any(|a| a.unload == Some(None)), "--phase-sweep: no whole-part unload act (--act SLOT,DT,unload,N)");
    let second: Vec<i64> = if ks.len() == 2 { vals[1].clone() } else { vec![0] };
    // (ironspec 2026-11-11: phases x orders in one command) the load orders to sweep under
    // (`--reload-orders`, `crate::queue::parse_reload_orders`; the acts' own when none is named)
    let mut ords: Vec<Option<redsim::reload::Order>> = crate::queue::parse_reload_orders(orders).map_err(|e| anyhow::anyhow!(e))?.into_iter().map(Some).collect();
    if ords.is_empty() {
        ords.push(None);
    }
    let label = |o: Option<redsim::reload::Order>| o.map_or_else(|| "the acts' own".to_string(), |o| o.name());
    let mut combos: Vec<(usize, i64, i64)> = Vec::new();
    for oi in 0..ords.len() {
        for &a in &vals[0] {
            for &b in &second {
                combos.push((oi, a, b));
            }
        }
    }
    let mut results: std::collections::BTreeMap<(usize, i64, i64), (bool, String)> = std::collections::BTreeMap::new();
    for chunk in combos.chunks(jobs.max(1)) {
        let got: Vec<((usize, i64, i64), Result<String, String>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|&(oi, a, b)| {
                    let mut t = *tm;
                    if let Some(ord) = ords[oi] {
                        for act in t.acts.iter_mut().flatten() {
                            if act.unload == Some(None) {
                                act.order = ord;
                            }
                        }
                    }
                    let mut sensors = [None; 4];
                    sensors[0] = Some((ks[0], a));
                    if ks.len() == 2 {
                        sensors[1] = Some((ks[1], b));
                    }
                    t.phase_force = Some(crate::queue::PhaseForce { who, sensors });
                    sc.spawn(move || ((oi, a, b), crate::queue::run_loop_hdr(spec, o, &t, hdr, cycles, seed, false, None, None)))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
        });
        for ((oi, a, b), r) in got {
            let r = r.map_err(|e| anyhow::anyhow!(e))?;
            let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("?").trim_end_matches(':').to_string();
            let first = r.lines().skip_while(|l| !l.ends_with("problems:")).nth(1).map(|l| format!(" (first: {})", l.trim())).unwrap_or_default();
            let forced = r.lines().find(|l| l.contains("phases forced on")).map(|l| l.split(": ").skip(1).collect::<Vec<_>>().join(": ")).unwrap_or_else(|| "not forced".into());
            let woke = r.lines().find(|l| l.contains("(phases forced) woke at")).and_then(|l| l.split("woke at ").nth(1)).unwrap_or("not seen").to_string();
            let ok = probs.starts_with("0 problems");
            let names: Vec<String> = ks.iter().zip([a, b]).map(|(k, v)| format!("{}={v}", redsim::entity::mob::villager::SENSORS[*k].0)).collect();
            // (day-1 2026-11-11, ironspec's 05:58 question: the forced villager's own panic on the
            // slot ticks it spent at the stand after the load, from the stand line's per-villager
            // counts by its id after the load)
            let fid = r.lines().find_map(|l| l.split("phases forced on e").nth(1)).and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next()).and_then(|d| d.parse::<u32>().ok());
            let at_stand = fid
                .and_then(|id| {
                    let l = r.lines().find(|l| l.starts_with("stand batch panicking"))?;
                    let per = l.split("per villager").nth(1)?;
                    let toks: Vec<&str> = per.split_whitespace().collect();
                    let k = toks.iter().position(|t| *t == format!("e{id}"))?;
                    toks.get(k + 1).map(|v| format!("; at the stand e{id} panicking on {v} slot ticks"))
                })
                .unwrap_or_else(|| "; at the stand: not seen".into());
            println!("order {} phases {}: {probs}{first}; woke {woke}{at_stand}; {forced}", label(ords[oi]), names.join(" "));
            use std::io::Write;
            let _ = std::io::stdout().flush();
            results.insert((oi, a, b), (ok, woke));
        }
    }
    for (oi, ord) in ords.iter().enumerate() {
        let bad = results.iter().filter(|(k, (ok, _))| k.0 == oi && !ok).count();
        let n = results.keys().filter(|k| k.0 == oi).count();
        println!("order {}: table (rows {} {:?}, columns {}; . clean, X a problem):", label(*ord), redsim::entity::mob::villager::SENSORS[ks[0]].0, vals[0], if ks.len() == 2 { format!("{} {:?}", redsim::entity::mob::villager::SENSORS[ks[1]].0, second) } else { "-".into() });
        for &a in &vals[0] {
            let row: String = second.iter().map(|&b| if results.get(&(oi, a, b)).is_some_and(|r| r.0) { '.' } else { 'X' }).collect();
            println!("  {a:>3} {row}");
        }
        println!("order {}: failing share of the phase space: {bad} of {n} ({:.1}%)", label(*ord), 100.0 * bad as f64 / n.max(1) as f64);
    }
    Ok(())
}

/// `queue-loop-run --reload-orders ...` (ironspec's reload-order rule; farmopt 2026-11-11, day-1's
/// unload rows): the same run (`--seed`, `--cycles`) once per load order, every whole-part unload
/// act loading in that order, on `jobs` threads; per order one line (its problems and first
/// problem, the visits line, the lap order, and the visits spanning an unload: those started up to
/// 6 slots before it, each one's rest slot and tick), then the row's verdict (a failure under any
/// order fails the row).
#[allow(clippy::too_many_arguments)]
fn reload_orders_cmd(spec: &redsim::entity::mob::farm::FarmSpec, o: &crate::queue::LoopOpts, tm: &crate::queue::LoopTiming, hdr: &crate::queue::Headers, names: &[String], cycles: i64, seed: i64, jobs: usize) -> Result<()> {
    use redsim::reload::Order;
    let orders: Vec<Order> = crate::queue::parse_reload_orders(names).map_err(|e| anyhow::anyhow!(e))?;
    let unloads: Vec<crate::queue::Act> = tm.acts.iter().flatten().filter(|a| a.unload == Some(None)).copied().collect();
    anyhow::ensure!(!unloads.is_empty(), "--reload-orders: no whole-part unload act (--act SLOT,DT,unload,N)");
    let spans = |l: &str| -> bool {
        l.strip_prefix("visit slot ").and_then(|r| r.split(' ').next()).and_then(|s| s.parse::<i64>().ok()).is_some_and(|s| unloads.iter().any(|a| s <= a.slot && s >= a.slot - 6))
    };
    let mut failed: Vec<String> = Vec::new();
    for chunk in orders.chunks(jobs.max(1)) {
        let got: Vec<(Order, Result<String, String>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|&ord| {
                    let mut t = *tm;
                    for a in t.acts.iter_mut().flatten() {
                        if a.unload == Some(None) {
                            a.order = ord;
                        }
                    }
                    sc.spawn(move || (ord, crate::queue::run_loop_hdr(spec, o, &t, hdr, cycles, seed, false, None, None)))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
        });
        for (ord, r) in got {
            let r = r.map_err(|e| anyhow::anyhow!(e))?;
            let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("?").trim_end_matches(':').to_string();
            let first = r.lines().skip_while(|l| !l.ends_with("problems:")).nth(1).map(|l| format!(" (first: {})", l.trim())).unwrap_or_default();
            let visits = r.lines().find_map(|l| l.strip_prefix("visits: ")).unwrap_or("-");
            let lap = r.lines().find(|l| l.starts_with("lap order: ")).map(|l| l.split(" (").next().unwrap_or(l)).unwrap_or("lap order: -");
            let around: Vec<String> = r
                .lines()
                .filter(|l| spans(l))
                .map(|l| {
                    let head = l.split(':').next().unwrap_or(l).trim_start_matches("visit slot ").to_string();
                    let back = l.split("back at rest in slot ").nth(1).map(|t| format!("slot {t}")).unwrap_or_else(|| "never back at rest".into());
                    format!("{head}: {back}")
                })
                .collect();
            if !probs.starts_with("0 problems") {
                failed.push(ord.name());
            }
            println!("order {}: {probs}{first}; visits: {visits}; {lap}; around the unload: {}", ord.name(), if around.is_empty() { "-".to_string() } else { around.join("; ") });
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }
    let acts: Vec<String> = unloads.iter().map(|a| format!("{},{},unload,{}", a.slot, a.dt, a.hold.unwrap_or(1))).collect();
    // (tick-exact 2026-11-11: each order is one sample of the game's random reload, so the row's
    // miss rate with its Wilson 95% interval)
    let (lo, hi) = crate::queue::wilson95(failed.len(), orders.len());
    let rate = format!("{} of {} orders missed ({:.0}%, Wilson 95% {:.0}-{:.0}%)", failed.len(), orders.len(), 100.0 * failed.len() as f64 / orders.len().max(1) as f64, 100.0 * lo, 100.0 * hi);
    println!("row {} (seed {seed}, {cycles} slots): {}; {rate}", acts.join(" + "), if failed.is_empty() { format!("PASS under all {} orders", orders.len()) } else { format!("FAIL under {}", failed.join(", ")) });
    Ok(())
}

/// A design's exposures in a terrain sample (`mobfarm exposures`' checks): its site's offset, its
/// golem and interior cell counts, and every exposure.
fn exposures_of(version: &str, design: &std::path::Path, terrain: &str) -> Result<([i32; 3], usize, usize, Vec<redsim::entity::mob::hostile::Exposure>)> {
    let (c, spec) = load(version, &design.to_path_buf())?;
    let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, terrain).map_err(|e| anyhow::anyhow!(e))?));
    let moved = site.spec(&spec);
    let w = circuit::mobfarm::settled_world(site, &moved);
    let summoners = {
        let mut with = w.clone();
        for (k, p, n) in &moved.entities {
            let _ = redsim::entity::spawn(&mut with, k, *p, n.as_deref());
        }
        redsim::entity::mob::site::villager_cells(&with)
    };
    let farm = circuit::mobfarm::hostile_farm(&w, &moved, site.afk, &summoners);
    let ex = redsim::entity::mob::hostile::check(&w, &farm, true);
    Ok((site.offset, farm.golem_cells.len(), farm.interior.len(), ex))
}

/// `mobfarm exposures AFTER --against BEFORE`: one line per class (channel, mob, what) in either,
/// `BEFORE -> AFTER` cells with the ones added and removed (design coordinates), unchanged classes
/// included; then the golem and interior cells and how many classes changed (ironspec
/// 2026-11-11: every class printed, so a cut diff cannot hide one).
fn exposures_against(version: &str, before: &std::path::Path, after: &std::path::Path, terrain: &str) -> Result<()> {
    use std::collections::{BTreeMap, BTreeSet};
    let (ob, gb, ib, eb) = exposures_of(version, before, terrain)?;
    let (oa, ga, ia, ea) = exposures_of(version, after, terrain)?;
    let design = |o: [i32; 3], cs: &[redsim::BlockPos]| -> BTreeSet<(i32, i32, i32)> { cs.iter().map(|c| (c.x - o[0], c.y - o[1], c.z - o[2])).collect() };
    let mut classes: BTreeMap<(String, String, String), (BTreeSet<(i32, i32, i32)>, BTreeSet<(i32, i32, i32)>)> = BTreeMap::new();
    for e in &eb {
        classes.entry((e.channel.to_string(), e.mob.to_string(), e.what.to_string())).or_default().0 = design(ob, &e.cells);
    }
    for e in &ea {
        classes.entry((e.channel.to_string(), e.mob.to_string(), e.what.to_string())).or_default().1 = design(oa, &e.cells);
    }
    let fmt = |s: &BTreeSet<(i32, i32, i32)>| s.iter().map(|(x, y, z)| format!("({x},{y},{z})")).collect::<Vec<_>>().join(" ");
    println!("exposures of {} against {} in {terrain} (design coordinates):", after.display(), before.display());
    let mut changed = 0;
    for ((ch, mob, what), (b, a)) in &classes {
        let (added, removed): (BTreeSet<_>, BTreeSet<_>) = (a.difference(b).copied().collect(), b.difference(a).copied().collect());
        let change = match (b.is_empty(), a.is_empty()) {
            (_, true) => "gone".to_string(),
            (true, _) => "new".to_string(),
            _ if added.is_empty() && removed.is_empty() => "unchanged".to_string(),
            _ => format!("{:+}", a.len() as i64 - b.len() as i64),
        };
        if change != "unchanged" {
            changed += 1;
        }
        let detail = [(!added.is_empty()).then(|| format!("added {}", fmt(&added))), (!removed.is_empty()).then(|| format!("removed {}", fmt(&removed)))].into_iter().flatten().collect::<Vec<_>>().join("; ");
        println!("{ch}: {mob} {what}: {} -> {} cells ({change}){}", b.len(), a.len(), if detail.is_empty() { String::new() } else { format!(": {detail}") });
    }
    println!("golem cells {gb} -> {ga}; interior cells {ib} -> {ia}");
    println!("{changed} of {} classes changed", classes.len());
    Ok(())
}

/// `mobfarm compare`: the exact compare (or the twin check), its report; a difference fails.
fn compare_cmd(version: &str, design: &std::path::Path, opts: &crate::exact::Opts, twin: bool) -> Result<()> {
    let t0 = std::time::Instant::now();
    let cmp = if twin { crate::exact::twin(version, design, opts, None)? } else { crate::exact::compare(version, design, opts, None)? };
    print!("{}", cmp.report());
    if let (Some(d), true, true) = (&cmp.first, opts.probe, twin) {
        crate::exact::twin(version, design, opts, Some(d.t))?;
    }
    if let (Some(d), true, false) = (&cmp.first, opts.probe, twin) {
        println!("probe: the game's draws around game time {} (a second, identical run)", d.t);
        let again = crate::exact::compare(version, design, opts, Some(d.t))?;
        if let Some(d2) = &again.first {
            for c in &d2.context {
                println!("  {c}");
            }
        }
    }
    println!("{:.1} s", t0.elapsed().as_secs_f64());
    if cmp.first.is_some() {
        bail!("{} differs", if twin { "the game against itself" } else { "redsim against the game" });
    }
    Ok(())
}

/// `mobfarm spots`: the design priced at each spot of a terrain sample, cheapest first.
fn spots_cmd(version: &str, design: &PathBuf, terrain: &str, repeat: u32, all: bool, list_caps: bool) -> Result<()> {
    use redsim::entity::mob::site;
    let (c, spec) = load(version, design)?;
    let d = &c.clock.design;
    let sample = redsim::terrain::samples::sample(terrain).ok_or_else(|| anyhow::anyhow!("no terrain sample {terrain}"))?;
    let (ground, _afk, clear_box) = site::params(&spec, d.meta.get("ground").map(String::as_str), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str)).map_err(|e| anyhow::anyhow!(e))?;
    let t0 = std::time::Instant::now();
    let w = site::sample_world(sample).map_err(|e| anyhow::anyhow!(e))?;
    let t_world = t0.elapsed().as_secs_f64();
    let fp = site::footprint(&spec, ground).map_err(|e| anyhow::anyhow!(e))?;
    let t1 = std::time::Instant::now();
    let spots = site::spots_for_spec(&w, sample, &spec, fp, clear_box);
    let t_spots = t1.elapsed().as_secs_f64();
    let t2 = std::time::Instant::now();
    let mut cheap = Vec::new();
    for _ in 0..repeat.max(1) {
        cheap = site::price_spots(&w, &spec, fp, ground, clear_box, &spots);
    }
    let t_cheap = t2.elapsed().as_secs_f64() / repeat.max(1) as f64;
    let t3 = std::time::Instant::now();
    let mut priced: Vec<site::Priced> = cheap.iter().map(|p| site::Priced { price: *p, covers: site::covers_from(&w, &spec, p.offset, clear_box, all), seals: site::seals_at(&w, &spec, p.offset, clear_box), clears: site::clears_at(&w, &spec, p.offset, clear_box) }).collect();
    let t_covers = t3.elapsed().as_secs_f64();
    priced.sort_by(|a, b| a.seconds().total_cmp(&b.seconds()));
    println!("{} in {terrain} ({}): footprint {}x{}, {} deep, {} spots", design.display(), if clear_box { "its box cleared" } else { "its cells written, natural rock for the rest" }, fp.size[0], fp.size[1], fp.below, spots.len());
    for (k, p) in priced.iter().enumerate() {
        println!("  {k:2}: offset {:?}: {} placed + {} covers + {} seals ({} dug first) + {} cleared of fire ({} replaced with cobblestone), {} dug: {:.0} s", p.price.offset, p.price.placed, p.covers.len(), p.seals.len(), p.seals.iter().filter(|(_, d)| *d).count(), p.clears.len(), p.clears.iter().filter(|(_, b)| !b.is_air()).count(), p.price.dug, p.seconds());
        if list_caps && k == 0 {
            let o = p.price.offset;
            println!("      caps (design cells): {}", p.covers.iter().map(|c| format!("({},{},{})", c.x - o[0], c.y - o[1], c.z - o[2])).collect::<Vec<_>>().join(" "));
        }
    }
    println!("times: sample world {:.3} s (once a process), spots {:.4} s (per footprint and depth), cheap part {:.6} s a call, covers {:.4} s for {} spots", t_world, t_spots, t_cheap, t_covers, priced.len());
    Ok(())
}

/// `mobfarm cubes`: clusters by the spawn's 10-box (`Villager.spawnGolemIfNeeded`: the summoner's
/// box inflated by 10), each golem spawn spot (`exit::spawn_spots`) owned by the clusters whose
/// villagers' spawn search reaches it (8 columns, 6 up or down from the villager's block), and its
/// golem box (1.4 x 2.7 at the cell's center) against the follow cube of every villager of the
/// other clusters (`NearestLivingEntitySensor`: the villager's box inflated by FOLLOW_RANGE on all
/// three axes; the range at its largest, 16 x (1 + 0.11485)).
fn cubes_cmd(version: &str, design: &PathBuf) -> Result<()> {
    use redsim::entity::mob::farm::{RunOpts, WalkMode, build};
    use redsim::entity::{Aabb, EntityKind};
    const R: f64 = 16.0 * 1.11485;
    let (_, spec) = load(version, design)?;
    let w = build(&spec, &RunOpts::steady(1, 0, WalkMode::Random)).map_err(|e| anyhow::anyhow!(e))?;
    let vs: Vec<(Aabb, redsim::BlockPos)> = w
        .entities
        .list
        .iter()
        .filter(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager))
        .map(|e| (e.bb, e.block_position()))
        .collect();
    // clusters: union-find over "inside the other's 10-box"
    let mut parent: Vec<usize> = (0..vs.len()).collect();
    fn root(p: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..vs.len() {
        for j in 0..vs.len() {
            if i != j && vs[i].0.inflate(10.0, 10.0, 10.0).intersects(vs[j].0) {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    let cluster: Vec<usize> = (0..vs.len()).map(|i| root(&mut parent, i)).collect();
    let mut ids: Vec<usize> = cluster.clone();
    ids.sort();
    ids.dedup();
    println!("{} villagers in {} clusters (by the spawn's 10-box)", vs.len(), ids.len());
    let spots = redsim::entity::mob::exit::spawn_spots(&w);
    let mut worst: Option<(f64, redsim::BlockPos, usize)> = None;
    let mut inside = 0;
    for c in &spots {
        let owners: Vec<usize> = (0..vs.len()).filter(|&i| (c.x - vs[i].1.x).abs() <= 8 && (c.z - vs[i].1.z).abs() <= 8 && (c.y - vs[i].1.y).abs() <= 6).map(|i| cluster[i]).collect();
        let (x, y, z) = (c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5);
        let g = Aabb::new(x - 0.7, y, z - 0.7, x + 0.7, y + 2.7, z + 0.7);
        for (i, (vb, _)) in vs.iter().enumerate() {
            if owners.contains(&cluster[i]) {
                continue;
            }
            let cube = vb.inflate(R, R, R);
            let sep = [cube.min_x - g.max_x, g.min_x - cube.max_x, cube.min_y - g.max_y, g.min_y - cube.max_y, cube.min_z - g.max_z, g.min_z - cube.max_z].into_iter().fold(f64::NEG_INFINITY, f64::max);
            if sep < 0.0 {
                inside += 1;
            }
            if worst.is_none_or(|(s, _, _)| sep < s) {
                worst = Some((sep, *c, i));
            }
        }
    }
    println!("{} spawn spots; {} (spot, foreign villager) pairs inside the follow cube", spots.len(), inside);
    match worst {
        Some((sep, c, i)) => println!("smallest margin {sep:.4} blocks: the spot {} {} {} against villager {} (feet {:.4})", c.x, c.y, c.z, i, vs[i].0.min_y),
        None => println!("no spot near a foreign cluster"),
    }
    Ok(())
}

/// `mobfarm sight`: each villager's eye to the zombie's eye in the built farm, as built and with
/// every trapdoor of the farm forced shut, then forced open.
/// `mobfarm sight --spawns`: every golem spawn cell from each farm villager standing at its cell
/// and 1, 2 and 3 above it (a villager on its bed's edge, a hop), in design coordinates.
fn spawns_cmd(version: &str, design: &PathBuf, terrain: Option<&str>) -> Result<()> {
    use redsim::entity::EntityKind;
    use redsim::entity::mob::types::MobType;
    let (c, spec) = load(version, design)?;
    let mut o = redsim::entity::mob::farm::RunOpts::steady(1, 0, redsim::entity::mob::farm::WalkMode::Random);
    let mut offset = [0, 0, 0];
    let moved = match terrain {
        Some(name) => {
            let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow::anyhow!(e))?));
            o.site = Some(site);
            offset = site.offset;
            println!("terrain {}: offset {:?} (design = world - offset)", site.sample, site.offset);
            site.spec(&spec)
        }
        None => spec.clone(),
    };
    let run = redsim::entity::mob::farm::Run::new(&moved, &o).map_err(|e| anyhow::anyhow!(e))?;
    let w = &run.world;
    let design_cells: std::collections::HashSet<redsim::BlockPos> = spec.blocks.iter().map(|(p, _)| *p).collect();
    let d = |p: redsim::BlockPos| redsim::BlockPos::new(p.x - offset[0], p.y - offset[1], p.z - offset[2]);
    for (i, e) in w.entities.list.iter().enumerate() {
        let EntityKind::Mob(m) = &e.kind else { continue };
        if m.ty != MobType::Villager || !run.own.contains(&e.id) {
            continue;
        }
        let cell = e.block_position();
        println!("villager #{i} at {:.2} {:.2} {:.2} (design cell {})", e.pos.x - offset[0] as f64, e.pos.y - offset[1] as f64, e.pos.z - offset[2] as f64, d(cell));
        for up in 0..=3 {
            let start = cell.offset(0, up, 0);
            let cells: Vec<String> = redsim::entity::mob::spawn::golem_spawn_cells_from(w, start)
                .into_iter()
                .filter(|(_, r)| r.is_ok())
                .map(|(p, _)| {
                    let on = d(p.below());
                    let whose = if design_cells.contains(&on) { "design" } else { "natural" };
                    format!("{} on {} ({whose})", d(p), w.get(p.below()))
                })
                .collect();
            println!("  standing {up} up (search from design y {}): {} spawn cells{}", d(start).y + 6, cells.len(), if cells.is_empty() { String::new() } else { format!(": {}", cells.join("; ")) });
        }
    }
    Ok(())
}

fn sight_cmd(version: &str, design: &PathBuf, terrain: Option<&str>) -> Result<()> {
    use redsim::block::Block;
    use redsim::entity::EntityKind;
    use redsim::entity::mob::types::MobType;
    let (c, spec) = load(version, design)?;
    let mut o = redsim::entity::mob::farm::RunOpts::steady(1, 0, redsim::entity::mob::farm::WalkMode::Random);
    let spec = match terrain {
        Some(name) => {
            let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow::anyhow!(e))?));
            o.site = Some(site);
            site.spec(&spec)
        }
        None => spec,
    };
    let run = redsim::entity::mob::farm::Run::new(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
    let own = run.own.clone();
    let mut base = run.world;
    base.entities.list.retain(|e| own.contains(&e.id));
    let eye = |w: &redsim::world::World, i: usize| redsim::entity::mob::villager::eye_pos(w, i);
    let zombies: Vec<usize> = base.entities.list.iter().enumerate().filter(|(_, e)| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Zombie)).map(|(i, _)| i).collect();
    if zombies.is_empty() {
        bail!("no zombie");
    }
    let trapdoors: Vec<redsim::BlockPos> = spec.blocks.iter().filter(|(_, b)| matches!(b, Block::Trapdoor { .. })).map(|(p, _)| *p).collect();
    for (label, open) in [("as built", None), ("trapdoors shut", Some(false)), ("trapdoors open", Some(true))] {
        let mut w = base.clone();
        if let Some(o) = open {
            for p in &trapdoors {
                if let Block::Trapdoor { iron, facing, top, powered, .. } = w.get(*p) {
                    w.set_block(*p, Block::Trapdoor { iron, facing, top, open: o, powered }, redsim::world::flags::RAW);
                }
            }
        }
        println!("{label}:");
        for (i, e) in w.entities.list.iter().enumerate() {
            let EntityKind::Mob(m) = &e.kind else { continue };
            if m.ty != MobType::Villager {
                continue;
            }
            // the nearest zombie (one per cluster)
            let z = *zombies.iter().min_by(|a, b| w.entities.list[**a].pos.distance_to_sqr(e.pos).total_cmp(&w.entities.list[**b].pos.distance_to_sqr(e.pos))).unwrap_or(&zombies[0]);
            let ze = eye(&w, z);
            // standing, then asleep on its bed (LivingEntity.startSleeping puts it at bed + 0.6875,
            // then gravity settles its 0.2 x 0.2 SLEEPING box on the bed's top, 0.5625: its eye
            // 0.2 above that)
            let mut eyes = vec![("standing", eye(&w, i))];
            if let Some(home) = m.brain.as_deref().and_then(|b| b.home) {
                eyes.push(("asleep", redsim::entity::Vec3::new(home.x as f64 + 0.5, home.y as f64 + 0.5625 + 0.2, home.z as f64 + 0.5)));
            }
            for (how, ve) in eyes {
                let what = match redsim::entity::mob::los::first_hit(&w, ve, ze) {
                    None => "SEES its zombie".to_string(),
                    Some(p) => format!("blind (stopped at {} {} {}: {})", p.x, p.y, p.z, w.get(p)),
                };
                println!("  villager at {:.2} {:.2} {:.2} {how}, eye {:.2} {:.4} {:.2}, zombie eye {:.2} {:.4} {:.2}: {what}", e.pos.x, e.pos.y, e.pos.z, ve.x, ve.y, ve.z, ze.x, ze.y, ze.z);
            }
        }
    }
    Ok(())
}

/// A design's seeds record, `<stem>.seeds.tsv` beside it (page copy's card fields).
fn seeds_file(design: &std::path::Path) -> PathBuf {
    let stem = design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    design.with_file_name(format!("{stem}.seeds.tsv"))
}

/// Record one `mobfarm seeds` run in the design's seeds file: the header `days held of seeds` and a
/// note when the file is new, then this run's row; a row with the same days and seeds is replaced
/// in place (a run again on the same seeds is the same measurement, newer), any other appended.
fn write_seeds_row(path: &std::path::Path, days: u32, held: usize, of: usize, seeds: &str) -> Result<()> {
    let old = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = old.lines().map(str::to_string).collect();
    if lines.iter().all(|l| l.starts_with('#') || l.trim().is_empty()) {
        lines = vec![
            "# the judge's runs on many seeds, redsim (riders counted at every midday): written by `mctas mobfarm seeds <design> --from A --to B --days N --write`".to_string(),
            "days\theld\tof\tseeds".to_string(),
        ];
    }
    let row = format!("{days}\t{held}\t{of}\t{seeds}");
    let same = |l: &str| {
        let c: Vec<&str> = l.split('\t').collect();
        c.len() >= 4 && c[0] == days.to_string() && c[3] == seeds
    };
    if let Some(i) = lines.iter().position(|l| same(l)) {
        lines[i] = row;
    } else {
        lines.push(row);
    }
    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

fn validate_cmd(version: &str, design: &PathBuf, days: u32, kill: bool, seeds: u32, trace: bool, repeat: u32) -> Result<()> {
    let (c, spec) = load(version, design)?;
    let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let mut all = crate::validate::GameRun::default();
    let mut game_rates = Vec::new();
    for r in 0..repeat.max(1) {
        let t0 = std::time::Instant::now();
        let g = crate::validate::game_run(&pool, &c, days, kill, trace, 1 + r as i64)?;
        println!("game run {r}: {} golems in {days} days ({:.1}/h), {} iron and {} poppies in the chest, {:.0} s", g.golems.len(), g.golems_per_hour(), g.iron_in_chest, g.poppies_in_chest, t0.elapsed().as_secs_f64());
        game_rates.push(g.golems_per_hour());
        all.ticks += g.ticks;
        all.golems.extend(g.golems);
        all.iron_in_chest += g.iron_in_chest;
        all.poppies_in_chest += g.poppies_in_chest;
    }
    let g = all;
    let gm = game_rates.iter().sum::<f64>() / game_rates.len() as f64;
    let gsd = if game_rates.len() > 1 { (game_rates.iter().map(|x| (x - gm) * (x - gm)).sum::<f64>() / (game_rates.len() - 1) as f64).sqrt() } else { 0.0 };
    println!("game: {} runs, golems/h mean {gm:.1} (95% of the mean {:.1}-{:.1}), {} iron and {} poppies in all", game_rates.len(), gm - 1.96 * gsd / (game_rates.len() as f64).sqrt(), gm + 1.96 * gsd / (game_rates.len() as f64).sqrt(), g.iron_in_chest, g.poppies_in_chest);
    let mut cells: std::collections::BTreeMap<[i32; 3], u32> = std::collections::BTreeMap::new();
    for (_, p) in &g.golems {
        *cells.entry(*p).or_default() += 1;
    }
    println!("game spawn cells: {cells:?}");
    // day (day time 10..12000) against night (REST), the game's day time starting at 1000
    let night = |day_time: i64| { let d = day_time.rem_euclid(24000); !(10..12000).contains(&d) };
    let g_night = g.golems.iter().filter(|(t, _)| night(1000 + *t as i64)).count();
    println!("game: {} golems by day, {g_night} by night", g.golems.len() - g_night);
    use redsim::entity::mob::farm::{RunOpts, WalkMode, run};
    let jobs: Vec<RunOpts> = (0..seeds).map(|k| RunOpts::steady(1000 + k as i64, days * 24000, WalkMode::Random)).collect();
    let runs = circuit::mobfarm::run_all(&spec, &jobs);
    let rates: Vec<f64> = runs.iter().filter_map(|r| r.as_ref().ok()).map(|r| r.golems_per_hour()).collect();
    let m = rates.iter().sum::<f64>() / rates.len().max(1) as f64;
    let sd = (rates.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (rates.len().max(2) - 1) as f64).sqrt();
    println!("redsim: {} runs of {days} days: golems/h mean {m:.1}, sd per run {sd:.1}: one game run is expected within {:.1}-{:.1} (95%)", rates.len(), m - 1.96 * sd, m + 1.96 * sd);
    let (mut sd_day, mut sd_night) = (0usize, 0usize);
    for (j, r) in jobs.iter().zip(runs.iter()) {
        if let Ok(r) = r {
            for t in &r.spawn_times {
                if night(j.day0 + *t) { sd_night += 1 } else { sd_day += 1 }
            }
        }
    }
    println!("redsim: {sd_day} golems by day, {sd_night} by night (night share {:.2}; game {:.2})", sd_night as f64 / (sd_day + sd_night).max(1) as f64, g_night as f64 / g.golems.len().max(1) as f64);
    // the spawn cells: redsim's shares against the game's counts (Pearson's chi-square)
    let mut sim: std::collections::BTreeMap<[i32; 3], u32> = std::collections::BTreeMap::new();
    for r in runs.iter().filter_map(|r| r.as_ref().ok()) {
        for p in &r.spawn_cells {
            *sim.entry(*p).or_default() += 1;
        }
    }
    let total_sim: u32 = sim.values().sum();
    let n = g.golems.len() as f64;
    let mut chi = 0.0;
    let mut bins = 0;
    for (p, k) in &sim {
        let e = n * *k as f64 / total_sim.max(1) as f64;
        let o = *cells.get(p).unwrap_or(&0) as f64;
        if e > 0.0 {
            chi += (o - e) * (o - e) / e;
            bins += 1;
        }
        println!("  cell {p:?}: redsim share {:.3}, game {o} of {n} (expected {e:.1})", *k as f64 / total_sim.max(1) as f64);
    }
    let strays = cells.keys().filter(|p| !sim.contains_key(*p)).count();
    println!("spawn cells: chi-square {chi:.2} over {bins} cells ({} degrees of freedom; 5% critical value {:.1}); {strays} game cells redsim never spawned in", bins.max(1) - 1, chi_crit(bins.max(1) - 1));
    let _ = run;
    Ok(())
}

/// The chi-square distribution's 95% quantile for `df` degrees of freedom (Wilson-Hilferty).
fn chi_crit(df: usize) -> f64 {
    let k = df.max(1) as f64;
    let z = 1.6449;
    k * (1.0 - 2.0 / (9.0 * k) + z * (2.0 / (9.0 * k)).sqrt()).powi(3)
}
