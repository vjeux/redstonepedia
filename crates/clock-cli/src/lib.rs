//! `mctas clock ...`: the clock study's commands (docs/clocks.md). Argument parsing and dispatch
//! only (compiled unoptimized, like the door CLI crates); the work is in `crates/clock`.

use anyhow::{Result, bail};
use clap::Subcommand;
use clock::sim::{SimOptions, SimReport, Verdict, simulate};
use clock::verify::{VerifyOptions, VerifyReport};
use clock::{Clock, wave};
use serde_json::json;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ClockCmd {
    /// The ambient vibration property of sculk clocks (`clock::vibration`): each clock run on
    /// redsim with and without footsteps, items and blocks placed or broken around it (seeded by the
    /// build); `yes` when its output sequence after the warmup is the same. A property, never a
    /// verdict. `--archive`: every archived clock holding a sensor; `--game N`: spot-check N of them
    /// in the game too. Writes the table (`--out`, default designs/clocks/archive/vibration.tsv).
    Vibration {
        designs: Vec<PathBuf>,
        #[arg(long)]
        archive: bool,
        /// Also run this many of them (spread over the list) in the game.
        #[arg(long, default_value_t = 0)]
        game: usize,
        #[arg(long, default_value_t = 8)]
        threads: usize,
        /// JVM workers for `--game` (capped by MCTAS_MAX_WORKERS).
        #[arg(long, default_value_t = 6)]
        workers: usize,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Print each design's first N ambient events (tick, cell, event) instead of the table: for
        /// comparing one build's run on redsim and in the game.
        #[arg(long, default_value_t = 0)]
        events: usize,
    },
    /// Exact period on redsim (milliseconds): place like the judge, kick, run until the whole
    /// state repeats; prints the period, warmup, waveform per changing cell and at the output,
    /// the family, blocks placed and the swept box. Blocks redsim does not model: UNMODELED.
    Sim {
        designs: Vec<PathBuf>,
        /// Ticks to run at most while looking for a repeat.
        #[arg(long, default_value_t = 50_000)]
        horizon: u32,
        /// Free cells simulated around the design (a write beyond = INVALID, escapes).
        #[arg(long, default_value_t = 4)]
        margin: i32,
        /// One JSON object per design.
        #[arg(long)]
        json: bool,
        /// Only the summary line per design.
        #[arg(long, short)]
        quiet: bool,
        /// Print every cell change of the first N ticks instead.
        #[arg(long)]
        trace: Option<u32>,
        /// Only the clocks whose output rises several times per period at unequal spacing (no
        /// single period a builder could time with), one line each, then a count.
        #[arg(long)]
        irregular: bool,
        /// Print the best exterior output cell (an air cell where the judge's lamp gets evenly
        /// spaced pulses, redsim's pick as `clock retap` makes it) and the line with it there.
        #[arg(long)]
        exterior: bool,
    },
    /// The official judge in the real game: N positions x 4 rotations, kick, run to the horizon,
    /// exact periodicity for >= 3 periods after warmup, the same period/waveform/family everywhere
    /// (else position- or rotation-dependent, flagged), no entities, never reaching the edge of
    /// the watched box. Prints one CLOCK line, then the redsim agreement check.
    Verify {
        designs: Vec<PathBuf>,
        /// Test positions (the door judge's offsets; vjeux 2026-10-19: 4 positions x 4 rotations).
        #[arg(long, default_value_t = 4)]
        positions: usize,
        /// Only as built (default: all 4 rotations).
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        /// Watched cells around the box (anything reaching the outer layer fails).
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// JVM workers (capped by MCTAS_MAX_WORKERS).
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Every build's result.
        #[arg(long, short)]
        verbose: bool,
        /// Skip the redsim agreement check.
        #[arg(long)]
        no_sim: bool,
        #[arg(long)]
        json: bool,
        /// Also save, unload and load the chunks again mid-run (after the warmup and 2 periods): the
        /// clock must run at the same state period afterwards, a short glitch allowed (a clock must
        /// survive a chunk reload). redsim judges every build's reload too (`redsim::reload`) and
        /// the game's runs confirm it: a `reload:` line, AGREES or DISAGREES (a failure).
        #[arg(long)]
        reload: bool,
    },
    /// The judge, then keep it: a copy in designs/clocks/archive/ (header: date, source, the
    /// judge's line), its verdict row (verdicts-v1.tsv) and a caption (captions.tsv; default: the
    /// design's first comment block). A clock already archived is skipped. Then run `clock catalog`.
    Archive {
        designs: Vec<PathBuf>,
        /// One line: how it works (default: the design's first comment block).
        #[arg(long)]
        caption: Option<String>,
        /// Keep ONE archived design per type (the earliest archived), remove the others (design,
        /// pictures, rows) and record them in aliases.tsv; no judging.
        #[arg(long)]
        dedupe: bool,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Designs whose neighbour-shaped blocks (a fence's arms, a fence gate's in_wall, a tripwire's
    /// connections) are stored otherwise than a player's placement shapes them (vjeux 2026-10-21:
    /// "why don't the fences connect?"; `clock archive` and `circuit archive` refuse those): each
    /// cell, stored and placed. `--write` rewrites them as placed (the old comments kept).
    Shapes {
        /// Design files (or directories of them).
        designs: Vec<PathBuf>,
        /// Rewrite each design whose neighbour-shaped blocks differ, as placement shapes them.
        #[arg(long)]
        write: bool,
    },
    /// The verdict table's `gaps` column (the ticks between the output's rising edges over one
    /// period), backfilled from redsim for every row whose redsim output reads like the judge's
    /// line; lists the rows it cannot measure. `--write` rewrites verdicts-v1.tsv.
    Gaps {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long)]
        write: bool,
    },
    /// The clocks kept off the catalog and the site (verdicts-v1.tsv's `hidden` column), counted
    /// per reason; `--write` also writes the list to `hidden.md` in the archive directory.
    Hidden {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long)]
        write: bool,
        /// Clear the hidden column of the rows whose reason starts with one of these.
        #[arg(long, num_args = 1..)]
        unhide: Vec<String>,
    },
    /// Per period, the record (the smallest shown clock) and whether a builder can tap its output
    /// (the `usable` column), else the smallest clock of the period that can (hidden ones
    /// included) and the blocks it adds; Markdown to stdout or --out.
    Wireable {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Apply the usable-output rule's best tap to design files (`clock::usable`'s pick at the
    /// period's spacing: the archived record's gap, else the period) and write each tapped design
    /// to OUT/<name>-tap.txt (a joined comparator or observer in it, or its port moved).
    Tap {
        files: Vec<PathBuf>,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// What the one-edge rule moves (the coordinator 2026-10-21: a clock counts for period P only
    /// with one rising edge per P; several edges per cycle list under their even spacing): counts,
    /// the cycles whose record changes and the spacings a moved clock now leads.
    Periods {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        /// Compare with an earlier archive folder: the builder periods whose record or shown set
        /// changed.
        #[arg(long)]
        before: Option<PathBuf>,
    },
    /// Strip clocks to their IRREDUCIBLE core on redsim (vjeux 2026-10-22: "a clock archive
    /// shouldn't be filled with unnecessary clock slop", `clock::strip`): blocks removed one at a
    /// time to a fixed point, then face-adjacent pairs, each removal kept only when the rest gives a
    /// builder the same (builder period, pulse widths, usable output, no new open hopper top, sculk
    /// no less vibration-proof); ports and the kick's block stay. Prints SLOP / IRREDUCIBLE / SKIP
    /// per design and the count by track; `--out` writes each core as <stem>-core.txt and a
    /// strip.tsv. Default: every shown archived clock.
    Strip {
        files: Vec<PathBuf>,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// Single blocks only (no face-adjacent pairs).
        #[arg(long)]
        no_pairs: bool,
        /// Extendable family bases too (stripped as the clock they are at their base n: the
        /// generator owns the members' geometry, so a slop base flags its family).
        #[arg(long)]
        families: bool,
        /// Only the designs redsim cannot judge a removal in (entities, fluids), stripped in the
        /// GAME: each removal judged by the clock judge (a base-build screen, then the full judge
        /// over positions and rotations), redsim's removals tried first.
        #[arg(long)]
        game: bool,
        /// With --game: seconds per design at most (what is left untried is reported).
        #[arg(long, default_value_t = 3000)]
        deadline: u64,
        /// With --game: take redsim's removals from an earlier strip run (its strip.tsv) instead
        /// of stripping on redsim again; designs with a seed go first.
        #[arg(long)]
        seeds: Option<PathBuf>,
        /// Strip the extendable families (`clock::strip::strip_family`): each shown family base's
        /// redsim removals kept only when every member n_lo..=n_hi still works without them; with
        /// --out, the new base designs (`extend remove`, `extend blocks` lowered) and families.tsv.
        #[arg(long)]
        family_check: bool,
        /// Archive the new family bases of a --family-check run (its --out folder): each judged in
        /// the game (the base and the family's sample members, as `clock archive` does), then the
        /// old base removed (aliases.tsv).
        #[arg(long)]
        apply_families: Option<PathBuf>,
        /// Hidden archived clocks too.
        #[arg(long)]
        all: bool,
        /// Exit 1 when a design strips smaller (the archive's guard, a selftest calibration).
        #[arg(long)]
        check: bool,
        /// Skip designs with more blocks.
        #[arg(long)]
        max_blocks: Option<usize>,
        /// Apply a strip run's results (its --out folder): each slop clock removed when an
        /// archived clock that is not slop gives a builder as much with no more blocks than its
        /// core, else its core judged in the game, archived, and the slop clock removed.
        #[arg(long)]
        apply: Option<PathBuf>,
        /// Game workers for --apply.
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Delete the pictures (img/<stem>.jpg, .strip.jpg) of designs no longer in the archive.
        #[arg(long)]
        prune_images: bool,
        /// Summarize a strip run's results (its --out folder) against the archive: slop by track,
        /// by the core's block kinds, the slop clocks whose core is observers only, and the
        /// builder periods whose record is slop.
        #[arg(long)]
        summary: Option<PathBuf>,
    },
    /// Remove archived designs replaced by another (design, pictures, rows of every table), each
    /// recorded in aliases.tsv under the design that stays.
    Remove {
        files: Vec<String>,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        /// The archived design that replaces them.
        #[arg(long)]
        kept: String,
        /// Why, for aliases.tsv.
        #[arg(long)]
        why: String,
    },
    /// Keep archived clocks off the catalog and the site (the `hidden` column) with the reason;
    /// an extendable family's base hides its whole family (its card and every member).
    Hide {
        files: Vec<String>,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        /// The reason, as the page and `clock hidden` list it (`-`: show the clocks again).
        #[arg(long)]
        why: String,
    },
    /// Every shown extendable family's members measured on redsim (the coordinator 2026-10-23: a
    /// member counts for the family's period only when its output gives a builder that period):
    /// each member the site lists generated, its output taken as the site takes it, its builder
    /// period (one-edge rule) compared with the family's claim. Prints the mismatches per family;
    /// `--hide` keeps a family with a mismatch off the catalog and the site.
    FamilyPeriods {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long, default_value_t = 32)]
        threads: usize,
        #[arg(long)]
        hide: bool,
        /// Only these family base files.
        files: Vec<String>,
    },
    /// The ITEMS property of archived clocks (clk-misc's `clock::verify::items_property`): each
    /// clock with an open hopper top judged once in the game (the base build) with items dropped
    /// on its intakes; prints the word, and `--write` puts it in the `items` column (a property,
    /// never a reason to hide).
    Items {
        /// Archived design file names (default: every archived clock with an open hopper top).
        files: Vec<String>,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long)]
        write: bool,
        /// Only list the clocks with an open hopper top.
        #[arg(long)]
        list: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// A three-way merge of verdict tables by design file (git's line union doubles the table):
    /// our changed rows, else their changed rows (of duplicates, the one with the most columns),
    /// else ours. Writes OUT and prints the counts.
    MergeRows {
        #[arg(long)]
        base: PathBuf,
        #[arg(long)]
        ours: PathBuf,
        #[arg(long)]
        theirs: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Every MINIMAL census clock of the periods (designs/clocks/census/*/minimal.tsv) through
    /// the usable pick on redsim: per period the smallest a builder can tap, its tapped design in
    /// OUT; compared with the record and the smallest shown wireable clock.
    WireCensus {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long, default_value = "designs/clocks/census")]
        census: PathBuf,
        /// Comma list of periods (default: every period whose record cannot be tapped).
        #[arg(long)]
        periods: Option<String>,
        #[arg(long, default_value_t = 32)]
        threads: usize,
        /// Screen every design file under these directories instead of the census (3 levels).
        #[arg(long, num_args = 1..)]
        pool: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// The wireable search: for each period (`--periods`, default every period whose record cannot
    /// be tapped), `period-search --usable` from its smallest archived clocks (their own block
    /// palette); prints per period the smallest find (blocks with its tap) against the record and
    /// the smallest shown wireable clock. Finds go to OUT/P<period>/<seed>/found/.
    WireSearch {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        /// Comma list of periods.
        #[arg(long)]
        periods: Option<String>,
        /// Seeds per period (its smallest archived clocks).
        #[arg(long, default_value_t = 3)]
        seeds: usize,
        #[arg(long, default_value_t = 2)]
        grow: i32,
        #[arg(long, default_value_t = 30_000)]
        iterations: usize,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 3000)]
        stale: usize,
        #[arg(long, default_value_t = 40)]
        repair: usize,
        #[arg(long, default_value_t = 400)]
        horizon: u32,
        /// Block states added to every seed's palette, `;`-separated (`minecraft:` optional; a
        /// state with `facing=` is added in all six facings).
        #[arg(long)]
        extra: Option<String>,
        /// Search nothing: count the finds a stopped run left in OUT.
        #[arg(long)]
        summarize: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Archived clocks whose output pulses unevenly: move `port output` to a cell of the clock
    /// that pulses evenly (redsim picks the longest such period), judge again in the game, and with
    /// `--write` replace the archived design and its verdict row when the judge confirms the same
    /// period with an even output (coordinator, 2026-10-20). Default: every archived clock.
    Retap {
        /// Archived design file names (default: every row of the verdict table).
        files: Vec<String>,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long)]
        write: bool,
        /// Only the files of shard i of n (`i/n`, by row order): run shards in parallel, each with
        /// its own --results file, then --apply them all.
        #[arg(long)]
        shard: Option<String>,
        /// Append one result line per file here instead of writing the archive (for --apply).
        #[arg(long)]
        results: Option<PathBuf>,
        /// Apply results files to the archive (designs, rows, the hidden column) and print counts.
        #[arg(long, num_args = 1..)]
        apply: Vec<PathBuf>,
        /// The chunk-reload pass instead: each clock still shown is judged in the game with a chunk
        /// reload mid-run (`clock verify --reload`); one that does not survive it is hidden.
        #[arg(long)]
        reload_pass: bool,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// The usable-output rule instead (vjeux 2026-10-21, `clock::usable`): every clock's
        /// output judged on redsim (a repeater in it must carry every pulse); an unusable one moves
        /// to the best usable cell, or keeps its output with a plain warning (the `usable`
        /// column). `--write` writes the designs and rows.
        #[arg(long)]
        usable: bool,
        /// With --usable: judge in the game instead, each period's smallest shown clock (and the
        /// `files` given): its output with the judge's lamp, and the tap run (the repeater in the
        /// output cell, the lamp after it); with --write the rows take the game's word.
        #[arg(long)]
        game: bool,
        /// With --usable: redsim threads.
        #[arg(long, default_value_t = 32)]
        threads: usize,
        /// With --usable: the ground level only (reading A of the rule, before the coordinator's
        /// ruling; for comparison, never written).
        #[arg(long)]
        usable_strict: bool,
        /// With --usable: only the clocks shown on the site.
        #[arg(long)]
        shown: bool,
        /// With --usable: the hidden `no output` clocks instead (clockinfra): each one's best
        /// usable tap on redsim; with --write, the period-improving ones (the smallest per period
        /// that beats the period's smallest shown wireable clock; all of them with --all) are
        /// judged in the game (the clock as tapped, and the tap run) and, when the game agrees,
        /// written with the tap and shown.
        #[arg(long)]
        hidden: bool,
        /// With --usable --hidden --write: every confirmed tap, not only the period-improving ones.
        #[arg(long)]
        all: bool,
        /// With --usable --hidden: only the clocks of periods up to this.
        #[arg(long)]
        max_period: Option<u32>,
    },
    /// Exhaustive census on redsim: every filling of an AxBxC box with kmin..=k blocks of the
    /// palette (every facing), one per class up to translation and the 4 rotations, judged
    /// self-starting and then with each block placed last; writes <out>/clocks.tsv (every clock:
    /// period, waveform, family, blocks, box, start, type key, canonical encoding), a design per
    /// smallest clock of each (period, family) in <out>/designs/, <out>/summary.md. Resumable
    /// (<out>/parts/, one file per finished chunk). The ground truth of the small clocks.
    /// Structured census of ENTITY clocks on redsim (kits.rs): one mechanism's design space in its
    /// own coordinates, one candidate per mirror class, screened then judged by the real-game
    /// judge's rule for designs with entities. Kits: `shuttle` (K1: a straight track of plain /
    /// powered / detector rails over stone or redstone blocks, bumpers, one minecart). Writes
    /// <out>/clocks.tsv, summary.md, designs/ (every minimal clock).
    Kit {
        kit: String,
        /// The longest track (`shuttle`).
        #[arg(long, default_value_t = 7)]
        max_len: usize,
        /// Ticks of the quick screen (comes to rest / no late block change: dropped).
        #[arg(long, default_value_t = 1000)]
        screen: u32,
        /// Threads (default: every core but 4).
        #[arg(long)]
        workers: Option<usize>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Annealing toward a target period on redsim (clock::period_search, track clk-rt): chains
    /// from each seed clock inside its box grown by --grow, scored by the period and, among
    /// multiples of --period, by the share of changing cells already repeating every --period gt.
    /// Designs of the target period go to DIR/<seed>/found/; DIR/summary.tsv.
    PeriodSearch {
        seeds: Vec<PathBuf>,
        #[arg(long, default_value_t = 3)]
        period: u32,
        /// Palette block states, `;`-separated (default: pistons, sticky pistons, redstone block,
        /// slime, honey, stone, obsidian, dust).
        #[arg(long)]
        palette: Option<String>,
        #[arg(long, default_value_t = 2)]
        grow: i32,
        #[arg(long, default_value_t = 200_000)]
        iterations: usize,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 4000)]
        stale: usize,
        #[arg(long, default_value_t = 40)]
        repair: usize,
        #[arg(long, default_value_t = 400)]
        horizon: u32,
        /// Only designs a builder can tap count as found (tier 4: `clock::usable`'s pick finds a
        /// repeater, comparator or observer tap; the wireable-clocks job).
        #[arg(long)]
        usable: bool,
        #[arg(long)]
        out: PathBuf,
    },
    Census {
        /// Box as AxBxC (x, y, z).
        #[arg(long = "box")]
        dims: String,
        /// Comma list of shorthands (core, stone, glass, rblock, dust, torch, repeater, comparator,
        /// observer, piston, sticky, lamp, bulb, note, target, slime, sculk, csculk, hopper,
        /// dropper, trapdoor, leaves, scaffold...) or block states.
        #[arg(long, default_value = "core")]
        palette: String,
        #[arg(long)]
        k: usize,
        #[arg(long, default_value_t = 1)]
        kmin: usize,
        /// Longest period looked for (the horizon: 4 periods + --warmup gt); a candidate whose
        /// whole state does not repeat within 4x it is dropped early (e.g. `--max-period 3
        /// --warmup 60`: the P <= 3 question).
        #[arg(long, default_value_t = 500)]
        max_period: u32,
        /// Warmup allowance in the horizon (gt).
        #[arg(long, default_value_t = 1000)]
        warmup: u32,
        #[arg(long, default_value_t = 2)]
        margin: i32,
        /// Self-starting only (no block placed last).
        #[arg(long)]
        no_kick: bool,
        /// Threads (default: every core but 4).
        #[arg(long)]
        workers: Option<usize>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Pictures in the REAL client (one session for all): each clock in its own sandbox on a grass
    /// floor at noon, stepped to the most active moment of a period after its warmup (or --tick),
    /// a still camera framing its whole swept box; saved as <out-dir>/<design stem>.jpg (--width
    /// px). --strip N: also N moments of one period tiled (<stem>.strip.jpg). The catalog shows
    /// them. Uses an X display: run it on a box where you may (docs/clocks.md).
    Shot {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = clock::shot::IMG_DIR)]
        out_dir: PathBuf,
        #[arg(long)]
        tick: Option<u32>,
        #[arg(long)]
        strip: Option<u32>,
        /// Every tick of A..B pictured as well (`<stem>.t<tick>.jpg`, tiled as the strip): real-game
        /// frames tick by tick (an explosion's particles).
        #[arg(long, value_parser = parse_tick_range)]
        ticks: Option<(u32, u32)>,
        #[arg(long, default_value_t = 800)]
        width: u32,
        /// Wait after the camera enters a clock's sandbox (ms; the default shooter: two sandboxes
        /// used in turn, ~2 s a clock).
        #[arg(long, default_value_t = 1500)]
        hold_ms: u64,
        /// One-world shooter: wait after a camera move (ms, plus 15 ms per block moved).
        #[arg(long, default_value_t = 700)]
        settle_ms: u64,
        /// EXPERIMENTAL, not recommended: ONE world per batch of clocks (staggered starts, only the
        /// camera moves); neighbors show in the frames. The default shooter is as fast now (~2 s a
        /// clock) with every clock alone on its floor.
        #[arg(long)]
        grid: bool,
        /// Take pictures that exist again.
        #[arg(long)]
        redo: bool,
        /// Take again only the pictures that are missing or bad (nearly uniform, dark, or with a
        /// dark band across the top).
        #[arg(long)]
        redo_bad: bool,
        /// Picture each design exactly as written: placed like the judge, no kick, nothing
        /// stepped, no marker, framing its blocks (a still frame exported by `mctas web snapshot`,
        /// compared by `mctas web compare --still`: the viewer against the game on the same
        /// blocks).
        #[arg(long)]
        still: bool,
    },
    /// Compare the pictures of a directory with the same-named ones of another (SSIM, lowest
    /// first): checks one shooter's output against a trusted set.
    ShotCompare {
        dir: PathBuf,
        #[arg(long, default_value = clock::shot::IMG_DIR)]
        against: PathBuf,
        /// Print only the pictures below this SSIM.
        #[arg(long, default_value_t = 1.0)]
        below: f64,
    },
    /// (videos: the real client) For each period, every given clock's labeled video (each filmed
    /// alone, >= 2 periods; clips already filmed are reused) after a title card, concatenated into
    /// <out-dir>/montage-p<P>.mp4 (the per-clock videos stay in <out-dir>/clips/).
    Montage {
        designs: Vec<PathBuf>,
        #[arg(long)]
        out_dir: PathBuf,
        /// Only these periods (comma list).
        #[arg(long, value_delimiter = ',')]
        periods: Vec<u32>,
        #[arg(long, default_value_t = 6.0)]
        min_secs: f64,
    },
    /// (COORDINATOR ONLY: the X displays are shared) The clock running in the real client, a
    /// still camera framing its swept box, >= 2 periods (and >= --min-secs of footage) after its
    /// warmup, with a corner label (period; family, blocks, box; duty, start; name) and an optional
    /// title card; H.264. The clean clip is kept as <out>.clip.mp4 (--relabel recuts from it), its
    /// first frame as <out>.png. --dry-run prints the plan and writes the label image only.
    Video {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 2)]
        periods: u32,
        #[arg(long, default_value_t = 8.0)]
        min_secs: f64,
        /// Ticks per second while filming (default 10; faster for long periods, <= 60 s).
        #[arg(long)]
        tps: Option<f64>,
        #[arg(long)]
        card: bool,
        #[arg(long)]
        relabel: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Merge census runs (their parts/ and params.txt) into one report: per (period, family) the
    /// fewest blocks among MINIMAL clocks, its box and census, the minimal type count; the periods
    /// never found below the largest. Writes Markdown to --out (default: stdout).
    CensusReport {
        dirs: Vec<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Set the catalog card group of extendable family base designs (archived copies, track
    /// sources): `extend group: <id>` (+ optional title, general period formula, note) after their
    /// `extend family:` line; families of one group share one card. `--clear` removes the group.
    Group {
        /// Family base designs (files with `extend family:`).
        designs: Vec<PathBuf>,
        /// The group id (e.g. `clk-item-dropring`).
        #[arg(long)]
        id: Option<String>,
        /// The card's title (e.g. "dropper ring").
        #[arg(long)]
        title: Option<String>,
        /// The general period formula, the card's heading (e.g. "20 + 14 (W - 3) + ...").
        #[arg(long)]
        period: Option<String>,
        /// One line under the title (e.g. "every P >= 42 together").
        #[arg(long)]
        note: Option<String>,
        /// Remove the group headers.
        #[arg(long)]
        clear: bool,
    },
    /// Judge instances of an extendable family (a design with `extend` headers) in the game:
    /// build each n with its generator, verify it, compare period and blocks with the formulas.
    ExtendCheck {
        design: PathBuf,
        /// `a..b`, `a,b,c` or one n (default: low, low+1, low+2, middle, high of `extend n:`).
        #[arg(long)]
        n: Option<String>,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Record the judged instances in the archive's extend.tsv (the catalog then leaves them
        /// out of the per-period gallery).
        #[arg(long)]
        record: bool,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        /// Ticks the judge runs each build (default: the judge's own, warm-up + 4 periods capped
        /// at 100 000 gt; a longer family member needs it said: warm-up + 4 periods).
        #[arg(long)]
        horizon: Option<u32>,
    },
    /// Write designs/clocks/CATALOG.md (per period: the distinct types, families and the smallest
    /// of each; the period x family coverage matrix; the impossibility table from
    /// designs/clocks/impossible.tsv) and designs/clocks/coverage.md (the matrix alone, mirrored
    /// on the board) from the archive's verdicts. Runs no game.
    Catalog {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long, default_value = clock::catalog::CATALOG)]
        out: PathBuf,
        #[arg(long, default_value = clock::catalog::COVERAGE)]
        coverage: PathBuf,
    },
    /// The archive tables' duplicate keys (clock::tables): a union merge keeps both versions of a
    /// row one branch updated. Lists them (exit 1 when any); `--fix` keeps one row per key (the
    /// most columns, the newest stamp or the design's current hash, the side the merge changed,
    /// else the last, reported): the command to run after every merge. `clock catalog` and
    /// `circuit catalog` run the fix too.
    Tables {
        /// The designs folder (its clocks/archive and circuits/*/archive tables).
        #[arg(long, default_value = "designs")]
        root: PathBuf,
        #[arg(long)]
        fix: bool,
        /// With --fix: print what would change, write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Track clk-obs's commands (crates/clock-obs: observer / piston / bulb / note block clocks).
    Obs {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track clk-rt's commands (crates/clock-rt: torch / repeater / comparator clocks).
    Rt {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track clk-sculk's commands (crates/clock-sculk: sculk / vibration clocks).
    Sculk {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track clk-item's commands (crates/clock-item: hopper / dropper / item clocks).
    Item {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track clk-misc's commands (crates/clock-misc: crafters, rails, doors, fence gates, heads and
    /// other blocks no other track covers).
    Misc {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

pub fn run(version: &str, cmd: ClockCmd) -> Result<()> {
    if let ClockCmd::Vibration { designs, archive, game, threads, workers, out, events } = cmd {
        if events > 0 {
            for path in &designs {
                let c = Clock::load(path)?;
                let r = simulate(&c, &SimOptions::default());
                let ticks = clock::vibration::horizon(r.warmup, r.period);
                println!("{}: warmup {} period {} output {:?}, swept {:?}", c.name(), r.warmup, r.period, r.output.as_ref().map(|o| o.pos), r.swept);
                for (t, a) in clock::vibration::ambient_actions(&c, r.swept, ticks).into_iter().take(events) {
                    if let clock::Action::GameEvent { pos, event } = a {
                        println!("  tick {t}: {event} at {pos:?}");
                    }
                }
            }
            return Ok(());
        }
        return vibration_cmd(version, designs, archive, game, threads, workers, out);
    }
    match cmd {
        ClockCmd::Vibration { .. } => unreachable!("handled above"),
        ClockCmd::Sim { designs, horizon, margin, json, quiet, trace, irregular, exterior } => {
            if designs.is_empty() {
                bail!("clock sim <design>...");
            }
            let o = SimOptions { horizon, margin };
            let (mut clocks, mut odd, mut none) = (0usize, 0usize, 0usize);
            for path in &designs {
                let c = Clock::load(path)?;
                if let Some(n) = trace {
                    for l in clock::sim::trace(&c, &o, n) {
                        println!("{l}");
                    }
                    continue;
                }
                let t0 = std::time::Instant::now();
                if exterior {
                    let mut d = c.design.clone();
                    d.ports.remove("output");
                    let free = Clock::from_design(d)?;
                    if quiet {
                        // (with -q: every exterior candidate that gets power, even or not)
                        let base = simulate(&free, &o);
                        println!("{}: {}", c.name(), base.summary());
                        for (q, w) in clock::sim::exterior_scan(&free, base.state_warmup, base.period, base.swept, &base.occupied, true) {
                            println!("  candidate {q:?}: on {}/{}, gaps {:?}{}", w.on, base.period, w.gaps, if w.irregular().is_some() { " (uneven)" } else { "" });
                        }
                    }
                    match clock::sim::lamp_outputs(&free, 1).into_iter().next() {
                        Some((q, r)) => println!("EXTERIOR: {} port output: {} {} {}: {}", c.name(), q[0], q[1], q[2], r.summary()),
                        None => println!("NO EXTERIOR: {}", c.name()),
                    }
                    continue;
                }
                let r = simulate(&c, &o);
                if irregular {
                    if r.verdict == Verdict::Clock {
                        clocks += 1;
                        if let Some(w) = r.output.as_ref().map(|o| &o.wave)
                            && let Some((lo, hi)) = w.irregular()
                        {
                            odd += 1;
                            // a cell a builder could tap instead: regular, the longest own period
                            let tap = r.analysis.cells.iter().filter(|c| c.tappable()).filter_map(|c| c.output_period().map(|p| (p, c.pos))).max();
                            if tap.is_none() {
                                none += 1;
                            }
                            let tap = match tap {
                                Some((p, pos)) => format!("regular tap: {pos:?} every {p} gt"),
                                None => "no regular cell".into(),
                            };
                            println!("IRREGULAR: {} period {} gt, {} rising edges, gaps {:?} ({lo}..{hi} gt); {tap}", c.name(), r.period, w.edges, w.gaps);
                        }
                    }
                    continue;
                }
                if json {
                    println!("{}", sim_json(&c, &r));
                } else if quiet {
                    println!("{}", r.summary());
                } else {
                    print_sim(&c, &r, t0.elapsed().as_secs_f64() * 1000.0);
                }
            }
            if irregular {
                println!("{odd} of {clocks} clocks (redsim CLOCK verdicts) have an irregular output; {none} of them have no regular cell at all");
            }
        }
        ClockCmd::Verify { designs, positions, no_rotations, horizon, margin, workers, verbose, no_sim, json, reload } => {
            if designs.is_empty() {
                bail!("clock verify <design>...");
            }
            let o = VerifyOptions { positions, rotations: !no_rotations, horizon, margin, workers, version: version.to_string(), reload, ..Default::default() };
            let mut failed = false;
            let pool = clock::verify::spawn_pool(&o)?;
            for path in &designs {
                let c = Clock::load(path)?;
                let t0 = std::time::Instant::now();
                let r = clock::verify::verify_in_pool(&pool, &c, &o)?;
                let secs = t0.elapsed().as_secs_f64();
                let sim = (!no_sim).then(|| simulate(&c, &SimOptions::default()));
                if json {
                    println!("{}", verify_json(&r, sim.as_ref()));
                } else {
                    print_verify(&r, verbose, secs, sim.as_ref());
                    // the ITEMS property (the real-use audit's item intrusion: a property, never a
                    // failure), when the clock has an open hopper top
                    if let Some(i) = r.base().and_then(|b| b.items.as_ref()) {
                        println!("ITEMS: {}: {i}", r.name);
                    }
                    // --reload: redsim's verdict on every build's chunk reload, the game's runs
                    // confirming it (printed with --no-sim too: it is the reload check's own)
                    if let Some(a) = r.reload_agreement() {
                        println!("  {a}");
                        failed |= a.contains("DISAGREES");
                    }
                }
                failed |= r.verdict != Verdict::Clock;
            }
            if failed {
                std::process::exit(1);
            }
        }
        ClockCmd::Archive { designs, caption, dedupe, dir, positions, horizon, margin, workers } => {
            if dedupe {
                let img = dir.parent().map(|p| p.join("img")).unwrap_or_else(|| PathBuf::from(clock::shot::IMG_DIR));
                for l in clock::catalog::dedupe(&dir, &img)? {
                    println!("{l}");
                }
                return Ok(());
            }
            if designs.is_empty() {
                bail!("clock archive <design>...");
            }
            let o = VerifyOptions { positions, rotations: true, horizon, margin, workers, version: version.to_string(), ..Default::default() };
            let pool = clock::verify::spawn_pool(&o)?;
            let mut failed = 0;
            for path in &designs {
                match clock::catalog::archive(&pool, path, &o, &dir, caption.as_deref()) {
                    Ok(a) if a.new => println!("ARCHIVED {}: {}", a.file.display(), a.summary),
                    Ok(a) if a.summary.starts_with("same type") => println!("SAME TYPE {}: {}", path.display(), a.summary),
                    Ok(a) => println!("ALREADY {}: {}", a.file.display(), a.summary),
                    Err(e) => {
                        failed += 1;
                        println!("NOT ARCHIVED {}: {e:#}", path.display());
                    }
                }
            }
            if failed > 0 {
                std::process::exit(1);
            }
        }
        ClockCmd::Shapes { designs, write } => {
            let mut files: Vec<PathBuf> = Vec::new();
            for p in designs {
                if p.is_dir() {
                    let mut v: Vec<PathBuf> = std::fs::read_dir(&p)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.extension().is_some_and(|x| x == "txt")).collect();
                    v.sort();
                    files.extend(v);
                } else {
                    files.push(p);
                }
            }
            let (mut off, mut fixed) = (0, 0);
            for f in &files {
                let text = std::fs::read_to_string(f)?;
                let Ok(c) = Clock::from_design(mctas::design::Design::parse(&text)?) else { continue };
                let bad = match clock::sim::shape_mismatches(&c) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if bad.is_empty() {
                    continue;
                }
                off += 1;
                for (p, stored, placed) in &bad {
                    println!("{}: {} {} {}: stored {stored}, placement gives {placed}", f.display(), p[0], p[1], p[2]);
                }
                if write && let Some(new) = clock::sim::with_placed_shapes(&text, &c).map_err(|e| anyhow::anyhow!(e))? {
                    std::fs::write(f, new)?;
                    fixed += 1;
                }
            }
            println!("shapes: {} designs, {off} store a neighbour-shaped block otherwise than placement{}", files.len(), if write { format!(", {fixed} rewritten") } else { String::new() });
        }
        ClockCmd::Gaps { dir, write } => {
            let b = clock::gaps::backfill(&dir, write)?;
            for f in &b.unknown {
                println!("UNMEASURED {f}");
            }
            println!("gaps: {} rows measured ({} uneven), {} not measurable on redsim{}", b.filled, b.uneven, b.unknown.len(), if write { "; verdicts-v1.tsv rewritten" } else { " (report only: --write)" });
        }
        ClockCmd::Tap { files, dir, out } => {
            let rows = clock::catalog::read_rows(&dir)?;
            let gaps = clock::wireable::record_gaps(&rows);
            std::fs::create_dir_all(&out)?;
            for f in &files {
                let mut d = mctas::design::Design::parse(&std::fs::read_to_string(f)?)?;
                d.ports.remove("output");
                let c = Clock::from_design(d)?;
                let r = clock::sim::simulate(&c, &clock::sim::SimOptions::default());
                if r.verdict != Verdict::Clock {
                    println!("NOT A CLOCK {}: {}", f.display(), r.summary());
                    continue;
                }
                let gs = clock::wireable::target_gaps(&gaps, r.period);
                let picked = gs.iter().find_map(|g| clock::usable::pick(&c, clock::usable::Rule::default(), Some(*g)).ok().map(|(u, _)| (*g, u)));
                match picked {
                    Some((g, u)) => {
                        let mut t = clock::usable::tapped_design(&c, u.tap)?;
                        let stem = f.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                        t.design.name = format!("{stem}-tap");
                        let p = out.join(format!("{}.txt", t.design.name));
                        t.design.save(&p)?;
                        println!("TAP {}: P{} every {g} gt, a {} tap, {} blocks: {}", f.display(), r.period, u.tap.kind.word(), t.blocks(), p.display());
                    }
                    None => println!("NO TAP {}: P{} every {gs:?} gt", f.display(), r.period),
                }
            }
        }
        ClockCmd::Periods { dir, before } => {
            let rows = clock::catalog::read_rows(&dir)?;
            if let Some(b) = before {
                let old = clock::catalog::read_rows(&b)?;
                type Rec = (usize, bool, i64, bool, String);
                let key = |r: &clock::catalog::Row| -> Rec { (r.blocks, !r.usable.starts_with("yes"), r.volume, r.file.starts_with("census-"), r.file.clone()) };
                let index = |rs: &[clock::catalog::Row]| -> std::collections::BTreeMap<u32, (Rec, usize)> {
                    let mut m: std::collections::BTreeMap<u32, (Rec, usize)> = Default::default();
                    for r in rs.iter().filter(|r| r.shown()) {
                        let Some(p) = clock::catalog::builder_period(r) else { continue };
                        let k = key(r);
                        let e = m.entry(p).or_insert((k.clone(), 0));
                        if k < e.0 {
                            e.0 = k;
                        }
                        e.1 += 1;
                    }
                    m
                };
                let (a, z) = (index(&old), index(&rows));
                let ps: std::collections::BTreeSet<u32> = a.keys().chain(z.keys()).copied().collect();
                let (mut rec_changed, mut set_changed) = (Vec::new(), Vec::new());
                for p in ps {
                    match (a.get(&p), z.get(&p)) {
                        (Some(x), Some(y)) => {
                            if x.0.4 != y.0.4 {
                                rec_changed.push(format!("P{p}: {} ({} blocks) -> {} ({} blocks)", x.0.4.trim_end_matches(".txt"), x.0.0, y.0.4.trim_end_matches(".txt"), y.0.0));
                            }
                            if x.1 != y.1 {
                                set_changed.push(format!("P{p} {}->{}", x.1, y.1));
                            }
                        }
                        (Some(x), None) => rec_changed.push(format!("P{p}: {} ({} blocks) -> no shown clock", x.0.4.trim_end_matches(".txt"), x.0.0)),
                        (None, Some(y)) => rec_changed.push(format!("P{p}: new, {} ({} blocks)", y.0.4.trim_end_matches(".txt"), y.0.0)),
                        (None, None) => {}
                    }
                }
                println!("records changed: {}", rec_changed.len());
                for l in &rec_changed {
                    println!("  {l}");
                }
                println!("shown sets changed: {} periods: {}", set_changed.len(), set_changed.join(", "));
                return Ok(());
            }
            for l in clock::catalog::period_rule_report(&rows) {
                println!("{l}");
            }
        }
        ClockCmd::Strip { files, dir, out, threads, no_pairs, all, check, max_blocks, apply, workers, summary, prune_images, families, game, deadline, seeds, family_check, apply_families } => {
            if let Some(results) = apply_families {
                let o = VerifyOptions { workers, version: version.to_string(), ..Default::default() };
                let pool = clock::verify::spawn_pool(&o)?;
                let tsv = std::fs::read_to_string(results.join("families.tsv"))?;
                let (mut replaced, mut kept) = (0, 0);
                for l in tsv.lines().skip(1) {
                    let f: Vec<&str> = l.split('\t').collect();
                    let (Some(file), Some(removed)) = (f.first(), f.get(2)) else { continue };
                    let stem = file.trim_end_matches(".txt");
                    let staged = results.join(format!("{stem}-core.txt"));
                    let caption = format!("{stem} without {removed} (`mctas clock strip --family-check`, 2026-10-23): blocks the generator placed that no member needs.");
                    match clock::catalog::archive(&pool, &staged, &o, &dir, Some(&caption)) {
                        Ok(a) if a.new => {
                            let new_file = a.file.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                            clock::catalog::write_tables(&dir, &[file.to_string()].into_iter().collect())?;
                            let _ = std::fs::remove_file(dir.join(file));
                            let ap = dir.join(clock::catalog::ALIASES);
                            let mut text = std::fs::read_to_string(&ap).unwrap_or_else(|_| "kept\talias_file\talias_name\n".into());
                            text.push_str(&format!("{new_file}\t{file}\t{stem} (slop: the family base with {removed}, which no member needs)\n"));
                            std::fs::write(&ap, text)?;
                            replaced += 1;
                            println!("REPLACED {file} by {new_file}: {}", a.summary);
                        }
                        Ok(a) => {
                            kept += 1;
                            println!("KEPT {file}: {}", a.summary);
                        }
                        Err(e) => {
                            kept += 1;
                            println!("KEPT {file}: its stripped base is not archived: {e:#}");
                        }
                    }
                }
                println!("strip --apply-families: {replaced} family bases replaced, {kept} kept");
                return Ok(());
            }
            if family_check {
                let paths: Vec<PathBuf> = if files.is_empty() {
                    clock::catalog::read_rows(&dir)?.into_iter().filter(|r| all || r.shown()).map(|r| dir.join(&r.file)).collect()
                } else {
                    files
                };
                let bases: Vec<PathBuf> = paths.into_iter().filter(|p| Clock::load(p).is_ok_and(|c| c.design.meta.contains_key("extend family"))).collect();
                if let Some(o) = &out {
                    std::fs::create_dir_all(o)?;
                }
                let mut tsv = String::from("file\tfamily\tremoved\tfewer\tmembers\n");
                let (mut slop, mut irreducible) = (0, 0);
                for p in &bases {
                    let file = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    let t0 = std::time::Instant::now();
                    match clock::strip::strip_family(p, threads) {
                        Err(e) => println!("SKIP {file}: {e:#}"),
                        Ok(None) => {
                            irreducible += 1;
                            println!("IRREDUCIBLE {file}");
                        }
                        Ok(Some(fs)) => {
                            for l in &fs.log {
                                println!("  {file}: {l}");
                            }
                            if fs.removed.is_empty() {
                                irreducible += 1;
                                println!("IRREDUCIBLE {file}: no removal keeps every member ({} members) [{:.0} s]", fs.members, t0.elapsed().as_secs_f64());
                                continue;
                            }
                            slop += 1;
                            let rt = fs.removed.iter().map(|(q, s)| format!("{},{},{} {}", q[0], q[1], q[2], clock::short(s))).collect::<Vec<_>>().join("; ");
                            println!("SLOP {file} (family {}): every member works without {rt} ({} blocks fewer each, {} members n = {}..{}) [{:.0} s]", fs.family.id, fs.fewer, fs.members, fs.family.n_lo, fs.family.n_hi, t0.elapsed().as_secs_f64());
                            tsv.push_str(&format!("{file}\t{}\t{rt}\t{}\t{}\n", fs.family.id, fs.fewer, fs.members));
                            if let Some(o) = &out {
                                let text = std::fs::read_to_string(p)?;
                                let cells: Vec<clock::Pos> = fs.removed.iter().map(|(q, _)| *q).collect();
                                let mut body = clock::strip::family_base_text(&text, &cells, fs.fewer, &fs.family)?;
                                let stem = file.trim_end_matches(".txt");
                                body = body.replacen(&format!("name: {}", Clock::load(p)?.name()), &format!("name: {stem}-core"), 1);
                                std::fs::write(o.join(format!("{stem}-core.txt")), body)?;
                            }
                        }
                    }
                }
                if let Some(o) = &out {
                    std::fs::write(o.join("families.tsv"), tsv)?;
                }
                println!("strip --family-check: {} family bases: {slop} slop, {irreducible} irreducible", bases.len());
                return Ok(());
            }
            if game {
                // the designs redsim cannot judge a removal in, stripped in the game: `threads`
                // designs at once, each with its own pool of `workers`
                let out = out.ok_or_else(|| anyhow::anyhow!("--game needs --out"))?;
                std::fs::create_dir_all(&out)?;
                let paths: Vec<PathBuf> = if files.is_empty() {
                    clock::catalog::read_rows(&dir)?.into_iter().filter(|r| all || r.shown()).map(|r| dir.join(&r.file)).collect()
                } else {
                    files
                };
                let mut jobs: Vec<PathBuf> = paths
                    .into_iter()
                    .filter(|p| Clock::load(p).is_ok_and(|c| clock::strip::redsim_inexact(&c).is_some() && (families || !c.design.meta.contains_key("extend family"))))
                    .collect();
                // redsim's removals from an earlier run: `x,y,z state; ...` per file
                let seed_of: std::collections::HashMap<String, Vec<clock::Pos>> = match &seeds {
                    Some(d) => std::fs::read_to_string(d.join("strip.tsv"))?
                        .lines()
                        .skip(1)
                        .filter_map(|l| {
                            let f: Vec<&str> = l.split('\t').collect();
                            let cells = f.get(4)?.split("; ").filter_map(|e| {
                                let v: Vec<i32> = e.split_whitespace().next()?.split(',').filter_map(|x| x.parse().ok()).collect();
                                (v.len() == 3).then(|| [v[0], v[1], v[2]])
                            });
                            Some((f.first()?.to_string(), cells.collect()))
                        })
                        .collect(),
                    None => Default::default(),
                };
                let name_of = |p: &PathBuf| p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                jobs.sort_by_key(|p| !seed_of.contains_key(&name_of(p)));
                println!("strip --game: {} designs with entities or fluids", jobs.len());
                let next = std::sync::atomic::AtomicUsize::new(0);
                let lines: std::sync::Mutex<Vec<(String, String)>> = Default::default();
                let tsv_rows: std::sync::Mutex<Vec<String>> = Default::default();
                std::thread::scope(|s| {
                    for _ in 0..threads.max(1) {
                        s.spawn(|| {
                            let o = VerifyOptions { workers, version: version.to_string(), ..Default::default() };
                            let pool = match clock::verify::spawn_pool(&o) {
                                Ok(p) => p,
                                Err(e) => {
                                    lines.lock().unwrap().push((String::new(), format!("ERROR no pool: {e:#}")));
                                    return;
                                }
                            };
                            loop {
                                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some(p) = jobs.get(i) else { break };
                                let file = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                                let t0 = std::time::Instant::now();
                                let Ok(c) = Clock::load(p) else { continue };
                                // redsim's removals as the first candidates (an earlier run's, else
                                // stripped now when redsim runs it)
                                let seed: Vec<clock::Pos> = match seed_of.get(&file) {
                                    Some(s) => s.clone(),
                                    None if seeds.is_some() => Vec::new(),
                                    None => clock::strip::strip(&c, false).map(|st| st.removed.iter().map(|(q, _)| *q).collect()).unwrap_or_default(),
                                };
                                let text = std::fs::read_to_string(p).unwrap_or_default();
                                let track = clock::strip::track_of(&text, &file);
                                let line = match clock::strip::strip_game(&pool, &c, &o, &seed, t0 + std::time::Duration::from_secs(deadline)) {
                                    Err(e) => format!("ERROR {file}: {e:#}"),
                                    Ok(None) => format!("SKIP {file}: no clock with an even output in the game"),
                                    Ok(Some(g)) => {
                                        let secs = t0.elapsed().as_secs_f64();
                                        let left = if g.untried > 0 { format!(", {} blocks untried at the deadline", g.untried) } else { String::new() };
                                        if g.stripped.core.blocks() < c.blocks() {
                                            let (period, widths) = g.core.base().and_then(clock::strip::game_pulse).unwrap_or((0, Vec::new()));
                                            let stem = file.trim_end_matches(".txt");
                                            let mut d = g.stripped.core.design.clone();
                                            d.name = format!("{stem}-core");
                                            let body = format!("# the irreducible core of {file} in the game (mctas clock strip --game, 2026-10-23): removed {}\n{}", g.stripped.removed_text(), d.to_text());
                                            let _ = std::fs::write(out.join(format!("{stem}-core.txt")), body);
                                            let ws: Vec<String> = widths.iter().map(|w| w.to_string()).collect();
                                            tsv_rows.lock().unwrap().push(format!("{file}\t{track}\t{}\t{}\t{}\t{secs:.0}\tgame\t{period}\t{}\n", c.blocks(), g.stripped.core.blocks(), g.stripped.removed_text(), ws.join(",")));
                                            format!("SLOP {file} [{track}]: {} blocks -> {} in the game (removed {}) [{} tries, {secs:.0} s{left}]", c.blocks(), g.stripped.core.blocks(), g.stripped.removed_text(), g.stripped.tries)
                                        } else {
                                            format!("IRREDUCIBLE {file} in the game [{} tries, {secs:.0} s{left}]", g.stripped.tries)
                                        }
                                    }
                                };
                                println!("{line}");
                                lines.lock().unwrap().push((file, line));
                            }
                        });
                    }
                });
                let lines = lines.into_inner().unwrap();
                let mut tsv = String::from("file\ttrack\tblocks\tcore_blocks\tremoved\tseconds\tjudge\tperiod\twidths\n");
                for r in tsv_rows.into_inner().unwrap() {
                    tsv.push_str(&r);
                }
                std::fs::write(out.join("strip.tsv"), tsv)?;
                let count = |w: &str| lines.iter().filter(|(_, l)| l.starts_with(w)).count();
                println!("strip --game: {} designs: {} slop, {} irreducible, {} skipped, {} errors", lines.len(), count("SLOP"), count("IRREDUCIBLE"), count("SKIP"), count("ERROR"));
                return Ok(());
            }
            if prune_images {
                // the pictures of designs removed in favour of another (aliases.tsv's alias files:
                // `clock archive --dedupe`, `clock strip --apply`), not other pictures (a family
                // member's <base>.n<N>.jpg has no design of its own)
                let img = dir.parent().map(|p| p.join("img")).unwrap_or_else(|| PathBuf::from(clock::shot::IMG_DIR));
                let aliases = std::fs::read_to_string(dir.join(clock::catalog::ALIASES)).unwrap_or_default();
                let mut n = 0;
                for l in aliases.lines().skip(1) {
                    let Some(f) = l.split('\t').nth(1) else { continue };
                    if dir.join(f).exists() {
                        continue;
                    }
                    let stem = f.trim_end_matches(".txt");
                    for p in [img.join(format!("{stem}.jpg")), img.join(format!("{stem}.strip.jpg"))] {
                        if p.exists() {
                            std::fs::remove_file(&p)?;
                            n += 1;
                        }
                    }
                }
                println!("prune-images: {n} pictures of designs removed in favour of another deleted from {}", img.display());
                return Ok(());
            }
            if let Some(results) = summary {
                let rows = clock::catalog::read_rows(&dir)?;
                let tsv = std::fs::read_to_string(results.join("strip.tsv"))?;
                let mut by_track: std::collections::BTreeMap<String, usize> = Default::default();
                let mut by_kinds: std::collections::BTreeMap<String, usize> = Default::default();
                let mut by_edges: std::collections::BTreeMap<&str, usize> = Default::default();
                let mut obs_only: Vec<String> = Vec::new();
                let mut one_edge: Vec<String> = Vec::new();
                let mut slop_files: std::collections::BTreeSet<String> = Default::default();
                for l in tsv.lines().skip(1) {
                    let f: Vec<&str> = l.split('\t').collect();
                    let (Some(file), Some(track)) = (f.first(), f.get(1)) else { continue };
                    slop_files.insert(file.to_string());
                    *by_track.entry(track.to_string()).or_default() += 1;
                    let Some(r) = rows.iter().find(|r| r.file == *file) else { continue };
                    *by_edges.entry(if r.edges <= 1 { "one edge per cycle" } else { "several edges per cycle" }).or_default() += 1;
                    let stem = file.trim_end_matches(".txt");
                    let Ok(core) = Clock::load(&results.join(format!("{stem}-core.txt"))) else { continue };
                    let kinds: Vec<String> = core.kinds().into_iter().collect();
                    *by_kinds.entry(kinds.join("+")).or_default() += 1;
                    if r.edges <= 1 {
                        one_edge.push(format!("P{} {stem} ({} blocks -> {}: {})", r.period, r.blocks, core.blocks(), f.get(4).unwrap_or(&"")));
                    }
                    if kinds == ["observer"] {
                        obs_only.push(format!("P{} {stem} ({} blocks -> {}: {})", clock::catalog::builder_period(r).unwrap_or(0), r.blocks, core.blocks(), f.get(4).unwrap_or(&"")));
                    }
                }
                println!("slop: {} clocks", slop_files.len());
                for (t, n) in &by_track {
                    println!("  track {t}: {n}");
                }
                for (e, n) in &by_edges {
                    println!("  {e}: {n}");
                }
                let mut kinds: Vec<(&String, &usize)> = by_kinds.iter().collect();
                kinds.sort_by(|a, b| b.1.cmp(a.1));
                for (k, n) in kinds.iter().take(12) {
                    println!("  core {k}: {n}");
                }
                println!("cores of observers only: {}", obs_only.len());
                println!("slop with one edge per cycle (decoration around a clock of that period): {}", one_edge.len());
                for l in &one_edge {
                    println!("  {l}");
                }
                obs_only.sort_by_key(|l| (l.split_whitespace().next().and_then(|p| p[1..].parse::<u32>().ok()).unwrap_or(0), l.clone()));
                let mut per_p: std::collections::BTreeMap<u32, usize> = Default::default();
                for l in &obs_only {
                    *per_p.entry(l.split_whitespace().next().and_then(|p| p[1..].parse::<u32>().ok()).unwrap_or(0)).or_default() += 1;
                }
                println!("  by builder period: {}", per_p.iter().map(|(p, n)| format!("P{p} {n}")).collect::<Vec<_>>().join(", "));
                for l in &obs_only {
                    println!("  {l}");
                }
                // the builder periods whose record (the smallest shown clock, a wireable one on a
                // tie) is slop
                let key = |r: &clock::catalog::Row| (r.blocks, !r.usable.starts_with("yes"), r.volume, r.file.starts_with("census-"), r.file.clone());
                let mut rec: std::collections::BTreeMap<u32, &clock::catalog::Row> = Default::default();
                for r in rows.iter().filter(|r| r.shown()) {
                    let Some(p) = clock::catalog::builder_period(r) else { continue };
                    let e = rec.entry(p).or_insert(r);
                    if key(r) < key(e) {
                        *e = r;
                    }
                }
                let slop_records: Vec<String> = rec.iter().filter(|(_, r)| slop_files.contains(&r.file)).map(|(p, r)| format!("{p} ({}, {} blocks)", r.file.trim_end_matches(".txt"), r.blocks)).collect();
                println!("builder periods whose record is slop: {}", slop_records.len());
                for l in &slop_records {
                    println!("  {l}");
                }
                // the same as Markdown, for the record (results/summary.md)
                let mut md = format!("# clock strip, {} slop clocks\n\nBy track: {}.\n\nBy output: {}.\n\n", slop_files.len(), by_track.iter().map(|(t, n)| format!("{t} {n}")).collect::<Vec<_>>().join(", "), by_edges.iter().map(|(e, n)| format!("{e} {n}")).collect::<Vec<_>>().join(", "));
                md.push_str(&format!("## Cores of observers only: {} (the basic observer clock with blocks around it)\n\n", obs_only.len()));
                for l in &obs_only {
                    md.push_str(&format!("- {l}\n"));
                }
                md.push_str(&format!("\n## One edge per cycle: {} (blocks around a clock of the same period)\n\n", one_edge.len()));
                for l in &one_edge {
                    md.push_str(&format!("- {l}\n"));
                }
                std::fs::write(results.join("summary.md"), md)?;
                return Ok(());
            }
            if let Some(results) = apply {
                let o = VerifyOptions { workers, version: version.to_string(), ..Default::default() };
                let pool = if check { None } else { Some(clock::verify::spawn_pool(&o)?) };
                let img = dir.parent().map(|p| p.join("img")).unwrap_or_else(|| PathBuf::from(clock::shot::IMG_DIR));
                let lines = clock::strip::apply(pool.as_ref(), &dir, &img, &results, &o)?;
                for l in &lines {
                    println!("{l}");
                }
                let count = |w: &str| lines.iter().filter(|l| l.starts_with(w)).count();
                println!(
                    "strip --apply{}: {} slop clocks: {} removed, {} replaced by their core, {} kept, {} would need the game",
                    if check { " (dry run: --check)" } else { "" },
                    lines.len(),
                    count("REMOVED"),
                    count("REPLACED"),
                    count("KEPT"),
                    count("ARCHIVE")
                );
                return Ok(());
            }
            // the designs: the files given, else every (shown) archived clock
            let paths: Vec<PathBuf> = if files.is_empty() {
                clock::catalog::read_rows(&dir)?.into_iter().filter(|r| all || r.shown()).map(|r| dir.join(&r.file)).collect()
            } else {
                files
            };
            if let Some(o) = &out {
                std::fs::create_dir_all(o)?;
            }
            // the biggest first (they take longest)
            let mut jobs: Vec<(usize, PathBuf)> = paths.into_iter().map(|p| (Clock::load(&p).map(|c| c.blocks()).unwrap_or(0), p)).collect();
            jobs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            let next = std::sync::atomic::AtomicUsize::new(0);
            let results: std::sync::Mutex<Vec<(PathBuf, String, Result<Option<clock::strip::Stripped>, String>, f64)>> = Default::default();
            std::thread::scope(|s| {
                for _ in 0..threads.max(1) {
                    s.spawn(|| {
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some((n, p)) = jobs.get(i) else { break };
                            let t0 = std::time::Instant::now();
                            let text = std::fs::read_to_string(p).unwrap_or_default();
                            let file = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                            let track = clock::strip::track_of(&text, &file);
                            let r = match Clock::load(p) {
                                Err(e) => Err(format!("does not load: {e:#}")),
                                Ok(_) if max_blocks.is_some_and(|m| *n > m) => Err(format!("{n} blocks, over --max-blocks")),
                                Ok(c) if !families && c.design.meta.contains_key("extend family") => Err("an extendable family's base (its generator owns its geometry)".into()),
                                Ok(c) if clock::strip::redsim_inexact(&c).is_some() => Err(format!("{}: redsim cannot judge a removal exactly", clock::strip::redsim_inexact(&c).unwrap_or_default())),
                                Ok(c) => Ok(clock::strip::strip(&c, !no_pairs)),
                            };
                            results.lock().unwrap().push((p.clone(), track, r, t0.elapsed().as_secs_f64()));
                        }
                    });
                }
            });
            let mut results = results.into_inner().unwrap();
            results.sort_by(|a, b| a.0.cmp(&b.0));
            let mut tsv = String::from("file\ttrack\tblocks\tcore_blocks\tremoved\tseconds\n");
            let mut by_track: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
            let (mut slop, mut irreducible, mut skipped) = (0, 0, 0);
            for (p, track, r, secs) in &results {
                let file = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                let e = by_track.entry(track.clone()).or_default();
                e.1 += 1;
                match r {
                    Err(why) => {
                        skipped += 1;
                        println!("SKIP {file}: {why}");
                    }
                    Ok(None) => {
                        skipped += 1;
                        println!("SKIP {file}: no clock with an even declared output on redsim");
                    }
                    Ok(Some(st)) if st.core.blocks() < Clock::load(p).map(|c| c.blocks()).unwrap_or(0) => {
                        slop += 1;
                        e.0 += 1;
                        let n = Clock::load(p).map(|c| c.blocks()).unwrap_or(0);
                        println!("SLOP {file} [{track}]: {n} blocks -> {} (removed {}) [{} tries, {secs:.1} s]", st.core.blocks(), st.removed_text(), st.tries);
                        tsv.push_str(&format!("{file}\t{track}\t{n}\t{}\t{}\t{secs:.1}\n", st.core.blocks(), st.removed_text()));
                        if let Some(o) = &out {
                            let stem = file.trim_end_matches(".txt");
                            let mut d = st.core.design.clone();
                            d.name = format!("{stem}-core");
                            let text = format!("# the irreducible core of {file} (mctas clock strip, 2026-10-22): removed {}\n{}", st.removed_text(), d.to_text());
                            std::fs::write(o.join(format!("{stem}-core.txt")), text)?;
                        }
                    }
                    Ok(Some(_)) => {
                        irreducible += 1;
                        println!("IRREDUCIBLE {file}");
                    }
                }
            }
            println!("strip: {} designs: {slop} slop (strip smaller), {irreducible} irreducible, {skipped} skipped", results.len());
            for (t, (s, n)) in &by_track {
                if *s > 0 {
                    println!("  {t}: {s} slop of {n}");
                }
            }
            if let Some(o) = &out {
                std::fs::write(o.join("strip.tsv"), tsv)?;
            }
            if check && slop > 0 {
                bail!("{slop} design(s) strip smaller: not irreducible");
            }
        }
        ClockCmd::Remove { files, dir, kept, why } => {
            if !dir.join(&kept).exists() {
                bail!("clock remove: --kept {kept}: not in the archive");
            }
            let gone: std::collections::BTreeSet<String> = files.iter().cloned().collect();
            clock::catalog::write_tables(&dir, &gone)?;
            let img = dir.parent().map(|p| p.join("img")).unwrap_or_else(|| PathBuf::from(clock::shot::IMG_DIR));
            let ap = dir.join(clock::catalog::ALIASES);
            let mut text = std::fs::read_to_string(&ap).unwrap_or_else(|_| "kept\talias_file\talias_name\n".into());
            for f in &files {
                let stem = f.trim_end_matches(".txt");
                for p in [dir.join(f), img.join(format!("{stem}.jpg")), img.join(format!("{stem}.strip.jpg"))] {
                    let _ = std::fs::remove_file(p);
                }
                text.push_str(&format!("{kept}\t{f}\t{stem} ({})\n", why.replace(['\t', '\n'], " ")));
                println!("REMOVED {f}: replaced by {kept}");
            }
            std::fs::write(&ap, text)?;
        }
        ClockCmd::Hide { files, dir, why } => {
            if why.trim().is_empty() {
                bail!("clock hide: --why must say why");
            }
            let mut rows = clock::catalog::read_rows(&dir)?;
            let mut n = 0;
            for f in &files {
                let Some(r) = rows.iter_mut().find(|r| &r.file == f) else { bail!("clock hide: {f}: no verdict row") };
                r.hidden = if why == "-" { String::new() } else { why.replace(['\t', '\n'], " ") };
                n += 1;
                println!("{} {f}: {}", if r.hidden.is_empty() { "SHOWN" } else { "HIDDEN" }, r.hidden);
            }
            clock::gaps::write_rows(&dir, &rows)?;
            println!("hide: {n} clocks kept off the catalog and the site");
        }
        ClockCmd::FamilyPeriods { dir, threads, hide, files } => {
            let rows = clock::catalog::read_rows(&dir)?;
            let instances = clock::extend::read_instances(&dir);
            // the shown family bases and the members the site lists (the range, and the judged
            // instances past it at the formula's period)
            let mut jobs: Vec<(usize, i64, i64)> = Vec::new();
            let mut fams: Vec<(String, clock::extend::Family)> = Vec::new();
            for r in rows.iter().filter(|r| r.shown() && (files.is_empty() || files.contains(&r.file))) {
                let Ok(c) = Clock::load(&dir.join(&r.file)) else { continue };
                let Ok(Some(f)) = clock::extend::Family::of(&c) else { continue };
                let fi = fams.len();
                let mut ns: Vec<i64> = (f.n_lo..=f.n_hi).collect();
                ns.extend(instances.iter().filter(|x| x.family == f.id && (x.n < f.n_lo || x.n > f.n_hi) && f.period.eval(x.n) == x.period as i64).map(|x| x.n));
                ns.sort();
                ns.dedup();
                for n in ns {
                    jobs.push((fi, n, f.period.eval(n).saturating_mul(f.blocks.eval(n).max(1))));
                }
                fams.push((r.file.clone(), f));
            }
            jobs.sort_by_key(|j| std::cmp::Reverse(j.2));
            println!("family-periods: {} shown families, {} members", fams.len(), jobs.len());
            let tmp = mctas::paths::run_dir().join(format!("family-periods-{}", std::process::id()));
            let next = std::sync::atomic::AtomicUsize::new(0);
            let out: std::sync::Mutex<Vec<(usize, clock::extend::Measured)>> = Default::default();
            std::thread::scope(|s| {
                for _ in 0..threads.max(1) {
                    s.spawn(|| {
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some((fi, n, _)) = jobs.get(i) else { break };
                            let (file, f) = &fams[*fi];
                            let m = clock::extend::measure_member(f, &dir.join(file), *n, &tmp.join(format!("{fi}")));
                            out.lock().unwrap().push((*fi, m));
                        }
                    });
                }
            });
            let _ = std::fs::remove_dir_all(&tmp);
            let mut out = out.into_inner().unwrap();
            out.sort_by_key(|(fi, m)| (*fi, m.n));
            let (mut ok_fams, mut bad_fams, mut members_bad, mut members_unmeasured) = (0, 0, 0, 0);
            let mut rows = rows;
            for (fi, (file, f)) in fams.iter().enumerate() {
                let ms: Vec<&clock::extend::Measured> = out.iter().filter(|(i, _)| *i == fi).map(|(_, m)| m).collect();
                let bad: Vec<&&clock::extend::Measured> = ms.iter().filter(|m| m.period.is_some_and(|p| p as i64 != m.claim)).collect();
                let unmeasured: Vec<&&clock::extend::Measured> = ms.iter().filter(|m| m.period.is_none()).collect();
                members_bad += bad.len();
                members_unmeasured += unmeasured.len();
                if bad.is_empty() {
                    ok_fams += 1;
                    if !unmeasured.is_empty() {
                        println!("UNMEASURED {file} (family {}): {} of {} members: {}", f.id, unmeasured.len(), ms.len(), unmeasured.iter().take(3).map(|m| format!("n = {}: {}", m.n, m.why.chars().take(90).collect::<String>())).collect::<Vec<_>>().join("; "));
                    }
                    continue;
                }
                bad_fams += 1;
                let list = bad.iter().take(6).map(|m| format!("n = {}: every {} gt, claims {}", m.n, m.period.unwrap_or(0), m.claim)).collect::<Vec<_>>().join("; ");
                println!("MISMATCH {file} (family {}, P = {}): {} of {} members{}: {list}", f.id, f.period.text(), bad.len(), ms.len(), if unmeasured.is_empty() { String::new() } else { format!(" ({} unmeasured)", unmeasured.len()) });
                if hide && let Some(r) = rows.iter_mut().find(|r| &r.file == file) {
                    let ex = bad.first().map(|m| format!("n = {} pulses every {} gt at its output, the family claims {}", m.n, m.period.unwrap_or(0), m.claim)).unwrap_or_default();
                    r.hidden = format!("wrong period: {} of {} members give a builder another period than P = {} ({ex}; clockinfra 2026-10-23, clock family-periods)", bad.len(), ms.len(), f.period.text());
                }
            }
            if hide {
                clock::gaps::write_rows(&dir, &rows)?;
            }
            println!("family-periods: {ok_fams} families match their claim, {bad_fams} do not ({members_bad} members another period, {members_unmeasured} unmeasured){}", if hide { "; the mismatches hidden" } else { "" });
        }
        ClockCmd::Items { files, dir, write, list, workers } => {
            let mut rows = clock::catalog::read_rows(&dir)?;
            let mut todo: Vec<usize> = Vec::new();
            for (i, r) in rows.iter().enumerate() {
                if !files.is_empty() && !files.contains(&r.file) {
                    continue;
                }
                let Ok(c) = Clock::load(&dir.join(&r.file)) else { continue };
                if !clock::intrusion::intakes(&c.design, &[]).is_empty() {
                    todo.push(i);
                }
            }
            println!("items: {} archived clocks with an open hopper top", todo.len());
            if list {
                for &i in &todo {
                    println!("  {}", rows[i].file);
                }
                return Ok(());
            }
            // the base build only (the property is judged there)
            let o = VerifyOptions { positions: 1, rotations: false, workers, version: version.to_string(), ..Default::default() };
            let pool = clock::verify::spawn_pool(&o)?;
            let (mut ok, mut changed) = (0, 0);
            let mut words: std::collections::HashMap<String, String> = Default::default();
            for (k, &i) in todo.iter().enumerate() {
                let c = Clock::load(&dir.join(&rows[i].file))?;
                let rep = clock::verify::verify_in_pool(&pool, &c, &o)?;
                let word = rep.base().and_then(|b| b.items.clone()).unwrap_or_default();
                if word == "ok" {
                    ok += 1;
                } else if !word.is_empty() {
                    changed += 1;
                }
                println!("ITEMS [{}/{}] {}: {}", k + 1, todo.len(), rows[i].file, if word.is_empty() { format!("not judged ({})", rep.summary()) } else { word.clone() });
                words.insert(rows[i].file.clone(), word);
            }
            println!("items: {ok} ok, {changed} changed by a dropped item, {} not judged", todo.len() - ok - changed);
            if write {
                // the table as it is now (other passes may have written meanwhile), only this column
                rows = clock::catalog::read_rows(&dir)?;
                for r in rows.iter_mut() {
                    if let Some(w) = words.get(&r.file) {
                        r.items = w.clone();
                    }
                }
                clock::gaps::write_rows(&dir, &rows)?;
            }
        }
        ClockCmd::MergeRows { base, ours, theirs, out } => {
            let (t, (oc, tt, dups)) = clock::catalog::merge_rows(&std::fs::read_to_string(&base)?, &std::fs::read_to_string(&ours)?, &std::fs::read_to_string(&theirs)?);
            std::fs::write(&out, &t)?;
            println!("merge-rows: {} rows; {oc} changed on our side, {tt} taken from theirs, {dups} duplicate rows of theirs dropped: {}", t.lines().count().saturating_sub(1), out.display());
        }
        ClockCmd::WireCensus { dir, census, periods, threads, pool, out } => {
            let rows = clock::catalog::read_rows(&dir)?;
            let usable = |r: &clock::catalog::Row| if r.usable.starts_with("yes") { Some(true) } else if r.usable.starts_with("no") { Some(false) } else { None };
            let lines = clock::wireable::report(&rows, usable);
            let ps: Vec<u32> = match periods {
                Some(s) => s.split(',').map(|x| x.trim().parse::<u32>()).collect::<Result<_, _>>()?,
                None => lines.iter().filter(|l| !l.record_usable).map(|l| l.period).collect(),
            };
            let gaps = clock::wireable::record_gaps(&rows);
            let targets: std::collections::HashMap<u32, Vec<u32>> = ps.iter().map(|p| (*p, clock::wireable::target_gaps(&gaps, *p))).collect();
            let best = if pool.is_empty() { clock::wireable::census_screen(&census, &targets, threads, &out)? } else { clock::wireable::pool_screen(&pool, &targets, threads, &out)? };
            for p in &ps {
                let l = lines.iter().find(|l| l.period == *p);
                let rec = l.map(|l| l.record.blocks.to_string()).unwrap_or_default();
                let sw = l.and_then(|l| l.best_usable.as_ref()).map(|r| r.blocks.to_string()).unwrap_or_else(|| "none".into());
                let f = best.iter().find(|b| b.0 == *p).map(|b| format!("{} ({})", b.1, b.2.display())).unwrap_or_else(|| "none".into());
                println!("P{p}: record {rec}, shown wireable {sw}, census wireable {f}");
            }
        }
        ClockCmd::WireSearch { dir, periods, seeds, grow, iterations, workers, rng, stale, repair, horizon, extra, summarize, out } => {
            let rows = clock::catalog::read_rows(&dir)?;
            let usable = |r: &clock::catalog::Row| if r.usable.starts_with("yes") { Some(true) } else if r.usable.starts_with("no") { Some(false) } else { None };
            let lines = clock::wireable::report(&rows, usable);
            let ps: Vec<u32> = match periods {
                Some(s) => s.split(',').map(|x| x.trim().parse::<u32>()).collect::<Result<_, _>>()?,
                None => lines.iter().filter(|l| !l.record_usable).map(|l| l.period).collect(),
            };
            // the extra palette: each state with `minecraft:`, one with a facing in all six
            let mut extra_states: Vec<String> = Vec::new();
            for s in extra.as_deref().unwrap_or_default().split(';').map(str::trim).filter(|s| !s.is_empty()) {
                let s = if s.starts_with("minecraft:") { s.to_string() } else { format!("minecraft:{s}") };
                let d = mctas::design::Design { blocks: [([0, 0, 0], s)].into_iter().collect(), ..Default::default() };
                extra_states.extend(clock::wireable::seed_palette(&d).into_iter().filter(|x| x != "minecraft:stone"));
            }
            let o = clock::wireable::SearchOptions { seeds, grow, iterations, workers, rng, stale, repair, horizon, extra: extra_states, summarize };
            std::fs::create_dir_all(&out)?;
            let finds = clock::wireable::search(&dir, &rows, &ps, &o, &out)?;
            let mut tsv = String::from("period\trecord blocks\tshown wireable blocks\tfound blocks\tfound design\tfinds\n");
            for f in &finds {
                let l = lines.iter().find(|l| l.period == f.period);
                let rec = l.map(|l| l.record.blocks.to_string()).unwrap_or_default();
                let sw = l.and_then(|l| l.best_usable.as_ref()).map(|r| r.blocks.to_string()).unwrap_or_else(|| "none".into());
                let (fb, fp) = f.best.as_ref().map(|(p, b)| (b.to_string(), p.display().to_string())).unwrap_or_else(|| ("none".into(), String::new()));
                println!("P{}: record {rec}, shown wireable {sw}, found {fb} ({} finds) {fp}", f.period, f.found);
                tsv += &format!("{}\t{rec}\t{sw}\t{fb}\t{fp}\t{}\n", f.period, f.found);
            }
            std::fs::write(out.join("summary.tsv"), tsv)?;
        }
        ClockCmd::Wireable { dir, out } => {
            let rows = clock::catalog::read_rows(&dir)?;
            let lines = clock::wireable::report(&rows, |r| {
                if r.usable.starts_with("yes") {
                    Some(true)
                } else if r.usable.starts_with("no") {
                    Some(false)
                } else {
                    None
                }
            });
            let md = clock::wireable::markdown(&lines);
            match out {
                Some(p) => {
                    std::fs::write(&p, &md)?;
                    println!("{}", md.lines().nth(2).unwrap_or_default());
                    println!("wrote {}", p.display());
                }
                None => print!("{md}"),
            }
        }
        ClockCmd::Hidden { dir, write, unhide } => {
            if !unhide.is_empty() {
                let files = clock::gaps::unhide(&dir, &unhide)?;
                println!("unhidden: {} ({})", files.len(), files.join(", "));
            }
            let (md, counts) = clock::gaps::hidden_report(&dir)?;
            for (reason, n) in &counts {
                println!("{n:5} {reason}");
            }
            if write {
                std::fs::write(dir.join("hidden.md"), md)?;
                println!("wrote {}", dir.join("hidden.md").display());
            }
        }
        ClockCmd::Retap { files, dir, write, shard, results, apply, reload_pass, positions, horizon, margin, workers, usable, game, threads, usable_strict, shown, hidden, all, max_period } => {
            if usable {
                let o = VerifyOptions { positions, rotations: true, horizon, margin, workers, version: version.to_string(), ..Default::default() };
                let rule = if usable_strict { clock::usable::Rule::strict() } else { clock::usable::Rule::default() };
                if hidden {
                    return usable_hidden(&dir, files, write, all, threads, max_period, &o);
                }
                return if game { usable_game(&dir, files, write, &o) } else { usable_cmd(&dir, files, write, threads, rule, shown) };
            }
            if !apply.is_empty() {
                let res = clock::gaps::read_results(&apply)?;
                let a = clock::gaps::apply(&dir, &res)?;
                for (kind, files) in &a.hidden {
                    println!("HIDDEN {kind}: {} ({})", files.len(), files.join(", "));
                }
                println!("apply: {} results, {} moved, {} kept, {} hidden", res.len(), a.moved, a.kept, a.hidden.values().map(|v| v.len()).sum::<usize>());
                return Ok(());
            }
            let mut files: Vec<String> = if files.is_empty() { clock::catalog::read_rows(&dir)?.into_iter().map(|r| r.file).collect() } else { files };
            if let Some(s) = &shard {
                let (i, n) = s.split_once('/').and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))).ok_or_else(|| anyhow::anyhow!("--shard i/n"))?;
                files = files.into_iter().enumerate().filter(|(k, _)| k % n == i).map(|(_, f)| f).collect();
            }
            let o = VerifyOptions { positions, rotations: true, horizon, margin, workers, version: version.to_string(), ..Default::default() };
            let pool = clock::verify::spawn_pool(&o)?;
            let mut out = match &results {
                Some(p) => Some(std::fs::OpenOptions::new().create(true).append(true).open(p)?),
                None => None,
            };
            let (mut moved, mut refused, mut notap, mut hidden) = (0, 0, 0, 0);
            let shown: std::collections::BTreeSet<String> = if reload_pass { clock::catalog::read_rows(&dir)?.into_iter().filter(|r| r.shown()).map(|r| r.file).collect() } else { Default::default() };
            // a results file already holding a file's result (a restarted shard): skip it
            let done: std::collections::BTreeSet<String> = match &results {
                Some(p) if p.exists() => clock::gaps::read_results(std::slice::from_ref(p))?.into_keys().collect(),
                _ => Default::default(),
            };
            for f in &files {
                if done.contains(f) {
                    continue;
                }
                let r = if reload_pass {
                    if !shown.contains(f) {
                        continue;
                    }
                    clock::gaps::reload_check(&pool, &o, &dir, f)
                } else {
                    clock::gaps::retap(&pool, &o, &dir, f, write && out.is_none())
                };
                if let (Some(w), Ok(r)) = (out.as_mut(), r.as_ref()) {
                    use std::io::Write as _;
                    writeln!(w, "{}", r.result_line(f))?;
                    w.flush()?;
                }
                match r {
                    Ok(clock::gaps::Retap::Even) => {}
                    Ok(clock::gaps::Retap::Moved { from, to, line, .. }) => {
                        moved += 1;
                        println!("MOVED {f}: output {from:?} -> {to:?}: {line}");
                    }
                    Ok(clock::gaps::Retap::Refused { to, line }) => {
                        refused += 1;
                        println!("REFUSED {f}: output -> {to:?}: {line}");
                    }
                    Ok(clock::gaps::Retap::NoTap(why)) => {
                        notap += 1;
                        println!("NO OUTPUT {f}: {why}");
                    }
                    Ok(clock::gaps::Retap::Hide(why)) => {
                        hidden += 1;
                        println!("HIDE {f}: {why}");
                    }
                    Ok(clock::gaps::Retap::Skip(why)) => println!("SKIP {f}: {why}"),
                    Ok(clock::gaps::Retap::Reload(v)) => {
                        if v != "ok" {
                            println!("RELOAD {f}: {v}");
                        }
                    }
                    Err(e) => println!("ERROR {f}: {e:#}"),
                }
            }
            println!("retap: {moved} moved{}, {refused} refused by the game, {notap} with no output, {hidden} hidden by design", if write || results.is_some() { "" } else { " (report only: --write or --results)" });
        }
        ClockCmd::Shot { designs, out_dir, tick, strip, ticks, width, hold_ms, settle_ms, grid, redo, redo_bad, still } => {
            let mut items = Vec::new();
            let mut hints = std::collections::HashMap::new();
            let build_dir = mctas::paths::run_dir().join(format!("shot-family-{}", std::process::id()));
            for p in &designs {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Some(h) = std::fs::read_to_string(p).ok().as_deref().and_then(clock::shot::judge_hint) {
                    hints.insert(stem.clone(), h);
                }
                let c = Clock::load(p)?;
                // an extendable family's base: its two smallest members are pictured too, built
                // with the family's generator (`<stem>.n<k>.jpg`; the base's own n is `<stem>.jpg`)
                if let Ok(Some(f)) = clock::extend::Family::of(&c) {
                    for n in [f.n_lo, f.n_lo + 1] {
                        if n > f.n_hi || n == f.base {
                            continue;
                        }
                        let mstem = format!("{stem}.n{n}");
                        if !redo && !redo_bad && clock::shot::shot_path(&out_dir, &mstem).exists() {
                            continue;
                        }
                        match f.build(p, n, &build_dir).and_then(|bp| Clock::load(&bp)) {
                            Ok(mc) => items.push((mstem, mc)),
                            Err(e) => println!("SKIP {mstem}: {e:#}"),
                        }
                    }
                }
                items.push((stem, c));
            }
            // --redo-bad: only the pictures that are missing or bad (checked 16 at a time)
            if redo_bad {
                let paths: Vec<std::path::PathBuf> = items.iter().map(|(s, _)| clock::shot::shot_path(&out_dir, s)).collect();
                let bad: Vec<Option<String>> = std::thread::scope(|sc| {
                    let parts: Vec<&[std::path::PathBuf]> = paths.chunks(paths.len().div_ceil(16).max(1)).collect();
                    let hs: Vec<_> = parts.into_iter().map(|part| sc.spawn(move || part.iter().map(|p| clock::shot::bad_picture(p)).collect::<Vec<_>>())).collect();
                    hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
                });
                let mut kept = Vec::new();
                for ((stem, c), why) in items.into_iter().zip(bad) {
                    if let Some(why) = why {
                        println!("BAD {stem}: {why}");
                        kept.push((stem, c));
                    }
                }
                items = kept;
            }
            let o = clock::shot::ShotOptions { out_dir, tick, strip, ticks, width, hold_ms, settle_ms, redo: redo || redo_bad, hints, still, ..Default::default() };
            if still && grid {
                bail!("--still pictures each design alone (no --grid)");
            }
            let lines = if grid && strip.is_none() && ticks.is_none() { clock::shot::shots_grid(version, &items, &o)? } else { clock::shot::shots(version, &items, &o)? };
            for l in lines {
                println!("{l}");
            }
        }
        ClockCmd::ShotCompare { dir, against, below } => {
            let r = clock::shot::compare(&dir, &against)?;
            let n = r.len();
            for (name, v) in r.iter().filter(|(_, v)| *v < below) {
                println!("{v:.3} {name}");
            }
            let mean = if n > 0 { r.iter().map(|(_, v)| v).sum::<f64>() / n as f64 } else { 0.0 };
            println!("{n} pictures compared, mean SSIM {mean:.3}");
        }
        ClockCmd::Montage { designs, out_dir, periods, min_secs } => {
            let mut items = Vec::new();
            for p in &designs {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                items.push((stem, Clock::load(p)?));
            }
            let o = clock::video::ClockVideoOptions { min_secs, ..Default::default() };
            let ps = (!periods.is_empty()).then_some(periods.as_slice());
            for l in clock::video::montage(version, &items, &out_dir, ps, &o)? {
                println!("{l}");
            }
        }
        ClockCmd::Video { design, out, periods, min_secs, tps, card, relabel, dry_run } => {
            let c = Clock::load(&design)?;
            let o = clock::video::ClockVideoOptions { periods, min_secs, tps, card, relabel, dry_run, ..Default::default() };
            let p = clock::video::video(version, &c, &out, &o)?;
            println!(
                "{} {}: P {} gt, skip {} ticks (warmup + kick), film {} ticks ({} periods) at {} tps = {:.1} s{}",
                if dry_run { "PLAN" } else { "VIDEO" },
                out.display(),
                p.period,
                p.skip,
                p.ticks,
                p.ticks / p.period.max(1),
                p.tps,
                p.ticks as f64 / p.tps,
                if dry_run { format!("; label: {}", out.with_extension("label.png").display()) } else { String::new() }
            );
        }
        ClockCmd::CensusReport { dirs, out } => {
            let md = clock::census::report(&dirs)?;
            match out {
                Some(p) => {
                    std::fs::write(&p, &md)?;
                    println!("{}", p.display());
                }
                None => print!("{md}"),
            }
        }
        ClockCmd::Group { designs, id, title, period, note, clear } => {
            if id.is_none() && !clear {
                anyhow::bail!("give --id <group> (or --clear)");
            }
            let id = if clear { None } else { id };
            for d in &designs {
                clock::extend::set_group(d, id.as_deref(), title.as_deref(), period.as_deref(), note.as_deref())?;
                println!("{}: {}", d.display(), id.as_deref().map(|g| format!("extend group: {g}")).unwrap_or_else(|| "group cleared".into()));
            }
        }
        ClockCmd::ExtendCheck { design, n, positions, workers, record, dir, horizon } => {
            let c = Clock::load(&design)?;
            let f = clock::extend::Family::of(&c)?.ok_or_else(|| anyhow::anyhow!("{}: no `extend family:` header", design.display()))?;
            let ns = match &n {
                Some(s) => clock::extend::parse_ns(s)?,
                None => f.sample(),
            };
            let mut o = VerifyOptions { positions, rotations: true, workers, version: version.to_string(), ..Default::default() };
            if let Some(h) = horizon {
                o.horizon = h;
            }
            let pool = clock::verify::spawn_pool(&o)?;
            println!("extend family {}: P = {} gt ({}), blocks = {}, n = {}..{}; judging n = {}", f.id, f.period.text(), f.period.seconds(), f.blocks.text(), f.n_lo, f.n_hi, ns.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "));
            let rows = clock::extend::check(&pool, &design, &f, &ns, &o, false)?;
            let bad = rows.iter().filter(|(_, ok, _)| !ok).count();
            for (_, _, line) in &rows {
                println!("{line}");
            }
            if record {
                let good: Vec<clock::extend::Instance> = rows.iter().filter(|(_, ok, _)| *ok).map(|(i, _, _)| i.clone()).collect();
                clock::extend::record(&dir, &good)?;
                println!("recorded {} instance(s) in {}", good.len(), dir.join(clock::extend::EXTEND_TSV).display());
            }
            if bad > 0 {
                bail!("{bad} of {} instances do not follow the formulas", rows.len());
            }
        }
        ClockCmd::Catalog { dir, out, coverage } => {
            // the tables first: one row per key (a merge may have doubled updated rows)
            if let Some(designs) = dir.parent().and_then(|p| p.parent()) {
                for l in clock::tables::fix_all(designs, false)? {
                    println!("{l}");
                }
            }
            let s = clock::catalog::catalog(&dir, &out, &coverage)?;
            println!("{}: {} clocks, {} types, {} families, {} periods; {}", out.display(), s.clocks, s.types, s.families, s.periods, coverage.display());
        }
        ClockCmd::Tables { root, fix, dry_run } => {
            if fix {
                let lines = clock::tables::fix_all(&root, dry_run)?;
                for l in &lines {
                    println!("{l}");
                }
                println!("tables: {} fixed{}", lines.iter().filter(|l| l.starts_with("FIXED")).count(), if dry_run { " (dry run, nothing written)" } else { "" });
            } else {
                let all = clock::tables::tables(&root);
                let (mut bad, mut orphaned) = (0, 0);
                for t in &all {
                    let d = clock::tables::duplicates(t)?;
                    if let Some((k, n)) = d.first() {
                        bad += 1;
                        println!("DUPLICATE {}: {} key(s) with several rows (first: {} x{n})", t.path.display(), d.len(), k.replace('\t', " "));
                    }
                    let text = std::fs::read_to_string(&t.path)?;
                    let o = clock::tables::orphans(t, &text);
                    if let Some((_, name)) = o.first() {
                        orphaned += 1;
                        println!("ORPHAN {}: {} row(s) of a design or family that is gone (first: {name})", t.path.display(), o.len());
                    }
                }
                println!("tables: {} tables, {bad} with duplicate keys, {orphaned} with orphan rows{}", all.len(), if bad + orphaned > 0 { " (run `mctas clock tables --fix`)" } else { "" });
                if bad + orphaned > 0 {
                    std::process::exit(1);
                }
            }
        }
        ClockCmd::Kit { kit, max_len, screen, workers, out } => {
            let workers = workers.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(4).max(1));
            let o = clock::kits::KitOptions { kit, max_len, workers, out, screen };
            let t0 = std::time::Instant::now();
            let all = clock::kits::run(&o, &|done, total, n| eprintln!("  {done}/{total} candidates, {n} clocks, {:.0} s", t0.elapsed().as_secs_f64()))?;
            let best = clock::kits::best_per_period(&o, &all);
            let (n, written) = clock::kits::write_outputs(&o, &all, &best)?;
            println!("kit {}: {n} clock types, {written} designs (the smallest per period) in {:.1} s: {}", o.kit, t0.elapsed().as_secs_f64(), o.out.display());
        }
        ClockCmd::PeriodSearch { seeds, period, palette, grow, iterations, workers, rng, stale, repair, horizon, usable, out } => {
            let palette: Vec<String> = palette.map(|p| p.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
            let usable: Option<Box<dyn Fn(&Clock) -> bool + Sync>> = if usable { Some(Box::new(|c: &Clock| clock::usable::pick(c, clock::usable::Rule::default(), None).is_ok())) } else { None };
            let o = clock::period_search::Options { period, palette, grow, iterations, workers, rng, stale, repair, horizon, usable };
            clock::period_search::search_all(&seeds, &o, &out)?;
        }
        ClockCmd::Census { dims, palette, k, kmin, max_period, warmup, margin, no_kick, workers, out } => {
            let d: Vec<i32> = dims.split('x').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            if d.len() != 3 || d.iter().any(|x| *x < 1) {
                bail!("--box AxBxC, got {dims:?}");
            }
            let workers = workers.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(4).max(1));
            let o = clock::census::CensusOptions {
                dims: [d[0], d[1], d[2]],
                palette: clock::census::expand_palette(&palette)?,
                k,
                kmin,
                horizon: 4 * max_period + warmup,
                max_period,
                margin,
                kick_last: !no_kick,
                workers,
                out: out.clone(),
            };
            println!("census {}x{}x{} k {}..={} palette {} states, {workers} threads -> {}", d[0], d[1], d[2], kmin, k, o.palette.len(), out.display());
            let t0 = std::time::Instant::now();
            let last = std::sync::Mutex::new(std::time::Instant::now());
            let found = clock::census::census(&o, &|done, total, n| {
                let mut l = last.lock().unwrap();
                if l.elapsed().as_secs() >= 30 || done == total {
                    *l = std::time::Instant::now();
                    eprintln!("  {done}/{total} chunks, {n} clocks, {:.0} s", t0.elapsed().as_secs_f64());
                }
            })?;
            let (n, types, pairs) = clock::census::write_outputs(&o, &found)?;
            println!("{n} clocks, {types} types, {pairs} (period, family) pairs in {:.1} s: {}/clocks.tsv, summary.md, designs/", t0.elapsed().as_secs_f64(), out.display());
        }
        ClockCmd::Obs { args } => clock_obs::run(args)?,
        ClockCmd::Rt { args } => clock_rt::run(args)?,
        ClockCmd::Sculk { args } => clock_sculk::run(args)?,
        ClockCmd::Item { args } => clock_item::run(args)?,
        ClockCmd::Misc { args } => clock_misc::run(args)?,
    }
    Ok(())
}

