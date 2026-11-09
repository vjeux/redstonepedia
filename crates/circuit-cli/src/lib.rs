//! `mctas circuit ...`: the contraption studies (docs/circuits.md). Clap definitions and dispatch
//! only; the judge is crates/circuit, each track's own commands are its crate
//! (`mctas circuit <track> <args...>` -> `circuit_<track>::run`).

pub mod shrink;

use anyhow::Result;
use circuit::Circuit;
use circuit::verify::Options;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum CircuitCmd {
    /// Parse designs and print their kind, inputs (and how the judge drives them), outputs,
    /// payloads and block count.
    Info { designs: Vec<PathBuf> },
    /// The redsim screen (milliseconds): the kind's stimulus (sized from redsim's recovery time),
    /// one run, the kind's checker; prints the test table and `CIRCUIT: <name> <kind> <metrics>,
    /// <blocks> blocks, <WxHxD>, redsim` (or why it fails). With --stimulus (or a design without
    /// `circuit:`), the waveform of that stimulus.
    /// The exact gate of a redsim model (any kind): the kind's stimulus on the base build in redsim
    /// AND in the game, every cell of the watched box compared at every tick. Prints `IDENTICAL:
    /// <name>, <frames> frames` or the first divergence.
    TraceCompare {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Record runs of the real game for `mctas test`'s simulator pieces (circuit::recorded): each
    /// design frozen into `--dir`, the run (`--kind`: sorter-run, smelter-run, brewer-run,
    /// composter-run with `--run`, or circuit-trace: the circuit judge's stimulus on the base
    /// build) in the game AND on redsim, which must be identical, and the game's frames saved as
    /// `<dir>/<stem>-<run>.rec`. Then `mctas recorded write <dir> --fresh`.
    Record {
        designs: Vec<PathBuf>,
        #[arg(long)]
        kind: String,
        #[arg(long, default_value = "base")]
        run: String,
        #[arg(long)]
        dir: PathBuf,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Replay recorded runs (`.rec` files or directories of them) on redsim without the game,
    /// every cell of the watched box compared every gt with the game's recorded frames; a
    /// recording whose design changed is STALE and fails. `N of M recordings identical`.
    Replay {
        recordings: Vec<PathBuf>,
        /// Recordings replayed at once (0: every core).
        #[arg(long, default_value_t = 0)]
        jobs: usize,
    },
    Sim {
        designs: Vec<PathBuf>,
        /// An explicit stimulus: `a 10+ 14-; b pulse 2 every 10 x 3`.
        #[arg(long)]
        stimulus: Option<String>,
        /// The test spacing in gt (default: redsim's recovery time + 10).
        #[arg(long)]
        gap: Option<u32>,
        /// Print the waveform (inputs and outputs per gt) of the whole run.
        #[arg(long)]
        wave: bool,
        /// Print the cells that change, frame by frame, over `<from>..<to>` (the waveform's frame
        /// numbers), with the inputs and output levels: how a design works, tick by tick.
        #[arg(long)]
        cells: Option<String>,
        /// Print the checker's table (one line per test).
        #[arg(long)]
        table: bool,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// A flying machine over the ground: grass N air blocks below its lowest block, under its
        /// whole corridor (the ground clearance's runs, `circuit::robust::fly_clearance`).
        #[arg(long)]
        ground: Option<i32>,
    },
    /// stimulus in its own sandbox; each run is checked (no entity, nothing reaching the edge of
    /// the watched box, the kind's spec) and the builds compared. Prints `CIRCUIT: <name> <kind>
    /// <metrics>, <blocks> blocks, <WxHxD>, robust|position-dependent|rotation-dependent` or
    /// `FAIL: ...`; exits 1 unless every design passes.
    Verify {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        /// Only as built (no rotations).
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long)]
        stimulus: Option<String>,
        #[arg(long)]
        gap: Option<u32>,
        /// Print the base build's checker table.
        #[arg(long)]
        table: bool,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Greedy block removal judged in the real game: each round tries removing every block and
    /// keeps the first removal whose verdict still passes with the SAME metrics. Ends with the
    /// smaller design written beside the original (or `--out`).
    Shrink {
        design: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Also try removing the blocks at the port cells (an input's receiving block, an output lamp cell).
        #[arg(long)]
        free_ports: bool,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Obsidian -> stone wherever the kind's judge still passes with the same kind, axis and
    /// metrics (circuit::material, vjeux 2026-10-21: only obsidian where needed); the changed
    /// designs to --out with material.tsv (each kept obsidian's reason: blast resistance or
    /// immovable); --judge confirms them in the game.
    /// Write THE OBVIOUS full-size sugar cane farm (circuit::canefarm, designs/briefs/
    /// farm-sugar-cane.md): `modules` double rows of `length` canes on mud, an observer and a
    /// piston per cane, hoppers under every mud and water cell into one chest.
    CaneGen {
        #[arg(long, default_value_t = 3)]
        modules: i32,
        #[arg(long, default_value_t = 16)]
        length: i32,
        /// Layout 4 instead: a water-lattice field `--lattice W,L` driven by this clock design.
        #[arg(long, value_delimiter = ',')]
        lattice: Vec<i32>,
        /// Layout 5 instead: the lattice field `--sweeper W,L` swept by flying machines.
        #[arg(long, value_delimiter = ',')]
        sweeper: Vec<i32>,
        #[arg(long, default_value = "designs/clocks/archive/hopclk-sealed-p600.txt")]
        clock: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Material {
        designs: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        judge: bool,
    },
    /// MINIMUM VOLUME for any kind (circuit::compact, track clk-rt): box-bounded annealing from a
    /// seed on the kind's own redsim judge; a candidate passes with the seed's kind and axis value,
    /// ranked by the judge's box then blocks; the front goes to DIR/front/, `--game N` judges the
    /// smallest that beat the seed in the real game (robust, same axis). `--all` (no seed): every
    /// study's minimum-volume robust design per (kind, axis) from the archive, DIR/sweep.tsv.
    Compact {
        /// One seed (a verbose run into --out), or several (one run each, DIR/sweep.tsv).
        seeds: Vec<PathBuf>,
        #[arg(long)]
        all: bool,
        /// Collect a finished (or running) sweep's game-OK designs into --out/<study>/ with
        /// captions from their seeds' (then `circuit archive` them).
        #[arg(long)]
        collect: Option<PathBuf>,
        /// With --all: these studies (default: every study but tnt, which has `circuit tnt compact`,
        /// and shulker).
        #[arg(long, value_delimiter = ',')]
        studies: Vec<String>,
        /// With --all: only these kinds (a kind name, or its first word: `counter` covers every
        /// `counter N`).
        #[arg(long, value_delimiter = ',')]
        kinds: Vec<String>,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        /// The region `x0,y0,z0,x1,y1,z1` (default: the seed's box).
        #[arg(long)]
        region: Option<String>,
        /// Palette block states (default: the seed's own states plus stone), `;`-separated.
        #[arg(long)]
        palette: Option<String>,
        #[arg(long, default_value_t = 4000)]
        iterations: usize,
        #[arg(long, default_value_t = 24)]
        workers: usize,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 1500)]
        stale: usize,
        #[arg(long, default_value_t = 40)]
        repair: usize,
        #[arg(long, default_value_t = 2)]
        game: usize,
        /// Block ids never deleted or replaced, comma-separated (e.g. `hopper,dropper,comparator`:
        /// a hopper ring's hop count sets a precise drop's burst point).
        #[arg(long, value_delimiter = ',')]
        keep: Vec<String>,
        /// The axis value a passing design must have (default: the seed's): seed with a smaller
        /// design of a neighbouring axis (e.g. `--axis "off L = 4 gt"` from an L 16 design).
        #[arg(long)]
        axis: Option<String>,
        /// An item sorter: candidates need not keep the seed's badges (`overflow-proof`).
        #[arg(long)]
        any_badges: bool,
        /// A sorter or smelter seed that FAILS its judge (with `--axis`): a walk toward a passing
        /// design, broken candidates ranked by the runs they pass and the items they deliver.
        #[arg(long, requires = "axis")]
        from_failing: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Judge designs in the real game and keep the passing ones in their study's archive
    /// (designs/circuits/<study>/archive/: the design with its judge line, a verdict row, a
    /// caption); a structure already archived as the same kind is refused. `--dedupe` (no
    /// designs, no game): keep one design per kind and type in every study, the others listed in
    /// aliases.tsv.
    Archive {
        designs: Vec<PathBuf>,
        /// The caption (default: the design's first comment block).
        #[arg(long)]
        caption: Option<String>,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long)]
        dedupe: bool,
        /// Judge every archived design again with the current judge (in the game, family bases
        /// with their sampled members): passing ones get their new metrics, failing ones leave
        /// the archive and go back to their track (`update-less/` or `rejudged/`).
        #[arg(long)]
        rejudge: bool,
        /// With --rejudge or --fill-families: only these studies (default: all).
        #[arg(long)]
        study: Vec<String>,
        /// Judge in the game every member of each archived extendable family that extend.tsv
        /// does not list yet (every n of its range, up to 16 members), and add the passing ones.
        #[arg(long)]
        fill_families: bool,
        /// Write the `game rts` metric (the farm judge's robustness speed, `farm::game_rts`, from
        /// the vanilla twin run alone: no game) into every archived harvester's verdict (`--study`
        /// narrows it), before its bill, as the judge now records it; other metrics are kept.
        #[arg(long)]
        fill_game_rts: bool,
        /// Take designs out of their archive first (`<study>/<file>`, e.g. one another design now
        /// dominates): the file, its verdict row and its caption.
        #[arg(long)]
        remove: Vec<String>,
    },
    /// Write every study's CATALOG.md and variants.md, the index designs/circuits/CATALOG.md and
    /// the website's data designs/circuits/catalog.tsv from the archives (no game).
    Catalog {
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
    },
    /// Replace archived designs' captions from a TSV file, one `<study>/<file><TAB><caption>` per
    /// line (the site shows a caption's first sentence as the plain explanation, the rest under
    /// details); `#` lines are comments. No game.
    Captions {
        file: PathBuf,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
    },
    /// R7 comparison (redsim, no game): every archived design with a plain input run with the
    /// judge's old block placement and with real input devices (the judge's stone powered by its
    /// repeater); lists the ones passing only by the block placement, refused or changed.
    R7 {
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        /// Only these studies (default: all).
        #[arg(long)]
        study: Vec<String>,
    },
    /// Seal designs' open hopper tops (the real-use audit's item intrusion): glass in each open
    /// intake cell (around one holding a thin block), then the design judged on redsim with the
    /// audit on; a sealed design that passes with the original's metrics is written to --out as
    /// `<name>-sealed.txt`.
    Seal {
        designs: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// The real-use audit's comparison (redsim, no game): every archived design judged without
    /// and with the audit's stimulus parts (real button presses, a lever held down, an extender's
    /// lever flipped back too soon); lists the designs that now fail or measure otherwise.
    RealUse {
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        /// Only these studies (default: all).
        #[arg(long)]
        study: Vec<String>,
        #[arg(long, default_value_t = 16)]
        threads: usize,
    },
    /// Set an archived design's caption (captions.tsv of its study's archive; no game): for the
    /// plain-language pass (one sentence a non-redstoner follows first, the detail after it).
    Caption {
        /// The study directory name (designs/circuits/<study>).
        study: String,
        /// The archived file (`tnt-launch-14b.txt` or without `.txt`).
        file: Option<String>,
        #[arg(long)]
        text: Option<String>,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        /// Set every `file<TAB>caption` row of this file instead (after a merge took the other
        /// side's captions.tsv, for instance).
        #[arg(long)]
        from: Option<PathBuf>,
    },
    /// Family generators: `repeater-line` (the judge's own calibration family) = n + 1 delay-1
    /// repeaters in a row on stone (a delay line, N = 2 + 2n gt, 2 + 2n blocks); `dropper-lift` = the
    /// dropper-chain item elevator, 4 + 2n blocks up in 13 + 5n blocks (`circuit::lift::dropper_chain`);
    /// `dropper-pair` = the dropper-pair item elevator, 4 + 2n up in 11 + 5n (`lift::dropper_pair`);
    /// `dropper-bend` = the odd heights, 5 + 2n up in 14 + 5n (`lift::dropper_bend`);
    /// `bubble-column` = a bubble column fed by one clocked dropper, 4 + n up in 40 + 5n;
    /// `bubble-rest` = the same with the clock gated by the dropper's contents (it rests), 3 + n up in 40 + 5n.
    Gen {
        family: String,
        #[arg(long)]
        n: i64,
        #[arg(long)]
        out: PathBuf,
        /// Write the member as a design of its own under this name: no `extend` headers (the site
        /// publishes archived designs only, a family just its base: vjeux 2026-10-22).
        #[arg(long)]
        standalone: Option<String>,
    },
    /// Track pulse's commands (crates/circuit-pulse).
    Pulse {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track bud's commands (crates/circuit-bud): BUD switches, study 10.
    Bud {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track tff's commands (crates/circuit-tff).
    Tff {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track memory's commands (crates/circuit-memory).
    Memory {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track delay's commands (crates/circuit-delay).
    Delay {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track gates's commands (crates/circuit-gates).
    Gates {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track pext's commands (crates/circuit-pext).
    Pext {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track arith's commands (crates/circuit-arith, study 12: adders, subtractors, incrementers,
    /// comparators).
    Arith {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track fly's commands (crates/circuit-fly).
    Fly {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track shulker's commands (crates/circuit-shulker).
    Shulker {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track tnt's commands (crates/circuit-tnt).
    Tnt {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track pulsemult's commands (crates/circuit-pulsemult): study 11's searches.
    Pulsemult {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track timer's commands (crates/circuit-timer): study 11's monostables, off-delays and
    /// on-delays.
    Timer {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Study 15's 7-segment decoders (crates/circuit-display): design generators.
    Display {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Study 14: the 3D interleave search. Dial 0 fixed, dial 1 at every offset (dx, dy, dz) and
    /// mirror, cells two dials share must be stone in both; every candidate judged on redsim
    /// (lock 2), the passing ones ranked by blocks then box; the best pattern repeated for 3 dials.
    LockSearch {
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Study 14: write the `lock N` family's member (lock3-alt's pattern for N dials,
    /// circuit::lock::alt_design): two comparators per dial, the frames on the west wall. With
    /// `--hidden`, the hidden-code family (circuit::lock::hidden_design): the code frames on the
    /// back face, facing away from the player.
    LockGen {
        #[arg(long)]
        n: u32,
        #[arg(long)]
        hidden: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Study 13, item sorters: ONE run of the sorter judge (circuit::sorter::Run) in the game at
    /// rotation 0, its table printed (what reached each chest) and the outcome: a design aid;
    /// `mctas circuit verify` is the judge. `--sim --run all`: the whole judge on redsim (every
    /// run, the badges), one line per design.
    SorterRun {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        /// `base` (one slice, an input chest), `tiled` (slices side by side, a hopper line),
        /// `stream` (a water stream on the row's input hopper), `jam` (the middle chest full),
        /// `direct` (a water stream right on the first slice), `all` (with --sim: the whole
        /// judge).
        #[arg(long, default_value = "base")]
        run: String,
        /// The old badge names (`tileable` = `--run tiled`, `overflow` = `--run jam`).
        #[arg(long)]
        badge: Option<String>,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// On redsim (milliseconds) instead of the game.
        #[arg(long)]
        sim: bool,
        /// The exact gate: the run in the game AND on redsim, every cell of the watched box
        /// compared every gt (`IDENTICAL: <name>, <frames> frames` or the first divergence).
        #[arg(long, conflicts_with = "sim")]
        compare: bool,
    },
    /// Auto-crafters (`circuit: crafter <result>`): one of the judge's runs at rotation
    /// 0, its table (what reached the output, the crafts before the tail, the supply left). The
    /// judge is `circuit verify`.
    CrafterRun {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        /// `base` (full and partial stacks, a quiet check, a tail), `intrusion` (strays on every
        /// open hopper top), `all` (with --sim: both).
        #[arg(long, default_value = "base")]
        run: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// On redsim (milliseconds) instead of the game.
        #[arg(long)]
        sim: bool,
        /// Print the cells that change every 20 gt (block states and container contents).
        #[arg(long)]
        trace: bool,
    },
    /// Study 18, crop harvesters: ONE run in the game at rotation 0 (circuit::farm: real random
    /// ticks at --rts for --ticks, then a rest), its table and the outcome: a design aid; `mctas
    /// circuit verify` is the judge (RTS 300, 24000 ticks, positions x rotations, the copies).
    /// One run of the automatic door judge (study 21: armor-stand walkers through a 2x2 door from both
    /// sides, walking and sprinting, two in a row, one stopping in the doorway), its table printed
    /// (crates/circuit/src/autodoor.rs).
    DoorRun {
        design: PathBuf,
        #[arg(long, default_value_t = 0)]
        turns: u8,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        offset: Vec<i32>,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// The run in redsim (`autodoor::sim_run`: the walkers teleported as the game's commands do)
        /// instead of the game.
        #[arg(long)]
        sim: bool,
        /// The whole judge's redsim twin (`autodoor::sim_verify`), every build's line.
        #[arg(long)]
        builds: bool,
        /// The whole judge in the game and on its redsim twin, build by build (they must agree).
        #[arg(long)]
        compare_verify: bool,
    },
    /// The blocks a hopper-minecart unloader or loader does not need: each removed alone in parallel
    /// game runs, then the spare ones removed one at a time (crates/circuit/src/cartio.rs::trim).
    CartTrim {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// One run of the hopper-minecart unloader or loader judge (study 19: 12 carts, mixed loads,
    /// several speeds; every item accounted for), its table printed (crates/circuit/src/cartio.rs).
    CartRun {
        design: PathBuf,
        /// One build of `circuit verify`: quarter turns, then this offset (x,y,z).
        #[arg(long, default_value_t = 0)]
        turns: u8,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        offset: Vec<i32>,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// Record the game run as an exact trace in this directory (`<name>-judged.trace` and its
        /// design), for `redsim-fuzz trace-compare`.
        #[arg(long)]
        trace: Option<PathBuf>,
        /// The run on the redsim twin (`cartio::sim_run`) instead of the game.
        #[arg(long)]
        sim: bool,
        /// The whole judge's redsim twin (`cartio::sim_verify`), every build's line.
        #[arg(long)]
        builds: bool,
        /// The whole judge in the game and on its redsim twin, build by build (they must agree).
        #[arg(long)]
        compare_verify: bool,
    },
    /// One run of the minecart station judge (study 19: real carts, ridden and empty, at several
    /// speeds; presses at random moments), its table printed (crates/circuit/src/station.rs).
    StationRun {
        design: PathBuf,
        /// One build of `circuit verify`: quarter turns, then this offset (x,y,z).
        #[arg(long, default_value_t = 0)]
        turns: u8,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        offset: Vec<i32>,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// Record the game run as an exact trace in this directory (`<name>-judged.trace` and its
        /// design), for `redsim-fuzz trace-compare` (the redsim twin's tick-by-tick gate).
        #[arg(long)]
        trace: Option<PathBuf>,
        /// The run on the redsim twin (`station::sim_run`) instead of the game.
        #[arg(long)]
        sim: bool,
        /// The whole judge's redsim twin (`station::sim_verify`), every build's line.
        #[arg(long)]
        builds: bool,
        /// The whole judge in the game and on its redsim twin, build by build (they must agree).
        #[arg(long)]
        compare_verify: bool,
    },
    /// Write a full-size farm (coordinator 2026-10-21): `--layout stem-lattice --crop melon --n 8`
    /// (n x n stems, every stem with four shared fruit cells; `circuit::farmgen`).
    FarmGen {
        #[arg(long, default_value = "stem-lattice")]
        layout: String,
        #[arg(long, default_value = "minecraft:melon")]
        crop: String,
        #[arg(long, default_value_t = 8)]
        n: i32,
        /// (bamboo-sweeper) the field's length along z (default: n).
        #[arg(long, default_value_t = 0)]
        len: i32,
        /// The design file to write.
        #[arg(long)]
        out: PathBuf,
    },
    FarmRun {
        design: PathBuf,
        /// The copies side by side (`tile:`) instead of one.
        #[arg(long)]
        tiled: bool,
        /// One tick per batch (a different run): the last 40 moves of every crop item that goes off
        /// its way or comes to rest on a crop.
        #[arg(long)]
        trace: bool,
        /// One build of `circuit verify`: quarter turns, then this offset (x,y,z).
        #[arg(long, default_value_t = 0)]
        turns: u8,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        offset: Vec<i32>,
        /// The run's random_tick_speed (default: the judge's, `circuit::farm::rts_for`: 300 up to
        /// 16 plants, less for bigger farms).
        #[arg(long)]
        rts: Option<u32>,
        #[arg(long, default_value_t = circuit::farm::RUN)]
        ticks: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// The judge's redsim twin (`circuit::farm::sim_run`) instead of the game (`--turns` turns
        /// the build without the game: `Circuit::turned_pure`).
        #[arg(long)]
        sim: bool,
        /// The run in the game AND on redsim: their records side by side (the twin follows the
        /// game run of the same build and seed draw for draw, so they must agree); a turned build
        /// is also checked to turn alike without the game.
        #[arg(long, conflicts_with = "sim")]
        compare: bool,
        /// The WHOLE judge in the game (`circuit verify`'s builds and tiled run) and its redsim
        /// twin (`circuit sim`), build by build: every build's outcome and the verdicts compared,
        /// and every turned design checked to turn alike without the game.
        #[arg(long, conflicts_with_all = ["sim", "compare"])]
        compare_verify: bool,
        /// With `--compare`: the run's every tick in the game (one tick per step) and on redsim,
        /// the first tick whose blocks or entities differ printed (the twin's calibration).
        #[arg(long, requires = "compare")]
        probe: bool,
    },
    /// Study 17, auto-smelters: ONE run of the smelter judge (circuit::smelter::Run) in the game at
    /// rotation 0, its table printed; `--sim --run all`: the whole judge on redsim, one line per
    /// design; `--compare`: the run in the game and on redsim, every cell compared every gt.
    SmelterRun {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        /// `base`, `tiled` (a slice: 3 copies), `intrusion` (strays), `all` (with --sim).
        #[arg(long, default_value = "base")]
        run: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long)]
        sim: bool,
        #[arg(long, conflicts_with = "sim")]
        compare: bool,
        /// With --sim: print, per furnace, the gt each coal reached its fuel slot (stderr).
        #[arg(long)]
        coal_log: bool,
        /// Every hopper's and furnace's contents every 20 gt for the first 6000 gt (stderr).
        #[arg(long)]
        trace: bool,
        /// With --run chest, in the game: the chest run at 4 rotations x this many positions (the
        /// super smelter's judge until it is wired into `circuit verify`).
        #[arg(long)]
        rotations: Option<usize>,
    },
    /// The whole cart line's run (circuit::cartsys, designs/briefs/cart-transport.md): the double
    /// supply chest filled with 3456 items, one empty hopper minecart on the track, minutes until
    /// every item is in the storage chest; `--trickle`: a slow feed, then nothing, and the cart
    /// must end parked at the loader.
    CartSystem {
        /// A whole cart line: loader, track, unloader (circuit::cartsys: ports items(.b),
        /// output(.b), cart).
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        #[arg(long)]
        sim: bool,
        /// The trickle run: the supply fed 5 items every 100 gt up to 300, then stopped; all must
        /// arrive and the cart must end parked at the loader.
        #[arg(long)]
        trickle: bool,
    },
    /// Write a whole cart line from two stations (circuit::cartsys::compose): the unloader turned
    /// at the west end, `--track` blocks of track, the loader at the east end.
    CartLine {
        #[arg(long)]
        loader: PathBuf,
        #[arg(long)]
        unloader: PathBuf,
        #[arg(long, default_value_t = 128)]
        track: i32,
        /// A powered rail every this many blocks.
        #[arg(long, default_value_t = 8)]
        spacing: i32,
        #[arg(long)]
        out: PathBuf,
    },
    /// Study 23: one run of the kelp factory judge (circuit::factory) at rotation 0, in the game or
    /// on redsim (--sim): kelp planted, one dried kelp block in the fuel chest, `--hours` of
    /// vanilla random ticks, the output chest counted.
    FactoryRun {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        /// The run's length in Minecraft hours (72000 gt each; the judge's: 6).
        #[arg(long, default_value_t = 6.0)]
        hours: f64,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long)]
        sim: bool,
        /// With --sim: every container of the design and the furnace, comparator and dropper
        /// states every 1000 gt (stderr).
        #[arg(long)]
        trace: bool,
        /// The intrusion run: strays on every open hopper top at a quarter, half and three
        /// quarters of the run.
        #[arg(long)]
        strays: bool,
    },
    /// Study 22: one run of the auto-composter judge (circuit::composter) at rotation 0, in the
    /// game or on redsim (--sim); `--sim --run all`: the whole judge on redsim; `--compare`: the
    /// run in the game and on redsim, every cell of the watched box compared every gt.
    ComposterRun {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        /// `base` (a slice: its row of 4), `tiled8` (a slice's row of 8), `intrusion`, `all` (with --sim).
        #[arg(long, default_value = "base")]
        run: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long)]
        sim: bool,
        #[arg(long, conflicts_with = "sim")]
        compare: bool,
    },
    /// Study 21: one run of the auto-brewer judge (circuit::brewer) at rotation 0, in the game or
    /// on redsim (--sim); `--sim --run all`: the whole judge on redsim; `--compare`: the run in the
    /// game and on redsim, every cell of the watched box compared every gt.
    BrewerRun {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        /// `base`, `tiled` (a row of 4 stands), `tiled8`, `intrusion` (strays), `all` (with --sim).
        #[arg(long, default_value = "base")]
        run: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long)]
        sim: bool,
        #[arg(long, conflicts_with = "sim")]
        compare: bool,
        /// Print every stand's contents, the hoppers above and under it and its brew timer every
        /// sample (20 gt) of the first 4000 gt, on stderr.
        #[arg(long)]
        trace: bool,
    },
    /// Item elevators (`circuit: item-elevator N`, circuit::lift): ONE run of the judge at
    /// rotation 0 (`--run base|intrusion`), in the game or on redsim (`--sim`), its table and
    /// outcome: a design aid; `mctas circuit verify` is the judge (`circuit sim` its redsim twin).
    /// (`circuit run` does the same for every registered study.)
    LiftRun {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "base")]
        run: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long)]
        sim: bool,
        /// The whole judge's redsim twin (`circuit::lift::sim_verify`), every build's line.
        #[arg(long)]
        builds: bool,
        /// The whole judge in the game and on its redsim twin, build by build (they must agree).
        #[arg(long)]
        compare_verify: bool,
        /// With --sim: one of the judge's builds, rotated this many quarter turns...
        #[arg(long, default_value_t = 0)]
        turns: u8,
        /// ...and moved by `x,y,z` (as `--builds` prints them).
        #[arg(long)]
        offset: Option<String>,
        /// With --sim: print every gt the item entities whose id contains this (where an item goes).
        #[arg(long)]
        watch: Option<String>,
    },
    /// A registered study's kind (circuit::registry, `src/studies/*.rs`): ONE run (`--run`, the
    /// kind's runs; the first by default) in the game, or on its redsim twin (`--sim`; with
    /// `--turns/--offset` one of the judge's builds); `--builds` the twin's whole judge, build by
    /// build; `--compare-verify` the game judge and the twin build by build (they must agree);
    /// `--trace DIR` records the game run (kinds that record). `mctas circuit verify` is the judge.
    Run {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        #[arg(long)]
        run: Option<String>,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long)]
        sim: bool,
        #[arg(long)]
        builds: bool,
        #[arg(long)]
        compare_verify: bool,
        #[arg(long, default_value_t = 0)]
        turns: u8,
        /// `x,y,z`, as `--builds` prints the builds.
        #[arg(long)]
        offset: Option<String>,
        /// The kind's own per-tick note (stderr).
        #[arg(long)]
        watch: Option<String>,
        #[arg(long)]
        trace: Option<PathBuf>,
    },
    /// design with up to --remove of the base's blocks removed (hoppers and comparators stay) and
    /// fewer palette blocks added in the air cells of --cells; prints those passing the base, tiled
    /// and overflow runs and writes the smallest to --out.
    SorterCensus {
        base: PathBuf,
        #[arg(long, default_value_t = 2)]
        remove: usize,
        /// The box the added blocks go in: `x,y,z..x,y,z`.
        #[arg(long)]
        cells: String,
        /// Block states, comma-separated (`stone,glass,wire,torch,wtorch-n,wtorch-s,rep-n,rep-s`
        /// are shorthands).
        #[arg(long, default_value = "stone,wire,torch,wtorch-n,wtorch-s,rep-n,rep-s")]
        palette: String,
        #[arg(long, default_value_t = 0)]
        threads: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// The local census for ANY kind (circuit::local): every design with up to --remove of the
    /// base's blocks removed (none whose state contains a --keep word) and fewer palette blocks
    /// added in the air cells of --cells, judged on redsim: a sorter by its runs, any other kind by
    /// its checker with the base's main value (its first metric: L, N, the function...). Prints the
    /// passing designs and writes the smallest to --out; the game judge comes after.
    LocalCensus {
        base: PathBuf,
        #[arg(long, default_value_t = 2)]
        remove: usize,
        /// The box the added blocks go in: `x,y,z..x,y,z`, or `grow:N` (the base's bounds grown by
        /// N, the default `grow:1`).
        #[arg(long, default_value = "grow:1")]
        cells: String,
        /// Block states, comma-separated (shorthands: stone, glass, wire, torch, wtorch-n/s/e/w,
        /// rep-n/s/e/w, obs-n/s/e/w/u/d).
        #[arg(long, default_value = "stone,wire,torch,wtorch-n,wtorch-s,rep-n,rep-s")]
        palette: String,
        /// Blocks never removed: words of their states, comma-separated.
        #[arg(long, default_value = "")]
        keep: String,
        /// Metrics a candidate must share with the base besides its main value
        /// (comma-separated: `delay` keeps an instant design instant).
        #[arg(long, default_value = "")]
        same: String,
        /// Look for another main value than the base's (the first metric's value as the checker
        /// prints it, e.g. `3 gt`): the base only seeds the search.
        #[arg(long)]
        value: Option<String>,
        #[arg(long, default_value_t = 0)]
        threads: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn info(c: &Circuit) -> String {
    let mut out = format!("{}: {}, {} blocks", c.name(), c.kind.as_ref().map(|k| format!("{} (study {})", k.name(), k.study())).unwrap_or_else(|| "no kind".into()), c.blocks());
    for i in &c.inputs {
        let how = match &i.how {
            circuit::InputHow::Block { on, off } => format!("{} / {}", clock::short(on), clock::short(off)),
            circuit::InputHow::Driven { dir, through } => format!(
                "driven: {}, powered by the judge's repeater {dir:?} from it{}",
                c.clock.design.blocks.get(&i.pos).map(|s| format!("the design's {}", clock::short(s))).unwrap_or_else(|| "the judge's stone".into()),
                if *through { " (fed through the build)" } else { "" }
            ),
            circuit::InputHow::Lever => "lever".into(),
            circuit::InputHow::Button => "button".into(),
            circuit::InputHow::Strength { .. } => "strength (a chest the judge fills)".into(),
            circuit::InputHow::Frame { rotation } => format!("item frame at rotation {rotation} (the judge turns it, one right-click per step)"),
        };
        out.push_str(&format!("\n  input {} at {:?}: {how}", i.name, i.pos));
    }
    for o in &c.outputs {
        out.push_str(&format!("\n  output {} at {:?}", o.name, o.pos));
    }
    for p in &c.payloads {
        out.push_str(&format!("\n  payload {} at {:?}", p.name, p.pos));
    }
    if let Some(t) = c.travel {
        out.push_str(&format!("\n  travel {t:?}{}", c.distance.map(|d| format!(", distance {d}")).unwrap_or_default()));
    }
    out
}

pub fn run(version: &str, cmd: CircuitCmd) -> Result<()> {
    match cmd {
        CircuitCmd::Info { designs } => {
            for p in &designs {
                match Circuit::load(p) {
                    Ok(c) => println!("{}", info(&c)),
                    Err(e) => println!("ERROR {}: {e:#}", p.display()),
                }
            }
        }
        CircuitCmd::Record { designs, kind, run, dir, margin } => {
            anyhow::ensure!(circuit::recorded::KINDS.contains(&kind.as_str()), "--kind {kind}: one of {}", circuit::recorded::KINDS.join(", "));
            let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
            let mut bad = 0;
            for d in &designs {
                let (file, r) = circuit::recorded::record(&kind, &run, d, &dir, margin, &pool)?;
                match r {
                    Ok(n) => println!("RECORDED: {}, {n} frames (the game = redsim)", file.display()),
                    Err(why) => {
                        bad += 1;
                        println!("NOT RECORDED: {}: {why}", d.display());
                    }
                }
            }
            anyhow::ensure!(bad == 0, "{bad} runs differ between the game and redsim: nothing recorded for them");
        }
        CircuitCmd::Replay { recordings, jobs } => {
            let jobs = if jobs > 0 { jobs } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8) };
            circuit::recorded::replay_all(&recordings, jobs)?;
        }
        CircuitCmd::TraceCompare { designs, margin } => {
            let o = Options { margin, version: version.to_string(), ..Default::default() };
            for p in &designs {
                let c = Circuit::load(p)?;
                match circuit::verify::trace_compare(&c, &o)? {
                    Ok(frames) => println!("IDENTICAL: {}, {frames} frames (every cell of the watched box, redsim = the game)", c.name()),
                    Err(why) => println!("DIVERGES: {}: {why}", c.name()),
                }
            }
        }
        CircuitCmd::Sim { designs, stimulus, gap, wave, cells, table, margin, ground } => {
            let o = Options { stimulus, gap, margin, version: version.to_string(), ..Default::default() };
            for p in &designs {
                // a design the judge refuses to load (R7: no way in for its input repeater) is
                // reported and the others still run
                let mut c = match Circuit::load(p) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {:#}", p.display(), e);
                        continue;
                    }
                };
                if let Some(g) = ground {
                    c.grounded = true;
                    c.drop = g;
                }
                let r = circuit::verify::sim(&c, &o)?;
                if let (true, Some(out)) = (table, &r.outcome) {
                    for l in &out.table {
                        println!("  {l}");
                    }
                }
                if let Ok(t) = &r.trace
                    && (wave || r.outcome.is_none())
                {
                    for l in t.strip(&c, 0, t.frames()) {
                        println!("  {l}");
                    }
                }
                if let (Some(range), Ok(t)) = (&cells, &r.trace) {
                    print_cell_changes(t, range);
                }
                println!("{}", r.summary(&c));
            }
        }
        CircuitCmd::CaneGen { modules, length, lattice, sweeper, clock, out } => {
            let d = if sweeper.len() == 3 {
                circuit::canefarm::sweeper_test(sweeper[0], sweeper[1], sweeper[2])
            } else if sweeper.len() == 2 {
                circuit::canefarm::sweeper(sweeper[0], sweeper[1])
            } else if lattice.len() == 2 {
                circuit::canefarm::lattice(lattice[0], lattice[1], &mctas::design::Design::load(&clock)?)
            } else {
                circuit::canefarm::obvious(modules, length)
            };
            std::fs::write(&out, d.to_text())?;
            println!("{}: {} blocks, {} canes", out.display(), d.blocks.len(), d.ports.keys().filter(|k| k.starts_with("crop")).count());
        }
        CircuitCmd::Material { designs, out, judge } => circuit::material::cli(&designs, &out, judge, version)?,
        CircuitCmd::Compact { seeds, all, collect, studies, kinds, root, region, palette, iterations, workers, rng, stale, repair, game, keep, axis, any_badges, from_failing, out } => {
            if let Some(sweep) = collect {
                for p in circuit::compact::collect(&root, &sweep, &out)? {
                    println!("{}", p.display());
                }
                return Ok(());
            }
            let region = match region {
                Some(s) => {
                    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 6, "--region x0,y0,z0,x1,y1,z1");
                    Some(([v[0], v[1], v[2]], [v[3], v[4], v[5]]))
                }
                None => None,
            };
            let palette: Vec<String> = palette.map(|p| p.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
            let o = circuit::compact::Options { region, palette, iterations, workers, rng, stale, repair, game, version: version.to_string(), keep, axis, any_badges, from_failing };
            if all {
                let studies: Vec<String> = if studies.is_empty() { circuit::archive::studies().iter().map(|(s, _)| s.to_string()).filter(|s| s != "tnt" && s != "shulker").collect() } else { studies };
                circuit::compact::compact_all(&root, &studies, &kinds, &o, &out)?;
            } else if seeds.len() > 1 {
                circuit::compact::compact_list(&seeds, &o, &out)?;
            } else {
                let seed = seeds.first().cloned().ok_or_else(|| anyhow::anyhow!("a seed design, or --all"))?;
                let r = circuit::compact::compact_one(&seed, &o, &out, true)?;
                println!("{}: {} {}: seed volume {}, {} blocks", seed.display(), r.kind, r.axis, r.seed_volume, r.seed_blocks);
                for (v, b, f, g) in &r.best {
                    println!("  {v} volume, {b} blocks: {} {}", f.display(), g.chars().take(220).collect::<String>());
                }
                if r.best.is_empty() {
                    println!("  no smaller design");
                }
            }
        }
        CircuitCmd::Shrink { design, out, free_ports, positions, no_rotations, workers, margin } => {
            let o = Options { positions, rotations: !no_rotations, workers, stimulus: None, gap: None, margin, version: version.to_string(), sim_threads: 0, exact_runs: 1 };
            let out = out.unwrap_or_else(|| {
                let stem = design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                design.with_file_name(format!("{stem}-shrunk.txt"))
            });
            shrink::shrink(&design, &out, !free_ports, &o)?;
        }
        CircuitCmd::Verify { designs, positions, no_rotations, workers, stimulus, gap, table, margin } => {
            let o = Options { positions, rotations: !no_rotations, workers, stimulus, gap, margin, version: version.to_string(), sim_threads: 0, exact_runs: 1 };
            let pool = circuit::verify::spawn_pool(&o)?;
            // the designs side by side (clk-misc 2026-10-23: the selftest's calibration list grew
            // with every study and verified its designs one after another, each waiting for its
            // slowest build while the other workers idled): up to the pool's size of designs at
            // once, every build on the one pool (Pool::map hands each worker to one design's job
            // queue at a time; MCTAS_MAX_WORKERS caps the pool), the reports printed in the given
            // order. A design that does not load stops the run there, after the ones before it.
            let loaded: Vec<Result<Circuit>> = designs.iter().map(|p| Circuit::load(p)).collect();
            let upto = loaded.iter().position(|c| c.is_err()).unwrap_or(loaded.len());
            let circuits: Vec<&Circuit> = loaded[..upto].iter().filter_map(|c| c.as_ref().ok()).collect();
            let slots: std::sync::Mutex<Vec<Option<Result<circuit::verify::VerifyReport>>>> = std::sync::Mutex::new((0..circuits.len()).map(|_| None).collect());
            let ready = std::sync::Condvar::new();
            let next = std::sync::atomic::AtomicUsize::new(0);
            let mut failed = 0;
            let mut stop: Option<anyhow::Error> = None;
            std::thread::scope(|s| {
                for _ in 0..pool.len().clamp(1, circuits.len().max(1)) {
                    s.spawn(|| {
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if i >= circuits.len() {
                                break;
                            }
                            let rep = circuit::verify::verify_in_pool(&pool, circuits[i], &o);
                            slots.lock().unwrap()[i] = Some(rep);
                            ready.notify_all();
                        }
                    });
                }
                for (i, c) in circuits.iter().enumerate() {
                    let rep = {
                        let mut g = slots.lock().unwrap();
                        while g[i].is_none() {
                            g = ready.wait(g).unwrap();
                        }
                        g[i].take().unwrap()
                    };
                    match rep {
                        Ok(rep) => failed += print_verify_report(c, &rep, table) as usize,
                        Err(e) => {
                            // (as before: the run stops at the first design the judge cannot run)
                            next.store(usize::MAX / 2, std::sync::atomic::Ordering::Relaxed);
                            stop = Some(e);
                            break;
                        }
                    }
                }
            });
            if let Some(e) = stop {
                return Err(e);
            }
            if let Some(Err(e)) = loaded.into_iter().nth(upto) {
                return Err(e);
            }
            if failed > 0 {
                std::process::exit(1);
            }
        }
        CircuitCmd::Archive { designs, caption, root, positions, workers, dedupe, rejudge, study, fill_families, fill_game_rts, remove } => {
            for spec in &remove {
                println!("{}", circuit::archive::unarchive(&root, spec)?);
            }
            if !remove.is_empty() && designs.is_empty() && !rejudge && !dedupe && !fill_families && !fill_game_rts {
                return Ok(());
            }
            if dedupe {
                for l in circuit::archive::dedupe(&root)? {
                    println!("{l}");
                }
                if designs.is_empty() && !rejudge && !fill_families && !fill_game_rts {
                    return Ok(());
                }
            }
            // no game: before the pool
            if fill_game_rts {
                for l in circuit::archive::fill_game_rts(&root, &study, Options::default().margin)? {
                    println!("{l}");
                }
                if designs.is_empty() && !rejudge && !fill_families {
                    return Ok(());
                }
            }
            let o = Options { positions, workers, version: version.to_string(), ..Default::default() };
            let pool = circuit::verify::spawn_pool(&o)?;
            if rejudge {
                for l in circuit::archive::rejudge(&pool, &root, &o, &study)? {
                    println!("{l}");
                }
            }
            if fill_families {
                for l in circuit::archive::fill_families(&pool, &root, &o, &study)? {
                    println!("{l}");
                }
            }
            for p in &designs {
                match circuit::archive::archive(&pool, p, &o, &root, caption.as_deref()) {
                    Ok(a) => println!("{} {}: {}", if a.new { "ARCHIVED" } else { "SKIPPED" }, a.file.display(), a.summary),
                    Err(e) => println!("REFUSED {}: {e:#}", p.display()),
                }
            }
        }
        CircuitCmd::R7 { root, study } => {
            let o = Options { version: version.to_string(), ..Default::default() };
            for l in circuit::archive::r7_compare(&root, &study, &o)? {
                println!("{l}");
            }
        }
        CircuitCmd::RealUse { root, study, threads } => {
            let o = Options { version: version.to_string(), ..Default::default() };
            for l in circuit::archive::real_use_compare(&root, &study, &o, threads)? {
                println!("{l}");
            }
        }
        CircuitCmd::Seal { designs, out } => {
            let o = Options { version: version.to_string(), ..Default::default() };
            std::fs::create_dir_all(&out)?;
            for p in &designs {
                let c = circuit::Circuit::load(p)?;
                // the original's metrics without the audit (its intrusion run is what fails)
                circuit::check::set_real_use(false);
                let base = circuit::verify::sim(&c, &o).ok().and_then(|r| r.outcome);
                circuit::check::set_real_use(true);
                let Some(base) = base.filter(|b| b.pass) else {
                    println!("SKIP {}: it does not pass on redsim without the audit", c.name());
                    continue;
                };
                let (mut d, added) = circuit::intrusion::seal(&c);
                if added.is_empty() {
                    println!("SKIP {}: no open hopper top", c.name());
                    continue;
                }
                let name = format!("{}-sealed", c.name());
                d.name = name.clone();
                let cells: Vec<String> = added.iter().map(|q| format!("{} {} {}", q[0], q[1], q[2])).collect();
                let note = format!("# Sealed by `mctas circuit seal` (the real-use audit's item intrusion, 2026-10-21): {} glass on\n# the open hopper tops of {} ({}), so no stray item reaches a hopper of its timing.\n", added.len(), c.name(), cells.join(", "));
                let sc = circuit::Circuit::from_design(d.clone())?;
                match circuit::verify::sim(&sc, &o).ok().and_then(|r| r.outcome) {
                    Some(s) if s.pass && s.metrics == base.metrics => {
                        let f = out.join(format!("{name}.txt"));
                        std::fs::write(&f, format!("{note}{}", d.to_text()))?;
                        println!("SEALED {}: +{} glass ({}), {} blocks: {}", c.name(), added.len(), cells.join(", "), sc.blocks(), f.display());
                    }
                    Some(s) if s.pass => println!("CHANGED {}: sealed it measures {:?}, not {:?}", c.name(), s.metrics, base.metrics),
                    Some(s) => println!("FAILS {}: sealed: {}", c.name(), s.why),
                    None => println!("FAILS {}: sealed: no redsim verdict", c.name()),
                }
            }
        }
        CircuitCmd::Captions { file, root } => {
            for l in circuit::archive::set_captions(&root, &std::fs::read_to_string(&file)?)? {
                println!("{l}");
            }
        }
        CircuitCmd::Caption { study, file, text, root, from } => {
            // the rows to set: one from the flags, or every `file<TAB>caption` row of --from
            let rows: Vec<(String, String)> = match &from {
                Some(f) => std::fs::read_to_string(f)?.lines().filter_map(|l| l.split_once('\t')).filter(|(f, _)| *f != "file").map(|(f, c)| (f.to_string(), c.to_string())).collect(),
                None => vec![(file.clone().unwrap_or_default(), text.clone().unwrap_or_default())],
            };
            let dir = root.join(&study).join("archive");
            let path = dir.join("captions.tsv");
            for (file, text) in rows {
                let file = if file.ends_with(".txt") { file } else { format!("{file}.txt") };
                anyhow::ensure!(dir.join(&file).exists(), "{} is not archived", dir.join(&file).display());
                anyhow::ensure!(!text.is_empty() && !text.contains('\t') && !text.contains('\n'), "a caption is one non-empty line without tabs");
                let old = std::fs::read_to_string(&path).unwrap_or_else(|_| "file\tcaption\n".into());
                let mut out = String::new();
                let mut done = false;
                for (i, line) in old.lines().enumerate() {
                    if i > 0 && line.split('\t').next() == Some(file.as_str()) {
                        out.push_str(&format!("{file}\t{text}\n"));
                        done = true;
                    } else {
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                if !done {
                    out.push_str(&format!("{file}\t{text}\n"));
                }
                std::fs::write(&path, out)?;
                println!("{}: caption of {file} set", path.display());
            }
        }
        CircuitCmd::Catalog { root } => {
            // the tables first: one row per key (a merge may have doubled updated rows)
            if let Some(designs) = root.parent() {
                for l in clock::tables::fix_all(designs, false)? {
                    println!("{l}");
                }
            }
            for l in circuit::archive::catalog(&root)? {
                println!("{l}");
            }
            println!("{}/CATALOG.md, {}/catalog.tsv", root.display(), root.display());
        }
        CircuitCmd::Gen { family, n, out, standalone } => {
            if family == "dropper-lift" || family == "dropper-pair" || family == "dropper-bend" || family == "bubble-column" || family == "bubble-rest" {
                anyhow::ensure!((0..=40).contains(&n), "{family} --n {n}: 0..40");
                let text = match family.as_str() {
                    "dropper-pair" => circuit::lift::dropper_pair(n as u32),
                    "dropper-bend" => circuit::lift::dropper_bend(n as u32),
                    "bubble-column" => circuit::lift::bubble_column(n as u32),
                    "bubble-rest" => circuit::lift::bubble_rest(n as u32),
                    _ => circuit::lift::dropper_chain(n as u32),
                };
                // --standalone NAME: the member as a design of its own (no extend headers)
                let text = match &standalone {
                    Some(name) => {
                        let mut v: Vec<String> = Vec::new();
                        for l in text.lines() {
                            if l.starts_with("extend ") {
                                continue;
                            }
                            if l.starts_with("name: ") {
                                v.push(format!("name: {name}"));
                                v.push(format!("# The {family} family's member n = {n}, archived as a design of its own (`mctas circuit gen {family} --n {n} --standalone {name}`)."));
                                continue;
                            }
                            v.push(l.to_string());
                        }
                        v.join("\n") + "\n"
                    }
                    None => text,
                };
                std::fs::write(&out, text)?;
                println!("{}", out.display());
                return Ok(());
            }
            if family != "repeater-line" {
                anyhow::bail!("unknown family {family:?} (repeater-line, dropper-lift, dropper-pair, dropper-bend, bubble-column, bubble-rest)");
            }
            let k = (n + 1).max(1) as usize;
            let text = format!(
                "name: cal-repeater-line-n{n}\ncircuit: delay\n# n + 1 delay-1 repeaters in a row on stone: a delay line of N = 2 + 2n gt.\nextend family: cal-repeater-line\nextend axis: 2 + 2n\nextend blocks: 2 + 2n\nextend rule: one more repeater (and its stone) at the end of the line\nextend n: 0..8\nextend base: {n}\nextend generator: mctas circuit gen repeater-line --n {{n}} --out {{out}}\nport input: 0 1 0\nport output: {} 1 0\npalette:\n  S minecraft:stone\n  r minecraft:repeater[delay=1,facing=west,locked=false,powered=false]\nlayer 0:\n  .{}\nlayer 1:\n  .{}\n",
                k + 1,
                "S".repeat(k),
                "r".repeat(k)
            );
            std::fs::write(&out, text)?;
            println!("{}", out.display());
        }
        CircuitCmd::Pulse { args } => circuit_pulse::run(args)?,
        CircuitCmd::Bud { args } => circuit_bud::run(args)?,
        CircuitCmd::Tff { args } => circuit_tff::run(args)?,
        CircuitCmd::Memory { args } => circuit_memory::run(args)?,
        CircuitCmd::Delay { args } => circuit_delay::run(args)?,
        CircuitCmd::Gates { args } => circuit_gates::run(args)?,
        CircuitCmd::Pext { args } => circuit_pext::run(args)?,
        CircuitCmd::Arith { args } => circuit_arith::run(args)?,
        CircuitCmd::Fly { args } => circuit_fly::run(args)?,
        CircuitCmd::Shulker { args } => circuit_shulker::run(args)?,
        CircuitCmd::Tnt { args } => circuit_tnt::run(args)?,
        CircuitCmd::Pulsemult { args } => circuit_pulsemult::run(args)?,
        CircuitCmd::Timer { args } => circuit_timer::run(args)?,
        CircuitCmd::Display { args } => circuit_display::run(args)?,
        CircuitCmd::SorterCensus { base, remove, cells, palette, threads, out } => census_cmd(&base, remove, &cells, &palette, "minecraft:hopper,minecraft:comparator", "", None, threads, out.as_deref())?,
        CircuitCmd::LocalCensus { base, remove, cells, palette, keep, same, value, threads, out } => census_cmd(&base, remove, &cells, &palette, &keep, &same, value.as_deref(), threads, out.as_deref())?,
        CircuitCmd::LockSearch { out_dir } => {
            std::fs::create_dir_all(&out_dir)?;
            let o = Options { version: version.to_string(), ..Default::default() };
            let judge = |text: &str| -> bool {
                let Ok(d) = mctas::design::Design::parse(text) else { return false };
                let Ok(c) = Circuit::from_design(d) else { return false };
                circuit::verify::sim(&c, &o).ok().and_then(|r| r.outcome).is_some_and(|out| out.pass)
            };
            let mut found: Vec<(usize, i64, (i32, i32, i32, bool), String)> = Vec::new();
            let mut tried = 0;
            // the known 2-dial pattern (lock2-alt) must pass: a check of the search itself
            if let Some((text, _, _)) = circuit::lock::placed_design(&[(0, 3, 0, false), (0, 2, 2, false)]) {
                std::fs::write(out_dir.join("lock2-known.txt"), &text)?;
                let why = match mctas::design::Design::parse(&text).map_err(|e| format!("{e:#}")).and_then(|d| Circuit::from_design(d).map_err(|e| format!("{e:#}"))) {
                    Err(e) => e,
                    Ok(c) => match circuit::verify::sim(&c, &o) {
                        Err(e) => format!("{e:#}"),
                        Ok(r) => r.summary(&c),
                    },
                };
                println!("lock-search: the known lock2-alt pattern: {why}");
            }
            for dx in -3..=3 {
                for dy in -2..=2 {
                    for dz in -3..=3 {
                        for m in [false, true] {
                            let pl = [(0, 3, 0, false), (dx, 3 + dy, dz, m)];
                            let Some((text, blocks, vol)) = circuit::lock::placed_design(&pl) else { continue };
                            tried += 1;
                            if judge(&text) {
                                found.push((blocks, vol, (dx, dy, dz, m), text));
                            }
                        }
                    }
                }
            }
            found.sort_by_key(|f| (f.0, f.1));
            println!("lock-search: {tried} 2-dial placements screened, {} pass", found.len());
            for f in found.iter().take(8) {
                println!("  2 dials: {} blocks, box {}: dial 1 at dx {} dy {} dz {} mirror {}", f.0, f.1, f.2 .0, f.2 .1, f.2 .2, f.2 .3);
            }
            // each passing 2-dial pattern repeated: dial 2 = dial 0 moved by twice the offset
            // (same mirror as dial 0), or by the offset from dial 1 (dial 1's mirror)
            let mut best3: Vec<(usize, i64, String, String)> = Vec::new();
            for f in &found {
                let (dx, dy, dz, m) = f.2;
                for third in [(2 * dx, 3 + 2 * dy, 2 * dz, false), (2 * dx, 3 + 2 * dy, 2 * dz, m), (2 * dx, 3, 2 * dz, false)] {
                    let pl = [(0, 3, 0, false), (dx, 3 + dy, dz, m), third];
                    let Some((text, blocks, vol)) = circuit::lock::placed_design(&pl) else { continue };
                    if judge(&text) {
                        best3.push((blocks, vol, format!("{pl:?}"), text));
                    }
                }
            }
            best3.sort_by_key(|b| (b.0, b.1));
            println!("lock-search: {} 3-dial repeats pass", best3.len());
            for (k, b) in best3.iter().take(5).enumerate() {
                println!("  3 dials: {} blocks, box {}: {}", b.0, b.1, b.2);
                std::fs::write(out_dir.join(format!("lock3-search-{k}.txt")), &b.3)?;
            }
        }
        CircuitCmd::LockGen { n, hidden, out } => {
            if n == 0 || n > 6 {
                anyhow::bail!("lock-gen: 1 to 6 dials");
            }
            std::fs::write(&out, if hidden { circuit::lock::hidden_design(n) } else { circuit::lock::alt_design(n) })?;
            println!("wrote {} (lock {n})", out.display());
        }
        CircuitCmd::Run { designs, run, margin, sim, builds, compare_verify, turns, offset, watch, trace } => {
            let off = match &offset {
                Some(s) => {
                    let v: Vec<i32> = s.trim_matches(|c| c == '[' || c == ']').split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!("--offset {s}: {e}"))?;
                    anyhow::ensure!(v.len() == 3, "--offset x,y,z");
                    [v[0], v[1], v[2]]
                }
                None => [0, 0, 0],
            };
            let o = circuit::verify::Options { margin, version: version.to_string(), ..Default::default() };
            let mut pool: Option<mctas::sim::Pool> = None;
            for design in &designs {
                let c = match Circuit::load(design) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {e:#}", design.display());
                        continue;
                    }
                };
                let Some(circuit::Kind::Study(sk)) = &c.kind else {
                    anyhow::bail!("{}: `circuit run` runs the registered studies' kinds ({}); {} has its own command", design.display(), circuit::registry::all().map(|(_, d)| d.word).collect::<Vec<_>>().join(", "), c.kind.as_ref().map(|k| k.name()).unwrap_or_else(|| "no kind".into()));
                };
                let d = sk.def();
                let twin = |c: &Circuit, o: &circuit::verify::Options| match d.twin {
                    circuit::registry::Twin::Exact(f) => f(c, o),
                    circuit::registry::Twin::GameOnly(why) => Err(why.to_string()),
                };
                if compare_verify {
                    twin_compare_verify(&c, &o, d.verify, &twin)?;
                    continue;
                }
                if builds {
                    print_twin_builds(&c, twin(&c, &o))?;
                    continue;
                }
                let r = run.clone().unwrap_or_else(|| d.runs.first().copied().unwrap_or("base").to_string());
                if !sim && pool.is_none() {
                    pool = Some(mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?);
                }
                let at = circuit::registry::RunAt { run: &r, turns, offset: off, margin, watch: watch.as_deref(), trace: trace.as_deref() };
                let out = (d.run)(if sim { None } else { pool.as_ref() }, &c, &at)?;
                for l in &out.table {
                    println!("{l}");
                }
                let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                let how = if sim { "redsim" } else { "game" };
                if out.pass {
                    println!("RUN: {} {} {r} run: {}, {} blocks (rotation {turns} at {off:?}, {how})", c.name(), sk.name(), m.join(", "), c.blocks());
                } else {
                    println!("FAIL: {} {} {r} run (rotation {turns} at {off:?}, {how}): {} ({})", c.name(), sk.name(), out.why, m.join(", "));
                }
            }
        }
        CircuitCmd::LiftRun { designs, run, margin, sim, builds, compare_verify, turns, offset, watch } => {
            let r = match run.as_str() {
                "base" => circuit::lift::Run::Base,
                "intrusion" => circuit::lift::Run::Intrusion,
                other => anyhow::bail!("lift-run --run {other}: base or intrusion"),
            };
            if sim && (turns != 0 || offset.is_some() || watch.is_some()) {
                let off = match &offset {
                    Some(s) => {
                        let v: Vec<i32> = s.trim_matches(|c| c == '[' || c == ']').split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!("--offset {s}: {e}"))?;
                        anyhow::ensure!(v.len() == 3, "--offset x,y,z");
                        [v[0], v[1], v[2]]
                    }
                    None => [0, 0, 0],
                };
                for design in &designs {
                    let c = Circuit::load(design)?;
                    let out = circuit::lift::run_one_sim_at(&c, r, margin, turns, off, watch.as_deref())?;
                    for l in &out.table {
                        println!("{l}");
                    }
                    println!("{} {}: {} (rotation {turns} at {off:?}, redsim)", if out.pass { "LIFT" } else { "FAIL" }, c.name(), out.why);
                }
                return Ok(());
            }
            if compare_verify {
                let o = circuit::verify::Options { margin, version: version.to_string(), ..Default::default() };
                for design in &designs {
                    lift_compare_verify(&Circuit::load(design)?, &o)?;
                }
                return Ok(());
            }
            if builds {
                let o = circuit::verify::Options { margin, version: version.to_string(), ..Default::default() };
                for design in &designs {
                    let c = Circuit::load(design)?;
                    match circuit::lift::sim_verify(&c, &o) {
                        Ok(rep) => {
                            for b in &rep.builds {
                                let line = match &b.result {
                                    Ok(k) => format!("{} {} [{}]", if k.outcome.pass { "pass" } else { "FAIL" }, k.outcome.why, k.outcome.metrics.iter().map(|(n, v)| format!("{n} {v}")).collect::<Vec<_>>().join(", ")),
                                    Err(e) => format!("refused: {e}"),
                                };
                                println!("  rotation {} at {:?}: {}", b.turns, b.offset, line.chars().take(300).collect::<String>());
                            }
                            println!("{} (redsim)", rep.summary(&c));
                        }
                        Err(why) => println!("UNMODELED: {}: {why}", c.name()),
                    }
                }
                return Ok(());
            }
            let pool = if sim { None } else { Some(mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?) };
            for design in &designs {
                let c = match Circuit::load(design) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {e:#}", design.display());
                        continue;
                    }
                };
                let out = circuit::lift::run_one(pool.as_ref(), &c, r, margin)?;
                for l in &out.table {
                    println!("{l}");
                }
                let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                let how = if sim { "redsim" } else { "game" };
                if out.pass {
                    println!("LIFT: {} {} {} run: {}, {} blocks ({how})", c.name(), c.kind.as_ref().map(|k| k.name()).unwrap_or_default(), r.title(), m.join(", "), c.blocks());
                } else {
                    println!("FAIL: {} {} run ({how}): {} ({})", c.name(), r.title(), out.why, m.join(", "));
                }
            }
        }
        CircuitCmd::SmelterRun { designs, run, margin, sim, compare, coal_log, trace, rotations } => {
            circuit::smelter::TRACE.store(trace, std::sync::atomic::Ordering::Relaxed);
            circuit::smelter::COAL_LOG.store(coal_log, std::sync::atomic::Ordering::Relaxed);
            let all = run == "all";
            let r = if all { circuit::smelter::Run::Base } else { circuit::smelter::Run::parse(&run).ok_or_else(|| anyhow::anyhow!("smelter-run: --run {run}: one of base, tiled, intrusion, chest, all"))? };
            if all && !sim {
                anyhow::bail!("smelter-run --run all: redsim only (--sim); the game's whole judge is `mctas circuit verify`");
            }
            let pool = if sim { None } else { Some(mctas::sim::Pool::spawn(if rotations.is_some() { 8 } else { 1 }, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?) };
            for design in &designs {
                let c = match Circuit::load(design) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {e:#}", design.display());
                        continue;
                    }
                };
                let kind = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
                if all {
                    let (_, out) = circuit::smelter::sim_score(&c);
                    if out.pass {
                        let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                        let d = circuit::sorter::dims(&c);
                        println!("SMELTER: {} {kind} {}, {} blocks, {}x{}x{}, redsim", c.name(), m.join(", "), c.blocks(), d[0], d[1], d[2]);
                    } else {
                        for l in &out.table {
                            println!("  {l}");
                        }
                        println!("FAIL: {} {kind}: {}", c.name(), out.why);
                    }
                    continue;
                }
                if compare {
                    match circuit::smelter::compare(pool.as_ref().unwrap(), &c, margin, r)? {
                        Ok(frames) => println!("IDENTICAL: {}, {frames} frames (every cell of the watched box every gt of the judge's {} run, redsim = the game)", c.name(), r.name()),
                        Err(why) => println!("DIVERGES: {}: {why}", c.name()),
                    }
                    continue;
                }
                if let Some(n) = rotations {
                    let pool = pool.as_ref().ok_or_else(|| anyhow::anyhow!("smelter-run --rotations: the game only (no --sim)"))?;
                    if r != circuit::smelter::Run::Chest {
                        anyhow::bail!("smelter-run --rotations: the chest run only (--run chest)");
                    }
                    let outs = circuit::smelter::chest_rotations(pool, &c, margin, n)?;
                    let fails: Vec<String> = outs.iter().filter(|(_, _, o)| !o.pass).map(|(t, off, o)| format!("rotation {t} at {off:?}: {}", o.why)).collect();
                    for (t, off, o) in &outs {
                        let m: Vec<String> = o.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                        println!("  rotation {t} at {off:?}: {}", if o.pass { m.join(", ") } else { format!("FAIL {}", o.why) });
                    }
                    let d = circuit::sorter::dims(&c);
                    let bill = circuit::shulker::bill(&c);
                    if fails.is_empty() {
                        let base = &outs[0].2;
                        let m: Vec<String> = base.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                        println!("SMELTER: {} {kind} (chest run, {} builds): {}, bill {bill}, {} blocks, {}x{}x{}, robust", c.name(), outs.len(), m.join(", "), c.blocks(), d[0], d[1], d[2]);
                    } else {
                        println!("FAIL: {} (chest run): {}", c.name(), fails.join("; "));
                    }
                    continue;
                }
                let out = match &pool {
                    None => circuit::smelter::sim_one(&c, r, margin)?,
                    Some(p) => circuit::smelter::run_one(p, &c, r, margin)?,
                };
                for l in &out.table {
                    println!("  {l}");
                }
                if out.pass {
                    let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                    println!("SMELTER: {} {kind} ({} run): {}", c.name(), r.name(), m.join(", "));
                } else {
                    println!("FAIL: {} ({} run): {}", c.name(), r.name(), out.why);
                }
            }
        }
        CircuitCmd::BrewerRun { designs, run, margin, sim, compare, trace } => {
            circuit::brewer::TRACE.store(trace, std::sync::atomic::Ordering::Relaxed);
            let all = run == "all";
            let r = if all { circuit::brewer::Run::Base } else { circuit::brewer::Run::parse(&run).ok_or_else(|| anyhow::anyhow!("brewer-run: --run {run}: one of base, tiled, tiled8, intrusion, all"))? };
            if all && !sim {
                anyhow::bail!("brewer-run --run all: redsim only (--sim); the game's whole judge is `mctas circuit verify`");
            }
            let pool = if sim { None } else { Some(mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?) };
            for design in &designs {
                let c = match Circuit::load(design) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {e:#}", design.display());
                        continue;
                    }
                };
                let kind = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
                if all {
                    let (_, out) = circuit::brewer::sim_score(&c);
                    if out.pass {
                        let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                        let d = circuit::sorter::dims(&c);
                        println!("BREWER: {} {kind} {}, {} blocks, {}x{}x{}, redsim", c.name(), m.join(", "), c.blocks(), d[0], d[1], d[2]);
                    } else {
                        for l in &out.table {
                            println!("  {l}");
                        }
                        println!("FAIL: {} {kind}: {}", c.name(), out.why);
                    }
                    continue;
                }
                if compare {
                    match circuit::brewer::compare(pool.as_ref().unwrap(), &c, margin, r)? {
                        Ok(frames) => println!("IDENTICAL: {}, {frames} frames (every cell of the watched box every gt of the judge's {} run, redsim = the game)", c.name(), r.name()),
                        Err(why) => println!("DIVERGES: {}: {why}", c.name()),
                    }
                    continue;
                }
                let out = match &pool {
                    None => circuit::brewer::sim_one(&c, r, margin)?,
                    Some(p) => circuit::brewer::run_one(p, &c, r, margin)?,
                };
                for l in &out.table {
                    println!("  {l}");
                }
                if out.pass {
                    let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                    println!("BREWER: {} {kind} ({} run): {}", c.name(), r.name(), m.join(", "));
                } else {
                    println!("FAIL: {} ({} run): {}", c.name(), r.name(), out.why);
                }
            }
        }
        CircuitCmd::CartLine { loader, unloader, track, spacing, out } => {
            let name = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let d = circuit::cartsys::compose(&name, &clock::Clock::load(&loader)?, &clock::Clock::load(&unloader)?, track, spacing)?;
            let head = format!("# A whole cart line (mctas circuit cart-line): {} at the west end turned half a turn, {track} blocks of track with a powered rail every {spacing}, {} at the east end.\n", unloader.display(), loader.display());
            std::fs::write(&out, format!("{head}{}", d.to_text()))?;
            println!("wrote {} ({} blocks)", out.display(), d.blocks.len());
        }
        CircuitCmd::CartSystem { designs, sim, trickle } => {
            let pool = if sim { None } else { Some(mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?) };
            for design in &designs {
                let (pass, line, table) = circuit::cartsys::run_one(pool.as_ref(), design, trickle)?;
                for l in &table {
                    println!("  {l}");
                }
                let name = design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if pass {
                    println!("SYSTEM: {name} {line}, {}", if sim { "redsim" } else { "game" });
                } else {
                    println!("FAIL: {name}: {line}");
                }
            }
        }
        CircuitCmd::FactoryRun { designs, hours, margin, sim, trace, strays } => {
            circuit::factory::TRACE.store(trace, std::sync::atomic::Ordering::Relaxed);
            let ticks = (hours * circuit::factory::HOUR) as u32;
            let pool = if sim { None } else { Some(mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?) };
            for design in &designs {
                let c = match Circuit::load(design) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {e:#}", design.display());
                        continue;
                    }
                };
                let kind = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
                let out = match &pool {
                    None => circuit::factory::sim_one(&c, ticks, margin, strays)?,
                    Some(p) => circuit::factory::run_one(p, &c, ticks, margin, strays)?,
                };
                for l in &out.table {
                    println!("  {l}");
                }
                if out.pass {
                    let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                    let d = circuit::sorter::dims(&c);
                    println!("FACTORY: {} {kind} {}, {} blocks, {}x{}x{}, {}", c.name(), m.join(", "), c.blocks(), d[0], d[1], d[2], if sim { "redsim" } else { "game" });
                } else {
                    println!("FAIL: {} {kind}: {}", c.name(), out.why);
                }
            }
        }
        CircuitCmd::ComposterRun { designs, run, margin, sim, compare } => {
            let all = run == "all";
            let r = if all { circuit::composter::Run::Base } else { circuit::composter::Run::parse(&run).ok_or_else(|| anyhow::anyhow!("composter-run: --run {run}: one of base, tiled8, intrusion, all"))? };
            if all && !sim {
                anyhow::bail!("composter-run --run all: redsim only (--sim); the game's whole judge is `mctas circuit verify`");
            }
            let pool = if sim { None } else { Some(mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?) };
            for design in &designs {
                let c = match Circuit::load(design) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {e:#}", design.display());
                        continue;
                    }
                };
                let kind = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
                if all {
                    let (_, out) = circuit::composter::sim_score(&c);
                    if out.pass {
                        let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                        let d = circuit::sorter::dims(&c);
                        println!("COMPOSTER: {} {kind} {}, {} blocks, {}x{}x{}, redsim", c.name(), m.join(", "), c.blocks(), d[0], d[1], d[2]);
                    } else {
                        for l in &out.table {
                            println!("  {l}");
                        }
                        println!("FAIL: {} {kind}: {}", c.name(), out.why);
                    }
                    continue;
                }
                if compare {
                    match circuit::composter::compare(pool.as_ref().unwrap(), &c, margin, r)? {
                        Ok(frames) => println!("IDENTICAL: {}, {frames} frames (every cell of the watched box every gt of the judge's {} run, redsim = the game)", c.name(), r.name()),
                        Err(why) => println!("DIVERGES: {}: {why}", c.name()),
                    }
                    continue;
                }
                let out = match &pool {
                    None => circuit::composter::sim_one(&c, r, margin)?,
                    Some(p) => circuit::composter::run_one(p, &c, r, margin)?,
                };
                for l in &out.table {
                    println!("  {l}");
                }
                if out.pass {
                    let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                    println!("COMPOSTER: {} {kind} ({} run): {}", c.name(), r.name(), m.join(", "));
                } else {
                    println!("FAIL: {} ({} run): {}", c.name(), r.name(), out.why);
                }
            }
        }
        CircuitCmd::SorterRun { designs, run, badge, margin, sim, compare } => {
            let name = badge.unwrap_or(run);
            let all = name == "all";
            let run = if all { circuit::sorter::Run::Base } else { circuit::sorter::Run::parse(&name).ok_or_else(|| anyhow::anyhow!("sorter-run: --run {name}: one of base, tiled, stream, jam, direct, intrusion, row, all"))? };
            if all && !sim {
                anyhow::bail!("sorter-run --run all: redsim only (--sim); the game's whole judge is `mctas circuit verify`");
            }
            let pool = if sim { None } else { Some(mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?) };
            for design in &designs {
                let c = match Circuit::load(design) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {e:#}", design.display());
                        continue;
                    }
                };
                let kind = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
                if all {
                    let out = circuit::sorter::sim_verdict(&c);
                    if out.pass {
                        let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                        let d = circuit::sorter::dims(&c);
                        println!("SORTER: {} {kind} {}, {} blocks, {}x{}x{}, redsim", c.name(), m.join(", "), c.blocks(), d[0], d[1], d[2]);
                    } else {
                        println!("FAIL: {} {kind}: {}", c.name(), out.why);
                    }
                    continue;
                }
                if compare {
                    match circuit::sorter::compare(pool.as_ref().unwrap(), &c, margin, run)? {
                        Ok(frames) => println!("IDENTICAL: {}, {frames} frames (every cell of the watched box every gt of the judge's {} run, redsim = the game)", c.name(), run.name()),
                        Err(why) => println!("DIVERGES: {}: {why}", c.name()),
                    }
                    continue;
                }
                let out = match &pool {
                    None => circuit::sorter::sim_one(&c, run, margin)?,
                    Some(p) => circuit::sorter::run_one(p, &c, run, margin)?,
                };
                for l in &out.table {
                    println!("  {l}");
                }
                if out.pass {
                    let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                    println!("SORTER: {} {kind} ({} run): {}", c.name(), run.name(), m.join(", "));
                } else {
                    println!("FAIL: {} ({} run): {}", c.name(), run.name(), out.why);
                }
            }
        }
        CircuitCmd::DoorRun { design, turns, offset, margin, sim, builds, compare_verify } => {
            let c = Circuit::load(&design)?;
            let off = [offset.first().copied().unwrap_or(0), offset.get(1).copied().unwrap_or(0), offset.get(2).copied().unwrap_or(0)];
            if compare_verify || builds {
                let o = circuit::verify::Options { margin, version: version.to_string(), ..Default::default() };
                return if compare_verify { twin_compare_verify(&c, &o, circuit::autodoor::verify_in_pool, &circuit::autodoor::sim_verify) } else { print_twin_builds(&c, circuit::autodoor::sim_verify(&c, &o)) };
            }
            let out = if sim {
                let rc = c.turned_pure(turns)?.translated(off)?;
                match circuit::autodoor::sim_run(&rc, margin) {
                    Ok(Ok(log)) => circuit::autodoor::judge(&rc, &log),
                    Ok(Err(why)) => circuit::check::Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
                    Err(why) => circuit::check::Outcome { pass: false, why: format!("redsim does not model it: {why:#}"), metrics: Vec::new(), table: Vec::new() },
                }
            } else {
                let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
                circuit::autodoor::run_one(&pool, &c, turns, off, margin)?
            };
            for l in &out.table {
                println!("  {l}");
            }
            let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
            if out.pass {
                println!("DOOR: {} {}, {} blocks: {}", c.name(), c.kind.as_ref().map(|k| k.name()).unwrap_or_default(), c.blocks(), m.join(", "));
            } else {
                println!("FAIL: {}: {}", c.name(), out.why);
            }
        }
        CircuitCmd::CartTrim { design, out, workers, margin } => {
            let c = Circuit::load(&design)?;
            let pool = mctas::sim::Pool::spawn(workers, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
            let (d, gone) = circuit::cartio::trim(&pool, &c, margin)?;
            let text = std::fs::read_to_string(&design)?;
            let head: String = text.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
            std::fs::write(&out, format!("{head}# trimmed (clk-obs `circuit cart-trim`): {} blocks the cart judge does not need: {gone:?}\n{}", gone.len(), d.to_text()))?;
            println!("{}: {} blocks removed, {} left", out.display(), gone.len(), d.blocks.len());
        }
        CircuitCmd::CartRun { design, turns, offset, margin, trace, sim, builds, compare_verify } => {
            let c = Circuit::load(&design)?;
            let off = [offset.first().copied().unwrap_or(0), offset.get(1).copied().unwrap_or(0), offset.get(2).copied().unwrap_or(0)];
            if compare_verify || builds {
                let o = circuit::verify::Options { margin, version: version.to_string(), ..Default::default() };
                return if compare_verify { twin_compare_verify(&c, &o, circuit::cartio::verify_in_pool, &circuit::cartio::sim_verify) } else { print_twin_builds(&c, circuit::cartio::sim_verify(&c, &o)) };
            }
            let out = if sim {
                let rc = c.turned_pure(turns)?.translated(off)?;
                match circuit::cartio::sim_run(&rc, margin) {
                    Ok(Ok(log)) => circuit::cartio::judge(&rc, &log),
                    Ok(Err(why)) => circuit::check::Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
                    Err(why) => circuit::check::Outcome { pass: false, why: format!("redsim does not model it: {why}"), metrics: Vec::new(), table: Vec::new() },
                }
            } else {
                let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
                circuit::cartio::run_one_rec(&pool, &c, turns, off, margin, trace.as_deref())?
            };
            for l in &out.table {
                println!("  {l}");
            }
            let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
            if out.pass {
                println!("CART: {} {}, {} blocks: {}", c.name(), c.kind.as_ref().map(|k| k.name()).unwrap_or_default(), c.blocks(), m.join(", "));
            } else {
                println!("FAIL: {}: {}", c.name(), out.why);
            }
        }
        CircuitCmd::CrafterRun { designs, run, margin, sim, trace } => {
            circuit::crafter::TRACE.store(trace, std::sync::atomic::Ordering::Relaxed);
            let all = run == "all";
            let r = if all { circuit::crafter::Run::Base } else { circuit::crafter::Run::parse(&run).ok_or_else(|| anyhow::anyhow!("crafter-run: --run {run}: one of base, intrusion, all"))? };
            if all && !sim {
                anyhow::bail!("crafter-run --run all: redsim only (--sim); the game's whole judge is `mctas circuit verify`");
            }
            let pool = if sim { None } else { Some(mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?) };
            for design in &designs {
                let c = match Circuit::load(design) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {e:#}", design.display());
                        continue;
                    }
                };
                let kind = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
                let out = if all {
                    circuit::crafter::sim_verdict(&c)
                } else {
                    match &pool {
                        None => circuit::crafter::sim_one(&c, r, margin)?,
                        Some(p) => circuit::crafter::run_one(p, &c, r, margin)?,
                    }
                };
                for l in &out.table {
                    println!("  {l}");
                }
                let d = circuit::crafter::dims(&c);
                if out.pass {
                    let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                    println!("CRAFTER: {} {kind} ({run} run{}): {}, {} blocks, {}x{}x{} ({})", c.name(), if sim { ", redsim" } else { "" }, m.join(", "), c.blocks(), d[0], d[1], d[2], d[0] * d[1] * d[2]);
                } else {
                    println!("FAIL: {} ({run} run{}): {}", c.name(), if sim { ", redsim" } else { "" }, out.why);
                }
            }
        }
        CircuitCmd::StationRun { design, turns, offset, margin, trace, sim, builds, compare_verify } => {
            let c = Circuit::load(&design)?;
            let off = [offset.first().copied().unwrap_or(0), offset.get(1).copied().unwrap_or(0), offset.get(2).copied().unwrap_or(0)];
            if compare_verify || builds {
                let o = circuit::verify::Options { margin, version: version.to_string(), ..Default::default() };
                return if compare_verify { twin_compare_verify(&c, &o, circuit::station::verify_in_pool, &circuit::station::sim_verify) } else { print_twin_builds(&c, circuit::station::sim_verify(&c, &o)) };
            }
            let out = if sim {
                let rc = c.turned_pure(turns)?.translated(off)?;
                match circuit::station::sim_run(&rc, margin) {
                    Ok(Ok(log)) => circuit::station::judge(&rc, &log),
                    Ok(Err(why)) => circuit::check::Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
                    Err(why) => circuit::check::Outcome { pass: false, why: format!("redsim does not model it: {why}"), metrics: Vec::new(), table: Vec::new() },
                }
            } else {
                let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
                circuit::station::run_one_rec(&pool, &c, turns, off, margin, trace.as_deref())?
            };
            for l in &out.table {
                println!("  {l}");
            }
            let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
            if out.pass {
                println!("STATION: {} {}, {} blocks: {}", c.name(), c.kind.as_ref().map(|k| k.name()).unwrap_or_default(), c.blocks(), m.join(", "));
            } else {
                println!("FAIL: {}: {}", c.name(), out.why);
            }
        }
        CircuitCmd::FarmGen { layout, crop, n, len, out } => {
            let crop = if crop.contains(':') { crop } else { format!("minecraft:{crop}") };
            let text = match layout.as_str() {
                "stem-lattice" => circuit::farmgen::stem_lattice(&crop, n),
                "rows" => circuit::farmgen::stem_rows(&crop, n),
                "bamboo-rows" => circuit::farmgen::bamboo_rows(n),
                "bamboo-line" => circuit::farmgen::bamboo_line(n),
                "bamboo-sweeper" => circuit::canefarm::bamboo_sweeper(n, if len > 0 { len } else { n }).to_text(),
                other => anyhow::bail!("farm-gen: unknown layout {other} (stem-lattice, rows, bamboo-rows, bamboo-line, bamboo-sweeper)"),
            };
            std::fs::write(&out, text)?;
            let c = Circuit::load(&out)?;
            println!("wrote {}: {} blocks, {} crops", out.display(), c.blocks(), c.stores.iter().filter(|s| s.role == "crop").count());
        }
        CircuitCmd::FarmRun { design, tiled, trace, turns, offset, rts, ticks, margin, sim, compare, compare_verify, probe } => {
            let c = Circuit::load(&design)?;
            let rts = rts.unwrap_or_else(|| if tiled { circuit::tiled(&c).map(|t| circuit::farm::rts_for(&t)).unwrap_or(circuit::farm::RTS) } else { circuit::farm::rts_for(&c) });
            circuit::farm::TRACE.store(trace, std::sync::atomic::Ordering::Relaxed);
            let off = [offset.first().copied().unwrap_or(0), offset.get(1).copied().unwrap_or(0), offset.get(2).copied().unwrap_or(0)];
            if compare_verify {
                let o = circuit::verify::Options { margin, version: version.to_string(), ..Default::default() };
                return farm_compare_verify(&c, &o);
            }
            if probe {
                let base = if tiled { circuit::tiled(&c)? } else { c.clone() };
                let rc = base.turned_pure(turns)?.translated(off)?;
                let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
                let game = pool.map(vec![0u8], |w, _, _| circuit::farm::game_probe(w, &rc, rts, ticks, margin)).into_iter().next().ok_or_else(|| anyhow::anyhow!("no game run"))??;
                let mut sim: Vec<String> = Vec::new();
                if let Err(why) = circuit::farm::sim_run_probe(&rc, rts, ticks, margin, Some(&mut sim)) {
                    println!("redsim does not model it: {why}");
                }
                match game.iter().zip(&sim).position(|(a, b)| a != b) {
                    None => println!("{}: SAME for {} ticks (game {}, redsim {})", rc.name(), game.len().min(sim.len()).saturating_sub(1), game.len(), sim.len()),
                    Some(t) => {
                        let split = |s: &str| -> Vec<String> { s.split(" | ").flat_map(|part| part.split("; ")).filter(|x| !x.is_empty()).map(String::from).collect() };
                        let (a, b) = (split(&game[t]), split(&sim[t]));
                        println!("{}: tick {t} of the run differs", rc.name());
                        for x in a.iter().filter(|x| !b.contains(x)) {
                            println!("  game only:   {x}");
                        }
                        for x in b.iter().filter(|x| !a.contains(x)) {
                            println!("  redsim only: {x}");
                        }
                        if t > 0 {
                            // which drop (its number and cell: rng::Drop::seeded) each new item is
                            let (bf, bt, seed) = circuit::farm::judge_box(&rc, margin);
                            for (who, list) in [("game", &a), ("redsim", &b)] {
                                for x in list.iter().filter(|x| x.starts_with("minecraft:item ")) {
                                    let v: Vec<f64> = x.split_whitespace().filter_map(|w| w.parse::<f64>().ok()).collect();
                                    if v.len() < 6 {
                                        continue;
                                    }
                                    let found = circuit::farm::which_drop(seed, bf, bt, [v[0], v[1], v[2]]);
                                    let drift = circuit::farm::which_drift(seed, [v[3], v[4], v[5]]);
                                    println!("  {who}'s item {x}: {}; drift of drop {drift:?}", if found.is_empty() { "no drop matches".to_string() } else { found.join(", ") });
                                }
                            }
                            let prev: Vec<String> = split(&game[t - 1]);
                            for x in a.iter().filter(|x| !prev.contains(x)).take(12) {
                                println!("  game, new this tick: {x}");
                            }
                            let ents = |s: &str| s.split(" | ").nth(1).unwrap_or("").to_string();
                            let canes = |s: &str| s.split(" | ").next().unwrap_or("").split("; ").filter(|x| x.contains("sugar_cane") || x.contains("bamboo") || x.contains("kelp")).collect::<Vec<_>>().join("; ");
                            println!("  crop blocks before: {}", canes(&game[t - 1]));
                            println!("  crop blocks now (game):   {}", canes(&game[t]));
                            println!("  crop blocks now (redsim): {}", canes(&sim[t]));
                            println!("  entities before (game):   {}", ents(&game[t - 1]));
                            println!("  entities before (redsim): {}", ents(&sim[t - 1]));
                            println!("  entities now (game):      {}", ents(&game[t]));
                            println!("  entities now (redsim):    {}", ents(&sim[t]));
                            // the containers that differ, over the ticks before (game, redsim)
                            let conts = |s: &str| -> Vec<String> { s.split(" | ").nth(2).unwrap_or("").split("; ").filter(|x| !x.is_empty()).map(String::from).collect() };
                            let differing: Vec<String> = conts(&game[t]).iter().chain(conts(&sim[t]).iter()).filter(|x| !conts(&game[t]).contains(x) || !conts(&sim[t]).contains(x)).filter_map(|x| x.split(']').next().map(|p| format!("{p}]"))).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                            for back in (1..=3usize.min(t)).rev() {
                                for (who, list) in [("game", &game), ("redsim", &sim)] {
                                    let at: Vec<String> = conts(&list[t - back]).into_iter().filter(|x| differing.iter().any(|p| x.starts_with(p.as_str()))).collect();
                                    println!("  containers {back} tick(s) before ({who}): {}", if at.is_empty() { "(empty, no cooldown)".to_string() } else { at.join("; ") });
                                }
                            }
                        }
                    }
                }
                return Ok(());
            }
            if sim || compare {
                let base = if tiled { circuit::tiled(&c)? } else { c.clone() };
                let rc = base.turned_pure(turns)?.translated(off)?;
                let twin = circuit::farm::sim_run(&rc, rts, ticks, margin);
                if compare {
                    let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
                    let (game, gc) = pool
                        .map(vec![0u8], |w, _, _| {
                            let gc = base.rotated(w, turns)?.translated(off)?;
                            Ok((circuit::farm::game_run(w, &gc, rts, ticks, margin)?, gc))
                        })
                        .into_iter()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("no game run"))??;
                    if let Some(d) = circuit::farm::turn_difference(&gc, &rc) {
                        println!("TURN DIFFERS: {}: rotation {turns}: {d}", c.name());
                    }
                    let show = |r: &std::result::Result<circuit::farm::RunLog, String>| match r {
                        Ok(l) => format!("grown {} harvested {} delivered {} held {} wait {} tallest {} full {} intruders {} in chests / {} loose; loose {:?}; lost {:?}; stuck {:?}; restless {:?}; clocked {:?}; left {:?}", l.grown, l.harvested, l.delivered, l.held, l.wait_max, l.height_max, l.full_max, l.intruders_delivered, l.intruders_loose, l.loose, l.lost, l.stuck, l.restless, l.clocked, l.left.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>()),
                        Err(e) => format!("does not run: {e}"),
                    };
                    let g = show(&game);
                    let s = match &twin {
                        Ok(r) => show(r),
                        Err(why) => format!("UNMODELED: {why}"),
                    };
                    println!("game:   {g}");
                    println!("redsim: {s}");
                    let (bf, bt, seed) = circuit::farm::judge_box(&rc, margin);
                    println!("{}: {} (box {bf:?}..{bt:?}, seed {seed})", rc.name(), if g == s { "SAME" } else { "DIFFERENT" });
                    return Ok(());
                }
                let out = match twin {
                    Ok(Ok(log)) => circuit::farm::judge(&rc, &log),
                    Ok(Err(why)) => circuit::check::Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
                    Err(why) => circuit::check::Outcome { pass: false, why: format!("redsim does not model it: {why}"), metrics: Vec::new(), table: Vec::new() },
                };
                for l in &out.table {
                    println!("{l}");
                }
                let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                if out.pass {
                    println!("FARM: {} {}{} (redsim): {}", c.name(), c.kind.as_ref().map(|k| k.name()).unwrap_or_default(), if tiled { " (tiled run)" } else { "" }, m.join(", "));
                } else {
                    println!("FAIL: {} (redsim): {} ({})", c.name(), out.why, m.join(", "));
                }
                return Ok(());
            }
            let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
            let out = circuit::farm::run_one(&pool, &c, tiled, turns, off, rts, ticks, margin)?;
            for l in &out.table {
                println!("{l}");
            }
            let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
            if out.pass {
                println!("FARM: {} {}{}: {}", c.name(), c.kind.as_ref().map(|k| k.name()).unwrap_or_default(), if tiled { " (tiled run)" } else { "" }, m.join(", "));
            } else {
                println!("FAIL: {}: {} ({})", c.name(), out.why, m.join(", "));
            }
        }
    }
    Ok(())
}

