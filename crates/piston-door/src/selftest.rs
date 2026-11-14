//! `door selftest`: the known answers of every search and judge tool and of the simulator, the
//! table `mctas test` picks its pieces from (testplan.rs: the pieces of a change, AGENTS.md).
//! Each check runs real `mctas ...` commands (this binary, as child processes, from the
//! repository root) on known answers and compares their output with what must come out: archived
//! anchors that WORK, regions the pocket search must re-find, model checks with zero mismatches, a
//! candidate the update-order model must reject, the SAT model re-finding a known wiring, `door
//! ideal-rs` agreeing with `door ideal`, generator membership that must not change silently, and
//! the simulator's recorded game answers (`sim:recorded-*`: exact game traces replayed on redsim,
//! stale recordings failing) and differential fuzzers (`sim:fuzz-*`). One line per check, PASS or
//! FAIL. `door selftest` runs them all (`--quick`: the ones that start no JVM); `mctas test` runs
//! the ones a change picks. A check whose known answer changes on purpose (e.g. a generator that
//! now admits fast-clean) is updated here in the same commit, with the reason.

use anyhow::Result;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub struct Opts {
    pub quick: bool,
    pub only: Option<String>,
    pub list: bool,
    pub jobs: usize,
    pub jvm_jobs: usize,
    pub keep: bool,
}

pub(crate) type Pred = Box<dyn Fn(&[String]) -> std::result::Result<String, String> + Send + Sync>;

pub(crate) struct Check {
    pub(crate) name: String,
    pub(crate) what: &'static str,
    /// in the old quick subset (kept for `door selftest --list`)
    pub(crate) quick: bool,
    /// starts the game (JVM workers: testplan::ROWS sizes them)
    pub(crate) jvm: bool,
    /// commands run in order (args after `mctas`); `{tmp}` is this check's scratch folder,
    /// `{workers}` its JVM workers, `{threads}` its CPU threads (testplan.rs sizes both)
    pub(crate) steps: Vec<Vec<String>>,
    pub(crate) timeout: u64,
    pub(crate) pred: Pred,
}

/// The directories of recorded game answers the simulator pieces replay (`sim:recorded-*`), each
/// with its manifest (`recorded.tsv`, [`mctas::recorded`]): a stale recording fails.
pub const RECORDING_DIRS: &[&str] = &[
    "designs/clocks/traces/exact",
    "designs/clocks/tracks/clk-misc/redsim-tests/traces",
    "designs/clocks/tracks/clockinfra/redsim-tests/traces",
    "designs/circuits/shulker/traces/exact",
    "designs/boat-leads/traces",
    "designs/redsim/recorded/doors",
    "designs/redsim/recorded/clocks",
    "designs/redsim/recorded/lifts",
    "designs/redsim/recorded/circuits",
    "designs/redsim/recorded/sorter",
    "designs/redsim/recorded/smelter",
    "designs/redsim/recorded/brewer",
    "designs/redsim/recorded/composter",
    "designs/redsim/recorded/mobai",
    PENDING,
];

/// (tickexact 2026-11-14, G1 car-14 CHANGES 1 (3)) the pending mob AI recordings: listed in RECORDING_DIRS (their manifest
/// kept current by `recorded check`) and replayed by `sim:recorded-pending`
pub const PENDING: &str = "designs/redsim/recorded/pending";

/// The pending recordings `sim:recorded-pending` does not replay, each with why (never silently: the piece's verdict names
/// how many it excluded)
pub const PENDING_EXCLUDED: &[(&str, &str)] = &[(
    "e-string-empty.rec",
    "redsim differs from the game at its first tick: the minecart #0's own random (the game made 212 more next() calls than redsim at tick 1; redsim does not model what draws them yet), open for the gating car",
)];

const A: &str = "designs/piston-door-3x3/archive";
const C79: &str = "designs/piston-door-3x3/tracks/c79/c79-closed.txt";
const BULB: &str = "designs/piston-door-3x3/selftest/c79-bulb-periodic.txt";
/// A white-concrete door (the community 7x8's material): FULL + frame 12/12 follow the door's own
/// material in the redsim evaluator, as in `door properties`.
const CONCRETE: &str = "designs/piston-door-3x3/selftest/e78s-43-white-concrete.txt";
const ANCHORS: [&str; 5] = ["c79-community-63b-51b", "e88-t7b-43b", "t7b-nb2-43b", "s27-bl52-52b", "m79s-sculk-63b-50-50b"];
const SAT_PALETTE: &str = "block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,glass,leaves,upiston";

