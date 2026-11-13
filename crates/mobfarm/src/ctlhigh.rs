//! The high re-phase's circuit as blocks (day-1, Nov 12 2026; ironspec 09:49 / 10:18: H = 15 adopted with
//! its stated limit, before adoption a block bench of the comparator, its rising edge, the window decode
//! and the load path at every phase, `--mirror-z`, and the counter model's H path checked against it).
//! HI (a comparator in compare mode reading the detector's line, a redstone block on its side: output only
//! at level 15) -> its rising edge E_H (subtract: HI's dust in the rear, HI through a delay-4 repeater on
//! the side: an 8-gt pulse at the rise, none at the fall) -> the AND (subtract: E_H in the rear, the window
//! decode's IN_WIN on the side: E_H passes only while C is outside the window) -> H_LOAD, the load request
//! into the counter's pending latch (tens := 5, the FF to u 9 on the D8 tap: box piece 2's load path).
//! IN_WIN is the decode of the tens digit 5..9 (an OR of five tens taps); the bench drives it as a level,
//! at 15 (the requirement: the OR reaches the AND's side through a repeater) and swept 1..15 as the weak-OR
//! control (G1 on 6b9be93ea point 3).
//! The detector's level is a 16-dust line whose end feeds HI's rear (a redstone block on the cell 16 - s
//! from the end gives the end level s; a reading change moves the block in one tick, as the drop latch's
//! bench does).

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};

type P = [i32; 3];

/// HI (compare, input from the south), its side block, its output dust.
pub const HI: P = [0, 0, 0];
const HI_SIDE: P = [1, 0, 0];
/// E_H (subtract, rear HI's dust (0,0,-1), side the delay-4 repeater (-1,0,-2)).
pub const EH: P = [0, 0, -2];
/// The AND (subtract, rear E_H, side IN_WIN driven at (1,0,-3)), its output cell H_LOAD.
pub const AND: P = [0, 0, -3];
pub const IN_WIN: P = [1, 0, -3];
pub const H_LOAD: P = [0, 0, -4];
/// The level line: dust from (0,0,1) (the end, HI's rear) to (0,0,16).
const LINE_END: i32 = 1;

