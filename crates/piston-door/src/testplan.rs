//! `mctas test`: the tests of THIS change (vjeux 2026-11-08: "The session that changes the
//! redstone simulator should run the tests to make sure that the changes are safe. The session
//! that changes a farm should run the tests to make sure the farm is working. We shouldn't test
//! everything for every change."; AGENTS.md, docs/tools.md#mctas-test).
//!
//! The change is the session's tree against main: `git diff --no-renames <merge-base of HEAD and
//! origin/main>` (committed, staged and unstaged) plus the untracked files. Each touched file
//! picks pieces through [`RULES`] (explicit, conservative inside a study, local across them):
//! * the SIMULATOR (redsim with its mob, entity, fluid and light code, redsim-fuzz, the bridge,
//!   mctas' sim and vanilla code) picks the simulator pieces: every recorded game answer replayed
//!   on redsim (`sim:recorded-*`, seconds, no JVM, stale recordings fail: [`mctas::recorded`]),
//!   redsim's own tests, the live differential fuzzers of the areas it touched (`sim:fuzz-*`)
//!   and, for the bridge, the official-server cross-checks (`sim:vanilla-*`). Never a judge;
//! * a JUDGE or STUDY's code picks that study's checks on its calibration designs (the
//!   selftest's checks of that study, on redsim and in the game), and the recorded pieces when
//!   the recordings replay through its code (the clock crate, the circuit judge, the four exact
//!   gates' studies, the shulker replayer: seconds); never a live fuzzer;
//! * a DESIGN (a door, clock, circuit or farm) picks THAT design's judge (`door verify
//!   --extra-offsets 28`, `clock verify`, `circuit verify`, a gadget's `door drive`), plus its
//!   study's checks when it is one of their calibration or selftest files;
//! * EVERY CRATE's code also picks the crate's own unit tests (`unit:<crate>`: `cargo test
//!   --release -p <crate> --all-targets`, beside the pieces above; redsim's run in
//!   `sim:redsim-tests`, [`TESTS_RUN_BY`]), and a full run runs every crate's: a build failure,
//!   a failing test or a crashed test binary FAILS; a crate with no tests passes saying "no unit
//!   tests in <crate>" (no code gate for our process: the parent's ruling, ironspec 2026-11-12
//!   19:42 PT); then clippy where the toolchain has it, a deny-level error failing only in the
//!   files the change touches, every other finding counted. A TOOL crate no calibration piece
//!   covers (the mob farm tools, the farm optimizer, the site's and the viewer's code, the video
//!   and download code...) picks its unit tests only, and a change under crates/ that still picks
//!   nothing fails (ironspec 2026-11-12: three crates/mobfarm commits landed on "0 pieces,
//!   nothing to run");
//! * docs and web/ pick nothing (publishing runs no tests).
//!
//! The plan is printed with the files behind every piece, the pieces run in parallel sized from
//! the machine (cores and free memory; MCTAS_MAX_WORKERS caps every pool, as everywhere), each
//! gets PASS or FAIL, and the last line is the one to put in the commit message.

use crate::selftest::{Check, checks, run_step};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// A study (or the simulator): the unit a code path picks.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Group {
    /// redsim, the bridge and their differential tests
    Sim,
    /// the door judge and the door tools (piston-door, crates/door-*)
    Door,
    /// the clock judge (crates/clock*)
    Clock,
    /// the circuit judge's own calibration (every kind's metrics, the R7 refusals)
    Circuit,
    /// study 19: minecart stations, cart loaders, the cart line
    Rail,
    /// study 21: automatic doors
    Autodoor,
    /// study 8: shulker loaders
    Shulker,
    /// the TNT judge's lever check
    Tnt,
    /// study 13: item sorters
    Sorter,
    /// study 17: auto-smelters (and the super smelter)
    Smelter,
    /// study 21: auto-brewers
    Brewer,
    /// study 22: auto-composters
    Composter,
    /// the item elevator
    Lift,
    /// study 18: crop farms
    Farm,
}

impl Group {
    pub const ALL: [Group; 14] = [Group::Sim, Group::Door, Group::Clock, Group::Circuit, Group::Rail, Group::Autodoor, Group::Shulker, Group::Tnt, Group::Sorter, Group::Smelter, Group::Brewer, Group::Composter, Group::Lift, Group::Farm];
    /// Every circuit study (the circuit judge's shared code picks them all).
    pub const CIRCUITS: [Group; 11] = [Group::Circuit, Group::Rail, Group::Autodoor, Group::Shulker, Group::Tnt, Group::Sorter, Group::Smelter, Group::Brewer, Group::Composter, Group::Lift, Group::Farm];

    pub fn name(self) -> &'static str {
        match self {
            Group::Sim => "sim",
            Group::Door => "door",
            Group::Clock => "clock",
            Group::Circuit => "circuit",
            Group::Rail => "rail",
            Group::Autodoor => "autodoor",
            Group::Shulker => "shulker",
            Group::Tnt => "tnt",
            Group::Sorter => "sorter",
            Group::Smelter => "smelter",
            Group::Brewer => "brewer",
            Group::Composter => "composter",
            Group::Lift => "lift",
            Group::Farm => "farm",
        }
    }

    pub fn parse(s: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|g| g.name() == s)
    }
}

/// The simulator's areas (the live fuzzers of the areas a change touched). Every simulator
/// change replays every recording (`traces`, seconds).
pub const AREAS: [&str; 6] = ["traces", "redstone", "pistons", "containers", "sculk", "vanilla"];