/// `circuit farm-run --compare-verify`: the whole harvester judge in the game and its redsim
/// twin, build by build, plus every turned design (and the tiled copies') checked to turn alike
/// without the game. Prints `SAME` or the first differences.
fn lift_compare_verify(c: &Circuit, o: &circuit::verify::Options) -> Result<()> {
    twin_compare_verify(c, o, circuit::lift::verify_in_pool, &circuit::lift::sim_verify)
}

/// One design's `circuit verify` report (`--table`: the base build's table, then the first failing
/// build's), its failing builds and note, then its summary line; whether it failed.
fn print_verify_report(c: &Circuit, rep: &circuit::verify::VerifyReport, table: bool) -> bool {
    if table {
        // the base build's table, else (nothing passes) the first build's: why it fails
        let b = rep.base().or_else(|| rep.builds.first().and_then(|b| b.result.as_ref().ok()));
        for l in b.map(|b| b.outcome.table.as_slice()).unwrap_or(&[]) {
            println!("  {l}");
        }
        // and the first failing build's, when some pass
        if rep.base().is_some()
            && let Some(f) = rep.builds.iter().find(|b| b.result.as_ref().is_ok_and(|b| !b.outcome.pass))
            && let Ok(fo) = &f.result
        {
            println!("  == failing build: rotation {} at {:?}", f.turns, f.offset);
            for l in &fo.outcome.table {
                println!("  {l}");
            }
        }
    }
    if !rep.pass {
        for b in rep.builds.iter().filter(|b| !b.result.as_ref().is_ok_and(|b| b.outcome.pass)).take(3) {
            let why = match &b.result {
                Ok(b) => b.outcome.why.clone(),
                Err(e) => e.clone(),
            };
            println!("  rotation {} at {:?}: {why}", b.turns, b.offset);
        }
    }
    if !rep.note.is_empty() {
        println!("  {}", rep.note);
    }
    println!("{}", rep.summary(c));
    !rep.pass
}

