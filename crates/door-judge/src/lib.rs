//! `mctas door` commands: the judge and the boards: verify, blocks, properties, props-dyn, eligibility, relever, leaderboard, frontier, shapes, tiles, catalog, turn, trace, patch, gen, audit.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
// The command bodies (cmd/*.rs) name piston-door's modules `crate::<module>` through this glob.
use piston_door::*;

mod cmd;

/// `mctas test` (the tests of this change) and `mctas recorded` (the recordings' manifests):
/// top-level commands whose arguments and bodies live here, next to `door selftest`.
pub use cmd::checks::{RecordedCmd, TestArgs, cmd_recorded, cmd_test};

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Write the archive catalog (archive/README.md): every archived design with its size (the
    /// volume) and blocks placed, boards, open/close ticks, rotations, buildability (`door
    /// buildable`'s scripts), video link and how it works (archive/captions.tsv,
    /// archive/records.tsv). Records nothing; verdicts come from the verdict cache, sizes from the
    /// properties cache. `door video-all` writes it too.
    Catalog(cmd::boards::CatalogArgs),
    /// The Pareto frontier over (open ticks, close ticks, size: the volume): every working design
    /// on HEAD and the explore/* branches (verdicts from the leaderboard cache, uncached ones
    /// verified), then each --check design verified with the judge and placed against it: on the
    /// frontier, or the doors that dominate it (see leaderboard.rs `frontier_report`).
    Frontier(cmd::boards::FrontierArgs),
    /// The shape of every working design relative to its door: layers in front of and behind the
    /// door plane (thickness), rows below and above the opening, columns left and right of it,
    /// bounding box; then for each shape rule (wall, wall2, flat, ground, no-above, narrow1,
    /// narrow2, wall-ground, plus --rule) the best rotation-safe design today per league, and the
    /// (volume, blocks) frontier. Verdicts come from the leaderboard cache (uncached designs are
    /// verified with the judge).
    Shapes(cmd::boards::ShapesArgs),
    /// Verify every design on HEAD and on the explore/* branches and write the boards
    /// (LEADERBOARD.md: each board's leader with its GIF, video and caption, the rest collapsed).
    Leaderboard(cmd::boards::LeaderboardArgs),
    /// The distinct PISTON LAYOUTS among the working doors of one size (layouts.rs): every design
    /// on HEAD, origin/main and the explore/* branches the leaderboard's caches judge WORKS with
    /// those volume dims; layout = its pistons at rest (position from the door, facing, sticky),
    /// wiring ignored; identical layouts grouped, mirrors paired, per layout doors / best blocks /
    /// best open-close / example / first find (earliest commit carrying one of its files, track).
    Layouts(cmd::boards::LayoutsArgs),
    /// What our rules, checks, generators and wiring tools silently exclude (audit.rs, track
    /// audit): on working doors (default: the community 7x9, tracks/c79/c79-closed.txt), (1) every
    /// check that rejects or mislabels it, (2) its mechanism extracted from a redsim cycle and its
    /// membership in each generator's space (families' own generator restricted to its pistons,
    /// every bound relaxed in turn) with the bounds its real moves break, (3) the blocks and
    /// mechanics each wiring tool cannot represent, (4) every hard-coded rule (file:line, live).
    Audit(cmd::checks::AuditArgs),
    /// Which archived doors are ours and which derive from community designs (provenance.rs):
    /// a name / archive provenance naming a root (c79, community), or a mechanism (redsim cycle,
    /// wiring ignored, mirrors merged) within --near pieces of a root's; writes
    /// archive/provenance.tsv (name, origin = ours | community-derived, reason) + a summary.
    Provenance(cmd::checks::ProvenanceArgs),
    /// Write a design turned and/or mirrored (the game's own block-state rotate/mirror). A turn
    /// of 90 or 270 degrees gives an east/west-facing door (`door-axis: z` in the file), which
    /// every tool judges in that orientation: the way to build a variant for that direction.
    Turn(cmd::design::TurnArgs),
    /// Print a built-in design (see designs.rs) in the text format.
    Gen(cmd::design::GenArgs),
    Patch(cmd::design::PatchArgs),
    /// List a design's blocks with their coordinates, one `x y z state` line each (the same
    /// coordinates `door patch` and `door drive` use), optionally only those inside a box.
    Blocks(cmd::design::BlocksArgs),
    /// Place a design, settle, flip the lever and print every block change tick by tick.
    Trace(cmd::design::TraceArgs),
    /// Make working doors record-eligible for repeater input (relever.rs): the lever's cell becomes
    /// a note block (or --subs dropper,dispenser,lamp,bulb) with the lever hung on each of its free
    /// faces. Every variant (and the design as it is) is judged like a record (positions, cycles,
    /// rotation check) and must pass the --eligible checks (repeater input at every position,
    /// hallway, footsteps for sculk, QUART SEAMLESS+, lever place). Prints the passing variants
    /// per design and writes them to --out-dir.
    Relever(cmd::properties::ReleverArgs),
    /// Record eligibility of designs, one line each (what `--eligible` searches keep): the judge
    /// (the lever, every judge position) and the rotation check, repeater input at every judge
    /// position (props-dyn's rig: the lever left off, a delay-1 repeater into its support block,
    /// both edges; worst open/close ticks after the input), the hallway check, footsteps (sculk
    /// designs only) and the SEAMLESS tier (props).
    Eligible(cmd::properties::EligibleArgs),
    /// Dynamic properties of working designs (properties_dyn.rs): hallway-compatible, contained
    /// (shells of air, smooth stone, slime, honey, obsidian, a mix), snow-proof (+ flammable
    /// blocks on top), repeater input (open/close ticks), footsteps, 20 cycles. One position,
    /// one orientation. Writes a TSV (joins properties.rs on the design hash) and prints a table.
    PropertiesDyn(cmd::properties::PropertiesDynArgs),
    /// Known-answer selftest (selftest.rs): every search / judge tool on known answers (archived
    /// anchors WORK in redsim and the game, pockets re-find known regions, satwire's model on
    /// c79 and on a saved BUD candidate, boxproof on c79, ideal-rs vs ideal, generator
    /// membership) and the simulator's recorded game runs and fuzzers. One PASS / FAIL line per
    /// check, in parallel sized from the machine; the table `mctas test` picks a change's pieces
    /// from (AGENTS.md: the tests of a change); all of it on demand only (`--quick`: the checks
    /// that start no JVM). Run from the repository root.
    Selftest(cmd::checks::SelftestArgs),
    /// Verify a design many times in ONE reused sandbox (as the search does) and count passes:
    /// shows state that leaks across `clear` (e.g. redstone torch burnout memory).
    ReuseCheck(cmd::checks::ReuseCheckArgs),
    /// Every tick of one open/close cycle (design position) that breaks SUPER seamless: the
    /// circuitry seen in the tunnel T or the hallway ring S (props' reading); `*` marks what is
    /// still seen when a ring cell counts only while its T neighbour holds no door block (the
    /// covered reading, tier `supercov`).
    SeamlessWhy(cmd::properties::SeamlessWhyArgs),
    /// Redstone Squid catalogue properties (properties.rs) of the given designs, or of every
    /// working design on HEAD and the explore/* branches (the leaderboard's list, verdicts from
    /// the archive's verdicts-v5.tsv): seamless tiers, dents, trapdoor/flush/deluxe, hipster,
    /// symmetry, sync, clean, component restrictions, volume, timings. Cached by design hash in
    /// <archive-dir>/properties-v1.tsv; prints a markdown table.
    Properties(cmd::properties::PropertiesArgs),
    /// Check that a design is a working 3x3 piston door; print timings and block count.
    Verify(cmd::verify::VerifyArgs),
    /// Tile board (tile.rs): every design with a `tiling:` header (e.g. `tiling: period:8:3`) on
    /// HEAD and the explore/* branches, judged as a tile module; ranked per tiling kind by blocks
    /// per opening in a long row (TILEBOARD.md).
    TileBoard(cmd::verify::TileBoardArgs),
    /// Tileability scan (tile.rs): every design under every tiling of --tiles, in parallel; prints
    /// per design which tilings work and the smallest working period.
    TileScan(cmd::verify::TileScanArgs),
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::Catalog(a) => cmd::boards::cmd_catalog(version, a),
        DoorCmd::Frontier(a) => cmd::boards::cmd_frontier(version, a),
        DoorCmd::Shapes(a) => cmd::boards::cmd_shapes(version, a),
        DoorCmd::Leaderboard(a) => cmd::boards::cmd_leaderboard(version, a),
        DoorCmd::Layouts(a) => cmd::boards::cmd_layouts(a),
        DoorCmd::Audit(a) => cmd::checks::cmd_audit(version, a),
        DoorCmd::Provenance(a) => cmd::checks::cmd_provenance(a),
        DoorCmd::Turn(a) => cmd::design::cmd_turn(version, a),
        DoorCmd::Gen(a) => cmd::design::cmd_gen(version, a),
        DoorCmd::Patch(a) => cmd::design::cmd_patch(version, a),
        DoorCmd::Blocks(a) => cmd::design::cmd_blocks(a),
        DoorCmd::Trace(a) => cmd::design::cmd_trace(version, a),
        DoorCmd::Relever(a) => cmd::properties::cmd_relever(version, a),
        DoorCmd::Eligible(a) => cmd::properties::cmd_eligible(version, a),
        DoorCmd::PropertiesDyn(a) => cmd::properties::cmd_properties_dyn(version, a),
        DoorCmd::Selftest(a) => cmd::checks::cmd_selftest(a),
        DoorCmd::ReuseCheck(a) => cmd::checks::cmd_reuse_check(version, a),
        DoorCmd::SeamlessWhy(a) => cmd::properties::cmd_seamless_why(version, a),
        DoorCmd::Properties(a) => cmd::properties::cmd_properties(version, a),
        DoorCmd::Verify(a) => cmd::verify::cmd_verify(version, a),
        DoorCmd::TileBoard(a) => cmd::verify::cmd_tile_board(version, a),
        DoorCmd::TileScan(a) => cmd::verify::cmd_tile_scan(version, a),
    }
}