pub fn design() -> Design {
    let mut d = Design { name: "high-rephase-h15".into(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let mut put = |p: P, s: &str| {
        d.blocks.insert(p, s.to_string());
    };
    let dust = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";
    let dust_ns = "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]";
    // floors (and, for the bench's weak-OR rows only, a floor east of IN_WIN for its dust line: x 2..17 at z -3)
    for x in -3..=2 {
        for z in -5..=17 {
            put([x, -1, z], "minecraft:stone");
        }
    }
    for x in 3..=17 {
        put([x, -1, -3], "minecraft:stone");
    }
    // the level line
    for z in LINE_END..=16 {
        put([0, 0, z], dust_ns);
    }
    // (a diode's `facing` points at its input, as in the game)
    put(HI, "minecraft:comparator[facing=south,mode=compare,powered=false]");
    put(HI_SIDE, "minecraft:redstone_block");
    // HI's output dust and the side branch to the delay-4 repeater
    put([0, 0, -1], dust);
    for p in [[-1, 0, -1], [-2, 0, -1], [-2, 0, -2]] {
        put(p, dust);
    }
    put([-1, 0, -2], "minecraft:repeater[delay=4,facing=west,locked=false,powered=false]");
    put(EH, "minecraft:comparator[facing=south,mode=subtract,powered=false]");
    put(AND, "minecraft:comparator[facing=south,mode=subtract,powered=false]");
    // (H_LOAD is the air cell the AND outputs into: the mob-farm loader's port is air or a chest)
    d.ports.insert("output".into(), H_LOAD);
    d
}

/// The piece's own blocks (the bench's level line, floors, IN_WIN's drive and the readout aside): HI, its
/// side block, 4 dust, the delay-4 repeater, E_H, the AND.
pub const BLOCKS: usize = 9;

/// The level line's cell for an end level `s` (None: 0, no block).
fn src_cell(s: i32) -> Option<P> {
    if s <= 0 { None } else { Some([0, 0, LINE_END + (15 - s.min(15)) + 1]) }
}

struct Trial {
    /// H_LOAD's pulses after the level change: (start offset, width).
    pulses: Vec<(i64, i64)>,
}

/// One trial: the level at `from` for 60 gt (settled), then `to` at t = 0; IN_WIN set (true: inside)
/// over [win_from, win_to) relative to the change (None: never inside), reaching the AND's side at
/// `win_level` (15: a repeater's or a block's; less: a weak OR's dust, G1 on 6b9be93ea point 3); H_LOAD's
/// pulses recorded for 60 gt.
fn trial(spec: &FarmSpec, from: i32, to: i32, win: Option<(i64, i64)>, win_level: i32) -> Result<Trial, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&crate::mirror::spec(spec)?, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let dust = Block::parse("minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]").map_err(|e| e.to_string())?;
    let dust_ew = Block::parse("minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let all = redsim::world::flags::ALL;
    let bp = |p: P| crate::mirror::at(p);
    let set_level = |run: &mut Run, prev: i32, s: i32| {
        if let Some(p) = src_cell(prev) {
            run.world.set_block(bp(p), dust.clone(), all);
        }
        if let Some(p) = src_cell(s) {
            run.world.set_block(bp(p), on.clone(), all);
        }
    };
    // (IN_WIN below 15: a dust line east from the AND's side cell, the source block 15 - level cells on, so
    // the side cell's dust is at `win_level`; at 15 the block sits on the side cell itself)
    let win_src: P = if win_level >= 15 { IN_WIN } else { [IN_WIN[0] + (15 - win_level), IN_WIN[1], IN_WIN[2]] };
    if win_level < 15 {
        for x in IN_WIN[0]..win_src[0] {
            run.world.set_block(bp([x, IN_WIN[1], IN_WIN[2]]), dust_ew.clone(), all);
        }
    }
    set_level(&mut run, 0, from);
    let mut win_on = false;
    let mut pulses: Vec<(i64, i64)> = Vec::new();
    let mut was = false;
    for t in -60i64..60 {
        if t == 0 {
            set_level(&mut run, from, to);
        }
        let want = win.is_some_and(|(a, b)| t >= a && t < b);
        if want != win_on {
            let off = if win_level >= 15 { air.clone() } else { dust_ew.clone() };
            run.world.set_block(bp(win_src), if want { on.clone() } else { off }, all);
            win_on = want;
        }
        run.tick();
        let now = run.world.best_neighbor_signal(bp(H_LOAD)) > 0;
        if t >= 0 {
            if now && !was {
                pulses.push((t + 1, i64::MAX));
            }
            if !now && was
                && let Some(l) = pulses.last_mut()
            {
                l.1 = t + 1 - l.0;
            }
        }
        was = now;
    }
    Ok(Trial { pulses })
}

/// The bench: the H rise (14 -> 15, and the weather's 13 -> 15 jump) and fall (15 -> 14) with IN_WIN low
/// (outside: a load request) and high (inside: none); IN_WIN's edge at every offset -12..12 around the
/// rise (the request whole, cut, or none: the counter's pending latch takes any pulse); the request's
/// delay after the level change (the model's H lag).
pub fn bench(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let fmt = |v: &[(i64, i64)]| if v.is_empty() { "none".to_string() } else { v.iter().map(|(s, w)| format!("+{s} for {w}")).collect::<Vec<_>>().join(", ") };
    let mut lines = Vec::new();
    let mut rows = Vec::new();
    let mut ok = true;
    for (label, from, to, win, want) in [
        ("rise 14 -> 15, outside the window", 14, 15, None, true),
        ("rise 13 -> 15 (a weather jump), outside", 13, 15, None, true),
        ("rise 14 -> 15, inside the window", 14, 15, Some((-100, 100)), false),
        ("fall 15 -> 14, outside", 15, 14, None, false),
        ("rise 14 -> 14 (no change: the control)", 14, 14, None, false),
    ] {
        let tr = trial(spec, from, to, win, 15)?;
        let fired = !tr.pulses.is_empty();
        ok &= fired == want;
        rows.push(format!("{label}: {} ({})", fmt(&tr.pulses), if fired == want { "ok" } else { "WRONG" }));
    }
    let base = trial(spec, 14, 15, None, 15)?;
    let (lag, width) = base.pulses.first().copied().unwrap_or((-1, -1));
    // IN_WIN's edge around the rise: inside until `k` (the window closing at the rise), and inside from `k`
    let mut cut: Vec<String> = Vec::new();
    for k in -12i64..=12 {
        let a = trial(spec, 14, 15, Some((-100, k)), 15)?;
        let b = trial(spec, 14, 15, Some((k, 100)), 15)?;
        cut.push(format!("{k}: {} / {}", fmt(&a.pulses), fmt(&b.pulses)));
    }
    // (G1 on 6b9be93ea point 3, ironspec 20:55 PT (2)(4)) IN_WIN's level at the AND's side swept 1..15 inside the
    // window at the rise: the AND (subtract) passes E_H - side, so a side at E_H's own level or above holds the
    // request back and anything weaker leaks; the bench measures where the levels split (E_H reaches the AND
    // below 15, through its dust) and the built OR must reach the side at or above it (a repeater gives 15). The
    // weak levels are the control.
    let mut weak: Vec<String> = Vec::new();
    let mut leak_levels: Vec<i32> = Vec::new();
    for s in 1..=15 {
        let tr = trial(spec, 14, 15, Some((-100, 100)), s)?;
        if !tr.pulses.is_empty() {
            leak_levels.push(s);
        }
        weak.push(format!("{s}:{}", if tr.pulses.is_empty() { "held" } else { "LEAKS" }));
    }
    // (the least level that holds; the split is clean when every level below it leaks and every level from it
    // holds, with 15 holding and 1 leaking)
    let split = (1..=15).find(|s| !leak_levels.contains(s));
    let clean_split = split.is_some_and(|t| t > 1 && (1..t).all(|s| leak_levels.contains(&s)) && (t..=15).all(|s| !leak_levels.contains(&s)));
    lines.push(format!(
        "the high re-phase's circuit ({BLOCKS} blocks: HI compare at 15 with a redstone block on its side, its rising edge E_H (subtract, a delay-4 side), the AND (subtract, IN_WIN on the side); + the window decode (tens 5..9: an OR of five tens taps, ~6) and the load path (tens := 5, the FF to u 9, ~4) in box piece 2): {}; {}",
        rows.join("; "),
        if ok { "every case right" } else { "FAIL" }
    ));
    lines.push(format!(
        "  the load request H_LOAD after the level's change: +{lag} gt for {width} gt (the counter model's H lag: a request due at a detector update reaches the pending latch {lag} gt later)"
    ));
    lines.push(format!(
        "  IN_WIN's edge at offset k from the rise (inside before k / inside from k: the request's pulses): {}",
        cut.join("; ")
    ));
    lines.push(format!(
        "  IN_WIN's level at the AND's side, inside the window at the rise (level:request): {}; {}",
        weak.join(" "),
        match (split, clean_split) {
            (Some(t), true) => format!("the levels split at {t} (E_H's level at the AND): 1..{} leak, {t}..15 hold, so the weak-OR control bites; the requirement: the window decode's OR reaches the AND's side at {t} or more (a repeater gives 15)", t - 1),
            _ => "FAIL: the levels do not split cleanly (a weaker level holds or a stronger one leaks)".to_string(),
        }
    ));
    Ok(lines)
}
