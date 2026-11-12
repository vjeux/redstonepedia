//! `mctas mobfarm ...`: the generators and the judge's tools.

use anyhow::{Result, bail};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum MobfarmCmd {
    /// The pausable timers' shared lockable chain (`chainbench.rs`): writes its design (`--stages`
    /// 1-tick repeaters with side lockers on a NOT-ALIVE bus) to `--out` and prints every stage's
    /// unfrozen delay, the lag after a `--freeze-len` gt freeze begun at every phase, the 200-gt
    /// frozen control and the block count.
    ChainBench {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 80)]
        stages: i32,
        #[arg(long, default_value_t = 50)]
        freeze_len: i64,
        /// The pulse train instead (G1): this many trigger pulses (one per release) ...
        #[arg(long, default_value_t = 0)]
        pulses: i64,
        /// ... every this many gt ...
        #[arg(long, default_value_t = 100)]
        every: i64,
        /// ... each this long.
        #[arg(long, default_value_t = 10)]
        width: i64,
        /// The brief's latched start: P set at each trigger, held until stage 1 rises.
        #[arg(long)]
        pb: bool,
        /// With --pb: P clears on this stage's rise (1: a 2-gt wave; 2: 4 gt; 3: ironspec's pick, 6 gt).
        #[arg(long, default_value_t = 3)]
        pb_stage: i32,
        /// The P/B wave under one and two freezes in transit (`chainbench::bench_pb`): N 1 and 2 (the
        /// controls) and --pb-stage, the bus between gts and mid-gt, both P-clear orders.
        #[arg(long)]
        pb_transit: bool,
        /// With --pb-transit: the grid of the two-freeze pairs (first freeze's phase, gap), gt.
        #[arg(long, default_value_t = 2)]
        pair_stride: i64,
        /// A dummy repeater after the last tap (G1: the last tap's rise then EXTREMELY_HIGH like the
        /// interior stages', so every tap keeps the wave's width).
        #[arg(long)]
        terminator: bool,
        /// With --pb-transit: hold-settled drops when this stage (the release tap) rises (ironspec
        /// Nov 12: the release at stage 3; stage 1 is the control that must break); 0 = off.
        #[arg(long, default_value_t = 0)]
        release_stage: i32,
    },
    /// The controller's units digit (`ctlring.rs`, day-1 Nov 12): one 10-dropper ring, four tokens
    /// (the u = 0 one unstackable), one tap; writes the design to `--out` and pulses its clock
    /// `--cycles` times every `--period` gt (`--width` gt pulses, `--jitter` gt offsets cycling),
    /// checking the tap's level before each pulse (2 at u 0, 1 at u 3/6/7, 0 otherwise).
    CtlRingBench {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 30)]
        cycles: i64,
        #[arg(long, default_value_t = 100)]
        period: i64,
        #[arg(long, default_value_t = 4)]
        width: i64,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        jitter: Vec<i64>,
    },
    /// The controller's day counter, re-phase logic (`ctlcounter.rs`, day-1 Nov 12, ironspec's item
    /// 3): the units ring fast-forwarded to u 0 at its own step rate on RISE / trigger 2 (each at the
    /// next release), a pending-step latch for a release during the FF, the trigger-2 gate; every
    /// release phase x detector phase x starting units phase over normal days, skips and rain.
    CtlCounterBench {
        /// The FF clock's period (the ring steps correctly at >= 18 gt).
        #[arg(long, default_value_t = 18)]
        ff_period: i64,
        /// No pending-step latch (a release during the FF is absorbed).
        #[arg(long)]
        no_pend: bool,
        #[arg(long, default_value_t = 141)]
        window_open: i64,
        #[arg(long, default_value_t = 221)]
        window_close: i64,
        /// Trigger 2 accepted at C >= gate_hi or C <= gate_lo.
        #[arg(long, default_value_t = 136)]
        gate_hi: i64,
        #[arg(long, default_value_t = 15)]
        gate_lo: i64,
        /// C loaded by trigger 2 (FF: units 0 or 9; no-reset: its tens at the first wrap) and RISE.
        #[arg(long, default_value_t = 19)]
        load2: i64,
        #[arg(long, default_value_t = 0)]
        load1: i64,
        /// The first napper at the bed stop's entry this long after the window's first release.
        #[arg(long, default_value_t = 120)]
        to_bed: i64,
        /// The bed stop takes an arrival up to this long before clear beds (12,523).
        #[arg(long, default_value_t = 60)]
        early_tol: i64,
        /// The latest lie-down this long after the window opens (the worst trip).
        #[arg(long, default_value_t = 8200)]
        last_nap: i64,
        /// Picks also need the daylight level < L (the morning-pick inhibit; linear level model).
        #[arg(long)]
        day_inhibit: Option<i64>,
    },
    /// The controller's siding windows (`ctlcounter.rs::sidings`, ironspec's option 1): per lane the
    /// window's phase and chain tap centred on the predecessor's pass, from each lane's T_pb (the
    /// pocket -> branch time; `--t-pb A,B,C`, repeatable for a grid); the ring's token set.
    CtlSidingBench {
        #[arg(long, default_value_t = 141)]
        window_open: i64,
        /// T_pb for lanes A,B,C (x -1, 3, 7); repeat the flag for more rows.
        #[arg(long, value_delimiter = ',', num_args = 1)]
        t_pb: Vec<i64>,
        /// The release chain's last tap (gt after RELEASE).
        #[arg(long, default_value_t = 40)]
        a_max: i64,
    },
    /// The fast liveness ring (`ringbench.rs`): writes its design (24 rail cells, 8 detectors, 2
    /// carts) to `--out`, runs it `--ticks` gt and prints the gaps between the detectors' rising
    /// edges, the false drops of a `--stretch` gt stretcher, each detector's own spacing, the carts'
    /// speed; `--gone-at T`: the carts removed at T (the positive control: the edges stop).
    RingBench {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 240000)]
        ticks: u32,
        #[arg(long, default_value_t = 5)]
        stretch: i64,
        #[arg(long)]
        gone_at: Option<u32>,
        /// The carts' starting speed (b/t).
        #[arg(long, default_value_t = 0.4)]
        speed: f64,
        /// Empty carts on the ring (1, or 2 half a lap apart).
        #[arg(long, default_value_t = 1)]
        carts: usize,
        /// The ring sealed against mobs (solid interior, outer wall, a roof over the rails).
        #[arg(long)]
        sealed: bool,
        /// ALIVE built: a waxed copper bulb beside each detector and an observer on it.
        #[arg(long)]
        alive: bool,
    },
    /// The gravity queue's napper picker, modelled on the loop's order alone (`napper.rs`): the
    /// switch on top of the climb diverts a cart to the bed, it re-enters the tail chain after its
    /// trip; does every rider nap exactly once a night, night after night? Prints one line per picker
    /// (`--sweep`: every k from 1 to 2 x carts, both counting rules).
    NapperPicker {
        #[arg(long, default_value_t = 21)]
        carts: usize,
        #[arg(long, default_value_t = 21)]
        visits: usize,
        /// from-pick, from-return, anchored (fix (b)) or register (today's baseline).
        #[arg(long, default_value = "from-pick")]
        picker: String,
        #[arg(long, default_value_t = 11)]
        k: usize,
        /// (anchored) the first pick this many places after last night's last napper.
        #[arg(long, default_value_t = 1)]
        offset: usize,
        /// (register) a visit every this many releases.
        #[arg(long, default_value_t = 4)]
        every: usize,
        /// The napper keeps its place (its slot a travelling hole: the register).
        #[arg(long)]
        keep_place: bool,
        /// Fix (c): the sleeper re-enters after exactly M passes since its pick.
        #[arg(long)]
        gate_m: Option<usize>,
        /// Fix (c'): the lap-exact gate (re-enter right behind the cart that crossed just before it,
        /// on its next lap: carts - 1 - (nappers already away) passes).
        #[arg(long)]
        gate_lap: bool,
        /// 58f770cc's gate: re-enter just after the N-th batch following the pick passed the switch.
        #[arg(long)]
        gate_batches: Option<usize>,
        /// With --gate-lap: the picker counts each re-entry as a pass (a virtual hole).
        #[arg(long)]
        virtual_count: bool,
        /// (a negative control) Passes added to the lap-exact gate's count.
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        gate_lap_off: i64,
        /// The lap-exact gate on the virtual clock (carts - 1 virtual slots since the pick).
        #[arg(long)]
        gate_virtual: bool,
        /// The bed free only once the sleeper re-entered the loop (58f770cc's picker-sim).
        #[arg(long)]
        free_at_reentry: bool,
        /// The dusk re-arm's spread: LO,HI gt after true dusk (uniform, drawn each night).
        #[arg(long, value_delimiter = ',', default_values_t = vec![0i64, 0])]
        rearm_jitter: Vec<i64>,
        /// Fault rows: this share of cycles has no golem ...
        #[arg(long, default_value_t = 0.0)]
        golemless: f64,
        /// ... and the day counter counts golem events (else every controller beat).
        #[arg(long)]
        count_golems: bool,
        /// A freeze of LEN gt just before day D's dusk: D,LEN.
        #[arg(long, value_delimiter = ',')]
        freeze: Vec<i64>,
        /// Players sleep through night D (dayTime jumps to dawn mid-night).
        #[arg(long)]
        skip_night: Option<usize>,
        /// With --skip-night: the observer-only sentinel (the early wake refused: ALARM the next night).
        #[arg(long)]
        skip_alarm: bool,
        /// A missed (-1) or double (+1) pass edge at day D's 5th pick: D,F.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        pass_fault: Vec<i64>,
        /// The register's batch-cart index carried across nights (the built W controller; with
        /// --visits 99, no 21 cap).
        #[arg(long)]
        register_carry: bool,
        /// The batch's spacing at the pick point, gt (4 for the climb's batch; the W stand's 10).
        #[arg(long, default_value_t = 4)]
        spacing: i64,
        /// One slow visit: the night's 5th pick on every EVERY-th night takes T gt: EVERY,T.
        #[arg(long, value_delimiter = ',')]
        slow_visit: Vec<i64>,
        /// After each row, its longest zero-loss trip (fixed trips scanned up in 5-gt steps).
        #[arg(long)]
        trip_bound: bool,
        /// The carts the gate assumes when the loop holds fewer (--carts 20 --gate-carts 21: a lost
        /// cart or a stranded napper).
        #[arg(long)]
        gate_carts: Option<usize>,
        /// Cart 0 rides empty (a riderless cart carried in the loop, diverted like any pick).
        #[arg(long)]
        empty_cart: bool,
        /// The register diverts the stand's cart after its batch's summon (the built W).
        #[arg(long)]
        register_after: bool,
        /// The riderless cart's own trip LO,HI (divert to tier-clear; the tier's give-up timeout).
        #[arg(long, value_delimiter = ',')]
        empty_trip: Vec<i64>,
        /// Fix (a): releases skipped a day.
        #[arg(long, default_value_t = 0)]
        skip_per_day: usize,
        /// Event-driven cycles: a cycle starts only at a golem event or the watchdog (G1's lock-up).
        #[arg(long)]
        events: bool,
        /// The watchdog's W, live gt after the last cycle start (implies --events).
        #[arg(long)]
        watchdog: Option<i64>,
        /// A summon's pad roll: the chance an eligible batch's summon finds its golem.
        #[arg(long, default_value_t = 1.0)]
        pad_roll: f64,
        /// The batch reaches the stand this many gt after its cycle start.
        #[arg(long, default_value_t = 60)]
        arrive: i64,
        /// Event-driven cycles only from this day (a warm start: healthy fixed beats before it).
        #[arg(long, default_value_t = 0)]
        events_from: usize,
        /// A recovery picker mode (candidate): after a watchdog fire inside the night window, the
        /// picker counts K' passes instead of k for the rest of that night.
        #[arg(long)]
        recover_k: Option<usize>,
        /// Rule 4a's 600-gt self-lock: a rider whose batch made a golem at T counts again only after
        /// T + 600.
        #[arg(long)]
        lock: bool,
        /// The lock's length: free at T + this (600: tick-exact's read; 700 = the old "600 locked").
        #[arg(long, default_value_t = 600)]
        lock_gt: i64,
        /// The chance a batch member is re-marked just after its summon (misses a 600-gt turn).
        #[arg(long, default_value_t = 0.0)]
        remark: f64,
        /// Idle beats: L,I = the last I of every L beats idle (18 villagers' 7-beat: 7,1).
        #[arg(long, value_delimiter = ',')]
        idle: Vec<usize>,
        /// Q2: this many lanes in lockstep (carts / lanes each), one release line, the shared divert
        /// (or a bed per lane with --beds = lanes); 0 = Q1.
        #[arg(long, default_value_t = 0)]
        lanes: usize,
        /// Z4: beds in the bed stop (one napper each at a time).
        #[arg(long, default_value_t = 1)]
        beds: usize,
        /// Q2: the napper re-enters its lane right behind the cart that went before it there.
        #[arg(long)]
        lane_exact: bool,
        /// Q2 with one bed and per-lane counting: the lanes take the bed in sequence (1) or
        /// round-robin (2).
        #[arg(long, default_value_t = 0)]
        turns: u8,
        /// Strict rhythm: a strand outlier, one trip + ADD gt every EVERY gt of play: EVERY,ADD.
        #[arg(long, value_delimiter = ',')]
        outlier: Vec<i64>,
        /// Strict rhythm: the lift's foot to the lane hold, gt (the trip range then being the bed's
        /// cycle, entry -> R -> the lift's foot).
        #[arg(long, default_value_t = 100)]
        post: i64,
        /// Strict rhythm: the napper's foot: 0 the whole bed cycle, 1 (a) the foot gate, 2 (c) the
        /// napper-only lift (see `napper::LaneOpts::foot`).
        #[arg(long, default_value_t = 0)]
        foot: u8,
        /// Strict rhythm, --foot 1|2: R to the foot gate (a) or to the merge-level hold (c), gt.
        #[arg(long, default_value_t = 48)]
        r_leg: i64,
        /// Strict rhythm, --foot 1|2: the gate's or the hold's open window, gt after a release
        /// (mod 100): LO,HI.
        #[arg(long, value_delimiter = ',', default_values_t = vec![28i64, 72])]
        window: Vec<i64>,
        /// Strict rhythm, --foot 1: the foot gate to the evidence detector (the queue's release), gt.
        #[arg(long, default_value_t = 2)]
        det: i64,
        /// Strict rhythm: the stop's entry to the lie-down, gt.
        #[arg(long, default_value_t = 80)]
        lie: i64,
        /// Strict rhythm: also search the strand margin, the largest +ADD (a strand once a night, 10-gt
        /// steps up to 600) that loses no golem.
        #[arg(long)]
        strand_search: bool,
        /// The most nappers away at once.
        #[arg(long)]
        max_away: Option<usize>,
        /// A night skip: D,IN_DAY (the model's day starts at the re-arm, dayTime 13,670: a bed-time skip
        /// at dayTime 12,642 is 22972, in rain 22340, a daytime thunder skip at 6,000 is 16330).
        #[arg(long, value_delimiter = ',')]
        jump: Vec<i64>,
        /// The night skip every day from D.
        #[arg(long)]
        jump_every: bool,
        /// The player away (the farm unloaded): day D from DT gt after dusk for LEN gt: D,DT,LEN.
        #[arg(long, value_delimiter = ',')]
        absence: Vec<i64>,
        /// ironspec's watchdog rows (Nov 11 20:38), each W of the list: normal running with P = 0.99
        /// rolls, a 30,000-gt absence returning by day and mid-night, the first start with never-slept
        /// riders; plus the lock-up's control (no watchdog).
        #[arg(long, value_delimiter = ',')]
        watchdog_table: Vec<i64>,
        /// A nap's trip, the switch to its re-entry point: LO,HI gt (uniform).
        #[arg(long, value_delimiter = ',', default_values_t = vec![200i64, 260])]
        trip: Vec<i64>,
        /// Lie-down (LAST_SLEEP) this many gt after the pick.
        #[arg(long, default_value_t = 40)]
        sleep_at: i64,
        #[arg(long, default_value_t = 30)]
        days: usize,
        #[arg(long, default_value_t = 2)]
        warmup: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// The night window (gt; day time 13,670..22,330).
        #[arg(long, default_value_t = 8660)]
        night_gt: i64,
        /// The batch's first pass of the pick point, gt into the golem cycle.
        #[arg(long, default_value_t = 60)]
        cross_at: i64,
        /// Dusk, gt into a golem cycle (a day is 240 cycles, so it falls there every night).
        #[arg(long, default_value_t = 0)]
        dusk_at: i64,
        /// The lane model: the day time of the pick window's start (in_day 0).
        #[arg(long, default_value_t = 13_670)]
        start_dt: i64,
        /// The lane model: after a night skip, the next window opens this much later (gt; the
        /// counter re-phasing on the detector's 170..188 rise instead of 22,331: ~1,840).
        #[arg(long, default_value_t = 0)]
        rephase_lag: i64,
        /// The lane model: the window after a skip keeps only its first N releases' picks (the daytime
        /// thunderstorm skip as is).
        #[arg(long)]
        truncate_after_skip: Option<i64>,
        /// The lane model + the controller's counter (ironspec Nov 12 02:10): a skip at every day time
        /// 0..12,000 (step `--sweep-step`) plus the rain-dusk and bed-time references, as is and with
        /// the falling-8 arm: naps that night and the night after (the counter, daylight inhibit L 7),
        /// the largest LAST_SLEPT age and the golems lost to expiry (the lane model, that night cut to
        /// the counter's naps).
        #[arg(long)]
        thunder_sweep: bool,
        #[arg(long, default_value_t = 100)]
        sweep_step: i64,
        /// The lane model: a napper held at the bed queue, D,IN_DAY,ADD (the first pick at or after
        /// day D's IN_DAY gets +ADD gt on its trip).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        hold_at: Vec<i64>,
        /// The lane model, strict rhythm: the controller's siding rule (a napper rejoins only at its
        /// lane's phase window, behind the cart passing then; a late one waits a whole period).
        #[arg(long)]
        siding_window: bool,
        /// The lane model: a batch cart's time from its release to its lane's back (the stand's
        /// cycle + the return), for the lane's occupancy at every arrival.
        #[arg(long, default_value_t = 250)]
        ret: i64,
        /// The comparison: today's register, gravity's pickers, and fixes (a), (b), (c).
        #[arg(long)]
        table: bool,
        /// The given row at every k from 1 to 2 x carts.
        #[arg(long)]
        sweep_k: bool,
    },
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
        /// (onechunk Nov 12: the assembled module, its bed in the file) Flood it as a part: every
        /// villager and every --positions cell summons (the homeless static checks; a golem nothing
        /// kills, that walks out or goes through a portal reported), the bed a block like any other.
        #[arg(long)]
        part_flood: bool,
    },
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
        /// The redstone components and blocks instead of the bill (farmopt and ironspec, Nov 11):
        /// repeaters, comparators, observers, dust, torches, detector rails, powered rails (on or
        /// off at rest), redstone blocks, other redstone, gates (the controlled places, listed),
        /// rails and every block, counted from the file; per `--part` box too; the design's Nether
        /// part (its `nether layer Y:` sections: the kill room, a loader's portal) in its own
        /// "nether" column and in "whole".
        #[arg(long)]
        components: bool,
        /// With --components: a named box counted on its own column, NAME=X0,Y0,Z0..X1,Y1,Z1 in
        /// the file's cells (repeatable; a cell counts in the first box holding it, cells in no
        /// box as "rest").
        #[arg(long, allow_hyphen_values = true)]
        part: Vec<String>,
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
    /// The chunk-line crossings of a design's rails (ironspec's placement rule, Nov 11): each pair
    /// of connected rail cells in different chunk columns, both cells' block, shape and powered
    /// state as placed, and walking away from the line on each side the distance to the first
    /// climb, drop, peak and powered rail; a crossing passes with plain flat cells on both sides
    /// and no climb within 2 cells. queue-loop-run prints its summary on every run.
    Borders {
        design: PathBuf,
        /// World = design + X,Z (0,0: the design's own frame is the world's).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        offset: Vec<i32>,
        /// How far along the track each side is walked.
        #[arg(long, default_value_t = 12)]
        depth: usize,
    },
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
    /// (9fc3155f, ironspec Nov 11 21:24: the smallest one-bed farm) The rules-only envelope of each
    /// layout family (F1 stack, F2 side, F3 inverted stack, F5 close) with each queue (Q1 one lane
    /// of N waiting carts, Q2 L lanes x P): the station (v4's 3 summoners and 44 golem cells), the
    /// waiting villagers out of every lockout, the bed >= 17 from every non-napper, the envelope's
    /// box and volume, the binding part per axis, the tiling pitch per axis (out of phase).
    Envelope {
        #[arg(long, value_delimiter = ',', default_values_t = ["F1".to_string(), "F2".to_string(), "F3".to_string(), "F5".to_string()])]
        families: Vec<String>,
        /// Q1 sizes (waiting carts).
        #[arg(long, value_delimiter = ',', default_values_t = [21usize])]
        q1: Vec<usize>,
        /// Q2 sizes, LxP (lanes x waiting carts per lane).
        #[arg(long, value_delimiter = ',', default_values_t = ["3x7".to_string()])]
        q2: Vec<String>,
        /// The Pareto table instead: one row per villager count N (and family, queue, nap option):
        /// the out-of-range group min(N - 3, 15) (the batches past the floor's 6 linger in range),
        /// golems a day (51b2aacd's derived rows), the envelope, blocks, redstone, tiling area.
        #[arg(long, value_delimiter = ',')]
        villagers: Vec<usize>,
    },
    /// (ironspec 2026-11-11 12:50) The rail-junction check: every plain rail joined to three or four
    /// rails, its stored shape against the game's settle order for its signal at rest (both power
    /// states printed); exit status 1 when a stored shape would re-settle at its first update.
    JunctionCheck {
        design: PathBuf,
    },
    /// (9fc3155f, ironspec Nov 11 20:55: tiling) The cross-module margins of the design tiled with
    /// a copy of itself: per --at offset (`dx,dy,dz`, or `dx,dy,dz,m` mirrored in z first), (a) a
    /// golem on one pad sensed by the other's riders, (b) a summon marking the other's riders, (e)
    /// zombies within 6.05 of the other's riders, (f) the beds' distance, (g) chunk alignment.
    TileMargins {
        design: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        at: Vec<String>,
    },
    /// (9fc3155f, ironspec Nov 12 00:11) The static push-range scan: every cell where a cart rests
    /// or creeps (--rest cells, and every unpowered powered rail unless --no-auto; detector rails
    /// only as --rest cells) against every rail cell of another track within push range (boxes 0.98
    /// wide and 0.7 tall, the resting one inflated 0.2 in x and z and 0.5 along its axis, y ranges
    /// overlapping; a slope's box spans its rise). Then the rail-T scan (each 3-connection plain
    /// rail's reaching signal-source updates, from redsim's block model), the switched-cell scan
    /// (every source powering each switched cell) and the placement-order scan (every rail vanilla
    /// placement would not give its designed shape, with the fix).
    PushScan {
        design: PathBuf,
        /// A rest or creep cell x,y,z (repeatable).
        #[arg(long, allow_hyphen_values = true)]
        rest: Vec<String>,
        /// Only the --rest cells.
        #[arg(long)]
        no_auto: bool,
        /// The rail-T scan's declared switch x,y,z (repeatable): a signal source meant to flip
        /// the T beside it; every other source beside a T is undeclared.
        #[arg(long, allow_hyphen_values = true)]
        switch: Vec<String>,
        /// Print the rail-T scan's positive controls (one per source type) first.
        #[arg(long)]
        controls: bool,
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
        /// A POI ticket taken before the check (`x,y,z`, repeatable: the shared bed taken, a
        /// sentinel's bed): the strolls' village bias for that bed state (9fc3155f, ironspec Nov 11
        /// 19:18).
        #[arg(long, allow_hyphen_values = true)]
        claim: Vec<String>,
        /// Print every place's stamp-arming class (arms and clears / arms, never clears / never
        /// arms), not only the "arms, never clears" places.
        #[arg(long)]
        classes: bool,
        /// The golem-proof clearing cells for a place (`x,y,z`, repeatable): every glass floor
        /// within 2 sideways and 2 up or down that turns its class from "arms, never clears".
        #[arg(long, allow_hyphen_values = true)]
        fix_search: Vec<String>,
        /// With --fix-search: also every PAIR of candidate glass floors (a two-cell walkway).
        #[arg(long)]
        fix_pairs: bool,
        /// DIAGNOSTIC; e6 positive control not met (the runner's 1,019-gt e6 stamp on W is cleared
        /// within a few gt by these static roles). The path-integrated stamp age along a rider's
        /// path: a `queue-loop-run --watch K` file (one cart position a gt); each stamp's arming
        /// cell, clearing cell and age, the worst against 1,000 gt.
        #[arg(long)]
        path_watch: Option<PathBuf>,
        /// DIAGNOSTIC (see --path-watch). With --path-watch: every cell change between these two
        /// path labels' ticks (`c287+0,c297+40`: slot and dt) with its role (arms / clears /
        /// neither) and its draws' reachable shares.
        #[arg(long)]
        path_trace: Option<String>,
        /// The scarer search (ironspec Nov 11 22:53): candidate post cells in this box
        /// x0,y0,z0,x1,y1,z1 (an air cell with air above and a block below), each scored on the rider
        /// points it covers (a zombie in a boat 6.05..7.95 in sight, refused with a rider point
        /// inside 6.05; a pillager 8..15 in sight, refused when it sees a rider point within 8);
        /// prints a greedy cover and its entity lines.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        scarer_search: Vec<i32>,
        /// The scarer --scarer-search places: zombie (in a boat) or pillager.
        #[arg(long, default_value = "zombie")]
        scarer: String,
        /// With --scarer-search: an extra rider's feet x,y,z (repeatable; a hold's or a fall's cart
        /// position, seat 0).
        #[arg(long, allow_hyphen_values = true)]
        scarer_rider: Vec<String>,
        /// With --scarer-search: at most this many posts.
        #[arg(long, default_value_t = 12)]
        scarer_max: usize,
        /// With --scarer-search: a post brings its own floor (any air cell with air above is a
        /// candidate; the added floor block is left out of the sight test).
        #[arg(long)]
        scarer_any_floor: bool,
        /// With --scarer-search: each post with its stone roof and two-high walls on three sides in
        /// the sight test (the four open sides tried, the best kept; the post's blocks printed).
        #[arg(long)]
        scarer_walls: bool,
        /// With --scarer-search: only the rider points whose labels contain one of these count
        /// toward the cover (repeatable: e.g. a place `9,0,6` or `rider 9.50`); every point still
        /// counts for the refusals (the posts already laid cover the rest).
        #[arg(long)]
        scarer_target: Vec<String>,
        /// With --scarer-search: the exact least cover over both kinds' posts (pillagers and zombie
        /// posts, every valid open side, every refusal rule), up to this many posts: the fewest
        /// posts, the fewest pillagers, and pillagers only (--scarer and --scarer-max ignored).
        #[arg(long)]
        scarer_exact: Option<usize>,
        /// The laid scarers' check: a post x,y,z,pillager|zombie (repeatable; the boat's cell, its
        /// blocks already in the design), seated, against every rider point (the design's rails and
        /// --scarer-rider): the refusals, what each covers, the places no post covers.
        #[arg(long, allow_hyphen_values = true)]
        scarer_check: Vec<String>,
        /// With --scarer-exact / --scarer-check: a declared transient place x,y,z (repeatable; a
        /// rail cell no resting rider holds, ironspec's ruling): its points are not required, and
        /// the output names it as flee-check's exception.
        #[arg(long, allow_hyphen_values = true)]
        scarer_transient: Vec<String>,
        /// With --scarer-exact: the mix (scarer mobs are their own bill line): from the fewest
        /// posts' pillagers down, the least cover with at most p pillagers, each passing the built
        /// re-check inside the search (every post laid at once).
        #[arg(long)]
        scarer_mix: bool,
        /// With --scarer-exact and --golem-pad: a post a pad golem sees within its 16 follow range
        /// is shielded by a visor instead of refused (ironspec Nov 12 03:54): glass on every such
        /// golem's eye ray to the post's mob and on no covered rider's ray, inside the box and out
        /// of every excluded box (a greedy hitting set), every golem cell and covered point
        /// re-checked with it laid; its cells are printed with the post and laid in the built
        /// re-check.
        #[arg(long)]
        scarer_visor: bool,
        /// With --scarer-search: a box x0,y0,z0,x1,y1,z1 no post (with its floor and its head
        /// cell) may touch (repeatable: other parts' space, the pad's spawn columns, rider cells).
        #[arg(long, allow_hyphen_values = true)]
        scarer_exclude: Vec<String>,
        /// A declared pillager seated in a boat, the BOAT's position x,y,z (repeatable; redsim does
        /// not spawn pillagers; G1's review: its feet at boat y + 0.1875 - 0.6, its eye 1.6575 over
        /// them): a panic source within 15 of a rider point when seen, fled from within 6.
        #[arg(long, allow_hyphen_values = true)]
        pillager: Vec<String>,
        /// Instead: whether an iron golem standing on any cell of --golem-pad has a path to this
        /// cell x,y,z (within 2: close enough to strike a scarer seated there).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        golem_reach: Vec<i32>,
        /// With --golem-reach: the golems' feet cells x0,y0,z0,x1,y1,z1 (the pad).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        golem_pad: Vec<i32>,
        /// With --scarer-search: the near bar instead of the kind's (8 for a pillager): a margin,
        /// e.g. 8.5 (ironspec Nov 11 23:57).
        #[arg(long)]
        scarer_near: Option<f64>,
        /// Instead: a post's drift: a boat with a seated zombie (the stand-in) at this cell x,y,z,
        /// ticked --drift-ticks times; its position spread.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        post_drift: Vec<i32>,
        /// With --post-drift: the ticks.
        #[arg(long, default_value_t = 24000)]
        drift_ticks: u32,
        /// The floor block --fix-search places, refused unless golem-proof (LEGACY_IRON_GOLEM's
        /// excluded list: glass, stained or tinted glass, panes, glowstone, sea lantern, leaves,
        /// ice...); default glass (not a stroll destination, which needs a solid-render block below;
        /// glowstone and sea lantern are both golem-proof and solid-render).
        #[arg(long, default_value = "minecraft:glass")]
        fix_block: String,
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
        /// Only the part's blocks, block entities and entities inside this box of the part's own
        /// frame (X0,Y0,Z0,X1,Y1,Z1, inclusive): a copy of one area of a design (a second stand
        /// area pasted into its own design, say).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        r#box: Vec<i32>,
        /// The part's overworld cells only: no ports, no Nether part, no build, station, declared
        /// or flights lines (a copy that shares the base's controls and Nether side).
        #[arg(long)]
        overworld_only: bool,
        /// The part mirrored in z (z to -z, then moved by --at), its blocks' north/south
        /// properties swapped (facing, rail shapes, wire sides): a stand copied facing the other way.
        #[arg(long)]
        mirror_z: bool,
        /// `--keep base`'s older name (S3's recipes, wip/s3/README.md): a contested cell keeps the
        /// base's block, listed.
        #[arg(long)]
        keep_base: bool,
        /// One box of the base cleared first (X0,Y0,Z0,X1,Y1,Z1, the base's frame): `--clear` with
        /// one box, by its older name (S3's recipes, wip/s3/README.md).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        remove_box: Vec<i32>,
        /// Set (or move) one of the result's ports, NAME=x,y,z (repeatable): a moved part's port
        /// (the bed's `home`) in the base's frame.
        #[arg(long, allow_hyphen_values = true)]
        set_port: Vec<String>,
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
    /// (onechunk 2026-11-11, ironspec 20:27) The golem chance per summon
    /// (`columnrun::summon_chance`): for each villager riding a cart over a --stand cell, its 289
    /// summon columns (the game's LEGACY_IRON_GOLEM scan, checkSpawnObstruction with the file's
    /// carts in place): n_i landing on --pad and passing, off-pad passes, wasted landings, no-floor
    /// columns; P = 1 - prod (1 - n_i/289)^10 and golems a day.
    SummonChance {
        design: PathBuf,
        /// The pad's spawn cells (feet), x0,y0,z0,x1,y1,z1 in the file's frame.
        #[arg(long, allow_hyphen_values = true)]
        pad: String,
        /// A stand rest cell x,y,z (repeatable), the file's frame.
        #[arg(long, allow_hyphen_values = true)]
        stand: Vec<String>,
        /// Summons a day.
        #[arg(long, default_value_t = 238.0)]
        per_day: f64,
    },
    /// (onechunk 2026-11-12; ironspec 00:02/00:16, tick-exact's linking traps) Every portal's
    /// search as the game runs it (`mobfarm::portallinks`): from each overworld portal (its cells,
    /// absolute) the Nether portals within 16 of its x 1/8 point, 3D-closest, ties to the lower y;
    /// from each Nether portal the overworld ones within 128 of its x 8 point; the candidates with
    /// their distances, the chosen portal per target, the expected partner and its least margin,
    /// and any empty search (a portal created there).
    PortalLinks {
        /// An overworld portal NAME=x0,y0,z0,x1,y1,z1[;...] (its cells), repeatable.
        #[arg(long, allow_hyphen_values = true)]
        ow: Vec<String>,
        /// A Nether portal NAME=x0,y0,z0,x1,y1,z1[;...], repeatable.
        #[arg(long, allow_hyphen_values = true)]
        nether: Vec<String>,
        /// The intended partner SOURCE=TARGET (either direction), repeatable.
        #[arg(long)]
        expect: Vec<String>,
        /// The entity's centre anywhere over a cell grown this much on x and z (a golem: 0.7).
        #[arg(long, default_value_t = 0.7)]
        margin: f64,
    },
    /// (onechunk 2026-11-11/12; farmopt 22:22, tick-exact's code reads, G1's CHANGES 4) L1‴, the
    /// item-loop loader, benched in redsim (`mobfarm::l1items`): a same-axis portal pair; on each
    /// side a firing dropper at y 12 or 13 (`--fire-y`) fed by up-facing droppers, the exit face's
    /// bottom two rows closed, packed ice and a hopper row past the frame, a far-face wall
    /// (`--far-rows`), a backstop, a glass enclosure and a REAL observer clock; dedicated items.
    /// Over many seeds (threads): failures with their Wilson interval, each side's worst gap
    /// against the 300-gt ticket, items lost or held by a mob, items held in a portal, the landing
    /// spread against the floor and backstop and each arrival's drift from its column's centre,
    /// the flight's clearance to the frame and to a bottom-row mob's reach (standing and jumping).
    L1ItemBench {
        #[arg(long)]
        design_out: Option<PathBuf>,
        /// The portals' interior width (each side fires from its own column).
        #[arg(long, default_value_t = 5)]
        width: i32,
        /// The portals' interior rows (3: the firing dropper's y 12 is the top row; 4: a row over
        /// it, room for a slow item's arc).
        #[arg(long, default_value_t = 5)]
        height: i32,
        /// The firing dropper's y: the top row (13 in 4 rows, 14 in 5: the ruling); the pocket floor 2 under it.
        #[arg(long, default_value_t = 14)]
        fire_y: i32,
        /// The overworld far face's walled rows from the bottom (default 2: the bottom two, the trim;
        /// the Nether far face is closed in every row).
        #[arg(long)]
        far_rows: Option<i32>,
        /// The pocket floor: hoppers (no spawnable floor: the ruling) or ice (packed ice: redsim's
        /// slippery block, the 4-row variant).
        #[arg(long, default_value = "hoppers")]
        floor: String,
        #[arg(long, default_value_t = 4)]
        items: u32,
        /// Game ticks a seed.
        #[arg(long, default_value_t = 24000)]
        ticks: i64,
        /// Seeds: N (1..N) or A..B.
        #[arg(long, default_value = "1")]
        seeds: String,
        /// Redsim's level-32 freeze (entities skip; blocks and block entities run) D gt from T,
        /// in WORLD (ow, nether or both; default both): T,D[,WORLD], repeatable.
        #[arg(long)]
        freeze: Vec<String>,
        /// A 3-gt login freeze of both worlds every N gt.
        #[arg(long)]
        login_each: Option<i64>,
        /// A 3-gt login freeze of both worlds starting at every dispense (the item crosses after
        /// the thaw: the login lands just before each arrival).
        #[arg(long)]
        login_before_arrival: bool,
        /// Both worlds unloaded at T and loaded again at T + 1 (a full stop), repeatable.
        #[arg(long)]
        stop: Vec<i64>,
        /// A full stop every N gt, each one 1 gt later in the cycle than the last (stops at N,
        /// 2N + 1, 3N + 2, ...: every phase).
        #[arg(long)]
        stop_every: Option<i64>,
        /// The restarts' entity load order: save, reverse, seed:K, or all (the 10 orders of
        /// liveness::Restart::rows(), one pass each).
        #[arg(long, default_value = "save")]
        restart_order: String,
        /// A mob (loot pickup on, empty hand) placed in a bottom row at column X at time T, on
        /// SIDE (a, b or both; default both), KIND adult (default), baby or jockey (a baby on a
        /// chicken), armed `sword` (a golden sword in hand, as portal piglins spawn):
        /// T,X[,SIDE[,KIND[,sword]]], repeatable (AI on).
        #[arg(long)]
        piglin: Vec<String>,
        /// Every N gt each mob on the ground jumps (a forced jump: its box toward ~13.2).
        #[arg(long)]
        mob_jump: Option<i64>,
        #[arg(long)]
        trace: bool,
    },
    /// (onechunk 2026-11-11; farmopt 22:07, ironspec's self-contained loaders) L1′, the cart-loop
    /// loader, benched in redsim (`mobfarm::l1bench`): writes its design (--design-out) or runs it:
    /// the overworld portal and its Nether twin, each side an exit track into a 2-pocket queue, 3
    /// empty carts. EMULATED (labelled): pocket 1's release pulse (also powering the release line's
    /// two boosters), pocket 2's 2-gt nudge once pocket 1 and the line are clear, each release
    /// timed from that cart's own arrival. Each side's arrivals against the 300-gt ticket, the
    /// Nether's ticket lapsing, carts held inside a portal.
    L1Bench {
        /// Write the bench design here and run it.
        #[arg(long)]
        design_out: Option<PathBuf>,
        #[arg(long, default_value_t = 3000)]
        ticks: i64,
        #[arg(long, default_value_t = 320)]
        hold: i64,
        #[arg(long, default_value_t = 20)]
        pulse: i64,
        /// The first releases (A's pocket 1, B's pocket 1, A's pocket 2), gt after the start: phases
        /// 0, 227, 453 of a ~680-gt cart cycle (each side then gets an arrival every ~227 gt).
        #[arg(long, default_value = "5,113,232")]
        stagger: String,
        /// Redsim's level-32 freeze D gt from T in WORLD (ow, nether or both): T,D[,WORLD].
        #[arg(long)]
        freeze: Vec<String>,
        /// A zombified piglin placed standing in A's bottom portal row (the inbound column) at T
        /// (AI on), repeatable.
        #[arg(long)]
        piglin: Vec<i64>,
        #[arg(long)]
        trace: bool,
    },
    /// (onechunk 2026-11-11; farmopt 19:14, ironspec 19:16) A gravity column run from its own
    /// file (`mobfarm::columnrun`): the file built as placed, its header's run cells
    /// (`run-gates:`, `run-cells:`, `run-controller:`), T at every world time multiple of 100, the
    /// design's controller driving its gates (every drive EMULATED and labelled), and the measures:
    /// new cart-cart contacts with both stored speeds (bar 0.2), gate shuts onto a cart, each
    /// cart's passes over the named detectors, every rider inside a stand summoner's 10-box at T.
    ColumnRun {
        design: PathBuf,
        /// Cycles of 100 gt measured after the warm-up.
        #[arg(long, default_value_t = 10)]
        cycles: i64,
        /// Warm-up ticks before the first measured T (the file's carts settle into place).
        #[arg(long, default_value_t = 200)]
        warm: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// The stand's arrival fill: a gate shuts when its k-th cart's centre is this far past the
        /// gate cell's west face.
        #[arg(long, default_value_t = 1.6)]
        close_at: f64,
        /// The departure: S1's and S2's slopes powered this long after their gates open
        /// (dfb89499's slope delay).
        #[arg(long, default_value_t = 6)]
        slope_delay: i64,
        /// The departure: S2 opens this long after S0 and S1 (dfb89499's G2 +2).
        #[arg(long, default_value_t = 2)]
        g2_plus: i64,
        /// The hold's release: Ga shuts no sooner than T+3+4 plus this, once the hold is empty.
        #[arg(long, default_value_t = 0)]
        hold_shut_delay: i64,
        /// The tail's moves: each gate's pulse (gt) once its pocket's cart is at rest and the
        /// next pocket is empty.
        #[arg(long, default_value_t = 12)]
        pulse: i64,
        /// Laps printed: the first carts released after the warm-up, each cart's detector passes.
        #[arg(long, default_value_t = 3)]
        laps: usize,
        /// Print every control event.
        #[arg(long)]
        trace: bool,
    },
    /// (onechunk 2026-11-11, vjeux's "Can we have the system in one chunk?") The one-chunk
    /// column's skeleton (`mobfarm::column`): each --part's committed design, or its cells in a
    /// kept box, moved into the column's frame and stacked into its tier, nothing wired; prints
    /// each part's committed box and the chunk lines it crosses, its placed box and where it
    /// leaves the column, the cells two parts hold, the tiers top down, the column's box and
    /// blocks³; --out writes the skeleton (`mobfarm bill` and `mobfarm exit` read it).
    Column {
        /// A part, NAME=FILE[@fx,fy,fz][^tx,ty,tz][#x0,y0,z0,x1,y1,z1]: `@` the move from the
        /// file's frame into the column's, `^` from its committed place to its tier, `#` the
        /// cells kept (the file's frame, inclusive). `NAME!=` re-lays cleared cells; `NAME~=` takes
        /// the file's Nether part as the part's cells (a Nether layout). Repeatable; the first part
        /// keeps its ports.
        #[arg(long = "part", allow_hyphen_values = true)]
        parts: Vec<String>,
        /// A reserved box NAME=x0,y0,z0,x1,y1,z1 (the column's frame): a part not yet laid out for
        /// the column (a fold its owner re-generates, a run not benched yet); in the fit, clash and
        /// tier report and the column's box, holding no block. Repeatable.
        #[arg(long = "reserve", allow_hyphen_values = true)]
        reserves: Vec<String>,
        /// Every `# reserved (no blocks): NAME x a..b y c..d z e..f` header line of this design
        /// file (a budget) as a `--reserve`, but the names in `--reserve-skip` (the parts laid).
        #[arg(long)]
        reserves_from: Option<PathBuf>,
        /// A reserved box of `--reserves-from` left out (its part is laid). Repeatable.
        #[arg(long)]
        reserve_skip: Vec<String>,
        /// A port set on the skeleton, NAME=x,y,z (the judge's `port output` when no kept part
        /// cell holds one); repeatable, replaces a part's port of that name.
        #[arg(long = "port", allow_hyphen_values = true)]
        ports: Vec<String>,
        /// A sketch track NAME=x,y,z;x,y,z;... (the column's frame): plain rails through the
        /// waypoints (each leg along x or z, its height change one block a cell from the leg's
        /// start), a stone under each, curves at corners; it stands in for a lane's cells until
        /// they land. Repeatable.
        #[arg(long = "track", allow_hyphen_values = true)]
        tracks: Vec<String>,
        /// A cart on a sketch rail cell, x,y,z (ridden by a villager) or x,y,z,empty. Repeatable.
        #[arg(long = "cart", allow_hyphen_values = true)]
        carts: Vec<String>,
        /// A zombie post at its boat's cell x,y,z (the brief's: glass under, a persistent zombie in
        /// an oak boat, leaves 3 above). Repeatable.
        #[arg(long = "post", allow_hyphen_values = true)]
        posts: Vec<String>,
        /// A box x0,y0,z0,x1,y1,z1 (the column's frame) whose file-part blocks, block entities,
        /// entities and declared cells are taken out before the sketch is laid (a committed cell a
        /// lane re-lays). Repeatable.
        #[arg(long = "clear", allow_hyphen_values = true)]
        clears: Vec<String>,
        /// The column's cells x0,z0,x1,z1 in the frame.
        #[arg(long, default_value = "-6,-6,9,9", allow_hyphen_values = true)]
        column: String,
        /// The chunk lines' phase PX,PZ: lines at x = PX + 16k and z = PZ + 16k (the loop
        /// frame's at the farm's alignment).
        #[arg(long, default_value = "-6,-6", allow_hyphen_values = true)]
        chunk_phase: String,
        /// Each part's top view as committed in the frame.
        #[arg(long)]
        plan: bool,
        #[arg(long, default_value = "onechunk-skeleton")]
        name: String,
        /// Leave every rail junction as stored (by default each is set to the shape its as-placed
        /// power settles it to and listed, as `mobfarm paste` does).
        #[arg(long)]
        no_settle: bool,
        /// The skeleton's `ground:` (the design y lying at the natural ground layer, in the
        /// column's frame): `mobfarm bill --terrain` then digs the column in under it.
        #[arg(long, allow_hyphen_values = true)]
        ground: Option<i32>,
        /// Write the skeleton moved by dx,dy,dz (the farm's aligned frame: loop + 6,0,6 puts the
        /// column in world x, z 0..15, chunk lines every 16 from 0, as the runner, `borders` and
        /// `paste` read a file); the header says the shift. The report stays in the column's frame.
        #[arg(long, allow_hyphen_values = true)]
        out_shift: Option<String>,
        /// Drop every cart carrying a villager from the written skeleton (zombies and empty carts
        /// stay), so `mobfarm exit --positions` sets the summoners one pocket at a time.
        #[arg(long)]
        no_riders: bool,
        /// Write every villager position of the skeleton to this file as `x y z` lines in the
        /// written frame (`--out-shift` applied): each rider's cell, every rail cell of every part (a
        /// rider passes it) and every declared off-rail path cell (`--path`), for `mobfarm exit
        /// --positions` (the flood with a summoner at every villager position) and `site-check
        /// --positions`.
        #[arg(long)]
        positions_out: Option<PathBuf>,
        /// A declared off-rail rider path NAME=x,y,z..x,y,z[..x,y,z] (the column's frame; each
        /// consecutive pair a straight run of cells, ends included): a drop shaft, a bubble-column
        /// lift, a fall, a hold with no cart placed (G1: a cart off the rails is invisible to the
        /// rail cells). In `--positions-out` and `--bed` (judged like a part; `--bed-exempt NAME`).
        #[arg(long, allow_hyphen_values = true)]
        path: Vec<String>,
        /// A header line written into the skeleton (`# ` added): the joints table, a ruling.
        /// Repeatable, in order.
        #[arg(long = "note")]
        notes: Vec<String>,
        /// The occupied shared bed's head x,y,z (the column's frame): print the bed radius per
        /// part (ironspec Nov 11 18:02: every rider position more than 16 + 1 from its centre,
        /// the least over every track position; `ValidateNearbyPoi`'s 16) and exit 1 on a FAIL.
        #[arg(long, allow_hyphen_values = true)]
        bed: Option<String>,
        /// A part exempt from the bed radius (the napper's own path): printed, not judged.
        /// Repeatable (a sketch track's part is `track:<name>`).
        #[arg(long = "bed-exempt")]
        bed_exempt: Vec<String>,
        /// The sentinel villager's room x0,y0,z0,x1,y1,z1 (the column's frame): print its rule (c)
        /// (the least largest-axis gap to every villager position's box, the bar 11.7: out of
        /// `spawnGolemIfNeeded`'s inflate(10)) and its chunk section against the bed's (in the
        /// written frame, `--out-shift`); exit 1 under the bar.
        #[arg(long, allow_hyphen_values = true)]
        sentinel: Option<String>,
        /// Print condition (a) over every villager position (riders and rail cells): per y phase,
        /// how many lie more than 1 section (Chebyshev) from every bed's section (the shared bed,
        /// `--bed`, and the sentinel's room, `--sentinel`), in the written frame (`--out-shift`).
        #[arg(long)]
        sections: bool,
        /// A summoner's cell x,y,z (repeatable: the stand's pockets): print every other villager
        /// position inside its box grown by 10 on every axis (`spawnGolemIfNeeded`'s test; each
        /// gets the summon mark) with its least gap to a golem at `--golem-box` (17.84 or less on
        /// every axis: golem-locked while one exists).
        #[arg(long = "summon-box", allow_hyphen_values = true)]
        summon_box: Vec<String>,
        /// The pad's golem spots as feet cells x0,y0,z0,x1,y1,z1 (the column's frame).
        #[arg(long, allow_hyphen_values = true)]
        golem_box: Option<String>,
        /// A runner gate for `column-run`: NAME=x,y,z[/sx,sy,sz] (the fence gate, and the block
        /// powering the powered rail under it), in the column's frame (shifted with the file).
        #[arg(long = "run-gate", allow_hyphen_values = true)]
        run_gate: Vec<String>,
        /// A runner cell for `column-run`: NAME=x,y,z (a detector rail it reads, or a rest cell).
        #[arg(long = "run-cell", allow_hyphen_values = true)]
        run_cell: Vec<String>,
        /// The runner's controller for `column-run` (v4).
        #[arg(long)]
        run_controller: Option<String>,
        /// More header notes, one per line of this file (as --note).
        #[arg(long)]
        notes_file: Option<PathBuf>,
        /// A pocket chain NAME=x,y,z,DIR,N[,rstar]: N of the stand's pockets in a row heading DIR
        /// from the first P at x,y,z (pitch 5, net -1 a pocket), gates and runner cells included.
        #[arg(long, allow_hyphen_values = true)]
        pockets: Vec<String>,
        /// A single block x,y,z=STATE (repeatable).
        #[arg(long, allow_hyphen_values = true)]
        block: Vec<String>,
        /// A box of waiting places x0,y0,z0,x1,y1,z1 (repeatable): every rail cell inside is checked
        /// against the pad's golem sensor boxes (--golem-box).
        #[arg(long = "wait-box", allow_hyphen_values = true)]
        wait_box: Vec<String>,
        /// Where the skeleton goes (none: the report alone).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// A design's overworld box: its extents, volume, the chunk columns it crosses (X >> 4,
    /// Z >> 4 of its cells as written) and its blocks by kind, within `--box` if given.
    Box {
        design: PathBuf,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        r#box: Vec<i32>,
        /// Every block in the box too, one per line: x y z state (the design's own frame)
        #[arg(long)]
        list: bool,
    },
    /// Where a stand's zombie can stand under the 6-8 rule (ironspec 2026-11-11): every candidate
    /// cell of `--region` (a zombie's feet at its centre, y as given), scored against every rider
    /// position the flee check tries (three points along each rail cell, the cart's y: `fleecheck`)
    /// and the resting riders `--rest` (x,y,z,...): the nearest rider position (bar > 6), the
    /// farthest resting rider (bar <= 8), and whether a straight line from each resting rider's eye to
    /// the zombie's head crosses only see-through blocks. The best `--top` printed.
    ZombieCells {
        design: PathBuf,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        region: Vec<i32>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        rest: Vec<f64>,
        #[arg(long, default_value_t = 12)]
        top: usize,
    },
    /// A part made of straight runs, to price a route before it is designed: each option a list of
    /// segments `x0,y0,z0>x1,y1,z1` (`;`-separated; along x or z, y moving one block per cell toward
    /// y1 until it gets there) laid as plain rails on glass (`--rail`), boosters on redstone blocks
    /// (`--booster`) or dust on stone with a repeater every 15 cells (`--wire`), plus single blocks
    /// (`--block "x,y,z=state;..."`). Written as a design to paste (`paste --overworld-only`).
    Lines {
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        rail: String,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        booster: String,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        wire: String,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        block: String,
        #[arg(long)]
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
        /// With --approach 5: C''', a plain hump after the run's --boost-first boosters, its flat
        /// top this many blocks above the dip; the dip's plain flat across the line z 9|10, a plain
        /// climb to T (6 high at z 4), T and approach 5's descent; the queue's head moves north
        /// as far as the run needs.
        #[arg(long, default_value_t = 0)]
        hump: u32,
        /// With --hump: the dip's height (2: z 9 flat, four slopes up to T).
        #[arg(long, default_value_t = 2)]
        dip_y: u32,
        /// With --hump: the dip's flat cells, z 9 and north.
        #[arg(long, default_value_t = 2)]
        dip_flat: u32,
        /// With --hump: the run's boosters as the hump's first slopes up (not flat).
        #[arg(long)]
        boost_slopes: bool,
        /// With --boost-slopes: the first this many boosters flat at the head's height, the rest as
        /// slopes (S3's DP cell two before the hump's top clear of the boosters; at hump 4, 1).
        #[arg(long, default_value_t = 0)]
        boost_flat: u32,
        /// With --hump: the hump's flat top this many cells long.
        #[arg(long, default_value_t = 1)]
        top_flat: u32,
        /// With --hump: the top's flat cells boosters (backward carts never reach them).
        #[arg(long)]
        top_boost: bool,
        /// The hump's path with no hump (ironspec's h0, with --approach 5): the run's boosters,
        /// `--top-flat` plain flat cells at the run's level (S's), one slope up to the dip at
        /// `--dip-y`, the dip's flat across the line z 9|10, the boosted climb.
        #[arg(long)]
        dip_run: bool,
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
        /// S2, the comb stand (with --stand-stop --approach 5, not --stand-wired): each pocket on
        /// its own branch through stem-in switches flipped by single-cart detector edges
        /// (`mobfarm::comb`): 1 the stem-in S_y, 2 the straight S_y (wrong-side entry, pending
        /// its bridge). Emulation: the latches and gates bench-driven, the junctions real.
        #[arg(long, default_value_t = 0)]
        stand_comb: u32,
        /// With --stand-comb: the gravity pockets (gate slopes plain, the cells past G2 and G1
        /// detector rails).
        #[arg(long)]
        comb_gravity: bool,
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
        /// Built into this terrain sample (village-edge, plains, forest) at the judge's cheapest
        /// spot, or at `--at` / `--dug-in`; without it, the void.
        #[arg(long)]
        terrain: Option<String>,
        /// A fixed spot in the terrain sample, a control: the footprint's min column at world
        /// X,Z, its ground layer on the highest surface over it, no flatness rule ("fixed spot,
        /// not the judge's"; the relief and the dig printed).
        #[arg(long, allow_hyphen_values = true, conflicts_with = "dug_in")]
        at: Option<String>,
        /// The dug-in spot finder (the parent's ruling 2026-11-11): every farm block under the
        /// natural surface with `--cover`, the cheapest spot by the bill's minutes clear of village
        /// POIs; `--cover b` (default) the lightning rule (b)'s depth, `--cover 1` one block.
        #[arg(long)]
        dug_in: bool,
        #[arg(long, default_value = "b")]
        cover: String,
        /// More villager positions: files of `x y z` lines in design cells (`queue-loop-run
        /// --record-positions` over a full bench night), as queue-loop-spots reads them.
        #[arg(long, value_delimiter = ',')]
        positions: Vec<PathBuf>,
        /// Every rail cell of the design is a villager position too.
        #[arg(long)]
        rails: bool,
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
    /// (ironspec 2026-11-11, vjeux's stage correction: the carousel is the endgame farm) The sky
    /// variant: the module floating over a terrain sample, nothing on the natural surface, at the
    /// spot needing the least scaffold (every villager position 8 above the highest natural block
    /// within 8 columns, the box above everything natural under it), with an opaque roof of
    /// bottom slabs outside every strike's box (the footprint and 3 around, or the footprint and
    /// one lightning rod: `--rod`), a daylight detector on it, the boxes of `--remove` (the
    /// sentinel room) taken out; prints what it adds and removes, writes the sky design (its
    /// `ground:` at the natural ground under it, for `mobfarm bill`'s height rows).
    /// (ironspec 2026-11-11, vjeux's compact one-bed plan) A module's siting cost from its envelope
    /// alone (`mobfarm::siting`, for farmopt's envelope tool): the roof, the rod, the time signal,
    /// its own spawn-proofing, the pearl loader, the ground light and fence ring on the surface,
    /// the height's trips and edge placements, at a stage's prices; sky and surface, each with and
    /// without a rod.
    Siting {
        /// The module's box, W x D x H (blocks).
        #[arg(long)]
        envelope: String,
        /// The blocks it places.
        #[arg(long)]
        blocks: u32,
        /// Its own dark floors a monster stands on.
        #[arg(long, default_value_t = 0)]
        floors: u32,
        /// Its lowest ranged target's level over its floor (a villager position or a golem cell).
        #[arg(long, default_value_t = 0)]
        low: i32,
        /// Its highest villager position's level over its floor (the roof goes 6 over it; default:
        /// the box's top already clears it).
        #[arg(long, default_value_t = i32::MIN / 2)]
        high: i32,
        /// The share of its blocks placed at an edge in the sky (v4 C': 0.68).
        #[arg(long, default_value_t = 0.68)]
        edge: f64,
        /// The chunks it covers (default: its footprint on the chunk grid).
        #[arg(long)]
        chunks: Option<u32>,
        /// A shared loader's blocks a module instead of a pearl stasis cell per chunk (the
        /// ring-free plan's item loop, ~18).
        #[arg(long)]
        loader_blocks: Option<u32>,
        /// Its other trips, each to its mid-height (scarers placed, lava trips).
        #[arg(long, default_value_t = 0)]
        extra_trips: u32,
        /// Its own dark floors lit, one torch per this many floors, instead of a slab on each.
        #[arg(long)]
        floor_torch: Option<u32>,
        /// The modules in an array (9fc3155f's tiling), sharing the rod and the surface's lit
        /// ground and fence ring; the rows are a module's share.
        #[arg(long, default_value_t = 1)]
        array: u32,
        /// Sky: every target this high over the highest natural block within this many columns (8:
        /// the golem search's band; 16: the golem cells out of a ground skeleton's bow, the
        /// villagers out of a pillager's sensing, 15 each).
        #[arg(long, default_value_t = 8)]
        range: i32,
        /// Sky: the highest natural block within `range` columns over the highest under the module.
        #[arg(long, default_value_t = 0)]
        relief: i32,
        /// Surface: the lit and fenced ground's reach from the module.
        #[arg(long, default_value_t = 16)]
        radius: i32,
        #[arg(long, default_value = "late")]
        stage: String,
        /// The pocket level's height over the module's floor: with it, each sky siting prints its
        /// site condition, the highest ground top keeping the pocket at --max-pocket-y or lower
        /// (the standard layout's design choice, no game rule: ironspec 2026-11-12).
        #[arg(long)]
        pocket_level: Option<i32>,
        /// The highest world y the pocket level may take.
        #[arg(long, default_value_t = 110)]
        max_pocket_y: i32,
    },
    Sky {
        design: PathBuf,
        #[arg(long)]
        terrain: String,
        /// One lightning rod on the roof (the roof then over the footprint only).
        #[arg(long)]
        rod: bool,
        /// A box of design cells taken out of the design, X0,Y0,Z0..X1,Y1,Z1 (repeatable).
        #[arg(long, allow_hyphen_values = true)]
        remove: Vec<String>,
        /// Extra villager positions, as site-check's `--positions`.
        #[arg(long)]
        positions: Vec<PathBuf>,
        /// Every rail cell a villager position (riders on carts).
        #[arg(long)]
        rails: bool,
        /// Every villager position at least this high over the highest natural block within this
        /// many columns (8, the rule: the golem search's band; 16: the golem cells out of a ground
        /// skeleton's bow, the villagers out of a pillager's sensing, 15 each).
        #[arg(long, default_value_t = 8)]
        range: i32,
        /// The design y of the pocket level (its stand pockets): with it, spots lifting it over
        /// --max-pocket-y are refused: the standard layout's design choice, a tool default and no
        /// game rule (ironspec 2026-11-12: the shared kill room's portal at the pads' y, the loop
        /// pair 13 or more levels away and the whole Nether side under the roof for survival
        /// access need the pocket at 110 or less; 26.3 links pre-built portals at any y, G1).
        #[arg(long, allow_hyphen_values = true)]
        pocket_y: Option<i32>,
        /// The highest world y the pocket level may take (the standard layout's choice).
        #[arg(long, default_value_t = 110)]
        max_pocket_y: i32,
        /// Drop the standard layout's pocket cap, for a site that needs it: its gate is then
        /// onechunk's `portal-links` on that site's own Nether cells, every pad and loop portal
        /// linking to its intended partner by 3D distance with a margin of 2 or more (ironspec
        /// 2026-11-12); the report names it.
        #[arg(long)]
        no_pocket_cap: bool,
        /// The sky design written here.
        #[arg(long)]
        out: PathBuf,
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
    /// One-chunk (b), the climb bench (ironspec and farmopt 2026-11-11, paper v0): one batch of 3
    /// ridden carts from rest at the foot hold H up a straight powered climb of `--cells` boosters
    /// to today's approach top, the snap turn and today's 3 pockets (the S3 file's cells; gates
    /// bench-driven), H opened at T + `--h-open` (T a world time multiple of 100); each cart's rest
    /// tick and margin before T+100, arrival speeds into waiting carts, riders' damage, dismounts,
    /// eye point inside a block, the golem box gap, and a freeze span; at every H-open corner and
    /// seed. `--gravity` (with `--v3`, `--v4` or `--crest6`): the gravity queue's hold → stand
    /// descent instead.
    /// (7b90c3f1, farmopt Nov 11 20:58) The vertical lift bench: a soul-sand bubble column built
    /// alone, an entry flat at its foot (its east wall open `--entry-h` cells: 1 = the rail cell,
    /// glass over it, tick-exact: riders pass glass harmlessly), the exit at its top (`--exit`:
    /// none, rail, current, piston); villager-ridden carts launched along the entry; per cart the
    /// column met, the top, the end, the rider's air, seating, damage, eye-in-block ticks.
    LiftBench {
        #[arg(long, default_value_t = 20)]
        height: i32,
        #[arg(long, default_value_t = 6)]
        entry_len: i32,
        #[arg(long, default_value_t = 1)]
        entry_h: i32,
        #[arg(long, default_value_t = 0.4)]
        speed: f64,
        #[arg(long, default_value_t = 1)]
        carts: usize,
        #[arg(long, default_value_t = 12)]
        spacing: i64,
        #[arg(long, default_value_t = 160)]
        ticks: i64,
        #[arg(long, default_value = "none")]
        exit: String,
        /// One cart's line a tick.
        #[arg(long)]
        trace: Option<usize>,
        /// A glass block this many cells over the shaft's top water, capping the launch (0: none).
        #[arg(long, default_value_t = 0)]
        ceiling: i32,
        /// FROM,LEN: the bench frozen (level 32) from tick FROM for LEN gt.
        #[arg(long, value_delimiter = ',')]
        freeze: Vec<i64>,
        /// Cells descending west right after the exit's landing cell (the carts pulling apart).
        #[arg(long, default_value_t = 0)]
        exit_slope: i32,
        /// Q2's merge: lanes B (z 2) and C (z -2) joining the entry line through passive T
        /// junctions, carts launched on A, B, C in turn (use --entry-len 14 or more).
        #[arg(long)]
        merge3: bool,
        /// Q2's split: two powered junctions after the exit slope (use --exit-slope 1+), armed and
        /// disarmed by rise-keyed detector latches, carts back to lanes A/B/C in merge order.
        #[arg(long)]
        split3: bool,
        /// The shaft's four wall cells as unconnected oak fence posts (sight passes either side of the
        /// post; ironspec 22:20: the riders must see a scarer in the lift too).
        #[arg(long)]
        pane_walls: bool,
        /// The rider-height mark: the rider's feet this far over the entry's (17.2: clear of 17
        /// from the bed for a shaft at the budget's (-5, y, 8)).
        #[arg(long, default_value_t = 11.95)]
        reach_dy: f64,
        /// A detector rail on the entry line at this x (the napper-only leg's evidence detector);
        /// prints each cart's rise and the gt from it to the rider-height mark.
        #[arg(long)]
        detector_at: Option<i32>,
    },
    ClimbBench {
        #[arg(long, default_value_t = 14)]
        cells: i32,
        /// T's height (today's approach top 5; 3, the lowest with today's snap turn).
        #[arg(long, default_value_t = 5)]
        crest: i32,
        /// H's opening, gt after T, each corner ("1,2,3,4,5").
        #[arg(long, default_value = "1,2,3,4,5")]
        h_open: String,
        /// H shuts again this many gt after it opened (0: never).
        #[arg(long, default_value_t = 0)]
        h_for: i64,
        /// The per-gate hold's release spacing emulated (gt; 0: the batch queued back to back at
        /// H): cart k set at rest at Ga at T + h-open + k * this, each corner ("15,23").
        #[arg(long, default_value = "0")]
        h_every: String,
        /// The climb's pattern from its foot (`B` booster, `P` plain; empty: all boosters).
        #[arg(long, default_value = "")]
        pattern: String,
        /// onechunk's cornered route instead of the straight climb: H_RAILS,L1 (Ga's hand-off at
        /// rails H_RAILS, leg 1 of L1 slopes south, leg 2 west up to 3, the approach's flats north
        /// to T at 3); the carts handed off at Ga's B at 0.5 b/t, `--h-every` gt apart.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        route: Vec<i32>,
        /// onechunk's folded (b) route: Dh, a flat curve west, leg 1 (11 boosters), a curve north,
        /// leg 2 (9 boosters) to T at 3; the carts handed off at Dh at 0.5 b/t, `--h-every` gt
        /// apart (default 13: the pocket stand's gates send them 12..14 apart).
        #[arg(long)]
        folded: bool,
        /// The gravity queue's descent (hold → stand) instead of a climb: `--cells` slopes down
        /// north to the crest at +3 (`--pattern` B/P from the bottom); the carts handed off at the
        /// top at `--speed` b/t heading north, `--h-every` gt apart (0: back to back, 3 gt).
        #[arg(long)]
        gravity: bool,
        /// With --gravity: onechunk's skeleton v3 descent (Ga's exit at (13,24,4), leg A south, a
        /// curve, leg B west, a curve, leg C north into the committed crest (3,6,10)); `--cells`
        /// ignored. `--plain-c`: leg C's three slopes plain.
        #[arg(long)]
        v3: bool,
        /// With --gravity: the hold 9 levels lower (the inverted variant, until v4's cells): Ga's
        /// exit and three flat boosters at rail height 6 + `--cells`, a straight boosted descent
        /// into the committed crest (3,6,10) and the approach.
        #[arg(long)]
        crest6: bool,
        /// With --gravity: onechunk's skeleton v4 descent (the inverted variant): Ga's rail
        /// (13,15,5), south, a curve, west, a curve, north into the committed crest (3,6,10);
        /// every slope and flat boosted (`--plain-flats`: the flats plain).
        #[arg(long)]
        v4: bool,
        /// With --gravity: the descent re-laid as continuous slopes from loop (7,15,5) (Ga at
        /// (7,16,4)); my emulation until onechunk's cells landed.
        #[arg(long)]
        v5: bool,
        /// With --gravity: onechunk's re-laid v4 descent (635596597, 13 cells: G_a (13,15,10), a
        /// slope south, a curve, eight slopes west, a flat, a curve, a flat into the crest).
        #[arg(long)]
        v4r: bool,
        #[arg(long)]
        plain_flats: bool,
        #[arg(long)]
        plain_c: bool,
        /// The hand-off speed (b/t) for --gravity.
        #[arg(long, default_value_t = 0.4)]
        speed: f64,
        /// Per-cart hand-off speeds "A,B,C" (b/t), overriding --speed (the measured hold: leader
        /// 0.33..0.40, followers 0.4).
        #[arg(long, value_delimiter = ',')]
        speeds: Vec<f64>,
        /// The stand's gates on timers from the hold's release (live time, paused while frozen):
        /// G2 shuts at release + A, G1 at release + B (instead of position triggers).
        #[arg(long, value_delimiter = ',')]
        gate_shut: Vec<i64>,
        /// The batch's carts: 3, or 2 (the third stays parked).
        #[arg(long, default_value_t = 3)]
        carts: usize,
        #[arg(long, default_value_t = 1.29)]
        close_at: f64,
        #[arg(long, default_value_t = 160)]
        ticks: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 3)]
        seeds: i64,
        /// Which carts carry a rider, front first ("1,1,1"; "0,1,1": the front cart empty).
        #[arg(long, default_value = "1,1,1")]
        riders: String,
        /// The golem's box (design frame) while a golem exists (T..T+2): x0,y0,z0,x1,y1,z1.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        golem_box: Vec<f64>,
        /// The freeze: the bench's chunks at level 32 from T + FROM for LEN gt ("FROM,LEN").
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        freeze: Vec<i64>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        trace: bool,
    },
    /// The gravity queue's climb back (onechunk's skeleton v3, `--v4` the inverted v4): the batch at
    /// rest in today's pockets leaves at the stand's release (G0 and G1 at REL, G2 at REL + its
    /// delay, each slope powered as the meter powers it) up the S3 departure, three flat curves and
    /// boosters to the top; each cart's tick above feet `--above` against T+100, the curve rule at
    /// every curve, contacts, the riders, the arrival at the top (v4: and 4, 8, 12 cells past it,
    /// after REL); per REL corner, G2 delay and seed.
    ClimbBack {
        /// REL, gt after T, each corner ("3,4,5,6,7").
        #[arg(long, default_value = "3,4,5,6,7")]
        rel: String,
        /// G1's delay after REL, each value ("0").
        #[arg(long, default_value = "0")]
        g1_delay: String,
        /// G2's delay after REL, each value ("6").
        #[arg(long, default_value = "6")]
        g2_delay: String,
        #[arg(long, default_value_t = 24.5)]
        above: f64,
        /// onechunk's v4 (inverted) climb back: the east leg rises 5 then runs flat to the top
        /// (15,37,0) (v3: 14 up to 46).
        #[arg(long)]
        v4: bool,
        /// The batch's carts: 3, or 2 (cart 2 leaves the world before the release).
        #[arg(long, default_value_t = 3)]
        carts: usize,
        /// The east leg's slopes up to the top (default: v3 14, v4 5; C′'s top at 38: 6).
        #[arg(long)]
        east: Option<i32>,
        /// Every gate's slope powered as its gate opens (the meter powers G1's and G2's 8 gt
        /// after): a stand-side variant for the empty cart's lag.
        #[arg(long)]
        slopes_at_open: bool,
        /// G1's and G2's slopes powered this many gt after their gates open (the meter's 8).
        #[arg(long, default_value_t = 8)]
        slope_delay: i64,
        #[arg(long, default_value_t = 160)]
        ticks: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 3)]
        seeds: i64,
        #[arg(long, default_value = "1,1,1")]
        riders: String,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        freeze: Vec<i64>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        trace: bool,
    },
    /// The carousel's storage at the shared Nether kill room (b8949c96 2026-11-12, ironspec's
    /// spec; `mobfarm::storage`): writes the design (the kill floor's hoppers into --lines hopper
    /// lines, each a sorter-classic iron slice into an ironblock-v24 crafter, the iron blocks into
    /// a one-level hall of --chests double chests, the poppies into lava) and benches it in redsim
    /// at --modules modules' golem loot over --days game days (each --seeds seed in turn).
    Storage {
        /// Modules killing into the room (236 golems a game day each).
        #[arg(long, default_value_t = 4)]
        modules: u32,
        /// Hopper lines (default: 1 to 2 modules, 2 above).
        #[arg(long)]
        lines: Option<usize>,
        /// Double chests (default: 20 play-hours = 60 game days of iron blocks at --modules).
        #[arg(long)]
        chests: Option<usize>,
        #[arg(long, default_value_t = 3.0)]
        days: f64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 1)]
        seeds: i64,
        /// Iron blocks put in each double chest first (3456: the hall full, the full-storage row).
        #[arg(long, default_value_t = 0)]
        prefill: u32,
        /// The sorters set to poppies (the mis-set filter control).
        #[arg(long)]
        misset: bool,
        /// No shell (the ghast rule's: the walkway and every block under the blast bar closed by
        /// blast-resistant blocks; on by default).
        #[arg(long)]
        open: bool,
        /// Add the first enclosure (a cobblestone room round the hall) under the shell, to price
        /// it against the shell alone.
        #[arg(long = "box")]
        boxed: bool,
        /// Write the design moved by X,Y,Z without the bench's kill-floor ring (onechunk's Nether
        /// frame: the kill floor's 2 x 2 hoppers onto the kill room's), no bench.
        #[arg(long, allow_hyphen_values = true)]
        frame: Option<String>,
        /// The design written here (default: a temp file).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Write the design only.
        #[arg(long)]
        no_bench: bool,
    },
    /// The vertical drop, M1 (vjeux 21:00; `mobfarm::drop`; ironfarm's lane bench is
    /// `escape-bench`): the waiting batch on trapdoors straight above three braked pockets drops into
    /// them while the batch in them leaves at REL; the series layout (today's strip), Q2's parallel
    /// lanes or the F1 + Q2 station (`--layout station`, 9fc3155f's budget); each trapdoor gated on
    /// its pocket's occupancy string (--occupancy) or exit detector (--exit-gate). Per run the
    /// release positions, falls, landings against the pockets' re-brake, rest against the next
    /// summon, contacts, riders, departures, the refill, the night-panic coverage; the layout's
    /// footprint and height.
    DropBench {
        /// `series`, `parallel:P` (Q2: lanes P apart in z) or `station`, each ";"-separated.
        #[arg(long, default_value = "series")]
        layout: String,
        /// Logins T,D (ironfarm's): the entities frozen for D gt from T while the redstone runs
        /// (repeatable; in every run, on top of --freezes).
        #[arg(long)]
        login: Vec<String>,
        /// The hold's trapdoor block y (the pockets' rails at y -2..0), each value.
        #[arg(long, default_value = "15")]
        hold_y: String,
        /// REL, gt after T, each corner.
        #[arg(long, default_value = "3,5,7")]
        rel: String,
        /// Each pocket's power after REL, front first.
        #[arg(long, default_value = "0,0,2")]
        stagger: String,
        /// The pockets re-brake at REL + this, each value.
        #[arg(long, default_value = "32")]
        rebrake: String,
        /// The trapdoors open at T + this, each value.
        #[arg(long, default_value = "10")]
        release: String,
        /// The trapdoors shut this long after opening (0: stay open).
        #[arg(long, default_value_t = 0)]
        pulse: i64,
        /// Gate each trapdoor on its own pocket's string (held 20 gt: occupied, wait).
        #[arg(long)]
        occupancy: bool,
        /// The trapdoors' timer on world ticks (a plain redstone clock through a freeze): the
        /// strings' positive control.
        #[arg(long)]
        release_world: bool,
        /// The parallel layout's golem pad under the summoners' overlap (v4's 44-spot pattern).
        #[arg(long)]
        pad: bool,
        /// Each pocket re-brakes --rebrake after its own release (Q2's lanes), not after REL.
        #[arg(long)]
        rebrake_each: bool,
        /// (the station) Gate each trapdoor on its pocket's exit detector (the merge's first cell)
        /// having risen after that pocket's release, instead of on its string.
        #[arg(long)]
        exit_gate: bool,
        /// Scarers for the night-panic coverage print: "z:x,y,z;p:x,y,z;P:x,y,z" (a zombie's or a
        /// pillager's feet, the design's frame; `P` a pillager pinned in a 1 x 1 cell: checked from
        /// its centre and its four corners +-0.2).
        #[arg(long, default_value = "")]
        scarers: String,
        /// Extra blocks set in the design last, each "x,y,z,STATE" (repeatable): a scarer's cell and
        /// its sight openings, to check against the layout.
        #[arg(long)]
        block: Vec<String>,
        /// A zombie post at its boat's cell x,y,z (the brief's: glass under, a persistent zombie in
        /// an oak boat, leaves 3 above; repeatable): a scarer in the coverage print, and in the
        /// design for `mobfarm exit`'s golem check.
        #[arg(long = "post", allow_hyphen_values = true)]
        posts: Vec<String>,
        /// (the station) The hold's north stop a bottom slab (stops the cart off R_1, passes the
        /// hold rider's sight north) instead of a glass block.
        #[arg(long)]
        slab_stop: bool,
        /// The trapdoor opens this long after its gate is met (G1: the hold's release at stage 3,
        /// 4 gt after stage 1).
        #[arg(long, default_value_t = 0)]
        open_delay: i64,
        /// L1 (the lead): the controller and its timers on game ticks through a freeze or login
        /// (default: L2, the live clock).
        #[arg(long)]
        l1: bool,
        /// A restart T,ORDER in every run (cc767b78's `liveness::restart`; repeatable): between
        /// ticks T - 1 and T a save/stop/start in that entity order (save, reverse, seed:K). The
        /// cold start is the fresh design's row (--dep-riders x,x,x).
        #[arg(long)]
        restart: Vec<String>,
        /// The restart at every phase "FROM,TO,STEP": one run per tick and per order of the
        /// reload-order rule (save, reverse, seed:1..8).
        #[arg(long, value_delimiter = ',')]
        restart_sweep: Vec<i64>,
        /// (the station) Write the station as a committed design in the loop frame (onechunk's
        /// module): the build, the start state's carts (pockets and holds), `positions:` and the
        /// runner's `run-cells:`, the event table in the header.
        #[arg(long)]
        write_design: Option<PathBuf>,
        /// (the station) The R line: the lanes' R_1 cells with the next batch on them, powered from
        /// the side "D,L": D gt after the last trapdoor shut again (needs --pulse), for L gt.
        #[arg(long, default_value = "")]
        refill: String,
        #[arg(long, default_value = "1,1,1")]
        dep_riders: String,
        #[arg(long, default_value = "1,1,1")]
        arr_riders: String,
        /// Freezes, each one run: "FROM:LEN,..." (`-` unfrozen).
        #[arg(long, default_value = "-")]
        freezes: String,
        /// A freeze sweep: "FROM,TO,STEP,LEN[,LEN...]".
        #[arg(long, value_delimiter = ',')]
        freeze_sweep: Vec<i64>,
        #[arg(long, default_value_t = 200)]
        ticks: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 1)]
        seeds: i64,
        /// Only the summary line per layout and hold height.
        #[arg(long)]
        summary: bool,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        trace: bool,
    },
    /// (cc767b78; ironspec's L1‴ rows) The judge of the F1 + Q2 station in the L1 mode: per `--rel`
    /// its control, a save/stop/start at every phase `--starts A,B` (bench ticks from T), the cold
    /// start (the departing carts absent) and, for the pearl alternative, logins of `--ds` gt at every
    /// phase; the pass: every cart landed on a braked pocket and at rest before the next summon, 0
    /// wrong pockets, contacts, hurt, unseated, stuck, refill failures, trapdoors shut onto their cart,
    /// every launch >= `--launch-min` gt after the one before. `--rebrakes`, `--pulses`,
    /// `--open-delays`, `--refill-delays`, `--refill-lens`, `--staggers A:B:C,..`: the same matrix at
    /// other widths of one timed edge each (the thresholds).
    StationJudge {
        #[arg(long, default_value_t = 16)]
        hold_y: i32,
        /// The RELs (T+3..9 around the adopted REL = golem + 4: G1 2026-11-12 05:15).
        #[arg(long, value_delimiter = ',', default_value = "3,4,5,6,7,8,9")]
        rel: Vec<i64>,
        /// The launch offsets (dfb89499's run line at 4bce82b85: 0/14/28, ironspec's provisional s 14).
        #[arg(long, value_delimiter = ',', default_value = "0,14,28")]
        stagger: Vec<i64>,
        #[arg(long, default_value_t = 16)]
        rebrake: i64,
        #[arg(long, default_value_t = 3)]
        release: i64,
        #[arg(long, default_value_t = 4)]
        open_delay: i64,
        /// The trapdoor pulse (ironspec 2026-11-12 01:35: 12, past the lead rows' threshold by the L1
        /// bar and clear of the pearl alternative's logins).
        #[arg(long, default_value_t = 12)]
        pulse: i64,
        #[arg(long, value_delimiter = ',', default_value = "2,10")]
        refill: Vec<i64>,
        /// Zombie posts (x,y,z, repeatable; none in the run line since 4bce82b85: the hold post came out).
        #[arg(long = "post", allow_hyphen_values = true)]
        posts: Vec<String>,
        #[arg(long)]
        no_slab_stop: bool,
        /// Scarers for the night-panic print (none in the run line).
        #[arg(long, default_value = "")]
        scarers: String,
        /// The L2 mode instead (the controller on the live clock, paused with the entities).
        #[arg(long)]
        l2: bool,
        #[arg(long, default_value_t = 200)]
        ticks: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Seeds from --seed (every row runs at each; G1: dfb89499's control and cold rows ran 40).
        #[arg(long, default_value_t = 1)]
        seeds: i64,
        /// The landing lead's bar (the L1 edge bar: 3 gt + 1 scheduled tick).
        #[arg(long, default_value_t = crate::stationjudge::EDGE_BAR)]
        edge: i64,
        /// The rest margin's bar before the next summon (the L1 edge bar until dfb89499's panic measure
        /// lands; the 20-gt rest bar is retired: ironspec 2026-11-12 05:26).
        #[arg(long, default_value_t = crate::stationjudge::REST_BAR)]
        rest_bar: i64,
        #[arg(long, value_delimiter = ',', default_value = "0,150")]
        starts: Vec<i64>,
        /// Login lengths for the pearl alternative's load window (empty: none).
        #[arg(long, value_delimiter = ',', default_value = "1,2,3,4")]
        ds: Vec<i64>,
        #[arg(long)]
        no_restarts: bool,
        #[arg(long)]
        no_cold: bool,
        /// The launch-spacing bar (gt; 0: reported only: with the lift's merge offsets 0/6/12 its bar is
        /// the shaft-entry gap, the lift judge's).
        #[arg(long, default_value_t = 0)]
        launch_min: i64,
        #[arg(long, value_delimiter = ',')]
        rebrakes: Vec<i64>,
        #[arg(long, value_delimiter = ',')]
        pulses: Vec<i64>,
        #[arg(long, value_delimiter = ',')]
        open_delays: Vec<i64>,
        #[arg(long, value_delimiter = ',')]
        refill_delays: Vec<i64>,
        #[arg(long, value_delimiter = ',')]
        refill_lens: Vec<i64>,
        /// Staggers to try, each A:B:C.
        #[arg(long, value_delimiter = ',')]
        staggers: Vec<String>,
        #[arg(long, default_value_t = 8)]
        jobs: usize,
    },
    /// One-chunk's stand cycle (farmopt 18:52): the departing batch leaves today's pockets at REL
    /// (the meter's slope power) up the S3 departure and v4's climb back while the arriving batch
    /// comes down onechunk's re-laid descent from the hold's release into the same pockets; the
    /// stand's gates on explicit shut times. Per gate the shut window (the last passing cart's
    /// clear, departing or arriving, to the arriving cart it stops reaching it), gate edges on any
    /// cart, the arriving carts' pockets and rest against T+100, contacts; per REL and hold corner,
    /// order and seed.
    StandCycle {
        /// REL, gt after T, each corner.
        #[arg(long, default_value = "3,5,7")]
        rel: String,
        /// The hold's release, gt after T, each corner.
        #[arg(long, default_value = "1,3,5")]
        hold: String,
        #[arg(long, default_value_t = 0)]
        g1_delay: i64,
        /// G2's stagger after REL, each value ("0,1,2").
        #[arg(long, default_value = "2")]
        g2_delay: String,
        #[arg(long, default_value_t = 6)]
        slope_delay: i64,
        /// The hand-offs: cart 1 A gt after cart 0, cart 2 B after cart 1, each entry ("14:13").
        #[arg(long, default_value = "14:13")]
        h_every: String,
        #[arg(long, default_value_t = 0.4)]
        speed: f64,
        /// Per-cart hand-off speeds, each set ";"-separated ("0.33,0.4,0.4;0.4,0.4,0.4"),
        /// overriding --speed.
        #[arg(long, default_value = "")]
        speeds: String,
        /// G0's shut: `d1:K` (D1's first fall after REL + K; the backstop), `dd:K` (the departure
        /// detector's first fall after REL + K), `ddcount:K` (its rises counted to the departing
        /// count, + K), `ddretrig:K` (a delay restarted by every rise), `d2:ARM` (D2's first rise
        /// after REL + ARM), `rel:S` (REL + S), `-` (never); each entry one run set ("d1:10,dd:2").
        #[arg(long, default_value = "d1:10")]
        g0: String,
        /// G1's and G2's shuts keyed on D1's first rise after G0 shut: "A,B" (G1 + A, G2 + B), or
        /// `-`: `--shut`'s G1 and G2 entries after the hold's release.
        #[arg(long, default_value = "2,7")]
        fill: String,
        /// Fixed shuts (with `--g0 -`/`--fill -`): G0 after REL, G1 and G2 after the hold's release;
        /// "-" never.
        #[arg(long, default_value = "-,65,69")]
        shut: String,
        #[arg(long, default_value_t = 45)]
        backstop: i64,
        /// The qualified edges (ironspec 19:04): a fall counts only after 20 live gt, every keyed
        /// delay and the backstop on the live clock.
        #[arg(long)]
        qualified: bool,
        /// The departure leg, each a design: `N` (v4's re-laid crest, no detector), `A` (a sloped
        /// detector at the crest, loop (9,2,6)), `B` (a sloped detector for booster #1, loop
        /// (9,-3,1)), `legacy` (the bench's old flat crest); "N,A,B".
        #[arg(long, default_value = "N")]
        dd: String,
        /// The departing riders' feet height to cross by T+100.
        #[arg(long, default_value_t = 15.54)]
        above: f64,
        /// Freezes, each one run: "FROM:LEN,FROM:LEN" (gt after T; `-` an unfrozen run).
        #[arg(long, default_value = "-")]
        freezes: String,
        /// Only the grid's summary line per G0 key and freeze (no per-run lines).
        #[arg(long)]
        summary: bool,
        /// Only the overall line per G0 key (every freeze together), with --summary.
        #[arg(long)]
        overall_only: bool,
        /// The vertical drop (vjeux 21:00), each set ";"-separated "H,R,O": the hold's trapdoors at
        /// y H straight above the braked pockets, opened at REL + R, the pockets re-braked at REL +
        /// O (live); no descent, no stand gates.
        #[arg(long, default_value = "")]
        drop: String,
        /// A freeze sweep instead of --freezes: "FROM,TO,STEP,LEN[,LEN...]" (each start and length one
        /// run set).
        #[arg(long, value_delimiter = ',')]
        freeze_sweep: Vec<i64>,
        /// The departing batch, each order ";"-separated: per cart 1 ridden, 0 empty, x absent
        /// ("1,1,1;0,1,1;1,1,x").
        #[arg(long, default_value = "1,1,1")]
        dep_riders: String,
        /// The arriving batch, each order ";"-separated.
        #[arg(long, default_value = "1,1,1")]
        arr_riders: String,
        /// The climb back's east slopes (C′'s top at 38: 6).
        #[arg(long, default_value_t = 6)]
        east: i32,
        #[arg(long, default_value_t = 200)]
        ticks: i64,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 2)]
        seeds: i64,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        trace: bool,
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
    /// The rider-height string bench (ironfarm; ironspec 2026-11-11 21:27): a generic pocket cell
    /// with a tripwire string in the cell above its rest slope; one cart launched down a descent comes
    /// to rest at the pocket's gate under the string (the gate opens `--hold` gt after it stops) or
    /// passes it (`--pass`); prints the hook's powered ticks against the rider's box over the cell.
    StringBench {
        #[arg(long, default_value_t = 3)]
        descent: i32,
        #[arg(long, default_value_t = 0.0)]
        v0: f64,
        #[arg(long)]
        pass: bool,
        #[arg(long, default_value_t = 40)]
        hold: i64,
        #[arg(long, default_value_t = 200)]
        ticks: i64,
        /// An empty cart (default ridden).
        #[arg(long)]
        empty: bool,
    },
    /// The shared-release lanes bench (ironfarm, Q2): parallel pressed lanes, one release line
    /// opening every front gate together; per opening and lane the carts through (exactly 1), gate
    /// shuts onto a cart, the batch's spread across lanes, arrivals at the backs, the pressed spacing.
    LanesBench {
        #[arg(long, default_value_t = 3)]
        lanes: usize,
        #[arg(long, default_value_t = 2)]
        spacing: i32,
        #[arg(long, default_value_t = 8)]
        run: i32,
        #[arg(long, default_value_t = 7)]
        per_lane: usize,
        #[arg(long, default_value_t = 100)]
        period: i64,
        #[arg(long, default_value_t = 12)]
        width: i64,
        #[arg(long, default_value_t = 20)]
        cycles: i64,
        #[arg(long, default_value_t = 150)]
        ret: i64,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        corner: i64,
        #[arg(long, default_value_t = 0)]
        ring_q: i64,
        /// Emulated freezes T,D (repeatable).
        #[arg(long)]
        freeze: Vec<String>,
        /// Real level-32 freezes T,D (repeatable).
        #[arg(long)]
        freeze32: Vec<String>,
        #[arg(long)]
        empty: bool,
    },
    /// The per-lane escapement into the drop cell (ironfarm; explore/ironfarm's `drop-bench`): pressed
    /// lanes into an off-rail drop cell (a bottom-half trapdoor one block below the rail end, glass on
    /// three sides, a rider string above). With `--r-pulse` the switched-R lane (brake and rider,
    /// ironspec 22:32): R, a switched powered slope as the lane's last rail; cart 2 braked on R against
    /// cart 1's rider in D; one trapdoor line drops, then one R line restages. Without it the G0/G_P
    /// escapement (superseded: G0 splitting the pressed lane has a 13..14-gt band); `--pockets` the
    /// bucket brigade (the priced fallback).
    EscapeBench {
        #[arg(long, default_value_t = 3)]
        lanes: usize,
        #[arg(long, default_value_t = 4)]
        spacing: i32,
        #[arg(long, default_value_t = 10)]
        run: i32,
        #[arg(long, default_value_t = 8)]
        per_lane: usize,
        #[arg(long, default_value_t = 100)]
        period: i64,
        #[arg(long, default_value_t = 20)]
        cycles: i64,
        #[arg(long, default_value_t = 80)]
        ret: i64,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        corner: i64,
        #[arg(long, default_value_t = 13)]
        w0: i64,
        #[arg(long, default_value_t = 12)]
        wp: i64,
        #[arg(long, default_value_t = 12)]
        drop_w: i64,
        #[arg(long, default_value_t = 0)]
        entry: i32,
        #[arg(long, default_value_t = 20)]
        hold_n: i64,
        #[arg(long)]
        front_boost: bool,
        /// The bucket-brigade lane: this many pockets (0: the pressed lane).
        #[arg(long, default_value_t = 0)]
        pockets: usize,
        /// The brigade's strings: cells above each pocket's rail block.
        #[arg(long, default_value_t = 2)]
        string_dy: i32,
        /// The switched-R lane: the R line's pulse in gt (0: the G0/G_P lane).
        #[arg(long, default_value_t = 0)]
        r_pulse: i64,
        /// The switched-R lane: retry the R line this long after a pulse if a D hasn't settled.
        #[arg(long, default_value_t = 40)]
        retry: i64,
        /// Logins T,D: the entities frozen for D gt while the redstone runs (repeatable).
        #[arg(long)]
        login: Vec<String>,
        /// The login matrix: a login of 1..=3 gt at every phase of a cycle (one run per phase and
        /// length), printing the rows that fail.
        #[arg(long)]
        login_matrix: bool,
        /// The switched-R lane: the flat intake and the split's flat branch above the run.
        #[arg(long)]
        intake_flat: bool,
        /// The returns' speed on the split's flat branch (with --intake-flat).
        #[arg(long, default_value_t = 0.2)]
        v_in: f64,
        #[arg(long)]
        r_boost: bool,
        #[arg(long)]
        floor_bottom: bool,
        #[arg(long)]
        empty: bool,
        /// (cc767b78) Real level-32 freezes T,D (repeatable): entities frozen, redstone and the
        /// controller running.
        #[arg(long)]
        freeze32: Vec<String>,
    },
    /// (cc767b78; farmopt 2026-11-11 22:03, the L1 rule) The freeze-matrix judge of the lanes
    /// bench: the L1 load window (block events up to 3 gt ahead of the entities, at most 1 scheduled
    /// tick while they are frozen) as a real level-32 freeze of D gt (`--ds`, 1..4) at every 1-gt
    /// start `--starts A,B` gt from opening `--k`, at every `--corners`, in each `--modes` (gt:W
    /// game-tick widths, ringQ:W ring-edge widths); the bars: exactly 1 cart per lane per opening,
    /// 0 gate shuts onto a cart; the failing starts per corner and D.
    LanesJudge {
        #[arg(long, default_value_t = 3)]
        lanes: usize,
        #[arg(long, default_value_t = 2)]
        spacing: i32,
        #[arg(long, default_value_t = 8)]
        run: i32,
        #[arg(long, default_value_t = 7)]
        per_lane: usize,
        #[arg(long, default_value_t = 100)]
        period: i64,
        #[arg(long, default_value_t = 8)]
        cycles: i64,
        #[arg(long, default_value_t = 150)]
        ret: i64,
        #[arg(long, value_delimiter = ',', default_value = "gt:14,ring34:14")]
        modes: Vec<String>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "-10,89")]
        starts: Vec<i64>,
        #[arg(long, default_value_t = 3)]
        k: i64,
        #[arg(long, value_delimiter = ',', default_value = "1,2,3,4")]
        ds: Vec<i64>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "-2,-1,0,1,2")]
        corners: Vec<i64>,
        #[arg(long, default_value_t = 8)]
        jobs: usize,
        #[arg(long)]
        empty: bool,
    },
    /// (cc767b78, the L1 rule; explore/farmopt-cc-judge's `drop-judge`) The freeze-matrix judge of
    /// the escape bench: the L1 load window as a real level-32 freeze of D gt (`--ds`) at every 1-gt
    /// start `--starts A,B` gt from release `--k`, at every `--corners`; a run fails when worse than
    /// its corner's unfrozen control on its exactly-1 counts, cells not settled, shuts onto a cart.
    EscapeJudge {
        #[arg(long, default_value_t = 3)]
        lanes: usize,
        #[arg(long, default_value_t = 4)]
        spacing: i32,
        #[arg(long, default_value_t = 10)]
        run: i32,
        #[arg(long, default_value_t = 8)]
        per_lane: usize,
        #[arg(long, default_value_t = 100)]
        period: i64,
        #[arg(long, default_value_t = 8)]
        cycles: i64,
        #[arg(long, default_value_t = 80)]
        ret: i64,
        #[arg(long, default_value_t = 13)]
        w0: i64,
        #[arg(long, default_value_t = 12)]
        wp: i64,
        #[arg(long, default_value_t = 12)]
        drop_w: i64,
        #[arg(long, default_value_t = 0)]
        entry: i32,
        #[arg(long, default_value_t = 20)]
        hold_n: i64,
        #[arg(long)]
        front_boost: bool,
        #[arg(long)]
        r_boost: bool,
        #[arg(long)]
        floor_bottom: bool,
        #[arg(long)]
        empty: bool,
        /// (ironfarm) The switched-R lane (brake and rider): the R line's pulse (0: the G0/G_P lane).
        #[arg(long, default_value_t = 0)]
        r_pulse: i64,
        /// (ironfarm) The switched-R lane: the R-line retry after a pulse.
        #[arg(long, default_value_t = 40)]
        retry: i64,
        /// (ironfarm) The switched-R lane: the flat intake and the split's flat branch.
        #[arg(long)]
        intake_flat: bool,
        /// (ironfarm) The returns' speed on the split's branch (with --intake-flat).
        #[arg(long, default_value_t = 0.2)]
        v_in: f64,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "-10,89")]
        starts: Vec<i64>,
        #[arg(long, default_value_t = 3)]
        k: i64,
        #[arg(long, value_delimiter = ',', default_value = "1,2,3,4")]
        ds: Vec<i64>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "-2,-1,0,1,2")]
        corners: Vec<i64>,
        #[arg(long, default_value_t = 8)]
        jobs: usize,
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
        /// itself. Every run prints its frame (`--head-perm` too).
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
        /// Skip the rail-peak check (a slope's high end straight into a slope going down, no flat
        /// cell between: a top no cart crosses, which fails the run); the run says so.
        #[arg(long)]
        no_peak_check: bool,
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
        /// S3(ii)'s metered stand, emulated: D12,D23,CLOSE,DEP,HOLD_ON[,CHAIN[,FALLBACK[,MISS_D1[,VB[,OPEN_LEN[,WIRED_H[,H_DELAY[,GUARD[,G2_DELAY[,G1_DELAY]]]]]]]]]]
        /// (`queue::MeterHold`: the stand's circuit cut, the bench driving G0..G2, their slopes
        /// and a hold gate over the snap turn from detector edges only; VB 1: a virtual stand B
        /// takes the odd GOs' trains, so the stand cycles every 200 gt, VB 2: the wired second
        /// stand; OPEN_LEN: H's fixed short opening; GUARD 1: a chained opening waits for the
        /// previous cart's L edge; G2_DELAY (default 6, ironspec's nominal) and G1_DELAY
        /// (default 0): at REL, G2 or G1 opens that many gt after G0, the departure gaps).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        meter_hold: Vec<i64>,
        /// With --meter-hold: H's fixed opening swept at every gt step: L0,L1[,S0,S1] (open lengths,
        /// and start shifts: HOLD_ON and D12/D23 each + S), one run per combination on --jobs
        /// threads, the openings passing 0 / 1 / 2+ carts and the follower's margin per combination.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        meter_sweep: Vec<i64>,
        /// With --meter-hold: the freeze rows: SLOT,DT0,DT1,N[,X0,Z0,X1,Z1]: one run per gt from
        /// SLOT +DT0 to +DT1, each with the chunk box (default the stand's chunk 0,0..0,0) unloaded
        /// N gt from that gt; per run H's openings, its shuts, any gate shut on a cart.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        meter_freeze: Vec<i64>,
        /// With --meter-freeze: N0,N1: sweep the freeze's length from N0 to N1 gt at the fixed
        /// start SLOT +DT0 (DT1 and N ignored).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        meter_freeze_n: Vec<i64>,
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
        /// The late-arrival hold, emulated: X,Y,Z,N,W (a loop-frame cell before pocket 3, N gt, W
        /// gt): a cart entering that cell within the last N gt, while the controller's REL rose
        /// within the last W gt, holds G2 shut (the cart rests against G2 and leaves from rest).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        late_hold: Vec<i64>,
        /// The settled-release gate, emulated: S,T,L: the stand gets REL_eff, an L-gt pulse
        /// triggered once per controller REL pulse when the latch has been set S gt, or when the
        /// controller's REL has been on T gt (the timeout path).
        #[arg(long, value_delimiter = ',')]
        rel_gate: Vec<i64>,
        /// Option (a)'s two-part release, emulated: X,Y,Z,W (a loop-frame cell after G2, W gt):
        /// while the stand's REL cell is on, the first cart entering that cell shuts G2 behind it
        /// until the REL cell goes off or W gt have passed.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        split: Vec<i64>,
        /// ironspec's stand interlock, emulated: H (a sensor's hold after a cart leaves, gt): each
        /// gate opens only when its signal says so and the space past it reads empty.
        #[arg(long)]
        interlock: Option<i64>,
        /// With --interlock: hold the gates' slopes (the launches onto the space past each gate)
        /// instead of the gates.
        #[arg(long)]
        interlock_launch: bool,
        /// With --interlock: bind the gates only while the stand's REL is off (arrivals only).
        #[arg(long)]
        interlock_arrivals: bool,
        /// The stand's entry hold, emulated: X,Y,Z[,H] (a loop-frame cell on the approach): while
        /// pocket 3 reads occupied, a cart reaching it is kept at rest at its centre.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        stand_hold: Vec<i32>,
        /// With --interlock: G0 also shuts for N gt after each cart crosses it (the departure
        /// re-spacer).
        #[arg(long, default_value_t = 0)]
        depart_space: i64,
        /// With --rel-gate: DELAY,TIMEOUT: REL_eff ends DELAY gt after the next train's first cart
        /// enters C′'s detector cell (N), or TIMEOUT gt after its rise.
        #[arg(long, value_delimiter = ',')]
        rel_end_n: Vec<i64>,
        /// With --rel-gate: K,TIMEOUT: REL_eff ends 4 gt after the K-th cart since its rise crosses
        /// G0 (cart evidence: the batch out), or TIMEOUT gt after its rise.
        #[arg(long, value_delimiter = ',')]
        rel_end_count: Vec<i64>,
        /// With --rel-gate: X,Y,Z,D,TIMEOUT: REL_eff ends D gt after the first cart since its rise
        /// enters that loop-frame cell (the rearmost pocket's exit), or TIMEOUT gt after its rise.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        rel_end_rear: Vec<i64>,
        /// With --rel-gate: the unset-pass also needs the entry quiet (no cart in flight from the
        /// downslope hold to pocket 3's entry).
        #[arg(long)]
        unset_quiet: bool,
        /// The latch's set needs pocket-1 evidence independent of D1 and C′ (emulated: its set
        /// block glass while pocket 1 holds no cart and the latch is clear).
        #[arg(long)]
        q_needs_p1: bool,
        /// Print where the carts are when today's chain fires each step (a gate slope's power
        /// rising, REL at G0 falling), for the evidence chain's trigger cells.
        #[arg(long)]
        chain_study: bool,
        /// The release group's two gaps, head first: G12,G23 (gt; cart 1 to 2, cart 2 to 3), instead
        /// of --release-stagger's equal ones.
        #[arg(long, value_delimiter = ',')]
        release_gaps: Vec<i64>,
        /// G1's and G2's lines delayed N gt, both edges (a repeater's worth, emulated).
        #[arg(long, default_value_t = 0)]
        gate_delay: i64,
        /// With --interlock: G2's in-gate guard reads a tripwire over its cell at rider height (on
        /// at a rider's touch, off at the first 10 gt re-check with none), armed by the latch.
        #[arg(long)]
        g2_tripwire: bool,
        /// With --stand-hold: the hold passes at most one cart per N gt (S1t's metering).
        #[arg(long, default_value_t = 0)]
        stand_meter: i64,
        /// The departure's take-off stand-in: the loop z on the x 9 column whose crossing gaps are
        /// printed (R1's ledge sticks carts 4..6 gt apart; 11 is the last cell (c′)'s dip shares
        /// with R1's rise).
        #[arg(long, default_value_t = 11)]
        takeoff_z: i32,
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
        /// With --fault-table: every case at each REL phase A..B gt around the given timing's (each
        /// gt; the stagger and the rest as given), instead of --all-corners.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        rel_sweep: Vec<i64>,
        /// With --fault-table (a --cprime design): the table for each set of C''s delays
        /// "R1,R2,R3,R4,RIN,RS1;..." (redstone ticks), the design generated again with each.
        #[arg(long, default_value = "")]
        cprime_variants: String,
        /// With --fault-table: the cases' fault slot (5 by default; with --meter-hold's virtual
        /// stand B an even slot puts the fault on stand A's own train).
        #[arg(long, default_value_t = 5)]
        fault_slot: i64,
        /// With --fault-table and --meter-hold: every case at S3's 27 summon-margin corners (the
        /// launch ±2 gt, H's opening start ±2 (HOLD_ON, D12 and D23 together), H's fixed opening
        /// length ±2).
        #[arg(long)]
        meter_corners: bool,
        /// Router S's cell X,Y,Z[,DX,DY,DZ] (loop frame; with --head-perm's router), and DP's:
        /// at each router flip, the carts whose centre is in S's cell, ridden or empty (an empty
        /// one with the gt since it left DP's cell): ironspec's interlock rows.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        s_cell: Vec<i32>,
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
        /// S3(ii)'s per-stand head permissions, emulated on one stand (`mobfarm::headperm`):
        /// AX,AY,AZ,HX,HY,HZ[,G[,R[,BC0,BN]]]: D_arr (the hold's upper descent slope) and Dh
        /// (pocket 3's entry slope), loop frame; G 1: the latches gate the head's release (else
        /// observed only); R 1: the router emulated, phase-locked (A on even cycles, B on odd;
        /// this stand A, a virtual stand B), B's permission held from cycle BC0 for BN cycles.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        head_perm: Vec<i32>,
        /// Riderless trains: X,Y,Z[,S0,S1[,K]]: every rider of a cart entering the cell X,Y,Z (loop
        /// frame) in slots S0..=S1 (default all) removed, the cart running on empty; K > 0: only
        /// the K-th cart to enter the cell in each slot.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        strip_riders: Vec<i64>,
        /// Two carts traced per tick: A,B,T0,T1 (entity ids, the run's gt): their gap, stored and
        /// visible motion, and every push on either (mob on cart; cart on cart with the alignment
        /// test and the motions before and after).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        trace_pair: Vec<i64>,
        /// day-1's RETRY-only head on `--head-perm`'s router (G 1, R 1): ON[,AC0,AN[,LOST]]: A's
        /// stand REL only with a launch for A, or by RETRY (an EXPIRE GO: no raw pad pulse of A's
        /// in the 60 gt before its GO; A's stand not emptied since its last launch; no train in
        /// flight); A forced held from cycle AC0 for AN of its GOs; LOST: A's REL lost at that slot.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        head_retry: Vec<i64>,
        /// A partial train: SLOT,K: at that slot's launch only the front K release slopes go (a
        /// K-cart train; the other carts stay as stragglers for the next launch).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        train_size: Vec<i64>,
        /// The pulse-driven router on `--head-perm` (day-1's ports: R set by each stand's raw pad
        /// pulse, EXPIRE toggling it with none, a double tick toggling it, one train):
        /// BMISS1,BMISS2,BDOUBLE1,BDOUBLE2,WRONG (slots; -1 none): B's tick missed at those slots,
        /// B's pulse also at those (A's) slots, a forced wrong-stand GO (the positive control).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        router_pulses: Vec<i64>,
        /// Day-1's DEP_X, ideal: the loop z of a plane on the stand's approach column that a cart
        /// crosses going down toward the hold (after VB's parking point, z 15); with
        /// `--head-retry`, ARR_X/EMP_X re-arm on it instead of on the launch.
        #[arg(long)]
        dep_x: Option<f64>,
        /// A train that never arrives: SLOT,DT,Z0,Z1[,N]: from that tick for N ticks (default 1)
        /// every cart on the stand's approach column between loop z Z0 and Z1 is removed with its
        /// rider.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        vanish: Vec<f64>,
        /// A CANCEL_X stand-in until day-1's spec: with `--head-retry`, ARR_X armed this many gt
        /// with no D_arr edge is cleared (a train that never arrives releases the hold).
        #[arg(long)]
        cancel_x: Option<i64>,
        /// Day-1's stale-ARR clear with `--head-retry --dep-x`: ARR_X still set at the second
        /// EXPIRE edge since DEP_X, with no Dh3_X between, and no cart on the branch (emulated:
        /// the approach column between DEP_X and the hold), is cleared.
        #[arg(long)]
        stale_clear: bool,
        /// The stale-ARR clear without BRANCH_EMPTY_X's gate: the time-only fallback.
        #[arg(long)]
        stale_ungated: bool,
        /// The stale-ARR clear without its "no Dh3_X between" condition (the branch gate alone).
        #[arg(long)]
        stale_ignore_dh3: bool,
        /// ironspec's ruling on the ARR gap, (ii): the stale-ARR clear without the "no Dh3_X
        /// between" condition, guarded by the liveness gate (EXPIRE edges counted only while no
        /// branch chunk is frozen; emulated), no branch gate.
        #[arg(long)]
        stale_live: bool,
        /// ironspec's ruling on the ARR gap, (i): GO toward A also requires D_arr low.
        #[arg(long)]
        darr_gate: bool,
        /// With `--head-perm`'s router: the GO decides this many gt before the release acts
        /// (day-1's GO S+9 to core REL S+11: 2).
        #[arg(long, default_value_t = 0)]
        go_lead: i64,
        /// An emulated freeze of K carts alone: SLOT,DT,Z0,Z1,N,K: the K highest carts on A's
        /// approach column between loop z Z0 and Z1 at that tick held in place for N gt.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        hold_cart: Vec<f64>,
        /// A fix candidate for `--darr-gate`'s drain: day-1's RETRY (a REL alone) also fires on an
        /// EXPIRE GO while D_arr is on (carts waiting at H), not only while EMP_X is armed. Now
        /// the head model's default with `--head-retry` (day-1 builds it); kept for old command
        /// lines.
        #[arg(long)]
        retry_darr: bool,
        /// With `--head-retry`: day-1's original RETRY (EXPIRE and EMP_X armed only), the D_arr
        /// term off (an opt-out of the default).
        #[arg(long)]
        retry_emp_only: bool,
        /// With `--head-retry`: ironspec's ARR-gap rules off ((i) the D_arr gate and (ii) the
        /// liveness-gated stale clear, both the head model's default): an opt-out.
        #[arg(long)]
        no_arr_rules: bool,
        /// A fault: SLOT,LEVEL: the head's D_arr read stuck at LEVEL (0 off, 1 on) from slot SLOT.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        darr_stuck: Vec<i64>,
        /// A cart lost in transit: SLOT,DT,X0,X1,Z0,Z1[,K]: from SLOT +DT, the first K carts (1)
        /// found in that loop box (any height) removed with their riders.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        vanish_box: Vec<f64>,
        /// A slow transit: SLOT,DT,X0,X1,Z0,Z1,N: the first cart found in that loop box from SLOT
        /// +DT held in place (position and stored motion) N gt.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        hold_box: Vec<f64>,
        /// The head model's summon tick S as the bench sees it (one tick after the slot +93 line):
        /// an opt-out of day-1's count (S the slot +93 line, the edges reading GO S+9, core REL
        /// S+11, stand REL S+21 on the validated set with `--go-lead 2`).
        #[arg(long)]
        summon_raw: bool,
        /// Passages logged at up to 8 cells ("x,y,z;x,y,z", loop frame): each cart's box entry and
        /// exit, ridden or empty, its stored speed in and its least inside; per cell the corner gaps
        /// (the follower's entry after the leader's exit) for empty and ridden followers.
        #[arg(long, default_value = "")]
        cell_log: String,
        /// Up to 4 loop boxes ("x0,x1,z0,z1;..."): any empty cart at rest >= 10 gt inside one is
        /// logged with its cell, the rail there and the powered rails beside it, and how it moved
        /// again (a cart-cart push: the next cart pushed it through; or by itself), or never.
        #[arg(long, default_value = "")]
        stall_box: String,
        /// The riderless matrix: "STAGGERS;KS" (e.g. "2,3,4,5,6;0,1,2,3"): each release stagger x
        /// each empty-cart position (0: every cart; k: the k-th cart of each train, at
        /// --strip-riders' cell), run in parallel (--jobs); per run the --cell-log cells' empty
        /// passages, the --stall-box stalls, backward entries, merges, the closest centres by rest
        /// place, T-cell crossings.
        #[arg(long, default_value = "")]
        riderless_rows: String,
        /// The head model's ARR SET on every DEP_X pulse (the old model), instead of day-1's
        /// one-shot (L_X: only the first DEP_X after a launch arms ARR_X).
        #[arg(long)]
        lx_off: bool,
        /// A fault: SLOT,LEVEL[,N]: day-1's L_X read stuck at LEVEL (1 high: every DEP_X pulse
        /// arms; 0 low: no DEP_X arms) over N slots from slot SLOT (default: to the end).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        lx_stuck: Vec<i64>,
        /// The old EMP_X read at DEP_X (the occupancy snapshot of the last REL, stale after SE): a
        /// positive control for the live E_X read.
        #[arg(long)]
        emp_snapshot: bool,
        /// A fault: SLOT,DT: a stray DEP_X pulse at that tick (no cart crossing).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        dep_pulse: Vec<i64>,
        /// A junction watched: X,Y,Z (loop frame): while any cart sits in its cell, its state per
        /// tick (a re-settle under the cart is a problem), the carts in it, the closest other cart.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        t_watch: Vec<i32>,
        /// A fault on P2's transit latch: SLOT,LEVEL[,N]: stuck set (1) or clear (0) over N slots
        /// from slot SLOT (default: to the end); a stuck-set latch still times out and re-sets.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        p2_stuck: Vec<i64>,
        /// The head's RETRY fires even while P2 holds an earlier REL of the stand (the old model;
        /// by default a pending REL suppresses RETRY: never two RELs in one cycle).
        #[arg(long)]
        retry_over_pending: bool,
        /// A candidate rule (iii): GO toward A also requires every branch chunk live (emulated:
        /// no chunk of the approach column between H and DEP_X frozen).
        #[arg(long)]
        live_go: bool,
        /// Every first contact anywhere between a moving cart and one at rest (a cart-cart push or
        /// a dead stop at a rider's box), with the mover's stored speed; the count over 0.2.
        #[arg(long)]
        contacts_all: bool,
        /// The whole-column freeze matrix: "SLOT,DT0,DT1,STEP;N1,N2,..": one run per start (SLOT
        /// +DT0..+DT1 every STEP gt) and length N, each with the whole part frozen at level 32
        /// (redstone running, entities frozen: `--act SLOT,DT,freeze32,N`), in parallel (--jobs);
        /// per run the bars: stand RELs per cycle, merges, contacts over 0.2, gates shut on a cart,
        /// backward entries, the head's holds and its edges after the thaw, carts lost, the
        /// detection latency.
        #[arg(long, default_value = "")]
        freeze_matrix: String,
        /// The pausing controller clock, an emulated liveness gate: X,Z[,RESYNC] (a loop cell of
        /// the column): the bench's slot clock and every emulated timer and pulse (the release,
        /// the stand's gates, the meter, the head) advance only while that cell's chunk ticks
        /// entities; during a freeze the world ticks alone, redstone running; RESYNC 1: after a
        /// pause, wait until the slot clock is back in phase with gameTime (the next summon).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        pause_clock: Vec<i32>,
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
    /// (ironspec 17:00, 01:16) The lead's bed stop (F1 + Q2, under the station; loop = local +
    /// (0,-18,-1)): write its design (`--bench`: with a run-out past R and the two-visit bench's
    /// queue cell).
    Bedtier {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        bench: bool,
    },
    /// The bed stop's one-visit bench (a `bedtier --bench` design) over seeds and entry speeds: F
    /// --after-sleep live gt after the lie-down, the release --release-after live gt after F. One
    /// row with --reload/--freeze START,LEN and --hour DAYTIME; a sweep over --reload-phases (each
    /// a restart of --sweep-len gt: 1 is a save, stop and start), --freeze-phases and --hours.
    /// Prints each visit (one row) or each row's clean count, and the totals: seated, not out,
    /// HOME losses, the pickup speeds, each exit's delay against the same seed and speed's night.
    BedtierBench {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        from: u64,
        #[arg(long, default_value_t = 8)]
        to: u64,
        #[arg(long, default_value = "0.1,0.2,0.3")]
        speeds: String,
        #[arg(long, default_value_t = 10)]
        after_sleep: i64,
        #[arg(long, default_value_t = 30)]
        release_after: i64,
        /// By day (2,000): the napper should not sleep.
        #[arg(long)]
        day: bool,
        #[arg(long)]
        empty: bool,
        #[arg(long)]
        no_home: bool,
        #[arg(long)]
        hide_after_wake: bool,
        #[arg(long)]
        freeze: Option<String>,
        #[arg(long)]
        reload: Option<String>,
        #[arg(long)]
        hour: Option<i64>,
        #[arg(long)]
        reload_phases: Option<String>,
        #[arg(long)]
        freeze_phases: Option<String>,
        #[arg(long)]
        hours: Option<String>,
        #[arg(long, default_value_t = 1)]
        sweep_len: i64,
    },
    /// (G1's point 1, ironspec 00:52) The two-visit row on the lead's geometry: A's visit, the next
    /// napper C waiting awake in its cart at the queue cell, the queue released on A's R (C at the
    /// entry --transit gt later), A up its lift (emulated, from --lift-delay gt after R at
    /// --lift-speed b/t); a restart (one tick) at each --restart-phases gt after A's entry (and a
    /// row without); prints A's distance to the bed's head at C's lie-down (bar > 17) and both
    /// HOMEs.
    BedtierTwo {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        from: u64,
        #[arg(long, default_value_t = 8)]
        to: u64,
        #[arg(long, default_value = "0.1,0.2,0.3")]
        speeds: String,
        #[arg(long)]
        restart_phases: Option<String>,
        #[arg(long, default_value_t = 37)]
        transit: i64,
        #[arg(long, default_value_t = 5)]
        lift_delay: i64,
        #[arg(long, default_value_t = 0.7)]
        lift_speed: f64,
    },
}

