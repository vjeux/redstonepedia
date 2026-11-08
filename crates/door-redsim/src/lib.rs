//! `mctas door` commands: redsim-based tools: cosearch, cycle, sheet, redsim-fuzz, redsim-case, redsim-verify, pistfuzz.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
// The command bodies (cmd/*.rs) name piston-door's modules `crate::<module>` through this glob.
use piston_door::*;

mod cmd;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Exhaustive REGION FILLING judged as a real door on redsim (fill.rs, track shrink): the
    /// region's cells are cleared, every filling of kmin..=k of them from a palette (+ `--fix`
    /// cells always filled) is screened by the redsim judge at the first position, passes are
    /// judged at 4 + --extra positions with FULL seamless, the hallway check and frame 12/12. No
    /// ideal plan: the rest of the door stays real wiring, so a region other wiring watches needs
    /// no region closure (cosearch pockets' limit on e88-t7b's bottom).
    Fill(cmd::fill::FillArgs),
    /// Flip a design's lever N times in redsim at the judge's 4 positions and check it alternates
    /// between an open state (opening clear, frame 12/12) and a closed one (cycle.rs). Accepts a
    /// door that rests open, e.g. a community design read from a screenshot of its open state.
    Cycle(cmd::judge::CycleArgs),
    /// Draw mechanisms by eye (sheet.rs): for each ideal schedule (or design), the door's plane at
    /// rest and once the opening settles (replayed in redsim, no Minecraft), as one markdown page.
    Sheet(cmd::ideal::SheetArgs),
    /// Pure-Rust door judge on redsim (no JVM: the judge's checks, FULL seamless, the hallway
    /// check, the volume), its agreement with the real judge, and the joint mechanism + wiring
    /// search on top of it (track cosearch, cosearch.rs / cosearch_search.rs).
    Cosearch(cmd::cosearch::CosearchArgs),
    /// Differential fuzzing of the redsim model (crates/redsim) against the real game: random
    /// circuits run in both, every block state compared after placement and every tick and flip;
    /// mismatches are shrunk and saved as case files (`door redsim-case` replays one).
    RedsimFuzz(cmd::fuzz::RedsimFuzzArgs),
    /// Replay a redsim case file in the game and in redsim; print the differing frames.
    RedsimCase(cmd::fuzz::RedsimCaseArgs),
    /// Exact real-game traces (`.trace`: `clock misc trace-export --exact`, the rail and farm
    /// judges' `--trace`) replayed on redsim, every cell and entity compared every tick, on every
    /// core (`redsim-fuzz trace-compare`); one line per trace, then `N of M traces
    /// tick-identical`; exits 1 unless all are. Takes trace files or directories of them.
    TraceCompare(cmd::fuzz::TraceCompareArgs),
    /// Judge designs in redsim (crates/redsim) instead of the game: the judge's checks at the
    /// default positions (+ --extra-offsets), microseconds per tick. With --judge, also run the
    /// real judge and report any verdict disagreement (redsim as a search pre-screen must never
    /// reject a working door).
    RedsimVerify(cmd::judge::RedsimVerifyArgs),
    /// Obsidian -> stone wherever the door still passes with the same ticks (piston-door
    /// material.rs, vjeux 2026-10-21: only obsidian where needed): every obsidian block tried as
    /// stone on the redsim door screen (all at once, else one at a time, then together); the
    /// changed designs to --out with material.tsv; --judge confirms them in the game.
    Material(cmd::judge::MaterialArgs),
    /// `door ideal` on redsim (no JVM; piston-door ideal_rs.rs): the same schedule file and checks,
    /// in milliseconds; `--sweep` grids run in parallel (timing windows), `--trace` prints every
    /// block change of the first cycle at the first position; `--compact-dir` pulls every input
    /// group as early as the mechanism allows and writes `<name>-cpt.ideal` with its windows.
    IdealRs(cmd::ideal::IdealRsArgs),
    /// Per firing of an ideal plan (each redstone block it places): the pistons it powers and
    /// where a FLAT build could feed them instead (piston-door feeds.rs): free cells next to the
    /// piston (not its front), quasi-connectivity into the cell above it, or a conductor block
    /// powered from a free cell; cells must stay free all cycle inside `--box`. Firings with no
    /// option are listed last (cores for static wiring).
    Feeds(cmd::ideal::FeedsArgs),
    /// Re-time a `door ideal` plan: every event of the listed input cells moved by its shift,
    /// the others by `--rest` (ticks evaluated and written as numbers); for trying an area that
    /// starts later than the lever without editing the plan by hand.
    ShiftPlan(cmd::ideal::ShiftPlanArgs),
    /// Drop every input pulse a `door ideal` plan does not need (greedy, latest first, each drop
    /// kept only while the plan with all drops so far still works): lighter wiring areas.
    PrunePlan(cmd::ideal::PrunePlanArgs),
    /// Replace a plan's ideal inputs by OBSERVERS watching the mechanism itself (piston-door
    /// selffeed.rs, track pext): for each input, every observer in a free plane cell that powers
    /// the same piston (directly, through the cell above it, or through a solid block touching
    /// either) is judged on redsim with that input removed; greedy rounds keep the fastest that
    /// works at the full positions. What is left is a smaller plan for the wiring tools.
    Selffeed(cmd::selffeed::SelffeedArgs),
    /// ONE pulse train for both lever edges (piston-door interleave.rs, track pext): merge a
    /// plan's open and close step sequences, a step of one edge firing during the other only where
    /// it changes nothing there (breadth-first over the steps done on each edge), then judge the
    /// merged train as a plan with the same events on both edges.
    Interleave(cmd::selffeed::InterleaveArgs),
    /// Feed a plan's inputs from a SHUTTLED observer (piston-door shuttle.rs, track clk-rt): a
    /// sticky piston carries an observer one cell out and back, and the observer fires 2 gt after
    /// each landing, so pistons with no free neighbour (boxed wiggle pistons, pistons on a drop
    /// cell) get pulses. Every placement (piston cell and facing, observer facing, free landing
    /// cell) and per-pulse arrival/return timing is judged on redsim with the target inputs
    /// removed; `--absorb` then drops other plan pulses the observer's free pulses can give.
    Shuttle(cmd::shuttle::ShuttleArgs),
    /// Differential fuzzing of the redsim piston model (track pistmodel): random piston setups
    /// (7x7x1 flat or 6x6x3) with scripted power changes, every block state, moving block and
    /// queue size compared after every tick against the bridge; failures shrunk and saved.
    Pistfuzz(cmd::fuzz::PistfuzzArgs),
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::Fill(a) => cmd::fill::cmd_fill(a),
        DoorCmd::Cycle(a) => cmd::judge::cmd_cycle(a),
        DoorCmd::Sheet(a) => cmd::ideal::cmd_sheet(a),
        DoorCmd::Cosearch(a) => cmd::cosearch::cmd_cosearch(version, a),
        DoorCmd::RedsimFuzz(a) => cmd::fuzz::cmd_redsim_fuzz(version, a),
        DoorCmd::RedsimCase(a) => cmd::fuzz::cmd_redsim_case(version, a),
        DoorCmd::TraceCompare(a) => cmd::fuzz::cmd_trace_compare(a),
        DoorCmd::RedsimVerify(a) => cmd::judge::cmd_redsim_verify(version, a),
        DoorCmd::Material(a) => cmd::judge::cmd_material(version, a),
        DoorCmd::IdealRs(a) => cmd::ideal::cmd_ideal_rs(a),
        DoorCmd::Feeds(a) => cmd::ideal::cmd_feeds(a),
        DoorCmd::ShiftPlan(a) => cmd::ideal::cmd_shift_plan(a),
        DoorCmd::PrunePlan(a) => cmd::ideal::cmd_prune_plan(a),
        DoorCmd::Selffeed(a) => cmd::selffeed::cmd_selffeed(a),
        DoorCmd::Interleave(a) => cmd::selffeed::cmd_interleave(a),
        DoorCmd::Shuttle(a) => cmd::shuttle::cmd_shuttle(a),
        DoorCmd::Pistfuzz(a) => cmd::fuzz::cmd_pistfuzz(version, a),
    }
}
