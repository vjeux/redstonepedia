//! Contraption study 5, track memory (docs/circuits.md): `mctas circuit memory <args...>`. OWNED BY
//! THE TRACK (the tff explorer runs both).
//!
//! * [`specs`]: census specs for rs-latch, d-latch and counter N mirroring the judge's stimuli, run
//!   on track tff's census engine (`circuit_tff::census`).
//! * [`ring`]: dropper-ring counters around the input cell (`circuit memory ring --n N`).

pub mod compose;
pub mod items;
pub mod ring;
pub mod counter8;
pub mod loopbench;
pub mod loopgen;
pub mod loopmodel;
pub mod releasegen;
pub mod sidinggen;
pub mod autoroute;
pub mod releasebench;
pub mod droptest;
pub mod schedbench;
pub mod schedgen;
pub mod schedmodel;
pub mod seqbench;
pub mod specs;
pub mod stamp;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit memory", about = "Memory circuits (track memory): census for RS / D latches and counters")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The item counter: two droppers facing each other hold N items, each pulse fires only the
    /// one the copper bulb phase does not hold, so each runs empty once every N pulses. 39 blocks
    /// for every N up to 576 - the only counter whose size does not grow with N.
    Items {
        /// Counter sizes, comma-separated (2..=576).
        #[arg(long, value_delimiter = ',')]
        n: Vec<usize>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Dropper-ring counters: the smallest cycles dropper -> hopper -> ... whose droppers the
    /// input cell fires (directly or through dust next to it), one design per ring (`--keep`).
    Ring {
        /// Counter sizes, comma-separated.
        #[arg(long, value_delimiter = ',')]
        n: Vec<usize>,
        /// Most dust cells next to the input.
        #[arg(long, default_value_t = 2)]
        max_dust: usize,
        /// Designs kept per N, smallest first.
        #[arg(long, default_value_t = 3)]
        keep: usize,
        /// Copper bulb T flip-flops chained after the comparator (counter N * 2^bulbs).
        #[arg(long, default_value_t = 0)]
        bulbs: usize,
        /// More rings, each around a stone the previous ring's comparator powers (comma-separated
        /// sizes: counter N * THEN1 * THEN2 ...).
        #[arg(long, value_delimiter = ',')]
        then: Vec<usize>,
        /// With --bulbs: judge as a divider (the last comparator toggles once every N * 2^(K-1)
        /// pulses) instead of a level counter N * 2^K.
        #[arg(long)]
        divider: bool,
        /// Write the smallest candidates without judging them (by default each is judged in
        /// redsim, smallest first, and only passing ones are written).
        #[arg(long)]
        unchecked: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run a design under this track's census spec for its kind (`circuit:` header) and print the
    /// output's rest state, change frames, samples and the spec's verdict (debugging the census).
    Check { design: PathBuf, #[arg(long, default_value_t = 40)] gap: u32 },
    /// One design from several placed at offsets (and turned): `PATH@DX,DY,DZ[,TURNS][=PREFIX]`
    /// each; a cell two modules fill must hold the same block; ports renamed with the prefix
    /// (`input` -> `input.PREFIX`, `output.n` -> `output.PREFIX-n`) or dropped (`=-`).
    Compose {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "composed")]
        name: String,
        parts: Vec<String>,
    },
    /// The carousel's nightly scheduler core, generated (crates/circuit-memory/src/schedgen.rs):
    /// the head-pointer ring, the 7 lane cells and their OR bus, the visit edge, the hump window,
    /// STEP, the level ring.
    SchedGen {
        #[arg(long)]
        out: PathBuf,
        /// The module designs (vcell.txt, vedge-c2.txt, fedge.txt, l-ring.txt).
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-scheduler")]
        modules: PathBuf,
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-scheduler/offdelay-k3.txt")]
        offdelay: PathBuf,
    },
    /// The single-lane loop scheduler's bench on redsim (crates/circuit-memory/src/loopbench.rs).
    LoopBench {
        design: PathBuf,
        #[arg(long, default_value_t = 60)]
        days: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.01)]
        miss: f64,
        #[arg(long, default_value_t = 0.3)]
        burst: f64,
        #[arg(long, default_value_t = 13_670)]
        dusk: u64,
        #[arg(long, default_value_t = 22_330)]
        end: u64,
        #[arg(long, default_value_t = 0)]
        game: u32,
        /// The night window: `ideal`, or a normal daylight detector reading 0 under `clear`,
        /// `rain`, `thunder` or `cycle` (random weather spells).
        #[arg(long, default_value = "ideal")]
        weather: String,
        /// Repeat the run with this many chunk reloads at random ticks: the outputs must match.
        #[arg(long, default_value_t = 0)]
        restarts: u32,
        /// Place the design raw: written states and block entities, no shape or neighbour
        /// updates, no settle ("works as placed").
        #[arg(long)]
        placed_raw: bool,
    },
    /// The bed siding's control module (crates/circuit-memory/src/sidinggen.rs; WIP).
    SidingCtlGen {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-scheduler/carousel-loop-scheduler-v0.txt")]
        sched: PathBuf,
    },
    /// Print a design's non-air cells in a box, one per line (`x y z state`), z, then y, then x.
    Cells {
        file: PathBuf,
        /// X0,Y0,Z0,X1,Y1,Z1 (inclusive)
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        r#box: Vec<i32>,
    },
    /// A box of a design as one ASCII map per y layer (x across, z down): `.` air, `#` stone,
    /// `g` glass, `+` dust, `>` `<` `^` `v` a repeater's output side, `C` comparator, `D` dropper,
    /// `H` hopper, `B` copper bulb, `R` redstone block, `T` torch, `=` rail, `o` other; `*` the
    /// cells listed by `--mark` (to see free channels for a route).
    Slice {
        file: PathBuf,
        /// X0,Y0,Z0,X1,Y1,Z1 (inclusive)
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        r#box: Vec<i32>,
    },
    /// The bed siding's rejoin as a side drop into an empty queue slot (crates/circuit-memory/src/droptest.rs).
    DropTest {
        #[arg(long, default_value_t = 0.2)]
        speed: f64,
        #[arg(long)]
        occupied: bool,
        #[arg(long, default_value_t = 60)]
        ticks: u32,
        #[arg(long)]
        no_stop: bool,
        /// The cart waits at rest on a braked powered rail 1 cell from the end, released at this tick.
        #[arg(long)]
        staged: Option<u32>,
    },
    /// Golem pulses at the given ticks into a release controller design; every output port's on
    /// intervals relative to the first pulse (the loop-wired probe file: each control).
    ReleaseTrace {
        design: PathBuf,
        #[arg(long, value_delimiter = ',', default_value = "200")]
        at: Vec<u64>,
        /// Ticks the loop's detector rail (-3,1,4) switches on (a redstone block for 4 gt).
        #[arg(long, value_delimiter = ',')]
        det: Vec<u64>,
        #[arg(long, default_value_t = 600)]
        ticks: u32,
        /// Fault injection (ironspec's self-recovery rule): `x,y,z@T+N=BLOCK`, that cell replaced by
        /// BLOCK from tick T for N ticks, then put back as it was (repeatable). A redstone block on
        /// a line is an extra pulse; glass in a line's cell, a missed one.
        #[arg(long)]
        fault: Vec<String>,
        /// More cells to print like the output ports (their strongest neighbour signal's on/off
        /// changes), `x,y,z` (repeatable): a ring's top dust, a latch.
        #[arg(long, allow_hyphen_values = true)]
        watch: Vec<String>,
    },
    /// The self-recovery survey (ironspec): a fault-free run and each scenario of a spec file (a
    /// fault per line, missed and extra pulses in each state element), with the cycles each takes
    /// to run as the fault-free run again (shifted by whole releases), or NEVER, and the visits,
    /// kicks and F pulses gained or lost (crates/circuit-memory/src/releasebench.rs).
    FaultSurvey {
        design: PathBuf,
        #[arg(long)]
        spec: PathBuf,
        /// Golem pulses, every 100 gt from 100.
        #[arg(long, default_value_t = 60)]
        pulses: u64,
    },
    /// One Rust command for a sweep or a survey (the parent 22:23: never a shell loop): the wired
    /// farm generated once, or once per value of a generator knob (`--knob MCTAS_K2_KICK`), then
    /// `--run` once per value (`{design}` the generated file, `{value}` the value), printing each
    /// value's problems and visits lines and the lines holding a `--show` string
    /// (crates/circuit-memory/src/releasebench.rs).
    ReleaseSweep {
        #[arg(long = "loop")]
        loop_file: PathBuf,
        #[arg(long, default_value = "designs/circuits/pulsemult/archive/off-delay-ring-L20.txt")]
        offdelay: PathBuf,
        #[arg(long)]
        siding: Option<PathBuf>,
        #[arg(long)]
        siding_ctl: Option<PathBuf>,
        #[arg(long)]
        pad: Option<PathBuf>,
        /// The generator's environment knob set to each value (none: the design generated once).
        #[arg(long)]
        knob: Option<String>,
        /// The values, `v1|v2|...` (a value may hold commas).
        #[arg(long, allow_hyphen_values = true)]
        values: String,
        /// The runner's command line (split on spaces, no shell): `{design}`, `{value}`.
        #[arg(long, allow_hyphen_values = true)]
        run: String,
        /// More output lines to print per value: those holding this string (repeatable).
        #[arg(long, allow_hyphen_values = true)]
        show: Vec<String>,
    },
    /// Two `release-trace` outputs compared port by port: identical, every edge shifted by one
    /// amount, or the first differing edge (crates/circuit-memory/src/releasebench.rs).
    TraceDiff { a: PathBuf, b: PathBuf },
    /// The shared bed's nearest rider (ironspec 2026-11-11: a calm villager within 16 of an
    /// occupied bed loses HOME for good): the least distance from a bed block's centre to a
    /// rider's place on a rail inside the riders' box (the loop's: its carts carry the riders
    /// while the bed is taken; the junctions carry only the napper itself, visits 400 gt apart)
    /// (the rail cell's centre, 0.35 up), and where (crates/circuit-memory/src/releasebench.rs).
    BedDistance {
        design: PathBuf,
        /// The riders' box, x0,y0,z0,x1,y1,z1 (design frame): rails inside it count.
        #[arg(long, allow_hyphen_values = true, default_value = "-6,-4,-7,9,6,53")]
        riders: String,
    },
    /// Each design's pricing line for the fold (the parent and ironspec 2026-11-10: "compact:
    /// volume decides", dig depth priced in build minutes): blocks, box, volume, the box's depth
    /// under the ground layer and its dug cells (footprint x depth, as the judge's site footprint
    /// reads it), the dig's minutes, the void bill's build minutes and its mid-air share
    /// (crates/circuit-memory/src/releasebench.rs).
    Price {
        designs: Vec<PathBuf>,
        /// The ground layer (design y): the box's cells under it are dug.
        #[arg(long, default_value_t = -1, allow_hyphen_values = true)]
        ground: i32,
        #[arg(long, default_value = "middle")]
        stage: String,
    },
    /// Edit a design by coordinates instead of by hand in its palette grid: move blocks
    /// (`--move-block X,Y,Z:X2,Y2,Z2`, its block entity with it) and entities (`--move-entity
    /// X,Y,Z:X2,Y2,Z2`, the entity within 0.01 of the first point, its passengers with it); a move
    /// onto an occupied cell, or of a missing block or entity, fails. Written to `--out`.
    DesignEdit {
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long = "move-block", allow_hyphen_values = true)]
        move_block: Vec<String>,
        #[arg(long = "move-entity", allow_hyphen_values = true)]
        move_entity: Vec<String>,
    },
    /// The night-1 lines from a `queue-loop-run --visits-observe` log (one seed): each villager's
    /// first nap, each batch's first fully napped slot, and the startup's slot cost (stand slots
    /// whose batch had a rider not yet napped) (crates/circuit-memory/src/releasebench.rs).
    NapReport { log: PathBuf },
    /// The release controller driven alone (crates/circuit-memory/src/releasebench.rs): golems in
    /// time, early (held), missed (fallback) and doubled (merged), checked against an exact model.
    ReleaseBench {
        design: PathBuf,
        #[arg(long, default_value_t = 1000)]
        pulses: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.05)]
        miss: f64,
        #[arg(long, default_value_t = 0.05)]
        early: f64,
        #[arg(long, default_value_t = 0.1)]
        double: f64,
        #[arg(long, default_value_t = 0)]
        restarts: u32,
        /// Replay the first N ticks in the real game (the shared server) and compare the outputs.
        #[arg(long, default_value_t = 0)]
        game: u32,
        /// Quarter turns the same run is repeated on (comma-separated, e.g. 1,2,3).
        #[arg(long, value_delimiter = ',')]
        turns: Vec<u8>,
        /// Place the design raw: written states and block entities, no shape or neighbour
        /// updates, no settle ("works as placed").
        #[arg(long)]
        placed_raw: bool,
    },
    /// The single-lane loop's release controller, generated (crates/circuit-memory/src/releasegen.rs).
    ReleaseGen {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "designs/circuits/pulsemult/archive/off-delay-ring-L20.txt")]
        offdelay: PathBuf,
        /// Build it into this loop file (ironfarm's carousel-single-lane-loop-wip.txt), wired to
        /// its 21 controls.
        #[arg(long = "loop")]
        loop_file: Option<PathBuf>,
        /// (with `--loop` on farmopt's pocket-stand loop) Paste this bed siding (farmopt's
        /// carousel-siding-wired.txt) and lay its junctions: the corner diverter, the entry and exit
        /// routes, T1/T2 and the three side drops (sidinggen.rs; four bench controls for now).
        #[arg(long)]
        siding: Option<PathBuf>,
        /// (with `--siding`) Wire the junctions' four controls from this scheduler design (the
        /// siding control module: sidinggen.rs `wire_control`) instead of leaving them bench cells.
        #[arg(long)]
        siding_ctl: Option<PathBuf>,
        /// (with `--siding-ctl`) Paste this portal pad (farmopt's carousel-station-pad.txt) at the
        /// loop's origin and wire its golem pulse into the controller (no bench pulse).
        #[arg(long)]
        pad: Option<PathBuf>,
    },
    /// The single-lane loop's nightly scheduler, generated (crates/circuit-memory/src/loopgen.rs).
    LoopGen {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-scheduler")]
        modules: PathBuf,
        #[arg(long, default_value = "designs/circuits/pulsemult/archive/off-delay-2stage-k1.txt")]
        offdelay: PathBuf,
    },
    /// The single-lane loop's sleep scheduling, modeled (crates/circuit-memory/src/loopmodel.rs).
    LoopModel {
        #[arg(long, default_value = "every")]
        policy: String,
        #[arg(long, default_value_t = 60)]
        days: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.01)]
        miss: f64,
        #[arg(long, default_value_t = 4)]
        every: u32,
        #[arg(long, default_value_t = 13_670)]
        dusk: u64,
        #[arg(long, default_value_t = 22_330)]
        end: u64,
        /// The visit's length (the cart leaves the coming-back batch until it rejoins), `A..B` gt.
        #[arg(long, default_value = "400..430")]
        visit: String,
        /// The nap after the visit starts, `A..B` gt.
        #[arg(long, default_value = "115..135")]
        nap: String,
        /// `tail` (rejoins at the queue's tail) or `place` (in its batch's place).
        #[arg(long, default_value = "tail")]
        rejoin: String,
        /// A batch's way back from the station to the tail, gt.
        #[arg(long, default_value_t = 60)]
        out: u64,
        #[arg(long)]
        verbose: bool,
    },
    /// Carousel counter v8 generated from v7 (crates/circuit-memory/src/counter8.rs): all 7 lines
    /// out of the south face at the paired-lane frame's wiring columns, two heights.
    CounterV8 {
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-counter/carousel-counter-v7.txt")]
        v7: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// The nightly scheduler's bench on redsim (crates/circuit-memory/src/schedbench.rs): the
    /// counter's lines driven ideal through days and nights with missed slots, the night window,
    /// visits read from the design, every slot's willingness; per villager its naps, nap
    /// day-time range and worst gap window.
    SchedBench {
        design: PathBuf,
        #[arg(long, default_value_t = 60)]
        days: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.01)]
        miss: f64,
        #[arg(long, default_value_t = 0.3)]
        burst: f64,
        #[arg(long, default_value_t = 13_670)]
        dusk: u64,
        #[arg(long, default_value_t = 22_330)]
        end: u64,
        /// Replay the run's first N days in the real game, every output probe compared every tick.
        #[arg(long, default_value_t = 0)]
        game: u32,
        /// Print every change of the lines and outputs over `FROM..TO` (ticks of the run).
        #[arg(long)]
        watch: Option<String>,
        /// Extra cells `x,y,z x,y,z ...` whose signal --watch prints.
        #[arg(long, default_value = "")]
        probes: String,
    },
    /// The sleep routine's nightly scheduler, its policy judged on timing alone (no circuit): the
    /// counter's advances with misses, a visit per dispatch (nap, length, one bed), every slot's
    /// willingness (gameTime - LAST_SLEPT < 24,000); per villager its naps a night, nap day-time
    /// range, worst gap window and slots lost (crates/circuit-memory/src/schedmodel.rs).
    SchedModel {
        /// dusk21, crt, round or urgent.
        #[arg(long, default_value = "round")]
        policy: String,
        #[arg(long, default_value_t = 60)]
        days: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.05)]
        miss: f64,
        #[arg(long, default_value_t = 0.3)]
        burst: f64,
        /// A dispatch every this many advances.
        #[arg(long, default_value_t = 5)]
        every: u32,
        #[arg(long, default_value_t = 12_000)]
        dusk: u64,
        #[arg(long, default_value_t = 24_010)]
        dawn: u64,
        /// The nap's delay after the dispatch, `MIN..MAX` gt.
        #[arg(long, default_value = "135..185")]
        nap: String,
        /// The visit's length, `MIN..MAX` gt.
        #[arg(long, default_value = "291..486")]
        visit: String,
        #[arg(long, default_value_t = 40)]
        ready: u64,
        /// Step the dispatch phase on every advance, day and night.
        #[arg(long)]
        phase_always: bool,
        /// `block`: a block starting within this many gt of dusk runs 22 visits and moves the head.
        #[arg(long, default_value_t = 200)]
        rotate: u64,
        /// `pring`: the window's last day time for a dispatch.
        #[arg(long, default_value_t = 22_965)]
        pring_end: u64,
        /// The yard queues a napper dispatched while the bed is busy.
        #[arg(long)]
        queue: bool,
        /// `pring`: hold a dispatch while the yard is busy (the pointer stays).
        #[arg(long)]
        hold: bool,
        /// The share of visits whose nap (and the yard's idle) slips 40..200 gt (the HOME re-take).
        #[arg(long, default_value_t = 0.0)]
        retake: f64,
        /// The share of visits drawn from the tail [--visit-body, the longest] (0: uniform).
        #[arg(long, default_value_t = 0.0)]
        visit_tail: f64,
        #[arg(long, default_value_t = 330)]
        visit_body: u32,
        #[arg(long)]
        verbose: bool,
    },
    /// The carousel counter's bench (designs/briefs/farm-iron-carousel.md, part 1): golem pulses
    /// on the 100-gt slot grid from a reactive farm (missed slots, early and late pulses), the 7
    /// lines read every tick (one advance per pulse within --tc, the cycle order kept, the fallback
    /// at 100 + m), then the same run with chunk reloads at random ticks, which must match it.
    CounterBench {
        design: PathBuf,
        #[arg(long, default_value_t = 100_000)]
        pulses: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 20)]
        restarts: usize,
        /// Golem pulse lengths `MIN..MAX` (gt).
        #[arg(long, default_value = "2..15")]
        width: String,
        #[arg(long, default_value_t = 5)]
        jitter: u32,
        #[arg(long, default_value_t = 0.05)]
        miss: f64,
        #[arg(long, default_value_t = 0.3)]
        burst: f64,
        #[arg(long, default_value_t = 40)]
        ready: u32,
        #[arg(long, default_value_t = 40)]
        tc: u32,
        /// The fallback's m `MIN..MAX` (gt past 100).
        #[arg(long, default_value = "6..10")]
        m: String,
        #[arg(long)]
        dump: Option<PathBuf>,
        #[arg(long, default_value_t = 20_000)]
        dump_ticks: u32,
        /// Write a copy of the design carrying the base run's input as its `stimulus:` line: `mctas
        /// circuit trace-compare <copy>` is the real game's check of this very run.
        #[arg(long)]
        stimulus_out: Option<PathBuf>,
        /// Quarter turns the same run is repeated on, comma-separated (each must give the same
        /// advances tick for tick); empty for none.
        #[arg(long, default_value = "1,2,3")]
        turns: String,
    },
    /// Stamp the tunable-group headers (`group:`, `group title:`, `group note:`) on counter
    /// designs by their name: bulb-chain, dropper-ring, ring-bulbs, ring-chain, ring-divider (the
    /// catalog shows each group as one card). The ring generator stamps its own designs.
    StampGroups {
        #[arg(long)]
        dry_run: bool,
        files: Vec<PathBuf>,
    },
    /// Census: every filling of k cells of a box (x 0..A, y 0..B, z 0..C) with palette states,
    /// the named inputs redstone blocks in air cells (`--inputs 'r=-1,0,0;s=3,0,0'`), judged on
    /// redsim with the kind's stimulus (`rs-latch`, `d-latch`, `counter N`); designs + summary.tsv.
    Census {
        /// `rs-latch`, `d-latch` or `counter N`.
        #[arg(long)]
        kind: String,
        #[arg(long = "box")]
        dims: String,
        /// `name=x,y,z;...` in the kind's input names (r, s / d, e / one input for counters).
        #[arg(long)]
        inputs: String,
        #[arg(long, default_value = "1..3")]
        k: String,
        #[arg(long, default_value = "stone,dust,torch,repeater1,comparator")]
        palette: String,
        #[arg(long, default_value_t = 40)]
        gap: u32,
        #[arg(long, default_value_t = 160)]
        jobs: usize,
        /// Stone under every placed dust, diode and standing torch (counted): search one layer
        /// of components with `--box Ax1xC` placed at y=1.
        #[arg(long)]
        auto_floor: bool,
        #[arg(long)]
        out: PathBuf,
    },
}