fn print_sim(c: &Clock, r: &SimReport, ms: f64) {
    println!("{}", r.summary());
    if r.verdict != Verdict::Clock {
        return;
    }
    let (lo, hi) = r.swept;
    println!("  start: {}; swept box {:?}..{:?} (volume {}); {:.1} ms", r.start, lo, hi, r.volume(), ms);
    let hidden = if r.state_period != r.period { format!(" (hidden state: the visible state repeats every {} gt)", r.period) } else { String::new() };
    println!("  the whole state (pending ticks, block entities, torch memory) repeats exactly every {} gt from tick {}{hidden}", r.state_period, r.state_warmup);
    if let Some(o) = &r.output {
        println!(
            "  output {:?}{}: {}",
            o.pos,
            if o.auto { " (auto: no `port output`, the cell with the most edges)" } else { "" },
            o.wave.describe(r.period)
        );
    }
    if !r.analysis.moved.is_empty() {
        println!("  moved only (not in the family): {}", r.analysis.moved.join(", "));
    }
    println!("  {} changing cells:", r.analysis.cells.len());
    for w in &r.analysis.cells {
        let states = if w.states.len() > 6 { format!("{} | ... ({} states)", w.states[..6].join(" | "), w.states.len()) } else { w.states.join(" | ") };
        println!("    {:?} {}: {}", w.pos, states, w.describe(r.period));
    }
    let _ = c;
}