fn args(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

fn has(out: &str, pat: &str) -> std::result::Result<(), String> {
    if out.contains(pat) { Ok(()) } else { Err(format!("no `{pat}` in the output: {}", tail(out))) }
}

pub(crate) fn tail(out: &str) -> String {
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    let n = lines.len();
    lines[n.saturating_sub(2)..].join(" | ").chars().take(300).collect()
}

/// The verdict line of `door ideal` / `door ideal-rs` (WORKS ... / BROKEN: ...), normalized.
fn ideal_verdict(out: &str) -> Option<String> {
    out.lines().rev().find(|l| l.starts_with("WORKS") || l.starts_with("BROKEN")).map(|l| l.replace(" in redsim", ""))
}

pub(crate) fn checks() -> Vec<Check> {
    let mut v: Vec<Check> = Vec::new();
    // 1. the redsim judge on the anchors (and the periodic bulb fixture)
    let mut files: Vec<String> = ANCHORS.iter().map(|a| format!("{A}/{a}.txt")).collect();
    files.push(BULB.into());
    files.push(CONCRETE.into());
    let mut s = vec!["door".to_string(), "cosearch".into(), "eval".into()];
    s.extend(files.iter().cloned());
    let fl = files.clone();
    v.push(Check {
        name: "judge:redsim-anchors".into(),
        what: "the redsim judge (cosearch eval: 32 positions x 3 cycles, FULL, hallway, frame) on the archived anchors + the periodic bulb fixture + a white-concrete door: WORKS",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(move |o| {
            let out = &o[0];
            for f in &fl {
                let line = out.lines().find(|l| l.starts_with(f.as_str())).ok_or(format!("{f}: no verdict line"))?;
                for need in ["WORKS", "FULL", "hallway ok", "frame 12/12"] {
                    if !line.contains(need) {
                        return Err(format!("{f}: no `{need}`: {}", line.chars().take(240).collect::<String>()));
                    }
                }
            }
            Ok(format!("{} designs WORK (FULL, hallway, frame 12/12)", fl.len()))
        }),
    });
    // 2. the real judge (JVM) on the anchors
    for (i, a) in ANCHORS.iter().enumerate() {
        v.push(Check {
            name: format!("judge:verify-{a}"),
            what: "`door verify` (the real game, 4 positions x 3 cycles, rotations) on an archived anchor: WORKS",
            quick: i == 0,
            jvm: true,
            steps: vec![args(&format!("door verify {A}/{a}.txt"))],
            timeout: 600,
            pred: Box::new(|o| {
                let l = o[0].lines().find(|l| l.starts_with("WORKS:")).ok_or(format!("not WORKS: {}", tail(&o[0])))?;
                Ok(l.chars().take(120).collect())
            }),
        });
    }
    v.push(Check {
        name: "judge:verify-periodic-bulb".into(),
        what: "`door verify` on c79 + a copper bulb that toggles once per cycle (closed state of period 2): the periodic judge accepts it",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("door verify {BULB} --no-rotations"))],
        timeout: 600,
        pred: Box::new(|o| has(&o[0], "WORKS:").map(|_| "WORKS (periodic rest)".into())),
    });
    // a NEGATIVE anchor: a judge that accepts everything fails here
    const NEG: &str = "designs/piston-door-3x3/selftest/c79-no-dust.txt";
    v.push(Check {
        name: "judge:redsim-negative".into(),
        what: "the redsim judge rejects c79 with its hidden dust (-2,4) removed (the door no longer opens)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("?door cosearch eval {NEG}"))],
        timeout: 120,
        pred: Box::new(|o| {
            let line = o[0].lines().find(|l| l.starts_with(NEG)).ok_or("no verdict line".to_string())?;
            if line.contains("WORKS") { Err(format!("accepted a broken door: {}", line.chars().take(200).collect::<String>())) } else { Ok("rejected".into()) }
        }),
    });
    v.push(Check {
        name: "judge:verify-negative".into(),
        what: "`door verify` rejects c79 with its hidden dust removed",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("?door verify {NEG} --no-rotations"))],
        timeout: 600,
        pred: Box::new(|o| if o[0].lines().any(|l| l.starts_with("WORKS:")) { Err("accepted a broken door".into()) } else { has(&o[0], "BROKEN").map(|_| "rejected (BROKEN)".into()) }),
    });
    // the 2x2 door (`door-size: 2`, coordinator 2026-10-21): the same judge on 4 cells
    const C2: &str = "designs/piston-door-2x2/selftest/classic.txt";
    const R2: &str = "designs/piston-door-2x2/selftest/recessed.txt";
    v.push(Check {
        name: "judge:2x2-classic".into(),
        what: "`door verify` on the classic 2x2 (four sticky pistons on the sides, quasi-connected): WORKS, 22 blocks, 7x4x1, rotation-safe",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("door verify {C2}"))],
        timeout: 600,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.starts_with("WORKS:")).ok_or(format!("not WORKS: {}", tail(&o[0])))?;
            if !l.contains("22 blocks, 7x4x1") {
                return Err(format!("expected 22 blocks, 7x4x1: {l}"));
            }
            if !o[0].lines().any(|l| l.starts_with("rotation-safe:")) {
                return Err(format!("not rotation-safe: {}", tail(&o[0])));
            }
            Ok(l.chars().take(120).collect())
        }),
    });
    v.push(Check {
        name: "judge:2x2-recessed".into(),
        what: "`door verify` rejects the classic 2x2 declared one block in front of its blocks (not flush: the opening's cells are air when closed)",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("?door verify {R2} --no-rotations"))],
        timeout: 600,
        pred: Box::new(|o| if o[0].lines().any(|l| l.starts_with("WORKS:")) { Err("accepted a recessed 2x2".into()) } else { has(&o[0], "not closed at rest").map(|_| "rejected (not closed at rest)".into()) }),
    });
    // obsidian only where needed (vjeux 2026-10-21): the material pass and the archive guards
    const OBS_DOOR: &str = "designs/piston-door-3x3/selftest/obsidian-replaceable.txt";
    const OBS_NEEDED_DOOR: &str = "designs/piston-door-3x3/archive/c88-s26-54b.txt";
    const OBS_ROTATION_DOOR: &str = "designs/piston-door-3x3/archive/bridge-p2-sculk-r-49b.txt";
    const OBS_TNT: &str = "designs/circuits/selftest/tnt-obsidian-replaceable.txt";
    v.push(Check {
        name: "material:door-obsidian".into(),
        what: "`door material` turns the fixture's replaceable obsidian to stone and keeps c88-s26-54b's needed one (redsim screen, same ticks)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door material {OBS_DOOR} --out {{tmp}}/a --threads 4")), args(&format!("door material {OBS_NEEDED_DOOR} --out {{tmp}}/b --threads 4"))],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[0], "1 designs with obsidian: 1 changed (1 obsidian -> stone), 1 obsidian kept")?;
            has(&o[1], "1 designs with obsidian: 0 changed (0 obsidian -> stone), 1 obsidian kept")?;
            Ok("replaceable -> stone, needed kept".into())
        }),
    });
    v.push(Check {
        name: "material:archive-guards".into(),
        what: "the archive guards: `door verify --archive` refuses the door fixture (obsidian that can be stone), archives c88-s26-54b (its obsidian is needed on redsim) and bridge-p2-sculk-r-49b (stone passes redsim but breaks its 180-degree build: the game's rotation check keeps the obsidian); `circuit archive` refuses the TNT fixture",
        quick: false,
        jvm: true,
        steps: vec![
            args(&format!("?door verify {OBS_DOOR} --no-rotations --archive --archive-dir {{tmp}}/doors")),
            args(&format!("door verify {OBS_NEEDED_DOOR} --no-rotations --archive --archive-dir {{tmp}}/doors")),
            args(&format!("?circuit archive {OBS_TNT} --root {{tmp}}/circuits")),
            args(&format!("door verify {OBS_ROTATION_DOOR} --no-rotations --archive --archive-dir {{tmp}}/doors")),
        ],
        timeout: 1200,
        pred: Box::new(|o| {
            has(&o[0], "not archived: obsidian at")?;
            has(&o[1], "archived ")?;
            if o[1].contains("not archived") {
                return Err(format!("refused a needed obsidian: {}", tail(&o[1])));
            }
            has(&o[2], "not archived: obsidian at")?;
            has(&o[3], "obsidian kept: with stone the door breaks a turned build")?;
            if o[3].contains("not archived") {
                return Err(format!("refused an obsidian its turned builds need: {}", tail(&o[3])));
            }
            Ok("replaceable refused (door, TNT), needed archived (redsim, rotation)".into())
        }),
    });
    // 3. pockets re-find known regions
    let r = "-1,-3;-2,-3;-2,-2;-2,-1;-1,-2;0,-2";
    v.push(Check {
        name: "pockets:c79-bottom-left".into(),
        what: "c79's bottom-left wiring erased: plan-from --region + pockets k=5 (stone, dust, observers) re-find a filling that screens perfect",
        quick: true,
        jvm: false,
        steps: vec![
            args(&format!("door cosearch plan-from {C79} --region={r} --out {{tmp}}/q79-bl.ideal")),
            args(&format!("door cosearch pockets {C79} --box=-2,-3:4,5 --assist {{tmp}}/q79-bl.ideal --region-cells={r} --keep-lever --no-hallway --k 5 --fill-palette stone,dust,observer --threads 32 --out-dir {{tmp}}/bl")),
        ],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[1], "FIND ")?;
            let n = o[1].lines().filter(|l| l.starts_with("FIND ")).count();
            Ok(format!("{n} find line(s)"))
        }),
    });
    let c88 = "designs/piston-door-3x3/tracks/c88";
    v.push(Check {
        name: "pockets:s25-br-gaps".into(),
        what: "c88 s25 -> s26: the bottom-right chain (observer, AIR, normal piston, observer) is found with --gaps and not without",
        quick: true,
        jvm: false,
        steps: vec![
            args(&format!("door cosearch pockets {c88}/s25.txt --box=-3,-2:4,5 --assist {c88}/s25.ideal --keep-lever --gaps --k 3 --fill-palette observer,piston,stone,dust --threads 32 --out-dir {{tmp}}/br")),
            args(&format!("door cosearch pockets {c88}/s25.txt --box=-3,-2:4,5 --assist {c88}/s25.ideal --keep-lever --k 3 --fill-palette observer,piston,stone,dust --threads 32 --out-dir {{tmp}}/br0")),
        ],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[0], "FIND ")?;
            if o[1].contains("FIND ") {
                return Err("found without --gaps too (the chain needs a gap)".into());
            }
            has(&o[1], " 0 finds")?;
            Ok("found with --gaps, not without".into())
        }),
    });
    // 4. satwire
    v.push(Check {
        name: "satwire:c79-from-wired-check".into(),
        what: "satwire's model on c79's own wiring (--from-wired --check): 0 piston-tick mismatches",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door satwire {C79} --from-wired --check --palette {SAT_PALETTE}"))],
        timeout: 300,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.starts_with("check: ") && l.contains("mismatches")).ok_or(format!("no check line: {}", tail(&o[0])))?;
            if l.starts_with("check: 0 piston-tick mismatches") { Ok(l.to_string()) } else { Err(l.to_string()) }
        }),
    });
    let bud = |uo: bool| {
        args(&format!(
            "door satwire {A}/pockets-63b-50-0-50b.txt --from-wired --full-frame --forbid-ring --sound --lever-oop --log-doors --one-warp --dmax 0 --skip 0 --palette {SAT_PALETTE} --explain designs/piston-door-3x3/tracks/satwire/fixtures/bud48-pk3-s0.txt{}",
            if uo { " --update-order" } else { "" }
        ))
    };
    v.push(Check {
        name: "satwire:bud48-update-order".into(),
        what: "a saved 48-block candidate the judge rejects (a BUD: R pair under a ceiling lever): satwire --update-order rejects it",
        quick: true,
        jvm: false,
        steps: vec![bud(true)],
        timeout: 300,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.contains("REJECTS")).ok_or(format!("not rejected: {}", tail(&o[0])))?;
            if !l.contains("update-order") {
                return Err(format!("rejected, but not by the update-order clauses: {l}"));
            }
            Ok("rejected by the update-order clauses".into())
        }),
    });
    v.push(Check {
        name: "satwire:bud48-old-model".into(),
        what: "the same candidate WITHOUT --update-order is accepted by the model (the hole the update-order clauses close)",
        quick: false,
        jvm: false,
        steps: vec![bud(false)],
        timeout: 300,
        pred: Box::new(|o| if o[0].contains("REJECTS") { Err("rejected without --update-order (fixture or model changed)".into()) } else { has(&o[0], "explain").map(|_| "accepted (as expected)".into()) }),
    });
    // 5. boxproof (whole-box SAT model)
    v.push(Check {
        name: "boxproof:c79-fixed".into(),
        what: "boxproof's model admits the community 7x9 with every block fixed (--split-rounds), redsim screen Pass",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door boxproof bmc --w 7 --h 9 --dx 2 --dy 3 --t 17 --lever-on --frame12 --split-rounds --fix {C79} --fix-all --iters 2 --out {{tmp}}/fixed"))],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[0], "SAT in")?;
            has(&o[0], "screen Pass")?;
            Ok("SAT, screen Pass".into())
        }),
    });
    v.push(Check {
        name: "boxproof:c79-no-split".into(),
        what: "without --split-rounds the same instance is UNSAT (two block events of one round meet at (1,-1): the order matters)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door boxproof bmc --w 7 --h 9 --dx 2 --dy 3 --t 17 --lever-on --frame12 --fix {C79} --fix-all --iters 2 --out {{tmp}}/nosplit"))],
        timeout: 300,
        pred: Box::new(|o| has(&o[0], "UNSAT in").map(|_| "UNSAT".into())),
    });
    v.push(Check {
        name: "boxproof:c79-refind-cell".into(),
        what: "boxproof re-finds a freed c79 wiring cell (the hidden dust (-2,4)): SAT, screen Pass",
        quick: false,
        jvm: false,
        steps: vec![args(&format!("door boxproof bmc --w 7 --h 9 --dx 2 --dy 3 --t 17 --lever-on --frame12 --split-rounds --palette stone,dust,obs --fix {C79} --fix-all --free 0,7 --iters 5 --out {{tmp}}/refind"))],
        timeout: 600,
        pred: Box::new(|o| has(&o[0], "screen Pass").map(|_| "re-found, screen Pass".into())),
    });
    v.push(Check {
        name: "boxproof:fuzz".into(),
        what: "boxproof's model vs redsim on 3000 random 5x5 boxes (seed 11): no unflagged mismatch",
        quick: false,
        jvm: false,
        steps: vec![args("door boxproof fuzz --count 3000 --w 5 --h 5 --ticks 30 --threads 32 --seed 11")],
        timeout: 600,
        pred: Box::new(|o| {
            if let Some(l) = o[0].lines().find(|l| l.contains("MISMATCH (unflagged)")) {
                return Err(l.trim().to_string());
            }
            has(&o[0], "match (static)").map(|_| "0 unflagged mismatches".into())
        }),
    });
    // 6. door ideal-rs vs door ideal
    for (plan, quick) in [("designs/piston-door-3x3/tracks/bridge/s79/c78z.ideal", true), ("designs/piston-door-3x3/tracks/bridge/s79/c78z-u1.ideal", false), ("designs/piston-door-3x3/tracks/c88/s25.ideal", false)] {
        let stem = Path::new(plan).file_stem().unwrap().to_string_lossy().to_string();
        v.push(Check {
            name: format!("ideal:agree-{stem}"),
            what: "`door ideal-rs` (redsim) and `door ideal` (the game) give the same verdict and ticks on a plan, in the default tile phase and with --between-ticks (c78z: a 1 gt pulse)",
            quick,
            jvm: true,
            steps: vec![args(&format!("?door ideal-rs {plan}")), args(&format!("?door ideal {plan}")), args(&format!("?door ideal-rs {plan} --between-ticks")), args(&format!("?door ideal {plan} --between-ticks"))],
            timeout: 600,
            pred: Box::new(|o| {
                let mut out = Vec::new();
                for (k, mode) in [(0, "tile"), (2, "between")] {
                    let a = ideal_verdict(&o[k]).ok_or(format!("ideal-rs {mode}: no verdict: {}", tail(&o[k])))?;
                    let b = ideal_verdict(&o[k + 1]).ok_or(format!("ideal {mode}: no verdict: {}", tail(&o[k + 1])))?;
                    if a != b {
                        return Err(format!("{mode}: ideal-rs `{a}` vs ideal `{b}`"));
                    }
                    out.push(format!("{mode}: {}", a.chars().take(60).collect::<String>()));
                }
                Ok(out.join(" | "))
            }),
        });
    }
    // 6a. the first sub-56 mechanism (7x7 = 49b, c77 w1): ideal inputs must keep working
    const W1: &str = "designs/piston-door-3x3/tracks/c77/seven/w1.ideal";
    // 6a'. door shuttle re-finds mm01z's shuttled observer (clk-rt): P (boxed, no free neighbour)
    v.push(Check {
        name: "shuttle:mm01z".into(),
        what: "`door shuttle` on c70b's 7x7 mm01 with P's inputs as targets re-finds mm01z's shuttle (S (-2,-2) east carrying an observer at (-1,-2) facing down into (0,-2)) and only it, WORKS at 32 positions",
        quick: true,
        jvm: false,
        steps: vec![args("door shuttle designs/piston-door-3x3/selftest/shuttle-mm01.ideal --target 0,-1,1;1,-1,1")],
        timeout: 300,
        pred: Box::new(|o| {
            let line = o[0].lines().find(|l| l.contains("shuttles WORK")).ok_or(format!("no summary: {}", tail(&o[0])))?;
            if !line.starts_with("1 shuttles WORK") {
                return Err(format!("expected exactly 1 shuttle: {line}"));
            }
            has(&o[0], "S [-2, -2, 0] facing east, O [-1, -2, 0] facing down").map(|_| "1 shuttle: S (-2,-2) east + observer (-1,-2) down".into())
        }),
    });
    v.push(Check {
        name: "ideal:7x7-w1".into(),
        what: "c77's 7x7 (49b) mechanism w1 (13 sticky pistons: the center emptied by a pull after a column wiggle, ring pistons escaping to the corners) WORKS with ideal inputs (--between-ticks) in 55 / 43",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("?door ideal-rs {W1} --between-ticks"))],
        timeout: 300,
        pred: Box::new(|o| {
            let v = ideal_verdict(&o[0]).ok_or(format!("ideal-rs: no verdict: {}", tail(&o[0])))?;
            if !v.starts_with("WORKS") || !v.contains("open after 55 ticks / closed after 43 ticks") {
                return Err(format!("w1 changed: `{v}` (expected WORKS 55 / 43)"));
            }
            Ok("WORKS 55 / 43".into())
        }),
    });
    // 6b. plan phase: an observer-driven input works at plan t exactly like the wired door at t
    const PH: &str = "designs/piston-door-3x3/selftest/s25-br-observer.ideal";
    v.push(Check {
        name: "ideal:plan-phase-observer".into(),
        what: "s25 + BR driven at the ticks s26's observer powers it: ideal-rs (default tile phase) opens/closes in 34/34 like the wired s26; the old --between-ticks rule is 1 gt late (35/35)",
        quick: true,
        jvm: false,
        steps: vec![
            args(&format!("?door ideal-rs {PH}")),
            args("?door cosearch eval designs/piston-door-3x3/tracks/c88/s26.txt"),
            args(&format!("?door ideal-rs {PH} --between-ticks")),
        ],
        timeout: 300,
        pred: Box::new(|o| {
            let plan = ideal_verdict(&o[0]).ok_or(format!("ideal-rs: no verdict: {}", tail(&o[0])))?;
            let wired = o[1].lines().find(|l| l.contains("s26.txt:")).ok_or(format!("s26: no verdict: {}", tail(&o[1])))?;
            if !wired.contains("WORKS open 34 close 34") {
                return Err(format!("the wired s26 changed: {}", wired.chars().take(120).collect::<String>()));
            }
            if !plan.contains("open after 34 ticks / closed after 34 ticks") {
                return Err(format!("tile-phase plan `{plan}` != the wired s26 (34 / 34)"));
            }
            let old = ideal_verdict(&o[2]).unwrap_or_default();
            if !old.contains("open after 35 ticks / closed after 35 ticks") {
                return Err(format!("--between-ticks gave `{old}` (expected 35 / 35)"));
            }
            Ok("plan at t = wired at t (34/34); between-ticks 35/35".into())
        }),
    });
    v.push(Check {
        name: "ideal:plan-phase-observer-jvm".into(),
        what: "the same plan in the game (`door ideal`, tile phase): 34/34 like the wired s26",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("?door ideal {PH}"))],
        timeout: 600,
        pred: Box::new(|o| {
            let v = ideal_verdict(&o[0]).ok_or(format!("no verdict: {}", tail(&o[0])))?;
            if v.contains("open after 34 ticks / closed after 34 ticks") { Ok(v.chars().take(100).collect()) } else { Err(v) }
        }),
    });
    // 6c. the first sub-56 mechanisms in 6-tall boxes (w86): ring pistons resting in frame cells, a
    // carried ring puller, wiggles, QC-coupled stacks avoided; redsim must keep opening and closing them
    for (plan, label) in [("designs/piston-door-3x3/selftest/w86-a3s-7x6.ideal", "a3s-7x6"), ("designs/piston-door-3x3/selftest/w86-a3r-8x6.ideal", "a3r-8x6")] {
        v.push(Check {
            name: format!("ideal:w86-{label}"),
            what: "w86's 6-tall mechanisms (7x6 = 42b a3s, 8x6 = 48b a3r: ideal inputs, FULL frame 12/12, the game agrees): `door ideal-rs` WORKS at 4 positions x 2 cycles",
            quick: true,
            jvm: false,
            steps: vec![args(&format!("?door ideal-rs {plan}"))],
            timeout: 120,
            pred: Box::new(|o| {
                let v = ideal_verdict(&o[0]).ok_or(format!("no verdict: {}", tail(&o[0])))?;
                if v.starts_with("WORKS") { Ok(v.chars().take(100).collect()) } else { Err(v) }
            }),
        });
    }
    // 7. generator membership (docs/audit.md acceptance list)
    let expect: Vec<(&str, [&str; 6])> = vec![
        ("c79-closed", ["out", "IN", "IN", "IN", "IN", "IN"]),
        ("fast-clean-a1c-107b", ["out", "out", "out", "out", "out", "out"]),
        ("eligible-s4ff-49b", ["out", "out", "out", "out", "out", "out"]),
    ];
    v.push(Check {
        name: "audit:generator-membership".into(),
        what: "docs/audit.md's acceptance list: c79 IN every default generator (g2+: extended-at-rest pullers are the default since 2026-10-19), out only of g1 = --no-ext-pullers; fast-clean and eligible-s4ff outside every generator (update here when a generator fix admits them)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door audit --sections mech --only-flat {C79} {A}/fast-clean-a1c-107b.txt {A}/eligible-s4ff-49b.txt"))],
        timeout: 300,
        pred: Box::new(move |o| {
            for (name, cols) in &expect {
                let row = o[0].lines().find(|l| l.starts_with(&format!("| {name} |"))).ok_or(format!("{name}: no row"))?;
                let cells: Vec<&str> = row.split('|').map(str::trim).collect();
                // | design | mechanism | g1 .. g6 | ...
                let got: Vec<&str> = cells.iter().skip(3).take(6).cloned().collect();
                if got != cols.to_vec() {
                    return Err(format!("{name}: g1..g6 = {got:?}, expected {cols:?}"));
                }
            }
            Ok("membership as documented".into())
        }),
    });
    // 7b. ideal-input mechanisms of the smaller-door campaign (piece lists): c77's 7x7 w1 is in
    // the every-flag generators (g6, g7 = + --ring-escape); w86's 7x6 a3s is outside every one
    // (BM leaves through the center column: a 3-move path). Update when a generator admits it.
    v.push(Check {
        name: "audit:member-ideal".into(),
        what: "door audit --member smaller/ideal-mechs.txt: 7x7 w1 IN g6 and g7 only, 7x6 a3s out of g1..g7",
        quick: true,
        jvm: false,
        steps: vec![args("door audit --member designs/piston-door-3x3/tracks/c70b/smaller/ideal-mechs.txt")],
        timeout: 300,
        pred: Box::new(|o| {
            let row = |tag: &str| -> std::result::Result<Vec<String>, String> {
                let l = o[0].lines().find(|l| l.starts_with(&format!("| {tag} "))).ok_or(format!("no {tag} row"))?;
                Ok(l.split('|').map(|s| s.trim().to_string()).skip(2).take(7).collect())
            };
            let w1 = row("7x7 (-2,-2)")?;
            let a3s = row("7x6 (-2,-1)")?;
            let want_w1 = ["out", "out", "out", "out", "out", "IN", "IN"];
            if w1 != want_w1 {
                return Err(format!("w1 g1..g7 = {w1:?}, expected {want_w1:?}"));
            }
            if a3s.iter().any(|c| c != "out") {
                return Err(format!("a3s g1..g7 = {a3s:?}, expected all out"));
            }
            Ok("w1 IN g6/g7, a3s outside every generator".into())
        }),
    });
    // 8. the clock judge (docs/clocks.md): known periods on redsim, the real game agreeing, a
    // non-clock rejected
    const CAL: &str = "designs/clocks/calibration";
    let cal: Vec<(&str, &str)> = vec![
        // (periods print with their real time since 2026-10-19, vjeux: "20 gt (1 s)"; outputs are
        // exterior air cells read by the power the judge's lamp there receives since 2026-10-20:
        // bulb-observer and burnout-torch have no such cell that pulses evenly and left the list)
        ("observer-pair", "cal-observer-pair period 6 gt (0.3 s), on 2/6, edges 1, warmup 2, family observer, 2 blocks, 2x1x1"),
        ("comparator-subtract", "cal-comparator-subtract period 4 gt (0.2 s), on 2/4, edges 1, warmup 2, family comparator+redstone_wire, 9 blocks, 3x2x2"),
        ("torch-repeater", "cal-torch-repeater period 20 gt (1 s), on 10/20, edges 1, warmup 0, family redstone_torch+redstone_wire+repeater, 14 blocks, 3x2x3"),
        ("observer-piston", "cal-observer-piston period 8 gt (0.4 s), on 4/8, edges 2, warmup 3, family observer+redstone_wire+sticky_piston, 8 blocks, 3x4x1"),
        ("sculk-piston", "cal-sculk-piston period 49 gt (2.45 s), on 30/49, edges 1, warmup 5, family repeater+sculk_sensor+sticky_piston, 6 blocks, 5x2x2"),
        ("hopper-pair", "cal-hopper-pair period 14 gt (0.7 s), on 7/14, edges 1, warmup 7, family comparator+hopper, 4 blocks, 3x2x1"),
    ];
    let mut s = args("clock sim -q");
    s.extend(cal.iter().map(|(f, _)| format!("{CAL}/{f}.txt")));
    let n = cal.len();
    v.push(Check {
        name: "clock:sim-calibration".into(),
        what: "`clock sim` (exact redsim period) on 6 calibration clocks (exterior air outputs): their known period, warmup, output waveform, family, blocks and box",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(move |o| {
            for (f, want) in &cal {
                if !o[0].lines().any(|l| l.starts_with("CLOCK: ") && l.contains(want)) {
                    return Err(format!("{f}: expected `{want}`: {}", tail(&o[0])));
                }
            }
            Ok(format!("{n} calibration clocks: known periods"))
        }),
    });
    const NOKICK: &str = "designs/clocks/selftest/observer-pair-no-kick.txt";
    // the archive tables (clock::tables): a union merge keeps both versions of a row one branch
    // updated (2026-10-21: every clock's verdict row twice, the new `usable` column hidden); no
    // table may hold a key twice (`clock tables --fix` after a merge), and the fix's rules hold on a
    // planted fixture: identical rows, the most columns, the newest stamp over the last line
    v.push(Check {
        name: "archive:tables".into(),
        what: "`clock tables`: no archive table (clock verdicts, captions, extend, aliases, vibration, type keys; each circuit study's verdicts, captions, extend) holds a key twice or a row of a design (or family) that is gone (a union merge brings deleted rows back); `--fix --dry-run` on designs/clocks/selftest/tables-dup keeps identical rows once, the row with the most columns, the newest stamp over the last line, and drops the planted orphans",
        quick: true,
        jvm: false,
        steps: vec![args("clock tables"), args("clock tables --root designs/clocks/selftest/tables-dup --fix --dry-run")],
        timeout: 60,
        pred: Box::new(|o| {
            has(&o[0], " with duplicate keys")?;
            let bad: Vec<&str> = o[0].lines().filter(|l| l.starts_with("DUPLICATE ") || l.starts_with("ORPHAN ")).collect();
            if !bad.is_empty() {
                return Err(format!("archive tables need `mctas clock tables --fix`: {}", bad.iter().take(2).copied().collect::<Vec<_>>().join(" | ")));
            }
            has(&o[1], "fam 0: 2 rows -> identical rows")?;
            has(&o[1], "z.txt: 3 rows -> the newest stamp")?;
            has(&o[1], "q.txt: 2 rows -> the most columns")?;
            has(&o[1], "orphans (gone): gone-fam")?;
            has(&o[1], "orphans (gone): gone.txt")?;
            Ok("no duplicate keys, no orphan rows; the fix keeps the newest row and drops orphans on the fixture".into())
        }),
    });
    v.push(Check {
        name: "clock:item-despawn".into(),
        what: "`clock sim` says NO-REPEAT for a leaf clock fed by an item a player could drop: the item despawns at age 6000 and the judge runs item designs 12000 gt (a clock must run forever)",
        quick: true,
        jvm: false,
        steps: vec![args("clock sim -q designs/clocks/selftest/item-despawn.txt")],
        timeout: 120,
        pred: Box::new(|o| has(&o[0], "NO-REPEAT: ").map(|_| "NO-REPEAT (the item despawns)".into())),
    });
    v.push(Check {
        name: "clock:sim-negative".into(),
        what: "`clock sim` says STOPS for the observer pair placed silently (no start kick: nothing ever fires)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("clock sim -q {NOKICK}"))],
        timeout: 60,
        pred: Box::new(|o| has(&o[0], "STOPS: ").map(|_| "STOPS (not a clock)".into())),
    });
    v.push(Check {
        name: "clock:strip-slop".into(),
        what: "`clock strip --check` (the archive's irreducibility guard, vjeux 2026-10-22: no clock slop) refuses the basic observer clock with two stones that do nothing (both removed, the 2-block pair left) and passes the pair alone",
        quick: true,
        jvm: false,
        steps: vec![
            args("?clock strip --check --threads 1 designs/clocks/selftest/observer-pair-decor.txt"),
            args("clock strip --check --threads 1 designs/clocks/calibration/observer-pair.txt"),
        ],
        timeout: 120,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.starts_with("SLOP ")).ok_or(format!("the decorated pair is not slop: {}", tail(&o[0])))?;
            if !l.contains("4 blocks -> 2") {
                return Err(format!("expected 4 blocks -> 2: {l}"));
            }
            has(&o[1], "IRREDUCIBLE ").map(|_| "the decorated pair is slop (4 -> 2 blocks), the pair irreducible".into())
        }),
    });
    v.push(Check {
        name: "clock:archive-refuses-slop".into(),
        what: "`clock archive` refuses the basic observer clock with two stones that do nothing: the game confirms the clock without a stone (slop)",
        quick: false,
        jvm: true,
        steps: vec![args("?clock archive designs/clocks/selftest/observer-pair-decor.txt --dir {tmp} --workers {workers}")],
        timeout: 600,
        pred: Box::new(|o| has(&o[0], "not archived: slop").map(|_| "refused as slop".into())),
    });
    v.push(Check {
        name: "clock:archive-refuses-family-period".into(),
        what: "`clock archive` refuses the copper-bulb divider as it was archived: its family claims P = 12·2^n but the member's output gets the observer pair's pulses every 6 gt (the family check reads the period at the output, the coordinator 2026-10-23)",
        quick: false,
        jvm: true,
        steps: vec![args("?clock archive designs/clocks/selftest/divider-untapped.txt --dir {tmp} --workers {workers}")],
        timeout: 600,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.contains("not archived")).ok_or(format!("not refused: {}", tail(&o[0])))?;
            if !l.contains("period at its output 6 gt") {
                return Err(format!("refused for another reason: {l}"));
            }
            Ok("refused: its output pulses every 6 gt, the family claims 12".into())
        }),
    });
    v.push(Check {
        name: "clock:verify-calibration".into(),
        what: "`clock verify` (the real game, 2 positions x 4 rotations) on the observer pair and the subtract comparator: robust, P6 and P4, and redsim agrees",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("clock verify {CAL}/observer-pair.txt {CAL}/comparator-subtract.txt --positions 2 --horizon 1000 --workers {{workers}}"))],
        timeout: 600,
        pred: Box::new(|o| {
            for want in ["CLOCK: cal-observer-pair period 6 gt (0.3 s)", "CLOCK: cal-comparator-subtract period 4 gt (0.2 s)"] {
                let l = o[0].lines().find(|l| l.starts_with(want)).ok_or(format!("no `{want}`: {}", tail(&o[0])))?;
                if !l.ends_with("robust") {
                    return Err(format!("not robust: {l}"));
                }
            }
            if o[0].matches("redsim AGREES").count() != 2 {
                return Err(format!("redsim does not agree on both: {}", tail(&o[0])));
            }
            Ok("robust P6 + P4, redsim agrees".into())
        }),
    });
    v.push(Check {
        name: "clock:verify-negative".into(),
        what: "`clock verify` rejects the observer pair placed silently (no start kick)",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("?clock verify {NOKICK} --positions 1 --no-rotations --no-sim --workers 1"))],
        timeout: 600,
        pred: Box::new(|o| if o[0].lines().any(|l| l.starts_with("CLOCK:")) { Err("accepted a non-clock".into()) } else { Ok("rejected".into()) }),
    });
    v.push(Check {
        name: "clock:verify-reload".into(),
        what: "`clock verify --reload` (the chunks saved, unloaded and loaded again mid-run): the torch-repeater clock survives it; the observer pair does not in every rotation (it comes back at P4); redsim's reload verdict (`redsim::reload`) agrees with the game's runs on every build of both",
        quick: false,
        jvm: true,
        steps: vec![
            args(&format!("clock verify {CAL}/torch-repeater.txt --positions 1 --workers 4 --reload --no-sim")),
            args(&format!("?clock verify {CAL}/observer-pair.txt --positions 2 --horizon 1000 --workers {{workers}} --reload --no-sim")),
        ],
        timeout: 900,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.starts_with("CLOCK: cal-torch-repeater")).ok_or(format!("torch-repeater: {}", tail(&o[0])))?;
            if !l.ends_with("robust") {
                return Err(format!("torch-repeater not robust across a reload: {l}"));
            }
            if o[1].lines().any(|l| l.starts_with("CLOCK: cal-observer-pair") && l.ends_with("robust")) {
                return Err(format!("the observer pair passed the reload in every rotation: {}", tail(&o[1])));
            }
            // (tickexact) redsim's reload verdict, build by build, confirmed by the game
            for (name, out) in [("torch-repeater", &o[0]), ("observer pair", &o[1])] {
                if !out.lines().any(|l| l.trim_start().starts_with("reload: redsim AGREES")) {
                    return Err(format!("{name}: redsim's reload verdict does not agree with the game: {}", tail(out)));
                }
            }
            Ok("torch-repeater survives a reload, the observer pair does not; redsim agrees build by build".into())
        }),
    });
    v.push(Check {
        name: "clock:verify-entities".into(),
        what: "`clock verify` with design entities: the observer pair beside a minecart on a rail is a robust clock (the cart summoned, turned and still there at the end); with the cart in the air (it falls away) it is rejected",
        quick: false,
        jvm: true,
        steps: vec![
            args("?clock verify designs/clocks/selftest/observer-pair-cart.txt --positions 1 --no-sim --workers {workers}"),
            args("?clock verify designs/clocks/selftest/observer-pair-falling-cart.txt --positions 1 --no-rotations --no-sim --workers 1"),
        ],
        timeout: 600,
        pred: Box::new(|o| {
            if !o[0].lines().any(|l| l.starts_with("CLOCK: entity-test-decor period 6 gt") && l.ends_with("robust")) {
                return Err(format!("the cart design is not a robust P6 clock: {}", o[0].lines().next().unwrap_or("")));
            }
            if o[1].lines().any(|l| l.starts_with("CLOCK:")) || !o[1].contains("entities at the end") {
                return Err(format!("the falling cart was not rejected for its entity: {}", o[1].lines().next().unwrap_or("")));
            }
            Ok("cart kept: robust P6; cart lost: rejected".into())
        }),
    });
    // 9. the circuit judge (docs/circuits.md): known metrics on redsim and in the real game, a
    // false claim refused. R7 (real input devices, 2026-10-20): a plain input is the judge's stone
    // (or the design's own receiving block) powered by the judge's repeater from outside the
    // build, so no pulse or gap is shorter than 2 gt ("2 gt (the shortest a real input makes)"),
    // and piston and TNT events come 1 gt earlier (the input arrives during a tick, not between
    // ticks): sticky-extender 3 -> 2 gt, tnt-duper-triggered latency 1 -> 0 gt
    const CIRC: &str = "designs/circuits/calibration";
    let circ: Vec<(&str, &str)> = vec![
        ("subtract-rising-edge", "cal-subtract-rising-edge rising-edge L 4 gt, delay 2 gt, min-pulse 4 gt, 12 blocks, 2x2x3"),
        ("bulb-tff", "cal-bulb-tff tff latency 2 gt, edge rising, min-pulse 2 gt (the shortest a real input makes), min-period 4 gt (the shortest a real input makes), 3 blocks, 2x2x1"),
        ("repeater-delay", "cal-repeater-delay delay N 4 gt, min-pulse 4 gt, min-gap 4 gt, 2 blocks, 1x2x1"),
        ("torch-not", "cal-torch-not gate not function not, latency 2 gt, 4 blocks, 3x2x1"),
        ("dust-or", "cal-dust-or gate or function or, latency 0 gt, class instant, 2 blocks, 1x2x1"),
        ("repeater-dlatch", "cal-repeater-dlatch d-latch latency 4 gt, enable low, 4 blocks, 1x2x2"),
        // 2026-10-21 (clk-rt, site QA): the counter judge adds `counts`; this toggling design gives
        // one pulse per 4 inputs (a square wave), so its axis is no longer plain `counter 2`
        ("bulb-divider", "cal-bulb-divider counter 2 N 2, output toggles every 2 pulses, counts one pulse per 4 inputs (toggles every 2: a square wave, a divider by 4), 6 blocks, 4x2x1"),
        ("sticky-extender", "cal-sticky-extender piston-extender 1 N 1, extend 2 gt, retract 2 gt, flips safe (the lever flipped back at any time), 2 blocks, 3x1x1"),
        // 2026-10-21 (track fly, clk-misc's real-use audit): a horizontal flyer's ground clearance
        // (circuit::robust::fly_clearance): fly-d1 has slime in its bottom layer (it would drag
        // the grass), the two-way machine an observer watching straight down (grass under it turns
        // to dirt on a random tick): both need a block of air under them
        ("flying-machine", "cal-flying-machine flying-machine speed 10 gt/block, period 10 gt, 1 cell(s), latency 2 gt, ground needs 1 block of air above the ground, 7 blocks, 4x2x1"),
        ("two-way-machine", "cal-two-way-machine flying-machine two-way period 98 gt over 6 positions, latency 2 gt, ground needs 1 block of air above the ground, 14 blocks, 2x2x9"),
        // 2026-10-21 (track fly, real use): a one-way flyer started by a real lever, judged over
        // the audit's flip gaps (check::LEVER_FLIPS: off 4..20 gt after the launch, on again ...)
        ("fly-start", "cal-fly-start flying-machine speed 10 gt/block, period 10 gt, 1 cell(s), latency 2 gt, start flip the lever, ground flies along the ground, 7 blocks, 3x4x1"),
        // the elevator judge (track fly 2026-10-21): rule B, a rider, the press sweep
        ("fly-elevator", "cal-fly-elevator elevator 5 height 5 blocks, trip up 89 gt, down 94 gt, rider carries a rider, calls and sends from each floor, 165 blocks, 8x11x12"),
        ("repeater-line", "cal-repeater-line-n0 delay N 2 gt, min-pulse 2 gt (the shortest a real input makes), min-gap 2 gt (the shortest a real input makes), 2 blocks, 1x2x1"),
        ("dropper-tff-reset", "cal-dropper-tff-reset tff-reset latency 13 gt, reset 6 gt, min-pulse 2 gt (the shortest a real input makes), 12 blocks, 4x4x3"),
        ("observer-bud", "cal-observer-bud bud detects place remove state shape, latency 2 gt, pulse 2 gt, re-arm 4 gt, footprint 1 cell, 1 blocks, 1x1x1"),
        ("slime-bud", "cal-slime-bud bud detects place remove state update, latency 3 gt, pulse 1 gt, re-arm 6 gt, jams update pair 2 gt apart, footprint 5 cells, 3 blocks, 1x4x1"),
        ("latched-bud", "cal-latched-bud bud detects place remove state update, latency 3 gt, reset manual, off 1 gt after it, re-arm 2 gt, footprint 5 cells, 2 blocks, 3x1x1"),
        ("observer-stone-bud", "cal-observer-stone-bud bud detects place remove state shape, latency 2 gt, pulse 2 gt, re-arm 4 gt, footprint 1 cell, tileable every 1 (0 0 1), 2 blocks, 2x1x1"),
        ("sculk-filter-rise", "cal-sculk-filter-rise rising-edge L 10 gt, delay 1 gt, min-pulse 2 gt (the shortest a real input makes), vibration-proof yes, 5 blocks, 2x2x4"),
        ("bulb-counter2", "cal-bulb-counter2 binary-counter 2 latency 6 gt, edge rising, per bit 4.0 blocks, 8 blocks, 5x2x2"),
        ("bulb-counter-slice", "cal-bulb-counter-slice binary-counter 1 latency 2 gt, edge falling, per bit 5.0 blocks, slice 5 blocks per bit, latency 2 + 4 (N-1) gt, 5 blocks, 3x2x2"),
        ("hex-gt", "cal-hex-gt comparator 4 hex latency 4 gt (b goes 0 -> 8 from a=8 b=0), per bit 1.0 blocks, 4 blocks, 1x2x2"),
        ("tnt-duper", "cal-tnt-duper tnt-duper rate 200 TNT/min, period 6 gt, per period 1, latency 1 gt, 15 blocks, 3x3x5"),
        ("tnt-duper-triggered", "cal-tnt-duper-triggered tnt-duper-triggered per pulse 1, latency 0 gt, gap 20 gt, 13 blocks, 3x2x4"),
        ("bulb-burst2", "cal-bulb-burst2 burst 2 N 2, pulse 2 gt, period 6 gt, latency 12 gt, span 20 gt, min-pulse 2 gt (the shortest a real input makes), retrigger mixed, repeat 12 gt, 7 blocks, 5x2x1"),
        ("bulb-divider4", "cal-bulb-divider4 divider 4 N 4, duty 50%, output square, latency 4 gt, min-period 4 gt (the shortest a real input makes), 6 blocks, 4x2x1"),
        ("bulb-monostable", "cal-bulb-monostable monostable L 8 gt, delay 12 gt, retrigger non-retriggerable, re-arm 16 gt, min-pulse 2 gt (the shortest a real input makes), 6 blocks, 4x2x1"),
        ("tap-off-delay", "cal-tap-off-delay off-delay L 4 gt, latency 2 gt, min-pulse 2 gt (the shortest a real input makes), 26 blocks, 5x2x3"),
        ("subtract-on-delay", "cal-subtract-on-delay on-delay T 8 gt, shortening 4 gt, latency 6 gt, 16 blocks, 3x3x3"),
        ("torch-pulse-train", "cal-torch-pulse-train pulse-train P 12 gt, pulse 6 gt, latency 6 gt, stop 4 gt, 15 blocks, 2x3x6"),
        // R7: adjacent inputs driven from opposite sides, one through the design's own repeater
        ("d-latch-adjacent", "cal-d-latch-adjacent d-latch latency 6 gt, enable high, tileable every 2 (0 2 0), 7 blocks, 2x2x3"),
        // study 11's stimulus scaled with a long timer: inputs up to 3x the sizing gap, honest
        // min-pulse (an item timer reloads one item per 8 gt), 4 gt trains not judged past it
        ("item-timer-off-delay", "cal-item-timer-off-delay off-delay L 310 gt, latency 4 gt, min-pulse 320 gt, 8 blocks, 2x3x3"),
    ];
    // R7 refusals: an observer watching the input cell (a real input changes no block there), the
    // window flip-flops whose clock feeds a comparator's side straight from the input cell (a
    // diode's side reads only a signal source: dust, a diode, a redstone block, not the judge's
    // powered stone), and the
    // dust encoder whose d3 sits between its two output lines (every way in puts the judge's
    // switch against a line or an output cell)
    let r7: [&str; 5] = ["observer-dual-edge", "window-dff", "window-dff-enable", "window-shift1", "dust-encoder2"];
    let mut s = args("circuit sim");
    s.extend(circ.iter().map(|(f, _)| format!("{CIRC}/{f}.txt")));
    s.extend(r7.iter().map(|f| format!("{CIRC}/{f}.txt")));
    s.push("designs/circuits/selftest/dust-or-claimed-and.txt".into());
    s.push("designs/circuits/selftest/floating-not.txt".into());
    s.push("designs/circuits/selftest/shortening-delay-line.txt".into());
    s.push("designs/circuits/selftest/plus4-stretcher.txt".into());
    // a circuit must drive something (the judge's lamp in each air output cell): a redirection-only
    // output refused, a comparator AND kept with its metrics
    s.push("designs/circuits/selftest/update-less-nor.txt".into());
    // a design file left with merge markers is not a design: parsing its HEAD half silently once
    // put a broken file on main (clk-item 2026-10-20)
    s.push("designs/circuits/selftest/merge-markers.txt".into());
    s.push("designs/circuits/selftest/comparator-and.txt".into());
    s.push("designs/circuits/selftest/stone-output-nimply.txt".into());
    // the sequential memory kinds: a claim each refused (a reset that drives nothing, a level
    // latch as an edge-triggered register, an enable that drives nothing, one cell as two)
    for f in ["bulb-tff-claimed-reset", "dlatch-claimed-dff", "dff-claimed-enable", "dff-claimed-shift2"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // the BUD kind: a clock, a BUD that never re-arms, a false claim, a pulse BUD given a reset
    // input (not latched) and an observer BUD claiming to tile side by side (cross-talk) refused
    for f in ["observer-pair-bud", "latching-bud", "observer-bud-claims-update", "observer-bud-claims-latched", "observer-bud-tile1"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // the vibration test: a sculk design that hears its own lever, a false `vibration: proof`
    for f in ["sculk-hears-lever", "sculk-claims-proof"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // study 12: a plain encoder claiming to be a priority encoder refused (two inputs at once)
    s.push("designs/circuits/selftest/dust-encoder2-claims-priority.txt".into());
    // study 11: a monostable whose second edge cuts its pulse, an OR off-delay that does not
    // restart, a false retriggerable claim refused; a tiled off-delay kept (`tile 5 blocks x3`)
    // and its crosstalking 4-block tiling refused
    for f in ["oneshot-retriggered", "repeater-or-off-delay", "bulb-claims-retriggerable", "tile-off-delay-ring", "tile-off-delay-crosstalk"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // the TNT judge's usefulness rule: a duper whose TNT explodes in its own water refused
    s.push("designs/circuits/selftest/tnt-water-duper.txt".into());
    // the TNT judge's PRECISE check over kicks: an air burst precise for 99.5% of kick angles loses
    // the flag (a TNT in a sliver of angles stops short at its x-stop and goes off elsewhere)
    s.push("designs/circuits/selftest/tnt-airburst-sliver.txt".into());
    // the TNT judge's turned builds: a fast duper that primes nothing turned once is rotation-dependent
    s.push("designs/circuits/selftest/tnt-fast-rotation.txt".into());
    // the real-use audit's calibrations (2026-10-21)
    for f in ["tff-train-bulb", "counter-train-bulb", "delay-limiter-hold", "pext-flips", "buried-delay", "tnt-ring-intrusion"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // the start judge: a lever watched by an observer that stays on the parked part (a flip 4 gt
    // after the launch fires the machine again mid-cycle) refused
    s.push("designs/circuits/selftest/fly-start-watcher-stays.txt".into());
    // the docking judge: a machine that makes one trip per switch-on and parks with the switch on
    s.push("designs/circuits/selftest/fly-docking-parks.txt".into());
    // and one that a 16 gt reflip leaves docked with the switch on
    s.push("designs/circuits/selftest/fly-docking-reflip.txt".into());
    // the elevator judge: the generator's elevator without the top floor's send path (a press at
    // the top while it stands there must send it down)
    s.push("designs/circuits/selftest/elevator-no-top-send.txt".into());
    let circ2 = circ.clone();
    let n = circ.len();
    v.push(Check {
        name: "circuit:sim-calibration".into(),
        what: "`circuit sim` (redsim) on the 31 calibration circuits (edge detectors, T flip-flop, delay lines, NOT, OR, D latch, divider, piston extender, flying machine, a lever-started flying machine, two-way flying machine with return stations, T flip-flop with reset, an observer BUD and the slime-block QC BUD, a latched QC BUD, a tileable observer BUD, self-running and triggered TNT dupers, study 11's burst, /4 divider, monostable, off-delay, on-delay and gated pulse train, a vibration-proof sculk rising edge, study 12's 2-bit bulb ripple counter and its bit slice stacked to 4 bits, the 2-comparator hex a > b on chest strengths), inputs driven by real devices (R7): their known metrics; the OR gate claiming AND, a floating dust, a shortening delay line claiming pulse-limiter, a +4 gt stretcher claiming pulse-extender, an update-less (redirection-only) NOR, a stone output, four false memory claims, five false BUDs (a clock, a BUD that never re-arms, an observer claiming pure updates, an observer BUD with a reset input, an observer BUD claiming to tile side by side), three false study-11 timers (a one-shot cut by a second edge, an OR off-delay that does not restart, a false retriggerable claim), two sculk designs failing the vibration test (one hears its own lever, one falsely says vibration-proof) and a plain encoder claiming priority refused; five R7 refusals (an observer watching its input cell, three window flip-flops feeding a comparator's side from the input cell, study 12's dust encoder whose d3 input has no way in); a comparator AND kept (its b input through the design's own repeater, R7 model B); study 11's `tile:` judged copy by copy: an off-delay ring tiled 5 blocks apart kept (`tileable every 5`), 4 apart refused (copy 0's lamp reads copy 1's ring)",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(move |o| {
            for (f, want) in &circ {
                if !o[0].lines().any(|l| l.starts_with("CIRCUIT: ") && l.contains(want) && l.ends_with("redsim")) {
                    return Err(format!("{f}: expected `{want}`: {}", tail(&o[0])));
                }
            }
            has(&o[0], "FAIL: cal-dust-or-claimed-and gate and: claims and but behaves as or")?;
            has(&o[0], "FAIL: cal-floating-not gate not: redstone_wire")?;
            has(&o[0], "FAIL: cal-shortening-delay-line pulse-limiter: limiting not witnessed")?;
            has(&o[0], "REFUSED: designs/circuits/selftest/merge-markers.txt: parsing designs/circuits/selftest/merge-markers.txt: line 6: \"<<<<<<< HEAD\": unresolved merge markers, not a design")?;
            has(&o[0], "FAIL: cal-plus4-stretcher pulse-extender: never extends")?;
            // R7: the update-less NOR's dust turned toward a redstone block written into an input
            // cell; the judge's stone there is no source to turn toward, so it never switches
            has(&o[0], "FAIL: cal-update-less-nor gate nor: not a gate: the output is true (truth table 1111)")?;
            has(&o[0], "FAIL: cal-stone-output-nimply gate nimply: output out at [2, 1, 1] holds stone, which drives nothing by itself")?;
            has(&o[0], "CIRCUIT: cal-comparator-and gate and function and, latency 6 gt, glitches 2, 7 blocks, 3x2x2, redsim")?;
            has(&o[0], "FAIL: cal-observer-dual-edge dual-edge: the longest input pulse (40 gt) gives 0 output pulse(s)")?;
            has(&o[0], "FAIL: cal-window-dff dff: rising edge: M 1 clk rises: q = 0, want 1")?;
            has(&o[0], "FAIL: cal-window-dff-enable dff-enable: rising edge: M 1 clk rises: q = 0, want 1")?;
            has(&o[0], "FAIL: cal-window-shift1 shift 1: rising edge: M 1 clk rises: q = 0, want 1")?;
            has(&o[0], "FAIL: cal-bulb-tff-claimed-reset tff-reset: A r 4: an r pulse must clear q")?;
            has(&o[0], "FAIL: cal-dlatch-claimed-dff dff: rising edge: M 1 d=1 clk=0: q = 1, want 0")?;
            has(&o[0], "FAIL: cal-dff-claimed-enable dff-enable: rising edge:")?;
            has(&o[0], "FAIL: cal-dff-claimed-shift2 shift 2: rising edge:")?;
            has(&o[0], "FAIL: cal-observer-pair-bud bud: the output is on at rest")?;
            has(&o[0], "FAIL: cal-latching-bud bud: the output is on at rest")?;
            has(&o[0], "FAIL: cal-observer-bud-claims-update bud: claims update but does not detect it")?;
            has(&o[0], "FAIL: cal-observer-bud-claims-latched bud: not latched: after a place the output goes off")?;
            has(&o[0], "FAIL: cal-observer-bud-tile1 bud: not tileable (every 1 (0 0 1)): copy 0: cross-talk")?;
            has(&o[0], "FAIL: cal-sculk-hears-lever rising-edge: hears its input device")?;
            has(&o[0], "FAIL: cal-sculk-claims-proof falling-edge: not vibration-proof (it says `vibration: proof`)")?;
            has(&o[0], "FAIL: cal-dust-encoder2-claims-priority encoder 2 priority: input d3 at [2, 1, 2]: no side of the input cell leads out of the build")?;
            has(&o[0], "FAIL: cal-dust-encoder2 encoder 2: input d3 at [2, 1, 2]: no side of the input cell leads out of the build")?;
            has(&o[0], "FAIL: cal-oneshot-retriggered monostable: re-trigger behaviour changes with the delay")?;
            has(&o[0], "FAIL: cal-repeater-or-off-delay off-delay: min-pulse 8 gt")?;
            has(&o[0], "FAIL: cal-bulb-claims-retriggerable monostable retriggerable: claims retriggerable but behaves as non-retriggerable")?;
            has(&o[0], "CIRCUIT: cal-tile-off-delay-ring off-delay L 24 gt, latency 0 gt, min-pulse 4 gt, tileable every 5 (5 0 0), 13 blocks, 3x2x3, redsim")?;
            has(&o[0], "FAIL: cal-tile-off-delay-crosstalk off-delay: tile 4 0 0 (copies driven 7 gt apart): copy 0 of 3 measures L 33 gt")?;
            has(&o[0], "FAIL: cal-tnt-water-duper tnt-duper: useless TNT: a TNT explodes in a fluid")?;
            has(&o[0], "CIRCUIT: cal-tnt-airburst-sliver tnt-duper rate 22.2 TNT/min")?;
            if o[0].lines().any(|l| l.starts_with("CIRCUIT: cal-tnt-airburst-sliver ") && l.contains("precise:")) {
                return Err("cal-tnt-airburst-sliver: still PRECISE (the check over kicks, circuit::robust::precise_over_kicks, must drop the flag)".into());
            }
            if !o[0].lines().any(|l| l.starts_with("CIRCUIT: cal-tnt-fast-rotation ") && l.contains("rotation-dependent: turned 1 time(s): no primed TNT")) {
                return Err(format!("cal-tnt-fast-rotation: not flagged rotation-dependent (verify::sim checks a TNT duper's turned builds): {}", tail(&o[0])));
            }
            // the real-use audit (2026-10-21): a T flip-flop and a counter that a held input
            // keeps toggling (a gated clock in front of a copper bulb), a pulse limiter claiming
            // to be a delay line (a lever held down comes out 38 gt), a double extender whose
            // lever flipped back early strands its pistons (`flips breaks`)
            has(&o[0], "FAIL: cal-tff-train-bulb tff: a 200 gt pulse does not toggle it exactly once")?;
            has(&o[0], "FAIL: cal-counter-train-bulb counter 2: counts 4 gt pulses but not real presses")?;
            has(&o[0], "FAIL: cal-delay-limiter-hold delay: a lever held 200 gt does not come out shifted (+2 for 38)")?;
            has(&o[0], "CIRCUIT: cal-pext-flips piston-extender 2 N 2, extend 7 gt, retract 12 gt, flips breaks: off 6 gt after on strands blocks, 7 blocks, 5x2x2, redsim")?;
            // ... and a delay line whose output cell is walled in (no wire can leave it)
            has(&o[0], "FAIL: cal-buried-delay delay: output out at [2, 1, 0] is buried")?;
            // ... and the TNT duper whose release ring is open on top (vjeux 15:24Z): a dropped
            // item joins the ring's stone (crate circuit::intrusion)
            has(&o[0], "FAIL: cal-tnt-ring-intrusion tnt-duper: item intrusion: 1 cobblestone, 64 iron ingots, 1 iron shovel dropped on the open hopper top(s) at [4, -2, -1]")?;
            has(&o[0], "FAIL: cal-fly-start-watcher-stays flying-machine: no steady flight after the launch")?;
            has(&o[0], "FAIL: cal-fly-docking-parks flying-machine: run 1: no round trip repeats while the switch is on")?;
            has(&o[0], "FAIL: cal-fly-docking-reflip flying-machine: run with a 16 gt reflip: stops shuttling before the switch turns off")?;
            has(&o[0], "FAIL: cal-elevator-no-top-send elevator 5: send down [frames 600..1080]: rests 5 blocks up, want the bottom floor")?;
            Ok(format!("{n} calibration circuits: known metrics; a false claim, a floating dust, a delay line, a stretcher, an update-less NOR, a stone output, 4 false memory claims, 5 false BUDs, 3 false study-11 timers, 2 sculk designs failing the vibration test, a false priority encoder and 5 R7 input refusals; the comparator AND kept; a tiled off-delay kept, its crosstalking tiling refused; a TNT duper exploding in water refused; an air burst imprecise in slivers of kick angles not flagged precise; a fast duper that primes nothing turned flagged rotation-dependent; real use: a T flip-flop and a counter a held input keeps toggling, a limiter claiming delay refused, an extender whose early lever flip strands blocks flagged, a buried output and an open hopper ring refused; a flyer whose watcher stays on the parked part, a docking machine that parks with the switch on and one a 16 gt reflip leaves docked refused; an elevator that cannot be sent down from the top refused"))
        }),
    });
    // the real game's verify of the calibration circuits, split in 5 checks run side by side
    // (mobinfra 2026-11-08, the coordinator's OK: one 39-design check took 595 s alone against its
    // 600 s deadline and timed out under any load; the same designs, every assertion word for
    // word, each part with its own 600 s deadline), each on its own pool (`{workers}`: 8, capped
    // by MCTAS_MAX_WORKERS and the machine, testplan.rs)
    let extras: Vec<(&str, &str)> = vec![
        ("update-less-nor", "FAIL: cal-update-less-nor gate nor: not a gate: the output is true (truth table 1111)"),
        ("comparator-and", "CIRCUIT: cal-comparator-and gate and function and, latency 6 gt, glitches 2, 7 blocks, 3x2x2, robust"),
        ("dlatch-claimed-dff", "FAIL: cal-dlatch-claimed-dff dff: rising edge: M 1 d=1 clk=0: q = 1, want 0"),
        // a copper-bulb toggle given a reset input is no latch (redsim does not model bulbs)
        ("bulb-toggle-bud", "FAIL: cal-bulb-toggle-bud bud: latches"),
        // study 11's `tile:`: three copies 5 apart, each judged against its own input
        ("tile-off-delay-ring", "CIRCUIT: cal-tile-off-delay-ring off-delay L 24 gt, latency 0 gt, min-pulse 4 gt, tileable every 5 (5 0 0), 13 blocks, 3x2x3, robust"),
        // the TNT judge's usefulness rule in the game: TNT exploding in the duper's own water
        ("tnt-water-duper", "FAIL: cal-tnt-water-duper tnt-duper: useless TNT: a TNT explodes in a fluid"),
        // the start judge in the game: the watcher left on the parked part refused
        ("fly-start-watcher-stays", "FAIL: cal-fly-start-watcher-stays flying-machine: no steady flight after the launch"),
    ];
    const PARTS: usize = 5;
    for part in 0..PARTS {
        // round robin, so the slow designs (the flyers, the elevator, the long timers) spread
        let mine: Vec<(&str, &str)> = circ2.iter().enumerate().filter(|(i, _)| i % PARTS == part).map(|(_, c)| *c).collect();
        let my_extras: Vec<(&str, &str)> = extras.iter().enumerate().filter(|(i, _)| i % PARTS == part).map(|(_, e)| *e).collect();
        let names: Vec<&str> = mine.iter().map(|(f, _)| *f).chain(my_extras.iter().map(|(f, _)| *f)).collect();
        let what: &'static str = Box::leak(
            format!(
                "`circuit verify` (the real game, 2 positions x 4 rotations, one pool of 8 workers) on calibration circuits, part {} of {PARTS}: {}; inputs driven by real devices (R7) (BUDs included: the judge's shape-only and pure-update actions in the game; TNT dupers included: every primed TNT recorded then removed; study 11's timers on their long input trains; the sculk rising edge with its lever's and ambient game events; study 12's encoder, counter, bit slice and hex comparator): robust, the same metrics as redsim; the update-less NOR, the latch claiming dff, a bulb toggle claiming a latched BUD, a TNT duper exploding in its water and a start lever whose watcher stays on the parked part refused, the comparator AND robust; study 11's tiled off-delay (3 copies 5 apart, each against its own input) robust",
                part + 1,
                names.join(", ")
            )
            .into_boxed_str(),
        );
        v.push(Check {
            name: format!("circuit:verify-calibration-{}", part + 1),
            what,
            quick: false,
            jvm: true,
            steps: vec![{
                let mut s = args("?circuit verify --positions 2 --workers {workers}");
                s.extend(mine.iter().map(|(f, _)| format!("{CIRC}/{f}.txt")));
                s.extend(my_extras.iter().map(|(f, _)| format!("designs/circuits/selftest/{f}.txt")));
                s
            }],
            timeout: 600,
            pred: Box::new(move |o| {
                for (f, want) in &mine {
                    if !o[0].lines().any(|l| l.starts_with("CIRCUIT: ") && l.contains(want) && l.ends_with("robust")) {
                        return Err(format!("{f}: expected `{want}`, robust: {}", tail(&o[0])));
                    }
                }
                for (_, line) in &my_extras {
                    has(&o[0], line)?;
                }
                Ok(format!("part {} of {PARTS}: {} calibration circuits robust, redsim's metrics; {} more as expected ({})", part + 1, mine.len(), my_extras.len(), my_extras.iter().map(|(f, _)| *f).collect::<Vec<_>>().join(", ")))
            }),
        });
    }
    // 9b. study 19: minecart stations, judged in the game by their own runner (real carts, ridden
    // and empty, 0.02 to 2.0 b/t; presses with the station empty, mid-arrival and after a random
    // hold): the one-way record's metrics, and the same station with its button hidden behind a
    // support refused (the rider clicks it from the cart: a look ray must reach it)
    v.push(Check {
        name: "circuit:verify-station".into(),
        what: "`circuit verify` (the real game, 1 position x 4 rotations) on the one-way minecart station record: robust, stop distance 1.60 blocks, departure delay 1 gt; the same station with its button on a support's side face refused (no look ray from a cart resting on the slopes reaches it); `circuit sim` (the redsim twin, riders and all: clk-misc 2026-10-21) agrees on both",
        quick: false,
        jvm: true,
        steps: vec![
            args("?circuit verify --positions 1 --workers {workers} designs/circuits/rail/archive/station-slope-v2.txt designs/circuits/selftest/station-hidden-button.txt"),
            args("circuit sim designs/circuits/rail/archive/station-slope-v2.txt designs/circuits/selftest/station-hidden-button.txt"),
        ],
        timeout: 900,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: station-slope-v2 station 1 line one-way, stop distance 1.60 blocks, departure delay 1 gt, leaves at 0.22 b/t, 6 blocks, 3x3x2, robust")?;
            has(&o[0], "FAIL: cal-station-hidden-button station 1: car 1 (ridden, 2.00 b/t): came to rest at (1.60, 1.46, 0.50), where the rider cannot click the button")?;
            has(&o[1], "CIRCUIT: station-slope-v2 station 1 line one-way, stop distance 1.60 blocks, departure delay 1 gt, leaves at 0.22 b/t, 6 blocks, 3x3x2, redsim (robust)")?;
            has(&o[1], "FAIL: cal-station-hidden-button station 1: car 1 (ridden, 2.00 b/t): came to rest at (1.60, 1.46, 0.50), where the rider cannot click the button")?;
            Ok("the one-way station robust with its metrics; a hidden button refused; the redsim twin agrees".into())
        }),
    });
    // 9c. study 19's redsim twin (clk-misc 2026-10-21): the rail judges' runs on redsim (riders,
    // hopper-minecart transfers, detector rails reading carts, strays), their verdicts and metrics
    // the game's (`station-run` / `cart-run --compare-verify`: the same in every build; the runs
    // themselves tick-identical, the trace suite's station and cart traces)
    v.push(Check {
        name: "circuit:rail-twin".into(),
        what: "`circuit sim` (the rail judges' redsim twins, every build) on the station and cart records: the archived verdicts and metrics; refused as in the game: a hidden button, an unloader a crawling cart never reaches, a loader whose full cart stalls, a loader that sends an empty cart away from an empty supply chest (coordinator 2026-10-22: an empty cart at a dry supply waits; this replaced cart-loader-t5, which did that and left the archive)",
        quick: true,
        jvm: false,
        steps: vec![args("circuit sim designs/circuits/rail/archive/station-slope-v2.txt designs/circuits/selftest/station-hidden-button.txt designs/circuits/rail/archive/cart-unloader-t3.txt designs/circuits/rail/archive/ld-chest.txt designs/circuits/rail/archive/ul-gate.txt designs/circuits/selftest/cart-unloader-crawl.txt designs/circuits/selftest/cart-loader-stall.txt designs/circuits/tracks/cart-transport/calibration/ld-chest-shuttles.txt")],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: station-slope-v2 station 1 line one-way, stop distance 1.60 blocks, departure delay 1 gt, leaves at 0.22 b/t, 6 blocks, 3x3x2, redsim (robust)")?;
            has(&o[0], "FAIL: cal-station-hidden-button station 1: car 1 (ridden, 2.00 b/t): came to rest at (1.60, 1.46, 0.50), where the rider cannot click the button")?;
            has(&o[0], "CIRCUIT: cart-unloader-t3 cart-unloader line terminus, unload rate 2.48 items/s, departure delay 15 gt, 22 blocks, 5x3x3, redsim (robust)")?;
            has(&o[0], "CIRCUIT: ld-chest cart-loader line terminus, load rate 16.87 items/s, departure delay 17 gt, 44 blocks, 9x4x6, redsim (robust)")?;
            has(&o[0], "CIRCUIT: ul-gate cart-unloader line terminus, unload rate 4.79 items/s, departure delay 14 gt, 31 blocks, 6x3x4, redsim (robust)")?;
            has(&o[0], "FAIL: cal-cart-unloader-crawl cart-unloader: cart 2 (one item, 0.02 b/t): still moving 300 gt after it was sent in")?;
            has(&o[0], "FAIL: cal-cart-loader-stall cart-loader: cart 7 (6 non-stackables: full, 1 left, 0.25 b/t): full with 5 items 49 gt after it came in, then: rests at (0.65, 0.72, 0.50) and has not moved 200 gt later")?;
            has(&o[0], "FAIL: ld-chest-shuttles cart-loader: cart 10 (no supply: waits, then 10 glass, 0.05 b/t): went on without stopping, empty, from a dry supply (an empty cart must wait for items)")?;
            Ok("the station and cart records with their metrics on redsim; the hidden button, the crawl, the stall and the shuttling loader refused".into())
        }),
    });
    // 9c. study 21: automatic doors, judged in the game by their own runner (armor-stand walkers
    // teleported tick by tick, both ways, walking and sprinting, two in a row, the two-walker
    // sweeps; last, two reported stops of 400 gt in the doorway): the record's metrics, a
    // plate-order door the sweeps refuse, and a door with no memory (any plate opens it) passed
    // with "stop in doorway: the door closes on them" (designs/briefs/auto-door.md, 2026-10-22:
    // stopping in the doorway is reported, not required: the door plane has a blind spot no plate
    // covers, and the squeeze costs a player half a heart where memory costs twice the blocks)
    v.push(Check {
        name: "circuit:verify-autodoor".into(),
        what: "`circuit verify` (the real game, 1 position x 4 rotations) on automatic doors: lock-v8 robust through the two-walker sweeps (open wait 6 gt, close delay 40 gt, the door stays open for someone standing in the doorway); a plate-order door with its rows 2 apart (selftest/autodoor-plate-order, clk-obs lock-v5) refused by them (two sprinting 2.75 apart: the door stays open with nobody inside); a door with no memory (plates only) robust, its reported stop saying the door closes on them; `circuit sim` (the judge's redsim twin, tickexact) gives lock-v8 the same card",
        quick: false,
        jvm: true,
        steps: vec![
            args("?circuit verify --positions 1 --workers {workers} designs/circuits/autodoor/archive/auto-door-lock-v8.txt designs/circuits/selftest/autodoor-plate-order.txt designs/circuits/selftest/autodoor-no-memory.txt"),
            args("circuit sim designs/circuits/autodoor/archive/auto-door-lock-v8.txt"),
        ],
        timeout: 900,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: auto-door-lock-v8 auto-door door 2x2, open wait 6 gt, close delay 40 gt, stop in doorway the door stays open, 99 blocks, 7x8x11, robust")?;
            has(&o[0], "FAIL: cal-autodoor-plate-order auto-door: walk 89 (two sprinting 2.75 apart): the door was not closed 300 gt after the last walker left (it stays open with nobody inside)")?;
            has(&o[0], "CIRCUIT: cal-autodoor-no-memory auto-door door 2x2, open wait 4 gt, close delay 40 gt, stop in doorway the door closes on them, 60 blocks, 8x5x5, robust")?;
            has(&o[1], "CIRCUIT: auto-door-lock-v8 auto-door door 2x2, open wait 6 gt, close delay 40 gt, stop in doorway the door stays open, 99 blocks, 7x8x11, redsim (robust)")?;
            Ok("lock-v8 robust through the two-walker sweeps and safe for a stop; the plate-order door lock-v5 refused by them (open with nobody inside); a door with no memory robust, its stop reported as closing on them; redsim's twin gives lock-v8 the same card".into())
        }),
    });
    // the autodoor judge's redsim twin (tickexact 2026-11-08, testgate's gap b): every build's
    // walks in redsim (the armor-stand walkers summoned, teleported and killed as the game's
    // commands do), on the three calibration designs; `circuit door-run --compare-verify` found
    // them the same as the game's on all 16 builds of each (every verdict, metric and walk line)
    v.push(Check {
        name: "sim:autodoor-twin".into(),
        what: "`circuit sim` on the three automatic-door calibration designs (the judge's redsim twin, 4 positions x 4 rotations, the walkers in redsim): lock-v8 robust (open wait 6 gt, close delay 40 gt, the door stays open for a stop), the plate-order door refused at walk 89 (open with nobody inside), the door with no memory robust (it closes on a stop): the game's verdicts",
        quick: false,
        jvm: false,
        steps: vec![
            args("circuit sim designs/circuits/autodoor/archive/auto-door-lock-v8.txt"),
            args("circuit sim designs/circuits/selftest/autodoor-plate-order.txt"),
            args("circuit sim designs/circuits/selftest/autodoor-no-memory.txt"),
        ],
        timeout: 600,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: auto-door-lock-v8 auto-door door 2x2, open wait 6 gt, close delay 40 gt, stop in doorway the door stays open, 99 blocks, 7x8x11, redsim (robust)")?;
            has(&o[1], "FAIL: cal-autodoor-plate-order auto-door: walk 89 (two sprinting 2.75 apart): the door was not closed 300 gt after the last walker left (it stays open with nobody inside)")?;
            has(&o[2], "CIRCUIT: cal-autodoor-no-memory auto-door door 2x2, open wait 4 gt, close delay 40 gt, stop in doorway the door closes on them, 60 blocks, 8x5x5, redsim (robust)")?;
            Ok("redsim's walkers give the three calibration doors the game's verdicts".into())
        }),
    });
    // 10. study 8: shulker loaders and unloaders, judged in the game by their own runner (judge
    // chests, every item and box counted): track shulker's always-on loader and unloader with
    // unstackables (27 items per box: a fast run) and their badges, and a loader that breaks its
    // boxes early refused. One piece per design (each 1 position x 4 rotations: ~3 min; the three
    // in one command took 9-10 min)
    let shulker = |name: &str, what: &'static str, design: &str, sim: bool, pred: Pred| Check {
        name: format!("circuit:verify-shulker-{name}"),
        what,
        quick: false,
        jvm: true,
        steps: std::iter::once(args(&format!("?circuit verify --positions 1 --workers {{workers}} {design}"))).chain(sim.then(|| args(&format!("circuit sim {design}")))).collect(),
        timeout: 900,
        pred,
    };
    // the drop's random offset can move a median time by a gt: match around the times
    fn circuit_line(o: &str, name: &str) -> String {
        o.lines().find(|l| l.starts_with(&format!("CIRCUIT: {name} "))).unwrap_or("").to_string()
    }
    v.push(shulker(
        "loader",
        "`circuit verify` (the real game, 1 position x 4 rotations) on track shulker's always-on loader with unstackables: 3 boxes, robust, hopper pace (cycle 224 gt), every badge it claims (always-on, colours, restart, top-up, box-starved, drop-free); `circuit sim` says redsim does not model it yet",
        "designs/circuits/shulker/tracks/shulker/auto-loader-1.txt",
        true,
        Box::new(|o| {
            let l = circuit_line(&o[0], "shulker-auto-loader-1");
            if !(l.contains("shulker-loader 1 boxes/min ") && l.contains("items/s 2.") && l.contains("cycle 22") && l.contains("hoppers awake 3, badges always-on+colours+restart+top-up+box-starved+drop-free, bill 16 iron, 4 quartz, 28 redstone, 1 slime, 31 blocks, 5x5x5, robust")) {
                return Err(format!("auto loader: {l:?}: {}", tail(&o[0])));
            }
            has(&o[1], "UNMODELED: shulker-auto-loader-1: redsim does not judge shulker loaders")?;
            Ok("the always-on loader robust at hopper pace with all its badges".into())
        }),
    ));
    v.push(shulker(
        "unloader",
        "`circuit verify` (the real game, 1 position x 4 rotations) on track shulker's always-on unloader with unstackables: 3 boxes, robust, hopper pace (cycle 220 gt), every badge it claims (always-on, colours, box-starved, drop-free)",
        "designs/circuits/shulker/tracks/shulker/auto-unloader-1.txt",
        false,
        Box::new(|o| {
            let l = circuit_line(&o[0], "shulker-auto-unloader-1");
            if !(l.contains("shulker-unloader 1 boxes/min ") && l.contains("items/s 2.4") && l.contains("cycle 22") && l.contains("badges always-on+colours+box-starved+drop-free, bill 16 iron, 5 quartz, 35 redstone, 1 slime, 49 blocks, 5x6x8, robust")) {
                return Err(format!("auto unloader: {l:?}: {}", tail(&o[0])));
            }
            Ok("the always-on unloader robust at hopper pace with all its badges".into())
        }),
    ));
    v.push(shulker(
        "early",
        "`circuit verify` (the real game, 1 position x 4 rotations): a loader that breaks its boxes at the first item refused",
        "designs/circuits/selftest/shulker-early-loader.txt",
        false,
        Box::new(|o| {
            has(&o[0], "FAIL: cal-shulker-early-loader shulker-loader 1: box 1 arrived at t=")?;
            Ok("the early breaker refused".into())
        }),
    ));
    // the TNT judge's on/off LEVER check (circuit::tnt::check_lever, vjeux 2026-10-21: "Can we have
    // an on off switch for all the tnt"): the clockless engine with its lever passes (off as built,
    // 4 on/off cycles at other moments of the period, back to rest each time); the same engine
    // without its inverter torch runs with the lever off and is refused. Full selftest only: the
    // 200 TNT/min engine's explosions take ~40 s of redsim each
    v.push(Check {
        name: "circuit:tnt-lever".into(),
        what: "`circuit sim` (redsim) on the TNT lever check: tnt-engine-lever-17b passes (200 TNT/min, stops within 2 gt, restarts within 5 gt), the engine without its inverter torch is refused (`lever off at rest: N TNT primed`)",
        quick: false,
        jvm: false,
        steps: vec![args("circuit sim designs/circuits/tracks/clk-misc/tnt-engine-lever-17b.txt designs/circuits/selftest/tnt-lever-nostop.txt")],
        timeout: 300,
        pred: Box::new(|o| {
            if !o[0].lines().any(|l| l.starts_with("CIRCUIT: tnt-engine-lever-17b tnt-duper rate 200 TNT/min, period 6 gt, per period 1, runs lever on, switch lever off: stops within 2 gt; on: restarts within 5 gt")) {
                return Err(format!("tnt-engine-lever-17b: not the known lever verdict: {}", tail(&o[0])));
            }
            has(&o[0], "FAIL: cal-tnt-lever-nostop tnt-duper: lever off at rest:")?;
            Ok("the lever engine kept (stops within 2 gt, restarts within 5 gt); the engine without its inverter refused at rest".into())
        }),
    });
    // the sorter's exact gate (the pick sweep, 2026-10-21): the judge's base run of the published
    // sorter in the game and on redsim, every cell with container contents compared every gt (it
    // diverged at frame 0 until the game side's container text kept named items: the filter
    // papers' `components` broke the item split); and its water-stream run ending right on the
    // first slice (track sorter 2026-10-21: the stream's item entities, the hopper taking whole
    // stacks)
    v.push(Check {
        name: "circuit:sorter-compare".into(),
        what: "`circuit sorter-run --compare` on the published sorter (sorter-classic): the judge's base run and its water stream ending on the first slice, in the game and on redsim, every cell of the watched box (block state, container contents, comparator outputs) identical every gt",
        quick: false,
        jvm: true,
        steps: vec![
            args("circuit sorter-run --compare designs/circuits/sorter/archive/sorter-classic.txt"),
            args("circuit sorter-run --compare --run direct designs/circuits/sorter/archive/sorter-classic.txt"),
            args("circuit sorter-run --compare --run jam designs/circuits/sorter/archive/storage-bulk.txt"),
            args("circuit sorter-run --compare designs/circuits/tracks/sorter/calibration/double-chest.txt"),
        ],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[0], "IDENTICAL: sorter-classic, 4121 frames")?;
            has(&o[1], "IDENTICAL: sorter-classic, 4341 frames")?;
            has(&o[2], "IDENTICAL: storage-bulk, 6441 frames")?;
            has(&o[3], "IDENTICAL: dc-test, 9141 frames")?;
            Ok("sorter-classic's base run (4121 frames) and direct water-stream run (4341 frames), storage-bulk's jam run (6441 frames: its double chest full, then emptied) and the double-chest test (9141 frames: the right half full, the drain fills the left half through the compound container, a comparator on the left half reads both) tick-identical in redsim".into())
        }),
    });
    // study 13's real-use judge (track sorter, vjeux 2026-10-21 "REAL USE IS THE BAR"; crate
    // circuit::sorter): one slice from an input chest, slices side by side fed by a hopper line and
    // by a water stream, the middle slice's chest full; a calibration design for each check that
    // must fail it (designs/circuits/tracks/sorter/calibration/)
    const SORTER_CAL: &str = "designs/circuits/tracks/sorter/calibration";
    let sorter_cal: Vec<String> = ["no-tile", "side-filter", "filter-gap", "leaky-18", "cobble-filler", "jam-lock", "b-empty", "recipe-18"].iter().map(|f| format!("{SORTER_CAL}/{f}.txt")).collect();
    let mut s = args("circuit sorter-run --sim --run all designs/circuits/sorter/archive/sorter-classic.txt designs/circuits/tracks/sorter/sorter-classic-16.txt designs/circuits/tracks/sorter/storage-classic.txt designs/circuits/tracks/sorter/calibration/storage-jam-lock.txt designs/circuits/sorter/archive/storage-bulk.txt");
    s.extend(sorter_cal.iter().cloned());
    v.push(Check {
        name: "circuit:sorter-judge".into(),
        what: "`circuit sorter-run --sim --run all` (redsim, the whole sorter judge): sorter-classic and its 16-stack recipe pass every required run (one slice from an input chest; slices side by side fed by a hopper line and by a water stream; the middle slice's chest full; stray items dropped on every open hopper top), badge overflow-proof, not stream-direct (a water stream ending on its first slice: 69 iron go past it); refused: no `tile:` header, a filter hopper pointing sideways (side by side it pushes its fillers onto the next filter's), a filter missing a filler (junk takes the gap), the 18 + 4 recipe with a 2-step threshold (a full chest unlocks its neighbour, which drains its recipe and sorts junk), plain cobblestone fillers (a stray cobblestone corrupts the filters), an empty drain hopper, a recipe the threshold does not match; kept without overflow-proof: a chest comparator that locks the line (a full chest stops it)",
        quick: true,
        jvm: false,
        steps: vec![s, args("circuit sorter-run --sim --run direct designs/circuits/sorter/archive/sorter-classic.txt")],
        timeout: 120,
        pred: Box::new(|o| {
            has(&o[0], "SORTER: sorter-classic sorter iron_ingot items/s 2.50, first item by 20 gt, hoppers awake 2, slice width 1, badges overflow-proof, 15 blocks, 1x5x5, redsim")?;
            has(&o[0], "SORTER: sorter-classic-16 sorter snowball items/s 2.50, first item by 20 gt, hoppers awake 2, slice width 1, badges overflow-proof, 15 blocks, 1x5x5, redsim")?;
            has(&o[0], "FAIL: cal-no-tile sorter iron_ingot: a sorter is a row of slices side by side")?;
            has(&o[0], "FAIL: cal-side-filter sorter iron_ingot: slices side by side fed by a hopper line: cobblestone x6 reached copy 1's chest")?;
            has(&o[0], "FAIL: cal-filter-gap sorter iron_ingot: cobblestone x70 reached copy 0's chest")?;
            has(&o[0], "FAIL: cal-leaky-18 sorter iron_ingot: slices side by side with the middle slice's chest full: cobblestone x64 reached copy 0's chest")?;
            has(&o[0], "FAIL: cal-cobble-filler sorter iron_ingot: stray items dropped on every open hopper top of the row:")?;
            has(&o[0], "SORTER: cal-jam-lock sorter iron_ingot items/s 2.50, first item by 20 gt, hoppers awake 3, slice width 2, badges none, 25 blocks, 2x5x7, redsim")?;
            has(&o[0], "FAIL: cal-b-empty sorter iron_ingot: 297 of 508 items delivered")?;
            has(&o[0], "FAIL: cal-recipe-18 sorter iron_ingot: 485 of 508 items delivered")?;
            has(&o[1], "FAIL: sorter-classic (direct run): iron_ingot x69 went past copy 0 to the end of the line")?;
            has(&o[0], "SORTER: storage-classic sorter iron_ingot items/s 2.50, first item by 20 gt, hoppers awake 2, slice width 1, storage row of 8 slices from an input chest, overflow chest, overflow never blocks, badges overflow-proof, 15 blocks, 1x5x5, redsim")?;
            has(&o[0], "FAIL: cal-storage-jam-lock sorter iron_ingot: slices side by side with the middle slice's chest full: a storage row's line must keep moving while a chest is full (the overflow chest takes the surplus): overflow-proof no (the line stops while copy 1's chest is full")?;
            has(&o[0], "SORTER: storage-bulk sorter iron_ingot items/s 2.50, first item by 20 gt, hoppers awake 2, slice width 1, storage bulk (a double chest a slice, 54 stacks), row of 8 slices from an input chest, overflow chest, overflow never blocks, badges overflow-proof, 15 blocks, 1x5x5, redsim")?;
            Ok("sorter-classic and its 16-stack recipe pass (overflow-proof, strays safe; a stream right on its first slice leaks 69 iron); no tile, a sideways filter, a filter gap, a leaky threshold, cobblestone fillers, an empty drain and a wrong recipe refused; the line-locking design kept without overflow-proof; as storage rows (8 slices from an input chest, overflow chest) the classic and its double-chest (bulk) form pass and the line-locking design is refused".into())
        }),
    });
    let mut s = args("?circuit verify --workers {workers} designs/circuits/sorter/archive/sorter-classic.txt");
    s.extend(sorter_cal.iter().take(6).cloned());
    v.push(Check {
        name: "circuit:sorter-verify".into(),
        what: "`circuit verify` (the real game) on the sorter calibration: sorter-classic robust with badge overflow-proof (not stream-direct); refused as on redsim: no `tile:` header, the sideways filter (slices side by side), the filter gap (junk in its chest), the leaky 18 + 4 recipe (a full chest drains its neighbour), the cobblestone fillers (strays); the line-locking design robust without overflow-proof",
        quick: false,
        jvm: true,
        steps: vec![s],
        timeout: 900,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: sorter-classic sorter iron_ingot items/s 2.50, first item by 20 gt, hoppers awake 2, slice width 1, badges overflow-proof, bill 15 iron, 1 quartz, 10 redstone, 15 blocks, 1x5x5, robust")?;
            has(&o[0], "FAIL: cal-no-tile sorter iron_ingot: a sorter is a row of slices side by side")?;
            has(&o[0], "FAIL: cal-side-filter sorter iron_ingot: slices side by side fed by a hopper line: rotation 0: cobblestone x6 reached copy 1's chest")?;
            has(&o[0], "FAIL: cal-filter-gap sorter iron_ingot: cobblestone x70 reached copy 0's chest")?;
            has(&o[0], "FAIL: cal-leaky-18 sorter iron_ingot: slices side by side with the middle slice's chest full: rotation 0: cobblestone x64 reached copy 0's chest")?;
            has(&o[0], "FAIL: cal-cobble-filler sorter iron_ingot: stray items dropped on every open hopper top of the row: rotation 0:")?;
            has(&o[0], "CIRCUIT: cal-jam-lock sorter iron_ingot items/s 2.50, first item by 20 gt, hoppers awake 3, slice width 2, badges none, bill 20 iron, 2 quartz, 27 redstone, 25 blocks, 2x5x7, robust")?;
            Ok("sorter-classic robust, overflow-proof; no tile, the sideways filter, the filter gap, the leaky threshold and the cobblestone fillers refused in the game; the line-locking design robust, not overflow-proof".into())
        }),
    });
    // study 17, auto-smelters (track sorter, coordinator 2026-10-21; crate circuit::smelter): the
    // furnace in redsim against the game, every cell every gt (furnace timers and lit state
    // included), on the one-furnace record's base run, the naive slice's tiled run and the
    // two-furnace record's base run (its item clock locking the hoppers around the furnaces)
    v.push(Check {
        name: "circuit:smelter-compare".into(),
        what: "`circuit smelter-run --compare` (the furnace model): smelter-1's base run, the naive slice's tiled run (3 copies) and smelter-clock-2's base run, in the game and on redsim, every cell of the watched box (block state with the lit furnace, container contents, furnace cook and burn timers) identical every gt",
        quick: false,
        jvm: true,
        steps: vec![
            args("circuit smelter-run --compare designs/circuits/smelter/archive/smelter-1.txt"),
            args("circuit smelter-run --compare --run tiled designs/circuits/tracks/smelter/calibration/naive-slice.txt"),
            args("circuit smelter-run --compare designs/circuits/smelter/archive/smelter-clock-2.txt"),
            args("circuit smelter-run --compare designs/circuits/smelter/archive/smelter-row.txt"),
        ],
        timeout: 400,
        pred: Box::new(|o| {
            has(&o[0], "IDENTICAL: smelter-1, 3661 frames")?;
            has(&o[1], "IDENTICAL: cal-smelter-naive-slice, 17241 frames")?;
            has(&o[2], "IDENTICAL: smelter-clock-2, 4061 frames")?;
            has(&o[3], "IDENTICAL: smelter-row, 4421 frames")?;
            Ok("smelter-1's base run (3661 frames), the naive slice's tiled run (17241 frames, 4 copies), smelter-clock-2's base run (4061 frames) and smelter-row's base run (4421 frames: three latch clocks, the rings, the rationed intakes) tick-identical in redsim".into())
        }),
    });
    // the auto-smelter's real-use judge and a calibration design for each check that must fail
    // it (designs/circuits/tracks/smelter/calibration/)
    const SMELTER_CAL: &str = "designs/circuits/tracks/smelter/calibration";
    let smelter_cal: Vec<String> = ["naive-slice", "open-top", "fuel-top", "plain-line"].iter().map(|f| format!("{SMELTER_CAL}/{f}.txt")).collect();
    let mut s = args("circuit smelter-run --sim --run all designs/circuits/smelter/archive/smelter-1.txt designs/circuits/smelter/archive/smelter-clock-2.txt designs/circuits/smelter/archive/smelter-row.txt designs/circuits/smelter/archive/smelter-row-2.txt designs/circuits/smelter/archive/smelter-row-3.txt");
    s.extend(smelter_cal.iter().cloned());
    s.push(format!("{SMELTER_CAL}/fast-fuel-row.txt"));
    v.push(Check {
        name: "circuit:smelter-judge".into(),
        what: "`circuit smelter-run --sim --run all` (redsim, the whole auto-smelter judge): smelter-1 (one furnace, 4 blocks) smelts the mixed load at 6 items a minute; smelter-clock-2 (two furnaces rationed by an item clock, the load passed back and forth over them) shares it 17 and 17; refused: the naive slice (the feeder under the first line hopper takes every item and every coal as it comes, so slices side by side jam: the second furnace gets an item and no fuel), a furnace fed through an open-topped hopper (stray iron ingots in its input), a fuel hopper pointing into the input hopper (coal never reaches the fuel slot), smelter-clock-2 with a plain line (the items run on to its last furnace: 8 and 26, not shared out)",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(|o| {
            has(&o[0], "SMELTER: smelter-1 smelter 1 items/min per furnace 6.0, first item by 220 gt, items per coal 6.0, spread 18 each, hoppers awake 3, at rest yes, blocks per furnace 4.0, 4 blocks, 1x3x2, redsim")?;
            has(&o[0], "SMELTER: smelter-clock-2 smelter 2 items/min per furnace 5.3, first item by 440 gt, items per coal 5.7, spread 17 each, hoppers awake 5, at rest no, a clock runs on, blocks per furnace 15.5, 31 blocks, 3x5x6, redsim")?;
            has(&o[0], "FAIL: cal-smelter-naive-slice smelter 1: slices side by side, 4 furnaces in all (one input, one fuel and one output chest): 57 of 66 items delivered after 17200 gt (the cap): it jams: the furnace at [1, 1, 0] holds iron_shovel but no fuel")?;
            has(&o[0], "FAIL: cal-smelter-open-top smelter 1: stray items dropped on every open hopper top: 11 of 84 items delivered after 7600 gt (the cap): it jams: iron_ingot in the input slot of the furnace at [1, 1, 0] does not smelt")?;
            has(&o[0], "FAIL: cal-smelter-fuel-top smelter 1: 0 of 18 items delivered after 7600 gt (the cap): it jams: the furnace at [0, 1, 0] holds raw_gold but no fuel")?;
            has(&o[0], "FAIL: cal-smelter-plain-line smelter 2: the furnace at [0, 1, 0] smelted 8 of 34 items (an even share is 17: the items are not shared out across the 2 furnaces)")?;
            has(&o[0], "SMELTER: smelter-row smelter 2 items/min per furnace 6.0, first item by 1140 gt, items per coal 5.7, spread 17 each, hoppers awake 65, at rest no, a clock runs on, blocks per furnace 14.0, end pieces 210 blocks, tileable every 1, rows items/min per furnace 4: 5.7, 8: 5.5, 16: 5.2, 238 blocks, 13x6x17, redsim")?;
            has(&o[0], "FAIL: cal-smelter-fast-fuel-row smelter 2: slices side by side, 16 furnaces in all (one input, one fuel and one output chest): 245 of 258 items delivered after 55600 gt (the cap): it jams: the furnace at [3, 1, -4] holds oak_log but no fuel")?;
            has(&o[0], "SMELTER: smelter-row-2 smelter 2 items/min per furnace 5.2, first item by 1420 gt, items per coal 6.8, spread 13-21, hoppers awake 38, at rest no, a clock runs on, blocks per furnace 14.0, end pieces 138 blocks, tileable every 1, rows items/min per furnace 4: 5.9, 8: 5.7, 16: 5.5, 166 blocks, 11x6x13, redsim")?;
            has(&o[0], "SMELTER: smelter-row-3 smelter 2 items/min per furnace 6.0, first item by 1240 gt, items per coal 5.7, spread 17 each, hoppers awake 38, at rest no, a clock runs on, blocks per furnace 12.0, end pieces 125 blocks, tileable every 1, rows items/min per furnace 4: 5.6, 8: 5.9, 16: 5.5, 149 blocks, 11x6x7, redsim")?;
            Ok("smelter-1 (6.0 items/min, 4 blocks), smelter-clock-2 (17 and 17), smelter-row (rows of 4, 8 and 16 shared, 14 blocks a furnace), smelter-row-2 (the same slice, 138-block end pieces) and smelter-row-3 (12 blocks a furnace) pass; the naive slice, the open-topped feeder, the fuel-into-input hopper, the plain line and the row whose coal comes every item window refused".into())
        }),
    });
    let mut s = args("?circuit verify --workers {workers} designs/circuits/smelter/archive/smelter-1.txt designs/circuits/smelter/archive/smelter-clock-2.txt designs/circuits/smelter/archive/smelter-row.txt");
    s.extend(smelter_cal.iter().cloned());
    v.push(Check {
        name: "circuit:smelter-verify".into(),
        what: "`circuit verify` (the real game) on the auto-smelter calibration: smelter-1 and smelter-clock-2 robust; the naive slice, the open-topped feeder, the fuel-into-input hopper and the plain line refused as on redsim",
        quick: false,
        jvm: true,
        steps: vec![s],
        timeout: 700,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: smelter-1 smelter 1 items/min per furnace 6.0, first item by 220 gt, items per coal 6.0, spread 18 each, hoppers awake 3, at rest yes, blocks per furnace 4.0, bill 15 iron, 0 quartz, 0 redstone, 4 blocks, 1x3x2, robust")?;
            has(&o[0], "CIRCUIT: smelter-clock-2 smelter 2 items/min per furnace 5.3, first item by 440 gt, items per coal 5.7, spread 17 each, hoppers awake 5, at rest no, a clock runs on, blocks per furnace 15.5, bill 60 iron, 1 quartz, 12 redstone, 31 blocks, 3x5x6, robust")?;
            has(&o[0], "FAIL: cal-smelter-naive-slice smelter 1: slices side by side, 4 furnaces in all (one input, one fuel and one output chest): rotation 0: 57 of 66 items delivered after 17200 gt (the cap): it jams: the furnace at [1, 1, 0] holds iron_shovel but no fuel")?;
            has(&o[0], "FAIL: cal-smelter-open-top smelter 1: stray items dropped on every open hopper top: rotation 0: 11 of 84 items delivered after 7600 gt (the cap): it jams: iron_ingot in the input slot")?;
            has(&o[0], "FAIL: cal-smelter-fuel-top smelter 1: 0 of 18 items delivered after 7600 gt (the cap): it jams: the furnace at [0, 1, 0] holds raw_gold but no fuel")?;
            has(&o[0], "FAIL: cal-smelter-plain-line smelter 2: the furnace at [0, 1, 0] smelted 8 of 34 items (an even share is 17: the items are not shared out across the 2 furnaces)")?;
            has(&o[0], "CIRCUIT: smelter-row smelter 2 items/min per furnace 6.0, first item by 1140 gt, items per coal 5.7, spread 17 each, hoppers awake 65, at rest no, a clock runs on, blocks per furnace 14.0, end pieces 210 blocks, tileable every 1, rows items/min per furnace 4: 5.7, 8: 5.5, 16: 5.2, bill 360 iron, 9 quartz, 79 redstone, 238 blocks, 13x6x17, robust")?;
            Ok("smelter-1, smelter-clock-2 and smelter-row (rows of 4, 8, 16) robust; the naive slice, the open-topped feeder, the fuel-into-input hopper and the plain line refused in the game".into())
        }),
    });
    // study 21, auto-brewers (track sorter, coordinator 2026-10-21; crate circuit::brewer): the
    // brewing stand in redsim against the game, every cell every gt (brew and fuel timers and the
    // bottle bits included): two calibration stands (one brew, one that never brews) and the
    // one-stand record's base run (four batches, both stages)
    const BREWER_CAL: &str = "designs/circuits/tracks/brewer/calibration";
    v.push(Check {
        name: "circuit:brewer-compare".into(),
        what: "`circuit brewer-run --compare` (the brewing stand model): the calibration stand whose bottom hopper is unlocked (every bottle out unbrewed), the one whose bottom hopper is locked for good (a brew: water to awkward) and brewer-small's base run (four batches of swiftness), in the game and on redsim, every cell of the watched box (container contents, brew and fuel timers, the bottle bits) identical every gt",
        quick: false,
        jvm: true,
        steps: vec![
            args(&format!("circuit brewer-run --compare {BREWER_CAL}/unlocked-bottom.txt")),
            args(&format!("circuit brewer-run --compare {BREWER_CAL}/locked-bottom.txt")),
            args("circuit brewer-run --compare designs/circuits/brew/archive/brewer-small.txt"),
        ],
        timeout: 400,
        pred: Box::new(|o| {
            has(&o[0], "IDENTICAL: cal-brew-unlocked-bottom, 4961 frames")?;
            has(&o[1], "IDENTICAL: cal-brew-locked-bottom, 4841 frames")?;
            has(&o[2], "IDENTICAL: brewer-small, 6821 frames")?;
            Ok("the unlocked-bottom stand (4961 frames), the locked-bottom stand (4841 frames, a brew) and brewer-small's base run (6821 frames: the latch clock, the droppers its torch fires, both stages of four batches) tick-identical in redsim".into())
        }),
    });
    // the auto-brewer's real-use judge and a calibration design for each check that must fail it
    // (brewer-1, the first record, 49 blocks, kept in the track: its droppers fired by a repeater
    // line from the clock's wall torch)
    let brewer_cal: Vec<String> = ["locked-bottom", "no-slow-release", "unlocked-bottom"].iter().map(|f| format!("{BREWER_CAL}/{f}.txt")).collect();
    let mut s = args("circuit brewer-run --sim --run all designs/circuits/brew/archive/brewer-small.txt designs/circuits/tracks/brewer/brewer-1.txt");
    s.extend(brewer_cal.iter().cloned());
    v.push(Check {
        name: "circuit:brewer-judge".into(),
        what: "`circuit brewer-run --sim --run all` (redsim, the whole auto-brewer judge): brewer-small (one stand, 42 blocks in 3x6x5) and brewer-1 (49 blocks) brew four batches of swiftness at 1.9 potions a minute; refused: a stand whose bottom hopper is locked for good (nothing delivered), one whose bottom hopper is unlocked (every water bottle out unbrewed), brewer-1 without its slow release (the second batch's first bottle out unbrewed)",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(|o| {
            has(&o[0], "BREWER: brewer-small brewer 1 potions/min per stand 1.9, first potion by 1680 gt, blaze powder 1 for 8 brews, hoppers awake 9, at rest no, a clock runs on, blocks per stand 42.0, 42 blocks, 3x6x5, redsim")?;
            has(&o[0], "BREWER: brewer-1 brewer 1 potions/min per stand 1.9, first potion by 1680 gt, blaze powder 1 for 8 brews, hoppers awake 5, at rest no, a clock runs on, blocks per stand 49.0, 49 blocks, 5x6x5, redsim")?;
            has(&o[0], "FAIL: cal-brew-locked-bottom brewer 1: 0 of 12 potions of swiftness delivered after 4800 gt: it stops short")?;
            has(&o[0], "FAIL: cal-brew-no-slow-release brewer 1: potion[water] x3 reached the output chest (a water bottle never brewed)")?;
            has(&o[0], "FAIL: cal-brew-unlocked-bottom brewer 1: potion[water] x12 reached the output chest (a water bottle never brewed)")?;
            Ok("brewer-small (1.9 potions/min, 42 blocks, 3x6x5) and brewer-1 (49 blocks) pass; the locked, the unlocked and the race-prone bottom hopper refused".into())
        }),
    });
    let mut s = args("?circuit verify --workers {workers} designs/circuits/brew/archive/brewer-small.txt");
    s.push(format!("{BREWER_CAL}/no-slow-release.txt"));
    s.push(format!("{BREWER_CAL}/unlocked-bottom.txt"));
    v.push(Check {
        name: "circuit:brewer-verify".into(),
        what: "`circuit verify` (the real game) on the auto-brewer calibration: brewer-small robust; the unlocked bottom hopper and brewer-1 without its slow release refused as on redsim",
        quick: false,
        jvm: true,
        steps: vec![s],
        timeout: 700,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: brewer-small brewer 1 potions/min per stand 1.9, first potion by 1680 gt, blaze powder 1 for 8 brews, hoppers awake 9, at rest no, a clock runs on, blocks per stand 42.0, bill 60 iron, 3 quartz, 19 redstone, 42 blocks, 3x6x5, robust")?;
            has(&o[0], "FAIL: cal-brew-no-slow-release brewer 1: potion[water] x3 reached the output chest (a water bottle never brewed)")?;
            has(&o[0], "FAIL: cal-brew-unlocked-bottom brewer 1: potion[water] x12 reached the output chest (a water bottle never brewed)")?;
            Ok("brewer-small robust; the race-prone and the unlocked bottom hopper refused in the game".into())
        }),
    });
    // cart transport (designs/briefs/cart-transport.md, track sorter): the whole-line run on the
    // rail twin (crate::cartsys): a double chest through one hopper minecart over 128 blocks
    let mut s = args("circuit cart-system --sim designs/circuits/rail/system/line-b.txt");
    s.push("designs/circuits/tracks/cart-transport/line-a.txt".to_string());
    let mut tr = args("circuit cart-system --sim --trickle designs/circuits/rail/system/line-b.txt");
    tr.push("designs/circuits/tracks/cart-transport/calibration/line-shuttles.txt".to_string());
    v.push(Check {
        name: "circuit:cart-system".into(),
        what: "`circuit cart-system --sim` (redsim, the whole cart line: 3456 items from a double supply chest through one hopper minecart over 128 blocks into a double storage chest): line-b (the cart under the supply chest; the two-hopper unloader ul-gate) in 20.9 min, the obvious line-a (one-hopper unloader) in 32.4 min; `--trickle` (5 items every 100 gt up to 300, then nothing): line-b delivers all and its cart ends parked at the loader; the loader before the dry-supply fix sends its empty cart away from the empty chest at the start and the cart stalls at the foot of the slope (10 of 300)",
        quick: true,
        jvm: false,
        steps: vec![s, tr],
        timeout: 200,
        pred: Box::new(|o| {
            has(&o[0], "SYSTEM: line-b minutes for a double chest 20.9, 2.75 items/s, 11 trips")?;
            has(&o[0], "SYSTEM: line-a minutes for a double chest 32.4, 1.78 items/s, 11 trips")?;
            has(&o[1], "SYSTEM: line-b trickle: 300 items fed 5 every 100 gt, all delivered by 5.9 min, the cart parked at the loader at (133.83, 0.89, 0.50)")?;
            has(&o[1], "FAIL: line-shuttles: 10 of 300 delivered after 115 min (the cap)")?;
            Ok("line-b 20.9 min, line-a 32.4 min for a double chest over 128 blocks; trickle: line-b's cart parks at the loader, the old loader's line stalls".into())
        }),
    });
    // the super smelter (designs/briefs/super-smelter.md, track sorter): the smelter judge's chest
    // run on redsim (a double chest of raw iron, the coal it needs plus a stack, never refilled):
    // the self-clocked metered row at the ceiling, the obvious row starved of coal
    let mut s = args("circuit smelter-run --sim --run chest designs/circuits/smelter/archive/super-smelter-8.txt");
    s.push("designs/circuits/tracks/super-smelter/obvious-8.txt".to_string());
    v.push(Check {
        name: "circuit:super-smelter".into(),
        what: "`circuit smelter-run --sim --run chest` (redsim, the super smelter's judge: 3456 raw iron, 496 coal never refilled): super-smelter-8 shares the load and the coal evenly (432 per furnace, 72.2 min = the ceiling, at rest); the obvious row (hopper lines over and beside the furnaces) stops at 30%: its first two positions take the coal",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 200,
        pred: Box::new(|o| {
            has(&o[0], "SMELTER: super-smelter-8 smelter 8 (chest run): minutes for a double chest 72.2, of the ceiling 100% (72.0 min: 8 furnaces always busy), first ingot by 340 gt, spread 432 each, coal left 64 (64 in the machine), at rest yes")?;
            has(&o[0], "FAIL: obvious-8 (chest run): 30% smelted (1028 of 3456 ingots)")?;
            Ok("super-smelter-8 at the ceiling (72.2 min, 432 each); the obvious row refused at 30% smelted".into())
        }),
    });
    // study 22, auto-composters (track sorter, coordinator 2026-10-21; crate circuit::composter):
    // the composter in redsim against the game, every cell every gt, the composting chances drawn
    // from the mirrored level random: the naive build and compost-row's base run (a row of 4)
    const COMPOST_CAL: &str = "designs/circuits/tracks/composter/calibration";
    v.push(Check {
        name: "circuit:composter-compare".into(),
        what: "`circuit composter-run --compare` (the composter model, the level random mirrored): the naive build (a hopper over the composter fed from the input chest) and compost-row's base run (a row of 4 slices, one crop each), in the game and on redsim, every cell of the watched box (composter levels, container contents) identical every gt",
        quick: false,
        jvm: true,
        steps: vec![
            args(&format!("circuit composter-run --compare {COMPOST_CAL}/naive.txt")),
            args("circuit composter-run --compare designs/circuits/compost/archive/compost-row.txt"),
        ],
        timeout: 400,
        pred: Box::new(|o| {
            has(&o[0], "IDENTICAL: cal-compost-naive, 2241 frames")?;
            has(&o[1], "IDENTICAL: compost-row, 5241 frames")?;
            Ok("the naive build (2241 frames) and compost-row's row of 4 (5241 frames: 22 bone meal, every chance drawn alike) tick-identical in redsim".into())
        }),
    });
    let mut s = args("circuit composter-run --sim --run all designs/circuits/compost/archive/compost-row.txt");
    s.push(format!("{COMPOST_CAL}/naive.txt"));
    v.push(Check {
        name: "circuit:composter-judge".into(),
        what: "`circuit composter-run --sim --run all` (redsim, the whole auto-composter judge): compost-row (one composter a slice, 18 blocks) composts the four crops of a row of 4 and 8, every other item to the overflow chest; refused: the naive build (its hopper over the composter keeps the cobblestone for good)",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(|o| {
            has(&o[0], "COMPOSTER: compost-row composter 1 bone meal/h per composter 100, items composted/min 111.4, bone meal 22 from 368 items, first bone meal by 340 gt, hoppers awake 12, at rest yes, blocks per composter 18.0, tileable every 1, 18 blocks, 1x6x5, redsim")?;
            has(&o[0], "FAIL: cal-compost-naive composter 1: 0 of 20 cobblestone reached the overflow chest (stuck in the machine)")?;
            Ok("compost-row (22 bone meal from 368 crops in a row of 4) passes; the naive build refused".into())
        }),
    });
    let mut s = args("?circuit verify --workers {workers} designs/circuits/compost/archive/compost-row.txt");
    s.push(format!("{COMPOST_CAL}/naive.txt"));
    s.push(format!("{COMPOST_CAL}/open-tops.txt"));
    v.push(Check {
        name: "circuit:composter-verify".into(),
        what: "`circuit verify` (the real game) on the auto-composter calibration: compost-row robust; the naive build refused; compost-row without the blocks on its line hoppers refused (the strays crowd the line: at rotation 1 wheat passes its filter)",
        quick: false,
        jvm: true,
        steps: vec![s],
        timeout: 700,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: compost-row composter 1 bone meal/h per composter 100, items composted/min 111.4, bone meal 22 from 368 items, first bone meal by 340 gt, hoppers awake 12, at rest yes, blocks per composter 18.0, tileable every 1, bill 20 iron, 1 quartz, 10 redstone, 18 blocks, 1x6x5, robust")?;
            has(&o[0], "FAIL: cal-compost-naive composter 1: 0 of 20 cobblestone reached the overflow chest (stuck in the machine)")?;
            has(&o[0], "FAIL: cal-compost-open-tops composter 1: stray items dropped on every open hopper top: rotation 1: wheat x10 reached the overflow chest (a compostable item not composted)")?;
            Ok("compost-row robust; the naive build and the open-topped line refused in the game".into())
        }),
    });
    // the item elevator's judge (clk-misc 2026-10-21; crate circuit::lift): the archived 5-up dropper
    // column, the resting bubble column (a dropper shooting items into the world, the bubble column
    // lifting them, the clock gated by the dropper's contents) and a calibration design whose second
    // clock's phase follows the placement order
    const LIFT: [&str; 3] = [
        "designs/circuits/elevator-items/tracks/clk-misc/lift5-chain-14b.txt",
        "designs/circuits/elevator-items/tracks/clk-misc/bubble-rest-n0.txt",
        "designs/circuits/elevator-items/tracks/clk-misc/calibration/two-clocks-11b.txt",
    ];
    let mut s = args("circuit lift-run --builds");
    s.extend(LIFT.iter().map(|f| f.to_string()));
    v.push(Check {
        name: "circuit:lift-judge".into(),
        what: "`circuit lift-run --builds` (redsim, the whole item-elevator judge): lift5-chain-14b (a dropper column fired by one observer clock through a chain, each group 4 gt after the one below) lifts the mixed load 5 blocks at 2.50 items/s in every build; bubble-rest-n0 (a dropper shooting each item into a bubble column, its clock running only while the dropper holds an item) lifts it 3 blocks and rests; refused: two independent clocks (the lower group's top dropper reads the upper group's power through quasi-connectivity and stays triggered when the clocks start in the wrong order, which follows the placement order: rotation 0 jams)",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 180,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: lift5-chain-14b item-elevator 5 items/s 2.50, first item 30 gt, at rest no, bill 5 iron, 7 quartz, 20 redstone, 14 blocks, 4x6x1, robust (redsim)")?;
            has(&o[0], "CIRCUIT: bubble-rest-n0 item-elevator 3 items/s 2.50, first item 41 gt, at rest yes, bill 11 iron, 3 quartz, 9 redstone, 1 slime, 40 blocks, 4x8x4, robust (redsim)")?;
            has(&o[0], "FAIL: cal-lift-two-clocks item-elevator 5: rotation 0 fails in every position: 0 of 193 items reached the output chest")?;
            Ok("lift5-chain-14b robust at 2.50 items/s; bubble-rest-n0 robust and at rest; two independent clocks refused (rotation 0 jams)".into())
        }),
    });
    let mut s = args("?circuit verify --workers {workers}");
    s.extend(LIFT.iter().map(|f| f.to_string()));
    v.push(Check {
        name: "circuit:lift-verify".into(),
        what: "`circuit verify` (the real game) on the item-elevator calibration: lift5-chain-14b robust at 2.50 items/s, bubble-rest-n0 robust and at rest, the two independent clocks refused as on redsim",
        quick: false,
        jvm: true,
        steps: vec![s],
        timeout: 900,
        pred: Box::new(|o| {
            has(&o[0], "CIRCUIT: lift5-chain-14b item-elevator 5 items/s 2.50, first item 30 gt, at rest no, bill 5 iron, 7 quartz, 20 redstone, 14 blocks, 4x6x1, robust")?;
            has(&o[0], "CIRCUIT: bubble-rest-n0 item-elevator 3 items/s 2.50, first item 41 gt, at rest yes, bill 11 iron, 3 quartz, 9 redstone, 1 slime, 40 blocks, 4x8x4, robust")?;
            has(&o[0], "FAIL: cal-lift-two-clocks item-elevator 5: rotation 0 fails in every position: 0 of 193 items reached the output chest")?;
            Ok("lift5-chain-14b and bubble-rest-n0 robust; the two independent clocks refused in the game".into())
        }),
    });
    // study 18, crop farms (track w86, crate circuit::farm): one game run each (a run repeats for
    // its build): the cane record harvests every growth into the chest; the melon record's drops
    // (3 to 7 slices in one stack a fruit) are each seen once and all reach the chest; the melon
    // calibration (no hopper minecart: a stack squeezed by the piston head shoots out of the fruit's
    // cell before the hopper takes it) must fail on the lost slices
    v.push(Check {
        name: "circuit:farm-run".into(),
        what: "`circuit farm-run` (the real game, real random ticks, ITEM INTRUSION): cane-mc-8b and melon-mc-7b harvest every growth with nothing lost and report their efficiency (items per hour per block; melon: every drop stack seen and counted); the melon calibration without its hopper minecart loses a squeezed stack and fails",
        quick: false,
        jvm: true,
        steps: vec![
            args("circuit farm-run designs/circuits/farm/archive/cane-mc-8b.txt"),
            args("circuit farm-run designs/circuits/farm/archive/melon-mc-7b.txt"),
            args("circuit farm-run designs/circuits/farm/tracks/w86/calibration/melon-lost.txt"),
        ],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[0], "FARM: cane-mc-8b harvester sugar_cane: items per hour per block ")?;
            has(&o[1], "drops seen")?;
            has(&o[1], "FARM: melon-mc-7b harvester melon: items per hour per block ")?;
            has(&o[2], "FAIL: cal-melon-lost: ")?;
            has(&o[2], "harvested items are neither in the judge's chests")?;
            Ok("cane-mc-8b and melon-mc-7b pass (every melon drop counted); the melon calibration fails on its lost slices".into())
        }),
    });
    // redsim's shulker port: the real-game traces of the reference loader and unloader replayed
    // tick for tick (box placement, hoppers, comparators, the piston-broken drop as the bridge
    // seeds it, its pickup); since 2026-11-08 (`mctas test`) every exact shulker trace, the 8
    // whose archived designs left the archive replayed from their restored fixtures
    // (traces/fixtures/, `redsim-fuzz trace-fixtures --compare 'circuit shulker compare'`)
    let shulker_traces = files_in("designs/circuits/shulker/traces/exact", "trace");
    let ns = shulker_traces.len();
    let mut s = args("circuit shulker compare");
    s.extend(shulker_traces);
    v.push(Check {
        name: "circuit:shulker-compare".into(),
        what: "`circuit shulker compare` replays every exact real-game trace of the shulker loaders and unloaders (the reference loader and unloader, 3 boxes of unstackables each; 1, 16 and 64 boxes; the always-on, flush, pitch-3, 1-wide, speed and mixed builds) in redsim: every cell and entity identical every tick",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(move |o| {
            has(&o[0], "OK shulker-ref-loader-1: 687 ticks identical")?;
            has(&o[0], "OK shulker-ref-unloader-1: 678 ticks identical")?;
            let ok = o[0].lines().filter(|l| l.starts_with("OK ") && l.ends_with(" ticks identical")).count();
            if ok != ns {
                return Err(format!("{ok} of {ns} shulker traces identical: {}", tail(&o[0])));
            }
            Ok(format!("{ns} of {ns} shulker traces tick-identical in redsim (the reference loader and unloader among them)"))
        }),
    });
    // the judge's full-chest check (a chest full of boxes, as a player fills it): track pext's
    // 28-block loader jams there (its first dispenser fire comes before a box arrives and the
    // chest never empties); track shulker's always-on loader and unloader pass. The endless-supply
    // check (that chest kept full, refilled before every gt): track pext's first 1x6x9 loader jams
    // there (it counts the chest's level changes); the always-on loader and unloader pass
    v.push(Check {
        name: "circuit:shulker-full-chest".into(),
        what: "`circuit shulker rs --only full-chest`, `--only endless` and `--only one-box` (redsim): a chest full of boxes (27), that chest kept full, then boxes coming one at a time; a loader whose box-arrival line counts the chest's on and off jams with 0 of 26 boxes in the first, one that counts its level changes jams in the second, one that reads the chest at all jams in the third, track shulker's always-on loader and unloader deliver all in every one",
        quick: true,
        jvm: false,
        steps: vec![
            args("circuit shulker rs --only full-chest designs/circuits/selftest/shulker-full-chest-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt designs/circuits/shulker/tracks/shulker/auto-unloader-1.txt"),
            args("circuit shulker rs --only endless designs/circuits/selftest/shulker-endless-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt designs/circuits/shulker/tracks/shulker/auto-unloader-1.txt"),
            args("circuit shulker rs --only one-box designs/circuits/selftest/shulker-one-box-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt designs/circuits/shulker/tracks/shulker/auto-unloader-1.txt"),
            args("circuit shulker rs --only one-box-burst designs/circuits/selftest/shulker-one-box-burst-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt"),
            args("circuit shulker rs --only one-box-burst,one-box-burst-23 designs/circuits/selftest/shulker-one-box-pace-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt"),
            args("circuit shulker intakes designs/circuits/selftest/shulker-intrusion.txt designs/circuits/shulker/archive/shulker-loader-1wide-1x6x11-36b.txt"),
        ],
        timeout: 120,
        pred: Box::new(|o| {
            has(&o[0], "FAIL cal-shulker-full-chest-jam full-chest")?;
            has(&o[0], "0 of 26 boxes")?;
            has(&o[0], "OK shulker-auto-loader-1 full-chest")?;
            has(&o[0], "OK shulker-auto-unloader-1 full-chest")?;
            has(&o[1], "FAIL cal-shulker-endless-jam endless")?;
            has(&o[1], "0 of 26 boxes")?;
            has(&o[1], "OK shulker-auto-loader-1 endless")?;
            has(&o[1], "OK shulker-auto-unloader-1 endless")?;
            has(&o[2], "FAIL cal-shulker-one-box-jam one-box")?;
            has(&o[2], "0 of 3 boxes")?;
            has(&o[2], "OK shulker-auto-loader-1 one-box")?;
            has(&o[2], "OK shulker-auto-unloader-1 one-box")?;
            has(&o[3], "FAIL cal-shulker-one-box-burst-jam one-box-burst")?;
            has(&o[3], "OK shulker-auto-loader-1 one-box-burst")?;
            has(&o[4], "OK cal-shulker-one-box-pace-jam one-box-burst ")?;
            has(&o[4], "FAIL cal-shulker-one-box-pace-jam one-box-burst-23")?;
            has(&o[4], "2 of 3 boxes")?;
            has(&o[4], "OK shulker-auto-loader-1 one-box-burst-23")?;
            has(&o[5], "cal-shulker-intrusion: intake 1,1,0 (the hopper under it feeds the item line)")?;
            has(&o[5], "the boxes differ: [(27, 27), (27, 27), (0, 0)]")?;
            has(&o[5], "shulker-loader-1wide-1x6x11-36b: intake 0,0,1")?;
            has(&o[5], "passes (3 boxes as without the strays")?;
            Ok("full chest, endless supply, one box at a time, nearly full boxes one at a time (at three paces), stray items on the open hopper tops: the weak loaders jam or lose a box, the always-on loader and unloader and the 36b deliver every box".into())
        }),
    });
    sim_checks(&mut v);
    v
}