fn load(_version: &str, path: &PathBuf) -> Result<(circuit::Circuit, redsim::entity::mob::farm::FarmSpec)> {
    let c = circuit::Circuit::load(path)?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow::anyhow!(e))?;
    Ok((c, spec))
}

/// One `--track NAME=x,y,z;...` value: its name, waypoints, and whether its first and last
/// waypoints are context cells (`@x,y,z`: another part's rail, shaping the neighbour, not laid).
fn column_track(t: &str) -> Result<(String, Vec<[i32; 3]>, [bool; 2])> {
    let (name, pts) = t.split_once('=').ok_or_else(|| anyhow::anyhow!("--track {t}: NAME=x,y,z;x,y,z;... (an end waypoint @x,y,z: a context cell)"))?;
    let words: Vec<&str> = pts.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();
    let mut out = Vec::new();
    let mut context = [false, false];
    for (i, w) in words.iter().enumerate() {
        let (w, at) = match w.strip_prefix('@') {
            Some(r) => (r, true),
            None => (*w, false),
        };
        if at {
            anyhow::ensure!(i == 0 || i + 1 == words.len(), "--track {t}: only an end waypoint can be a context cell (@)");
            context[if i == 0 { 0 } else { 1 }] = true;
        }
        let n: Vec<i32> = w.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
        anyhow::ensure!(n.len() == 3, "--track {t}: {w}: give x,y,z");
        out.push([n[0], n[1], n[2]]);
    }
    Ok((name.trim().to_string(), out, context))
}