/// What a touched path picks.
#[derive(Clone, Copy, Debug)]
pub enum Pick {
    /// every piece (the test tool itself, the shared design format, the workspace)
    Everything,
    /// these studies' checks
    Groups(&'static [Group]),
    /// the simulator: the recordings plus the live tests of these areas
    Sim(&'static [&'static str]),
    /// the simulator areas AND these studies (a crate that is both)
    SimAnd(&'static [&'static str], &'static [Group]),
    /// nothing from the calibration pieces, and why (a path under crates/ still picks its crate's
    /// own unit tests: `unit_piece`)
    Nothing(&'static str),
}

/// The engine of redsim: a change there touches every area.
const ENGINE: &[&str] = &["traces", "redstone", "pistons", "containers", "sculk", "terrain"];
/// The bridge: the engine's areas and the official-server cross-checks.
const EVERY_AREA: &[&str] = &["traces", "redstone", "pistons", "containers", "sculk", "vanilla", "terrain"];
const CIRCUITS: &[Group] = &Group::CIRCUITS;

/// The file -> piece table, most specific prefix first (the first match wins). Explicit and
/// conservative inside a study (a study's shared code picks all of its checks; a circuit study's
/// module picks the studies whose modules call into it, crates/circuit/src/*.rs `crate::<m>::`),
/// local across studies (a simulator change picks no judge; a judge change no simulator test).
/// A path no rule matches outside designs/ picks everything (and says so); `rules_cover_every_
/// crate` checks every crate has a rule.
pub const RULES: &[(&str, Pick)] = &[
    // the test tool itself and the workspace: everything
    ("crates/piston-door/src/selftest.rs", Pick::Everything),
    ("crates/piston-door/src/testplan.rs", Pick::Everything),
    ("crates/door-judge/src/cmd/checks.rs", Pick::Everything),
    ("Cargo.toml", Pick::Everything),
    ("Cargo.lock", Pick::Everything),
    ("crates/mctas-cli/", Pick::Everything),
    // mctas: the bridge and the official server are the simulator; videos, fonts and downloads
    // pick nothing; the rest (the design format, block states, paths, setup) everything
    ("crates/mctas/src/sim.rs", Pick::Sim(EVERY_AREA)),
    ("crates/mctas/src/bridge.rs", Pick::Sim(EVERY_AREA)),
    ("crates/mctas/src/recorded.rs", Pick::Sim(&["traces"])),
    ("crates/mctas/src/vanilla/", Pick::Sim(&["vanilla"])),
    ("crates/mctas/src/video.rs", Pick::Nothing("videos")),
    ("crates/mctas/src/mcfont.rs", Pick::Nothing("videos (the client font)")),
    ("crates/mctas/src/fabric.rs", Pick::Nothing("videos (the client launcher)")),
    ("crates/mctas/src/net.rs", Pick::Nothing("downloads (setup)")),
    ("crates/mctas/", Pick::Everything),
    ("bridge/", Pick::Sim(EVERY_AREA)),
    // redsim, by area: the engine touches every area; the entity, fluid, crop, TNT, rail, light
    // and weather code is covered by the recorded traces (no live fuzzer generates entities)
    ("crates/redsim/src/piston/", Pick::Sim(&["traces", "pistons"])),
    ("crates/redsim/src/gravity.rs", Pick::Sim(&["traces", "pistons"])),
    ("crates/redsim/src/redstone/container", Pick::Sim(&["traces", "containers"])),
    ("crates/redsim/src/redstone/hopper", Pick::Sim(&["traces", "containers"])),
    ("crates/redsim/src/redstone/dropper", Pick::Sim(&["traces", "containers"])),
    ("crates/redsim/src/redstone/", Pick::Sim(&["traces", "redstone", "pistons", "containers"])),
    ("crates/redsim/src/sculk.rs", Pick::Sim(&["traces", "sculk"])),
    // the random ticks of real terrain (`sim:terrain-ticks-*`: the bridge against redsim for a
    // day with spawning off): the entities' level draws, fluids (lava's fire), weather and crops
    ("crates/redsim/src/entity/", Pick::Sim(&["traces", "terrain"])),
    ("crates/redsim/src/fluid/", Pick::Sim(&["traces", "terrain"])),
    ("crates/redsim/src/weather/", Pick::Sim(&["traces", "terrain"])),
    ("crates/redsim/src/tnt.rs", Pick::Sim(&["traces"])),
    ("crates/redsim/src/explosion.rs", Pick::Sim(&["traces"])),
    ("crates/redsim/src/rails.rs", Pick::Sim(&["traces"])),
    ("crates/redsim/src/crops.rs", Pick::Sim(&["traces", "terrain"])),
    ("crates/redsim/src/stems.rs", Pick::Sim(&["traces", "terrain"])),
    ("crates/redsim/src/bamboo.rs", Pick::Sim(&["traces", "terrain"])),
    ("crates/redsim/src/cactus.rs", Pick::Sim(&["traces", "terrain"])),
    ("crates/redsim/src/kelp.rs", Pick::Sim(&["traces", "terrain"])),
    ("crates/redsim/src/daylight.rs", Pick::Sim(&["traces"])),
    ("crates/redsim/src/loot", Pick::Sim(&["traces"])),
    ("crates/redsim/src/fence.rs", Pick::Sim(&["traces", "redstone"])),
    ("crates/redsim/", Pick::Sim(ENGINE)),
    ("crates/redsim-fuzz/", Pick::Sim(ENGINE)),
    ("crates/boat-leads/", Pick::Sim(&["traces"])),
    // the mob AI's differential tools (path probe, fixtures, their recordings' replay)
    ("crates/mobai/", Pick::Sim(&["traces"])),
    // piston-door: the differential tools are the simulator's, the rest is the door study
    ("crates/piston-door/src/pistfuzz.rs", Pick::Sim(&["traces", "pistons", "sculk"])),
    ("crates/piston-door/src/vanilla_", Pick::Sim(&["vanilla"])),
    ("crates/piston-door/", Pick::Groups(&[Group::Door])),
    ("crates/door-vanilla/", Pick::Sim(&["vanilla"])),
    ("crates/door-redsim/", Pick::SimAnd(&["traces", "redstone", "pistons", "containers", "sculk"], &[Group::Door])),
    ("crates/door-video/", Pick::Nothing("videos")),
    ("crates/door-", Pick::Groups(&[Group::Door])),
    // the clock crate is the circuit judge's base too (designs, placement, the redsim harness), and
    // every exact trace replays through it (`Clock::load`): the recordings too
    ("crates/clock/", Pick::SimAnd(&["traces"], &[Group::Clock, Group::Circuit, Group::Rail, Group::Autodoor, Group::Shulker, Group::Tnt, Group::Sorter, Group::Smelter, Group::Brewer, Group::Composter, Group::Lift, Group::Farm])),
    ("crates/clock-obs/", Pick::Groups(&[Group::Clock, Group::Circuit])),
    ("crates/clock-misc/", Pick::Groups(&[Group::Clock])),
    ("crates/clock-", Pick::Groups(&[Group::Clock])),
    // the circuit studies' modules: the study and the studies that call into it (and the recorded
    // runs of the four exact gates, which replay through their scenario code)
    ("crates/circuit/src/sorter.rs", Pick::SimAnd(&["traces"], &[Group::Sorter, Group::Smelter, Group::Brewer, Group::Composter, Group::Lift])),
    ("crates/circuit/src/smelter.rs", Pick::SimAnd(&["traces"], &[Group::Smelter, Group::Sorter, Group::Brewer])),
    ("crates/circuit/src/brewer.rs", Pick::SimAnd(&["traces"], &[Group::Brewer])),
    ("crates/circuit/src/composter.rs", Pick::SimAnd(&["traces"], &[Group::Composter, Group::Sorter])),
    ("crates/circuit/src/recorded.rs", Pick::Sim(&["traces"])),
    ("crates/circuit/src/studies/lift.rs", Pick::Groups(&[Group::Lift])),
    ("crates/circuit/src/farm.rs", Pick::Groups(&[Group::Farm])),
    ("crates/circuit/src/canefarm.rs", Pick::Groups(&[Group::Farm])),
    ("crates/circuit/src/farmgen.rs", Pick::Groups(&[Group::Farm])),
    ("crates/circuit/src/station.rs", Pick::Groups(&[Group::Rail])),
    ("crates/circuit/src/cartio.rs", Pick::Groups(&[Group::Rail])),
    ("crates/circuit/src/cartsys.rs", Pick::Groups(&[Group::Rail])),
    ("crates/circuit/src/rig.rs", Pick::Groups(&[Group::Rail])),
    ("crates/circuit/src/autodoor.rs", Pick::SimAnd(&["traces"], &[Group::Autodoor])),
    ("crates/circuit/src/tnt.rs", Pick::Groups(&[Group::Tnt, Group::Circuit])),
    ("crates/circuit/src/fly.rs", Pick::Groups(&[Group::Circuit])),
    ("crates/circuit/src/elevator.rs", Pick::Groups(&[Group::Circuit])),
    ("crates/circuit/src/studies/mobfarm.rs", Pick::Nothing("the mob farm study: no calibration pieces; a mob farm design runs its own judge and game gate")),
    ("crates/circuit/src/mobfarm_game.rs", Pick::Nothing("the mob farms' real-game gate: no calibration pieces; a mob farm design runs its own judge and game gate (circuit verify)")),
    ("crates/circuit/src/mobfarm_exact.rs", Pick::Nothing("the mob farms' tick-exact gate: no calibration pieces; a mob farm design runs its own judge and its gates (circuit verify)")),
    ("crates/circuit/src/studies/bedrock.rs", Pick::Nothing("the bedrock study: no calibration pieces; a bedrock design runs its own judge")),
    ("crates/circuit/", Pick::SimAnd(&["traces"], CIRCUITS)),
    ("crates/circuit-cli/", Pick::SimAnd(&["traces"], CIRCUITS)),
    ("crates/circuit-shulker/", Pick::SimAnd(&["traces"], &[Group::Shulker])),
    ("crates/circuit-tnt/", Pick::Groups(&[Group::Tnt, Group::Circuit])),
    ("crates/circuit-", Pick::Groups(&[Group::Circuit])),
    // the exact compare's harness (the bridge's world, redsim's mirror of it)
    ("crates/mobfarm/src/exact.rs", Pick::Sim(&["terrain"])),
    ("crates/mobfarm/", Pick::Nothing("the mob farm tools: no calibration pieces; a mob farm design runs its own judge and game gate")),
    ("crates/bedrock/", Pick::Nothing("the bedrock tools: no calibration pieces; a bedrock design runs its own judge")),
    ("crates/terrain/", Pick::Nothing("the terrain tools (the official server's terrain samples): no calibration pieces")),
    ("crates/farmopt", Pick::Nothing("the farm optimizer (farmopt, farmopt-iron): no calibration pieces; the farms it finds run their own judge")),
    // the site, the viewer, docs: nothing (publishing runs no tests)
    ("crates/site/", Pick::Nothing("the site")),
    ("crates/site-model/", Pick::Nothing("the site")),
    ("crates/web-cli/", Pick::Nothing("the viewer")),
    ("crates/mcview", Pick::Nothing("the viewer")),
    ("crates/litematic/", Pick::Nothing("schematics")),
    ("crates/trends/", Pick::Nothing("search demand")),
    ("web/", Pick::Nothing("the site")),
    ("docs/", Pick::Nothing("docs")),
    ("AGENTS.md", Pick::Nothing("docs")),
    ("README.md", Pick::Nothing("docs")),
    ("LICENSE", Pick::Nothing("docs")),
    (".gitignore", Pick::Nothing("git")),
    (".gitattributes", Pick::Nothing("git")),
];

/// How each check of the selftest table runs in the plan: its group (and simulator area), the
/// JVM workers it starts (its pool: `{workers}` in its commands), the CPU threads it keeps busy
/// and its rough wall time alone in seconds (the longest start first). The first row whose name
/// (or prefix ending in `-` or `:`) matches wins; `every_check_has_a_row` checks the table.
pub const ROWS: &[(&str, Group, &str, usize, usize, u32)] = &[
    // (check, group, area, JVM workers, CPU threads, seconds)
    ("sim:recorded-traces", Group::Sim, "traces", 0, 64, 15),
    ("sim:redsim-tests", Group::Sim, "traces", 0, 16, 60),
    ("sim:recorded-", Group::Sim, "traces", 0, 8, 10),
    ("circuit:shulker-compare", Group::Sim, "traces", 0, 1, 5),
    ("sim:fuzz-redstone-", Group::Sim, "redstone", 8, 16, 60),
    ("sim:fuzz-containers", Group::Sim, "containers", 8, 16, 90),
    ("sim:fuzz-pistons-", Group::Sim, "pistons", 8, 16, 60),
    ("sim:fuzz-sculk", Group::Sim, "sculk", 8, 16, 90),
    ("sim:vanilla-", Group::Sim, "vanilla", 2, 4, 150),
    ("sim:terrain-ticks-", Group::Sim, "terrain", 1, 2, 330),
    ("sim:autodoor-twin", Group::Sim, "traces", 0, 16, 10),
    ("judge:redsim-", Group::Door, "", 0, 1, 30),
    ("judge:verify-", Group::Door, "", 1, 2, 90),
    ("judge:2x2-", Group::Door, "", 1, 2, 90),
    ("material:archive-guards", Group::Door, "", 8, 8, 400),
    ("material:", Group::Door, "", 0, 4, 60),
    ("pockets:", Group::Door, "", 0, 32, 60),
    ("satwire:", Group::Door, "", 0, 1, 60),
    ("boxproof:fuzz", Group::Door, "", 0, 32, 120),
    ("boxproof:", Group::Door, "", 0, 1, 60),
    ("ideal:agree-", Group::Door, "", 1, 2, 120),
    ("ideal:plan-phase-observer-jvm", Group::Door, "", 1, 2, 90),
    ("ideal:", Group::Door, "", 0, 1, 15),
    ("shuttle:", Group::Door, "", 0, 1, 20),
    ("audit:", Group::Door, "", 0, 4, 60),
    ("archive:tables", Group::Clock, "", 0, 1, 10),
    ("clock:archive-refuses-", Group::Clock, "", 2, 4, 180),
    ("clock:verify-calibration", Group::Clock, "", 8, 16, 180),
    ("clock:verify-negative", Group::Clock, "", 1, 2, 60),
    ("clock:verify-reload", Group::Clock, "", 8, 16, 300),
    ("clock:verify-entities", Group::Clock, "", 4, 8, 180),
    ("clock:", Group::Clock, "", 0, 1, 15),
    ("circuit:sim-calibration", Group::Circuit, "", 0, 1, 60),
    ("circuit:verify-calibration-", Group::Circuit, "", 8, 16, 220),
    ("circuit:tnt-lever", Group::Tnt, "", 0, 1, 150),
    ("circuit:verify-station", Group::Rail, "", 5, 10, 300),
    ("circuit:rail-twin", Group::Rail, "", 0, 1, 60),
    ("circuit:cart-system", Group::Rail, "", 0, 1, 60),
    ("circuit:verify-autodoor", Group::Autodoor, "", 5, 10, 400),
    ("circuit:verify-shulker-", Group::Shulker, "", 4, 8, 200),
    ("circuit:shulker-", Group::Shulker, "", 0, 1, 30),
    ("circuit:sorter-compare", Group::Sorter, "", 1, 2, 200),
    ("circuit:sorter-verify", Group::Sorter, "", 8, 16, 400),
    ("circuit:sorter-", Group::Sorter, "", 0, 1, 60),
    ("circuit:smelter-compare", Group::Smelter, "", 1, 2, 300),
    ("circuit:smelter-verify", Group::Smelter, "", 8, 16, 400),
    ("circuit:smelter-", Group::Smelter, "", 0, 1, 60),
    ("circuit:super-smelter", Group::Smelter, "", 0, 1, 60),
    ("circuit:brewer-compare", Group::Brewer, "", 1, 2, 300),
    ("circuit:brewer-verify", Group::Brewer, "", 8, 16, 400),
    ("circuit:brewer-", Group::Brewer, "", 0, 1, 60),
    ("circuit:composter-compare", Group::Composter, "", 1, 2, 300),
    ("circuit:composter-verify", Group::Composter, "", 8, 16, 400),
    ("circuit:composter-", Group::Composter, "", 0, 1, 60),
    ("circuit:lift-verify", Group::Lift, "", 8, 16, 400),
    ("circuit:lift-", Group::Lift, "", 0, 1, 60),
    ("circuit:farm-run", Group::Farm, "", 1, 2, 300),
];

/// A check's row (panics on a check without one: `every_check_has_a_row`).
pub fn row(name: &str) -> (Group, &'static str, usize, usize, u32) {
    for (n, g, a, w, c, s) in ROWS {
        let prefix = n.ends_with('-') || n.ends_with(':');
        if *n == name || (prefix && name.starts_with(n)) {
            return (*g, a, *w, *c, *s);
        }
    }
    panic!("check {name} has no row in testplan::ROWS");
}

/// What one touched path picks: the first rule whose prefix matches.
pub fn pick(path: &str) -> Option<Pick> {
    RULES.iter().find(|(p, _)| path.starts_with(p)).map(|(_, k)| *k)
}

/// The crate a path under crates/ belongs to (`crates/<name>/...`; a file directly under crates/
/// belongs to none).
pub fn crate_of(path: &str) -> Option<&str> {
    path.strip_prefix("crates/")?.split_once('/').map(|(k, _)| k).filter(|s| !s.is_empty())
}

/// The crates whose own tests another piece already runs, so they get no unit piece of their own:
/// redsim's in `sim:redsim-tests` (`cargo test --release -p redsim --lib --tests`), which every
/// redsim rule picks (`a_redsim_change_runs_its_tests_in_sim_redsim_tests`).
pub const TESTS_RUN_BY: &[(&str, &str)] = &[("redsim", "sim:redsim-tests")];

/// Every crate of the workspace whose own tests no other piece runs: the git root's crates/
/// directories with a Cargo.toml, less TESTS_RUN_BY. Each is a unit piece when its code changes
/// and in every full run. An unreadable or empty workspace is an error, never an empty list (G1 on
/// f82897a92: a silently empty list ran no unit piece).
pub fn unit_crates() -> Result<Vec<String>, String> {
    unit_crates_in(Path::new("."))
}

/// [`unit_crates`] for the checkout `cwd` is in.
fn unit_crates_in(cwd: &Path) -> Result<Vec<String>, String> {
    let git = Command::new("git").args(["rev-parse", "--show-toplevel"]).current_dir(cwd).output().map_err(|e| format!("git rev-parse --show-toplevel: {e}"))?;
    if !git.status.success() {
        return Err(format!("git rev-parse --show-toplevel: {}", String::from_utf8_lossy(&git.stderr).trim()));
    }
    let dir = PathBuf::from(String::from_utf8_lossy(&git.stdout).trim()).join("crates");
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out: Vec<String> = rd.flatten().filter(|e| e.path().join("Cargo.toml").is_file()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    if out.is_empty() {
        return Err(format!("no crate (a directory with a Cargo.toml) under {}", dir.display()));
    }
    out.retain(|k| !TESTS_RUN_BY.iter().any(|(c, _)| c == k));
    out.sort();
    Ok(out)
}

/// A piece that always fails with `msg`: the plan could not be made in full (an unreadable
/// workspace), and the run must say so rather than run less.
fn broken_piece(name: &str, msg: String) -> Piece {
    let pred: crate::selftest::Pred = Box::new(move |_: &[String]| Err(msg.clone()));
    let check = Check { name: name.to_string(), what: "the plan itself (a piece it could not make)", quick: true, jvm: false, steps: Vec::new(), timeout: 60, pred };
    Piece { check, group: "unit".into(), workers: 0, cpu: 1, cost: 1, why: vec!["the plan could not be made in full".to_string()] }
}

/// The design files a check's commands read (its arguments under designs/): touching one picks
/// the check.
pub(crate) fn check_designs(c: &Check) -> Vec<String> {
    let mut out = Vec::new();
    for st in &c.steps {
        for a in st {
            let a = a.trim_start_matches('?');
            let v = a.rsplit_once('=').map(|(_, v)| v).unwrap_or(a);
            if v.starts_with("designs/") {
                out.push(v.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The study of a design file and the command that judges it (None: not a design, or one no
/// judge takes on its own).
pub fn design_judge(path: &str) -> Option<(&'static str, Vec<String>)> {
    let text = std::fs::read_to_string(path).ok()?;
    let is_design = text.lines().any(|l| l.trim_start().starts_with("palette:")) && text.lines().any(|l| l.trim_start().starts_with("layer "));
    let has = |key: &str| text.lines().any(|l| l.trim_start().starts_with(key));
    if path.ends_with(".drive") && path.starts_with("designs/gadgets/") {
        return Some(("gadget", vec!["door".into(), "drive".into(), "--quiet".into(), path.into()]));
    }
    if !path.ends_with(".txt") || !is_design {
        return None;
    }
    if path.starts_with("designs/piston-door-") && has("port door") && has("port lever") {
        return Some(("door", vec!["door".into(), "verify".into(), path.into(), "--extra-offsets".into(), "28".into()]));
    }
    if (path.starts_with("designs/circuits/") || path.starts_with("designs/bedrock/")) && has("circuit:") {
        return Some(("circuit", vec!["?circuit".into(), "verify".into(), "--workers".into(), "{workers}".into(), path.into()]));
    }
    if path.starts_with("designs/clocks/") {
        return Some(("clock", vec!["?clock".into(), "verify".into(), path.into(), "--workers".into(), "{workers}".into()]));
    }
    None
}

/// The change: every path the tree changes against `from`'s merge base (committed, staged,
/// unstaged, deleted, untracked). None when git cannot say (then everything runs).
pub fn changed_files(from: &str) -> Option<(String, Vec<String>)> {
    let git = |args: &[&str]| -> Option<String> {
        let o = Command::new("git").args(args).output().ok()?;
        o.status.success().then(|| String::from_utf8_lossy(&o.stdout).to_string())
    };
    let base = git(&["merge-base", "HEAD", from])?.trim().to_string();
    let mut files: Vec<String> = git(&["diff", "--name-only", "--no-renames", &base])?.lines().map(String::from).collect();
    files.extend(git(&["ls-files", "--others", "--exclude-standard"])?.lines().map(String::from));
    files.sort();
    files.dedup();
    Some((base, files))
}

/// One piece of the plan: a check (from the selftest table, or a design's judge) and why.
pub struct Piece {
    pub(crate) check: Check,
    pub group: String,
    pub workers: usize,
    pub cpu: usize,
    pub cost: u32,
    pub why: Vec<String>,
}

impl Piece {
    pub fn name(&self) -> &str {
        &self.check.name
    }
}

/// The recording directories of `sim:recorded-traces` and the other recorded pieces: a touched
/// file a recording was made from picks them (the recording may now be stale).
pub fn recording_deps() -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for dir in crate::selftest::RECORDING_DIRS {
        for r in mctas::recorded::load(Path::new(dir)).unwrap_or_default() {
            for (d, _) in &r.deps {
                out.entry(d.clone()).or_default().push(format!("{dir}/{}", r.file));
            }
        }
    }
    out
}

/// The checks picked so far and why.
struct Picks<'a> {
    all: &'a [Check],
    why: BTreeMap<String, Vec<String>>,
}

impl Picks<'_> {
    fn add(&mut self, name: &str, reason: &str) {
        let v = self.why.entry(name.to_string()).or_default();
        if !v.iter().any(|r| r == reason) {
            v.push(reason.to_string());
        }
    }

    /// A group's checks (the simulator's: the recordings and the live tests of `areas`).
    fn group(&mut self, g: Group, areas: &[&str], reason: &str) -> usize {
        let names: Vec<String> = self
            .all
            .iter()
            .filter(|c| {
                let (cg, area, ..) = row(&c.name);
                cg == g && (g != Group::Sim || area == "traces" || areas.contains(&area))
            })
            .map(|c| c.name.clone())
            .collect();
        for n in &names {
            self.add(n, reason);
        }
        names.len()
    }

    fn everything(&mut self, reason: &str) {
        let names: Vec<String> = self.all.iter().map(|c| c.name.clone()).collect();
        for n in &names {
            self.add(n, reason);
        }
    }
}

/// What a touched file under designs/ picks: the recorded pieces when a recording was made from
/// it or it is one, the checks that read it (calibration and selftest files), the archive tables
/// check for a table, and its own judge for a design (pushed to `designs`). Returns the plan line.
fn pick_design_file(f: &str, p: &mut Picks, rec_deps: &BTreeMap<String, Vec<String>>, designs: &mut Vec<String>, lints: &mut Vec<String>) -> String {
    let mut what: Vec<String> = Vec::new();
    if let Some(recs) = rec_deps.get(f) {
        p.group(Group::Sim, &[], &format!("{f}: a recording was made from it"));
        what.push(format!("sim:recorded (recorded from it: {})", recs.iter().take(2).cloned().collect::<Vec<_>>().join(", ")));
    }
    if crate::selftest::RECORDING_DIRS.iter().any(|d| f.starts_with(&format!("{d}/"))) || f.starts_with("designs/redsim/recorded/") {
        p.group(Group::Sim, &[], &format!("{f}: a recording"));
        what.push("sim:recorded (a recording)".into());
    }
    let readers: Vec<String> = p.all.iter().filter(|c| check_designs(c).iter().any(|d| f == d || f.starts_with(&format!("{d}/")))).map(|c| c.name.clone()).collect();
    for n in &readers {
        p.add(n, &format!("{f}: its calibration or selftest file"));
    }
    if !readers.is_empty() {
        what.push(format!("{} (read it)", readers.join(", ")));
    }
    if f.ends_with(".tsv") && (f.starts_with("designs/clocks/") || f.starts_with("designs/circuits/")) {
        p.add("archive:tables", &format!("{f}: an archive table"));
        what.push("archive:tables".into());
    }
    let calibration = ["/calibration/", "/selftest/", "/redsim-tests/", "/traces/", "/fixtures/"].iter().any(|t| f.contains(t)) || f.starts_with("designs/redsim/");
    if !calibration && Path::new(f).is_file()
        && let Some((study, _)) = design_judge(f)
    {
        designs.push(f.to_string());
        what.push(format!("its own judge ({study})"));
    }
    // (every changed layout with redstone wire: the capped-climb check, ironspec 2026-11-11 09:58:
    // a router change must not cut a route unseen; `--no-stair-lint` opts out)
    if !calibration && f.ends_with(".txt") && is_wired_layout(f) {
        lints.push(f.to_string());
        what.push("lint:stair-lint (capped climbs that cut a read net from its driver)".into());
    }
    if what.is_empty() {
        let why = if !Path::new(f).exists() { "deleted; no piece reads it" } else if calibration { "a calibration, selftest or test-fixture file no piece reads" } else { "not a design a judge takes; no piece reads it" };
        format!("  {f} -> nothing ({why})")
    } else {
        format!("  {f} -> {}", what.join("; "))
    }
}

/// Pick the pieces of a change: `files` (None: git could not say, so everything), explicit
/// `groups` and `designs`, or `everything` (`--all`). The plan's lines go to `out`. `stair_lint`:
/// the capped-climb check on every changed wired layout (on by default; `--no-stair-lint`).
pub fn plan(files: Option<&[String]>, groups: &[Group], designs: &[String], everything: bool, stair_lint: bool, out: &mut Vec<String>) -> Vec<Piece> {
    plan_in(files, groups, designs, everything, stair_lint, out, &unit_crates)
}

/// [`plan`] with the workspace's crate list from `crates` (a test injects an unreadable one).
fn plan_in(files: Option<&[String]>, groups: &[Group], designs: &[String], everything: bool, stair_lint: bool, out: &mut Vec<String>, crates: &dyn Fn() -> Result<Vec<String>, String>) -> Vec<Piece> {
    let all = checks();
    let mut p = Picks { all: &all, why: BTreeMap::new() };
    if everything {
        p.everything("--all");
    } else if files.is_none() {
        out.push("  git cannot say what changed -> everything".into());
        p.everything("git cannot say what changed: everything");
    }
    for g in groups {
        p.group(*g, &AREAS, &format!("--pick {}", g.name()));
    }
    let rec_deps = recording_deps();
    let mut designs: Vec<String> = designs.to_vec();
    let mut lints: Vec<String> = Vec::new();
    // every crate whose code changed: its own unit tests, beside its calibration pieces (ironspec
    // 2026-11-12 19:42 PT: every crate that has unit tests runs them when it changes; redsim's run
    // in sim:redsim-tests, TESTS_RUN_BY)
    let mut units: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // every crate's own tests too: `--all`, "git cannot say", or a path every piece goes through
    let mut every_unit = everything || files.is_none();
    for f in files.unwrap_or(&[]) {
        if f.starts_with("designs/") {
            let line = pick_design_file(f, &mut p, &rec_deps, &mut designs, &mut lints);
            out.push(line);
            continue;
        }
        let names = |gs: &[Group]| gs.iter().map(|g| g.name()).collect::<Vec<_>>().join(", ");
        let unit = crate_of(f).filter(|k| !TESTS_RUN_BY.iter().any(|(c, _)| c == k));
        let and_unit = unit.map(|k| format!(" and unit:{k}")).unwrap_or_default();
        match pick(f) {
            Some(Pick::Everything) => {
                out.push(format!("  {f} -> everything (every piece goes through it){and_unit}"));
                p.everything(&format!("{f}: every piece goes through it"));
                every_unit = true;
            }
            Some(Pick::Groups(gs)) => {
                out.push(format!("  {f} -> the {} checks{and_unit}", names(gs)));
                for g in gs {
                    p.group(*g, &[], f);
                }
            }
            Some(Pick::Sim(areas)) => {
                out.push(format!("  {f} -> sim ({}){and_unit}", areas.join(", ")));
                p.group(Group::Sim, areas, f);
            }
            Some(Pick::SimAnd(areas, gs)) => {
                out.push(format!("  {f} -> sim ({}) and the {} checks{and_unit}", areas.join(", "), names(gs)));
                p.group(Group::Sim, areas, f);
                for g in gs {
                    p.group(*g, &[], f);
                }
            }
            Some(Pick::Nothing(what)) => match unit {
                // a crate's code that no calibration piece covers: the crate's own unit tests only
                // (ironspec 2026-11-12: a code change never picks nothing)
                Some(k) => out.push(format!("  {f} -> unit:{k} (no calibration pieces: {what}; the crate's own tests)")),
                None => out.push(format!("  {f} -> nothing ({what})")),
            },
            None => {
                out.push(format!("  {f} -> everything (no rule in testplan::RULES for it: add one){and_unit}"));
                p.everything(&format!("{f}: no rule, everything"));
                every_unit = true;
            }
        }
        if let Some(k) = unit {
            units.entry(k.to_string()).or_default().push(f.clone());
        }
    }
    let mut why = p.why;
    let mut pieces = Vec::new();
    for c in all {
        if let Some(w) = why.remove(&c.name) {
            let (g, area, workers, cpu, cost) = row(&c.name);
            let group = if g == Group::Sim { format!("sim {area}") } else { g.name().to_string() };
            pieces.push(Piece { check: c, group, workers, cpu, cost, why: w });
        }
    }
    designs.sort();
    designs.dedup();
    for d in designs {
        if let Some(piece) = design_piece(&d) {
            pieces.push(piece);
        }
    }
    for (k, fs) in &units {
        pieces.push(unit_piece(k, fs));
    }
    // `--all`, "git cannot say" and a path every piece goes through run every crate's own tests
    // too: a full suite never runs less than a targeted run (ironspec 2026-11-12 18:27); a
    // workspace the plan cannot read fails the run instead of running less
    if every_unit {
        match crates() {
            Ok(ks) => {
                for k in ks {
                    if !units.contains_key(&k) {
                        pieces.push(unit_piece(&k, &[]));
                    }
                }
            }
            Err(e) => {
                out.push(format!("  every crate's own tests: cannot list the workspace's crates: {e}"));
                pieces.push(broken_piece("unit:workspace", format!("cannot list the workspace's crates: {e}")));
            }
        }
    }
    if stair_lint && !lints.is_empty() {
        pieces.push(stair_lint_piece(&lints));
    } else if !lints.is_empty() {
        out.push(format!("  lint:stair-lint skipped (--no-stair-lint) on {} changed layout(s)", lints.len()));
    }
    pieces
}

/// A design file with redstone wire in it (a layout a route can cut).
fn is_wired_layout(path: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else { return false };
    let is_design = text.lines().any(|l| l.trim_start().starts_with("palette:")) && text.lines().any(|l| l.trim_start().starts_with("layer "));
    is_design && text.contains("minecraft:redstone_wire")
}

/// The capped-climb check on the changed wired layouts as one piece: `circuit memory stair-lint`,
/// PASS iff its total line reports 0 broken routes.
fn stair_lint_piece(files: &[String]) -> Piece {
    let mut argv: Vec<String> = vec!["?circuit".into(), "memory".into(), "stair-lint".into()];
    argv.extend(files.iter().cloned());
    let pred: crate::selftest::Pred = Box::new(|o: &[String]| {
        let total = o[0].lines().find(|l| l.starts_with("total: ")).map(|l| l.to_string());
        match total {
            Some(t) if t.ends_with(" 0 broken routes") => Ok(t),
            Some(t) => Err(format!("{t}: {}", o[0].lines().filter(|l| l.contains("BROKEN ROUTE")).map(|l| l.trim()).collect::<Vec<_>>().join("; "))),
            None => Err(format!("no total line: {}", crate::selftest::tail(&o[0]))),
        }
    });
    let check = Check { name: "lint:stair-lint".into(), what: "the capped-climb check on the changed wired layouts (a read net cut from its driver by a capped climb)", quick: true, jvm: false, steps: vec![argv], timeout: 600, pred };
    Piece { check, group: "lint".into(), workers: 0, cpu: 1, cost: 5, why: files.iter().map(|f| format!("{f}: a changed wired layout")).collect() }
}

/// A crate's own unit tests as one piece (`unit:<crate>`): `cargo test --release -p <crate>
/// --all-targets` (no doc tests: they need rustdoc on PATH, as `sim:redsim-tests`). Three
/// outcomes: no "test result:" line is a build failure (FAIL, with its first `error[` line and the
/// compile tail); a result not ok, or cargo's "error: test failed" line (a test binary that crashed
/// or was killed, even after earlier binaries passed), FAILS; every result ok passes, a crate with
/// 0 tests saying "no unit tests in <crate>"
/// (no code gate for our process: the parent's ruling, ironspec 2026-11-12 19:42 PT). Then clippy
/// where the toolchain has it, scoped to the change ([`clippy_verdict`]).
fn unit_piece(krate: &str, files: &[String]) -> Piece {
    if WASM_ONLY.contains(&krate) {
        return wasm_piece(krate, files);
    }
    // `?`: the step's exit is read by the verdict below (a build failure prints its tail)
    let argv: Vec<String> = ["?@cargo", "test", "--release", "-p", krate, "--all-targets"].iter().map(|s| s.to_string()).collect();
    let lint: Vec<String> = ["?@cargo", "clippy", "--release", "-p", krate, "--all-targets"].iter().map(|s| s.to_string()).collect();
    let name = krate.to_string();
    let changed: Vec<String> = files.to_vec();
    let pred: crate::selftest::Pred = Box::new(move |o: &[String]| {
        let out = &o[0];
        let results: Vec<&str> = out.lines().filter(|l| l.starts_with("test result: ")).collect();
        if let Some(bad) = results.iter().find(|l| !l.starts_with("test result: ok.")) {
            return Err(bad.to_string());
        }
        // a test binary that crashed or was killed (SIGKILL on a loaded box, a segfault, a stack
        // overflow) prints no result line of its own and cargo stops there: cargo's own line
        // decides, never the earlier binaries' "ok." (G1 on 1e66e5edc)
        if let Some(failed) = out.lines().find(|l| l.starts_with("error: test failed")) {
            let why = out.lines().find(|l| l.contains("process didn't exit successfully")).map(|l| format!(" ({})", l.trim())).unwrap_or_default();
            return Err(format!("crates/{name}: {failed}{why}"));
        }
        if results.is_empty() {
            // no test binary ran: a build failure when cargo printed an error or never finished;
            // otherwise the crate built with no test target (`test = false`): no unit tests
            // (ironspec 2026-11-12 19:51 PT (4a))
            let error = out.lines().find(|l| l.starts_with("error"));
            if error.is_some() || !out.lines().any(|l| l.trim_start().starts_with("Finished")) {
                let first = out.lines().find(|l| l.starts_with("error[")).or(error).map(|l| format!("{l}; ")).unwrap_or_default();
                return Err(format!("crates/{name} did not build: {first}{}", crate::selftest::tail(out)));
            }
            return Ok(format!("no unit tests in {name} (it builds; no test target); {}", clippy_verdict(o.get(1).map(|s| s.as_str()), &changed)?));
        }
        let passed: usize = results.iter().filter_map(|l| l.split_whitespace().nth(3).and_then(|n| n.parse::<usize>().ok())).sum();
        let tests = if passed == 0 { format!("no unit tests in {name} (it builds; {} test binaries)", results.len()) } else { format!("{passed} {name} tests pass ({} test binaries)", results.len()) };
        Ok(format!("{tests}; {}", clippy_verdict(o.get(1).map(|s| s.as_str()), &changed)?))
    });
    let check = Check { name: format!("unit:{krate}"), what: "the crate's own unit tests (`cargo test --release -p <crate> --all-targets`: a build failure or a failing test fails, a crate with no tests passes saying so) and clippy where the toolchain has it (a deny-level error fails only in the changed files)", quick: true, jvm: false, steps: vec![argv, lint], timeout: UNIT_TIMEOUT, pred };
    let why = if files.is_empty() { vec!["--all: every crate's own tests".to_string()] } else { files.iter().map(|f| format!("{f}: its crate's own tests")).collect() };
    Piece { check, group: "unit".into(), workers: 0, cpu: 16, cost: 90, why }
}

/// clippy's verdict, scoped to the change (no repo-wide gate: ironspec 2026-11-12 19:42 PT): a
/// deny-level error fails only when its `-->` location is a changed file; every other error, every
/// warning, and every finding of a full run (no changed files) is counted and never fails. clippy
/// absent from the toolchain is said.
fn clippy_verdict(out: Option<&str>, changed: &[String]) -> Result<String, String> {
    let Some(c) = out else { return Ok("clippy not run".to_string()) };
    if c.contains("no such command") || c.contains("no such subcommand") || c.contains("is not installed") {
        return Ok("clippy is not on this toolchain: not run".to_string());
    }
    let diags = clippy_diagnostics(c);
    let touched = |p: &Option<String>| p.as_deref().is_some_and(|p| changed.iter().any(|f| f == p));
    if let Some((_, p, head)) = diags.iter().find(|(err, p, _)| *err && touched(p)) {
        return Err(format!("clippy: a deny-level error in a changed file ({}): {head}", p.as_deref().unwrap_or("?")));
    }
    let errors = diags.iter().filter(|d| d.0).count();
    let warnings = diags.len() - errors;
    let mine = diags.iter().filter(|(err, p, _)| !*err && touched(p)).count();
    let crate_wide = format!("{errors} error(s) and {warnings} warning(s) in the crate, counted, not failing");
    Ok(if changed.is_empty() { format!("clippy: {crate_wide}") } else { format!("clippy: {mine} warning(s) in the changed files; {crate_wide}") })
}

/// clippy's diagnostics as (deny-level, the file of its `-->` line, its first line): a multi-line
/// diagnostic is attributed by its location line, not its header. The summary lines ("could not
/// compile", "aborting due to", "generated N warnings") are not diagnostics.
fn clippy_diagnostics(c: &str) -> Vec<(bool, Option<String>, String)> {
    let lines: Vec<&str> = c.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let err = l.starts_with("error");
        if !(err || l.starts_with("warning")) || l.contains("could not compile") || l.contains("aborting due to") || l.contains(" generated ") {
            continue;
        }
        // the location: the first `-->` line before the next diagnostic
        let path = lines[i + 1..].iter().take_while(|n| !n.starts_with("error") && !n.starts_with("warning")).find_map(|n| n.trim_start().strip_prefix("--> ")).map(|s| s.split(':').next().unwrap_or(s).to_string());
        out.push((err, path, l.to_string()));
    }
    out
}

/// A unit piece's step timeout: the unit pieces share the target directory's build lock, so a
/// full run builds their test binaries one after another and the last waits for the others
/// (`--all` on 2026-11-12: unit:mobfarm finished 1,208 s after it started).
const UNIT_TIMEOUT: u64 = 3600;

/// Crates compiled only for wasm32, empty on native targets (no native test can run): their
/// piece builds them for the browser as `mctas web bundle` does, where the toolchain has the
/// wasm32 target.
const WASM_ONLY: &[&str] = &["mcview-web"];

/// A wasm-only crate's piece (`unit:<crate>`): `cargo build -p <crate> --profile web --target
/// wasm32-unknown-unknown`. With no wasm32 target on the toolchain, a change to the crate fails
/// (it must be built somewhere that can) and a full run says it was not built, as clippy's
/// absence is said.
fn wasm_piece(krate: &str, files: &[String]) -> Piece {
    let argv: Vec<String> = ["?@cargo", "build", "-p", krate, "--profile", "web", "--target", "wasm32-unknown-unknown"].iter().map(|s| s.to_string()).collect();
    let name = krate.to_string();
    let changed = !files.is_empty();
    let pred: crate::selftest::Pred = Box::new(move |o: &[String]| {
        let out = &o[0];
        let missing = out.contains("target may not be installed") || out.contains("can't find crate for `core`") || out.contains("can't find crate for `std`");
        if missing {
            return if changed {
                Err(format!("a change to {name} must build for wasm32, and the wasm32-unknown-unknown target is not on this toolchain: run `mctas test` where it is"))
            } else {
                Ok(format!("{NOT_RUN}{name} not built: the wasm32-unknown-unknown target is not on this toolchain ({name} is wasm only; `mctas web bundle` builds it where it is)"))
            };
        }
        if let Some(e) = out.lines().find(|l| l.starts_with("error")) {
            return Err(e.to_string());
        }
        if out.lines().any(|l| l.trim_start().starts_with("Finished")) {
            Ok(format!("{name} builds for wasm32 (its native build is empty)"))
        } else {
            Err(format!("cargo build of {name} for wasm32 did not finish: {}", crate::selftest::tail(out)))
        }
    });
    let check = Check { name: format!("unit:{krate}"), what: "a wasm-only crate built for the browser (`cargo build -p <crate> --profile web --target wasm32-unknown-unknown`)", quick: true, jvm: false, steps: vec![argv], timeout: UNIT_TIMEOUT, pred };
    let why = if files.is_empty() { vec!["--all: every crate's own check".to_string()] } else { files.iter().map(|f| format!("{f}: its crate builds for wasm32 (no native test can run)")).collect() };
    Piece { check, group: "unit".into(), workers: 0, cpu: 16, cost: 90, why }
}

/// A design's own judge as a piece.
fn design_piece(path: &str) -> Option<Piece> {
    let (study, argv) = design_judge(path)?;
    let pred: crate::selftest::Pred = match study {
        "door" => Box::new(|o: &[String]| o[0].lines().find(|l| l.starts_with("WORKS:")).map(|l| l.chars().take(120).collect()).ok_or_else(|| format!("not WORKS: {}", crate::selftest::tail(&o[0])))),
        "clock" => Box::new(|o: &[String]| o[0].lines().find(|l| l.starts_with("CLOCK: ") && l.ends_with("robust")).map(|l| l.chars().take(140).collect()).ok_or_else(|| format!("not a robust clock: {}", crate::selftest::tail(&o[0])))),
        "circuit" => Box::new(|o: &[String]| {
            o[0].lines().find(|l| l.starts_with("CIRCUIT: ") && l.contains("robust")).map(|l| l.chars().take(140).collect()).ok_or_else(|| format!("not robust: {}", crate::selftest::tail(&o[0])))
        }),
        _ => Box::new(|o: &[String]| Ok(crate::selftest::tail(&o[0]))),
    };
    let (workers, cpu) = match study {
        "door" => (1, 2),
        "gadget" => (1, 2),
        _ => (8, 16),
    };
    let name = format!("design:{}", path.trim_start_matches("designs/"));
    let what = match study {
        "door" => "the door judge (the record judge: 32 positions x 3 cycles, the rotation check) on the changed design",
        "clock" => "the clock judge (the real game) on the changed design",
        "circuit" => "the circuit judge (its kind's runs, every build) on the changed design",
        _ => "the gadget's drive script (every expect) on the changed gadget",
    };
    let check = Check { name, what, quick: false, jvm: true, steps: vec![argv], timeout: 3600, pred };
    Some(Piece { check, group: format!("design ({study})"), workers, cpu, cost: 300, why: vec![format!("{path}: a changed {study} design")] })
}

/// The machine's room: CPU threads (cores minus the load already there) and JVM workers (two
/// threads each, 1.3 GB of free memory each).
pub fn machine() -> (usize, usize, String) {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
    let load = std::fs::read_to_string("/proc/loadavg").ok().and_then(|s| s.split_whitespace().next().and_then(|x| x.parse::<f64>().ok())).unwrap_or(0.0);
    let free_cpu = cores.saturating_sub(load.ceil() as usize).max(4);
    let mem_kb = std::fs::read_to_string("/proc/meminfo").ok().and_then(|s| s.lines().find(|l| l.starts_with("MemAvailable:")).and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse::<f64>().ok()))).unwrap_or(16.0 * 1048576.0);
    let mem_gb = mem_kb / 1048576.0;
    let jvms = ((free_cpu / 2).min((mem_gb / 1.3) as usize)).max(1);
    (free_cpu, jvms, format!("{cores} cores (load {load:.1}), {mem_gb:.0} GB free"))
}

/// MCTAS_MAX_WORKERS: the cap of every pool (sim.rs `Pool::spawn`).
pub fn pool_cap() -> Option<usize> {
    std::env::var("MCTAS_MAX_WORKERS").ok().and_then(|v| v.parse::<usize>().ok()).filter(|n| *n > 0)
}

pub struct RunOpts {
    pub cpu: usize,
    pub jvms: usize,
    pub keep: bool,
    pub fail_fast: bool,
    pub version: String,
}

struct Slots {
    cpu: usize,
    jvm: usize,
    running: usize,
}

/// Run the pieces in parallel (longest first; each admitted when its CPU threads and JVM workers
/// fit, one at a time when it alone is bigger than the machine) and report. Ok(()) iff every
/// piece passed.
pub fn run(pieces: Vec<Piece>, o: &RunOpts, commit_line: &str) -> Result<()> {
    if !Path::new("designs/piston-door-3x3").is_dir() {
        bail!("run `mctas test` from the repository root (designs/piston-door-3x3 not found)");
    }
    let exe = std::env::current_exe().context("current exe")?;
    let root: PathBuf = std::env::temp_dir().join(format!("mctas-test-{}", std::process::id()));
    std::fs::create_dir_all(&root)?;
    if pieces.iter().any(|p| p.workers > 0 || p.check.jvm) {
        // the bridge once, before any pool starts (pools started together would build it together)
        mctas::bridge::build(&o.version).context("building the bridge")?;
    }
    let cap = pool_cap();
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    order.sort_by(|a, b| pieces[*b].cost.cmp(&pieces[*a].cost).then(a.cmp(b)));
    let queue = Mutex::new(order);
    let slots = Arc::new((Mutex::new(Slots { cpu: o.cpu, jvm: o.jvms, running: 0 }), Condvar::new()));
    let stop = AtomicBool::new(false);
    let t0 = Instant::now();
    let results: Mutex<Vec<(usize, bool, String, f64)>> = Mutex::new(Vec::new());
    let threads = pieces.len().clamp(1, o.cpu.max(1));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                // take the first queued piece that fits (or any piece when nothing runs)
                let (k, workers, cpu) = {
                    let (m, cv) = &*slots;
                    let mut sl = m.lock().unwrap();
                    loop {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        let mut q = queue.lock().unwrap();
                        if q.is_empty() {
                            return;
                        }
                        let fit = q.iter().position(|&k| {
                            let p = &pieces[k];
                            let w = grant(p.workers, cap, o.jvms);
                            (w <= sl.jvm && p.cpu.min(o.cpu) <= sl.cpu) || sl.running == 0
                        });
                        if let Some(i) = fit {
                            let k = q.remove(i);
                            let p = &pieces[k];
                            let w = grant(p.workers, cap, o.jvms);
                            let c = p.cpu.min(o.cpu);
                            sl.jvm = sl.jvm.saturating_sub(w);
                            sl.cpu = sl.cpu.saturating_sub(c);
                            sl.running += 1;
                            break (k, w, c);
                        }
                        drop(q);
                        sl = cv.wait_timeout(sl, Duration::from_millis(500)).unwrap().0;
                    }
                };
                let p = &pieces[k];
                let tmp = root.join(p.check.name.replace([':', '/'], "_"));
                let _ = std::fs::create_dir_all(&tmp);
                let tc = Instant::now();
                let mut outs = Vec::new();
                let mut err = None;
                for (i, st) in p.check.steps.iter().enumerate() {
                    let argv: Vec<String> = st.iter().map(|a| a.replace("{tmp}", &tmp.display().to_string()).replace("{workers}", &workers.max(1).to_string()).replace("{threads}", &cpu.max(1).to_string())).collect();
                    match run_step(&exe, &argv, &tmp, i, Duration::from_secs(p.check.timeout), Some(&stop)) {
                        Ok(t) => outs.push(t),
                        Err(e) => {
                            err = Some(e);
                            break;
                        }
                    }
                }
                {
                    let (m, cv) = &*slots;
                    let mut sl = m.lock().unwrap();
                    sl.jvm += workers;
                    sl.cpu += cpu;
                    sl.running -= 1;
                    cv.notify_all();
                }
                let (ok, msg) = verdict(p, &outs, err);
                let secs = tc.elapsed().as_secs_f64();
                println!("{} {:<40} {:>6.1} s  [{:>5.0} s]  {}", if !ok { "FAIL" } else if msg.starts_with(NOT_RUN) { "SKIP" } else { "PASS" }, p.check.name, secs, t0.elapsed().as_secs_f64(), msg);
                if !ok && o.fail_fast {
                    stop.store(true, Ordering::Relaxed);
                }
                results.lock().unwrap().push((k, ok, msg, secs));
            });
        }
    });
    let res = results.into_inner().unwrap();
    let failed: Vec<&str> = res.iter().filter(|r| !r.1).map(|r| pieces[r.0].check.name.as_str()).collect();
    let skipped = pieces.len() - res.len();
    let mut by_time: Vec<&(usize, bool, String, f64)> = res.iter().collect();
    by_time.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());
    let slow: Vec<String> = by_time.iter().take(5).map(|r| format!("{} {:.0} s", pieces[r.0].check.name, r.3)).collect();
    let total = t0.elapsed().as_secs_f64();
    let line = summary(&res.iter().map(|r| (pieces[r.0].check.name.as_str(), r.1, r.2.as_str())).collect::<Vec<_>>());
    println!("mctas test: {line}{} in {total:.0} s; slowest: {}", if skipped > 0 { format!(", {skipped} not run (--fail-fast)") } else { String::new() }, slow.join(", "));
    if failed.is_empty() && skipped == 0 {
        // a piece this box could not run is never folded into "passed" (ironspec 2026-11-12 20:35)
        if line.contains("not run") || line.contains("clippy") || line.contains("no unit tests") {
            println!("{commit_line}: {line} in {total:.0} s");
        } else {
            println!("{commit_line}: all PASS in {total:.0} s");
        }
        if !o.keep {
            let _ = std::fs::remove_dir_all(&root);
        }
        return Ok(());
    }
    bail!("mctas test FAILED: {} (outputs in {})", failed.join(", "), root.display())
}