/// redsim's own known answers (`cargo test --release -p redsim --lib --tests`; the doc tests need
/// rustdoc on PATH): the mob layer's golem cadence
/// and memory, weather, entity physics, leashes, links, shapes, and the recorded game fixtures
/// (crates/redsim/tests/fixtures). It needs cargo (every agent box builds with it).
fn redsim_tests() -> Check {
    Check {
        name: "sim:redsim-tests".into(),
        what: "redsim's known answers and recorded fixtures (`cargo test --release -p redsim --lib --tests`: the mob layer's golem sensor memory, the 600 gt summon cycle, spawn spots, villager schedules, weather, item and fall physics, leashes, links, shapes, the game's frames of crates/redsim/tests/fixtures): every test passes",
        quick: true,
        jvm: false,
        steps: vec![args("@cargo test --release -p redsim --lib --tests")],
        timeout: 1200,
        pred: Box::new(|o| {
            let results: Vec<&str> = o[0].lines().filter(|l| l.starts_with("test result: ")).collect();
            if results.is_empty() {
                return Err(format!("no test result: {}", tail(&o[0])));
            }
            if let Some(bad) = results.iter().find(|l| !l.starts_with("test result: ok.")) {
                return Err(bad.to_string());
            }
            let passed: usize = results.iter().filter_map(|l| l.split_whitespace().nth(3).and_then(|n| n.parse::<usize>().ok())).sum();
            Ok(format!("{passed} redsim tests pass ({} test binaries)", results.len()))
        }),
    }
}