/// `mobfarm column`'s sketch part from its `--track`, `--cart` and `--post` flags (none: no part).
fn column_sketch(tracks: &[String], carts: &[String], posts: &[String]) -> Result<Vec<crate::column::Part>> {
    if tracks.is_empty() && carts.is_empty() && posts.is_empty() {
        return Ok(Vec::new());
    }
    let cell = |t: &str| -> Result<[i32; 3]> {
        let n: Vec<i32> = t.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
        anyhow::ensure!(n.len() == 3, "{t}: give x,y,z");
        Ok([n[0], n[1], n[2]])
    };
    let mut tr = Vec::new();
    for t in tracks {
        tr.push(column_track(t)?);
    }
    let mut cs = Vec::new();
    for c in carts {
        let (rest, empty) = match c.strip_suffix(",empty") {
            Some(r) => (r, true),
            None => (c.as_str(), false),
        };
        let f: Vec<&str> = rest.split(',').collect();
        anyhow::ensure!(f.len() == 3 || f.len() == 4, "--cart {c}: x,y,z[,Y][,empty]");
        let y: Option<f64> = match f.get(3) {
            Some(t) => Some(t.trim().parse()?),
            None => None,
        };
        cs.push((cell(&f[..3].join(","))?, y, empty));
    }
    let ps: Vec<[i32; 3]> = posts.iter().map(|p| cell(p)).collect::<Result<_>>()?;
    crate::column::sketch_part(&tr, &cs, &ps).map_err(|e| anyhow::anyhow!(e))
}