/// A twin judge's report, one line per build and its verdict (`--builds`).
fn print_twin_builds(c: &Circuit, rep: std::result::Result<circuit::verify::VerifyReport, String>) -> Result<()> {
    match rep {
        Ok(rep) => {
            for b in &rep.builds {
                let line = match &b.result {
                    Ok(k) => format!("{} {} [{}]", if k.outcome.pass { "pass" } else { "FAIL" }, k.outcome.why, k.outcome.metrics.iter().map(|(n, v)| format!("{n} {v}")).collect::<Vec<_>>().join(", ")),
                    Err(e) => format!("refused: {e}"),
                };
                println!("  rotation {} at {:?}: {}", b.turns, b.offset, line.chars().take(300).collect::<String>());
            }
            println!("{} (redsim)", rep.summary(c));
        }
        Err(why) => println!("UNMODELED: {}: {why}", c.name()),
    }
    Ok(())
}

type GameVerify = fn(&mctas::sim::Pool, &Circuit, &circuit::verify::Options) -> Result<circuit::verify::VerifyReport>;
type TwinVerify<'a> = &'a dyn Fn(&Circuit, &circuit::verify::Options) -> std::result::Result<circuit::verify::VerifyReport, String>;

/// A judge in the game and on its redsim twin, build by build (`--compare-verify`): the turned
/// builds (the game's `rotated` against `turned_pure`), then every build's verdict, notes and
/// metrics, then the verdict; `SAME` or each difference.
fn twin_compare_verify(c: &Circuit, o: &circuit::verify::Options, game_verify: GameVerify, twin_verify: TwinVerify<'_>) -> Result<()> {
    let pool = circuit::verify::spawn_pool(o)?;
    let mut differ = 0usize;
    for t in 1..4u8 {
        let game = pool.map(vec![t], |w, _, t| c.rotated(w, t)).into_iter().next().ok_or_else(|| anyhow::anyhow!("no turn"))??;
        if let Some(why) = circuit::farm::turn_difference(&game, &c.turned_pure(t)?) {
            differ += 1;
            println!("TURN DIFFERS: rotation {t}: {why}");
        }
    }
    let game = game_verify(&pool, c, o)?;
    let twin = match twin_verify(c, o) {
        Ok(r) => r,
        Err(why) => {
            println!("{}: the twin does not model it: {why}", c.name());
            return Ok(());
        }
    };
    let sig = |b: &circuit::verify::Build| match &b.result {
        Ok(k) => format!("{} {} [{}] {}", if k.outcome.pass { "pass" } else { "FAIL" }, k.outcome.why, k.outcome.metrics.iter().map(|(n, v)| format!("{n} {v}")).collect::<Vec<_>>().join(", "), k.outcome.table.join(" | ")),
        Err(e) => format!("refused: {e}"),
    };
    for (g, s) in game.builds.iter().zip(&twin.builds) {
        let (a, b) = (sig(g), sig(s));
        if a != b || g.turns != s.turns || g.offset != s.offset {
            differ += 1;
            println!("BUILD DIFFERS: rotation {} at {:?}:\n  game:   {a}\n  redsim: {b}", g.turns, g.offset);
        }
    }
    if game.builds.len() != twin.builds.len() {
        differ += 1;
        println!("{} game builds, {} twin builds", game.builds.len(), twin.builds.len());
    }
    let (gs, ts) = (game.summary(c), twin.summary(c));
    if gs != ts || game.note != twin.note {
        differ += 1;
        println!("VERDICT DIFFERS:\n  game:   {gs} ({})\n  redsim: {ts} ({})", game.note, twin.note);
    }
    println!("{}: {} ({} builds; game: {gs})", c.name(), if differ == 0 { "SAME" } else { "DIFFERENT" }, game.builds.len());
    Ok(())
}