/// The files of a directory with this extension, sorted (repository paths).
fn files_in(dir: &str, ext: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == ext)).map(|p| p.to_string_lossy().to_string()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// The simulator's pieces (vjeux 2026-11-08: the session that changes the simulator runs the
/// simulator tests): every recorded game answer replayed on redsim (no JVM, seconds: the game
/// never changes, so a recording is the game's answer while the files it was recorded from stay
/// the same, and `recorded check` fails a stale one) and the live differential fuzzers, one per
/// area (testplan::RULES picks the areas a change touches; fixed seeds: the same cases every run).
fn sim_checks(v: &mut Vec<Check>) {
    v.push(redsim_tests());
    // the exact traces of the clock, entity, crop, TNT, rail and item scenes (clk-misc's and
    // clockinfra's redsim tests, the kept clock traces), and the recordings made for `mctas test`
    // (2026-11-08) of what the door and clock judges' game checks ran: the door anchors' lever
    // cycles (3 open/close cycles; at a second position; turned), the clock calibration and
    // selftest clocks (the cart clock and the falling cart included), and the item elevators of
    // circuit:lift-verify running on a loaded hopper
    let trace_dirs = [
        "designs/clocks/traces/exact",
        "designs/clocks/tracks/clk-misc/redsim-tests/traces",
        "designs/clocks/tracks/clockinfra/redsim-tests/traces",
        "designs/redsim/recorded/doors",
        "designs/redsim/recorded/clocks",
        "designs/redsim/recorded/lifts",
    ];
    let n: usize = trace_dirs.iter().map(|d| files_in(d, "trace").len()).sum();
    let mut check = args("recorded check");
    check.extend(RECORDING_DIRS.iter().map(|d| d.to_string()));
    let mut replay = args("door trace-compare --jobs {threads}");
    replay.extend(trace_dirs.iter().map(|d| d.to_string()));
    v.push(Check {
        name: "sim:recorded-traces".into(),
        what: "every recording current (`recorded check`: no recording's design changed since it was recorded, none edited, unlisted or missing), then the exact real-game traces of the clock, entity, crop, TNT, rail, item and boat scenes replayed on redsim, every cell and entity identical every tick (`door trace-compare`, on every core)",
        quick: true,
        jvm: false,
        steps: vec![check, replay],
        timeout: 300,
        pred: Box::new(move |o| {
            has(&o[0], "RECORDINGS CURRENT")?;
            has(&o[1], &format!("{n} of {n} traces tick-identical")).map(|_| format!("recordings current; {n} of {n} exact traces tick-identical"))
        }),
    });
    let boats = files_in("designs/boat-leads/traces", "trace");
    let nb = boats.len();
    let mut s = args("boat-leads compare");
    s.extend(boats);
    v.push(Check {
        name: "sim:recorded-boats".into(),
        what: "the boat and lead fixtures' exact real-game traces (positions, motion, rotations, leashes, blocks and the level random every frame) replayed on redsim (`boat-leads compare`)",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(move |o| has(&o[0], &format!("CALIBRATION: {nb}/{nb} exact fixture comparisons passed")).map(|_| format!("{nb}/{nb} boat and lead traces exact"))),
    });
    // the mob AI's scenes recorded in the game (`mobai record`): every mob's position, motion,
    // rotations, randoms, goals, path, target, brain memories and behaviours, the blocks and the
    // level random every tick, replayed on redsim (`mobai replay`)
    let nm = files_in("designs/redsim/recorded/mobai", "rec").len();
    v.push(Check {
        name: "sim:recorded-mobai".into(),
        what: "the mob AI scenes recorded in the real game (zombies strolling, pathing round walls, to a villager and through a door on Hard; a golem and a zombie fighting; villagers claiming beds and going to sleep; a creeper going for a player; skeletons at night and fleeing the sun; spiders climbing walls and fighting a golem; bats flying, hanging and waking) replayed on redsim, every mob's state and the level random identical every tick (`mobai replay`)",
        quick: true,
        jvm: false,
        steps: vec![args("mobai replay designs/redsim/recorded/mobai")],
        timeout: 300,
        pred: Box::new(move |o| has(&o[0], &format!("REPLAY: {nm}/{nm} mob AI recordings exact")).map(|_| format!("{nm}/{nm} mob AI recordings exact"))),
    });
    // (tickexact 2026-11-14, G1 car-14 CHANGES 1 (3)) the PENDING recordings (`designs/redsim/recorded/pending`: recorded by
    // a lane, waiting for the gating car that moves them into recorded/mobai), replayed every run like the gated ones; a
    // recording whose scene changed since it was recorded fails the replay by name (`mobai replay` checks each header's
    // `scene_hash` against the scene now: fixture.rs, "the scene changed since it was recorded"); the ones that do not
    // replay exactly are named here with the reason, and every other one must
    let np: Vec<String> = files_in(PENDING, "rec").into_iter().filter(|f| !PENDING_EXCLUDED.iter().any(|(n, _)| f.ends_with(&format!("/{n}")))).collect();
    let npn = np.len();
    let mut pending_step = args("mobai replay");
    pending_step.extend(np);
    v.push(Check {
        name: "sim:recorded-pending".into(),
        what: "the pending mob AI recordings (designs/redsim/recorded/pending: a lane's game recordings waiting for their gating car), replayed on redsim at 0 differences, the excluded ones named in PENDING_EXCLUDED with their reason",
        quick: true,
        jvm: false,
        steps: vec![pending_step],
        timeout: 300,
        pred: Box::new(move |o| has(&o[0], &format!("REPLAY: {npn}/{npn} mob AI recordings exact")).map(|_| format!("{npn}/{npn} pending mob AI recordings exact ({} excluded by name)", PENDING_EXCLUDED.len()))),
    });
    // the judges' runs recorded in the game for `mctas test` (circuit record, 2026-11-08): the
    // sorter, smelter, brewer and composter exact gates (their old `*-compare` checks: every cell
    // every gt, furnace and brew timers and the mirrored level random included) and the circuit
    // judge's stimulus on every calibration circuit (what circuit:verify-calibration-* ran)
    let rec = |dirs: &[&str]| -> usize { dirs.iter().map(|d| files_in(d, "rec").len()).sum() };
    let study_dirs = ["designs/redsim/recorded/sorter", "designs/redsim/recorded/smelter", "designs/redsim/recorded/brewer", "designs/redsim/recorded/composter"];
    let nr = rec(&study_dirs);
    let mut s = args("circuit replay --jobs {threads}");
    s.extend(study_dirs.iter().map(|d| d.to_string()));
    v.push(Check {
        name: "sim:recorded-studies".into(),
        what: "the sorter, smelter, brewer and composter judges' runs recorded in the real game (sorter-classic's base and direct water-stream runs, storage-bulk's jam run, the double-chest test; smelter-1, smelter-clock-2, smelter-row and the naive slice's tiled run; the brewing stands locked and unlocked and brewer-small; the naive composter and compost-row) replayed on redsim, every cell of the watched box identical every gt (`circuit replay`)",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 300,
        pred: Box::new(move |o| has(&o[0], &format!("{nr} of {nr} recordings identical")).map(|_| format!("{nr} of {nr} recorded judge runs identical on redsim"))),
    });
    // `door ideal` (the game) on the ideal:agree-* and plan-phase plans, recorded 2026-11-08
    // (`mctas door ideal <plan> [--between-ticks]`, the game 26.3, on 4ab5af6ed) on frozen copies of
    // the plans and their designs (designs/redsim/recorded/ideal: track work never touches them):
    // `door ideal-rs` must give the game's verdict and ticks, as ideal:agree-* compared them live.
    // The plan's and its design's hashes are recorded with them: a changed plan or design is
    // STALE (record the verdicts again with `door ideal`, the hashes with `mctas recorded hash`)
    const ID: &str = "designs/redsim/recorded/ideal";
    let ideal: Vec<(String, &str, &str, bool, &str)> = vec![
        // (plan, its hash, its design's hash, --between-ticks, the game's verdict)
        (format!("{ID}/c78z.ideal"), "f86fa500c59bcedb", "dd3f5af9d5683f4d", false, "BROKEN: at offset [0, 0, 0]: cycle 0: not open: [1, 1, 0] is minecraft:stone; [0, 0, 0] is minecraft:stone; [1, 0, 0] is minecraft:stone"),
        (format!("{ID}/c78z.ideal"), "f86fa500c59bcedb", "dd3f5af9d5683f4d", true, "WORKS (4 positions x 2 cycles): open after 35 ticks / closed after 35 ticks"),
        (format!("{ID}/c78z-u1.ideal"), "2debac9098fb06ed", "dd3f5af9d5683f4d", false, "BROKEN: at offset [0, 0, 0]: cycle 0: not open: [0, 0, 0] is minecraft:stone; [1, 0, 0] is minecraft:stone; [2, 0, 0] is minecraft:stone"),
        (format!("{ID}/c78z-u1.ideal"), "2debac9098fb06ed", "dd3f5af9d5683f4d", true, "BROKEN: at offset [0, 0, 0]: cycle 0: not open: [1, 0, 0] is minecraft:stone; [2, 0, 0] is minecraft:stone"),
        (format!("{ID}/s25.ideal"), "830b401bd0b30252", "570585920dd7d4e4", false, "WORKS (4 positions x 2 cycles): open after 32 ticks / closed after 32 ticks"),
        (format!("{ID}/s25.ideal"), "830b401bd0b30252", "570585920dd7d4e4", true, "WORKS (4 positions x 2 cycles): open after 33 ticks / closed after 33 ticks"),
        (format!("{ID}/s25-br-observer.ideal"), "b09bc46977cea098", "570585920dd7d4e4", false, "WORKS (4 positions x 2 cycles): open after 34 ticks / closed after 34 ticks"),
        (format!("{ID}/s25-br-observer.ideal"), "b09bc46977cea098", "570585920dd7d4e4", true, "WORKS (4 positions x 2 cycles): open after 35 ticks / closed after 35 ticks"),
    ];
    let steps: Vec<Vec<String>> = ideal.iter().map(|(p, _, _, between, _)| args(&format!("?door ideal-rs {p}{}", if *between { " --between-ticks" } else { "" }))).collect();
    v.push(Check {
        name: "sim:recorded-ideal".into(),
        what: "`door ideal-rs` (redsim) on the c78z, c78z-u1, s25 and s25 + observer-driven BR plans, in the tile phase and with --between-ticks (c78z: a 1 gt pulse), gives the game's recorded `door ideal` verdict and ticks (what ideal:agree-* and ideal:plan-phase-observer-jvm compared live); a plan or design changed since the recording fails as stale",
        quick: true,
        jvm: false,
        steps,
        timeout: 300,
        pred: Box::new(move |o| {
            for (k, (plan, ph, dh, between, game)) in ideal.iter().enumerate() {
                let design = std::fs::read_to_string(plan).ok().and_then(|t| t.lines().find_map(|l| l.strip_prefix("design ").map(|d| d.trim().to_string()))).unwrap_or_default();
                let now = (mctas::recorded::file_hash(Path::new(plan)).unwrap_or_default(), mctas::recorded::file_hash(Path::new(&design)).unwrap_or_default());
                if now.0 != *ph || now.1 != *dh {
                    return Err(format!("STALE: {plan} or its design {design} changed since the game's verdict was recorded: record it again (`door ideal {plan}{}`) and update selftest.rs", if *between { " --between-ticks" } else { "" }));
                }
                let rs = ideal_verdict(&o[k]).ok_or(format!("ideal-rs {plan}: no verdict: {}", tail(&o[k])))?;
                if rs != *game {
                    return Err(format!("{plan}{}: ideal-rs `{rs}` vs the game `{game}`", if *between { " --between-ticks" } else { "" }));
                }
            }
            Ok(format!("{} plan runs: redsim's verdicts and ticks are the game's", ideal.len()))
        }),
    });
    let circ_dirs = ["designs/redsim/recorded/circuits"];
    let nc = rec(&circ_dirs);
    if nc > 0 {
        v.push(Check {
            name: "sim:recorded-circuits".into(),
            what: "the circuit judge's stimulus on every calibration circuit (edge detectors, T flip-flops, delay lines, gates, latches, dividers, extenders, flying machines, BUDs, sculk, counters, TNT dupers, timers; the R7 and claim fixtures the game verify ran) recorded in the real game, replayed on redsim: every cell of the watched box identical every gt (`circuit replay`)",
            quick: true,
            jvm: false,
            steps: vec![args("circuit replay --jobs {threads} designs/redsim/recorded/circuits")],
            timeout: 300,
            pred: Box::new(move |o| has(&o[0], &format!("{nc} of {nc} recordings identical")).map(|_| format!("{nc} of {nc} recorded calibration circuits identical on redsim"))),
        });
    }
    // the live differential fuzzers: random circuits run in the game and on redsim, every cell
    // compared every tick (fixed seeds, sized for about a minute on a pool of 8)
    let fuzz = |name: &str, what: &'static str, steps: Vec<Vec<String>>| Check {
        name: name.into(),
        what,
        quick: false,
        jvm: true,
        steps,
        timeout: 900,
        pred: Box::new(|o| {
            for out in o {
                if let Some(l) = out.lines().find(|l| l.contains(" cases in ") && l.contains(" mismatches")) {
                    if !l.contains(": 0 mismatches") || !l.contains(" 0 errors") {
                        return Err(l.trim().to_string());
                    }
                } else if let Some(l) = out.lines().find(|l| l.starts_with("pistfuzz: ")) {
                    if !l.contains(" 0 mismatching, 0 errors") {
                        return Err(l.trim().to_string());
                    }
                } else {
                    return Err(format!("no summary line: {}", tail(out)));
                }
            }
            let sums: Vec<String> = o.iter().filter_map(|out| out.lines().find(|l| (l.contains(" cases in ") && l.contains(" mismatches")) || l.starts_with("pistfuzz: ")).map(|l| l.split(" (").next().unwrap_or(l).trim().to_string())).collect();
            Ok(sums.join(" | "))
        }),
    };
    // (one piece a fuzz run, so the plan runs them at once: tickexact 2026-11-08, each piece's
    // runs one after the other made the pistons' and the redstone's the slowest of a simulator
    // change's mctas test)
    v.push(fuzz(
        "sim:fuzz-redstone-all",
        "`door redsim-fuzz --palette all` (random redstone circuits: every component family) in the game and on redsim, every cell compared every tick: 0 mismatches",
        vec![args("door redsim-fuzz --n 3000 --seed 9101 --palette all --workers {workers} --out {tmp}/all --max-report 2")],
    ));
    v.push(fuzz(
        "sim:fuzz-redstone-core",
        "`door redsim-fuzz --palette core` (the core palette in a bigger box) in the game and on redsim, every cell compared every tick: 0 mismatches",
        vec![args("door redsim-fuzz --n 1500 --seed 9201 --palette core --size 8,4,8 --workers {workers} --out {tmp}/core --max-report 2")],
    ));
    v.push(fuzz(
        "sim:fuzz-containers",
        "`door redsim-fuzz --gen containers` (droppers and hoppers with items, comparators reading them) in the game and on redsim: 0 mismatches",
        vec![args("door redsim-fuzz --gen containers --n 2000 --seed 9301 --workers {workers} --out {tmp}/containers --max-report 2")],
    ));
    v.push(fuzz(
        "sim:fuzz-pistons-mix",
        "`door pistfuzz --mode mix` (random piston setups, flat and boxed, redstone around them) in the game and on redsim, every block, moving block, queue size, drop and the idle flag compared every tick: 0 mismatching",
        vec![args("door pistfuzz --count 1500 --seed 9401 --mode mix --workers {workers} --out-dir {tmp}/mix")],
    ));
    v.push(fuzz(
        "sim:fuzz-pistons-mixed",
        "`door pistfuzz --mode mix --observers --mixed` (piston setups with observers, mixed) in the game and on redsim, compared every tick: 0 mismatching",
        vec![args("door pistfuzz --count 800 --seed 9402 --mode mix --observers --mixed --workers {workers} --out-dir {tmp}/mixed")],
    ));
    v.push(fuzz(
        "sim:fuzz-pistons-slime",
        "`door pistfuzz --mode mix --slime --shapes` (piston setups with slime and shaped blocks) in the game and on redsim, compared every tick: 0 mismatching",
        vec![args("door pistfuzz --count 500 --seed 9403 --mode mix --slime --shapes --workers {workers} --out-dir {tmp}/slime")],
    ));
    v.push(fuzz(
        "sim:fuzz-sculk",
        "`door pistfuzz --sculk` (sculk sensors, calibrated sensors with frequency filters, wool, amethyst, comparators and observers on sensors, among pistons and wiring) in the game and on redsim: 0 mismatching",
        vec![args("door pistfuzz --count 600 --seed 9501 --sculk --workers {workers} --out-dir {tmp}/sculk")],
    ));
    // the official-server cross-checks (the unmodified 26.3 server jar, RCON, a bot player
    // clicking the lever, region files read back): the bridge = the official server, on small
    // focused fixtures (internals.md#the-official-server-cross-checks). The bot's click waits for
    // the lever's OTHER state (vanilla_check::flip; 2026-11-08: waiting for a fixed state let a
    // click land a tick late under load on c79, whose lever rests powered)
    v.push(Check {
        name: "sim:vanilla-check".into(),
        what: "`door vanilla-check` on the community 7x9 (c79): the unmodified official server and the simulator identical tick for tick under the judge's rules at 8 positions x 3 cycles, plus a real-time cycle",
        quick: false,
        jvm: true,
        steps: vec![args("door vanilla-check designs/piston-door-3x3/archive/c79-community-63b-51b.txt")],
        timeout: 900,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.starts_with("vanilla-check: ")).ok_or(format!("no verdict: {}", tail(&o[0])))?;
            if l.contains("PASS") && !l.contains("DIVERGES") { Ok(l.chars().take(140).collect()) } else { Err(l.chars().take(240).collect()) }
        }),
    });
    // the official server steps slowly (a snapshot per tick: ~45 s per gadget position, ~75 s per
    // fuzz case), so these are one piece per gadget and per pair of fuzz cases, all in parallel
    // terrain's regression check of the random ticks (tickexact, 2026-11-08): a terrain sample's
    // world on the deterministic bridge and on redsim for a full game day with mob spawning off on
    // both sides (`mobfarm compare --no-spawning`): every random tick (grass, crops, farmland,
    // amethyst, mushrooms, speleothems), precipitation, lava's fire and the chunk shuffle draw the
    // level random identically, every block identical, every tick. The design is the AFK-pad
    // fixture (no villager, no golem: the terrain's draws alone, not a farm's brains)
    for sample in ["village-edge", "plains"] {
        v.push(Check {
            name: format!("sim:terrain-ticks-{sample}"),
            what: Box::leak(format!("`mobfarm compare --no-spawning` of the AFK-pad fixture in the {sample} sample: a full game day (24,200 ticks) on the deterministic 26.3 bridge and on redsim, the level random, randValue, weather and every block identical every tick (the random ticks, precipitation and lava's fire of real terrain)").into_boxed_str()),
            quick: false,
            jvm: true,
            steps: vec![args(&format!("mobfarm compare designs/circuits/mobfarm/fixtures/afk-pad.txt --terrain {sample} --no-spawning --only world,blocks --days 1 --batch 1000"))],
            timeout: 1200,
            pred: Box::new(|o| {
                let l = o[0].lines().find(|l| l.ends_with(" ticks compared: 0 differences") || l.starts_with("FIRST DIFFERENCE")).ok_or(format!("no verdict: {}", tail(&o[0])))?;
                if l == "24200 ticks compared: 0 differences" { Ok(l.to_string()) } else { Err(l.chars().take(240).collect()) }
            }),
        });
    }
    for (g, what) in [("bulb-toggle", "a copper bulb toggle"), ("comparator-odd", "an odd comparator"), ("dispenser-qc", "a dispenser's quasi-connectivity"), ("double-move", "a double piston move"), ("dust-reach", "dust reach"), ("early-window", "an early window")] {
        v.push(Check {
            name: format!("sim:vanilla-gadget-{g}"),
            what: Box::leak(format!("`door vanilla-gadget` on {what} (designs/gadgets/{g}.drive) at 2 positions: its drive script replayed on the unmodified official server, identical tick for tick with the simulator, every expect true").into_boxed_str()),
            quick: false,
            jvm: true,
            steps: vec![args(&format!("door vanilla-gadget designs/gadgets/{g}.drive --positions 2"))],
            timeout: 600,
            pred: Box::new(|o| {
                let l = o[0].lines().find(|l| l.starts_with("vanilla-gadget: ")).ok_or(format!("no verdict: {}", tail(&o[0])))?;
                if l.contains(" PASS: ") && l.contains("drive PASS") { Ok(l.chars().take(160).collect()) } else { Err(l.chars().take(300).collect()) }
            }),
        });
    }
    for seed in [7, 9, 11, 13] {
        v.push(Check {
            name: format!("sim:vanilla-fuzz-{seed}"),
            what: "`door vanilla-fuzz` (2 random small contraptions run on the unmodified official server and in the simulator, a fixed seed): none diverges, no error",
            quick: false,
            jvm: true,
            steps: vec![args(&format!("door vanilla-fuzz --count 2 --seed {seed} --out-dir {{tmp}}/vfuzz"))],
            timeout: 600,
            pred: Box::new(|o| {
                let l = o[0].lines().find(|l| l.starts_with("vanilla-fuzz: ")).ok_or(format!("no summary: {}", tail(&o[0])))?;
                if l.contains(" 0 diverging") && l.contains(" 0 errors") { Ok(l.to_string()) } else { Err(l.to_string()) }
            }),
        });
    }
}

