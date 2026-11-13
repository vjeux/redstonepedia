//! The J/K junction drivers (day-1, Nov 12 2026; ironspec 05:35 / farmopt 05:39: the last condition for
//! making s 12 final, against <= 10 gt under the +4 rule at s 12's junction gaps 14..16). A junction is
//! power-forced from the controller: the napper-present latch (a two-torch RS NOR, dust-coupled, as the
//! drop latch's) set by the routing edge, its output repeater pointing into the junction rail's cell. A
//! T-junction rail's curve flips with power, so the junction's switch time is the edge to the rail's
//! shape change, and its release the reset edge to the shape's return. Redsim settles the rail in the
//! same gt as its driver (rails::neighbor_changed re-shapes on the spot, as vanilla's RailBlock does; G1
//! 19:47), so the bench prints the measured switch and return against the bar with no allowance (the 1-gt
//! allowance ironspec's 05:35 rule added is dropped, ironspec 20:05).

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};

type P = [i32; 3];

pub const SET_DRIVE: P = [1, 1, 0];
pub const RESET_DRIVE: P = [-4, 1, 2];
/// The junction rail (a T: arms west and east, the stem south; the driver repeater on its north).
pub const JUNCTION: P = [-1, 1, 2];
/// s 12's least junction gap (ironspec Nov 12 05:35 / farmopt 05:39: gaps 14..16).
pub const LEAST_GAP: i64 = 14;
/// ironspec 05:35's +4 rule: switch time + 4 <= the least gap.
pub const RULE_PLUS: i64 = 4;
/// The bar: switch time <= s 12's least gap 14 - the +4 rule's 4 = 10 (ironspec 05:35); the 1-gt allowance that rule
/// added is dropped (ironspec 20:05: redsim re-shapes the rail in its driver's own gt, G1 19:47).
pub const BAR: i64 = LEAST_GAP - RULE_PLUS;
/// The driver repeater (the control removes it: the junction must then never switch).
const DRIVER: P = [-1, 1, 1];
/// The design's output port: an air cell clear of the circuit.
const PORT: P = [3, 1, 6];

const DUST_NS: &str = "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]";
const DUST_EW: &str = "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]";