fn farm_compare_verify(c: &Circuit, o: &circuit::verify::Options) -> Result<()> {
    let pool = circuit::verify::spawn_pool(o)?;
    let mut differ = 0usize;
    let mut designs = vec![("design", c.clone())];
    if c.tile.is_some() {
        designs.push(("tiled", circuit::tiled(c)?));
    }
    for (label, d) in &designs {
        for t in 1..4u8 {
            let game = pool.map(vec![t], |w, _, t| d.rotated(w, t)).into_iter().next().ok_or_else(|| anyhow::anyhow!("no turn"))??;
            if let Some(why) = circuit::farm::turn_difference(&game, &d.turned_pure(t)?) {
                differ += 1;
                println!("TURN DIFFERS: {label} rotation {t}: {why}");
            }
        }
    }
    let game = circuit::farm::verify_in_pool(&pool, c, o)?;
    let twin = match circuit::farm::sim_verify(c, o) {
        Ok(r) => r,
        Err(why) => {
            println!("{}: the twin does not model it: {why}", c.name());
            return Ok(());
        }
    };
    let sig = |b: &circuit::verify::Build| match &b.result {
        Ok(k) => format!("{} {} [{}] {}", if k.outcome.pass { "pass" } else { "FAIL" }, k.outcome.why, k.outcome.metrics.iter().map(|(n, v)| format!("{n} {v}")).collect::<Vec<_>>().join(", "), k.outcome.table.join(" | ")),
        Err(e) => format!("refused: {e}"),
    };
    for (g, s) in game.builds.iter().zip(&twin.builds) {
        let (a, b) = (sig(g), sig(s));
        if a != b || g.turns != s.turns || g.offset != s.offset {
            differ += 1;
            println!("BUILD DIFFERS: rotation {} at {:?}:\n  game:   {a}\n  redsim: {b}", g.turns, g.offset);
        }
    }
    if game.builds.len() != twin.builds.len() {
        differ += 1;
        println!("{} game builds, {} twin builds", game.builds.len(), twin.builds.len());
    }
    let (gs, ts) = (game.summary(c), twin.summary(c));
    if gs != ts || game.note != twin.note {
        differ += 1;
        println!("VERDICT DIFFERS:\n  game:   {gs} ({})\n  redsim: {ts} ({})", game.note, twin.note);
    }
    let tiled_line = |r: &circuit::verify::VerifyReport| r.base().map(|b| b.outcome.table.iter().filter(|l| l.contains("tileable")).cloned().collect::<Vec<_>>().join(" ")).unwrap_or_default();
    if tiled_line(&game) != tiled_line(&twin) {
        differ += 1;
        println!("TILED DIFFERS:\n  game:   {}\n  redsim: {}", tiled_line(&game), tiled_line(&twin));
    }
    println!("{}: {} ({} builds; game: {gs})", c.name(), if differ == 0 { "SAME" } else { "DIFFERENT" }, game.builds.len());
    Ok(())
}

