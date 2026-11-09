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
//! * docs, the site, the viewer and the videos pick nothing (publishing runs no tests).
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
    /// nothing, and why
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
    ("sim:fuzz-redstone", Group::Sim, "redstone", 8, 16, 90),
    ("sim:fuzz-containers", Group::Sim, "containers", 8, 16, 90),
    ("sim:fuzz-pistons", Group::Sim, "pistons", 8, 16, 90),
    ("sim:fuzz-sculk", Group::Sim, "sculk", 8, 16, 90),
    ("sim:vanilla-", Group::Sim, "vanilla", 2, 4, 150),
    ("sim:terrain-ticks-", Group::Sim, "terrain", 1, 2, 330),
    ("sim:autodoor-twin", Group::Sim, "traces", 0, 1, 90),
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
fn pick_design_file(f: &str, p: &mut Picks, rec_deps: &BTreeMap<String, Vec<String>>, designs: &mut Vec<String>) -> String {
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
    if what.is_empty() {
        let why = if !Path::new(f).exists() { "deleted; no piece reads it" } else if calibration { "a calibration, selftest or test-fixture file no piece reads" } else { "not a design a judge takes; no piece reads it" };
        format!("  {f} -> nothing ({why})")
    } else {
        format!("  {f} -> {}", what.join("; "))
    }
}

/// Pick the pieces of a change: `files` (None: git could not say, so everything), explicit
/// `groups` and `designs`, or `everything` (`--all`). The plan's lines go to `out`.
pub fn plan(files: Option<&[String]>, groups: &[Group], designs: &[String], everything: bool, out: &mut Vec<String>) -> Vec<Piece> {
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
    for f in files.unwrap_or(&[]) {
        if f.starts_with("designs/") {
            let line = pick_design_file(f, &mut p, &rec_deps, &mut designs);
            out.push(line);
            continue;
        }
        let names = |gs: &[Group]| gs.iter().map(|g| g.name()).collect::<Vec<_>>().join(", ");
        match pick(f) {
            Some(Pick::Everything) => {
                out.push(format!("  {f} -> everything (every piece goes through it)"));
                p.everything(&format!("{f}: every piece goes through it"));
            }
            Some(Pick::Groups(gs)) => {
                out.push(format!("  {f} -> the {} checks", names(gs)));
                for g in gs {
                    p.group(*g, &[], f);
                }
            }
            Some(Pick::Sim(areas)) => {
                out.push(format!("  {f} -> sim ({})", areas.join(", ")));
                p.group(Group::Sim, areas, f);
            }
            Some(Pick::SimAnd(areas, gs)) => {
                out.push(format!("  {f} -> sim ({}) and the {} checks", areas.join(", "), names(gs)));
                p.group(Group::Sim, areas, f);
                for g in gs {
                    p.group(*g, &[], f);
                }
            }
            Some(Pick::Nothing(what)) => out.push(format!("  {f} -> nothing ({what})")),
            None => {
                out.push(format!("  {f} -> everything (no rule in testplan::RULES for it: add one)"));
                p.everything(&format!("{f}: no rule, everything"));
            }
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
    pieces
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
                let (ok, msg) = match err {
                    Some(e) => (false, e),
                    None => match (p.check.pred)(&outs) {
                        Ok(m) => (true, m),
                        Err(m) => (false, m),
                    },
                };
                let secs = tc.elapsed().as_secs_f64();
                println!("{} {:<40} {:>6.1} s  [{:>5.0} s]  {}", if ok { "PASS" } else { "FAIL" }, p.check.name, secs, t0.elapsed().as_secs_f64(), msg);
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
    println!("mctas test: {} passed, {} failed{} in {total:.0} s; slowest: {}", res.len() - failed.len(), failed.len(), if skipped > 0 { format!(", {skipped} not run (--fail-fast)") } else { String::new() }, slow.join(", "));
    if failed.is_empty() && skipped == 0 {
        println!("{commit_line}: all PASS in {total:.0} s");
        if !o.keep {
            let _ = std::fs::remove_dir_all(&root);
        }
        return Ok(());
    }
    bail!("mctas test FAILED: {} (outputs in {})", failed.join(", "), root.display())
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
        let p = plan(Some(&["crates/redsim/src/piston/moving.rs".to_string()]), &[], &[], false, &mut out);
        assert!(!p.is_empty() && p.iter().all(|p| p.group.starts_with("sim")), "{:?}", p.iter().map(|p| &p.check.name).collect::<Vec<_>>());
        assert!(p.iter().any(|p| p.check.name == "sim:fuzz-pistons") && !p.iter().any(|p| p.check.name == "sim:fuzz-sculk"));
        // a study's code: its checks, and the recordings that replay through it (seconds), never a
        // live fuzzer
        let p = plan(Some(&["crates/circuit/src/sorter.rs".to_string()]), &[], &[], false, &mut out);
        assert!(p.iter().all(|p| !p.group.starts_with("sim") || p.group == "sim traces") && p.iter().any(|p| p.check.name == "circuit:sorter-judge"));
        assert!(!p.iter().any(|p| p.check.name.starts_with("judge:") || p.check.name.starts_with("clock:")));
        let p = plan(Some(&["docs/tools.md".to_string(), "web/site/index.html".to_string(), "crates/site/src/lib.rs".to_string()]), &[], &[], false, &mut out);
        assert!(p.is_empty());
    }
}