/// `mctas circuit memory <args...>`.
pub fn run(args: Vec<String>) -> Result<()> {
    let a = match Args::try_parse_from(std::iter::once("mctas circuit memory".to_string()).chain(args)) {
        Ok(a) => a,
        Err(e) if matches!(e.kind(), clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion) => {
            print!("{e}");
            return Ok(());
        }
        Err(e) => bail!("{e}"),
    };
    match a.cmd {
        Cmd::Items { n, out } => items::run(&n, &out)?,
        Cmd::Ring { n, max_dust, keep, bulbs, then, divider, unchecked, out } => ring::run(&n, max_dust, keep, bulbs, &then, divider, !unchecked, &out)?,
        Cmd::StampGroups { dry_run, files } => stamp::run(&files, dry_run)?,
        Cmd::Compose { out, name, parts } => compose::compose(&out, &name, &parts)?,
        Cmd::SchedGen { out, modules, offdelay } => schedgen::generate(&out, &modules, &offdelay)?,
        Cmd::CounterV8 { v7, out } => counter8::generate(&v7, &out)?,
        Cmd::LoopGen { out, modules, offdelay } => loopgen::generate(&out, &modules, &offdelay)?,
        Cmd::ReleaseTrace { design, at, det, ticks, fault, watch } => releasebench::trace(&design, &at, &det, ticks, &fault, &watch)?,
        Cmd::FaultSurvey { design, spec, pulses } => releasebench::fault_survey(&design, &spec, pulses)?,
        Cmd::NapReport { log } => releasebench::nap_report(&log)?,
        Cmd::TraceDiff { a, b } => releasebench::trace_diff(&a, &b)?,
        Cmd::BedDistance { design, riders } => releasebench::bed_distance(&design, &riders)?,
        Cmd::Price { designs, ground, stage } => releasebench::price(&designs, ground, &stage)?,
        Cmd::DesignEdit { file, out, move_block, move_entity } => releasebench::design_edit(&file, &out, &move_block, &move_entity)?,
        Cmd::ReleaseSweep { loop_file, offdelay, siding, siding_ctl, pad, knob, values, run, show } => releasebench::release_sweep(&loop_file, &offdelay, siding.as_deref(), siding_ctl.as_deref(), pad.as_deref(), knob.as_deref(), &values, &run, &show)?,
        Cmd::SidingCtlGen { out, sched } => sidinggen::control_gen(&out, &sched)?,
        Cmd::Slice { file, r#box } => {
            let [x0, y0, z0, x1, y1, z1] = r#box[..] else { anyhow::bail!("--box takes X0,Y0,Z0,X1,Y1,Z1") };
            let d = mctas::design::Design::load(&file)?;
            let ch = |s: &str| -> char {
                let n = s.trim_start_matches("minecraft:");
                let kind = n.split('[').next().unwrap_or("");
                let facing = n.split(['[', ',', ']']).find_map(|t| t.strip_prefix("facing=")).unwrap_or("");
                match kind {
                    "air" => '.',
                    "stone" | "smooth_stone" | "cobblestone" => '#',
                    "glass" => 'g',
                    "redstone_wire" => '+',
                    // (a repeater's facing is its input side: the arrow points to its output)
                    "repeater" => match facing {
                        "east" => '<',
                        "west" => '>',
                        "north" => 'v',
                        _ => '^',
                    },
                    "comparator" => 'C',
                    "dropper" => 'D',
                    "hopper" => 'H',
                    "redstone_block" => 'R',
                    k if k.contains("copper_bulb") => 'B',
                    k if k.contains("torch") => 'T',
                    k if k.contains("rail") => '=',
                    _ => 'o',
                }
            };
            for y in (y0..=y1).rev() {
                println!("y {y} (x {x0}..{x1} across, z {z0}..{z1} down)");
                for z in z0..=z1 {
                    let row: String = (x0..=x1).map(|x| d.blocks.get(&[x, y, z]).map(|s| ch(s)).unwrap_or('.')).collect();
                    println!("{z:>4} {row}");
                }
            }
        }
        Cmd::Cells { file, r#box } => {
            let [x0, y0, z0, x1, y1, z1] = r#box[..] else { anyhow::bail!("--box takes X0,Y0,Z0,X1,Y1,Z1") };
            let d = mctas::design::Design::load(&file)?;
            let mut cells: Vec<(&[i32; 3], &String)> = d.blocks.iter().filter(|(p, s)| (x0..=x1).contains(&p[0]) && (y0..=y1).contains(&p[1]) && (z0..=z1).contains(&p[2]) && !s.ends_with(":air")).collect();
            cells.sort_by_key(|(p, _)| (p[2], p[1], p[0]));
            for (p, s) in cells {
                println!("{} {} {} {}", p[0], p[1], p[2], s.trim_start_matches("minecraft:"));
            }
        }
        Cmd::DropTest { speed, occupied, ticks, no_stop, staged } => {
            droptest::run(speed, occupied, ticks, !no_stop, staged)?;
        }
        Cmd::ReleaseGen { out, offdelay, loop_file, siding, siding_ctl, pad } => match loop_file {
            Some(l) => releasegen::generate_loop(&out, &l, &offdelay, siding.as_deref(), siding_ctl.as_deref(), pad.as_deref())?,
            None => releasegen::generate(&out, &offdelay)?,
        },
        Cmd::ReleaseBench { design, pulses, seed, miss, early, double, restarts, game, turns, placed_raw } => {
            let ok = releasebench::bench(&design, &releasebench::Opts { pulses, seed, miss, early, double, restarts, game, turns, raw: placed_raw })?;
            if !ok {
                anyhow::bail!("release-bench: violations");
            }
        }
        Cmd::LoopBench { design, days, seed, miss, burst, dusk, end, game, weather, restarts, placed_raw } => {
            let o = loopbench::Opts { days, seed, miss, burst, dusk, end, game_days: game, weather, restarts, raw: placed_raw, ..Default::default() };
            if !loopbench::bench(&design, &o)? {
                bail!("the loop scheduler fails its bench");
            }
        }
        Cmd::LoopModel { policy, days, seed, miss, every, dusk, end, visit, nap, rejoin, out, verbose } => {
            let pair = |s: &str| -> Result<(u32, u32)> {
                let (a, b) = s.split_once("..").unwrap_or((s, s));
                Ok((a.parse()?, b.parse()?))
            };
            let o = loopmodel::Opts { policy, days, seed, miss, every, dusk, end, visit: pair(&visit)?, nap: pair(&nap)?, rejoin, out, verbose };
            if !loopmodel::run(&o) {
                bail!("the loop schedule loses slots or naps");
            }
        }
        Cmd::SchedBench { design, days, seed, miss, burst, dusk, end, game, watch, probes } => {
            let watch = match watch {
                Some(s) => {
                    let (a, z) = s.split_once("..").ok_or_else(|| anyhow::anyhow!("--watch FROM..TO"))?;
                    Some((a.parse()?, z.parse()?))
                }
                None => None,
            };
            let probes: Vec<[i32; 3]> = probes
                .split_whitespace()
                .map(|s| {
                    let v: Vec<i32> = s.split(',').filter_map(|n| n.parse().ok()).collect();
                    [v.first().copied().unwrap_or(0), v.get(1).copied().unwrap_or(0), v.get(2).copied().unwrap_or(0)]
                })
                .collect();
            let o = schedbench::Opts { days, seed, miss, burst, dusk, end, game_days: game, watch, probes, ..Default::default() };
            if !schedbench::bench(&design, &o)? {
                bail!("the scheduler fails its bench");
            }
        }
        Cmd::SchedModel { policy, days, seed, miss, burst, every, dusk, dawn, nap, visit, ready, phase_always, rotate, pring_end, queue, hold, retake, visit_tail, visit_body, verbose } => {
            let range = |s: &str| -> Result<(u32, u32)> {
                let (a, b) = s.split_once("..").ok_or_else(|| anyhow!("{s:?}: MIN..MAX"))?;
                Ok((a.trim().parse()?, b.trim().parse()?))
            };
            let o = schedmodel::Opts { policy, days, seed, miss, burst, every, dusk, dawn, nap: range(&nap)?, visit: range(&visit)?, ready, phase_always, verbose, rotate, pring_end, queue, hold, retake, visit_tail, visit_body };
            if !schedmodel::run(&o)? {
                bail!("the policy loses slots or leaves a villager a night without a nap");
            }
        }
        Cmd::CounterBench { design, pulses, seed, restarts, width, jitter, miss, burst, ready, tc, m, dump, dump_ticks, stimulus_out, turns } => {
            let turns: Vec<u8> = turns.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| s.parse::<u8>().with_context(|| format!("--turns {s:?}"))).collect::<Result<_>>()?;
            let range = |s: &str| -> Result<(i64, i64)> {
                let (a, b) = s.split_once("..").ok_or_else(|| anyhow!("{s:?}: MIN..MAX"))?;
                Ok((a.trim().parse()?, b.trim().parse()?))
            };
            let (w0, w1) = range(&width)?;
            let o = seqbench::Opts { pulses, seed, restarts, width: (w0 as u32, w1 as u32), jitter, miss, burst, ready, tc, m: range(&m)?, dump, dump_ticks, stimulus_out, turns, ..Default::default() };
            if !seqbench::bench(&design, &o)? {
                bail!("the counter fails its bench");
            }
        }
        Cmd::Check { design, gap } => {
            let c = circuit_tff::model::Circuit::load(&design)?;
            let words: Vec<String> = std::iter::once(c.kind.clone()).chain(c.args.iter().cloned()).collect();
            let words: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
            let spec: Box<dyn circuit_tff::census::Spec> = match words.as_slice() {
                ["rs-latch"] => Box::new(specs::RsSpec { gap }),
                ["d-latch"] => Box::new(specs::DSpec { gap }),
                ["counter", n] => Box::new(specs::CounterSpec { n: n.parse().context("counter N")?, gap }),
                ["dff"] => Box::new(specs::DffSpec { gap }),
                ["tff-reset"] => Box::new(specs::TffResetSpec { gap }),
                _ => bail!("kind {words:?}: rs-latch | d-latch | counter N | dff | tff-reset"),
            };
            let mut inputs = c.inputs.clone();
            inputs.sort_by(|a, b| a.name.cmp(&b.name));
            let census = circuit_tff::census::Census {
                inputs: inputs.iter().map(|i| redsim::BlockPos::new(i.pos[0], i.pos[1], i.pos[2])).collect(),
                fixed: Vec::new(),
                fixed_be: Vec::new(),
                free: Vec::new(),
                palette: Vec::new(),
                palette_be: Vec::new(),
                k_min: 0,
                k_max: 0,
                outputs: Some(c.outputs.iter().map(|(_, p)| redsim::BlockPos::new(p[0], p[1], p[2])).collect()),
                jobs: 1,
                margin: 3,
                auto_floor: false,
            };
            let mut blocks = Vec::new();
            for (p, s) in &c.design.blocks {
                blocks.push((redsim::BlockPos::new(p[0], p[1], p[2]), redsim::Block::parse(s).map_err(|e| anyhow!(e))?));
            }
            let be: Vec<(redsim::BlockPos, String)> = c.design.block_entities.iter().map(|(p, s)| (redsim::BlockPos::new(p[0], p[1], p[2]), s.clone())).collect();
            match census.simulate_debug(&blocks, &be, spec.as_ref()) {
                Ok(lines) => {
                    for l in lines {
                        println!("{l}");
                    }
                }
                Err(e) => println!("rejected: {e}"),
            }
        }
        Cmd::Census { kind, dims, inputs, k, palette, gap, jobs, auto_floor, out } => {
            let d: Vec<i32> = dims.split('x').map(|t| t.parse::<i32>()).collect::<Result<_, _>>().context("--box AxBxC")?;
            if d.len() != 3 {
                bail!("--box AxBxC");
            }
            let mut free = Vec::new();
            for y in 0..d[1] {
                for z in 0..d[2] {
                    for x in 0..d[0] {
                        free.push(redsim::BlockPos::new(x, y, z));
                    }
                }
            }
            let mut named: Vec<(String, redsim::BlockPos)> = Vec::new();
            for part in inputs.split(';').filter(|s| !s.trim().is_empty()) {
                let (n, p) = part.split_once('=').ok_or_else(|| anyhow!("--inputs name=x,y,z;..."))?;
                let v: Vec<i32> = p.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().context("--inputs")?;
                named.push((n.trim().to_string(), redsim::BlockPos::new(v[0], v[1], v[2])));
            }
            // the judge orders inputs by name
            named.sort_by(|a, b| a.0.cmp(&b.0));
            let words: Vec<&str> = kind.split_whitespace().collect();
            let spec: Box<dyn circuit_tff::census::Spec> = match words.as_slice() {
                ["rs-latch"] => Box::new(specs::RsSpec { gap }),
                ["d-latch"] => Box::new(specs::DSpec { gap }),
                ["counter", n] => Box::new(specs::CounterSpec { n: n.parse().context("counter N")?, gap }),
                ["dff"] => Box::new(specs::DffSpec { gap }),
                ["tff-reset"] => Box::new(specs::TffResetSpec { gap }),
                _ => bail!("--kind rs-latch | d-latch | counter N | dff | tff-reset"),
            };
            let want_inputs = match words[0] {
                "rs-latch" => vec!["r", "s"],
                "d-latch" => vec!["d", "e"],
                "dff" => vec!["clk", "d"],
                "tff-reset" => vec!["r", "t"],
                _ => vec![""],
            };
            let names: Vec<&str> = named.iter().map(|(n, _)| n.as_str()).collect();
            if names != want_inputs && !(want_inputs == vec![""] && names.len() == 1) {
                bail!("--inputs for {kind}: {want_inputs:?}, got {names:?}");
            }
            let (pal, pal_be) = circuit_tff::cli::parse_palette(&palette)?;
            let (k_min, k_max) = circuit_tff::cli::parse_k(&k)?;
            let free: Vec<redsim::BlockPos> = free.into_iter().filter(|p| !named.iter().any(|(_, q)| q == p)).collect();
            eprintln!("{kind}: {} free cells, {} palette states, k {k_min}..={k_max}", free.len(), pal.len());
            let c = circuit_tff::census::Census {
                inputs: named.iter().map(|(_, p)| *p).collect(),
                fixed: Vec::new(),
                fixed_be: Vec::new(),
                free,
                palette: pal,
                palette_be: pal_be,
                k_min,
                k_max,
                outputs: None,
                jobs,
                margin: 3,
                auto_floor,
            };
            let found = c.run(spec.as_ref(), true);
            std::fs::create_dir_all(&out)?;
            let mut rows: Vec<(String, usize, u32, String)> = Vec::new();
            let short = words.join("");
            for f in &found {
                let fam = circuit_tff::cli::family_of(&f.blocks);
                let h = circuit_tff::cli::fnv(&f.key);
                let name = format!("{short}-{}-{}b-{}", if fam.is_empty() { "none".to_string() } else { fam.replace('+', "-") }, f.blocks.len(), &h[..8]);
                let named_in: Vec<(String, redsim::BlockPos)> = named.iter().map(|(n, p)| (if want_inputs == vec![""] { String::new() } else { n.clone() }, *p)).collect();
                let des = circuit_tff::cli::found_design(&kind, &name, f, &named_in);
                des.save(&out.join(format!("{name}.txt")))?;
                rows.push((fam, f.blocks.len(), f.score, name));
            }
            rows.sort();
            let mut tsv = String::from("family\tblocks\tscore\tdesign\n");
            for (fam, b, s, n) in &rows {
                tsv.push_str(&format!("{fam}\t{b}\t{s}\t{n}\n"));
            }
            std::fs::write(out.join("summary.tsv"), &tsv)?;
            println!("{} {kind} types found (summary.tsv in {})", rows.len(), out.display());
            for (fam, b, s, n) in rows.iter().take(30) {
                println!("  {b} blocks, {fam}, score {s}: {n}");
            }
        }
    }
    Ok(())
}