fn wave_json(w: &wave::CellWave) -> serde_json::Value {
    json!({"pos": w.pos, "states": w.states, "kinds": w.kinds, "on": w.on, "edges": w.edges, "pulses": w.widths})
}

fn sim_json(c: &Clock, r: &SimReport) -> serde_json::Value {
    json!({
        "name": c.name(),
        "verdict": r.verdict.word(),
        "why": r.why,
        "period": r.period,
        "state_period": r.state_period,
        "warmup": r.warmup,
        "state_warmup": r.state_warmup,
        "family": wave::signature(&r.analysis.family),
        "blocks": r.blocks,
        "dims": r.dims(),
        "volume": r.volume(),
        "start": r.start,
        "output": r.output.as_ref().map(|o| json!({"pos": o.pos, "auto": o.auto, "on": o.wave.on, "edges": o.wave.edges, "pulses": o.wave.widths, "gaps": o.wave.gaps, "irregular": o.wave.irregular(), "output_period": o.wave.output_period()})),
        "cells": r.analysis.cells.iter().map(wave_json).collect::<Vec<_>>(),
        "summary": r.summary(),
    })
}

fn agreement(r: &VerifyReport, sim: &SimReport) -> String {
    let Some(b) = r.base() else {
        return format!("redsim: {}", sim.summary());
    };
    if sim.verdict == Verdict::Unmodeled {
        return format!("redsim: UNMODELED ({}): the real game is the only judge", sim.why);
    }
    let (son, sedges) = sim.output.as_ref().map(|o| (o.wave.on, o.wave.edges)).unwrap_or((0, 0));
    let (von, vedges) = b.output.as_ref().map(|o| (o.wave.on, o.wave.edges)).unwrap_or((0, 0));
    let same = sim.verdict == Verdict::Clock
        && sim.period == b.period
        && sim.warmup == b.warmup
        && son == von
        && sedges == vedges
        && sim.analysis.family == b.analysis.family
        && sim.dims() == b.dims;
    if same {
        "redsim AGREES (period, warmup, output waveform, family, box)".into()
    } else {
        format!("redsim DISAGREES: {}", sim.summary())
    }
}