/// A piece's verdict from its steps' outputs, or the error of the step that stopped it: the
/// predicate judges the outputs, so a zero-step piece (`unit:workspace`) is judged on no output.
fn verdict(p: &Piece, outs: &[String], err: Option<String>) -> (bool, String) {
    match err {
        Some(e) => (false, e),
        None => match (p.check.pred)(outs) {
            Ok(m) => (true, m),
            Err(m) => (false, m),
        },
    }
}

/// The verdict a piece gives when this box cannot run it (a wasm-only crate with no wasm32 target):
/// the piece shows SKIP and the summary counts it apart, never as passed.
pub const NOT_RUN: &str = "NOT RUN: ";

/// The summary's counts over (name, ok, verdict): passed (and, as information, how many of them
/// are crates with no unit tests, so "passed" never overstates: ironspec 2026-11-12 19:51 PT (4b)),
/// failed, the pieces this box could not run (named, with their reason) and the unit pieces whose
/// clippy was not on the toolchain.
fn summary(res: &[(&str, bool, &str)]) -> String {
    let failed = res.iter().filter(|r| !r.1).count();
    let not_run: Vec<String> = res.iter().filter(|r| r.1 && r.2.starts_with(NOT_RUN)).map(|r| format!("{} ({})", r.0, r.2.trim_start_matches(NOT_RUN))).collect();
    let no_tests: Vec<&str> = res.iter().filter(|r| r.1 && r.2.starts_with("no unit tests in ")).map(|r| r.0.trim_start_matches("unit:")).collect();
    let no_clippy = res.iter().filter(|r| r.1 && r.2.contains("clippy is not on this toolchain")).count();
    let mut s = format!("{} passed", res.len() - failed - not_run.len());
    if !no_tests.is_empty() {
        s += &format!(" ({} of them crates with no unit tests, built only: {})", no_tests.len(), no_tests.join(", "));
    }
    s += &format!(", {failed} failed");
    if !not_run.is_empty() {
        s += &format!(", {} not run on this box: {}", not_run.len(), not_run.join("; "));
    }
    if no_clippy > 0 {
        s += &format!(", clippy not run in {no_clippy} unit piece(s) (not on this toolchain)");
    }
    s
}