/// `circuit sim --cells <from>..<to>`: per frame, the cells that changed since the frame before
/// (`[x, y, z] block`, air when a block left), with every input (0/1) and output level.
fn print_cell_changes(t: &circuit::trace::Trace, range: &str) {
    let n = t.cells.len();
    let (a, b) = match range.split_once("..") {
        Some((a, b)) => (a.trim().parse::<usize>().unwrap_or(0), b.trim().parse::<usize>().unwrap_or(n)),
        None => (0, n),
    };
    let short = |s: &str| s.strip_prefix("minecraft:").unwrap_or(s).to_string();
    for f in a.max(1)..b.min(n) {
        let prev: std::collections::HashMap<_, u32> = t.cells[f - 1].iter().copied().collect();
        let cur: std::collections::HashMap<_, u32> = t.cells[f].iter().copied().collect();
        let mut ch: Vec<String> = cur.iter().filter(|(p, id)| prev.get(*p) != Some(*id)).map(|(p, id)| format!("{p:?} {}", short(&t.names[*id as usize]))).collect();
        ch.extend(prev.keys().filter(|p| !cur.contains_key(*p)).map(|p| format!("{p:?} air")));
        ch.sort();
        let ins: String = t.inputs.iter().map(|v| if v.get(f).copied().unwrap_or(false) { '1' } else { '0' }).collect();
        let outs: Vec<String> = t.levels.iter().map(|v| v.get(f).copied().unwrap_or(0).to_string()).collect();
        if !ch.is_empty() || f == a.max(1) {
            println!("  frame {f} in {ins} out {}: {}", outs.join(","), ch.join("; "));
        }
    }
}