fn print_verify(r: &VerifyReport, verbose: bool, secs: f64, sim: Option<&SimReport>) {
    println!("{}", r.summary());
    let n_ok = r.runs.iter().filter(|x| x.result.is_ok()).count();
    println!("  {}; {} builds periodic of {} ({:.1} s)", r.start, n_ok, r.runs.len(), secs);
    for run in &r.runs {
        let line = match &run.result {
            Ok(k) => format!(
                "P={} W={} family {} output {}",
                k.period,
                k.warmup,
                wave::signature(&k.analysis.family),
                k.output.as_ref().map(|o| o.wave.describe(k.period)).unwrap_or_default()
            ),
            Err(e) => format!("FAIL {e}"),
        };
        let show = verbose || run.result.is_err() || r.robustness != "robust";
        if show {
            println!("    rotation {} at {:?}: {line}", run.turns, run.offset);
        }
    }
    if let Some(s) = sim {
        println!("  {}", agreement(r, s));
    }
}

fn verify_json(r: &VerifyReport, sim: Option<&SimReport>) -> serde_json::Value {
    json!({
        "name": r.name,
        "verdict": r.verdict.word(),
        "robustness": r.robustness,
        "summary": r.summary(),
        "runs": r.runs.iter().map(|run| match &run.result {
            Ok(k) => json!({"turns": run.turns, "offset": run.offset, "period": k.period, "warmup": k.warmup, "family": wave::signature(&k.analysis.family), "dims": k.dims,
                "output": k.output.as_ref().map(|o| json!({"pos": o.pos, "on": o.wave.on, "edges": o.wave.edges, "pulses": o.wave.widths}))}),
            Err(e) => json!({"turns": run.turns, "offset": run.offset, "fail": e}),
        }).collect::<Vec<_>>(),
        "redsim": sim.map(|s| agreement(r, s)),
    })
}