pub fn design() -> Design {
    let mut d = Design { name: "junction-driver".into(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let mut put = |p: P, s: &str| {
        d.blocks.insert(p, s.to_string());
    };
    // (a diode's `facing` points at its input, as in the game)
    // the set repeater (input SET_DRIVE at its east) into A
    put([0, 1, 0], "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]");
    // the RS NOR latch: A, its wall torch west, dust into B; B's wall torch north, the return dust at z
    // -2 and down into A from the north
    put([-1, 1, 0], "minecraft:stone");
    put([-2, 1, 0], "minecraft:redstone_wall_torch[facing=west,lit=true]");
    put([-3, 1, 0], DUST_EW);
    put([-4, 1, 0], "minecraft:stone");
    put([-4, 1, -1], "minecraft:redstone_wall_torch[facing=north,lit=true]");
    for x in [-4, -3, -2] {
        put([x, 1, -2], DUST_EW);
    }
    put([-1, 1, -2], DUST_NS);
    put([-1, 1, -1], DUST_NS);
    // the reset repeater (input RESET_DRIVE at its south) into B
    put([-4, 1, 1], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    // the driver: a repeater reading A (input at its north), pointing into the junction rail's cell
    put(DRIVER, "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]");
    // the T-junction: the junction cell, arms west and east (two rails each), the stem south
    put(JUNCTION, "minecraft:rail[shape=north_south,waterlogged=false]");
    for p in [[-2, 1, 2], [-3, 1, 2], [0, 1, 2], [1, 1, 2]] {
        put(p, "minecraft:rail[shape=east_west,waterlogged=false]");
    }
    for p in [[-1, 1, 3], [-1, 1, 4]] {
        put(p, "minecraft:rail[shape=north_south,waterlogged=false]");
    }
    let cells: Vec<P> = d.blocks.keys().copied().collect();
    for p in cells {
        d.blocks.entry([p[0], 0, p[2]]).or_insert_with(|| "minecraft:stone".to_string());
    }
    for p in [SET_DRIVE, RESET_DRIVE, PORT] {
        d.blocks.entry([p[0], 0, p[2]]).or_insert_with(|| "minecraft:stone".to_string());
    }
    // (the mob-farm kind's output port must be an air cell: the judge puts its chest there)
    d.ports.insert("output".into(), PORT);
    d
}

/// The driver's own blocks (floors aside): the set and reset repeaters, the latch (2 blocks, 2 torches,
/// 1 + 5 dust), the driver repeater.
pub fn piece_blocks() -> usize {
    2 + 10 + 1
}

/// (the chain fix) The control's design: the driver repeater removed (glass on its floor's cell instead), so the
/// junction never switches, which the one-change-per-phase bar must catch.
pub fn control_design() -> Design {
    let mut d = design();
    d.blocks.insert(DRIVER, "minecraft:glass".to_string());
    d.name = "junction-driver-control-no-driver".into();
    d
}

struct JOut {
    sw: Vec<i64>,
    rel: Vec<i64>,
    bad: Vec<String>,
    shapes: (String, String),
    overrides: Vec<(String, String, String)>,
    phases: usize,
}

/// At every phase 0..19, the latch reset, then a 6-gt set edge (the routing event): the junction rail's shape must
/// change once, `switch` gt after the edge; then a 6-gt reset edge: the shape must return, `release` gt after it.
fn run_bench(spec: &FarmSpec) -> Result<JOut, String> {
    let horizon = 20 * 200 + 400;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, horizon as u32 + 20, WalkMode::Random) };
    let mut run = Run::start(&crate::mirror::checked(spec)?, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let all = redsim::world::flags::ALL;
    let bp = |p: P| crate::mirror::at(p);
    let shape = |run: &Run| run.world.get(bp(JUNCTION)).to_string();
    let mut t = 0i64;
    let tick = |run: &mut Run, t: &mut i64| {
        run.tick();
        *t += 1;
    };
    let mut overrides: Vec<(String, String, String)> = Vec::new();
    let (mut sw, mut rel) = (Vec::new(), Vec::new());
    let mut bad = Vec::new();
    let mut shapes = (String::new(), String::new());
    let mut phases = 0usize;
    for off in 0..20i64 {
        // (the latch reset: the reset drive's 6-gt pulse)
        let before = run.world.get(bp(RESET_DRIVE)).to_string();
        run.world.set_block(bp(RESET_DRIVE), on.clone(), all);
        if off == 0 {
            overrides.push(("the reset drive (-4,1,2)".into(), before, run.world.get(bp(RESET_DRIVE)).to_string()));
        }
        for _ in 0..6 {
            tick(&mut run, &mut t);
        }
        run.world.set_block(bp(RESET_DRIVE), air.clone(), all);
        for _ in 0..40 {
            tick(&mut run, &mut t);
        }
        while t.rem_euclid(20) != off {
            tick(&mut run, &mut t);
        }
        phases += 1;
        let idle = shape(&run);
        // the set edge
        let t0 = t;
        let before = run.world.get(bp(SET_DRIVE)).to_string();
        run.world.set_block(bp(SET_DRIVE), on.clone(), all);
        if off == 0 {
            overrides.push(("the set drive (1,1,0)".into(), before, run.world.get(bp(SET_DRIVE)).to_string()));
        }
        let mut changes: Vec<(i64, String)> = Vec::new();
        let mut last = idle.clone();
        for k in 0..60 {
            if k == 6 {
                run.world.set_block(bp(SET_DRIVE), air.clone(), all);
            }
            tick(&mut run, &mut t);
            let s = shape(&run);
            if s != last {
                changes.push((t - t0, s.clone()));
                last = s;
            }
        }
        let forced = last.clone();
        match changes.as_slice() {
            [(d, _)] => sw.push(*d),
            other => bad.push(format!("set at phase {off}: shape changes {:?}", other.iter().map(|c| c.0).collect::<Vec<_>>())),
        }
        // the reset edge
        let t1 = t;
        run.world.set_block(bp(RESET_DRIVE), on.clone(), all);
        let mut changes: Vec<i64> = Vec::new();
        let mut last = forced.clone();
        for k in 0..60 {
            if k == 6 {
                run.world.set_block(bp(RESET_DRIVE), air.clone(), all);
            }
            tick(&mut run, &mut t);
            let s = shape(&run);
            if s != last {
                changes.push(t - t1);
                last = s;
            }
        }
        if changes.len() == 1 && last == idle {
            rel.push(changes[0]);
        } else {
            bad.push(format!("reset at phase {off}: shape changes {changes:?}, back to idle {}", last == idle));
        }
        shapes = (idle, forced);
    }
    Ok(JOut { sw, rel, bad, shapes, overrides, phases })
}

/// The bench: the design's run judged through `v` (every phase one switch and one return, the worst <= the bar), and
/// the control (the driver removed) through the same run, which must break the one-change bar.
pub fn bench(spec: &FarmSpec, control: &FarmSpec, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    let o = run_bench(spec)?;
    let span = |v: &[i64]| -> String {
        match (v.iter().min(), v.iter().max()) {
            (Some(a), Some(b)) if a == b => a.to_string(),
            (Some(a), Some(b)) => format!("{a}..{b}"),
            _ => "-".into(),
        }
    };
    let ov: Vec<String> = o.overrides.iter().map(|(w, b, a)| v.over(w, b, a)).collect();
    let once = v.bar_n("one switch and one return at every phase", o.phases, o.bad.is_empty());
    let worst = o.sw.iter().chain(o.rel.iter()).max().copied();
    let bar = v.bar_n(&format!("the worst switch / return <= {BAR} gt"), o.sw.len() + o.rel.len(), o.bad.is_empty() && worst.is_some_and(|w| w <= BAR));
    let c = run_bench(control)?;
    let ctl = v.control("the junction without its driver repeater never switches (the one-change bar catches it)", c.phases, !c.bad.is_empty());
    Ok(vec![
        format!(
            "J/K junction driver ({} blocks: the set and reset repeaters, the RS NOR napper-present latch, one driver repeater into the junction rail's cell): at {} phases the junction switches {} gt after the routing edge and returns {} gt after the reset edge ({once}){}; the worst is {} gt against the bar <= {BAR} (s 12's least junction gap {LEAST_GAP} - ironspec 05:35's +{RULE_PLUS} rule; its 1-gt allowance dropped, ironspec 20:05: {bar})",
            piece_blocks(),
            o.phases,
            span(&o.sw),
            span(&o.rel),
            if o.bad.is_empty() { String::new() } else { format!(" (the misses: {})", o.bad.join("; ")) },
            worst.map_or("-".into(), |w| w.to_string()),
        ),
        format!("  the junction rail: idle {} -> forced {}; the overrides: {}", o.shapes.0.replace("minecraft:", ""), o.shapes.1.replace("minecraft:", ""), ov.join("; ")),
        format!("  CONTROL (the chain fix: the driver repeater removed): {} of {} phases miss the one-change bar ({ctl})", c.bad.len().min(c.phases), c.phases),
    ])
}