/// Run one command with a timeout; stdout + stderr. A first argument starting with `?` marks a
/// command whose exit status is part of its answer (e.g. `door ideal` on a BROKEN plan exits 1).
/// The command runs with TMPDIR = the check's own scratch folder (nothing it writes to the temp
/// dir meets another check's); `stop` (set by `--fail-fast`) kills it.
pub(crate) fn run_step(exe: &Path, argv: &[String], tmp: &Path, k: usize, timeout: Duration, stop: Option<&AtomicBool>) -> std::result::Result<String, String> {
    let any_exit = argv.first().is_some_and(|a| a.starts_with('?'));
    let mut argv: Vec<String> = argv.to_vec();
    if any_exit {
        argv[0] = argv[0][1..].to_string();
    }
    // `@cargo ...`: another program than mctas (the crate tests of the simulator)
    let program: std::path::PathBuf = match argv.first().and_then(|a| a.strip_prefix('@')) {
        Some("cargo") => {
            argv.remove(0);
            std::env::var_os("CARGO").map(std::path::PathBuf::from).unwrap_or_else(|| "cargo".into())
        }
        Some(other) => return Err(format!("unknown program @{other}")),
        None => exe.to_path_buf(),
    };
    let out = tmp.join(format!("step{k}.log"));
    let f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
    let f2 = f.try_clone().map_err(|e| e.to_string())?;
    let mut child = Command::new(&program).args(&argv).env("TMPDIR", tmp).stdout(Stdio::from(f)).stderr(Stdio::from(f2)).spawn().map_err(|e| format!("{}: {e}", program.display()))?;
    let t0 = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        if t0.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("timeout after {} s: mctas {}", timeout.as_secs(), argv.join(" ")));
        }
        if stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("stopped (--fail-fast: another piece failed)".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    if !status.success() && !any_exit {
        return Err(format!("exit {:?}: mctas {}: {}", status.code(), argv.join(" ").chars().take(120).collect::<String>(), tail(&text)));
    }
    Ok(text)
}