/// `mctas clock vibration`: the ambient property table (`clock::vibration`).
fn vibration_cmd(version: &str, designs: Vec<PathBuf>, archive: bool, game: usize, threads: usize, workers: usize, out: Option<PathBuf>) -> Result<()> {
    let dir = PathBuf::from("designs/clocks/archive");
    let mut paths = designs;
    if archive {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        v.sort();
        paths.extend(v);
    }
    if paths.is_empty() {
        bail!("clock vibration <design>... | --archive");
    }
    let t0 = std::time::Instant::now();
    let mut rows = clock::vibration::sim_rows(&paths, threads);
    eprintln!("redsim: {} sculk clocks in {:.0} s", rows.len(), t0.elapsed().as_secs_f64());
    if game > 0 && !rows.is_empty() {
        // spread over the list, the rows that have a redsim property first
        let have: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].sim.is_some()).collect();
        let step = (have.len() / game).max(1);
        let picks: Vec<usize> = have.iter().step_by(step).take(game).copied().collect();
        let gdir = if archive { dir.clone() } else { paths[0].parent().map(|p| p.to_path_buf()).unwrap_or_default() };
        clock::vibration::game_rows(&mut rows, &gdir, &picks, workers, version)?;
    }
    let proof = rows.iter().filter(|r| r.sim.as_ref().is_some_and(|p| p.proof)).count();
    let sensitive = rows.iter().filter(|r| r.sim.as_ref().is_some_and(|p| !p.proof)).count();
    let none = rows.len() - proof - sensitive;
    let agree = rows.iter().filter(|r| r.game.is_some()).filter(|r| r.game.as_ref().map(|g| g.proof) == r.sim.as_ref().map(|s| s.proof)).count();
    let checked = rows.iter().filter(|r| r.game.is_some()).count();
    let mut text = String::from(clock::vibration::TSV_HEADER);
    text.push('\n');
    for r in &rows {
        text.push_str(&r.line());
        text.push('\n');
    }
    let out = out.unwrap_or_else(|| dir.join("vibration.tsv"));
    std::fs::write(&out, text)?;
    println!("{} sculk clocks: {proof} vibration-proof, {sensitive} not, {none} without a property; game spot-check {agree}/{checked} agree with redsim; {}", rows.len(), out.display());
    Ok(())
}