/// A palette shorthand's block state (the local census): stone, glass, wire, torch, wtorch-<d>,
/// rep-<d> (delay 1), obs-<d> (d = n, s, e, w; observers also u, d); anything else is a state.
fn palette_state(s: &str) -> String {
    let dir = |c: &str| match c {
        "n" => "north",
        "s" => "south",
        "e" => "east",
        "w" => "west",
        "u" => "up",
        _ => "down",
    };
    match s.trim() {
        "stone" => "minecraft:stone".into(),
        "glass" => "minecraft:glass".into(),
        "wire" => "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]".into(),
        "torch" => "minecraft:redstone_torch[lit=true]".into(),
        t if t.starts_with("wtorch-") => format!("minecraft:redstone_wall_torch[facing={},lit=true]", dir(&t[7..])),
        t if t.starts_with("rep-") => format!("minecraft:repeater[delay=1,facing={},locked=false,powered=false]", dir(&t[4..])),
        t if t.starts_with("obs-") => format!("minecraft:observer[facing={},powered=false]", dir(&t[4..])),
        other => other.to_string(),
    }
}

/// `circuit local-census` / `sorter-census`: the census, its passing designs, the smallest
/// written to `out`.
#[allow(clippy::too_many_arguments)]
fn census_cmd(base: &std::path::Path, remove: usize, cells: &str, palette: &str, keep: &str, same: &str, value: Option<&str>, threads: usize, out: Option<&std::path::Path>) -> Result<()> {
    let c = Circuit::load(base)?;
    let pos = |s: &str| -> Result<[i32; 3]> {
        let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "a cell is x,y,z");
        Ok([v[0], v[1], v[2]])
    };
    let box_ = if let Some(n) = cells.strip_prefix("grow:") {
        let n: i32 = n.parse()?;
        let (lo, hi) = c.bounds();
        ([lo[0] - n, lo[1] - n, lo[2] - n], [hi[0] + n, hi[1] + n, hi[2] + n])
    } else {
        let (a, b) = cells.split_once("..").ok_or_else(|| anyhow::anyhow!("--cells x,y,z..x,y,z or grow:N"))?;
        (pos(a)?, pos(b)?)
    };
    let states: Vec<String> = palette.split(',').map(palette_state).collect();
    let keep: Vec<String> = keep.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let threads = if threads > 0 { threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(6).max(1) };
    let found = if matches!(c.kind, Some(circuit::Kind::Sorter(_))) {
        circuit::sorter::census(&c, remove, box_, &states, threads)?
    } else {
        let same: Vec<String> = same.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let judge = circuit::local::trace_judge_value(&c, &same, value)?;
        circuit::local::census(&c, remove, box_, &states, &keep, threads, &judge)
    };
    println!("local census: {} designs pass (base {} blocks)", found.len(), c.blocks());
    if let (Some(path), Some((n, d))) = (out, found.first()) {
        let mut d = d.clone();
        d.meta.insert("name".into(), format!("{}-{n}", c.name()));
        std::fs::write(path, d.to_text())?;
        println!("wrote {} ({n} blocks)", path.display());
    }
    Ok(())
}