/// `door selftest`: every check (`--quick`: the ones that start no JVM; `--only`: by name), run
/// like `mctas test --all` (testplan.rs: parallel, sized from the machine).
pub fn run(o: &Opts) -> Result<()> {
    if o.list {
        for c in checks() {
            let (g, area, workers, ..) = crate::testplan::row(&c.name);
            let group = if area.is_empty() { g.name().to_string() } else { format!("{} {area}", g.name()) };
            println!("{:<40} {:<14} {:>2} JVM {:<5} {}", c.name, group, workers, if c.quick { "quick" } else { "" }, c.what);
        }
        return Ok(());
    }
    let mut out = Vec::new();
    let mut pieces = crate::testplan::plan(None, &[], &[], true, true, &mut out);
    pieces.retain(|p| (!o.quick || p.workers == 0) && o.only.as_ref().is_none_or(|s| s.split(',').any(|q| p.check.name.contains(q))));
    let (cpu, jvms, about) = crate::testplan::machine();
    let cpu = if o.jobs > 0 { o.jobs } else { cpu };
    let jvms = if o.jvm_jobs > 0 { o.jvm_jobs } else { jvms };
    println!("door selftest{}: {} checks on {about}: {cpu} threads, {jvms} JVM workers at once (MCTAS_MAX_WORKERS {} per pool)", if o.quick { " --quick" } else { "" }, pieces.len(), crate::testplan::pool_cap().map(|n| n.to_string()).unwrap_or("unset".into()));
    let line = format!("Tests: door selftest{} ({} checks)", if o.quick { " --quick" } else { "" }, pieces.len());
    crate::testplan::run(pieces, &crate::testplan::RunOpts { cpu, jvms, keep: o.keep, fail_fast: false, version: mctas::DEFAULT_VERSION.to_string() }, &line)
}