/// `A..B` (or `A..=B`): an inclusive tick range.
fn parse_tick_range(s: &str) -> std::result::Result<(u32, u32), String> {
    let (a, b) = s.split_once("..").ok_or_else(|| format!("{s}: expected A..B"))?;
    let b = b.trim_start_matches('=');
    let (a, b): (u32, u32) = (a.trim().parse().map_err(|e| format!("{a}: {e}"))?, b.trim().parse().map_err(|e| format!("{b}: {e}"))?);
    if a > b {
        return Err(format!("{s}: {a} after {b}"));
    }
    Ok((a, b))
}

/// `clock retap --usable`: every archived clock's output by the usable-output rule on redsim
/// (`clock::usable`), printed; `--write` moves the shown clocks' unusable outputs to their best
/// tap and fills the `usable` column.
fn usable_cmd(dir: &std::path::Path, files: Vec<String>, write: bool, threads: usize, rule: clock::usable::Rule, only_shown: bool) -> Result<()> {
    use clock::usable::Outcome;
    let rows = clock::catalog::read_rows(dir)?;
    let shown: std::collections::HashSet<String> = rows.iter().filter(|r| r.shown()).map(|r| r.file.clone()).collect();
    let files: Vec<String> = if files.is_empty() { rows.iter().map(|r| r.file.clone()).filter(|f| !only_shown || shown.contains(f)).collect() } else { files };
    if write && rule != clock::usable::Rule::default() {
        bail!("--usable-strict is a reading to compare, never written");
    }
    let t0 = std::time::Instant::now();
    let res = clock::usable::retap_all(dir, &files, threads, rule)?;
    let p = |q: &[i32; 3]| format!("{} {} {}", q[0], q[1], q[2]);
    // (all, shown) per outcome; moves by tap
    let mut n = [(0usize, 0usize); 4];
    let mut by_tap: std::collections::BTreeMap<&str, usize> = Default::default();
    for (f, o) in &res {
        let s = shown.contains(f) as usize;
        let k = match o {
            Outcome::Unchanged(_) => 0,
            Outcome::Moved { from, was, tap, stand } => {
                *by_tap.entry(tap.kind.word()).or_default() += 1;
                println!("MOVED {f}: output {} -> {} ({} at {} facing {}, {stand}): {was}", from.as_ref().map(p).unwrap_or_else(|| "none".into()), p(&tap.port()), tap.kind.word(), p(&tap.out), tap.way());
                1
            }
            Outcome::NoUsable(why) => {
                println!("NO USABLE OUTPUT {f}: {why}");
                2
            }
            Outcome::Skip(why) => {
                println!("SKIP {f}: {why}");
                3
            }
        };
        n[k].0 += 1;
        n[k].1 += s;
    }
    let taps: Vec<String> = by_tap.iter().map(|(k, v)| format!("{v} {k}")).collect();
    println!(
        "usable: {} clocks in {:.0} s: {} unchanged (usable already), {} moved ({}), {} with no usable output (a warning), {} not judged; of the {} shown: {} unchanged, {} moved, {} no usable output, {} not judged",
        res.len(),
        t0.elapsed().as_secs_f64(),
        n[0].0,
        n[1].0,
        taps.join(", "),
        n[2].0,
        n[3].0,
        shown.len(),
        n[0].1,
        n[1].1,
        n[2].1,
        n[3].1
    );
    if write {
        let a = clock::usable::apply(dir, &res)?;
        for (f, why) in &a.failed {
            println!("NOT WRITTEN {f}: {why}");
        }
        println!("written: {} unchanged, {} moved, {} no usable output, {} skipped, {} not written", a.unchanged, a.moved, a.no_usable, a.skipped, a.failed.len());
    }
    Ok(())
}