/// A piece's JVM workers: what it can use, capped by MCTAS_MAX_WORKERS (every pool's cap) and
/// by the machine.
fn grant(want: usize, cap: Option<usize>, jvms: usize) -> usize {
    if want == 0 {
        return 0;
    }
    want.min(cap.unwrap_or(usize::MAX)).min(jvms).max(1)
}

/// The commit message's line: which pieces ran, by group.
pub fn commit_line(pieces: &[Piece], base: &str, flags: &str) -> String {
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for p in pieces {
        *by.entry(p.group.clone()).or_default() += 1;
    }
    let groups: Vec<String> = by.iter().map(|(g, n)| format!("{g} {n}")).collect();
    let git = |args: &[&str]| Command::new("git").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let head = git(&["rev-parse", "--short", "HEAD"]);
    let dirty = if git(&["status", "--porcelain"]).is_empty() { "" } else { " + uncommitted changes" };
    let vs = if base.is_empty() { String::new() } else { format!(" vs {}", &base[..base.len().min(9)]) };
    let flags = if flags.is_empty() { String::new() } else { format!(" {flags}") };
    format!("Tests: mctas test{flags} ({}; {} pieces) on {head}{dirty}{vs}", groups.join(", "), pieces.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_check_has_a_row() {
        for c in checks() {
            let (_, _, workers, ..) = row(&c.name);
            // a check that starts the game holds JVM workers in the plan, one that does not none
            assert_eq!(c.jvm, workers > 0, "{}: jvm {} but {workers} workers in testplan::ROWS", c.name, c.jvm);
        }
    }

    #[test]
    fn rules_cover_every_crate() {
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for e in std::fs::read_dir(&crates).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let probe = format!("crates/{name}/src/lib.rs");
            assert!(pick(&probe).is_some(), "no testplan::RULES row for crates/{name}");
        }
        // every tracked file outside designs/ (designs have their own picking) has a rule
        let root = crates.join("..");
        let o = Command::new("git").args(["ls-files"]).current_dir(&root).output().unwrap();
        let files = String::from_utf8_lossy(&o.stdout).to_string();
        for f in files.lines().filter(|f| !f.starts_with("designs/")) {
            assert!(pick(f).is_some(), "no testplan::RULES row for {f}");
        }
    }

    #[test]
    fn a_redsim_change_picks_no_judge_and_a_judge_change_no_live_simulator_test() {
        let mut out = Vec::new();
        let p = plan(Some(&["crates/redsim/src/piston/moving.rs".to_string()]), &[], &[], false, true, &mut out);
        assert!(!p.is_empty() && p.iter().all(|p| p.group.starts_with("sim")), "{:?}", p.iter().map(|p| &p.check.name).collect::<Vec<_>>());
        assert!(p.iter().any(|p| p.check.name.starts_with("sim:fuzz-pistons-")) && !p.iter().any(|p| p.check.name == "sim:fuzz-sculk"));
        // a study's code: its checks, and the recordings that replay through it (seconds), never a
        // live fuzzer
        let p = plan(Some(&["crates/circuit/src/sorter.rs".to_string()]), &[], &[], false, true, &mut out);
        assert!(p.iter().all(|p| !p.group.starts_with("sim") || p.group == "sim traces") && p.iter().any(|p| p.check.name == "circuit:sorter-judge"));
        assert!(!p.iter().any(|p| p.check.name.starts_with("judge:") || p.check.name.starts_with("clock:")));
        // docs and web/: nothing; the site's code: its crate's own tests only
        let p = plan(Some(&["docs/tools.md".to_string(), "web/site/index.html".to_string()]), &[], &[], false, true, &mut out);
        assert!(p.is_empty());
        let p = plan(Some(&["crates/site/src/lib.rs".to_string()]), &[], &[], false, true, &mut out);
        assert_eq!(p.iter().map(|p| p.name()).collect::<Vec<_>>(), ["unit:site"]);
    }

    #[test]
    fn a_full_suite_runs_every_unit_piece() {
        // ironspec 2026-11-12 18:27: `--all` never runs less than a targeted run; every crate's
        // own tests, redsim's in sim:redsim-tests (19:42 PT)
        let units = unit_crates().expect("the workspace's crates, from the git root");
        for k in ["mobfarm", "farmopt-iron", "mctas", "circuit", "clock", "piston-door", "door-judge", "redsim-fuzz"] {
            assert!(units.iter().any(|u| u == k), "{k} missing from {units:?}");
        }
        assert!(!units.iter().any(|u| u == "redsim"), "redsim's tests run in sim:redsim-tests");
        // an unreadable workspace is an error, never an empty list: a full run plans a
        // unit:workspace piece that the runner judges FAIL on no output
        assert!(unit_crates_in(Path::new("/")).is_err());
        let p = plan_in(Some(&[]), &[], &[], true, true, &mut Vec::new(), &|| Err::<Vec<String>, String>("crates: unreadable".to_string()));
        let ws = p.iter().find(|p| p.name() == "unit:workspace").expect("unit:workspace planned");
        assert!(ws.check.steps.is_empty());
        assert_eq!(verdict(ws, &[], None), (false, "cannot list the workspace's crates: crates: unreadable".to_string()));
        // a file directly under crates/ belongs to no crate
        assert_eq!(crate_of("crates/README.md"), None);
        assert_eq!(crate_of("crates/mobfarm/src/lib.rs"), Some("mobfarm"));
        let mut out = Vec::new();
        for p in [plan(Some(&[]), &[], &[], true, true, &mut out), plan(Some(&["crates/piston-door/src/testplan.rs".to_string()]), &[], &[], false, true, &mut out)] {
            let names: Vec<&str> = p.iter().map(|p| p.name()).collect();
            for k in &units {
                assert!(names.contains(&format!("unit:{k}").as_str()), "unit:{k} missing from a full run");
            }
            assert!(names.contains(&"sim:redsim-tests") && !names.contains(&"unit:redsim"));
        }
    }

    #[test]
    fn a_redsim_change_runs_its_tests_in_sim_redsim_tests() {
        // redsim's own tests run in sim:redsim-tests (TESTS_RUN_BY), which every redsim rule picks;
        // no unit:redsim beside it
        let all = checks();
        for (k, check) in TESTS_RUN_BY {
            let c = all.iter().find(|c| c.name == *check).unwrap_or_else(|| panic!("{check} is not a check"));
            assert!(c.steps.iter().any(|s| s.join(" ").starts_with(&format!("@cargo test --release -p {k} "))), "{check} does not run {k}'s tests: {:?}", c.steps);
        }
        let mut out = Vec::new();
        for (prefix, _) in RULES.iter().filter(|(p, _)| p.starts_with("crates/redsim/")) {
            let probe = if prefix.ends_with(".rs") { prefix.to_string() } else if prefix.ends_with('/') { format!("{prefix}lib.rs") } else { format!("{prefix}.rs") };
            let names: Vec<String> = plan(Some(&[probe.clone()]), &[], &[], false, true, &mut out).iter().map(|p| p.name().to_string()).collect();
            assert!(names.iter().any(|n| n == "sim:redsim-tests") && !names.iter().any(|n| n == "unit:redsim"), "{probe}: {names:?}");
        }
        // any other crate's change runs its own tests beside its calibration pieces
        let names: Vec<String> = plan(Some(&["crates/piston-door/src/lib.rs".to_string()]), &[], &[], false, true, &mut out).iter().map(|p| p.name().to_string()).collect();
        assert!(names.iter().any(|n| n == "unit:piston-door") && names.iter().any(|n| n.starts_with("judge:")), "{names:?}");
    }

    #[test]
    fn a_code_change_never_picks_nothing() {
        let mut out = Vec::new();
        // the mob farm tools: one piece for the crate, every touched file behind it
        let p = plan(Some(&["crates/mobfarm/src/bedtier.rs".to_string(), "crates/mobfarm/src/cli.rs".to_string()]), &[], &[], false, true, &mut out);
        assert_eq!(p.iter().map(|p| p.name()).collect::<Vec<_>>(), ["unit:mobfarm"], "{out:?}");
        assert_eq!(p[0].why.len(), 2);
        assert_eq!(p[0].check.steps[0].join(" "), "?@cargo test --release -p mobfarm --all-targets");
        assert_eq!(p[0].check.steps[1].join(" "), "?@cargo clippy --release -p mobfarm --all-targets");
        // a prefix rule ("crates/farmopt") and a study file inside a judge crate: each its own crate
        let p = plan(Some(&["crates/farmopt-iron/src/lib.rs".to_string(), "crates/circuit/src/studies/mobfarm.rs".to_string()]), &[], &[], false, true, &mut out);
        assert_eq!(p.iter().map(|p| p.name()).collect::<Vec<_>>(), ["unit:circuit", "unit:farmopt-iron"]);
        // every rule under crates/ picks at least one piece
        for (prefix, _) in RULES.iter().filter(|(p, _)| p.starts_with("crates/")) {
            let probe = if prefix.ends_with(".rs") { prefix.to_string() } else if prefix.ends_with('/') { format!("{prefix}src/lib.rs") } else { format!("{prefix}/src/lib.rs") };
            let p = plan(Some(&[probe.clone()]), &[], &[], false, true, &mut out);
            assert!(!p.is_empty(), "{probe} (rule {prefix}) picks no piece");
        }
        // the unit piece's verdict, three outcomes: no "test result:" line is a build failure (its
        // tail printed), any result not ok fails, every result ok passes, a crate with 0 tests
        // saying so (no code gate: the parent's ruling, ironspec 19:42 PT)
        let u = unit_piece("trends", &["crates/trends/src/lib.rs".to_string()]);
        let pred = |a: &str, b: &str| (u.check.pred)(&[a.to_string(), b.to_string()]);
        assert!(pred("   Compiling trends v0.1.0\nerror[E0425]: cannot find value `x` in this scope\nerror: could not compile `trends` (lib) due to 1 previous error", "").is_err_and(|e| e.contains("did not build") && e.contains("E0425")));
        assert!(pred("test result: ok. 0 passed; 0 failed; 0 ignored", "").is_ok_and(|s| s.starts_with("no unit tests in trends (it builds; 1 test binaries)")));
        // no test binary ran: a crate with no test target (`test = false`) that built passes; one
        // that never finished, or printed an error, did not build (ironspec 19:51 PT (4a))
        assert!(pred("   Compiling trends v0.1.0\n    Finished `release` profile [optimized] target(s) in 3.2s", "").is_ok_and(|s| s.starts_with("no unit tests in trends (it builds; no test target)")));
        assert!(pred("   Compiling trends v0.1.0", "").is_err_and(|e| e.contains("did not build")));
        assert!(pred("warning: unused manifest key\nerror: failed to load manifest for workspace member\n    Finished `release` profile", "").is_err_and(|e| e.contains("did not build")));
        assert!(pred("test result: ok. 3 passed; 0 failed\ntest result: FAILED. 1 passed; 1 failed", "").is_err());
        // a binary killed after an earlier one passed: cargo's own failure line decides
        let killed = "test result: ok. 3 passed; 0 failed; 0 ignored\nerror: test failed, to rerun pass `--test big`\n\nCaused by:\n  process didn't exit successfully: `/t/target/release/deps/big-0f1e` (signal: 9, SIGKILL: kill)";
        assert!(pred(killed, "").is_err_and(|e| e.contains("SIGKILL")));
        let ok = "test result: ok. 3 passed; 0 failed\ntest result: ok. 2 passed; 0 failed";
        assert_eq!(pred(ok, "    Finished `release` profile"), Ok("5 trends tests pass (2 test binaries); clippy: 0 warning(s) in the changed files; 0 error(s) and 0 warning(s) in the crate, counted, not failing".to_string()));
        assert_eq!(pred(ok, "warning: unused\n  --> crates/trends/src/lib.rs:3:5\nwarning: unused\n  --> crates/redsim/src/lib.rs:9:1"), Ok("5 trends tests pass (2 test binaries); clippy: 1 warning(s) in the changed files; 0 error(s) and 2 warning(s) in the crate, counted, not failing".to_string()));
        // clippy, scoped to the change: a deny-level error located in a changed file fails, by its
        // `-->` line (a multi-line diagnostic), not its header; one in an untouched file is counted
        let in_changed = "error: this comparison involving the minimum or maximum element\n   |\n  --> crates/trends/src/lib.rs:3:5\n   |\n3  |     x < u32::MIN\n   = help: because `u32::MIN` is the minimum value";
        assert!(pred(ok, in_changed).is_err_and(|e| e.contains("crates/trends/src/lib.rs")));
        let elsewhere = "error: this comparison involving the minimum or maximum element\n  --> crates/trends/src/main.rs:9:1\nerror: could not compile `trends` (bin \"trends\") due to 1 previous error";
        assert_eq!(pred(ok, elsewhere), Ok("5 trends tests pass (2 test binaries); clippy: 0 warning(s) in the changed files; 1 error(s) and 0 warning(s) in the crate, counted, not failing".to_string()));
        // a full run (no changed files) never fails on clippy
        let all = unit_piece("trends", &[]);
        assert!((all.check.pred)(&[ok.to_string(), in_changed.to_string()]).is_ok_and(|s| s.ends_with("clippy: 1 error(s) and 0 warning(s) in the crate, counted, not failing")));
        assert_eq!(pred(ok, "error: no such command: `clippy`"), Ok("5 trends tests pass (2 test binaries); clippy is not on this toolchain: not run".to_string()));
        // a wasm-only crate: built for wasm32; with no wasm32 target a full run says it was not
        // built and a change to it fails; any other error fails
        let w = unit_piece("mcview-web", &[]);
        assert_eq!(w.check.steps[0].join(" "), "?@cargo build -p mcview-web --profile web --target wasm32-unknown-unknown");
        let missing = "error[E0463]: can't find crate for `core`\n  = note: the `wasm32-unknown-unknown` target may not be installed".to_string();
        assert!((w.check.pred)(std::slice::from_ref(&missing)).is_ok_and(|s| s.starts_with(NOT_RUN) && s.contains("not built")));
        let wc = unit_piece("mcview-web", &["crates/mcview-web/src/lib.rs".to_string()]);
        assert!((wc.check.pred)(std::slice::from_ref(&missing)).is_err());
        assert!((wc.check.pred)(&["    Finished `web` profile [optimized] target(s) in 40.2s".to_string()]).is_ok());
        assert!((wc.check.pred)(&["error: could not compile `mcview-web` (lib) due to 1 previous error".to_string()]).is_err());
        // the summary never folds a piece the box could not run into "passed" (ironspec 20:35)
        let nr = format!("{NOT_RUN}mcview-web not built: no wasm32 target");
        assert_eq!(summary(&[("unit:site", true, "103 site tests pass (1 test binaries); clippy is not on this toolchain: not run"), ("unit:mcview-web", true, &nr), ("sim:x", true, "ok")]), "2 passed, 0 failed, 1 not run on this box: unit:mcview-web (mcview-web not built: no wasm32 target), clippy not run in 1 unit piece(s) (not on this toolchain)");
        assert_eq!(summary(&[("sim:x", true, "ok"), ("sim:y", false, "bad")]), "1 passed, 1 failed");
        // crates with no unit tests are counted apart, as information (ironspec 19:51 PT (4b))
        assert_eq!(summary(&[("unit:bud", true, "no unit tests in bud (it builds; 1 test binaries); clippy: 0 error(s) and 0 warning(s) in the crate, counted, not failing"), ("unit:site", true, "103 site tests pass (1 test binaries); clippy: 0 error(s) and 0 warning(s) in the crate, counted, not failing")]), "2 passed (1 of them crates with no unit tests, built only: bud), 0 failed");
    }
}
