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
    /// inputs), with its spawn spots.
    Exit {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        seed: i64,
    },
    /// The built farm (water settled) along a row: each cell's block from x0 to x1 at y, z.
    Probe {
        design: PathBuf,
        #[arg(long, allow_hyphen_values = true, num_args = 4)]
        row: Vec<i32>,
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
        #[arg(long)]
        out: PathBuf,
    },
    /// Lists the golem spawn cells a villager riding anywhere on a loop scene's track would find
    /// (none may exist away from the pad).
    QueueLoopSpots {
        design: PathBuf,
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
        /// The release group's slopes powered this long from the slot's start (day-1's release
        /// controller: 34).
        #[arg(long, default_value_t = 34)]
        release_for: i64,
        /// The queue's 3 advance pulses: the first this many gt after the slot's start, then
        /// every `--advance-every`, each `--pulse` long (day-1's controller: 38, 20, 10).
        #[arg(long, default_value_t = 38)]
        advance_at: i64,
        #[arg(long, default_value_t = 20)]
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
        /// Added to the stand's departure times (with `--wired`, day-1's release starts 4 gt
        /// after the golem pulse: pass 4).
        #[arg(long, default_value_t = 0)]
        stand_offset: i64,
        /// The release group's 3 slopes on front first, this many gt apart (0: together).
        #[arg(long, default_value_t = 0)]
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
        /// front of its control delays both edges), not all three off at --release-for.
        #[arg(long)]
        stagger_off: bool,
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
        #[arg(long, default_value_t = 10, allow_hyphen_values = true)]
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
        /// The wired pocket stand (`queue-loop --stand-wired`): its input cell REL a redstone
        /// block from `--pocket-release` for this many gt (then glass).
        #[arg(long, default_value_t = 40)]
        rel_for: i64,
        /// `--seeds` runs on this many threads (one process).
        #[arg(long, default_value_t = 1)]
        jobs: usize,
        #[arg(long)]
        verbose: bool,
        /// Print these carts (design order) every gt.
        #[arg(long, value_delimiter = ',')]
        watch: Vec<usize>,
        /// Write the run's drive log (every control change as `act <t> set x y z <state>`, each
        /// slot's cart positions as `expect <t> pos eN x y z`) for mobviewer's replay.
        #[arg(long)]
        drive_out: Option<PathBuf>,
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
        MobfarmCmd::Exit { design, seed } => {
            let (_, spec) = load(version, &design)?;
            let refused = redsim::entity::mob::refuse::static_checks(&spec);
            for r in &refused {
                println!("refused: {r}");
            }
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32);
            let t0 = std::time::Instant::now();
            let rep = redsim::entity::mob::exit::golem_exit(&spec, seed, threads).map_err(|e| anyhow::anyhow!(e))?;
            println!("{} spawn spots, {} positions, {} runs, worst life {} gt, {:.1} s", rep.spawn_spots, rep.starts, rep.runs, rep.worst_life, t0.elapsed().as_secs_f64());
            for f in &rep.failures {
                println!("FAIL {f}");
            }
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
            println!("{} ({} KB)", html.display(), std::fs::metadata(&html)?.len() / 1024);
            println!("{} ({} KB)", txt.display(), std::fs::metadata(&txt)?.len() / 1024);
            println!("{ticks} ticks recorded in {:.1} s", t0.elapsed().as_secs_f64());
        }
        MobfarmCmd::QueueLoop { slots, run, depart, boost_every, boost_first, straight, drop, vstand, no_tail_v, folded, row_gap, uturn_boost, line, no_tail_deep, plain, stand_desc, detector, zombie_every, zombie_x, stand_deep, tail_desc, peak_pad, short_depart, stand_stop, stand_wired, out } => {
            let tail_deep = !no_tail_deep && !plain && !tail_desc;
            let slots = if line && slots == 18 { 15 } else { slots };
            let o = crate::queue::LoopOpts { slots, run, depart, boost_every, boost_first, straight, drop, vstand, tail_v: !no_tail_v && !tail_deep && !plain && !tail_desc, bridge: !folded && !straight && !line, row_gap, uturn_boost, line, tail_deep, plain, stand_desc, detector, zombie_every, zombie_x, stand_deep, tail_desc, peak_pad, short_depart, stand_stop, stand_wired: stand_wired && stand_stop };
            std::fs::write(&out, crate::queue::loop_design(&o)?)?;
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
        MobfarmCmd::QueueLoopSpots { design } => {
            let (c, spec) = load(version, &design)?;
            let o = crate::queue::loop_opts_of(&c.clock.design.meta)?;
            println!("{}", crate::queue::loop_spots(&spec, &o).map_err(|e| anyhow::anyhow!(e))?);
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
        MobfarmCmd::QueueLoopRun { design, cycles, seed, seeds, release_for, advance_at, advance_every, pulse, depart_every, stand_brake, stand_detect, stand_detect_each, intake_brake, inject_at, depart_at, depart_period, open_after, wired, golem_for, stand_reset, shift_depart, s1_at, tail0_late, bench_stand, stand_offset, release_stagger, tail_check_at, stand_detect_slots, sweep, sweep_range, corners, entry_speed, adv_shift, adv_width, skew, ideal_stand_at, tail_check_rest, tail2_at, placed_raw, placed_both, placed_in_order, placed_all, build_gap_sweep, siding, siding_at, siding_turn, siding_box, stagger_off, side_drop, side_drop_staged, side_drop_staged2, shift, stop_open, stop_close, pocket_release, pocket_reset, stop_brake_delay, stop_cv, jobs, gate_close_delay, rel_for, verbose, watch, drive_out } => {
            let (c, mut spec) = load(version, &design)?;
            let o = crate::queue::loop_opts_of(&c.clock.design.meta)?;
            if let Some(sf) = &siding {
                // (a part pasted into the loop: farmopt's bed siding, inside its box)
                let (_, part) = load(version, sf)?;
                let at = match siding_at.as_slice() { [x, y, z] => [*x, *y, *z], _ => anyhow::bail!("--siding needs --siding-at x,y,z") };
                let bx = match siding_box.as_slice() { [a, b, c, d, e, f] => ([*a, *b, *c], [*d, *e, *f]), [] => ([-7, -4, -3], [6, 4, 9]), _ => anyhow::bail!("--siding-box takes x0,y0,z0,x1,y1,z1 in the part's own frame") };
                let n = crate::queue::paste_part(&mut spec, &part, at, siding_turn, bx).map_err(|e| anyhow::anyhow!(e))?;
                eprintln!("pasted {n} blocks of {} at {:?}, turned {siding_turn}", sf.display(), at);
            }
            let intake_brake = match intake_brake.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--intake-brake takes two values: slots 1-2, then slot 0") };
            let inject_at = inject_at.or(if o.line { Some(110) } else { None });
            let wired = match wired.as_slice() { [x, y, z] => Some(redsim::pos::BlockPos::new(*x, *y, *z)), [] => None, _ => anyhow::bail!("--wired takes the input cell: x,y,z") };
            let stand_reset = match stand_reset.as_slice() { [x, y, z] => Some(redsim::pos::BlockPos::new(*x, *y, *z)), [] => None, _ => anyhow::bail!("--stand-reset takes a cell: x,y,z") };
            let stand_detect_slots = match stand_detect_slots.as_slice() { [a, b, c] => Some([*a, *b, *c]), [] => None, _ => anyhow::bail!("--stand-detect-slots takes 3 delays: S1,S2,S3") };
            let tm = crate::queue::LoopTiming { release_for, advance_at, advance_every, pulse, depart_every, stand_brake, stand_detect, stand_detect_each, intake_brake, inject_at, depart_at, depart_period, open_after, wired, golem_for, stand_reset, shift_depart, s1_at, tail0_late, bench_stand, stand_offset, release_stagger, tail_check_at, stand_detect_slots, entry_speed, adv_shift: match adv_shift.as_slice() { [a, b, c] => [*a, *b, *c], [] => [0; 3], _ => anyhow::bail!("--adv-shift takes 3 shifts") }, adv_width: match adv_width.as_slice() { [a, b, c] => [*a, *b, *c], [] => [0; 3], _ => anyhow::bail!("--adv-width takes 3 deltas") }, skew: { let mut s = [0i64; 24]; for (i, v) in skew.iter().enumerate().take(24) { s[i] = *v; } s }, ideal_stand_at, tail_check_rest, tail2_at, placed_raw, build_gap: placed_in_order.filter(|_| !placed_all && !build_gap_sweep).map(|g| if g == 0 { 20 } else { g }), build_priced: placed_in_order == Some(0), stagger_off, side_drop: match side_drop.as_slice() { [c, s, d, v] => Some(crate::queue::SideDrop { cycle: *c as i64, slot: *s as usize, at: *d as i64, speed: *v, park: *s as usize, staged: side_drop_staged || side_drop_staged2, staged2: side_drop_staged2 }), [c, s, d, v, p] => Some(crate::queue::SideDrop { cycle: *c as i64, slot: *s as usize, at: *d as i64, speed: *v, park: *p as usize, staged: side_drop_staged || side_drop_staged2, staged2: side_drop_staged2 }), [] => None, _ => anyhow::bail!("--side-drop takes C,SLOT,D,V[,PARK]") }, stop_open, stop_close, pocket_release, pocket_reset, stop_brake_delay, stop_cv, pocket_close_delay: gate_close_delay, rel_for, ..Default::default() };
            // (`--shift name=gt,...`: controls shifted as the sweep shifts them, e.g. to rerun a
            // corner without one of its controls)
            let mut tm = tm;
            for s in &shift {
                let (n, v) = s.split_once('=').ok_or_else(|| anyhow::anyhow!("--shift takes name=gt"))?;
                tm = crate::queue::loop_shifted(&tm, n, v.parse()?).map_err(|e| anyhow::anyhow!(e))?;
            }
            if !sweep.is_empty() {
                // the tolerance lines: one control at a time, to the first failure each way
                let names: Vec<String> = if sweep == ["all"] { ["release-start", "release-end", "adv1-time", "adv1-width", "adv2-time", "adv2-width", "adv3-time", "adv3-width", "s1", "s2", "s3", "stand-brake", "tail12", "tail0", "speed"].iter().map(|s| s.to_string()).collect() } else { sweep.clone() };
                let mut lines = Vec::new();
                for name in &names {
                    let l = crate::queue::loop_sweep(&spec, &o, &tm, name, sweep_range, seed, seeds, cycles).map_err(|e| anyhow::anyhow!(e))?;
                    println!("{}", l.text);
                    lines.push(l);
                }
                if corners {
                    println!("{}", crate::queue::loop_corners(&spec, &o, &tm, &lines, seed, seeds, cycles).map_err(|e| anyhow::anyhow!(e))?);
                }
                return Ok(());
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
                    // (`--jobs`: the seeds run on that many threads at once, results in seed order)
                    let mut results: Vec<(i64, String)> = Vec::new();
                    let all: Vec<i64> = (seed..seed + seeds).collect();
                    for chunk in all.chunks(jobs.max(1)) {
                        let got: Vec<(i64, Result<String, String>)> = std::thread::scope(|sc| {
                            let hs: Vec<_> = chunk.iter().map(|&s| { let (spec, o, tm) = (&spec, &o, &tm); sc.spawn(move || (s, crate::queue::run_loop(spec, o, tm, cycles, s, false, None, None))) }).collect();
                            hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
                        });
                        for (s, r) in got {
                            results.push((s, r.map_err(|e| anyhow::anyhow!(e))?));
                        }
                    }
                    for (s, r) in results {
                        let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("?").to_string();
                        let first = r.lines().skip_while(|l| !l.ends_with("problems:")).nth(1).unwrap_or("").trim().to_string();
                        if seeds > 1 {
                            println!("{label}, seed {s}: {probs} {first}");
                        }
                        if probs.starts_with("0 problems") { clean += 1 } else { bad.push(s) }
                    }
                    all_clean &= bad.is_empty();
                    println!("{label}: {clean} of {seeds} seeds clean over {cycles} slots each{}", if bad.is_empty() { String::new() } else { format!("; problems on seeds {bad:?}") });
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
            let s = crate::queue::run_loop(&spec, &o, &tm, cycles, seed, verbose, if watch.is_empty() { None } else { Some(&watch) }, drive_out.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
            println!("{s}");
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
            let o = crate::station::PadOpts { x0, x1, planes, by, h, skip };
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
        MobfarmCmd::StationStorage { hours, seed, success, official } => {
            let lines = if official { crate::station::storage_official(version, hours, seed, success)? } else { crate::station::storage_bench(hours, seed, success).map_err(|e| anyhow::anyhow!(e))? };
            for l in lines {
                println!("{l}");
            }
        }
        MobfarmCmd::StationBench { design, from, to, days, jitter, jobs, timeline, hold_delay, stand_delay, verbose } => {
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
                        let r = crate::station::bench(&spec, &lanes, &pad, &crate::station::BenchOpts { seed, days, jitter, verbose: verbose && seed == from, timeline: if seed == from { timeline.clone() } else { None }, counter: counter.clone(), hold_delay, stand_delay, wired });
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
        MobfarmCmd::Run { design, seed, days, still, terrain, ticks_only, watch, cells, census, build } => {
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
            if !watch.is_empty() {
                let cells: Vec<redsim::BlockPos> = watch
                    .iter()
                    .map(|s| {
                        let v: Vec<i32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                        if v.len() == 3 { Ok(redsim::BlockPos::new(v[0], v[1], v[2])) } else { Err(anyhow::anyhow!("--watch {s}: expected x,y,z")) }
                    })
                    .collect::<Result<_>>()?;
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
        MobfarmCmd::Probe { design, row } => {
            let (_, spec) = load(version, &design)?;
            let o = redsim::entity::mob::farm::RunOpts::steady(1, 0, redsim::entity::mob::farm::WalkMode::Random);
            let w = redsim::entity::mob::farm::build(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
            let [x0, x1, y, z] = [row[0], row[1], row[2], row[3]];
            for x in x0..=x1 {
                println!("{x} {y} {z}: {}", w.get(redsim::BlockPos::new(x, y, z)));
            }
        }
        MobfarmCmd::Edit { design, chest, how, meta } => {
            let text = std::fs::read_to_string(&design)?;
            let out = crate::layouts::edit_meta(&text, chest, how.as_deref(), &meta)?;
            std::fs::write(&design, out)?;
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
        MobfarmCmd::Exposures { design, terrain, cells } => {
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
        MobfarmCmd::Gate { design, days, builds, no_rain_day, no_thunder_day, seed, parallel, warmup, keep, trace, terrain, world, no_monsters, runs, sim_only } => {
            crate::gate::gate_cmd(version, &design, crate::gate::GateArgs { days, builds, rain_day: !no_rain_day, thunder_day: !no_thunder_day, seed, parallel, warmup: (warmup[0], warmup[1]), keep, trace, terrain, world, no_monsters, runs, sim_only })?
        }
        MobfarmCmd::WeatherStats { days, servers, dt, sim_runs } => crate::weathercheck::weather_stats_cmd(version, days, servers, dt, sim_runs)?,
        MobfarmCmd::FireBurn { design, servers, max, seeds } => crate::weathercheck::fire_burn_cmd(version, &design, servers, max, seeds)?,
        MobfarmCmd::BoltCheck => crate::weathercheck::bolt_check_cmd(version)?,
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