/// `clock retap --usable --hidden` (clockinfra, the wireable-clocks job): the hidden `no output`
/// clocks' best usable taps on redsim (`clock::usable::retap_hidden`); `--write` judges in the game
/// the period-improving ones (per period, the smallest tapped clock that beats the period's
/// smallest shown wireable clock, or the period has none; every one with `--all`) and writes those
/// the game confirms (the clock as tapped is a clock of its period, the tap run carries its
/// pulses): the tap in the design, the row (redsim's numbers, then the game's line and
/// robustness), `usable: yes: <tap>`, shown.
fn usable_hidden(dir: &std::path::Path, files: Vec<String>, write: bool, all: bool, threads: usize, max_period: Option<u32>, o: &VerifyOptions) -> Result<()> {
    use clock::usable::Outcome;
    let rows = clock::catalog::read_rows(dir)?;
    let files: Vec<String> = if files.is_empty() { rows.iter().filter(|r| r.hidden.starts_with("no output") && max_period.is_none_or(|m| r.period <= m)).map(|r| r.file.clone()).collect() } else { files };
    let t0 = std::time::Instant::now();
    // each tap pulses at its period's spacing (the record's gap)
    let gaps = clock::wireable::record_gaps(&rows);
    let res = clock::usable::retap_hidden(dir, &files, threads, clock::usable::Rule::default(), &gaps)?;
    let mut best_shown: std::collections::BTreeMap<u32, usize> = Default::default();
    for r in rows.iter().filter(|r| r.shown() && r.usable.starts_with("yes")) {
        let e = best_shown.entry(r.period).or_insert(r.blocks);
        *e = (*e).min(r.blocks);
    }
    let mut cands: Vec<(u32, usize, String, Outcome)> = Vec::new();
    let (mut no, mut skip) = (0, 0);
    let mut kinds: std::collections::BTreeMap<&str, usize> = Default::default();
    for (f, oc) in &res {
        match oc {
            Outcome::Moved { tap, .. } | Outcome::Unchanged(tap) => {
                let Some(r) = rows.iter().find(|r| &r.file == f) else { continue };
                let tc = clock::usable::tapped_design(&Clock::load(&dir.join(f))?, *tap)?;
                *kinds.entry(tap.kind.word()).or_default() += 1;
                println!("TAP {f}: P{} {} blocks with a {} tap", r.period, tc.blocks(), tap.kind.word());
                cands.push((r.period, tc.blocks(), f.clone(), oc.clone()));
            }
            Outcome::NoUsable(why) => {
                no += 1;
                println!("NO {f}: {}", why.chars().take(160).collect::<String>());
            }
            Outcome::Skip(why) => {
                skip += 1;
                println!("SKIP {f}: {}", why.chars().take(160).collect::<String>());
            }
        }
    }
    cands.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
    let mut picks: Vec<&(u32, usize, String, Outcome)> = Vec::new();
    for c in &cands {
        let beats = best_shown.get(&c.0).is_none_or(|b| c.1 < *b);
        if all || (beats && picks.last().is_none_or(|p| p.0 != c.0)) {
            picks.push(c);
        }
    }
    let new_periods = picks.iter().filter(|c| !rows.iter().any(|r| r.shown() && r.period == c.0)).count();
    println!(
        "usable --hidden: {} clocks in {:.0} s: {} with a usable tap ({}), {no} without, {skip} not judged; {} to judge in the game ({} of them in periods with no shown clock)",
        res.len(),
        t0.elapsed().as_secs_f64(),
        cands.len(),
        kinds.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", "),
        picks.len(),
        new_periods
    );
    if !write {
        return Ok(());
    }
    let pool = clock::verify::spawn_pool(o)?;
    let mut confirmed: Vec<(String, Outcome)> = Vec::new();
    let mut game: std::collections::HashMap<String, clock::verify::VerifyReport> = Default::default();
    for (k, (p, _, f, oc)) in picks.iter().enumerate() {
        let (Outcome::Moved { tap, .. } | Outcome::Unchanged(tap)) = oc else { continue };
        let tc = clock::usable::tapped_design(&Clock::load(&dir.join(f))?, *tap)?;
        // the record's spacing, else once per period (the tap the redsim pass chose meets one)
        let mut res = None;
        for g in clock::wireable::target_gaps(&gaps, *p) {
            let w = clock::usable::game_word(&pool, o, &tc, Some(g));
            let tap_ok = matches!(&w, Ok(x) if x.tap.is_ok());
            res = Some(w);
            if tap_ok {
                break;
            }
        }
        let Some(res) = res else { continue };
        match res {
            Ok(w) => {
                let period_ok = w.lamp.base().is_some_and(|b| b.period == *p);
                match (&w.tap, w.lamp.verdict == Verdict::Clock && period_ok) {
                    (Ok(_), true) => {
                        println!("GAME [{}/{}] {f}: confirmed: {}", k + 1, picks.len(), w.lamp.summary());
                        confirmed.push((f.clone(), oc.clone()));
                        game.insert(f.clone(), w.lamp);
                    }
                    (tap, _) => println!("GAME [{}/{}] {f}: NOT confirmed: {}{}", k + 1, picks.len(), w.lamp.summary(), tap.as_ref().err().map(|e| format!("; {e}")).unwrap_or_default()),
                }
            }
            Err(e) => println!("ERROR {f}: {e:#}"),
        }
    }
    // an Unchanged hidden clock keeps its design: only its row changes (shown, usable)
    let a = clock::usable::apply(dir, &confirmed)?;
    for (f, why) in &a.failed {
        println!("NOT WRITTEN {f}: {why}");
    }
    let mut rows = clock::catalog::read_rows(dir)?;
    let mut shown_now = 0;
    for r in rows.iter_mut() {
        let Some(lamp) = game.get(&r.file) else { continue };
        if a.failed.iter().any(|(f, _)| f == &r.file) {
            continue;
        }
        r.hidden.clear();
        shown_now += 1;
        if let Some(b) = lamp.base() {
            let (on, edges, pulses, gaps) = b.output.as_ref().map(|o| (o.wave.on, o.wave.edges, format!("{:?}", o.wave.widths), format!("{:?}", o.wave.gaps))).unwrap_or_default();
            r.summary = lamp.summary();
            r.robustness = lamp.robustness.clone();
            r.warmup = b.warmup;
            r.on = on;
            r.edges = edges;
            r.pulses = pulses;
            r.gaps = gaps;
        }
    }
    clock::gaps::write_rows(dir, &rows)?;
    println!("written: {} confirmed in the game of {} judged, {shown_now} now shown, {} not written", confirmed.len(), picks.len(), a.failed.len());
    Ok(())
}