/// `mobfarm column --out`: the skeleton written (its `ground:`, moved by `shift` into the
/// written frame, the header naming both), then every rail junction settled to the shape its
/// as-placed power gives (paste's default, ironspec Nov 11 15:56), each listed.
#[allow(clippy::too_many_arguments)]
fn column_write(version: &str, out: &PathBuf, d: mctas::design::Design, head: &str, f: &crate::column::Frame, ground: Option<i32>, shift: [i32; 3], no_settle: bool) -> Result<()> {
    let mut d = d;
    if let Some(g) = ground {
        d.meta.insert("ground".into(), g.to_string());
    }
    let mut d = crate::column::shifted(&d, shift).map_err(|e| anyhow::anyhow!(e))?;
    let title = format!(
        "# The one-chunk column's skeleton (mctas mobfarm column; no wiring): column x {}..{} z {}..{}{}.\n",
        f.x0,
        f.x1,
        f.z0,
        f.z1,
        if shift == [0, 0, 0] { String::new() } else { format!(", WRITTEN in the frame column + ({},{},{}): every cell below is the column's moved by that", shift[0], shift[1], shift[2]) }
    );
    std::fs::write(out, format!("{title}{head}{}", d.to_text()))?;
    if !no_settle {
        let (_c, spec) = load(version, out)?;
        let settled = crate::queue::settled_junctions(&spec).map_err(|e| anyhow::anyhow!(e))?;
        let mut lines = String::new();
        for (c, from, to, sig) in &settled {
            let line = format!("{},{},{}: stored {}, settled to {} (its signal {})", c[0], c[1], c[2], from.name(), to.name(), if *sig { "on" } else { "off" });
            if let Some(s) = d.blocks.get_mut(c) {
                *s = s.replace(&format!("shape={}", from.name()), &format!("shape={}", to.name()));
            }
            println!("settled: {line}");
            lines.push_str(&format!("# settled junction: {line}\n"));
        }
        println!("junctions settled: {}", settled.len());
        if !settled.is_empty() {
            std::fs::write(out, format!("{title}{head}{lines}{}", d.to_text()))?;
        }
    }
    println!("{}", out.display());
    Ok(())
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

/// (tickexact 2026-11-11, ironspec's sweep) `mctas mobfarm --voxel-trace`: print each move a block
/// shape's voxel grid stops differently from a box test (`redsim::entity::collide`).
pub fn set_voxel_trace(on: bool) {
    redsim::entity::collide::set_voxel_trace(on);
}

/// Those moves so far (printed after every mobfarm command: 0 expected).
pub fn voxel_stops() -> u64 {
    redsim::entity::collide::voxel_stops()
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
        MobfarmCmd::Exit { design, seed, positions, offset, part_flood } => {
            let (c, spec) = load(version, &design)?;
            let part = part_flood || !spec.has_bed();
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
        MobfarmCmd::Paste { base, part, at, r#box, overworld_only, mirror_z, keep_base, remove_box, set_port, out, keep, clear, no_settle } => {
            let o: Vec<i32> = at.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(o.len() == 3, "--at x,y,z");
            anyhow::ensure!(r#box.is_empty() || r#box.len() == 6, "--box X0,Y0,Z0,X1,Y1,Z1");
            anyhow::ensure!(remove_box.is_empty() || remove_box.len() == 6, "--remove-box X0,Y0,Z0,X1,Y1,Z1");
            anyhow::ensure!(!(keep_base && keep.is_some()), "--keep-base is `--keep base`: one of them");
            let keep_part = match keep.as_deref() {
                None if keep_base => Some(false),
                None => None,
                Some("base") => Some(false),
                Some("part") => Some(true),
                Some(k) => anyhow::bail!("--keep {k}: base or part"),
            };
            let mut d = mctas::design::Design::parse(&std::fs::read_to_string(&base)?)?;
            // (`--clear`: the base's boxes emptied first, the cells the part replaces; `--remove-box`
            // one more box, by its older name)
            let mut cleared: Vec<String> = Vec::new();
            let remove_one = (remove_box.len() == 6).then(|| remove_box.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","));
            for bx in clear.as_deref().unwrap_or("").split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).chain(remove_one) {
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
            let mut p = mctas::design::Design::parse(&std::fs::read_to_string(&part)?)?;
            // (`--box`: only the part's cells and entities inside the box, its own frame)
            if r#box.len() == 6 {
                let b = r#box.clone();
                let inside = |c: &[i32; 3]| (0..3).all(|k| c[k] >= b[k].min(b[k + 3]) && c[k] <= b[k].max(b[k + 3]));
                p.blocks.retain(|c, _| inside(c));
                p.block_entities.retain(|c, _| inside(c));
                p.entities.retain(|e| inside(&[e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]));
            }
            // (`--overworld-only`: the copy shares the base's ports, controls and Nether side)
            if overworld_only {
                p.ports.clear();
                p.nether_blocks.clear();
                p.nether_block_entities.clear();
                p.nether_ports.clear();
                for k in ["build", "station", "declared", "flights"] {
                    p.meta.remove(k);
                }
            }
            // (`--mirror-z`: z to -z in the part's frame, north and south swapped in each state)
            if mirror_z {
                let flip = |s: &str| -> String {
                    let Some((name, rest)) = s.split_once('[') else { return s.to_string() };
                    let props = rest.trim_end_matches(']');
                    let swap = |v: &str| match v {
                        "north" => "south".to_string(),
                        "south" => "north".to_string(),
                        "ascending_north" => "ascending_south".to_string(),
                        "ascending_south" => "ascending_north".to_string(),
                        "north_east" => "south_east".to_string(),
                        "south_east" => "north_east".to_string(),
                        "north_west" => "south_west".to_string(),
                        "south_west" => "north_west".to_string(),
                        v => v.to_string(),
                    };
                    let kv: Vec<(String, String)> = props.split(',').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
                    let get = |k: &str| kv.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
                    let (n, so) = (get("north"), get("south"));
                    let out: Vec<String> = kv.iter().map(|(k, v)| match k.as_str() {
                        "north" => format!("north={}", so.clone().unwrap_or_else(|| v.clone())),
                        "south" => format!("south={}", n.clone().unwrap_or_else(|| v.clone())),
                        "facing" | "shape" | "axis" => format!("{k}={}", swap(v)),
                        _ => format!("{k}={v}"),
                    }).collect();
                    format!("{name}[{}]", out.join(","))
                };
                p.blocks = p.blocks.iter().map(|(c, s)| ([c[0], c[1], -c[2]], flip(s))).collect();
                p.block_entities = p.block_entities.iter().map(|(c, s)| ([c[0], c[1], -c[2]], s.clone())).collect();
                for e in &mut p.entities {
                    // (cell z goes to cell -z, i.e. [z, z+1) to [-z, 1-z): a point p to 1 - p)
                    e.pos[2] = 1.0 - e.pos[2];
                }
                for c in p.ports.values_mut() {
                    c[2] = -c[2];
                }
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
            for sp in &set_port {
                let (k, v) = sp.split_once('=').ok_or_else(|| anyhow::anyhow!("--set-port NAME=x,y,z: {sp}"))?;
                let c: Vec<i32> = v.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(c.len() == 3, "--set-port NAME=x,y,z: {sp}");
                d.ports.insert(k.to_string(), [c[0], c[1], c[2]]);
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
            // (ironspec Nov 11 15:46: the borders report is a default of the assembly as of the
            // runner: the pasted file's chunk-line crossings, its own frame taken as the world's,
            // as `queue-loop-run` takes it)
            println!("{}", crate::borders::summary(&d.blocks, [0, 0], 12));
            // (the slope-to-slope peak check, 9fc3155f Nov 11: a top no cart crosses)
            println!("{}", crate::borders::peaks_line(&d.blocks).0);
        }
        MobfarmCmd::PortalLinks { ow, nether, expect, margin } => {
            let ow: Vec<_> = ow.iter().map(|s| crate::portallinks::parse(s)).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let nether: Vec<_> = nether.iter().map(|s| crate::portallinks::parse(s)).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let mut ex = std::collections::BTreeMap::new();
            for e in &expect {
                let (a, b) = e.split_once('=').ok_or_else(|| anyhow::anyhow!("--expect SOURCE=TARGET"))?;
                ex.insert(a.trim().to_string(), b.trim().to_string());
            }
            print!("{}", crate::portallinks::report(&ow, &nether, &ex, margin));
        }
        MobfarmCmd::L1ItemBench { design_out, width, height, fire_y, far_rows, floor, items, ticks, seeds, freeze, login_each, login_before_arrival, stop, stop_every, restart_order, piglin, mob_jump, trace } => {
            let path = design_out.unwrap_or_else(|| std::env::temp_dir().join(format!("l1-item-bench-{}.txt", std::process::id())));
            anyhow::ensure!((13..=9 + height).contains(&fire_y), "--fire-y between 13 and the top row (9 + height)");
            let far_rows = far_rows.unwrap_or(2).clamp(2, height);
            let hopper_floor = match floor.as_str() {
                "hoppers" => true,
                "ice" => false,
                o => anyhow::bail!("--floor {o}: hoppers or ice"),
            };
            std::fs::write(&path, crate::l1items::design_text(width, height, fire_y, far_rows, hopper_floor, items).map_err(|e| anyhow::anyhow!(e))?)?;
            let (_c, spec) = load(version, &path)?;
            let freezes = parse_freezes(&freeze)?;
            let seeds: Vec<i64> = match seeds.split_once("..") {
                Some((a, b)) => (a.trim().parse::<i64>()?..=b.trim().parse::<i64>()?).collect(),
                None => (1..=seeds.trim().parse::<i64>()?).collect(),
            };
            let piglins: Vec<(i64, i32, u8, u8)> = piglin.iter().map(|p| {
                let f: Vec<&str> = p.split(',').map(|s| s.trim()).collect();
                anyhow::ensure!((2..=5).contains(&f.len()), "--piglin T,X[,a|b|both[,adult|baby|jockey[,sword]]]");
                let side = match f.get(2).copied().unwrap_or("both") {
                    "a" => 1u8,
                    "b" => 2,
                    "both" => 3,
                    o => anyhow::bail!("--piglin side {o}: a, b or both"),
                };
                let kind = match f.get(3).copied().unwrap_or("adult") {
                    "adult" => 0u8,
                    "baby" => 1,
                    "jockey" => 2,
                    o => anyhow::bail!("--piglin kind {o}: adult, baby or jockey"),
                };
                let armed = match f.get(4).copied() {
                    None | Some("") => 0u8,
                    Some("sword") => 4,
                    Some(o) => anyhow::bail!("--piglin armed {o}: sword or nothing"),
                };
                Ok((f[0].parse()?, f[1].parse()?, side, kind | armed))
            }).collect::<Result<_>>()?;
            let mut stops = stop;
            if let Some(n) = stop_every {
                anyhow::ensure!(n > 2, "--stop-every N > 2");
                let mut t = n;
                let mut k = 0;
                while t + 1 < ticks {
                    stops.push(t);
                    k += 1;
                    t = (k + 1) * n + k;
                }
            }
            let orders: Vec<crate::liveness::Restart> = if restart_order == "all" {
                crate::liveness::Restart::rows()
            } else {
                vec![crate::liveness::Restart::parse(&restart_order).ok_or_else(|| anyhow::anyhow!("--restart-order {restart_order}: save, reverse, seed:K or all"))?]
            };
            println!("{}", path.display());
            print!("{}", crate::l1items::spawn_report(width, height, fire_y, far_rows, hopper_floor));
            for restart in orders {
                let o = crate::l1items::Opts { width, height, fire_y, far_rows, hopper_floor, items, ticks, freezes: freezes.clone(), login_each, login_before_arrival, stops: stops.clone(), restart, piglins: piglins.clone(), mob_jump, seeds: seeds.clone(), trace };
                if !stops.is_empty() {
                    println!("== the restarts' load order: {}", restart.label());
                }
                print!("{}", crate::l1items::bench(&spec, &o).map_err(|e| anyhow::anyhow!(e))?);
            }
        }
        MobfarmCmd::L1Bench { design_out, ticks, hold, pulse, stagger, freeze, piglin, trace } => {
            let path = design_out.unwrap_or_else(|| std::env::temp_dir().join(format!("l1-bench-{}.txt", std::process::id())));
            std::fs::write(&path, crate::l1bench::design_text())?;
            let (_c, spec) = load(version, &path)?;
            let st: Vec<i64> = stagger.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(st.len() == 3, "--stagger A1,B1,A2");
            let freezes = parse_freezes(&freeze)?;
            let o = crate::l1bench::Opts { ticks, hold, pulse, stagger: [st[0], st[1], st[2]], freezes, piglins: piglin, trace };
            println!("{}", path.display());
            print!("{}", crate::l1bench::bench(&spec, &o).map_err(|e| anyhow::anyhow!(e))?);
        }
        MobfarmCmd::SummonChance { design, pad, stand, per_day } => {
            let (_c, spec) = load(version, &design)?;
            let b = crate::column::Reserve::parse(&format!("pad={pad}")).map_err(|e| anyhow::anyhow!(e))?.b;
            let cells: Vec<[i32; 3]> = stand.iter().map(|t| {
                let n: Vec<i32> = t.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(n.len() == 3, "--stand x,y,z");
                Ok([n[0], n[1], n[2]])
            }).collect::<Result<_>>()?;
            print!("{}", crate::columnrun::summon_chance(&spec, b, &cells, per_day).map_err(|e| anyhow::anyhow!(e))?);
        }
        MobfarmCmd::ColumnRun { design, cycles, warm, seed, close_at, slope_delay, g2_plus, hold_shut_delay, pulse, laps, trace } => {
            let (c, spec) = load(version, &design)?;
            let o = crate::columnrun::Opts { cycles, warm, seed, close_at, slope_delay, g2_plus, hold_shut_delay, pulse, laps, trace };
            print!("{}", crate::columnrun::run(&spec, &c.clock.design.meta, &o).map_err(|e| anyhow::anyhow!(e))?);
        }
        MobfarmCmd::Column { parts, reserves, reserves_from, reserve_skip, ports, tracks, carts, posts, clears, column, chunk_phase, plan, name, no_settle, ground, out_shift, no_riders, positions_out, path, notes, bed, bed_exempt, sentinel, sections, summon_box, golem_box, run_gate, run_cell, run_controller, notes_file, pockets, block, wait_box, out } => {
            let mut notes = notes;
            if let Some(nf) = &notes_file {
                notes.extend(std::fs::read_to_string(nf)?.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from));
            }
            let mut run_gate = run_gate;
            let mut run_cell = run_cell;
            let mut extra_parts: Vec<crate::column::Part> = Vec::new();
            let p3 = |t: &str| -> Result<[i32; 3]> {
                let n: Vec<i32> = t.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(n.len() == 3, "{t}: x,y,z");
                Ok([n[0], n[1], n[2]])
            };
            for pk in &pockets {
                let (nm, rest) = pk.split_once('=').ok_or_else(|| anyhow::anyhow!("--pockets {pk}: NAME=x,y,z,DIR,N[,rstar]"))?;
                let f: Vec<&str> = rest.split(',').collect();
                anyhow::ensure!(f.len() == 5 || f.len() == 6, "--pockets {pk}: NAME=x,y,z,DIR,N[,rstar]");
                let start = p3(&f[..3].join(","))?;
                let (part, gs, cs) = crate::column::pocket_part(nm, start, f[3], f[4].trim().parse()?, f.get(5).is_some_and(|s| s.trim() == "rstar")).map_err(|e| anyhow::anyhow!(e))?;
                for (g, c, s) in gs {
                    run_gate.push(format!("{g}={},{},{}/{},{},{}", c[0], c[1], c[2], s[0], s[1], s[2]));
                }
                for (n, c) in cs {
                    run_cell.push(format!("{n}={},{},{}", c[0], c[1], c[2]));
                }
                extra_parts.push(part);
            }
            if !block.is_empty() {
                let bs: Vec<([i32; 3], String)> = block.iter().map(|b| {
                    let (c, s) = b.split_once('=').ok_or_else(|| anyhow::anyhow!("--block {b}: x,y,z=STATE"))?;
                    Ok((p3(c)?, s.to_string()))
                }).collect::<Result<_>>()?;
                extra_parts.push(crate::column::block_part(&bs));
            }
            let wait_boxes: Vec<([i32; 3], [i32; 3])> = wait_box.iter().map(|s| crate::column::Reserve::parse(&format!("wait={s}")).map(|r| r.b)).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let summoners: Vec<[i32; 3]> = summon_box.iter().map(|t| {
                let n: Vec<i32> = t.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(n.len() == 3, "--summon-box x,y,z");
                Ok([n[0], n[1], n[2]])
            }).collect::<Result<_>>()?;
            let golem_b: Option<([i32; 3], [i32; 3])> = match golem_box.as_deref() {
                None => None,
                Some(b) => Some(crate::column::Reserve::parse(&format!("golem={b}")).map_err(|e| anyhow::anyhow!(e))?.b),
            };
            anyhow::ensure!(summoners.is_empty() || golem_b.is_some(), "--summon-box needs --golem-box");
            let clear: Vec<([i32; 3], [i32; 3])> = clears.iter().map(|s| crate::column::Reserve::parse(&format!("clear={s}")).map(|r| r.b)).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let specs: Vec<crate::column::PartSpec> = parts.iter().map(|s| crate::column::PartSpec::parse(s)).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let mut reserves = reserves;
            if let Some(f) = &reserves_from {
                reserves.extend(crate::column::reserves_from_header(&std::fs::read_to_string(f)?, &reserve_skip));
            }
            let reserves: Vec<crate::column::Reserve> = reserves.iter().map(|s| crate::column::Reserve::parse(s)).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let mut given: Vec<(String, [i32; 3])> = Vec::new();
            for p in &ports {
                let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("--port {p}: NAME=x,y,z"))?;
                let n: Vec<i32> = v.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(n.len() == 3, "--port {p}: NAME=x,y,z");
                given.push((k.trim().to_string(), [n[0], n[1], n[2]]));
            }
            anyhow::ensure!(!specs.is_empty(), "give at least one --part");
            let c: Vec<i32> = column.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(c.len() == 4, "--column x0,z0,x1,z1");
            let ph: Vec<i32> = chunk_phase.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(ph.len() == 2, "--chunk-phase PX,PZ");
            let f = crate::column::Frame { x0: c[0], z0: c[1], x1: c[2], z1: c[3], phase_x: ph[0], phase_z: ph[1] };
            let bed_cell: Option<[i32; 3]> = match bed.as_deref() {
                None => None,
                Some(b) => {
                    let n: Vec<i32> = b.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(n.len() == 3, "--bed x,y,z");
                    Some([n[0], n[1], n[2]])
                }
            };
            let sentinel_box: Option<([i32; 3], [i32; 3])> = match sentinel.as_deref() {
                None => None,
                Some(b) => Some(crate::column::Reserve::parse(&format!("sentinel={b}")).map_err(|e| anyhow::anyhow!(e))?.b),
            };
            let shift_w: [i32; 3] = match out_shift.as_deref() {
                None => [0, 0, 0],
                Some(s) => {
                    let n: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(n.len() == 3, "--out-shift dx,dy,dz");
                    [n[0], n[1], n[2]]
                }
            };
            let mut sketch = column_sketch(&tracks, &carts, &posts)?;
            sketch.extend(extra_parts);
            let (text, d, bed_ok) = crate::column::run_full(&specs, sketch, &clear, &reserves, &given, &f, &name, plan, bed_cell.map(|b| (b, bed_exempt.as_slice())), sentinel_box, shift_w, golem_b.map(|g| (summoners.as_slice(), g))).map_err(|e| anyhow::anyhow!(e))?;
            print!("{text}");
            let paths: Vec<(String, Vec<[i32; 3]>)> = path.iter().map(|p| crate::column::path_cells(p)).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let mut bed_ok = bed_ok;
            if let (Some(b), false) = (bed_cell, paths.is_empty()) {
                let (t, ok) = crate::column::path_bed_radius(&paths, b, &bed_exempt);
                print!("{t}");
                bed_ok &= ok;
            }
            if !wait_boxes.is_empty() {
                let g = golem_b.ok_or_else(|| anyhow::anyhow!("--wait-box needs --golem-box"))?;
                print!("{}", crate::column::waiting_check(&d, &wait_boxes, g, shift_w).0);
            }
            if sections {
                let mut beds: Vec<[i32; 3]> = bed_cell.into_iter().collect();
                if let Some(sb) = sentinel_box {
                    beds.push(sb.0);
                }
                anyhow::ensure!(!beds.is_empty(), "--sections needs --bed (and --sentinel)");
                print!("{}", crate::column::section_report(&d, &beds, shift_w));
            }
            if let Some(pf) = &positions_out {
                let sh: [i32; 3] = match out_shift.as_deref() {
                    None => [0, 0, 0],
                    Some(s) => {
                        let n: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(n.len() == 3, "--out-shift dx,dy,dz");
                        [n[0], n[1], n[2]]
                    }
                };
                let mut cells: std::collections::BTreeSet<[i32; 3]> = Default::default();
                for e in d.entities.iter().filter(|e| e.kind.ends_with("minecart") && e.nbt.as_deref().is_some_and(|t| t.contains("villager"))) {
                    cells.insert(e.cell());
                }
                for t in &tracks {
                    let (_, pts, _) = column_track(t)?;
                    for c in crate::column::track_cells(&pts).map_err(|e| anyhow::anyhow!(e))? {
                        cells.insert(c);
                    }
                }
                // (every rail cell of every part: a rider passes each, the stand's approach and
                // departure and the hold's and tail chain's own rails included)
                for (c, s) in &d.blocks {
                    if crate::column::rail_shape_of(s).is_some() {
                        cells.insert(*c);
                    }
                }
                // (every declared off-rail path cell: shafts, lifts, falls, a hold with no cart)
                for (_, pc) in &paths {
                    for c in pc {
                        cells.insert(*c);
                    }
                }
                let body: String = cells.iter().map(|c| format!("{} {} {}\n", c[0] + sh[0], c[1] + sh[1], c[2] + sh[2])).collect();
                std::fs::write(pf, format!("# every villager position of {name} (riders' cells, every rail cell of every part and every declared off-rail path cell), the written frame\n{body}"))?;
                println!("positions: {} cells -> {}", cells.len(), pf.display());
            }
            let mut d = d;
            if !run_gate.is_empty() {
                d.meta.insert("run-gates".into(), run_gate.iter().map(|g| g.replacen('=', " ", 1)).collect::<Vec<_>>().join("; "));
            }
            if !run_cell.is_empty() {
                d.meta.insert("run-cells".into(), run_cell.iter().map(|g| g.replacen('=', " ", 1)).collect::<Vec<_>>().join("; "));
            }
            if let Some(c) = &run_controller {
                d.meta.insert("run-controller".into(), c.clone());
            }
            if no_riders {
                let n = d.entities.len();
                d.entities.retain(|e| !(e.kind.ends_with("minecart") && e.nbt.as_deref().is_some_and(|t| t.contains("villager"))));
                println!("riders dropped: {} (the summoners come from `mobfarm exit --positions`)", n - d.entities.len());
            }
            if let Some(out) = out {
                let head: String = notes.iter().map(|n| format!("# {n}\n")).chain(specs.iter().map(|s| format!("# + {}: {} moved by {},{},{} into the frame, then {},{},{} into its tier{}\n", s.name, s.file.display(), s.frame[0], s.frame[1], s.frame[2], s.tier[0], s.tier[1], s.tier[2], s.keep.map(|k| format!(" (its cells {},{},{}..{},{},{} kept)", k.0[0], k.0[1], k.0[2], k.1[0], k.1[1], k.1[2])).unwrap_or_default()))).chain(clear.iter().map(|b| format!("# cleared first (committed cells a lane re-lays): x {}..{} y {}..{} z {}..{}\n", b.0[0], b.1[0], b.0[1], b.1[1], b.0[2], b.1[2]))).chain(tracks.iter().map(|t| format!("# sketch track (plain rails, no control; a lane's cells replace it): {t}\n"))).chain(reserves.iter().map(|r| format!("# reserved (no blocks): {} x {}..{} y {}..{} z {}..{}\n", r.name, r.b.0[0], r.b.1[0], r.b.0[1], r.b.1[1], r.b.0[2], r.b.1[2]))).collect();
                let shift: [i32; 3] = match out_shift.as_deref() {
                    None => [0, 0, 0],
                    Some(s) => {
                        let n: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(n.len() == 3, "--out-shift dx,dy,dz");
                        [n[0], n[1], n[2]]
                    }
                };
                column_write(version, &out, d, &head, &f, ground, shift, no_settle)?;
            }
            if !bed_ok {
                std::process::exit(1);
            }
        }
        MobfarmCmd::Box { design, r#box, list } => {
            anyhow::ensure!(r#box.is_empty() || r#box.len() == 6, "--box X0,Y0,Z0,X1,Y1,Z1");
            let d = mctas::design::Design::parse(&std::fs::read_to_string(&design)?)?;
            let b = r#box.clone();
            let inside = |c: &[i32; 3]| b.len() != 6 || (0..3).all(|k| c[k] >= b[k].min(b[k + 3]) && c[k] <= b[k].max(b[k + 3]));
            let cells: Vec<(&[i32; 3], &String)> = d.blocks.iter().filter(|(c, s)| inside(c) && s.trim_start_matches("minecraft:") != "air").collect();
            anyhow::ensure!(!cells.is_empty(), "no block in the box");
            let lo = |k: usize| cells.iter().map(|(c, _)| c[k]).min().unwrap_or(0);
            let hi = |k: usize| cells.iter().map(|(c, _)| c[k]).max().unwrap_or(0);
            let ext = [hi(0) - lo(0) + 1, hi(1) - lo(1) + 1, hi(2) - lo(2) + 1];
            let chunks: std::collections::BTreeSet<(i32, i32)> = cells.iter().map(|(c, _)| (c[0].div_euclid(16), c[2].div_euclid(16))).collect();
            let (cx0, cx1) = (chunks.iter().map(|c| c.0).min().unwrap_or(0), chunks.iter().map(|c| c.0).max().unwrap_or(0));
            let (cz0, cz1) = (chunks.iter().map(|c| c.1).min().unwrap_or(0), chunks.iter().map(|c| c.1).max().unwrap_or(0));
            let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for (_, s) in &cells {
                *kinds.entry(s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string()).or_default() += 1;
            }
            let ents = d.entities.iter().filter(|e| inside(&[e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32])).count();
            println!("{}: {} overworld blocks{}, box x {}..{} y {}..{} z {}..{} = {}x{}x{} = {} volume; chunk columns X {}..{} Z {}..{} ({} with blocks of {} in the box); {} entities", design.display(), cells.len(), if b.len() == 6 { format!(" in the box {:?}", b) } else { String::new() }, lo(0), hi(0), lo(1), hi(1), lo(2), hi(2), ext[0], ext[1], ext[2], ext[0] as i64 * ext[1] as i64 * ext[2] as i64, cx0, cx1, cz0, cz1, chunks.len(), (cx1 - cx0 + 1) * (cz1 - cz0 + 1), ents);
            let mut v: Vec<(String, usize)> = kinds.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            println!("  {}", v.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", "));
            if list {
                let mut cs = cells.clone();
                cs.sort_by_key(|(c, _)| (c[1], c[2], c[0]));
                for (c, s) in cs {
                    println!("{} {} {} {}", c[0], c[1], c[2], s.trim_start_matches("minecraft:"));
                }
            }
        }
        MobfarmCmd::ZombieCells { design, region, rest, top } => {
            anyhow::ensure!(region.len() == 6, "--region X0,Y0,Z0,X1,Y1,Z1");
            anyhow::ensure!(rest.len() % 3 == 0 && !rest.is_empty(), "--rest x,y,z,...");
            let d = mctas::design::Design::parse(&std::fs::read_to_string(&design)?)?;
            // (the rider positions: three points along each rail cell's axis, at the cart's y)
            let mut riders: Vec<[f64; 3]> = Vec::new();
            for (c, s) in &d.blocks {
                if !s.contains("rail") {
                    continue;
                }
                let (x, y, z) = (c[0] as f64, c[1] as f64 + 0.0625, c[2] as f64);
                for t in [0.1, 0.5, 0.9] {
                    riders.push(if s.contains("east_west") || s.contains("ascending_east") || s.contains("ascending_west") {
                        [x + t, y, z + 0.5]
                    } else if s.contains("north_south") || s.contains("ascending_north") || s.contains("ascending_south") {
                        [x + 0.5, y, z + t]
                    } else {
                        [x + 0.5, y, z + 0.5]
                    });
                }
            }
            let rests: Vec<[f64; 3]> = rest.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
            let dist = |a: [f64; 3], b: [f64; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            // (sight: sample the segment every 0.1; a mob's line of sight is clipped by every block
            // with a collision shape (ClipContext.Block.COLLIDER: glass and leaves block it), so only
            // air, rails, dust, torches, tripwire, buttons, levers and plates let it through)
            let clear = |s: &str| {
                let n = s.trim_start_matches("minecraft:").split('[').next().unwrap_or("");
                n == "air" || n.contains("rail") || n == "redstone_wire" || n.contains("torch") || n.contains("tripwire") || n.contains("button") || n.contains("lever") || n.contains("pressure_plate") || (n.contains("fence_gate") && s.contains("open=true"))
            };
            let sees = |eye: [f64; 3], head: [f64; 3]| -> bool {
                let n = (dist(eye, head) / 0.1).ceil() as usize;
                (1..n).all(|k| {
                    let f = k as f64 / n as f64;
                    let p = [eye[0] + (head[0] - eye[0]) * f, eye[1] + (head[1] - eye[1]) * f, eye[2] + (head[2] - eye[2]) * f];
                    let c = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
                    d.blocks.get(&c).is_none_or(|s| clear(s))
                })
            };
            let r = &region;
            let mut rows: Vec<(f64, f64, [i32; 3], usize)> = Vec::new();
            for x in r[0].min(r[3])..=r[0].max(r[3]) {
                for y in r[1].min(r[4])..=r[1].max(r[4]) {
                    for z in r[2].min(r[5])..=r[2].max(r[5]) {
                        if d.blocks.get(&[x, y, z]).is_some_and(|s| !clear(s)) {
                            continue;
                        }
                        let f = [x as f64 + 0.5, y as f64, z as f64 + 0.5];
                        let near = riders.iter().map(|p| dist(*p, f)).fold(f64::MAX, f64::min);
                        let far = rests.iter().map(|p| dist(*p, f)).fold(0.0, f64::max);
                        if near > 6.0 && far <= 8.0 {
                            let seen = rests.iter().filter(|p| sees([p[0], p[1] + 1.62, p[2]], [f[0], f[1] + 1.74, f[2]])).count();
                            rows.push((near, far, [x, y, z], seen));
                        }
                    }
                }
            }
            rows.sort_by(|a, b| (b.3, (b.0 - 6.0).min(8.0 - b.1)).partial_cmp(&(a.3, (a.0 - 6.0).min(8.0 - a.1))).unwrap_or(std::cmp::Ordering::Equal));
            println!("{}: {} rider positions, {} resting riders; {} candidate cells meet > 6 from every rider position and <= 8 from every resting rider", design.display(), riders.len(), rests.len(), rows.len());
            for (near, far, c, seen) in rows.iter().take(top) {
                println!("  zombie feet at {:.1},{},{:.1}: nearest rider position {:.2} (margin {:.2}), farthest resting rider {:.2} (margin {:.2}), sight to {seen} of {} resting riders", c[0] as f64 + 0.5, c[1], c[2] as f64 + 0.5, near, near - 6.0, far, 8.0 - far, rests.len());
            }
        }
        MobfarmCmd::Lines { rail, booster, wire, block, out } => {
            let mut cells: std::collections::BTreeMap<[i32; 3], String> = std::collections::BTreeMap::new();
            let pt = |s: &str| -> anyhow::Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "a point is x,y,z: {s}");
                Ok([v[0], v[1], v[2]])
            };
            // (each segment's cells, along x or z, y stepping toward y1 one block per cell)
            let walk = |seg: &str| -> anyhow::Result<Vec<([i32; 3], &'static str)>> {
                let (a, b) = seg.split_once('>').ok_or_else(|| anyhow::anyhow!("a segment is x0,y0,z0>x1,y1,z1: {seg}"))?;
                let (a, b) = (pt(a)?, pt(b)?);
                anyhow::ensure!(a[0] == b[0] || a[2] == b[2], "a segment runs along x or z: {seg}");
                let (dx, dz) = ((b[0] - a[0]).signum(), (b[2] - a[2]).signum());
                let n = (b[0] - a[0]).abs().max((b[2] - a[2]).abs());
                let mut y = a[1];
                let mut v = Vec::new();
                for i in 0..=n {
                    let dy = if i < n { (b[1] - y).signum() } else { 0 };
                    let shape = match (dx, dz, dy) {
                        (_, _, 0) => if dx != 0 { "east_west" } else { "north_south" },
                        (1, _, 1) | (-1, _, -1) => "ascending_east",
                        (-1, _, 1) | (1, _, -1) => "ascending_west",
                        (_, 1, 1) | (_, -1, -1) => "ascending_south",
                        _ => "ascending_north",
                    };
                    v.push(([a[0] + dx * i, y, a[2] + dz * i], shape));
                    y += dy;
                }
                Ok(v)
            };
            for (list, top, support) in [(&rail, "rail", "minecraft:glass"), (&booster, "powered_rail", "minecraft:redstone_block")] {
                for seg in list.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                    for (c, shape) in walk(seg)? {
                        let st = if top == "rail" { format!("minecraft:rail[shape={shape},waterlogged=false]") } else { format!("minecraft:powered_rail[powered=true,shape={shape},waterlogged=false]") };
                        cells.insert(c, st);
                        cells.entry([c[0], c[1] - 1, c[2]]).or_insert_with(|| support.to_string());
                    }
                }
            }
            for seg in wire.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                for (i, (c, shape)) in walk(seg)?.into_iter().enumerate() {
                    let facing = if shape.contains("east") || shape.contains("west") { "east" } else { "south" };
                    let st = if i > 0 && i % 15 == 0 { format!("minecraft:repeater[delay=1,facing={facing},locked=false,powered=false]") } else { "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".to_string() };
                    cells.insert(c, st);
                    cells.entry([c[0], c[1] - 1, c[2]]).or_insert_with(|| "minecraft:stone".to_string());
                }
            }
            for t in block.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                let (p, s) = t.split_once('=').ok_or_else(|| anyhow::anyhow!("a block is x,y,z=state: {t}"))?;
                cells.insert(pt(p)?, if s.starts_with("minecraft:") { s.to_string() } else { format!("minecraft:{s}") });
            }
            anyhow::ensure!(!cells.is_empty(), "no cells");
            let mut d = mctas::design::Design::parse("name: lines\ncircuit: mob-farm iron_ingot\nport output: 0 0 0\norigin: 0 0 0\npalette:\n  G minecraft:glass\nlayer 0:\n  G\n")?;
            d.blocks.clear();
            for (c, s) in cells {
                d.blocks.insert(c, s);
            }
            std::fs::write(&out, d.to_text())?;
            println!("{}: {} blocks", out.display(), d.blocks.len());
        }
        MobfarmCmd::QueueLoop { slots, run, depart, boost_every, boost_first, straight, drop, vstand, no_tail_v, folded, row_gap, uturn_boost, line, no_tail_deep, plain, stand_desc, detector, zombie_every, zombie_x, stand_deep, tail_desc, peak_pad, short_depart, stand_stop, stand_wired, stand_guard, shift, tail_fifo, tail_esc, climb_boost, tail_brake, tail_plain, climb_every, ledge_slope, esc_door, esc_flat, landing_boost2, esc_g0, esc_adm, adm_ledge, adm_g1, adm_short, adm_qboost, adm_flatb, adm_rboost, adm_pdet, adm_sensors, adm_room_k, adm_latch_reset, adm_xq, tail_post_x, xq_gap, xq_drop, xq_z, row_a, tail_posts, approach, depart_crest, cprime, approach_boost, part_departure, crest_plain_dip, crest_descent, part_handoff, approach_climb_boost, part_allowed, hump, dip_y, dip_flat, boost_slopes, boost_flat, top_flat, top_boost, dip_run, stand_comb, comb_gravity, out } => {
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
            }, approach, depart_crest, cprime: match cprime.as_slice() { [a, b, c, d, e, f] => Some([*a, *b, *c, *d, *e, *f]), [] => None, _ => anyhow::bail!("--cprime takes six delays") }, approach_boost, crest_plain_dip, crest_descent, approach_climb_boost, hump, dip_y, dip_flat, boost_slopes, boost_flat, top_flat, top_boost, dip_run, stand_comb: if stand_stop && approach == 5 && !stand_wired { stand_comb } else { 0 }, comb_gravity: comb_gravity && stand_comb > 0 };
            // (C''''s flags: no unused or contradictory flag; the dip covers z 9 and 10, the line
            // z 9|10 on its flat)
            let humped = hump > 0 || dip_run;
            anyhow::ensure!(!dip_run || (hump == 0 && !boost_slopes && !top_boost), "--dip-run: no --hump, --boost-slopes or --top-boost (S's flat is plain, at the run's level)");
            anyhow::ensure!(!humped || approach == 4 || approach == 5, "--hump and --dip-run need --approach 5 (T at (-3,6,4), approach 5's descent) or 4 (C''s T)");
            anyhow::ensure!(humped || (dip_y == 2 && dip_flat == 2 && !boost_slopes && top_flat == 1 && !top_boost), "--dip-y, --dip-flat, --boost-slopes, --top-flat and --top-boost need --hump or --dip-run");
            anyhow::ensure!(boost_flat == 0 || (boost_slopes && boost_flat as i32 <= boost_first), "--boost-flat needs --boost-slopes and at most --boost-first boosters");
            anyhow::ensure!(top_flat >= 1, "--top-flat takes 1 or more");
            anyhow::ensure!(!humped || !approach_boost, "--hump, --dip-run: drop --approach-boost");
            anyhow::ensure!(!humped || (dip_flat >= 1 && dip_y >= 1 && dip_y <= approach), "--dip-flat takes 1 or more, --dip-y 1..=approach (the climb to T)");
            anyhow::ensure!(!humped || (2 * approach as i64 - dip_y as i64 + 1 <= 9 && 2 * approach as i64 - dip_y as i64 + dip_flat as i64 >= 10), "--hump: the dip's flat must cover z 9 and 10 (the line z 9|10 on flat track)");
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
            // (the approach's column from the head's peak to the snap turn, loop frame: z:y and the
            // cell's kind (B a booster, D the detector, - plain))
            if o.approach > 0 && o.stand_stop {
                let head = lay.slot_cells.last().map_or(0, |c| c[2]);
                let col: Vec<String> = lay.path.iter().filter(|(p, _)| p[0] == -o.row_gap + o.shift[0] && p[2] < head - 1).map(|(p, k)| format!("{}:{}{}", p[2] - o.shift[1], p[1], match k { crate::queue::Cell::Boost => "B", crate::queue::Cell::Detector => "D", _ => "" })).collect();
                println!("approach column x {} (loop z:y, B booster, D detector), the head slot at z {}{}: {}", -o.row_gap, head - o.shift[1], if crate::queue::hump_pad(&o) > 0 { format!(" (moved north {} by the hump)", crate::queue::hump_pad(&o)) } else { String::new() }, col.join(" "));
            }
        }
        MobfarmCmd::Siting { envelope, blocks, floors, low, high, edge, chunks, loader_blocks, extra_trips, floor_torch, array, range, relief, radius, stage, pocket_level, max_pocket_y } => {
            let v: Vec<i32> = envelope.split(['x', 'X']).map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "--envelope {envelope}: W x D x H, e.g. 16x16x69");
            let ix = match stage.as_str() {
                "day-1" | "day1" => 0,
                "middle" => 1,
                "late" | "fastest" => 2,
                s => anyhow::bail!("--stage {s}: day-1, middle or late"),
            };
            let e = crate::siting::Envelope { w: v[0], d: v[1], h: v[2], blocks, floors, low, high, edge, chunks, loader_blocks, extra_trips, floor_torch, array };
            for at in [
                crate::siting::Placement::Sky { range, relief, rod: true },
                crate::siting::Placement::Sky { range, relief, rod: false },
                crate::siting::Placement::Surface { radius, rod: true },
                crate::siting::Placement::Surface { radius, rod: false },
            ] {
                let s = crate::siting::siting(&e, &at, ix).map_err(|e| anyhow::anyhow!(e))?;
                print!("{}", crate::siting::render(&e, &at, &s));
                if let (Some(l), crate::siting::Placement::Sky { .. }) = (pocket_level, &at) {
                    // the pocket at ground top + 1 + lift + its level over the floor
                    println!("  site condition (the standard layout's design choice, no game rule: its kill-room portal at the pads' y, the loop pair 13+ levels away, the Nether side under the roof): the pocket level ({l} over the floor) at y {max_pocket_y} or lower: the ground top under the module at y {} or lower; a higher site's gate is onechunk's portal-links on its own Nether cells (every pad and loop portal to its intended partner by 3D distance, margin 2 or more)", max_pocket_y - 1 - s.lift - l);
                }
            }
        }
        MobfarmCmd::Sky { design, terrain, rod, remove, positions, rails, range, pocket_y, max_pocket_y, no_pocket_cap, out } => {
            let mut extra: Vec<redsim::pos::BlockPos> = Vec::new();
            for f in &positions {
                for line in std::fs::read_to_string(f)?.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                    let v: Vec<i32> = line.split([',', ' ']).filter(|s| !s.is_empty()).map(|n| n.parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "{}: a position takes x y z: {line}", f.display());
                    extra.push(redsim::pos::BlockPos::new(v[0], v[1], v[2]));
                }
            }
            let mut boxes = Vec::new();
            for r in &remove {
                let (a, b) = r.split_once("..").ok_or_else(|| anyhow::anyhow!("--remove {r}: X0,Y0,Z0..X1,Y1,Z1"))?;
                let p = |s: &str| -> Result<redsim::pos::BlockPos> {
                    let v: Vec<i32> = s.split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--remove {r}: X0,Y0,Z0..X1,Y1,Z1");
                    Ok(redsim::pos::BlockPos::new(v[0], v[1], v[2]))
                };
                let (a, b) = (p(a)?, p(b)?);
                boxes.push((redsim::pos::BlockPos::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)), redsim::pos::BlockPos::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z))));
            }
            let o = crate::sky::SkyOpts { rod, remove: boxes, positions: extra, rails, range, pocket: pocket_y.map(|y| (y, max_pocket_y)), capped: !no_pocket_cap };
            let (report, text) = crate::sky::sky(&design, &terrain, &o).map_err(|e| anyhow::anyhow!(e))?;
            std::fs::write(&out, text)?;
            println!("{report}\nwritten: {}", out.display());
        }
        MobfarmCmd::SiteCheck { design, terrain, at, dug_in, cover, positions, rails, no_controls, afk, speeds } => {
            let (c, spec) = load(version, &design)?;
            let mut extra: Vec<redsim::pos::BlockPos> = Vec::new();
            for f in &positions {
                for line in std::fs::read_to_string(f)?.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                    let v: Vec<i32> = line.split([',', ' ']).filter(|s| !s.is_empty()).map(|n| n.parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "{}: a position takes x y z: {line}", f.display());
                    extra.push(redsim::pos::BlockPos::new(v[0], v[1], v[2]));
                }
            }
            let cover = match cover.as_str() {
                "b" | "B" => crate::sitecheck::Cover::B,
                "1" => crate::sitecheck::Cover::OneBlock,
                other => anyhow::bail!("--cover {other}: b (the lightning rule (b)'s depth) or 1 (one block)"),
            };
            let spot = match (at.as_deref(), dug_in) {
                (Some(t), _) => {
                    let v: Vec<i32> = t.split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 2, "--at takes X,Z (the footprint's min world column): {t}");
                    crate::sitecheck::Spot::At(v[0], v[1])
                }
                (None, true) => crate::sitecheck::Spot::DugIn(cover),
                (None, false) => crate::sitecheck::Spot::Judge,
            };
            anyhow::ensure!(terrain.is_some() || spot == crate::sitecheck::Spot::Judge, "--at and --dug-in place the farm in a terrain sample: give --terrain");
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
            let placed = crate::sitecheck::place(&c, &spec, terrain.as_deref().map(|t| (t, spot)), &extra, rails, afk, speeds.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
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
        MobfarmCmd::LiftBench { height, entry_len, entry_h, speed, carts, spacing, ticks, exit, trace, ceiling, freeze, exit_slope, merge3, split3, pane_walls, reach_dy, detector_at } => {
            let o = crate::liftbench::LiftOpts { height, entry_len, entry_h, speed, carts, spacing, ticks, exit, trace, ceiling, freeze: (freeze.len() == 2).then(|| (freeze[0], freeze[1])), exit_slope, merge3, split3, pane_walls, reach_dy, detector_at };
            println!("{}", crate::liftbench::lift_bench(&o).map_err(|e| anyhow::anyhow!(e))?);
        }
        MobfarmCmd::ClimbBench { cells, crest, h_open, h_for, h_every, pattern, route, folded, gravity, v3, crest6, v4, v5, v4r, plain_flats, plain_c, speed, speeds, gate_shut, carts, close_at, ticks, seed, seeds, riders, golem_box, freeze, out, trace } => {
            anyhow::ensure!(speeds.is_empty() || speeds.len() == 3, "--speeds A,B,C");
            let hand_speeds_v: Option<[f64; 3]> = (speeds.len() == 3).then(|| [speeds[0], speeds[1], speeds[2]]);
            anyhow::ensure!(crest >= 3, "--crest: 3 or more (today's snap turn's high end is at 3)");
            anyhow::ensure!(gate_shut.is_empty() || gate_shut.len() == 2, "--gate-shut A,B");
            // (7b90c3f1's meter timer fill, MCTAS_TIMER_FILL=T1,T2: G1 stays open T1 and G2 T2
            // live gt after the hold's release; the bench drives its gates the same way)
            let gate_shut: Vec<i64> = match (gate_shut.is_empty(), std::env::var("MCTAS_TIMER_FILL")) {
                (true, Ok(v)) => {
                    let t: Vec<i64> = v.split(',').map(|s| s.trim().parse::<i64>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(t.len() == 2, "MCTAS_TIMER_FILL=T1,T2");
                    println!("the stand's gates on MCTAS_TIMER_FILL={v}: G1 open {} gt and G2 {} gt (live) after the hold's release (bench-driven gates)", t[0], t[1]);
                    vec![t[1], t[0]]
                }
                _ => gate_shut,
            };
            anyhow::ensure!(carts == 2 || carts == 3, "--carts 2 or 3");
            let rd: Vec<bool> = riders.split(',').map(|s| s.trim() != "0").collect();
            anyhow::ensure!(rd.len() == 3, "--riders takes three 0/1");
            // (an entry "A:B": cart 1 handed off A gt after cart 0, cart 2 B gt after cart 1)
            let pairs: Vec<(i64, Option<i64>)> = h_every
                .split(',')
                .map(|s| -> anyhow::Result<(i64, Option<i64>)> {
                    let mut it = s.trim().split(':');
                    let a = it.next().unwrap_or("0").parse::<i64>()?;
                    let b = it.next().map(|x| x.parse::<i64>()).transpose()?;
                    Ok((a, b))
                })
                .collect::<anyhow::Result<_>>()?;
            let everys: Vec<i64> = pairs.iter().map(|p| p.0).collect();
            let spaced = everys.iter().any(|e| *e > 0) && !gravity;
            anyhow::ensure!(gravity || !spaced || everys.iter().all(|e| *e > 0), "--h-every: all spaced (> 0) or 0");
            anyhow::ensure!(pattern.is_empty() || pattern.len() == cells as usize, "--pattern: one letter per climb cell");
            anyhow::ensure!(route.is_empty() || route.len() == 2, "--route H_RAILS,L1");
            let text = if gravity && v4r {
                crate::climb::design_gravity_v4r([rd[0], rd[1], rd[2]])
            } else if gravity && v5 {
                crate::climb::design_gravity_v5([rd[0], rd[1], rd[2]])
            } else if gravity && v4 {
                crate::climb::design_gravity_v4([rd[0], rd[1], rd[2]], plain_flats)
            } else if gravity && crest6 {
                crate::climb::design_gravity_c6(cells, [rd[0], rd[1], rd[2]])
            } else if gravity && v3 {
                crate::climb::design_gravity_v3([rd[0], rd[1], rd[2]], plain_c)
            } else if gravity {
                crate::climb::design_gravity(cells, &pattern, [rd[0], rd[1], rd[2]])
            } else if folded {
                crate::climb::design_folded([rd[0], rd[1], rd[2]])
            } else if route.len() == 2 {
                crate::climb::design_route(route[0], route[1], [rd[0], rd[1], rd[2]]).map_err(|e| anyhow::anyhow!(e))?
            } else {
                crate::climb::design_with(cells, crest, &pattern, [rd[0], rd[1], rd[2]], spaced)
            };
            // (the hand-off point and velocity (the gravity descent's top, Ga's B, or the folded
            // route's Dh) and the freeze's chunk box)
            use redsim::entity::Vec3 as V3;
            let route_geo = if gravity && v4r {
                // (on G_a's slope, its centre: the surface at 15.5)
                Some((V3::new(13.5, 15.5625, 10.5), V3::new(0.0, 0.0, speed), [0, 0, 0, 0]))
            } else if gravity && v5 {
                Some((V3::new(13.5, 15.0625, 10.5), V3::new(0.0, 0.0, speed), [0, 0, 0, 0]))
            } else if gravity && v4 {
                Some((V3::new(13.5, 15.0625, 5.5), V3::new(0.0, 0.0, speed), [0, 0, 0, 0]))
            } else if gravity && crest6 {
                Some((V3::new(3.5, (6 + cells) as f64 + 0.0625, (11 + cells + 3) as f64 + 0.5), V3::new(0.0, 0.0, -speed), [0, 0, 0, (15 + cells).div_euclid(16)]))
            } else if gravity && v3 {
                Some((V3::new(13.5, 24.0625, 4.5), V3::new(0.0, 0.0, speed), [0, 0, 0, 0]))
            } else if gravity {
                Some((V3::new(3.5, (3 + cells) as f64 + 0.0625, (8 + cells) as f64 + 0.5), V3::new(0.0, 0.0, -speed), [0, 0, 0, (9 + cells).div_euclid(16)]))
            } else if folded {
                Some((V3::new(15.5, -17.0 + 0.0625, 16.5), V3::new(0.0, 0.0, 0.5), [0, 0, 0, 1]))
            } else {
                (route.len() == 2).then(|| {
                    let (h, l1) = (route[0], route[1]);
                    let l2 = 3 - h - l1;
                    let (lx, fz) = (4 + l2, 13 - l1 + 1);
                    (V3::new(lx as f64 + 0.5, h as f64 + 0.0625, (fz - 3) as f64 + 0.5), V3::new(0.0, 0.0, 0.5), [0, (fz - 5).div_euclid(16), lx.div_euclid(16), 0])
                })
            };
            let path = out.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("climb-bench-{}.txt", std::process::id())));
            std::fs::write(&path, &text)?;
            let (_c, fspec) = load(version, &path)?;
            anyhow::ensure!(golem_box.is_empty() || golem_box.len() == 6, "--golem-box x0,y0,z0,x1,y1,z1");
            anyhow::ensure!(freeze.is_empty() || freeze.len() == 2, "--freeze FROM,LEN");
            let opens: Vec<i64> = h_open.split(',').map(|s| s.trim().parse::<i64>()).collect::<Result<_, _>>()?;
            println!("climb bench: {} cells{}{}, the design {}", cells, if gravity { " (the gravity descent)" } else { "" }, if pattern.is_empty() { String::new() } else { format!(" ({pattern})") }, path.display());
            for (e, e2) in &pairs {
                for h in &opens {
                    for s in seed..seed + seeds {
                        let o = crate::climb::ClimbOpts {
                            cells,
                            crest: if gravity && (v3 || crest6 || v4 || v5 || v4r) { 6 } else if route.len() == 2 || folded || gravity { 3 } else { crest },
                            h_open: *h,
                            h_for,
                            h_every: if gravity { 0 } else { *e },
                            close_at,
                            ticks,
                            seed: s,
                            golem_box: (golem_box.len() == 6).then(|| [golem_box[0], golem_box[1], golem_box[2], golem_box[3], golem_box[4], golem_box[5]]),
                            freeze: (freeze.len() == 2).then(|| (freeze[0], freeze[1])),
                            hand_off: route_geo.map(|(p, v, c)| (p, v, if *e > 0 { *e } else if gravity { 3 } else if folded { 13 } else { 10 }, c)),
                            hand_off_2: *e2,
                            hand_speeds: hand_speeds_v,
                            gate_shut: (gate_shut.len() == 2).then(|| (gate_shut[0], gate_shut[1])),
                            n_carts: carts,
                            trace,
                        };
                        let lines = crate::climb::bench(&fspec, &o).map_err(|e| anyhow::anyhow!(e))?;
                        println!("== H open T+{h}, every {e}{} gt, seed {s}", e2.map_or(String::new(), |b| format!(":{b}")));
                        for l in lines {
                            println!("{l}");
                        }
                    }
                }
            }
        }
        MobfarmCmd::ClimbBack { rel, g1_delay, g2_delay, above, v4, carts, east, slopes_at_open, slope_delay, ticks, seed, seeds, riders, freeze, out, trace } => {
            let rd: Vec<bool> = riders.split(',').map(|s| s.trim() != "0").collect();
            anyhow::ensure!(rd.len() == 3, "--riders takes three 0/1");
            anyhow::ensure!(carts == 2 || carts == 3, "--carts 2 or 3");
            anyhow::ensure!(freeze.is_empty() || freeze.len() == 2, "--freeze FROM,LEN");
            let list = |s: &str| s.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<Vec<_>, _>>();
            let (rels, g1s, g2s) = (list(&rel)?, list(&g1_delay)?, list(&g2_delay)?);
            let east = east.unwrap_or(if v4 { 5 } else { 14 });
            anyhow::ensure!((1..=14).contains(&east), "--east 1..14");
            let past = if v4 { 13 } else { 0 };
            let text = crate::climb::design_climb_back([rd[0], rd[1], rd[2]], east, past);
            let path = out.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("climb-back-{}.txt", std::process::id())));
            std::fs::write(&path, &text)?;
            let (_c, fspec) = load(version, &path)?;
            println!("climb back: the design {}", path.display());
            for g1 in &g1s {
                for g2 in &g2s {
                    for r in &rels {
                        for s in seed..seed + seeds {
                            let o = crate::climb::ClimbBackOpts { rel: *r, g1_delay: *g1, g2_delay: *g2, slopes_at_open, slope_delay, ticks, seed: s, above, freeze: (freeze.len() == 2).then(|| (freeze[0], freeze[1])), top_y: 32 + east, past, n_carts: carts, trace };
                            let lines = crate::climb::climb_back_bench(&fspec, &o).map_err(|e| anyhow::anyhow!(e))?;
                            println!("== REL T+{r}, G1 +{g1}, G2 +{g2}, seed {s}");
                            for l in lines {
                                println!("{l}");
                            }
                        }
                    }
                }
            }
        }
        MobfarmCmd::StandCycle { rel, hold, g1_delay, g2_delay, slope_delay, h_every, speed, speeds, g0, fill, shut, backstop, qualified, dd, above, freezes, summary, overall_only, drop, freeze_sweep, dep_riders, arr_riders, east, ticks, seed, seeds, out, trace } => {
            // (per-cart hand-off speed sets; none: --speed for every cart)
            let speed_sets: Vec<Option<[f64; 3]>> = if speeds.trim().is_empty() {
                vec![None]
            } else {
                speeds
                    .split(';')
                    .map(|s| -> anyhow::Result<Option<[f64; 3]>> {
                        let v: Vec<f64> = s.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 3, "--speeds A,B,C;...");
                        Ok(Some([v[0], v[1], v[2]]))
                    })
                    .collect::<anyhow::Result<_>>()?
            };
            // (each cart: 1 ridden, 0 empty, x absent)
            let r3 = |s: &str| -> anyhow::Result<([bool; 3], [bool; 3])> {
                let v: Vec<&str> = s.split(',').map(|x| x.trim()).collect();
                anyhow::ensure!(v.len() == 3, "riders take three of 1, 0 or x");
                Ok(([v[0] == "1", v[1] == "1", v[2] == "1"], [v[0] == "x", v[1] == "x", v[2] == "x"]))
            };
            let deps: Vec<&str> = dep_riders.split(';').map(|s| s.trim()).collect();
            let arrs: Vec<&str> = arr_riders.split(';').map(|s| s.trim()).collect();
            let list = |s: &str| s.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<Vec<_>, _>>();
            let (rels, holds, g2s) = (list(&rel)?, list(&hold)?, list(&g2_delay)?);
            let pairs: Vec<(i64, i64)> = h_every
                .split(',')
                .map(|s| -> anyhow::Result<(i64, i64)> {
                    let mut it = s.trim().split(':');
                    let a = it.next().unwrap_or("0").parse::<i64>()?;
                    let b = it.next().map(|x| x.parse::<i64>()).transpose()?.unwrap_or(a);
                    Ok((a, b))
                })
                .collect::<anyhow::Result<_>>()?;
            let sh: Vec<Option<i64>> = shut.split(',').map(|x| if x.trim() == "-" { Ok(None) } else { x.trim().parse::<i64>().map(Some) }).collect::<Result<_, _>>()?;
            anyhow::ensure!(sh.len() == 3, "--shut G0,G1,G2");
            let keys: Vec<crate::climb::G0Key> = g0
                .split(',')
                .map(|s| -> anyhow::Result<crate::climb::G0Key> {
                    let s = s.trim();
                    if s == "-" {
                        return Ok(crate::climb::G0Key::Fixed(sh[0]));
                    }
                    let (k, v) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--g0 d1:K, dd:K, rel:S or -"))?;
                    let v = v.parse::<i64>()?;
                    Ok(match k {
                        "d1" => crate::climb::G0Key::D1(v),
                        "dd" => crate::climb::G0Key::Dd(v),
                        "ddcount" => crate::climb::G0Key::DdCount(v),
                        "ddretrig" => crate::climb::G0Key::DdRetrig(v),
                        "d2" => crate::climb::G0Key::D2Arm(v),
                        "rel" => crate::climb::G0Key::Fixed(Some(v)),
                        _ => anyhow::bail!("--g0 d1:K, dd:K, ddcount:K, ddretrig:K, d2:ARM, rel:S or -"),
                    })
                })
                .collect::<anyhow::Result<_>>()?;
            // (the fill: "A,B" both on D1's first rise after G0 shut; "d2:A,B" G1 on that D1 rise
            // + A, G2 on D2's second rise after the D2 arming + B; `-` fixed)
            let (fill_key, fill_d2): (Option<(i64, i64)>, bool) = if fill.trim() == "-" {
                (None, false)
            } else {
                let (d2f, rest) = match fill.trim().strip_prefix("d2:") {
                    Some(r) => (true, r),
                    None => (false, fill.trim()),
                };
                let v = list(rest)?;
                anyhow::ensure!(v.len() == 2, "--fill A,B, d2:A,B or -");
                (Some((v[0], v[1])), d2f)
            };
            let mut frz: Vec<Option<(i64, i64)>> = freezes
                .split(',')
                .map(|s| -> anyhow::Result<Option<(i64, i64)>> {
                    let s = s.trim();
                    if s == "-" {
                        return Ok(None);
                    }
                    let (a, b) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--freezes FROM:LEN,..."))?;
                    Ok(Some((a.parse()?, b.parse()?)))
                })
                .collect::<anyhow::Result<_>>()?;
            if !freeze_sweep.is_empty() {
                anyhow::ensure!(freeze_sweep.len() >= 4 && freeze_sweep[2] > 0, "--freeze-sweep FROM,TO,STEP,LEN[,LEN...]");
                frz.clear();
                for len in &freeze_sweep[3..] {
                    let mut a = freeze_sweep[0];
                    while a <= freeze_sweep[1] {
                        frz.push(Some((a, *len)));
                        a += freeze_sweep[2];
                    }
                }
            }
            anyhow::ensure!((1..=14).contains(&east), "--east 1..14");
            let past = 13;
            // (the vertical drop's sets "H,R,O;...": the hold's trapdoor y, the trapdoors at REL + R,
            // the pockets re-braked at REL + O; none: the descent and the gates)
            let drops: Vec<Option<(i32, i64, i64)>> = if drop.trim().is_empty() {
                vec![None]
            } else {
                drop.split(';')
                    .map(|s| -> anyhow::Result<Option<(i32, i64, i64)>> {
                        let v: Vec<&str> = s.split(',').map(|x| x.trim()).collect();
                        anyhow::ensure!(v.len() == 3, "--drop H,R,O;...");
                        Ok(Some((v[0].parse()?, v[1].parse()?, v[2].parse()?)))
                    })
                    .collect::<anyhow::Result<_>>()?
            };
            for dset in &drops {
            for leg in dd.split(',').map(|s| s.trim()) {
                let (dd_design, dd_pos) = match leg {
                    "N" => (Some('N'), None),
                    "A" => (Some('A'), Some(redsim::pos::BlockPos::new(15, 2, 12))),
                    "B" => (Some('B'), Some(redsim::pos::BlockPos::new(15, -3, 7))),
                    "legacy" => (None, None),
                    _ => anyhow::bail!("--dd N, A, B or legacy"),
                };
                for dep_s in &deps {
                    for arr_s in &arrs {
                        let ((dep, dep_x), (arr, arr_x)) = (r3(dep_s)?, r3(arr_s)?);
                        let absent = [dep_x[0], dep_x[1], dep_x[2], arr_x[0], arr_x[1], arr_x[2]];
                        let d2_design = fill_d2 || keys.iter().any(|k| matches!(k, crate::climb::G0Key::D2Arm(_)));
                        let text = crate::climb::design_stand_cycle_with(dep, arr, east, past, dd_design, d2_design, dset.map(|d| d.0));
                        let path = out.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("stand-cycle-{}.txt", std::process::id())));
                        std::fs::write(&path, &text)?;
                        let (_c, fspec) = load(version, &path)?;
                        if !summary {
                            println!("stand cycle: the design {} (departure leg {leg})", path.display());
                        }
                        for g2 in &g2s {
                            for key in &keys {
                                // (a detector key needs a design with that detector)
                                if matches!(key, crate::climb::G0Key::Dd(_) | crate::climb::G0Key::DdCount(_) | crate::climb::G0Key::DdRetrig(_)) && dd_pos.is_none() {
                                    continue;
                                }
                                let mut all_runs: Vec<crate::climb::CycleStats> = Vec::new();
                                for f in &frz {
                                    let mut runs: Vec<crate::climb::CycleStats> = Vec::new();
                                    for sp3 in &speed_sets {
                                    for (e, e2) in &pairs {
                                        for r in &rels {
                                            for h in &holds {
                                                for s in seed..seed + seeds {
                                                    let o = crate::climb::CycleOpts {
                                                        hand_speeds: *sp3,
                                                        rel: *r,
                                                        g1_delay,
                                                        g2_delay: *g2,
                                                        slope_delay,
                                                        hold: *h,
                                                        every: *e,
                                                        every2: *e2,
                                                        speed,
                                                        g0_key: *key,
                                                        backstop,
                                                        fill_key,
                                                        shut_fixed: [sh[0], sh[1], sh[2]],
                                                        qualified,
                                                        dd: dd_pos,
                                                        d2: d2_design,
                                                        d2_arm: if let crate::climb::G0Key::D2Arm(a) = key { *a } else { 40 },
                                                        fill_d2,
                                                        above,
                                                        absent,
                                                        drop: *dset,
                                                        ticks,
                                                        seed: s,
                                                        freeze: *f,
                                                        top_y: 32 + east,
                                                        past,
                                                        trace,
                                                    };
                                                    let (lines, stats) = crate::climb::stand_cycle_bench(&fspec, &o).map_err(|e| anyhow::anyhow!(e))?;
                                                    runs.push(stats);
                                                    if !summary {
                                                        println!("== G0 {key:?}, freeze {}, G2 +{g2}, REL T+{r}, hold T+{h}, hand-offs {e}:{e2}{}, departing {dep_s}, arriving {arr_s}, seed {s}", f.map_or("-".to_string(), |(a, b)| format!("T+{a}..T+{}", a + b)), sp3.map_or(String::new(), |v| format!(" at {:.2},{:.2},{:.2}", v[0], v[1], v[2])));
                                                        for l in lines {
                                                            println!("{l}");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    }
                                    if !overall_only {
                                        println!(
                                            "{}",
                                            crate::climb::cycle_summary(
                                                &format!("SUMMARY leg {leg}, G0 {key:?}, fill {fill_key:?}{}{}, G2 +{g2}, departing {dep_s}, arriving {arr_s}, freeze {}", if fill_d2 { " (G2 on D2)" } else { "" }, if qualified { ", qualified" } else { "" }, f.map_or("-".to_string(), |(a, b)| format!("T+{a}..T+{}", a + b))),
                                                &runs
                                            )
                                        );
                                    }
                                    all_runs.extend(runs);
                                }
                                // (every freeze of this key together)
                                println!(
                                    "{}",
                                    crate::climb::cycle_summary(
                                        &format!("OVERALL leg {leg}{}, G0 {key:?}, fill {fill_key:?}{}{}, G2 +{g2}, departing {dep_s}, arriving {arr_s}, {} freezes", dset.map_or(String::new(), |(h, r, f)| format!(" (drop: hold y {h}, trapdoors REL+{r}, re-brake REL+{f})")), if fill_d2 { " (G2 on D2)" } else { "" }, if qualified { ", qualified" } else { "" }, frz.len()),
                                        &all_runs
                                    )
                                );
                                all_runs.clear();
                            }
                        }
                    }
                }
            }
            }
        }
        MobfarmCmd::Storage { modules, lines, chests, days, seed, seeds, prefill, misset, open, boxed, frame, out, no_bench } => {
            let lines = lines.unwrap_or(if modules <= 2 { 1 } else { 2 });
            // 20 play-hours = 60 game days of iron blocks (ironspec 01:13)
            let chests = chests.unwrap_or_else(|| ((modules as f64 * crate::storage::GOLEMS_A_DAY * 4.0 * 60.0 / 9.0).ceil() / 64.0 / 54.0).ceil() as usize);
            let l = crate::storage::Layout { lines, chests, enclose: !open, boxed };
            let b = crate::storage::design(&l).map_err(|e| anyhow::anyhow!(e))?;
            if let Some(f) = frame {
                let v: Vec<i32> = f.split(',').map(|s| s.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--frame X,Y,Z");
                let (d, opener, walkway) = crate::storage::shifted(&b, [v[0], v[1], v[2]]);
                let ex = crate::storage::exposed(&d, &opener, &walkway);
                let path = out.unwrap_or_else(|| std::env::temp_dir().join(format!("storage-framed-{lines}-{chests}.txt")));
                std::fs::write(
                    &path,
                    format!(
                        "# The storage (mobfarm storage, {lines} line(s), {chests} double chest(s)) moved by {f} into the Nether frame, without the bench's kill-floor ring and catch floor; {} blocks: the door, its 2 buttons and their supports {}{}, the shell {} (cobblestone bottom slabs in every empty cell a blast from outside reaches, 26-connected through empty cells and blocks under the 3.283 breaking bar, beside a block under that bar or a walkway cell); blocks under the breaking bar and walkway cells a blast from outside reaches: {} (0 required; the door's 2 stone buttons, the opener, aside).\n{}",
                        d.blocks.len(),
                        b.enclosure,
                        if l.boxed { " (with --box's room)" } else { "" },
                        b.shield,
                        ex.len(),
                        d.to_text()
                    ),
                )?;
                println!("{}", path.display());
                println!("blocks under the 3.283 breaking bar and walkway cells a blast from outside reaches (26-connected through empty cells and blocks under 3.283): {} (0 required; the door's buttons aside); the door and supports {}{}, the shell {}, {} blocks", ex.len(), b.enclosure, if l.boxed { " (with --box's room)" } else { "" }, b.shield, d.blocks.len());
                for (p, s) in ex.iter().take(12) {
                    println!("  reached: {s} at {p:?}");
                }
                // (per region, the slab wrap against a box round it: ironspec 2026-11-12)
                let shell: std::collections::BTreeSet<[i32; 3]> = b.shell_at.iter().map(|p| [p[0] + v[0], p[1] + v[1], p[2] + v[2]]).collect();
                let mut pre = d.clone();
                pre.blocks.retain(|p, _| !shell.contains(p));
                for r in crate::storage::regions(&pre, &opener, &walkway) {
                    println!(
                        "  region of {} cell(s) in {:?}..{:?}: the slab wrap {} cell(s), a box round it {} cell(s){}: {}",
                        r.cells,
                        r.lo,
                        r.hi,
                        r.wrap,
                        r.boxed,
                        if r.box_blocked { " (crossing the walkway)" } else { "" },
                        if r.box_blocked || r.wrap <= r.boxed { "the wrap" } else { "the box" }
                    );
                }
                return Ok(());
            }
            let path = out.unwrap_or_else(|| std::env::temp_dir().join(format!("storage-{}-{lines}-{chests}.txt", std::process::id())));
            std::fs::write(&path, crate::storage::design_text(&l, &b))?;
            println!("{}", path.display());
            if !no_bench {
                let (_c, spec) = load(version, &path)?;
                for s in seed..seed + seeds.max(1) {
                    let o = crate::storage::BenchOpts { modules, days, seed: s, prefill, misset };
                    let t = crate::storage::bench(&spec, &b, &l, &o).map_err(|e| anyhow::anyhow!(e))?;
                    print!("{}", crate::storage::report(&l, &o, &t));
                }
            }
        }
        MobfarmCmd::StationJudge { hold_y, rel, stagger, rebrake, release, open_delay, pulse, refill, posts, no_slab_stop, scarers, l2, ticks, seed, seeds, edge, rest_bar, starts, ds, no_restarts, no_cold, launch_min, rebrakes, pulses, open_delays, refill_delays, refill_lens, staggers, jobs } => {
            anyhow::ensure!(starts.len() == 2, "--starts A,B");
            anyhow::ensure!(stagger.len() == 3, "--stagger A,B,C");
            anyhow::ensure!(refill.len() == 2, "--refill D,L");
            let posts_v: Vec<(i32, i32, i32)> = posts
                .iter()
                .map(|s| -> anyhow::Result<(i32, i32, i32)> {
                    let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--post x,y,z");
                    Ok((v[0], v[1], v[2]))
                })
                .collect::<anyhow::Result<_>>()?;
            let scarers_v: Vec<(char, redsim::entity::Vec3)> = scarers
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| -> anyhow::Result<(char, redsim::entity::Vec3)> {
                    let (k, v) = s.trim().split_once(':').ok_or_else(|| anyhow::anyhow!("--scarers z:x,y,z;p:x,y,z;P:x,y,z"))?;
                    let c: Vec<f64> = v.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(c.len() == 3 && (k == "z" || k == "p" || k == "P"), "--scarers z:x,y,z;p:x,y,z;P:x,y,z");
                    Ok((k.chars().next().unwrap(), redsim::entity::Vec3::new(c[0], c[1], c[2])))
                })
                .collect::<anyhow::Result<_>>()?;
            let lo = crate::drop::DropLayout::Station;
            let ex = crate::drop::Extras { blocks: Vec::new(), posts: posts_v, slab_stop: !no_slab_stop };
            let spec_of = |dep: [bool; 3]| -> anyhow::Result<redsim::entity::mob::farm::FarmSpec> {
                let text = crate::drop::design_extra(lo, hold_y, dep, [true; 3], true, true, &ex);
                let path = std::env::temp_dir().join(format!("station-judge-{}-{}.txt", std::process::id(), dep[0] as u8));
                std::fs::write(&path, &text)?;
                let (_c, fspec) = load(version, &path)?;
                let _ = std::fs::remove_file(&path);
                Ok(fspec)
            };
            let (spec, cold_spec) = (spec_of([true; 3])?, spec_of([false; 3])?);
            let base = crate::drop::DropOpts { layout: lo, hold_y, rel: rel[0], stagger: [stagger[0], stagger[1], stagger[2]], rebrake, release, pulse, occupancy: false, exit_gate: true, open_delay, l1: !l2, dep_absent: [false; 3], restarts: Vec::new(), release_world: false, rebrake_each: true, scarers: scarers_v, refill: Some((refill[0], refill[1])), ticks, seed, freeze: None, logins: Vec::new(), trace: false };
            let o = crate::stationjudge::StationJudgeOpts { rels: rel.clone(), seeds: (seed..seed + seeds.max(1)).collect(), starts: (starts[0], starts[1]), ds: ds.clone(), restarts: !no_restarts, cold: !no_cold, launch_min, edge, rest_bar, jobs };
            println!(
                "station judge (cc767b78, bench, tool unreviewed; the {} mode: per REL {rel:?} the control, {}the cold start{}): drop-bench --layout station --pad --hold-y {hold_y} --stagger {},{},{} --rebrake {rebrake} --rebrake-each --release {release} --exit-gate --open-delay {open_delay} --pulse {pulse} --refill {},{} --post {:?} {}--scarers {scarers}, {ticks} ticks; the launch bar {}",
                if l2 { "L2" } else { "L1" },
                if no_restarts { String::new() } else { format!("a save/stop/start at every phase T+{}..T+{} in each of the 10 orders (save, reverse, seed:1..8), ", starts[0], starts[1]) },
                if ds.is_empty() { String::new() } else { format!(", logins of {ds:?} gt at every phase (the pearl alternative)") },
                stagger[0],
                stagger[1],
                stagger[2],
                refill[0],
                refill[1],
                posts,
                if no_slab_stop { "" } else { "--slab-stop " },
                if launch_min > 0 { format!("{launch_min} gt") } else { "none (reported only)".to_string() }
            );
            let (lines, fails) = crate::stationjudge::judge(&spec, &cold_spec, &base, &o).map_err(|e| anyhow::anyhow!(e))?;
            for l in lines {
                println!("{l}");
            }
            println!("over the matrix: {fails} failing runs in the lead's rows (controls included; the pearl alternative's logins reported above); PENDING: the stand's summon bar (dfb89499's panic measure: >= 1 caller at every T over all 400 sensor-phase pairs, margin >= 2 at REL T+2..6), the rest held to --rest-bar {rest_bar} gt meanwhile");
            let mut rows: Vec<(String, crate::drop::DropOpts)> = Vec::new();
            for v in &rebrakes {
                rows.push((format!("the re-brake {v}"), crate::drop::DropOpts { rebrake: *v, ..base.clone() }));
            }
            for v in &pulses {
                rows.push((format!("the trapdoor pulse {v}"), crate::drop::DropOpts { pulse: *v, ..base.clone() }));
            }
            for v in &open_delays {
                rows.push((format!("the open delay {v}"), crate::drop::DropOpts { open_delay: *v, ..base.clone() }));
            }
            for v in &refill_delays {
                rows.push((format!("the R line's delay {v}"), crate::drop::DropOpts { refill: Some((*v, refill[1])), ..base.clone() }));
            }
            for v in &refill_lens {
                rows.push((format!("the R line's length {v}"), crate::drop::DropOpts { refill: Some((refill[0], *v)), ..base.clone() }));
            }
            for s in &staggers {
                let v: Vec<i64> = s.split(':').map(|x| x.trim().parse::<i64>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--staggers A:B:C,...");
                rows.push((format!("the stagger {s}"), crate::drop::DropOpts { stagger: [v[0], v[1], v[2]], ..base.clone() }));
            }
            for (label, b) in rows {
                let (lines, fails) = crate::stationjudge::judge(&spec, &cold_spec, &b, &o).map_err(|e| anyhow::anyhow!(e))?;
                println!("threshold row, {label}: {fails} failing runs");
                for l in lines {
                    println!("  {l}");
                }
            }
        }
        MobfarmCmd::DropBench { layout, login, hold_y, rel, stagger, rebrake, release, pulse, occupancy, exit_gate, release_world, pad, rebrake_each, scarers, block, posts, slab_stop, open_delay, l1, restart, restart_sweep, write_design, refill, dep_riders, arr_riders, freezes, freeze_sweep, ticks, seed, seeds, summary, out, trace } => {
            let posts_v: Vec<(i32, i32, i32)> = posts
                .iter()
                .map(|s| -> anyhow::Result<(i32, i32, i32)> {
                    let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--post x,y,z");
                    Ok((v[0], v[1], v[2]))
                })
                .collect::<anyhow::Result<_>>()?;
            // (extra blocks "x,y,z,STATE": the state may hold commas)
            let blocks_v: Vec<(i32, i32, i32, String)> = block
                .iter()
                .map(|s| -> anyhow::Result<(i32, i32, i32, String)> {
                    let mut it = s.splitn(4, ',');
                    let (x, y, z, st) = (it.next(), it.next(), it.next(), it.next());
                    let (Some(x), Some(y), Some(z), Some(st)) = (x, y, z, st) else { anyhow::bail!("--block x,y,z,STATE") };
                    Ok((x.trim().parse()?, y.trim().parse()?, z.trim().parse()?, st.trim().to_string()))
                })
                .collect::<anyhow::Result<_>>()?;
            let logins_v: Vec<(i64, i64)> = login
                .iter()
                .map(|s| -> anyhow::Result<(i64, i64)> {
                    let (a, b) = s.split_once(',').ok_or_else(|| anyhow::anyhow!("--login T,D"))?;
                    Ok((a.trim().parse()?, b.trim().parse()?))
                })
                .collect::<anyhow::Result<_>>()?;
            let refill_v: Option<(i64, i64)> = if refill.trim().is_empty() {
                None
            } else {
                let v: Vec<i64> = refill.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 2 && pulse > 0, "--refill D,L (with --pulse)");
                Some((v[0], v[1]))
            };
            // (scarers "z:x,y,z;p:x,y,z": a zombie's or a pillager's feet)
            let scarers_v: Vec<(char, redsim::entity::Vec3)> = scarers
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| -> anyhow::Result<(char, redsim::entity::Vec3)> {
                    let (k, v) = s.trim().split_once(':').ok_or_else(|| anyhow::anyhow!("--scarers z:x,y,z;p:x,y,z"))?;
                    let c: Vec<f64> = v.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(c.len() == 3 && (k == "z" || k == "p" || k == "P"), "--scarers z:x,y,z;p:x,y,z;P:x,y,z");
                    Ok((k.chars().next().unwrap(), redsim::entity::Vec3::new(c[0], c[1], c[2])))
                })
                .collect::<anyhow::Result<_>>()?;
            let list = |s: &str| s.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<Vec<_>, _>>();
            let r3 = |s: &str| -> anyhow::Result<([bool; 3], [bool; 3])> {
                let v: Vec<&str> = s.split(',').map(|x| x.trim()).collect();
                anyhow::ensure!(v.len() == 3, "riders take three of 1, 0 or x (absent)");
                Ok(([v[0] == "1", v[1] == "1", v[2] == "1"], [v[0] == "x", v[1] == "x", v[2] == "x"]))
            };
            let ((dep, dep_x), (arr, _)) = (r3(&dep_riders)?, r3(&arr_riders)?);
            // (restart rows: --restart in every run, or one run per tick and order of --restart-sweep;
            // the cold start is the fresh design's --dep-riders row, G1: `parse_restart` refuses
            // "cold", the error points there)
            let fixed: Vec<(i64, crate::liveness::Restart)> = restart
                .iter()
                .map(|s| {
                    crate::liveness::parse_restart(s).map_err(|e| {
                        if s.trim_end().ends_with("cold") {
                            anyhow::anyhow!("{e}: the station's cold start is the fresh design's row (--dep-riders x,x,x; G1)")
                        } else {
                            anyhow::anyhow!(e)
                        }
                    })
                })
                .collect::<anyhow::Result<_>>()?;
            let mut reload_sets: Vec<Vec<(i64, crate::liveness::Restart)>> = vec![fixed];
            if !restart_sweep.is_empty() {
                anyhow::ensure!(restart_sweep.len() == 3 && restart_sweep[2] > 0, "--restart-sweep FROM,TO,STEP");
                let orders: Vec<crate::liveness::Restart> = crate::liveness::Restart::rows();
                reload_sets = (restart_sweep[0]..=restart_sweep[1]).step_by(restart_sweep[2] as usize).flat_map(|t| orders.iter().map(move |h| vec![(t, *h)])).collect();
            }
            let st = list(&stagger)?;
            anyhow::ensure!(st.len() == 3, "--stagger A,B,C");
            let (rels, rebrakes, releases, holds) = (list(&rel)?, list(&rebrake)?, list(&release)?, list(&hold_y)?);
            let mut frz: Vec<Option<(i64, i64)>> = freezes
                .split(',')
                .map(|s| -> anyhow::Result<Option<(i64, i64)>> {
                    let s = s.trim();
                    if s == "-" {
                        return Ok(None);
                    }
                    let (a, b) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--freezes FROM:LEN,..."))?;
                    Ok(Some((a.parse()?, b.parse()?)))
                })
                .collect::<anyhow::Result<_>>()?;
            if !freeze_sweep.is_empty() {
                anyhow::ensure!(freeze_sweep.len() >= 4 && freeze_sweep[2] > 0, "--freeze-sweep FROM,TO,STEP,LEN[,LEN...]");
                frz.clear();
                for len in &freeze_sweep[3..] {
                    let mut a = freeze_sweep[0];
                    while a <= freeze_sweep[1] {
                        frz.push(Some((a, *len)));
                        a += freeze_sweep[2];
                    }
                }
            }
            // (G1: a restart's unload and reload reset a freeze's entity box, so a run with both is
            // undefined: refused)
            anyhow::ensure!(
                reload_sets.iter().all(|s| s.is_empty()) || (frz.iter().all(|f| f.is_none()) && logins_v.is_empty()),
                "--restart / --restart-sweep with --freezes / --freeze-sweep / --login is undefined (the restart's unload resets the freeze's entity box): run them apart"
            );
            for lay in layout.split(';').map(|s| s.trim()) {
                let lo = match lay.split_once(':') {
                    Some(("parallel", p)) => crate::drop::DropLayout::Parallel(p.parse()?),
                    None if lay == "series" => crate::drop::DropLayout::Series,
                    None if lay == "station" => crate::drop::DropLayout::Station,
                    _ => anyhow::bail!("--layout series, parallel:P or station"),
                };
                for h in &holds {
                    let ex = crate::drop::Extras { blocks: blocks_v.clone(), posts: posts_v.clone(), slab_stop };
                    let text = crate::drop::design_extra(lo, *h as i32, dep, arr, pad, refill_v.is_some(), &ex);
                    // (onechunk's module: the station as a committed design in the loop frame)
                    if let Some(wd) = &write_design {
                        anyhow::ensure!(lo == crate::drop::DropLayout::Station, "--write-design: the station layout only");
                        let header: Vec<String> = vec![
                            "The F1 + Q2 station (dfb89499), in the loop frame: written by `mctas mobfarm drop-bench --layout station ... --write-design` (the bench's own file sits at loop + (6,0,6)); NOTES: designs/farms/iron/carousel-assembly/wip/station/NOTES.md.".into(),
                            format!(
                                "The lead: --l1 --open-delay {open_delay}; the build: {}{}; the trapdoor pulse {pulse}; the re-brake {}; offsets {} (lanes x -1/3/7).",
                                if slab_stop { "--slab-stop (each hold's north stop a bottom slab)" } else { "glass north stops" },
                                if posts_v.is_empty() { ", no zombie post (9fc3155f's summoner-only scarers)".to_string() } else { format!(", zombie posts at loop {}", posts_v.iter().map(|p| format!("({},{},{})", p.0 - 6, p.1, p.2 - 6)).collect::<Vec<_>>().join(" ")) },
                                rebrake,
                                stagger
                            ),
                            "Cells: pockets P0..2 at (x,0,0), x -1/3/7, north_south powered rails on switched supports (x,-1,0) (stone; a redstone block while powered); the launcher stone (x,0,-1) under an unattached string (x,1,-1); the pocket strings (x,1,0); departure (x,0,1) plain, the exit detector D_k (x,0,2), (x,0,3) plain, boosters (x,0,4..6): the merge row at z 7.".into(),
                            format!("Holds H0..2 at (x,{0},0): bottom-half trapdoors (open: the drop), the ring's glass, the north stop a bottom slab (x,{1},-1), the hold string (x,{1},0); R_1 (x,{1},1) a powered rail ascending south, side-powered at (x+1,{1},1) (glass; a redstone block while powered). The pad: portals x -2..7 rows z -5 and -2, golem feet -6 (PAD0..PAD1).", *h, *h + 1),
                            "Events (L1, game ticks, every drive EMULATED until the circuit is laid; T the summon tick): 1. pocket k's support powered at REL + offset k, unpowered 16 gt later (the re-brake). 2. pocket k's gate: D_k rises after REL + offset k, or pocket k's string is off then (the cold start); trapdoor k opens 4 gt after, not before T+3. 3. trapdoor k shuts pulse gt after it opens. 4. all three shut: + 2 gt, every R_1 side-powered for 10 gt (the next batch rolls into the holds).".into(),
                            "Carts: the batch in the pockets and the next in the holds (riders), the hold carts where the lane leaves them (0.49 into the cell, against the ring's north glass: tickexact's lane-handoff ca3cdb2ed); the lanes' carts (R_1 on) and R_2 are the lanes' file's. The pad's seal: cobblestone at y -8 under the floor rows, x -2..7 (40 blocks).".into(),
                            "The Nether part is the bench's PLACEHOLDER, not the build: the shared kill room's portal at the pads' y replaces it.".into(),
                            "Read path: the hold strings' hook supports (x+-2,17,0) are glowstone (golem-proof, lit, a solid-render stroll destination, a sturdy hook face); glowstone conducts no power, so each hold string is read from its hooks directly (dust or a repeater beside the hook), never through the support.".into(),
                            "R_1's power cell (x+1,17,1) sits face-adjacent (south) to the hold string's hook (x+1,17,0): the built driver is a repeater facing west into R_1, its input from the east (x+2,17,1), which no hook can drive (never dust, nor a repeater or comparator reading from the north); this file holds glass there, the bench's emulation (a redstone block while the R line is on).".into(),
                        ];
                        let loop_text = crate::drop::station_loop_text(&text, *h as i32, &header).map_err(|e| anyhow::anyhow!(e))?;
                        std::fs::write(wd, &loop_text)?;
                        println!("the station (loop frame) written to {}", wd.display());
                        if let Some(l) = loop_text.lines().find(|l| l.starts_with("# Placement order")) {
                            println!("{}", l.trim_start_matches("# "));
                        }
                    }
                    let path = out.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("drop-bench-{}.txt", std::process::id())));
                    std::fs::write(&path, &text)?;
                    let (_c, fspec) = load(version, &path)?;
                    println!("drop bench {lo:?}, hold y {h}: {}", crate::drop::footprint(lo, *h as i32));
                    let mut runs: Vec<crate::drop::DropStats> = Vec::new();
                    let mut by_rel: std::collections::BTreeMap<i64, Vec<crate::drop::DropStats>> = std::collections::BTreeMap::new();
                    for f in &frz {
                    for rset in &reload_sets {
                        for r in &rels {
                            for rb in &rebrakes {
                                for rl in &releases {
                                    for s in seed..seed + seeds {
                                        let o = crate::drop::DropOpts { layout: lo, hold_y: *h as i32, rel: *r, stagger: [st[0], st[1], st[2]], rebrake: *rb, release: *rl, pulse, occupancy, exit_gate, open_delay, l1, dep_absent: dep_x, restarts: rset.clone(), release_world, rebrake_each, scarers: scarers_v.clone(), refill: refill_v, ticks, seed: s, freeze: *f, logins: logins_v.clone(), trace };
                                        let (lines, stats) = crate::drop::bench(&fspec, &o).map_err(|e| anyhow::anyhow!(e))?;
                                        by_rel.entry(*r).or_default().push(stats.clone());
                                        runs.push(stats);
                                        if !summary {
                                            println!("== freeze {}{}, REL T+{r}, re-brake REL+{rb}, trapdoors T+{rl}, seed {s}", f.map_or("-".to_string(), |(a, b)| format!("T+{a}..T+{}", a + b)), if rset.is_empty() { String::new() } else { format!(", restart {}", rset.iter().map(|(t, h)| format!("T+{t} {}", h.label())).collect::<Vec<_>>().join(",")) });
                                            for l in lines {
                                                println!("{l}");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    }
                    println!("{}", crate::drop::summary(&format!("SUMMARY {lo:?}, hold y {h}, REL {rel}, re-brake {rebrake}, trapdoors {release}{}{}, {} freezes, {} restart rows", if occupancy { ", gated on the strings" } else { "" }, if l1 { ", L1" } else { ", L2" }, frz.len(), reload_sets.len()), &runs));
                    // (farmopt 03:25: the rest margin and the landing lead at each REL)
                    if by_rel.len() > 1 {
                        for (r, rs) in &by_rel {
                            let fails: usize = rs.iter().map(|s| s.wrong + s.contacts + s.hurt + s.stuck + s.shut_onto).sum();
                            println!(
                                "  REL T+{r}: {} runs; at rest least {} gt before the next summon, the landing lead least {} gt; failures {fails}",
                                rs.len(),
                                rs.iter().map(|s| s.rest).min().unwrap_or(i64::MIN),
                                rs.iter().filter_map(|s| s.brake_lead).min().map_or("-".to_string(), |v| v.to_string())
                            );
                        }
                    }
                }
            }
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
        MobfarmCmd::EscapeBench { lanes, spacing, run, per_lane, period, cycles, ret, corner, w0, wp, drop_w, entry, hold_n, front_boost, pockets, string_dy, r_pulse, retry, login, login_matrix, intake_flat, v_in, r_boost, floor_bottom, empty, freeze32 } => {
            let pair = |s: &String| -> anyhow::Result<(i64, i64)> {
                let mut it = s.split(',');
                let a = it.next().unwrap_or("").trim().parse::<i64>()?;
                let d = it.next().unwrap_or("").trim().parse::<i64>()?;
                Ok((a, d))
            };
            let login: Vec<(i64, i64)> = login.iter().map(pair).collect::<anyhow::Result<_>>()?;
            let freeze32: Vec<(i64, i64)> = freeze32.iter().map(pair).collect::<anyhow::Result<_>>()?;
            let o = crate::dropbench::DropOpts { lanes, spacing, run, per_lane, period, cycles, ret, corner, w0, wp, drop_w, entry, hold_n, front_boost, pockets, string_dy, r_pulse, retry, login, intake_flat, v_in, r_boost, floor_bottom, riders: !empty, freeze32 };
            if login_matrix {
                // (a login of 1..=3 gt landing at every phase of the third cycle)
                let base = 300 + 2 * period;
                let mut bad: Vec<String> = Vec::new();
                let mut rows = 0usize;
                for d in 1..=3i64 {
                    for p in 0..period {
                        let mut oo = o.clone();
                        oo.cycles = cycles.max(5);
                        oo.login = vec![(base + p, d)];
                        let r = crate::switchbench::switch_bench(&oo).map_err(|e| anyhow::anyhow!(e))?;
                        rows += 1;
                        let ok = r.per_release.keys().all(|k| *k == 1) && r.shuts == 0 && r.hanging == 0 && r.unsettled == 0;
                        if !ok {
                            bad.push(format!("login {d} gt at phase {p}: per release {:?}, shuts {}, unsettled {}, hanging {}, retries {}", r.per_release, r.shuts, r.unsettled, r.hanging, r.retries));
                        }
                    }
                }
                println!("login matrix: {rows} rows (1..3 gt at every phase of a {period}-gt cycle), {} failing{}", bad.len(), if bad.is_empty() { String::new() } else { format!(":\n{}", bad.iter().take(20).cloned().collect::<Vec<_>>().join("\n")) });
                return Ok(());
            }
            println!("{}", crate::dropbench::drop_bench(&o).map_err(|e| anyhow::anyhow!(e))?);
            return Ok(());
        }
        MobfarmCmd::LanesBench { lanes, spacing, run, per_lane, period, width, cycles, ret, corner, ring_q, freeze, freeze32, empty } => {
            let pair = |s: &String| -> anyhow::Result<(i64, i64)> {
                let mut it = s.split(',');
                let a = it.next().unwrap_or("").trim().parse::<i64>()?;
                let d = it.next().unwrap_or("").trim().parse::<i64>()?;
                Ok((a, d))
            };
            let freeze: Vec<(i64, i64)> = freeze.iter().map(pair).collect::<anyhow::Result<_>>()?;
            let freeze32: Vec<(i64, i64)> = freeze32.iter().map(pair).collect::<anyhow::Result<_>>()?;
            let o = crate::lanesbench::LanesOpts { lanes, spacing, run, per_lane, period, width, cycles, ret, corner, ring_q, freeze, freeze32, riders: !empty };
            println!("{}", crate::lanesbench::lanes_bench(&o).map_err(|e| anyhow::anyhow!(e))?);
            return Ok(());
        }
        MobfarmCmd::LanesJudge { lanes, spacing, run, per_lane, period, cycles, ret, modes, starts, k, ds, corners, jobs, empty } => {
            anyhow::ensure!(starts.len() == 2, "--starts A,B");
            let modes = modes.iter().map(|m| crate::lanesjudge::Mode::parse(m)).collect::<Result<Vec<_>, _>>().map_err(|e| anyhow::anyhow!(e))?;
            let base = crate::lanesbench::LanesOpts { lanes, spacing, run, per_lane, period, width: 0, cycles, ret, corner: 0, ring_q: 0, freeze: Vec::new(), freeze32: Vec::new(), riders: !empty };
            let o = crate::lanesjudge::JudgeOpts { modes, starts: (starts[0], starts[1]), k, ds, corners, jobs };
            let title = format!("lanes ({lanes} lanes, {run}-slope runs, {per_lane} carts a lane, period {period}, {cycles} cycles)");
            // (lanesbench's warm-up: 100 gt)
            let lines = crate::lanesjudge::judge(&title, 100, period, crate::lanesjudge::LANES_FAMILIES, &o, |m, c, fr| {
                let mut lo = base.clone();
                lo.ring_q = m.ring_q;
                lo.width = m.width;
                lo.corner = c;
                lo.freeze32 = fr;
                crate::lanesbench::lanes_bench(&lo)
            })
            .map_err(|e| anyhow::anyhow!(e))?;
            for l in lines {
                println!("{l}");
            }
            return Ok(());
        }
        MobfarmCmd::EscapeJudge { lanes, spacing, run, per_lane, period, cycles, ret, w0, wp, drop_w, entry, hold_n, front_boost, r_boost, floor_bottom, empty, r_pulse, retry, intake_flat, v_in, starts, k, ds, corners, jobs } => {
            anyhow::ensure!(starts.len() == 2, "--starts A,B");
            let base = crate::dropbench::DropOpts { lanes, spacing, run, per_lane, period, cycles, ret, corner: 0, w0, wp, drop_w, entry, hold_n, front_boost, pockets: 0, string_dy: 2, r_pulse, retry, login: Vec::new(), intake_flat, v_in, r_boost, floor_bottom, riders: !empty, freeze32: Vec::new() };
            let modes = vec![crate::lanesjudge::Mode::parse("gt").map_err(|e| anyhow::anyhow!(e))?];
            let o = crate::lanesjudge::JudgeOpts { modes, starts: (starts[0], starts[1]), k, ds, corners, jobs };
            let title = if r_pulse > 0 {
                format!("escape, the switched-R lane ({lanes} lanes, {run}-slope runs, {per_lane} carts a lane, the R line {r_pulse} gt, the trapdoors {drop_w} gt every {period}, hold {hold_n}, {cycles} cycles{})", if intake_flat { format!(", the flat intake at {v_in:.2} b/t") } else { String::new() })
            } else {
                format!("escape ({lanes} lanes, {run}-slope runs, {per_lane} carts a lane, G0 {w0} gt, G_P {wp} gt, the trapdoors {drop_w} gt every {period}, hold {hold_n}, {cycles} cycles)")
            };
            // (the escape bench's warm-up: 300 gt; the families its lane prints)
            let expected: &[&str] = if r_pulse > 0 { crate::lanesjudge::ESCAPE_SWITCHED_FAMILIES } else { crate::lanesjudge::ESCAPE_GATES_FAMILIES };
            let lines = crate::lanesjudge::judge(&title, 300, period, expected, &o, |_, c, fr| {
                let mut lo = base.clone();
                lo.corner = c;
                lo.freeze32 = fr;
                crate::dropbench::drop_bench(&lo)
            })
            .map_err(|e| anyhow::anyhow!(e))?;
            for l in lines {
                println!("{l}");
            }
            return Ok(());
        }
        MobfarmCmd::StringBench { descent, v0, pass, hold, ticks, empty } => {
            println!("{}", crate::stringbench::string_bench(descent, v0, pass, hold, ticks, !empty).map_err(|e| anyhow::anyhow!(e))?);
            return Ok(());
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
        MobfarmCmd::NapperPicker { carts, visits, picker, k, offset, every, keep_place, gate_m, gate_lap, gate_batches, virtual_count, gate_lap_off, gate_virtual, free_at_reentry, rearm_jitter, golemless, count_golems, freeze, skip_night, skip_alarm, pass_fault, register_carry, spacing, slow_visit, trip_bound, gate_carts, empty_cart, register_after, empty_trip, skip_per_day, events, watchdog, pad_roll, arrive, events_from, recover_k, lock, lock_gt, remark, idle, lanes, beds, lane_exact, turns, outlier, post, foot, r_leg, window, det, lie, strand_search, max_away, jump, jump_every, absence, watchdog_table, trip, sleep_at, days, warmup, seed, night_gt, cross_at, dusk_at, start_dt, rephase_lag, truncate_after_skip, thunder_sweep, sweep_step, hold_at, siding_window, ret, table, sweep_k } => {
            let absence = match absence.as_slice() {
                [] => None,
                [d, dt, len] => Some((*d as usize, *dt, *len)),
                _ => bail!("--absence D,DT,LEN"),
            };
            let jump = match jump.as_slice() {
                [] => None,
                [d, at] => Some((*d as usize, *at)),
                _ => bail!("--jump D,IN_DAY"),
            };
            let idle = match idle.as_slice() {
                [] => None,
                [l, i] => Some((*l, *i)),
                _ => bail!("--idle L,I"),
            };
            if lanes > 0 {
                let [lo, hi] = trip[..] else { bail!("--trip takes LO,HI") };
                let outlier = match outlier.as_slice() {
                    [] => None,
                    [e, a] => Some((*e, *a)),
                    _ => bail!("--outlier EVERY,ADD"),
                };
                let window = match window.as_slice() {
                    [a, b] => (*a, *b),
                    _ => bail!("--window LO,HI"),
                };
                let hold_at = match hold_at.as_slice() {
                    [] => None,
                    [d, at, add] => Some((*d as usize, *at, *add)),
                    _ => bail!("--hold-at D,IN_DAY,ADD"),
                };
                let lo_ = crate::napper::LaneOpts { lanes, per_lane: carts / lanes, k, beds, turns, visits, lane_exact, keep_place, skip_per_day, lock, trip: (lo, hi), sleep_at, spacing, cross_at, days, warmup, seed, night_gt, day_gt: 24000, dusk_at, jump, jump_every, outlier, post, foot, r_leg, window, det, lie, start_dt, rephase_lag, truncate_after_skip, hold_at, siding_window, ret, lock_gt, remark };
                if thunder_sweep {
                    // (the counter: daylight inhibit L 7 on the linear level model; the arm: a falling
                    // 8-crossing arms trigger 2 for 300 gt)
                    let co = crate::ctlcounter::CounterOpts { day_inhibit: Some(7), ..Default::default() };
                    let ca = crate::ctlcounter::CounterOpts { arm: Some(300), ..co.clone() };
                    let cd = crate::ctlcounter::CounterOpts { arm_drop2: true, ..co.clone() };
                    // (picks in the first n releases of a window: lanes at offsets 0/3/6 of 10)
                    let picks_in = |n: i64| 3 * (n / 10) + [0i64, 3, 6].iter().filter(|o| **o < n % 10).count() as i64;
                    let n_for = |naps: u32| -> Option<i64> {
                        if naps >= 24 {
                            return None;
                        }
                        (0..=80).find(|n| picks_in(*n) >= naps as i64)
                    };
                    let base = crate::napper::simulate_lanes_scored(&lo_).1;
                    println!(
                        "thunder sweep (ironspec Nov 12 02:10): the lane model as above (baseline unskipped: {:.1} a day, {} golems lost to expiry, the largest LAST_SLEPT age {} gt); per skip day time d: naps that night / the night after (the counter, min..max over 40 phases; for a skip at night, that night is the one it cuts), the largest LAST_SLEPT age, golems lost to expiry against the baseline; as is | the falling-8 arm (300 gt) | the 2-level-drop latch",
                        base.golems, base.lost_expiry, base.max_age
                    );
                    let mut rows: Vec<(String, i64)> = Vec::new();
                    for d in (0..=12_000).step_by(sweep_step.max(1) as usize) {
                        rows.push((format!("{d}"), d));
                    }
                    rows.push(("rain-dusk bed-time 12,102".into(), 12_102));
                    rows.push(("bed-time 12,623".into(), 12_623));
                    let mut worst = [0f64; 3];
                    for (label, d) in rows {
                        let jump_at = if d < 12_000 { (9usize, d + 24_000 - start_dt) } else { (10usize, d - start_dt) };
                        let mut cells = Vec::new();
                        for (vi, copts) in [&co, &ca, &cd].into_iter().enumerate() {
                            let (that, after, daylight) = crate::ctlcounter::skip_naps(copts, d);
                            // (the lane model cuts the window after the jump: a skip by day lands
                            // before "that night"; a skip at night cuts its own night, the model's
                            // next window is "the night after")
                            let cut = if d < 12_000 { that.0 } else { after.0 };
                            let o2 = crate::napper::LaneOpts { jump: Some(jump_at), truncate_after_skip: n_for(cut), ..lo_.clone() };
                            let st = crate::napper::simulate_lanes_scored(&o2).1;
                            let lost = st.lost_expiry - base.lost_expiry;
                            worst[vi] = worst[vi].max(lost);
                            cells.push(format!(
                                "naps {}..{} / {}..{}, LAST_SLEPT max {}, lost {:.0}{}",
                                that.0, that.1, after.0, after.1, st.max_age, lost,
                                if daylight > 0 { format!(", daylight picks {daylight}") } else { String::new() }
                            ));
                        }
                        println!("  d {label}: {} | {} | {}", cells[0], cells[1], cells[2]);
                    }
                    println!("  => the worst skip: as is {:.0} golems lost, the falling-8 arm {:.0}, the 2-level-drop latch {:.0}", worst[0], worst[1], worst[2]);
                    return Ok(());
                }
                println!("{}", crate::napper::simulate_lanes(&lo_));
                if strand_search {
                    // (the strand margin: the largest +ADD, one strand a night, that loses no golem)
                    let mut margin: Option<i64> = None;
                    let mut first_loss: Option<(i64, f64)> = None;
                    for add in (0..=600).step_by(10) {
                        let o2 = crate::napper::LaneOpts { outlier: Some((24000, add)), ..lo_.clone() };
                        let (_, st) = crate::napper::simulate_lanes_scored(&o2);
                        if st.golems >= 239.95 && first_loss.is_none() {
                            margin = Some(add);
                        } else if first_loss.is_none() {
                            first_loss = Some((add, st.golems));
                        }
                    }
                    println!(
                        "  the strand margin (one strand a night): {} (the first loss at +{})",
                        margin.map_or("none (losses with no strand)".to_string(), |m| format!("+{m} gt loses nothing")),
                        first_loss.map_or("-, none up to +600".to_string(), |(a, g)| format!("{a}: {g:.1} a day"))
                    );
                }
                return Ok(());
            }
            let empty_trip = match empty_trip.as_slice() {
                [] => None,
                [a, b] => Some((*a, *b)),
                _ => bail!("--empty-trip LO,HI"),
            };
            let slow_visit = match slow_visit.as_slice() {
                [] => None,
                [e, t] => Some((*e as usize, *t)),
                _ => bail!("--slow-visit EVERY,T"),
            };
            let [jlo, jhi] = rearm_jitter[..] else { bail!("--rearm-jitter takes LO,HI") };
            let freeze = match freeze[..] { [d, l] => Some((d as usize, l)), [] => None, _ => bail!("--freeze takes D,LEN") };
            let pass_fault = match pass_fault[..] { [d, f] => Some((d as usize, f)), [] => None, _ => bail!("--pass-fault takes D,F") };
            use crate::napper::{Opts, Picker};
            let [lo, hi] = trip[..] else { bail!("--trip takes LO,HI") };
            let pk = match picker.as_str() {
                "from-pick" => Picker::FromPick(k),
                "from-return" => Picker::FromReturn(k),
                "anchored" => Picker::Anchored(k, offset),
                "register" => Picker::Register(every),
                s => bail!("--picker {s}: from-pick, from-return, anchored or register"),
            };
            let base = Opts { carts, visits, picker: pk, keep_place, gate_m, gate_lap, gate_batches, virtual_count, gate_lap_off, gate_virtual, free_at_reentry, rearm_jitter: (jlo, jhi), golemless, count_golems, freeze, skip_night, skip_alarm, pass_fault, register_carry, slow_visit, gate_carts, empty_cart, register_after, empty_trip, skip_per_day, events: events || watchdog.is_some(), watchdog, pad_roll, arrive, events_from, recover_k, lock, lock_gt, remark, idle, beds, max_away, jump, jump_every, absence, trip: (lo, hi), sleep_at, days, warmup, seed, night_gt, day_gt: 24000, period: 100, batch: 3, spacing, cross_at, dusk_at };
            if !watchdog_table.is_empty() {
                // (ironspec Nov 11 20:38: W > 200 so a failed roll's batch retries at the next tick;
                // the rows per W, scored from day `warmup`: a warm start (healthy fixed beats before
                // `warmup`, event-driven after) at P 1 (the model's control: it must match the fixed
                // beats) and at P 0.99; the first start with never-slept riders (event-driven from
                // day 0); 30,000-gt absences on day 10 returning on day 11 mid-day (in_day 16,000) and
                // on day 12 mid-night (in_day 4,300); each with and without the recovery picker k' 5)
                let ev = Opts { events: true, pad_roll: 0.99, absence: None, events_from: warmup, recover_k: None, ..base.clone() };
                let mut ws: Vec<Option<i64>> = watchdog_table.iter().map(|w| Some(*w)).collect();
                ws.push(None);
                for w in ws {
                    let wl = w.map_or("no watchdog (the lock-up's control)".to_string(), |w| format!("W {w}"));
                    println!("{wl}:");
                    let chk = Opts { watchdog: w, pad_roll: 1.0, ..ev.clone() };
                    let chk_s = crate::napper::simulate(&chk);
                    println!("{}", crate::napper::event_line("warm start, P 1 (the model's control: the fixed beats' golems)", &chk, &chk_s, None));
                    let normal_o = Opts { watchdog: w, ..ev.clone() };
                    let normal_s = crate::napper::simulate(&normal_o);
                    let g = &normal_s.golems_by_day;
                    let sc_days: Vec<usize> = g.iter().copied().skip(warmup).take(days.saturating_sub(warmup)).collect();
                    let norm = if sc_days.is_empty() { 0.0 } else { sc_days.iter().sum::<usize>() as f64 / sc_days.len() as f64 };
                    println!("{}", crate::napper::event_line("normal running, warm start, P 0.99", &normal_o, &normal_s, None));
                    for rk in [None, Some(5usize)] {
                        let tag = rk.map_or(String::new(), |k| format!(", recovery picker k' {k}"));
                        let cold = Opts { watchdog: w, events_from: 0, recover_k: rk, ..ev.clone() };
                        println!("{}", crate::napper::event_line(&format!("the first start, never-slept riders{tag}"), &cold, &crate::napper::simulate(&cold), Some(norm)));
                        for (lab, ab) in [("a 30,000-gt absence returning mid-day", (10usize, 10_000i64, 30_000i64)), ("a 30,000-gt absence returning mid-night", (10, 22_300, 30_000)), ("a 2,000-gt freeze mid-night", (10, 4_000, 2_000)), ("a 2,000-gt freeze mid-day", (10, 14_000, 2_000)), ("a 20,000-gt absence from mid-day", (10, 14_000, 20_000))] {
                            let o = Opts { watchdog: w, absence: Some(ab), recover_k: rk, ..ev.clone() };
                            let s = crate::napper::simulate(&o);
                            println!("{}", crate::napper::event_line(&format!("{lab}{tag}"), &o, &s, Some(norm)));
                        }
                    }
                }
                return Ok(());
            }
            let rows: Vec<(String, Opts)> = if table {
                let reg = Opts { picker: Picker::Register(every), keep_place: true, gate_m: None, gate_lap: false, gate_batches: None, virtual_count: false, gate_lap_off: 0, gate_virtual: false, free_at_reentry: false, register_carry: false, skip_per_day: 0, ..base.clone() };
                let built = Opts { register_carry: true, visits: 99, spacing: 10, register_after: true, ..reg.clone() };
                let grav = Opts { picker: Picker::FromPick(k), keep_place: false, gate_m: None, gate_lap: false, gate_batches: None, virtual_count: false, gate_lap_off: 0, gate_virtual: false, free_at_reentry: false, register_carry: false, skip_per_day: 0, trip: (200, 240), ..base.clone() };
                let mut v: Vec<(String, Opts)> = vec![
                    ("register as built (calibration: after-stand divert, 22 visits a night, the cart index carried)".into(), built.clone()),
                    ("register as built + (a)".into(), Opts { skip_per_day: 2, ..built.clone() }),
                    ("register as built + (a) + the 21 cap".into(), Opts { skip_per_day: 2, visits: 21, ..built.clone() }),
                    ("register, before-stand divert (index reset at dusk, 21 cap)".into(), reg.clone()),
                    ("register, before-stand divert + (a)".into(), Opts { skip_per_day: 2, ..reg.clone() }),
                    ("gravity".into(), grav.clone()),
                    ("gravity + (b)".into(), Opts { picker: Picker::Anchored(k, offset), ..grav.clone() }),
                    ("gravity + (c) D=6, a trip inside 7 passes (the math's control)".into(), Opts { gate_m: Some(7), trip: (150, 190), ..grav.clone() }),
                    ("gravity + (c) D=6".into(), Opts { gate_m: Some(7), ..grav.clone() }),
                    ("gravity + (c) D=27".into(), Opts { gate_m: Some(28), ..grav.clone() }),
                    ("gravity + (c) D=21 + (a)".into(), Opts { gate_m: Some(22), skip_per_day: 2, ..grav.clone() }),
                ];
                for s in 0..4 {
                    v.push((format!("gravity + (c') lap-exact gate, {s} skipped"), Opts { gate_lap: true, skip_per_day: s, ..grav.clone() }));
                }
                for s in 0..4 {
                    v.push((format!("gravity, 58f770cc's picker (k 1, re-entry after the 2nd batch), {s} skipped"), Opts { picker: Picker::FromPick(1), gate_batches: Some(2), skip_per_day: s, ..grav.clone() }));
                }
                v.push(("gravity + (c') lap-exact gate, register-timed picks, 2 skipped".into(), Opts { picker: Picker::Register(every), gate_lap: true, skip_per_day: 2, ..grav.clone() }));
                for kk in [11usize, 13] {
                    for s in [0usize, 2] {
                        v.push((format!("gravity + (c'') lap-exact gate + the virtual count, k {kk}, {s} skipped"), Opts { picker: Picker::FromPick(kk), gate_lap: true, virtual_count: true, skip_per_day: s, ..grav.clone() }));
                    }
                }
                v
            } else if sweep_k {
                (1..=2 * carts).map(|kk| (format!("k {kk}"), Opts { picker: match base.picker { Picker::FromPick(_) => Picker::FromPick(kk), Picker::FromReturn(_) => Picker::FromReturn(kk), Picker::Anchored(_, o) => Picker::Anchored(kk, o), p => p }, ..base.clone() })).collect()
            } else {
                vec![("row".into(), base)]
            };
            for (label, o) in &rows {
                let sc_row = crate::napper::simulate(o);
                // (event-driven rows: `line()`'s loss buckets count a watchdog cycle and a held start
                // wrongly (G1 Nov 11 21:35); `event_line` alone)
                if !o.events {
                    println!("{}", crate::napper::line(label, o, &sc_row));
                }
                if o.events {
                    println!("{}", crate::napper::event_line(label, o, &sc_row, None));
                } else if o.lock || o.jump.is_some() || o.beds > 1 {
                    let g: Vec<usize> = sc_row.golems_by_day.iter().copied().skip(o.warmup).take(o.days.saturating_sub(o.warmup)).collect();
                    let mean = if g.is_empty() { 0.0 } else { g.iter().sum::<usize>() as f64 / g.len() as f64 };
                    println!("  {label}: {} carts; golems a day {mean:.1} (least {}, most {}); lost to the self-lock {:.2} a day; summon ticks with fewer than 21 carts present {:.1}%; the least turn spacing of any rider {} gt", o.carts, g.iter().min().copied().unwrap_or(0), g.iter().max().copied().unwrap_or(0), sc_row.lock_lost as f64 / sc_row.days.max(1) as f64, 100.0 * sc_row.short_present as f64 / sc_row.summon_ticks.max(1) as f64, if sc_row.least_spacing == i64::MAX { 0 } else { sc_row.least_spacing });
                    if let Some((d, _)) = o.jump {
                        let from = d.saturating_sub(1);
                        println!("  golems each day from day {from}: {}", sc_row.golems_by_day.iter().enumerate().skip(from).take(5).map(|(i, g)| format!("d{i} {g}")).collect::<Vec<_>>().join(", "));
                    }
                }
                if trip_bound {
                    // (G1 Nov 11 18:27: each row's longest zero-loss trip, fixed trips scanned up in
                    // 5-gt steps from 100: a design checks its measured trip against it)
                    let mut ok: Option<i64> = None;
                    let mut first_bad: Option<(i64, usize)> = None;
                    for tt in (100..=700).step_by(5) {
                        let sc = crate::napper::simulate(&Opts { trip: (tt, tt), slow_visit: None, ..o.clone() });
                        if sc.bad_days.is_empty() {
                            ok = Some(tt);
                        } else {
                            first_bad = Some((tt, sc.bad_days.len()));
                            break;
                        }
                    }
                    println!("  {label}: the longest zero-loss trip {} gt{}", ok.map_or("none from 100".to_string(), |t| t.to_string()), first_bad.map_or(String::new(), |(t, n)| format!(" (the first loss at {t} gt: {n} days losing more than the skips)")));
                }
            }
        }
        MobfarmCmd::ChainBench { out, stages, freeze_len, pulses, every, width, pb, pb_stage, pb_transit, pair_stride, terminator, release_stage } => {
            let d = crate::chainbench::design(stages, pb_transit, terminator);
            std::fs::write(&out, format!("# The pausable timers' shared lockable chain: {stages} 1-tick repeaters, side lockers on a NOT-ALIVE bus{}{} (mctas mobfarm chain-bench).\n{}", if terminator { ", a dummy repeater after the last tap" } else { "" }, if pb_transit { "; plus the bench's mid-gt bus drivers (scaffolding: a repeater per driver, not part of the chain)" } else { "" }, d.to_text()))?;
            let (_c, spec) = load(version, &out)?;
            let lines = if pb_transit {
                let mut ns = vec![1, 2];
                if !ns.contains(&pb_stage) {
                    ns.push(pb_stage);
                }
                crate::chainbench::bench_pb(&spec, stages, freeze_len, &ns, pair_stride, release_stage, terminator)
            } else if pulses > 0 {
                crate::chainbench::bench_train(&spec, stages, freeze_len, every, width, pulses, pb, pb_stage)
            } else {
                crate::chainbench::bench(&spec, stages, freeze_len)
            };
            for l in lines.map_err(|e| anyhow::anyhow!(e))? {
                println!("{l}");
            }
        }
        MobfarmCmd::CtlSidingBench { window_open, t_pb, a_max } => {
            let t_pb = if t_pb.is_empty() { vec![152, 215, 296] } else { t_pb };
            for row in t_pb.chunks(3) {
                if row.len() == 3 {
                    for l in crate::ctlcounter::sidings(window_open, [row[0], row[1], row[2]], a_max) {
                        println!("{l}");
                    }
                }
            }
        }
        MobfarmCmd::CtlCounterBench { ff_period, no_pend, window_open, window_close, gate_hi, gate_lo, load2, load1, to_bed, early_tol, last_nap, day_inhibit } => {
            let o = crate::ctlcounter::CounterOpts {
                ff_period,
                pend: !no_pend,
                window: (window_open, window_close),
                gate: (gate_hi, gate_lo),
                load2,
                load1,
                to_bed,
                early_tol,
                last_nap,
                day_inhibit,
                ..Default::default()
            };
            for l in crate::ctlcounter::bench(&o) {
                println!("{l}");
            }
        }
        MobfarmCmd::CtlRingBench { out, cycles, period, width, jitter } => {
            let d = crate::ctlring::design();
            std::fs::write(&out, format!("# The controller's units digit: a 10-dropper ring, 4 tokens (the u = 0 one unstackable), one tap on D8 (mctas mobfarm ctl-ring-bench).\n{}", d.to_text()))?;
            let (_c, spec) = load(version, &out)?;
            let jitter = if jitter.is_empty() { vec![0] } else { jitter };
            for l in crate::ctlring::bench(&spec, cycles, period, width, &jitter).map_err(|e| anyhow::anyhow!(e))? {
                println!("{l}");
            }
        }
        MobfarmCmd::RingBench { out, ticks, stretch, gone_at, speed, carts, sealed, alive } => {
            let d = crate::ringbench::design_alive(speed, carts, sealed, alive);
            std::fs::write(&out, format!("# The fast liveness ring: 24 rail cells (7 x 7), 8 detector rails every 3 cells, {carts} empty cart(s) (mctas mobfarm ring-bench).\n{}", d.to_text()))?;
            let (_c, spec) = load(version, &out)?;
            for l in crate::ringbench::bench(&spec, ticks, stretch, gone_at).map_err(|e| anyhow::anyhow!(e))? {
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
        MobfarmCmd::QueueLoopRun { design, cycles, seed, seeds, release_for, advance_at, advance_every, pulse, depart_every, stand_brake, stand_detect, stand_detect_each, intake_brake, inject_at, depart_at, depart_period, open_after, wired, golem_for, stand_reset, shift_depart, s1_at, tail0_late, bench_stand, stand_offset, release_stagger, tail_check_at, stand_detect_slots, sweep, sweep_range, corners, entry_speed, adv_shift, adv_width, skew, ideal_stand_at, tail_check_rest, tail2_at, placed_raw, placed_both, placed_in_order, placed_all, build_gap_sweep, siding, siding_at, siding_turn, siding_box, stagger_off, stagger_common_off, side_drop, side_drop_staged, side_drop_staged2, shift, stop_open, stop_close, pocket_release, pocket_reset, stop_brake_delay, stop_cv, jobs, reload_orders, force_phase, phase_sweep, gate_close_delay, visit, visits_nightly, visit_release_k, visit_release_cells, visit_t_at_release, visit_k_mod3, visits_observe, divert_branch, divert_at, divert_cell, t1_cell, t2_cell, siding_release_cell, rel_for, pocket_trigger, no_summons, calm, reach_probe, exposure_out, record_positions, golem_miss, no_inject, start_random, fifo_intake, esc_b, verbose_tail, tail_trace, verbose, watch, watch_cells, cross_list, watch_mob, watch_mob_slots, golem_jitter, golem_late, day_only, slept, summon_at, summon_unaligned, no_cross_push_check, placed_yaw, act, reset_queue_at, reset_stragglers, reset_stand_carts, reset_queue_every, drive_out, esc_adm, adm_k2, adm_evidence, adm_rpulse, rider_space_control, sight, p_always, adm_fault, entry_hold, hold_landing_rest, hold_strand_t, hold_progress, reg_interlock, l33_delay, adm_skip_req, arrivals_out, arrival_log, hold_back, bench_release, release_start, release_offs, release_computed, release_from, flee_gate, no_flee_check, no_eye_check, takeoff_list, calm_list, no_junction_check, no_peak_check, esc_bands, no_blind_check, blind_all, freeze_rows, freeze_for, freeze_after, place_rest, rest_speeds, c_prime, rel_ext, rel_min, late_hold, rel_gate, split, interlock, interlock_launch, interlock_arrivals, stand_hold, depart_space, rel_end_n, rel_end_count, rel_end_rear, unset_quiet, q_needs_p1, chain_study, release_gaps, gate_delay, g2_tripwire, stand_meter, takeoff_z, fault_table, perfect_tail, all_corners, rel_sweep, cprime_variants, bed_distance, approach_trace, rel_clear, head_wait, head_latch, corner_table, speeds_out, rise_list, meter_hold, meter_sweep, meter_freeze, meter_freeze_n, head_perm, strip_riders, trace_pair, head_retry, train_size, router_pulses, dep_x, vanish, cancel_x, stale_clear, stale_ungated, stale_ignore_dh3, stale_live, darr_gate, go_lead, hold_cart, retry_darr, darr_stuck, retry_emp_only, no_arr_rules, fault_slot, meter_corners, s_cell, vanish_box, hold_box, summon_raw, cell_log, stall_box, riderless_rows, lx_off, lx_stuck, emp_snapshot, dep_pulse, t_watch, p2_stuck, retry_over_pending, live_go, contacts_all, freeze_matrix, pause_clock } => {
            let (c, mut spec) = load(version, &design)?;
            // (every run reports the design's chunk-line crossings: ironspec's placement rule,
            // Nov 11; a report, the failing crossings listed)
            println!("{}", crate::borders::summary(&c.clock.design.blocks, [0, 0], 12));
            let o = crate::queue::loop_opts_of(&c.clock.design.meta)?;
            // (a static S3 file, `s3:` header: B's offset, mirror, delta, S's z and the liveness
            // clocks' probes as KEY=VALUE, the same as the MCTAS_* variables, which still win)
            if let Some(h) = c.clock.design.meta.get("s3") {
                for kv in h.split_whitespace() {
                    let Some((k, v)) = kv.split_once('=') else { continue };
                    let var = match k {
                        "b_off" => "MCTAS_WIRED_B_OFF",
                        "mirror" => "MCTAS_WIRED_B_MIRROR",
                        "delta" => "MCTAS_WIRED_B_DELTA",
                        "router_z" => "MCTAS_ROUTER_Z",
                        "clocks" => "MCTAS_CLOCK_PROBES",
                        _ => anyhow::bail!("s3: unknown key {k}"),
                    };
                    if std::env::var(var).is_err() {
                        // SAFETY: set once at startup, before the runner starts any thread
                        unsafe { std::env::set_var(var, v) };
                    }
                }
            }
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
            }, arrivals: arrivals_out.is_some(), speeds: speeds_out.is_some(), rise_list, verbose_tail, tail_trace: match tail_trace.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--tail-trace takes C0,C1") }, pulse_at: c.clock.design.ports.get("output.pulse").map(|p| redsim::pos::BlockPos::new(p[0], p[1], p[2])), release_start, bench_release, release_offs: match (release_computed, release_offs.as_slice()) { (true, _) | (false, []) => None, (false, [a, b, c]) => Some([*a, *b, *c]), _ => anyhow::bail!("--release-offs takes Q15,Q16,Q17") }, release_from, c_prime: match c_prime.as_slice() { [x, y, z, d, f] => Some(([*x as i32, *y as i32, *z as i32], *d, *f, 40)), [x, y, z, d, f, l] => Some(([*x as i32, *y as i32, *z as i32], *d, *f, *l)), [] => None, _ => anyhow::bail!("--c-prime takes X,Y,Z,DELAY,FOR[,LOCK]") }, rel_ext, rel_min, late_hold: { anyhow::ensure!((late_hold.len() % 6 == 0 && late_hold.len() <= 18) || late_hold.len() == 5, "--late-hold takes X,Y,Z,N,W[,K] (K the gate: 2 = G2, 1 = G1), up to 3 as one list of sixes"); let mut a = [None; 3]; if late_hold.len() == 5 { a[0] = Some(([late_hold[0] as i32, late_hold[1] as i32, late_hold[2] as i32], late_hold[3], late_hold[4], 2)); } else { for (i, c) in late_hold.chunks(6).enumerate() { a[i] = Some(([c[0] as i32, c[1] as i32, c[2] as i32], c[3], c[4], c[5] as usize)); } } a }, rel_gate: match rel_gate.as_slice() { [s, t, l] | [s, t, l, _] => Some((*s, *t, *l)), [] => None, _ => anyhow::bail!("--rel-gate takes S,D,L[,U]") }, rel_gate_unset: rel_gate.get(3).copied().unwrap_or(0), split: match split.as_slice() { [x, y, z, w] => Some(([*x as i32, *y as i32, *z as i32], *w)), [] => None, _ => anyhow::bail!("--split takes X,Y,Z,W") }, interlock, interlock_launch, interlock_arrivals, stand_hold: match stand_hold.as_slice() { [x, y, z] => Some([*x, *y, *z]), [x, y, z, _] => Some([*x, *y, *z]), [] => None, _ => anyhow::bail!("--stand-hold takes X,Y,Z[,H]") }, stand_hold_after: stand_hold.get(3).map(|h| *h as i64).unwrap_or(0), depart_space, rel_end_n: match rel_end_n.as_slice() { [d, to] => Some((*d, *to)), [] => None, _ => anyhow::bail!("--rel-end-n takes DELAY,TIMEOUT") }, rel_end_count: match rel_end_count.as_slice() { [k, to] => Some((*k, *to)), [] => None, _ => anyhow::bail!("--rel-end-count takes K,TIMEOUT") }, rel_end_rear: match rel_end_rear.as_slice() { [x, y, z, d, to] => Some(([*x as i32, *y as i32, *z as i32], *d, *to)), [] => None, _ => anyhow::bail!("--rel-end-rear takes X,Y,Z,D,TIMEOUT") }, unset_quiet, q_needs_p1, chain_study, release_gaps: match release_gaps.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--release-gaps takes G12,G23") }, gate_delay, g2_tripwire, stand_meter, takeoff_z, bed_at: match bed_distance.as_slice() { [x, y, z] => Some([*x, *y, *z]), [] => None, _ => anyhow::bail!("--bed-distance takes X,Y,Z") }, approach_trace: match approach_trace.as_slice() { [a, b] => Some((*a, *b, None)), [a, b, x] => Some((*a, *b, Some(*x as i32))), [] => None, _ => anyhow::bail!("--approach-trace takes C0,C1[,X]") }, rel_clear: match rel_clear.as_slice() { [x, y, z] => Some([*x, *y, *z]), [] => None, _ => anyhow::bail!("--rel-clear takes X,Y,Z") }, head_wait, head_latch: match head_latch.as_slice() { [k, x, y, z] => Some((*k as usize, [*x, *y, *z])), [] => None, _ => anyhow::bail!("--head-latch takes K,X,Y,Z") }, s_cell: match s_cell.as_slice() { [x, y, z] => Some(([*x, *y, *z], None)), [x, y, z, a, b, d] => Some(([*x, *y, *z], Some([*a, *b, *d]))), [] => None, _ => anyhow::bail!("--s-cell takes X,Y,Z[,DX,DY,DZ]") }, meter: match meter_hold.as_slice() { [] => None, [d12, d23, close, dep, hold_on, rest @ ..] => Some(crate::queue::MeterHold { d12: *d12, d23: *d23, close: *close, dep: *dep, hold_on: *hold_on, chain: rest.first().copied().unwrap_or(0), fallback: rest.get(1).copied().unwrap_or(0), miss_d1: rest.get(2).copied().unwrap_or(-1), vb: rest.get(3).copied().unwrap_or(0) == 1, wired_b: rest.get(3).copied().unwrap_or(0) == 2, open_len: rest.get(4).copied().unwrap_or(0), wired_h: rest.get(5).copied().unwrap_or(0), h_delay: rest.get(6).copied().unwrap_or(0).max(0), guard: rest.get(7).copied().unwrap_or(0) != 0, g2_delay: rest.get(8).copied().unwrap_or(6).max(0), g1_delay: rest.get(9).copied().unwrap_or(0).max(0), p2: rest.get(10).copied().unwrap_or(1) != 0, t_live: rest.get(11).copied().unwrap_or(50).max(1) }), _ => anyhow::bail!("--meter-hold takes D12,D23,CLOSE,DEP,HOLD_ON[,CHAIN[,FALLBACK[,MISS_D1]]]") }, head_perm: match head_perm.as_slice() { [a, b, c, d, e, f] => Some(([*a, *b, *c], [*d, *e, *f], false, false, None)), [a, b, c, d, e, f, g] => Some(([*a, *b, *c], [*d, *e, *f], *g != 0, false, None)), [a, b, c, d, e, f, g, r] => Some(([*a, *b, *c], [*d, *e, *f], *g != 0, *r != 0, None)), [a, b, c, d, e, f, g, r, c0, n] => Some(([*a, *b, *c], [*d, *e, *f], *g != 0, *r != 0, Some((*c0 as i64, *n as i64)))), [] => None, _ => anyhow::bail!("--head-perm takes AX,AY,AZ,HX,HY,HZ[,G[,R[,BC0,BN]]]") }, strip_riders: match strip_riders.as_slice() { [x, y, z] => Some(([*x as i32, *y as i32, *z as i32], 0, i64::MAX, 0)), [x, y, z, s0, s1] => Some(([*x as i32, *y as i32, *z as i32], *s0, *s1, 0)), [x, y, z, s0, s1, k] => Some(([*x as i32, *y as i32, *z as i32], *s0, *s1, *k as usize)), [] => None, _ => anyhow::bail!("--strip-riders takes X,Y,Z[,S0,S1[,K]]") }, trace_pair: match trace_pair.as_slice() { [a, b, t0, t1] => Some((*a as u32, *b as u32, *t0, *t1)), [] => None, _ => anyhow::bail!("--trace-pair takes A,B,T0,T1") }, head_retry: match head_retry.as_slice() { [] | [0] => None, [_] => Some((None, None)), [_, c0, n] => Some((Some((*c0, *n)), None)), [_, c0, n, lost] => Some((if *n > 0 { Some((*c0, *n)) } else { None }, Some(*lost))), _ => anyhow::bail!("--head-retry takes ON[,AC0,AN[,LOST]]") }, train_size: match train_size.as_slice() { [s, k] => Some((*s, *k)), [] => None, _ => anyhow::bail!("--train-size takes SLOT,K") }, router_pulses: match router_pulses.as_slice() { [a, b, c, d, e] => Some([*a, *b, *c, *d, *e]), [] => None, _ => anyhow::bail!("--router-pulses takes BMISS1,BMISS2,BDOUBLE1,BDOUBLE2,WRONG (-1 none)") }, dep_x, vanish: match vanish.as_slice() { [s, d, z0, z1] => Some((*s as i64, *d as i64, *z0, *z1, 1)), [s, d, z0, z1, n] => Some((*s as i64, *d as i64, *z0, *z1, *n as i64)), [] => None, _ => anyhow::bail!("--vanish takes SLOT,DT,Z0,Z1[,N]") }, cancel_x, stale_clear, stale_ungated, stale_ignore_dh3, stale_live: stale_live || (!head_retry.is_empty() && head_retry[0] != 0 && !no_arr_rules && !stale_clear && !stale_ungated && !stale_ignore_dh3), darr_gate: darr_gate || (!head_retry.is_empty() && head_retry[0] != 0 && !no_arr_rules), go_lead, hold_cart: match hold_cart.as_slice() { [s, d, z0, z1, n, k] => Some((*s as i64, *d as i64, *z0, *z1, *n as i64, *k as i64)), [] => None, _ => anyhow::bail!("--hold-cart takes SLOT,DT,Z0,Z1,N,K") }, retry_darr: retry_darr || (!head_retry.is_empty() && head_retry[0] != 0 && !retry_emp_only), darr_stuck: match darr_stuck.as_slice() { [s, lv] => Some((*s, *lv != 0)), [] => None, _ => anyhow::bail!("--darr-stuck takes SLOT,LEVEL") }, vanish_box: match vanish_box.as_slice() { [s, d, x0, x1, z0, z1] => Some((*s as i64, *d as i64, *x0, *x1, *z0, *z1, 1)), [s, d, x0, x1, z0, z1, k] => Some((*s as i64, *d as i64, *x0, *x1, *z0, *z1, *k as i64)), [] => None, _ => anyhow::bail!("--vanish-box takes SLOT,DT,X0,X1,Z0,Z1[,K]") }, hold_box: match hold_box.as_slice() { [s, d, x0, x1, z0, z1, n] => Some((*s as i64, *d as i64, *x0, *x1, *z0, *z1, *n as i64)), [] => None, _ => anyhow::bail!("--hold-box takes SLOT,DT,X0,X1,Z0,Z1,N") }, summon_raw, cell_log: { let mut a = [None; 8]; for (i, s) in cell_log.split(';').filter(|s| !s.trim().is_empty()).enumerate() { anyhow::ensure!(i < 8, "--cell-log: at most 8 cells"); let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("--cell-log: {e}"))?; anyhow::ensure!(v.len() == 3, "--cell-log takes x,y,z;x,y,z"); a[i] = Some([v[0], v[1], v[2]]); } a }, stall_box: { let mut a = [None; 4]; for (i, s) in stall_box.split(';').filter(|s| !s.trim().is_empty()).enumerate() { anyhow::ensure!(i < 4, "--stall-box: at most 4 boxes"); let v: Vec<f64> = s.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("--stall-box: {e}"))?; anyhow::ensure!(v.len() == 4, "--stall-box takes x0,x1,z0,z1;..."); a[i] = Some((v[0], v[1], v[2], v[3])); } a }, lx_off, lx_stuck: match lx_stuck.as_slice() { [s, lv] => Some((*s, *lv != 0, i64::MAX)), [s, lv, n] => Some((*s, *lv != 0, *n)), [] => None, _ => anyhow::bail!("--lx-stuck takes SLOT,LEVEL[,N]") }, emp_snapshot, dep_pulse: match dep_pulse.as_slice() { [s, d] => Some((*s, *d)), [] => None, _ => anyhow::bail!("--dep-pulse takes SLOT,DT") }, t_watch: match t_watch.as_slice() { [x, y, z] => Some([*x, *y, *z]), [] => None, _ => anyhow::bail!("--t-watch takes X,Y,Z") }, p2_stuck: match p2_stuck.as_slice() { [s, lv] => Some((*s, *lv != 0, i64::MAX)), [s, lv, n] => Some((*s, *lv != 0, *n)), [] => None, _ => anyhow::bail!("--p2-stuck takes SLOT,LEVEL[,N]") }, retry_over_pending, live_go, contacts_all: contacts_all || !freeze_matrix.is_empty(), pause_clock: match pause_clock.as_slice() { [x, z] => Some((*x, *z, false)), [x, z, r] => Some((*x, *z, *r != 0)), [] => None, _ => anyhow::bail!("--pause-clock takes X,Z[,RESYNC]") }, ..Default::default() };
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
                    println!("{}", crate::approach::fault_table(&spec2, &o2, &t, &seed_list, cycles, jobs, perfect_tail, if f.is_empty() { None } else { Some(f.as_str()) }, all_corners, match rel_sweep.as_slice() { [a, b] => Some((*a, *b)), [] => None, _ => anyhow::bail!("--rel-sweep takes A,B") }, fault_slot, meter_corners).map_err(|e| anyhow::anyhow!(e))?);
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
            // (the rail-peak check, a default gate (9fc3155f's find, Nov 11): a slope's high end
            // straight into a slope going down is a top no cart crosses; the opt-out says so)
            if !no_peak_check {
                let g = crate::borders::peaks_line(&c.clock.design.blocks);
                if !sweep.is_empty() || phase_sweep.is_some() {
                    println!("{}{}", g.0, if g.1 { " -- the static gate FAILS" } else { "" });
                }
                hdr.static_checks.push(g);
            } else {
                hdr.static_checks.push(("rail peaks: NOT CHECKED (--no-peak-check)".to_string(), false));
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
            // (`--meter-sweep L0,L1[,S0,S1]`: H's fixed opening swept at every gt step: each open
            // length L (OPEN_LEN) with every start shift S (each opening's start S gt later: HOLD_ON
            // and D12/D23 each + S, so H1 = the train's Darr-on + HOLD_ON + S, H2/H3 = the previous
            // Dh edge + D + S), one run each on `jobs` threads; per combination the openings that
            // passed 0 / 1 / 2+ carts, the carts in H's box at its shuts, the follower's margin,
            // the window line and the problems: the band where every opening passes exactly one
            // cart and no cart is in H's cell at the shut)
            if !meter_sweep.is_empty() {
                let base = tm.meter.ok_or_else(|| anyhow::anyhow!("--meter-sweep needs --meter-hold"))?;
                let (w0, w1, d0, d1) = match meter_sweep.as_slice() {
                    [a, b] => (*a, *b, 0, 0),
                    [a, b, c, d] => (*a, *b, *c, *d),
                    _ => anyhow::bail!("--meter-sweep takes L0,L1[,S0,S1]"),
                };
                let combos: Vec<(i64, i64)> = (w0..=w1).flat_map(|w| (d0..=d1).map(move |d| (w, d))).collect();
                println!("meter sweep: open lengths {w0}..{w1} gt, start shifts {d0}..{d1} gt on HOLD_ON {} D12 {} D23 {} ({} runs, seed {seed}, {cycles} cycles)", base.hold_on, base.d12, base.d23, combos.len());
                for chunk in combos.chunks(jobs.max(1)) {
                    let got: Vec<((i64, i64), Result<String, String>)> = std::thread::scope(|sc| {
                        let hs: Vec<_> = chunk
                            .iter()
                            .map(|&(w, d)| {
                                let (spec, o, hdr) = (&spec, &o, &hdr);
                                let mut t2 = tm.clone();
                                t2.meter = Some(crate::queue::MeterHold { open_len: w, hold_on: base.hold_on + d, d12: base.d12 + d, d23: base.d23 + d, ..base });
                                sc.spawn(move || ((w, d), crate::queue::run_loop_hdr(spec, o, &t2, hdr, cycles, seed, false, None, None)))
                            })
                            .collect();
                        hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
                    });
                    for ((w, d), r) in got {
                        let r = r.map_err(|e| anyhow::anyhow!(e))?;
                        let pick = |p: &str| r.lines().filter(|l| l.starts_with(p)).map(|l| l.trim_start_matches(p).trim().to_string()).collect::<Vec<_>>().join(" | ");
                        let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("?").to_string();
                        let touch = r.lines().find_map(|l| l.split("H closes, gt until a cart touches its face ").nth(1)).and_then(|x| x.split(';').next()).unwrap_or("-");
                        let merges = r.lines().find(|l| l.contains("merged carts:")).map(|l| l.trim().chars().take(60).collect::<String>()).unwrap_or_default();
                        let back = r.lines().find_map(|l| l.split("back travel up to ").nth(1)).map(|x| x.split(';').next().unwrap_or("").to_string()).unwrap_or_default();
                        let contact = r.lines().find_map(|l| l.strip_prefix("meter arrivals at a waiting cart (first contact on the hold's column: a push, or a dead stop at a rider's box): ")).map(|x| x.split("; the worst:").next().unwrap_or(x)).unwrap_or("-");
                        println!("L {w} S {d:+}: {} | shuts: {} | clear..follower window: {} | shut to the next cart at H's face: {touch} | back: {back} | contact: {contact} | {probs}{}", pick("meter H opening "), pick("meter H shuts: carts in H's box at the shut 0/1/2+:"), r.lines().find_map(|l| l.split("common window ").nth(1)).unwrap_or("-"), if merges.is_empty() { String::new() } else { format!(" | {merges}") });
                    }
                }
                return Ok(());
            }
            // (`--meter-freeze SLOT,DT0,DT1,N[,X0,Z0,X1,Z1]`: one run per gt from SLOT +DT0 to +DT1,
            // each with the chunk box (default the stand's chunk 0,0..0,0) unloaded N gt from that
            // gt (ironspec's freeze rows at every gt of an opening); per run H's openings, its
            // shuts, the gates shut on a cart and the problems)
            if !meter_freeze.is_empty() {
                let (slot, a, b, n, bx) = match meter_freeze.as_slice() {
                    [s, a, b, n] => (*s, *a, *b, *n, [0i64, 0, 0, 0]),
                    [s, a, b, n, x0, z0, x1, z1] => (*s, *a, *b, *n, [*x0, *z0, *x1, *z1]),
                    _ => anyhow::bail!("--meter-freeze takes SLOT,DT0,DT1,N[,X0,Z0,X1,Z1]"),
                };
                let free = tm.acts.iter().position(|x| x.is_none()).ok_or_else(|| anyhow::anyhow!("--meter-freeze: all 8 act slots taken"))?;
                // (`--meter-freeze-n N0,N1`: the freeze's length swept at the fixed start +DT0)
                let runs: Vec<(i64, i64)> = match meter_freeze_n.as_slice() {
                    [n0, n1] => (*n0..=*n1).map(|nn| (a, nn)).collect(),
                    [] => (a..=b).map(|dt| (dt, n)).collect(),
                    _ => anyhow::bail!("--meter-freeze-n takes N0,N1"),
                };
                if meter_freeze_n.is_empty() {
                    println!("meter freeze: the chunk box {},{}..{},{} unloaded {n} gt from slot {slot} +{a}..+{b} ({} runs, seed {seed}, {cycles} cycles)", bx[0], bx[1], bx[2], bx[3], runs.len());
                } else {
                    println!("meter freeze: the chunk box {},{}..{},{} unloaded from slot {slot} +{a} for {}..{} gt ({} runs, seed {seed}, {cycles} cycles)", bx[0], bx[1], bx[2], bx[3], meter_freeze_n[0], meter_freeze_n[1], runs.len());
                }
                for chunk in runs.chunks(jobs.max(1)) {
                    let got: Vec<((i64, i64), Result<String, String>)> = std::thread::scope(|sc| {
                        let hs: Vec<_> = chunk
                            .iter()
                            .map(|&(dt, nn)| {
                                let (spec, o, hdr) = (&spec, &o, &hdr);
                                let mut t2 = tm.clone();
                                let act = crate::queue::parse_act(&format!("{slot},{dt},unload,{},{},{},{},{nn}", bx[0], bx[1], bx[2], bx[3]));
                                sc.spawn(move || {
                                    let r = act.map(|x| {
                                        t2.acts[free] = Some(x);
                                    });
                                    ((dt, nn), r.and_then(|_| crate::queue::run_loop_hdr(spec, o, &t2, hdr, cycles, seed, false, None, None)))
                                })
                            })
                            .collect();
                        hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
                    });
                    for ((dt, nn), r) in got {
                        let r = r.map_err(|e| anyhow::anyhow!(e))?;
                        let pick = |p: &str| r.lines().filter(|l| l.starts_with(p)).map(|l| l.trim_start_matches(p).trim().to_string()).collect::<Vec<_>>().join(" | ");
                        let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("?").to_string();
                        let merges = r.lines().find(|l| l.contains("merged carts:")).map(|l| l.trim().chars().take(60).collect::<String>()).unwrap_or_default();
                        let back = r.lines().find_map(|l| l.split("back travel up to ").nth(1)).map(|x| x.split(';').next().unwrap_or("").to_string()).unwrap_or_default();
                        let contact = r.lines().find_map(|l| l.strip_prefix("meter arrivals at a waiting cart (first contact on the hold's column: a push, or a dead stop at a rider's box): ")).map(|x| x.split("; the worst:").next().unwrap_or(x)).unwrap_or("-");
                        // (by rest place, each arrival's stored speed, T-cell crossings)
                        let places = r.lines().find_map(|l| l.strip_prefix("meter arrivals by rest place: ")).unwrap_or("-");
                        let t_cross = r.lines().find_map(|l| l.split("backward crossings into T's cell: ").nth(1)).unwrap_or("-");
                        // (the head model's summary, when it runs: holds, RETRYs, D_arr holds,
                        // ARR at the end, trains onto waiting carts, stand RELs twice in a slot)
                        let head = if r.lines().any(|l| l.starts_with("head-perm router")) {
                            let holds = r.lines().find_map(|l| l.strip_prefix("head-perm router (phase-locked): ")).and_then(|x| x.split(" holds").next()).unwrap_or("?").to_string();
                            let retries = r.lines().find(|l| l.starts_with("head-perm stand REL")).map_or(0, |l| l.matches("RETRY (").count());
                            let darr = r.lines().find_map(|l| l.split("A's GOs held by it: ").nth(1)).unwrap_or("-").to_string();
                            let arr_end = r.lines().find_map(|l| l.split("ARR_X at the run's end: ").nth(1)).unwrap_or("-").to_string();
                            let second = r.lines().find_map(|l| l.split("a train reaching H while earlier carts still waited there: ").nth(1)).map(|x| x.split(';').next().unwrap_or(x).to_string()).unwrap_or_default();
                            let twice = r.lines().find_map(|l| l.split("slots with two or more: ").nth(1)).unwrap_or("-").to_string();
                            let last_go = r.lines().find_map(|l| l.strip_prefix("head-perm router (phase-locked): ")).and_then(|x| x.split("GO by GO: ").nth(1)).map(|x| x.split(' ').filter(|w| w.contains(':')).last().unwrap_or("").to_string()).unwrap_or_default();
                            format!(" | head: holds {holds}, RETRYs {retries}, D_arr holds {darr}, ARR at the end {arr_end}, trains onto waiting carts {second}, two stand RELs in a slot {twice}, last GO {last_go}")
                        } else {
                            String::new()
                        };
                        println!("freeze at +{dt} for {nn}: {} | shuts: {} | on a cart: {} | back: {back} | T crossings: {t_cross} | contact: {contact} | by place: {places} | {probs}{}{head}", pick("meter H opening "), pick("meter H shuts: carts in H's box at the shut 0/1/2+:"), pick("meter: gates shut on a cart:"), if merges.is_empty() { String::new() } else { format!(" | {merges}") });
                    }
                }
                return Ok(());
            }
            // (cc767b78, `--riderless-rows STAGGERS;KS`: the riderless matrix in parallel: each
            // release stagger x each empty-cart position K (0 every cart, k the k-th cart of each
            // train, via --strip-riders' cell); per run each --cell-log cell's empty passages (their
            // stored speed in, least inside, the least corner gap with an empty follower), the
            // stalls (and those the no-meetings rule bars or no cart pushed on), backward entries,
            // merges, the closest centres by rest place, T-cell crossings, problems)
            if !riderless_rows.is_empty() {
                let (ss, ks) = riderless_rows.split_once(';').ok_or_else(|| anyhow::anyhow!("--riderless-rows takes STAGGERS;KS, e.g. 2,3,4,5,6;0,1,2,3"))?;
                let parse = |s: &str| s.split(',').filter(|x| !x.trim().is_empty()).map(|x| x.trim().parse::<i64>()).collect::<Result<Vec<_>, _>>();
                let (ss, ks) = (parse(ss)?, parse(ks)?);
                let Some((cell, s0, s1, _)) = tm.strip_riders else { anyhow::bail!("--riderless-rows needs --strip-riders X,Y,Z[,S0,S1] for the stripping cell") };
                let runs: Vec<(i64, i64)> = ss.iter().flat_map(|s| ks.iter().map(move |k| (*s, *k))).collect();
                println!("riderless rows: release staggers {:?} x empty-cart positions {:?} (0: every cart; k: the k-th cart of each train), riders stripped at {},{},{} ({} runs, seed {seed}, {cycles} cycles)", ss, ks, cell[0], cell[1], cell[2], runs.len());
                let mut agg: std::collections::BTreeMap<String, (usize, f64, f64, Option<(i64, String)>)> = std::collections::BTreeMap::new();
                for chunk in runs.chunks(jobs.max(1)) {
                    let got: Vec<((i64, i64), Result<String, String>)> = std::thread::scope(|sc| {
                        let hs: Vec<_> = chunk
                            .iter()
                            .map(|&(st, k)| {
                                let (spec, o, hdr) = (&spec, &o, &hdr);
                                let mut t2 = tm.clone();
                                t2.release_stagger = st;
                                t2.strip_riders = Some((cell, s0, s1, k as usize));
                                sc.spawn(move || ((st, k), crate::queue::run_loop_hdr(spec, o, &t2, hdr, cycles, seed, false, None, None)))
                            })
                            .collect();
                        hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
                    });
                    for ((st, k), r) in got {
                        let r = r.map_err(|e| anyhow::anyhow!(e))?;
                        let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("0 problems").to_string();
                        let mut cells_txt: Vec<String> = Vec::new();
                        for l in r.lines().filter(|l| l.starts_with("cell-log (")) {
                            let name = l.split(':').next().unwrap_or("").trim_start_matches("cell-log ").to_string();
                            let passages = l.split("passages: ").nth(1).unwrap_or("");
                            let empties: Vec<(f64, f64)> = passages
                                .split(" | ")
                                .filter(|p| p.contains(" empty in "))
                                .filter_map(|p| {
                                    let sv = p.split("stored ").nth(1)?.split(' ').next()?.parse::<f64>().ok()?;
                                    let lv = p.split("least ").nth(1)?.split(' ').next()?.parse::<f64>().ok()?;
                                    Some((sv, lv))
                                })
                                .collect();
                            let gap = l.split("empty follower: ").nth(1).and_then(|x| x.split(';').next()).unwrap_or("-").to_string();
                            let gap_n = gap.split(' ').nth(1).and_then(|x| x.parse::<i64>().ok());
                            let (smin, lmin) = (empties.iter().map(|e| e.0).fold(f64::MAX, f64::min), empties.iter().map(|e| e.1).fold(f64::MAX, f64::min));
                            cells_txt.push(if empties.is_empty() { format!("{name}: no empties") } else { format!("{name}: {} empties, stored in {:.3}..{:.3}, least {:.3}, empty-follower gap {gap}", empties.len(), smin, empties.iter().map(|e| e.0).fold(0.0f64, f64::max), lmin) });
                            let a = agg.entry(name).or_insert((0, f64::MAX, f64::MAX, None));
                            a.0 += empties.len();
                            a.1 = a.1.min(smin);
                            a.2 = a.2.min(lmin);
                            if let Some(g) = gap_n
                                && a.3.as_ref().is_none_or(|x| g < x.0)
                            {
                                a.3 = Some((g, format!("stagger {st} K {k}: {gap}")));
                            }
                        }
                        let stalls = r.lines().find_map(|l| l.strip_prefix("empty carts at rest >= 10 gt in the stall boxes: ")).map(|x| x.split(" where the no-meetings").next().unwrap_or(x).to_string()).unwrap_or("-".to_string());
                        let back = r.lines().find_map(|l| l.strip_prefix("meter backward entries through a stand gate or H: ")).map(|x| x.split(' ').next().unwrap_or(x).trim_end_matches(';').to_string()).unwrap_or("-".to_string());
                        let merged = r.lines().find_map(|l| l.split("; merged ").nth(1)).map(|x| x.split(';').next().unwrap_or(x).to_string()).unwrap_or("-".to_string());
                        let places = r.lines().find_map(|l| l.strip_prefix("meter arrivals by rest place: ")).map(|x| x.split("; each arrival").next().unwrap_or(x).to_string()).unwrap_or("-".to_string());
                        let t_cross = r.lines().find_map(|l| l.split("backward crossings into T's cell: ").nth(1)).map(|x| x.split(' ').next().unwrap_or(x).to_string()).unwrap_or("-".to_string());
                        // (7b90c3f1's curve-spacing line: each curve's least margin, tick-exact's
                        // curve-pair rule; the first three curves listed (least first))
                        let curves = r.lines().find_map(|l| l.split("least first): ").nth(1).filter(|_| l.starts_with("curve spacing ("))).map(|x| x.split("); ").take(3).collect::<Vec<_>>().join("); ")).unwrap_or("-".to_string());
                        let curves_e = r.lines().find_map(|l| l.strip_prefix("curve spacing with an empty cart (the least margin with an empty leader, and with an empty follower; least first): ")).map(|x| x.split("); (").take(3).collect::<Vec<_>>().join("); (")).unwrap_or("-".to_string());
                        // (each stopped empty: where and how long, or never moved again)
                        let stall_list = r.lines().find_map(|l| l.strip_prefix("empty carts at rest >= 10 gt in the stall boxes: ")).and_then(|x| x.split_once("): ").map(|(_, rest)| rest.to_string())).map(|rest| {
                            rest.split(" | ")
                                .map(|s| {
                                    let at = s.split("at loop ").nth(1).map(|x| x.split(' ').next().unwrap_or(x)).unwrap_or("?");
                                    let wait = if s.contains("never moved again") { "never moved again".to_string() } else { s.split(" gt)").next().and_then(|x| x.rsplit('(').next()).map(|x| format!("{x} gt")).unwrap_or_default() };
                                    let how = if s.contains("by a cart-cart push") { ", pushed on" } else if s.contains("by itself") { ", by itself" } else { "" };
                                    format!("{at} {wait}{how}")
                                })
                                .collect::<Vec<_>>()
                                .join("; ")
                        }).unwrap_or_default();
                        println!("stagger {st} K {k}: stalls {stalls}){} | backward entries {back} | merged {merged} | T crossings {t_cross} | curves: {curves}) | empty-cart curves: {curves_e} | by place: {places} | {probs} | {}", if stall_list.is_empty() { String::new() } else { format!(" [{stall_list}]") }, cells_txt.join(" | "));
                    }
                }
                for (name, a) in &agg {
                    println!("over the matrix, {name}: {} empty passages, stored in from {:.3}, least inside {:.3}; the least empty-follower corner gap {}", a.0, if a.0 == 0 { f64::NAN } else { a.1 }, if a.0 == 0 { f64::NAN } else { a.2 }, a.3.as_ref().map_or("-".to_string(), |x| x.1.clone()));
                }
                return Ok(());
            }
            // (cc767b78, `--freeze-matrix SLOT,DT0,DT1,STEP;N1,N2,..`: the whole-column freeze at
            // every phase: the whole part at level 32 (redstone running) from each start for each
            // length, in parallel; per run the bars)
            if !freeze_matrix.is_empty() {
                let (starts, lens) = freeze_matrix.split_once(';').ok_or_else(|| anyhow::anyhow!("--freeze-matrix takes SLOT,DT0,DT1,STEP;N1,N2,.."))?;
                let sv: Vec<i64> = starts.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<_, _>>()?;
                let [slot, d0, d1, step] = sv.as_slice() else { anyhow::bail!("--freeze-matrix takes SLOT,DT0,DT1,STEP;N1,N2,..") };
                let ns: Vec<i64> = lens.split(',').filter(|x| !x.trim().is_empty()).map(|x| x.trim().parse::<i64>()).collect::<Result<_, _>>()?;
                let free = tm.acts.iter().position(|x| x.is_none()).ok_or_else(|| anyhow::anyhow!("--freeze-matrix: all 8 act slots taken"))?;
                let runs: Vec<(i64, i64)> = (*d0..=*d1).step_by((*step).max(1) as usize).flat_map(|dt| ns.iter().map(move |n| (dt, *n))).collect();
                println!("freeze matrix: the whole part at level 32 (redstone running, entities frozen) from slot {slot} +{d0}..+{d1} every {step} gt for {:?} gt ({} runs, seed {seed}, {cycles} cycles)", ns, runs.len());
                let (slot, step) = (*slot, *step);
                let _ = step;
                for chunk in runs.chunks(jobs.max(1)) {
                    let got: Vec<((i64, i64), Result<String, String>)> = std::thread::scope(|sc| {
                        let hs: Vec<_> = chunk
                            .iter()
                            .map(|&(dt, n)| {
                                let (spec, o, hdr) = (&spec, &o, &hdr);
                                let mut t2 = tm.clone();
                                let act = crate::queue::parse_act(&format!("{slot},{dt},freeze32,{n}"));
                                sc.spawn(move || {
                                    let r = act.map(|x| {
                                        t2.acts[free] = Some(x);
                                    });
                                    ((dt, n), r.and_then(|_| crate::queue::run_loop_hdr(spec, o, &t2, hdr, cycles, seed, false, None, None)))
                                })
                            })
                            .collect();
                        hs.into_iter().map(|h| h.join().expect("a bench thread panicked")).collect()
                    });
                    for ((dt, n), r) in got {
                        let r = r.map_err(|e| anyhow::anyhow!(e))?;
                        let after = |l: &str, key: &str| l.split(key).nth(1).map(|x| x.to_string());
                        let first_num = |s: &str| s.trim().split(|c: char| !c.is_ascii_digit()).next().unwrap_or("").to_string();
                        let find = |key: &str| r.lines().find_map(|l| after(l, key));
                        let s_rel = r.lines().find(|l| l.starts_with("freeze at t ")).and_then(|l| l.split(", S+").nth(1)).map(|x| format!("S+{}", first_num(x))).unwrap_or_default();
                        let holds = find("head-perm router (phase-locked): ").map(|x| first_num(&x)).unwrap_or("-".to_string());
                        let last_go = find("GO by GO: ").map(|x| x.split(' ').filter(|w| w.contains(':')).last().unwrap_or("").to_string()).unwrap_or_default();
                        let rels = r.lines().find_map(|l| l.strip_prefix("head-perm stand RELs: ")).map(|x| x.to_string()).unwrap_or("-".to_string());
                        let arr = find("ARR_X at the run's end: ").map(|x| x.split(' ').next().unwrap_or("").to_string()).unwrap_or("-".to_string());
                        let merged = r.lines().find(|l| l.starts_with("merged carts")).and_then(|l| after(l, "them): ")).map(|x| first_num(&x)).unwrap_or("-".to_string());
                        let contacts = find("over 0.2").is_some().then(|| r.lines().find_map(|l| l.strip_prefix("contacts anywhere (a moving cart's first contact with a cart at rest: a push, or a dead stop at a rider's box; the mover's stored speed before): ")).map(|x| x.split(';').next().unwrap_or(x).to_string()).unwrap_or_default()).unwrap_or("-".to_string());
                        let gates = r.lines().find_map(|l| l.strip_prefix("meter: gates shut on a cart:")).map(|x| first_num(x)).unwrap_or("0".to_string());
                        let back = find("meter backward entries through a stand gate or H: ").map(|x| first_num(&x)).unwrap_or("-".to_string());
                        let t_cross = find("backward crossings into T's cell: ").map(|x| first_num(&x)).unwrap_or("-".to_string());
                        let curve = find("must be 0): ").map(|x| first_num(&x)).unwrap_or("-".to_string());
                        let carts_end = find(" at the start, ").map(|x| x.split(" at the end").next().unwrap_or("").to_string()).unwrap_or("-".to_string());
                        let p2 = r.lines().find_map(|l| l.split("timeout): ").nth(1).filter(|_| l.starts_with("meter P2"))).map(|x| x.to_string()).unwrap_or("-".to_string());
                        // (the head's edges after the freeze: the launches whose offsets differ from
                        // the run's most common ones)
                        let edges = r.lines().find(|l| l.starts_with("head-perm edges vs the summon tick S")).map(|l| {
                            let rows: Vec<(String, String)> = l.split("): ").last().unwrap_or("").split("; ").filter_map(|x| x.split_once(": ").map(|(a, b)| (a.to_string(), b.to_string()))).filter(|(_, b)| b.starts_with("GO")).collect();
                            let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
                            for (_, b) in &rows {
                                *counts.entry(b.as_str()).or_default() += 1;
                            }
                            let mode = counts.iter().max_by_key(|x| x.1).map(|x| x.0.to_string()).unwrap_or_default();
                            let off: Vec<String> = rows.iter().filter(|(_, b)| *b != mode).map(|(a, b)| format!("{a}: {b}")).collect();
                            if off.is_empty() { format!("all {} launches at {mode}", rows.len()) } else { format!("{} of {} off ({})", off.len(), rows.len(), off.join("; ")) }
                        }).unwrap_or("-".to_string());
                        let latency = r.lines().find(|l| l.starts_with("freeze at t ")).map(|l| {
                            let ls: Vec<String> = l.split("detection latency ").skip(1).map(|x| first_num(x)).collect();
                            if ls.is_empty() { "0 (emulated gate)".to_string() } else { format!("{} gt", ls.join("/")) }
                        }).unwrap_or("-".to_string());
                        let vox = find("voxel-stop moves: ").map(|x| first_num(&x)).unwrap_or("-".to_string());
                        let pause = r.lines().find_map(|l| l.split("day-1's gate semantics pending): ").nth(1).filter(|_| l.starts_with("controller pause"))).map(|x| x.replace("controller paused at ", "").replace("the slot clock's phase against gameTime now ", "phase ")).unwrap_or("no pause clock".to_string());
                        let probs = r.lines().find(|l| l.ends_with("problems") || l.ends_with("problems:")).unwrap_or("0 problems").to_string();
                        println!("freeze +{dt} ({s_rel}) for {n}: stand RELs {rels} | holds {holds}, last GO {last_go}, ARR at the end {arr} | merges {merged} | contacts {contacts} | gates shut on a cart {gates} | backward entries {back} | T crossings {t_cross} | curve contact {curve} | carts at the end {carts_end} | P2 {p2} | edges {edges} | detection latency {latency} | pause: {pause} | voxel-stop {vox} | {probs}");
                    }
                }
                return Ok(());
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
        MobfarmCmd::PushScan { design, rest, no_auto, switch, controls } => {
            let (_c, spec) = load(version, &design)?;
            if controls {
                let cl = crate::pushscan::tee_controls();
                println!("rail-T scan controls (a 3-connection T at 0,0,0; each source type in its update reach, then moved out of it): {} of {} pass", cl.iter().filter(|l| l.ends_with("PASS")).count(), cl.len());
                for l in &cl {
                    println!("{l}");
                }
            }
            let mut rests = Vec::new();
            for r in &rest {
                let v: Vec<i32> = r.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--rest x,y,z");
                rests.push(redsim::pos::BlockPos::new(v[0], v[1], v[2]));
            }
            let mut switches = Vec::new();
            for r in &switch {
                let v: Vec<i32> = r.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--switch x,y,z");
                switches.push(redsim::pos::BlockPos::new(v[0], v[1], v[2]));
            }
            let (lines, n, tracks) = crate::pushscan::scan(&spec, &rests, !no_auto);
            let flagged = lines.iter().filter(|l| l.contains("PUSH")).count();
            println!("push-range scan ({n} rest or creep cells against {tracks} tracks; boxes 0.98 x 0.7, the resting one inflated 0.2 in x and z and 0.5 along its axis): {flagged} pairs within push range");
            for l in &lines {
                println!("{l}");
            }
            let (tl, undeclared) = crate::pushscan::tees(&spec, &switches);
            println!("rail-T scan (plain rails with 3+ potential connections; the signal-source updates reaching each (redsim's source set, each type's 26.3 update pattern) must be declared switches; a 4-way crossing is listed, not counted): {} Ts, {undeclared} undeclared sources", tl.len());
            for l in &tl {
                println!("{l}");
            }
            let (sl, su) = crate::pushscan::switched(&spec, &switches);
            println!("switched-cell scan (every 3-connection plain rail, powered or activator rail and trapdoor: every source that can power it, only declared --switch sources allowed, a design's own redstone block listed as fixed; each detector's and its below-block's face neighbours): {su} undeclared");
            for l in &sl {
                println!("{l}");
            }
            let pl = crate::pushscan::placement(&spec);
            println!("placement-order scan (every rail vanilla placement would not give its designed shape with the rest laid, or whose placement re-shapes a laid neighbour; the fix): {} cells", pl.len());
            for l in &pl {
                println!("{l}");
            }
        }
        MobfarmCmd::TileMargins { design, at } => {
            let (_c, spec) = load(version, &design)?;
            let s = crate::tile::sets(&spec).map_err(|e| anyhow::anyhow!(e))?;
            println!(
                "module: {} rider positions, {} waiting riders, summoners {}, {} golem cells (feet y {}), {} zombies, beds {}",
                s.riders.len(),
                s.waiting.len(),
                s.summoners.iter().map(|p| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z)).collect::<Vec<_>>().join(" "),
                s.golems.len(),
                s.golems.iter().map(|g| g.y as i64).min().map_or("-".into(), |a| format!("{a}..{}", s.golems.iter().map(|g| g.y as i64).max().unwrap_or(a))),
                s.zombies.len(),
                s.beds.iter().map(|b| format!("{},{},{}", b.x, b.y, b.z)).collect::<Vec<_>>().join(" ")
            );
            for a in &at {
                let v: Vec<&str> = a.split(',').collect();
                anyhow::ensure!(v.len() == 3 || v.len() == 4, "--at dx,dy,dz[,m]");
                let o = [v[0].trim().parse::<i32>()?, v[1].trim().parse::<i32>()?, v[2].trim().parse::<i32>()?];
                println!("{}", crate::tile::candidate(&s, o, v.len() == 4));
            }
        }
        MobfarmCmd::Envelope { families, q1, q2, villagers } => {
            use crate::envelope::{C, Queue};
            if !villagers.is_empty() {
                let st = crate::envelope::station_m1();
                let al = |p: i32| ((p + 15) / 16) * 16;
                // (golems a day by count, NOT measured here: 51b2aacd's derived table, with
                // tick-exact's re-mark price at 18 (~3-4.5% lost: ~229) and the skip hold at 21
                // (~236); day-1's rows to confirm (printed in the header))
                let gpd = |n: usize| -> f64 { match n { 0..=17 => 0.99 * 40.0 * n as f64 / 3.0, 18 => 229.0, 19..=21 => 236.0, _ => 238.0 } };
                struct Row { n: usize, fam: String, queue: String, nap: String, gpd: f64, vol: i64, blocks: i64, red: i64, area: f64, dims: String, margin: i64 }
                let mut rows: Vec<Row> = Vec::new();
                for &n in &villagers {
                    let b = n / 3;
                    let linger = b.saturating_sub(6);
                    // (the out-of-range group: N - 3, at most 15 (the batches past the floor's 6
                    // linger in range); saturating below 3 villagers)
                    let out_n = n.saturating_sub(3).min(15);
                    for f in &families {
                        if f == "F5" {
                            continue;
                        }
                        let mut qs: Vec<(Queue, String)> = vec![(Queue::Q1(out_n), format!("Q1 {out_n} out of range"))];
                        if n % 3 == 0 && out_n % 3 == 0 {
                            qs.push((Queue::Q2(3, out_n / 3), format!("Q2 3 x {}", out_n / 3)));
                        }
                        for (q, qn) in qs {
                            // (nap options: none below 18 (idle slots absorb the napper); at 18 (a) the
                            // lap-exact re-entry gate (Q1 70 / Q2 75) or (b) the equal-length bypass
                            // (~60 cells at 2 + a keyed gate 20); at 21 the skip hold (in the
                            // controller); at 24 Q2 + Z4 (3 beds) or Q1 skip-free)
                            let naps: Vec<(String, i64, i64, bool)> = match n {
                                0..=17 => vec![("none (idle slots)".into(), 0, 0, false)],
                                18 => vec![
                                    ("(a) lap gate".into(), 0, match q { Queue::Q1(_) => 70, _ => 75 }, false),
                                    // (onechunk's paper price: the return re-laid as a free fall into the
                                    // hold's rear pocket, net ~+15 blocks; the merge sync a 4-stage shift
                                    // register ~37 redstone)
                                    ("(b) equal-length bypass".into(), 15, 37, false),
                                ],
                                19..=21 => vec![
                                    ("(a) lap gate + skip hold".into(), 0, match q { Queue::Q1(_) => 70, _ => 75 }, false),
                                    // (onechunk: 600-gt laps are free, so the bypass costs nothing at 21)
                                    ("(b) none: 600-gt laps free".into(), 0, 0, false),
                                ],
                                _ => match q {
                                    Queue::Q2(..) => vec![("(c) Z4, a bed a lane".into(), 0, 0, true), ("skip-free, 1 bed".into(), 0, 0, false)],
                                    _ => vec![("skip-free, 1 bed".into(), 0, 0, false)],
                                },
                            };
                            for (nap, track, nap_red, z4) in naps {
                                let lay = crate::envelope::layout(f, q, &st, z4).map_err(|e| anyhow::anyhow!(e))?;
                                let (lo, hi) = crate::envelope::bound(&lay.boxes);
                                // (the strict pitch: every rider position, G1's review)
                                let (px, pz) = (crate::envelope::pitch(&lay, &st, 0, true), crate::envelope::pitch(&lay, &st, 2, true));
                                let (_, mper) = crate::envelope::pitch_mirror_z(&lay, &st, true);
                                let area = (al(px) as f64 * al(pz) as f64).min(if mper > 0 { al(px) as f64 * mper as f64 / 2.0 } else { f64::MAX });
                                let (mut tr, _) = crate::envelope::transport_blocks(f, q, &lay, &st);
                                // (the shared lift's height from the layout's lift column (x 1.5): 8H + 30,
                                // + 60 for a Q2 merge and split (7b90c3f1))
                                let lift: Vec<f64> = lay.transit.iter().filter(|p| (p.x - 1.5).abs() < 1e-9).map(|p| p.y).collect();
                                if !lift.is_empty() && f != "F5" {
                                    let h = (lift.iter().cloned().fold(f64::MIN, f64::max) - lift.iter().cloned().fold(f64::MAX, f64::min)).ceil() as i64;
                                    tr = 8 * h + 30 + if let Queue::Q2(..) = q { 60 } else { 0 } + if f == "F3" { 30 } else { 0 } + if f == "F2" { 2 * (lay.transit.len() as i64 - lift.len() as i64) / 3 } else { 0 };
                                }
                                let ctrl = match q { Queue::Q1(_) => 338, Queue::Q2(..) => 195 } + nap_red;
                                let tier_extra = if z4 { 400 - 152 + 3 * 34 * 2 } else { 0 };
                                let blocks = crate::envelope::blocks(out_n + 3 * linger, tr + track, ctrl, 13) + tier_extra;
                                // (the exit's margin: out of range by T + 100 (1 + the lingering summons))
                                // (falls: onechunk's correction, 26 blocks ~52 gt)
                                // (the departing batch's last cart out of range: F1 by the shared lift ~T+71
                                // (7b90c3f1), F2 by its track and lift, F3 by the fall (drag 0.95, ~52 gt))
                                let t_out = match f.as_str() { "F1" => 71, "F2" => 90, _ => 53 };
                                let margin = 100 * (1 + linger as i64) - t_out;
                                rows.push(Row {
                                    n,
                                    fam: lay.family.to_string(),
                                    queue: qn.clone(),
                                    nap,
                                    gpd: gpd(n),
                                    vol: crate::envelope::vol(lo, hi),
                                    blocks,
                                    red: ctrl,
                                    area,
                                    dims: format!("{}x{}x{}", hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1),
                                    margin,
                                });
                            }
                        }
                    }
                }
                // (the Pareto set: fewer villagers, more golems, less volume, fewer blocks, less
                // redstone, less tiling area)
                let dominated = |a: &Row| rows.iter().any(|b| {
                    let le = b.n <= a.n && b.gpd >= a.gpd && b.vol <= a.vol && b.blocks <= a.blocks && b.red <= a.red && b.area <= a.area;
                    let lt = b.n < a.n || b.gpd > a.gpd || b.vol < a.vol || b.blocks < a.blocks || b.red < a.red || b.area < a.area;
                    le && lt
                });
                println!("(the M1 station; every rider position in the bed check and in the pitch (strict); golems a day NOT measured here: 51b2aacd's derived rows, tick-exact's re-mark price at 18, day-1 to confirm; blocks are part-budget estimates)");
                println!("N | golems/day | family | queue | naps | envelope (blocks^3) | blocks (L1) | redstone | tiling area/module | exit margin (gt) | Pareto");
                for r in &rows {
                    println!(
                        "{} | {:.0} | {} | {} | {} | {} ({}) | {} | {} | {:.0} | {} | {}",
                        r.n, r.gpd, r.fam, r.queue, r.nap, r.dims, r.vol, r.blocks, r.red, r.area, r.margin, if dominated(r) { "" } else { "PARETO" }
                    );
                }
                return Ok(());
            }
            let st = crate::envelope::station_m1();
            let cb = crate::envelope::clear_bounds(&st);
            println!(
                "constants: sight {} sideways, {} above / {} below a golem's feet; mark {} / {} (inside the sight box); bed radius {}; margin {}; lift {} b/t; cart {} b/t; bed tier {}x{}x{}; S1 posts {} beside [code: 26.3 GolemSensor / spawnGolemIfNeeded at the worst-case follow range 17.84; derived: bed, posts]",
                C.sight_side, C.sight_above, C.sight_below, C.mark_side, C.mark_vert, C.bed_r, C.margin, C.lift, C.cart, C.tier[0], C.tier[1], C.tier[2], C.posts
            );
            println!(
                "station (the M1 station, dfb89499 a07e7a1b3; file frame): summoners {}; holds (waiting) at 16.19 over them; the pad's extent x 4..14 z 0..5 at feet -6 (its 44 reachable cells inside); blocks x {}..{} y {}..{} z {}..{}; out of range: x <= {:.2} or >= {:.2}, z <= {:.2} or >= {:.2}, feet >= {:.2} or <= {:.2}",
                st.summoners.iter().map(|p| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z)).collect::<Vec<_>>().join(" "),
                st.lo[0], st.hi[0], st.lo[1], st.hi[1], st.lo[2], st.hi[2], cb[0], cb[1], cb[2], cb[3], cb[4], cb[5]
            );
            let mut qs: Vec<Queue> = q1.iter().map(|n| Queue::Q1(*n)).collect();
            for s in &q2 {
                let (l, p) = s.split_once('x').ok_or_else(|| anyhow::anyhow!("--q2 LxP"))?;
                qs.push(Queue::Q2(l.parse()?, p.parse()?));
            }
            for f in &families {
                for q in &qs {
                  for z4 in [false, true] {
                    let lay = crate::envelope::layout(f, *q, &st, z4).map_err(|e| anyhow::anyhow!(e))?;
                    let (lo, hi) = crate::envelope::bound(&lay.boxes);
                    let bind = crate::envelope::binding(&lay);
                    let (px, pz) = (crate::envelope::pitch(&lay, &st, 0, true), crate::envelope::pitch(&lay, &st, 2, true));
                    let (wx, wz) = (crate::envelope::pitch(&lay, &st, 0, false), crate::envelope::pitch(&lay, &st, 2, false));
                    let (mp, mper) = crate::envelope::pitch_mirror_z(&lay, &st, true);
                    println!("  pitch with the waiting riders only (the departing batch lingers by rule): x {wx}, z {wz}; the strict pitch below takes every rider position");
                    for (axis, name) in [(0usize, "x"), (2usize, "z")] {
                        if let Some((rule, m, r, s)) = crate::envelope::pitch_worst(&lay, &st, axis, true, 32) {
                            println!("  the strict pitch's worst pair at {name} 32: {rule} {m:+.2}, the rider at {:.2},{:.2},{:.2} against {:.2},{:.2},{:.2}", r.x, r.y, r.z, s.x, s.y, s.z);
                        }
                    }
                    // (the roof over the highest part, ironspec Nov 12 01:55: bottom slabs whose
                    // underside clears every rider's box (feet + 1.95), so no rider's head is in it)
                    let ytop = lay.riders.iter().chain(lay.transit.iter()).chain(st.holds.iter()).map(|r| r.y).fold(f64::MIN, f64::max);
                    let roof = (ytop + 1.95).ceil() as i32;
                    println!("  the roof: bottom slabs at y {roof} (the highest rider's feet {ytop:.2}, its box's top {:.2} under the slab's underside); the rod at the roof's corner at y {}, the daylight detector's column up to the roof", ytop + 1.95, roof + 1);
                    let al = |p: i32| ((p + 15) / 16) * 16;
                    println!(
                        "{} with {}{}: envelope {} x {} x {} (x {}..{}, y {}..{}, z {}..{}) = {} blocks^3; binding x: {}; y: {}; z: {}; pitch x {} ({} chunk-aligned), z {} ({})",
                        lay.family,
                        match q { Queue::Q1(n) => format!("Q1 {n} waiting"), Queue::Q2(l, p) => format!("Q2 {l} x {p} waiting") },
                        if z4 { " + Z4 (3 beds)" } else { " + 1 bed" },
                        hi[0] - lo[0] + 1,
                        hi[1] - lo[1] + 1,
                        hi[2] - lo[2] + 1,
                        lo[0], hi[0], lo[1], hi[1], lo[2], hi[2],
                        crate::envelope::vol(lo, hi),
                        bind[0], bind[1], bind[2],
                        px, al(px), pz, al(pz)
                    );
                    for (a, b, n) in &lay.boxes {
                        println!("  box {n}: x {}..{} y {}..{} z {}..{}", a[0], b[0], a[1], b[1], a[2], b[2]);
                    }
                    let (lo2, hi2) = crate::envelope::bound(&lay.boxes);
                    let depth = hi2[2] - lo2[2] + 1;
                    println!(
                        "  mirrored in z: the pair's copy at z' = {} - z (gap {}), the next pair {} on: {} modules per {} blocks of z ({:.1} a module), against {} plain",
                        mp,
                        mp - 2 * hi2[2] - 1,
                        mper,
                        2,
                        mper,
                        mper as f64 / 2.0,
                        pz.max(depth)
                    );
                    println!(
                        "  controller (day-1's estimate, outside the envelope: it fills a gap): {} blocks + the liveness ring 192 (retirable with pearl loading, pending)",
                        match q { Queue::Q1(_) => 408, Queue::Q2(..) => 270 }
                    );
                    // (the blocks, L1 and L2: the controller without the re-entry gate at 24
                    // villagers (Q1 408 - 70, Q2 270 - 75); L2 the pearl per chunk with the ring,
                    // ALIVE and counters (~330); L1 the portal-loop loader (~40) shared by the
                    // modules of a 3 x 3 of chunks, no ring; the Nether kill room (124) shared)
                    let n_wait = lay.riders.len();
                    let (tr, trs) = crate::envelope::transport_blocks(f, *q, &lay, &st);
                    let ctrl = match q { Queue::Q1(_) => 338, Queue::Q2(..) => 195 };
                    let area_plain = (al(px) * al(pz)) as f64;
                    let area_mirror = if mper > 0 { al(px) as f64 * mper as f64 / 2.0 } else { area_plain };
                    let per_loader = (2304.0 / area_mirror.min(area_plain)).max(1.0);
                    let tier_extra = if z4 { 400 - 152 + 3 * 34 * 2 } else { 0 };
                    let b_l2 = crate::envelope::blocks(n_wait, tr, ctrl, 330) + tier_extra;
                    let b_l1 = crate::envelope::blocks(n_wait, tr, ctrl, (40.0 / per_loader).round() as i64) + tier_extra;
                    println!(
                        "  blocks (estimates): station 401 + queue {} + bed tier {} + transport {} [{}] + controller {}; L2 (pearl a chunk, ring/ALIVE/counters 330): {}; L1 (portal-loop loader shared by {:.1} modules in a 3x3, no ring): {} (saves {}); golems a day per block (235.7 a module, the shared 124-block Nether kill room over 4 modules): L2 {:.3}, L1 {:.3}; area a module {:.0} (plain {} x {}) or {:.0} (mirrored-z pairs)",
                        3 * n_wait,
                        200 + tier_extra,
                        tr,
                        trs,
                        ctrl,
                        b_l2,
                        per_loader,
                        b_l1,
                        b_l2 - b_l1,
                        235.7 / (b_l2 as f64 + 31.0),
                        235.7 / (b_l1 as f64 + 31.0),
                        area_plain,
                        al(px),
                        al(pz),
                        area_mirror
                    );
                    for n in &lay.notes {
                        println!("  note: {n}");
                    }
                  }
                }
            }
        }
        MobfarmCmd::JunctionCheck { design } => {
            let (_c, spec) = load(version, &design)?;
            let (text, failed) = crate::queue::junction_gate(&spec, &std::fs::read_to_string(&design)?);
            println!("{text}");
            // (ironspec Nov 12 02:46: placement-order shapes are accepted: each T's placement order
            // for the build guide, and the signal-source updates that can reach it)
            let (tl, undeclared) = crate::pushscan::tees(&spec, &[]);
            println!("placement order (a T keeps its placed shape while no signal source's update reaches it; {undeclared} reaching sources, none declared here):");
            for l in &tl {
                println!("{l}");
            }
            if failed {
                std::process::exit(1);
            }
        }
        MobfarmCmd::FleeCheck { design, home, show, foot, clearing, eye_control, pickup_control, sampled, claim, classes, fix_search, fix_pairs, fix_block, path_watch, path_trace, scarer_search, scarer, scarer_rider, scarer_max, scarer_any_floor, scarer_walls, scarer_target, scarer_exact, scarer_check, scarer_transient, scarer_mix, scarer_visor, scarer_exclude, pillager, golem_reach, golem_pad, scarer_near, post_drift, drift_ticks } => {
            if !post_drift.is_empty() {
                anyhow::ensure!(post_drift.len() == 3, "--post-drift x,y,z");
                let (_c, spec) = load(version, &design)?;
                for l in crate::fleecheck::post_drift(&spec, redsim::pos::BlockPos::new(post_drift[0], post_drift[1], post_drift[2]), drift_ticks).map_err(|e| anyhow::anyhow!(e))? {
                    println!("{l}");
                }
                return Ok(());
            }
            let (_c, spec) = load(version, &design)?;
            if !golem_reach.is_empty() {
                anyhow::ensure!(golem_reach.len() == 3 && golem_pad.len() == 6, "--golem-reach x,y,z --golem-pad x0,y0,z0,x1,y1,z1");
                let g = &golem_pad;
                let lo = [g[0].min(g[3]), g[1].min(g[4]), g[2].min(g[5])];
                let hi = [g[0].max(g[3]), g[1].max(g[4]), g[2].max(g[5])];
                let t = redsim::pos::BlockPos::new(golem_reach[0], golem_reach[1], golem_reach[2]);
                for l in crate::fleecheck::golem_reach(&spec, lo, hi, t).map_err(|e| anyhow::anyhow!(e))? {
                    println!("{l}");
                }
                return Ok(());
            }
            let mut pills = Vec::new();
            for r in &pillager {
                let v: Vec<f64> = r.split(',').map(|t| t.trim().parse::<f64>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--pillager x,y,z");
                pills.push(redsim::entity::Vec3::new(v[0], v[1] + crate::fleecheck::PILLAGER_SEAT, v[2]));
            }
            if !scarer_check.is_empty() {
                let mut extra = Vec::new();
                for r in &scarer_rider {
                    let v: Vec<f64> = r.split(',').map(|t| t.trim().parse::<f64>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--scarer-rider x,y,z");
                    extra.push(redsim::entity::Vec3::new(v[0], v[1], v[2]));
                }
                let mut posts = Vec::new();
                for p in &scarer_check {
                    let t: Vec<&str> = p.split(',').map(|t| t.trim()).collect();
                    anyhow::ensure!(t.len() == 4 && (t[3] == "pillager" || t[3] == "zombie"), "--scarer-check x,y,z,pillager|zombie");
                    posts.push((redsim::pos::BlockPos::new(t[0].parse()?, t[1].parse()?, t[2].parse()?), t[3] == "pillager"));
                }
                let gp = if golem_pad.len() == 6 { Some(([golem_pad[0].min(golem_pad[3]), golem_pad[1].min(golem_pad[4]), golem_pad[2].min(golem_pad[5])], [golem_pad[0].max(golem_pad[3]), golem_pad[1].max(golem_pad[4]), golem_pad[2].max(golem_pad[5])])) } else { None };
                for l in crate::fleecheck::scarer_check(&spec, &extra, &posts, &scarer_transient, gp).map_err(|e| anyhow::anyhow!(e))? {
                    println!("{l}");
                }
                return Ok(());
            }
            if !scarer_search.is_empty() {
                anyhow::ensure!(scarer_search.len() == 6, "--scarer-search x0,y0,z0,x1,y1,z1");
                anyhow::ensure!(scarer == "zombie" || scarer == "pillager", "--scarer zombie|pillager");
                let mut extra = Vec::new();
                for r in &scarer_rider {
                    let v: Vec<f64> = r.split(',').map(|t| t.trim().parse::<f64>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 3, "--scarer-rider x,y,z");
                    extra.push(redsim::entity::Vec3::new(v[0], v[1], v[2]));
                }
                let s = &scarer_search;
                let lo = [s[0].min(s[3]), s[1].min(s[4]), s[2].min(s[5])];
                let hi = [s[0].max(s[3]), s[1].max(s[4]), s[2].max(s[5])];
                let mut excl = Vec::new();
                for e in &scarer_exclude {
                    let v: Vec<i32> = e.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 6, "--scarer-exclude x0,y0,z0,x1,y1,z1");
                    excl.push(([v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5])], [v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])]));
                }
                let gp = if golem_pad.len() == 6 { Some(([golem_pad[0].min(golem_pad[3]), golem_pad[1].min(golem_pad[4]), golem_pad[2].min(golem_pad[5])], [golem_pad[0].max(golem_pad[3]), golem_pad[1].max(golem_pad[4]), golem_pad[2].max(golem_pad[5])])) } else { None };
                for l in crate::fleecheck::scarer_search(&spec, &extra, lo, hi, &scarer, scarer_max, scarer_any_floor, &excl, scarer_near, scarer_walls, &scarer_target, scarer_exact, &scarer_transient, scarer_mix, gp, scarer_visor).map_err(|e| anyhow::anyhow!(e))? {
                    println!("{l}");
                }
                return Ok(());
            }
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
            let claims: Vec<redsim::pos::BlockPos> = claim
                .iter()
                .map(|s| {
                    let v: Vec<i32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    if v.len() == 3 { Ok(redsim::pos::BlockPos::new(v[0], v[1], v[2])) } else { Err(anyhow::anyhow!("--claim x,y,z")) }
                })
                .collect::<Result<_, _>>()?;
            let places = crate::fleecheck::check_pillagers(&spec, bed, &foot, &panic_boxes, !sampled, &claims, &pills).map_err(|e| anyhow::anyhow!(e))?;
            if !claims.is_empty() {
                println!("POI tickets taken first: {}", claims.iter().map(|c| format!("{},{},{}", c.x, c.y, c.z)).collect::<Vec<_>>().join(" "));
            }
            for l in crate::fleecheck::stamp_classes(&places, classes) {
                println!("{l}");
            }
            if let Some(pw) = &path_watch {
                let path = crate::fleecheck::read_watch_path(&std::fs::read_to_string(pw)?);
                let roles = crate::fleecheck::stamp_path_roles(&places);
                let runs = crate::fleecheck::stamp_path(&roles, &path);
                let worst = runs.iter().map(|r| r.cleared.map_or(path.len() as i64 - r.t0, |(t, _)| t - r.t0)).max().unwrap_or(0);
                println!(
                    "path-integrated stamp age (DIAGNOSTIC: e6 positive control not met; no verdict: a bar needs a runtime runner's stamp; {} ticks of the rider's path; arms: a panicking cell whose draws no path reaches, clears: a panicking cell with a reachable draw; calm cells neither): {} stamps, the worst {} gt",
                    path.len(),
                    runs.len(),
                    worst
                );
                let label = |t: i64| path.get(t as usize).map_or(String::new(), |p| p.2.replace(' ', ""));
                for r in &runs {
                    match r.cleared {
                        Some((t, c)) => println!("  STAMP-AGE (DIAGNOSTIC) armed {} at {},{},{} ({}), cleared {} at {},{},{}: {} gt", label(r.t0), r.cell0.x, r.cell0.y, r.cell0.z, r.src, label(t), c.x, c.y, c.z, t - r.t0),
                        None => println!("  STAMP-AGE (DIAGNOSTIC) armed {} at {},{},{} ({}), still armed at the path's end: {} gt or more", label(r.t0), r.cell0.x, r.cell0.y, r.cell0.z, r.src, path.len() as i64 - r.t0),
                    }
                }
                if let Some(pt) = &path_trace {
                    let (a, b) = pt.split_once(',').ok_or_else(|| anyhow::anyhow!("--path-trace c287+0,c297+40"))?;
                    let ta = path.iter().position(|p| p.2.replace(' ', "") == a).unwrap_or(0);
                    let tb = path.iter().position(|p| p.2.replace(' ', "") == b).unwrap_or(path.len().saturating_sub(1));
                    let mut last: Option<redsim::pos::BlockPos> = None;
                    for p in &path[ta..=tb.min(path.len() - 1)] {
                        let Some((cell, role)) = crate::fleecheck::path_cell(&roles, p.1) else { continue };
                        if last == Some(cell) {
                            continue;
                        }
                        last = Some(cell);
                        let pl = places.iter().find(|q| q.cell == cell && !q.on_foot);
                        let shares = pl.map_or(String::new(), |q| {
                            format!(
                                "panic stroll {} cells reached {:.1}%, flee {} cells reached {:.1}% (hostiles within 6: {}, seen within 8: {})",
                                q.panic_stroll.cells,
                                100.0 * q.panic_stroll.reached_share,
                                if q.flee_from > 0 { q.flee.cells } else { 0 },
                                if q.flee_from > 0 { 100.0 * q.flee.reached_share } else { 0.0 },
                                q.flee_from,
                                q.seen
                            )
                        });
                        println!("  PATH {} {:.2},{:.2},{:.2} cell {},{},{}: {}; {}", p.2.replace(' ', ""), p.1.x, p.1.y, p.1.z, cell.x, cell.y, cell.z, if role.0 { "ARMS" } else if role.1 { "clears" } else { "neither" }, shares);
                    }
                }
            }
            for s in &fix_search {
                let v: Vec<i32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                anyhow::ensure!(v.len() == 3, "--fix-search x,y,z");
                let at = redsim::pos::BlockPos::new(v[0], v[1], v[2]);
                let fixes = crate::fleecheck::clearing_fixes(&spec, bed, &panic_boxes, &claims, at, &fix_block).map_err(|e| anyhow::anyhow!(e))?;
                let short = fix_block.trim_start_matches("minecraft:").to_string();
                println!(
                    "clearing cells for {},{},{} (a {short} floor within 2 sideways and 2 up or down; the class it gives): {}",
                    at.x,
                    at.y,
                    at.z,
                    if fixes.is_empty() { "none".to_string() } else { fixes.iter().map(|(f, k, ch)| format!("{short} {},{},{} -> {k} (the full re-check: {})", f.x, f.y, f.z, if ch.is_empty() { "no other place's class or seen changes".to_string() } else { ch.join(", ") })).collect::<Vec<_>>().join("; ") }
                );
                if fix_pairs {
                    let (n, pairs) = crate::fleecheck::clearing_fix_pairs(&spec, bed, &panic_boxes, &claims, at, &fix_block).map_err(|e| anyhow::anyhow!(e))?;
                    println!(
                        "clearing pairs for {},{},{} ({n} candidate floors, every pair; the class it gives): {}",
                        at.x,
                        at.y,
                        at.z,
                        if pairs.is_empty() { "none".to_string() } else { pairs.iter().map(|(a, b, k, ch)| format!("{short} {},{},{} + {},{},{} -> {k} (the full re-check: {})", a.x, a.y, a.z, b.x, b.y, b.z, if ch.is_empty() { "no other place's class or seen changes".to_string() } else { ch.join(", ") })).collect::<Vec<_>>().join("; ") }
                    );
                }
            }
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
        MobfarmCmd::Borders { design, offset, depth } => {
            let d = mctas::design::Design::parse(&std::fs::read_to_string(&design)?)?;
            let off = match offset.as_slice() { [x, z] => [*x, *z], [] => [0, 0], _ => anyhow::bail!("--offset takes X,Z") };
            println!("{}", crate::borders::report(&d.blocks, off, depth));
            println!("{}", crate::borders::peaks_line(&d.blocks).0);
        }
        MobfarmCmd::Trim { design, out, threads, days, runs } => crate::trim::trim_cmd(&crate::trim::TrimArgs { design, out, threads, days, runs })?,
        MobfarmCmd::Climb { design } => crate::trim::climb_cmd(&design)?,
        MobfarmCmd::Bury { design, out, cover, seeds, ticks } => {
            let (c, spec) = load(version, &design)?;
            let (cells, fluids) = crate::bury::envelope(&spec, &c.clock.design, seeds, ticks)?;
            let (d, removed, opened) = crate::bury::bury(&c.clock.design, &cells, &fluids, cover);
            std::fs::write(&out, d.to_text())?;
            println!("{}: {} open cells ({} dug open beyond its blocks), {} containment blocks left to the rock; ground layer {}", out.display(), cells.len(), opened, removed, d.meta.get("ground").cloned().unwrap_or_default());
        }
        MobfarmCmd::Bill { design, stage, terrain, components, part } => {
            if components {
                // (the design's own blocks, as written: a part file without ports or a farm spec
                // counts too; its Nether part, the `nether layer Y:` sections, in its own column
                // and in the whole)
                let d = mctas::design::Design::parse(&std::fs::read_to_string(&design)?)?;
                let blocks: Vec<([i32; 3], String)> = d.blocks.iter().map(|(p, k)| (*p, k.clone())).collect();
                let nether: Vec<([i32; 3], String)> = d.nether_blocks.iter().map(|(p, k)| (*p, k.clone())).collect();
                let parts = part.iter().map(|t| crate::components::parse_part(t)).collect::<Result<Vec<_>, _>>().map_err(|e| anyhow::anyhow!(e))?;
                if nether.is_empty() {
                    println!("{}: {} cells (no Nether part)", design.display(), blocks.len());
                } else {
                    println!("{}: {} cells ({} overworld + {} in its Nether part)", design.display(), blocks.len() + nether.len(), blocks.len(), nether.len());
                }
                print!("{}", crate::components::table(&blocks, &nether, &parts));
                return Ok(());
            }
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
        MobfarmCmd::Bedtier { out, bench } => {
            std::fs::write(&out, crate::bedtier::design(bench)?)?;
            println!("wrote {}", out.display());
        }
        MobfarmCmd::BedtierBench { design, from, to, speeds, after_sleep, release_after, day, empty, no_home, hide_after_wake, freeze, reload, hour, reload_phases, freeze_phases, hours, sweep_len } => {
            let (_c, spec) = load(version, &design)?;
            let faults = crate::bedtier::Faults { empty, no_home, hide_after_wake, freeze: crate::bedtier::start_len(freeze.as_deref())?, reload: crate::bedtier::start_len(reload.as_deref())?, hour };
            crate::bedtier::bench_cmd(&spec, from, to, &speeds, after_sleep, release_after, day, &faults, [reload_phases.as_deref(), freeze_phases.as_deref(), hours.as_deref()], sweep_len)?;
        }
        MobfarmCmd::BedtierTwo { design, from, to, speeds, restart_phases, transit, lift_delay, lift_speed } => {
            let (_c, spec) = load(version, &design)?;
            crate::bedtier::two_cmd(&spec, from, to, &speeds, restart_phases.as_deref(), transit, lift_delay, lift_speed)?;
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

/// The loader benches' `--freeze T,D[,ow|nether|both]` (redsim's level-32 freeze): (from, len,
/// world bits: 1 the overworld, 2 the Nether).
fn parse_freezes(freeze: &[String]) -> Result<Vec<(i64, i64, u8)>> {
    freeze
        .iter()
        .map(|f| {
            let p: Vec<&str> = f.split(',').map(|s| s.trim()).collect();
            anyhow::ensure!(p.len() == 2 || p.len() == 3, "--freeze T,D[,ow|nether|both]");
            let w = match p.get(2).copied().unwrap_or("both") {
                "ow" => 1u8,
                "nether" => 2,
                "both" => 3,
                o => anyhow::bail!("--freeze world {o}: ow, nether or both"),
            };
            Ok((p[0].parse()?, p[1].parse()?, w))
        })
        .collect()
}