/// `clock retap --usable --game`: each period's smallest shown clock (and `extra` files) judged in
/// the game: the clock as archived (the judge's lamp at its port) and the tap run (a repeater at
/// the port); `--write` puts the game's word in the `usable` column (a moved clock's row also
/// takes the game's line and robustness).
fn usable_game(dir: &std::path::Path, extra: Vec<String>, write: bool, o: &VerifyOptions) -> Result<()> {
    let mut rows = clock::catalog::read_rows(dir)?;
    let mut best: std::collections::BTreeMap<u32, (usize, i64, String)> = Default::default();
    for r in rows.iter().filter(|r| r.shown()) {
        let k = (r.blocks, r.volume, r.file.clone());
        let e = best.entry(r.period).or_insert_with(|| k.clone());
        if k < *e {
            *e = k;
        }
    }
    let mut files: Vec<String> = best.into_values().map(|(_, _, f)| f).collect();
    for f in extra {
        if !files.contains(&f) {
            files.push(f);
        }
    }
    let pool = clock::verify::spawn_pool(o)?;
    let (mut yes, mut no, mut refused, mut differ) = (0, 0, 0, 0);
    let t0 = std::time::Instant::now();
    for (k, f) in files.iter().enumerate() {
        let c = Clock::load(&dir.join(f))?;
        let row = rows.iter_mut().find(|r| &r.file == f);
        let gap = row.as_ref().and_then(|r| clock::usable::row_gap(r));
        let redsim = row.as_ref().map(|r| r.usable.clone()).unwrap_or_default();
        let kind = redsim.strip_prefix("yes: ").unwrap_or("repeater").to_string();
        let w = match clock::usable::game_word(&pool, o, &c, gap) {
            Ok(w) => w,
            Err(e) => {
                println!("ERROR {f}: {e:#}");
                continue;
            }
        };
        if w.lamp.verdict != Verdict::Clock {
            refused += 1;
            println!("REFUSED {f}: the clock as archived: {}", w.lamp.summary());
            continue;
        }
        let word = w.usable(&kind);
        let ok = word.starts_with("yes");
        if ok {
            yes += 1;
        } else {
            no += 1;
        }
        let agree = if redsim.is_empty() || redsim.starts_with("yes") == ok {
            ""
        } else {
            differ += 1;
            " (redsim said otherwise)"
        };
        println!("GAME [{}/{}] {f}: {word}{agree}", k + 1, files.len());
        if write && let Some(r) = row {
            let moved = std::fs::read_to_string(dir.join(f))?.contains("port output moved 2026-10-21 (usable-output rule)");
            if moved && let Some(b) = w.lamp.base() {
                // the game's numbers and robustness; the row keeps its mechanism family, blocks and
                // box (a joined tap's block changes state too: the family stays the mechanism's)
                let (on, edges, pulses, gaps) = b.output.as_ref().map(|o| (o.wave.on, o.wave.edges, format!("{:?}", o.wave.widths), format!("{:?}", o.wave.gaps))).unwrap_or_default();
                let summary = clock::usable::patch_line(&r.summary, on, edges, b.warmup, r.blocks, &r.dims);
                r.summary = match summary.rsplit_once(", ") {
                    Some((head, tail)) if tail == r.robustness => format!("{head}, {}", w.lamp.robustness),
                    _ => summary,
                };
                r.robustness = w.lamp.robustness.clone();
                r.warmup = b.warmup;
                r.on = on;
                r.edges = edges;
                r.pulses = pulses;
                r.gaps = gaps;
            }
            // the game's word when the game ran a tap; a clock redsim finds no tap for keeps its
            // row's reason (every candidate's, not only the declared output's)
            if !(word.starts_with("no: ") && word.ends_with(" (redsim)") && r.usable.starts_with("no: ")) {
                r.usable = word;
            }
        }
    }
    println!("usable --game: {} clocks in {:.0} s: {yes} usable in the game, {no} not, {refused} not a clock as archived, {differ} where redsim said otherwise", files.len(), t0.elapsed().as_secs_f64());
    if write {
        clock::gaps::write_rows(dir, &rows)?;
    }
    Ok(())
}
