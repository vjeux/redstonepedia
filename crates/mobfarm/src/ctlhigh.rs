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
    /// (G1's audit, defect 2) The AND's side cell's measured power at the change (the dust's power, or 15 for the
    /// block on the side cell), and the AND's rear input (E_H's output) at its peak after the change.
    side_power: i32,
    rear_peak: i32,
    /// The overrides: the level line's source move and IN_WIN's drive, each before -> after.
    overrides: Vec<(String, String, String)>,
}

/// One trial: the level at `from` for 60 gt (settled), then `to` at t = 0; IN_WIN set (true: inside)
/// over [win_from, win_to) relative to the change (None: never inside), reaching the AND's side at
/// `win_level` (15: a repeater's or a block's; less: a weak OR's dust, G1 on 6b9be93ea point 3); H_LOAD's
/// pulses recorded for 60 gt.
fn trial(spec: &FarmSpec, from: i32, to: i32, win: Option<(i64, i64)>, win_level: i32) -> Result<Trial, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&crate::mirror::checked(spec)?, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let dust = Block::parse("minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]").map_err(|e| e.to_string())?;
    let dust_ew = Block::parse("minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let all = redsim::world::flags::ALL;
    let bp = |p: P| crate::mirror::at(p);
    let mut overrides: Vec<(String, String, String)> = Vec::new();
    let set_level = |run: &mut Run, prev: i32, s: i32, ov: &mut Vec<(String, String, String)>| {
        if let Some(p) = src_cell(prev) {
            run.world.set_block(bp(p), dust.clone(), all);
        }
        if let Some(p) = src_cell(s) {
            let before = run.world.get(bp(p)).to_string();
            run.world.set_block(bp(p), on.clone(), all);
            ov.push((format!("the level line's source for {s} at ({},{},{})", p[0], p[1], p[2]), before, run.world.get(bp(p)).to_string()));
        }
    };
    // (IN_WIN below 15: a dust line east from the AND's side cell, the source block 16 - level cells on (G1's audit,
    // defect 2: the dust touching the source reads 15 and each cell toward the side loses one, so the side cell reads
    // 16 - (16 - level) = level); at 15 the block sits on the side cell itself)
    let win_src: P = if win_level >= 15 { IN_WIN } else { [IN_WIN[0] + (16 - win_level), IN_WIN[1], IN_WIN[2]] };
    if win_level < 15 {
        for x in IN_WIN[0]..win_src[0] {
            run.world.set_block(bp([x, IN_WIN[1], IN_WIN[2]]), dust_ew.clone(), all);
        }
    }
    set_level(&mut run, 0, from, &mut overrides);
    let mut win_on = false;
    let mut pulses: Vec<(i64, i64)> = Vec::new();
    let mut was = false;
    let (mut side_power, mut rear_peak) = (-1i32, 0i32);
    let power_of = |run: &Run, p: P| -> i32 {
        let s = run.world.get(bp(p)).to_string();
        if s.contains("redstone_block") {
            15
        } else {
            s.split("power=").nth(1).and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse().ok()).unwrap_or(0)
        }
    };
    for t in -60i64..60 {
        if t == 0 {
            set_level(&mut run, from, to, &mut overrides);
            side_power = power_of(&run, IN_WIN);
        }
        let want = win.is_some_and(|(a, b)| t >= a && t < b);
        if want != win_on {
            let off = if win_level >= 15 { air.clone() } else { dust_ew.clone() };
            let before = run.world.get(bp(win_src)).to_string();
            run.world.set_block(bp(win_src), if want { on.clone() } else { off }, all);
            if overrides.iter().filter(|o| o.0.starts_with("IN_WIN")).count() < 2 {
                overrides.push((format!("IN_WIN's drive at ({},{},{}) {}", win_src[0], win_src[1], win_src[2], if want { "on" } else { "off" }), before, run.world.get(bp(win_src)).to_string()));
            }
            win_on = want;
        }
        run.tick();
        let now = run.world.best_neighbor_signal(bp(H_LOAD)) > 0;
        if t >= 0 {
            // (the AND's rear input: E_H's output signal into the AND, read from E_H's own cell toward the AND, never the
            // AND's strongest input (G1's Y11: that read the side too); E_H sits on the AND's south (+z), north under the
            // z mirror)
            let rear_dir = if crate::mirror::on() { redsim::pos::Dir::North } else { redsim::pos::Dir::South };
            rear_peak = rear_peak.max(run.world.signal(bp(EH), rear_dir).min(15).max(0));
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
    Ok(Trial { pulses, side_power, rear_peak, overrides })
}

/// (the chain audit, hole 25) The load request's delay and width after a 14 -> 15 rise outside the window (the counter
/// model's H lag must equal it); None: no request.
pub fn measured_lag(spec: &FarmSpec) -> Result<Option<i64>, String> {
    let tr = trial(spec, 14, 15, None, 15)?;
    Ok(tr.pulses.first().map(|p| p.0))
}

/// The bench: the H rise (14 -> 15, and the weather's 13 -> 15 jump) and fall (15 -> 14) with IN_WIN low
/// (outside: a load request) and high (inside: none); IN_WIN's edge at every offset -12..12 around the
/// rise (the request whole, cut, or none: the counter's pending latch takes any pulse); the request's
/// delay after the level change (the model's H lag). (The chain fix) every bar through `v`.
pub fn bench(spec: &FarmSpec, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    let fmt = |v: &[(i64, i64)]| if v.is_empty() { "none".to_string() } else { v.iter().map(|(s, w)| format!("+{s} for {}", if *w == i64::MAX { "open".to_string() } else { w.to_string() })).collect::<Vec<_>>().join(", ") };
    let mut lines = Vec::new();
    let mut rows = Vec::new();
    let mut all_right = true;
    let mut ov_lines: Vec<String> = Vec::new();
    for (label, from, to, win, want) in [
        ("rise 14 -> 15, outside the window", 14, 15, None, true),
        ("rise 13 -> 15 (a weather jump), outside", 13, 15, None, true),
        ("rise 14 -> 15, inside the window", 14, 15, Some((-100, 100)), false),
        ("fall 15 -> 14, outside", 15, 14, None, false),
    ] {
        let tr = trial(spec, from, to, win, 15)?;
        let fired = !tr.pulses.is_empty();
        all_right &= fired == want;
        for (w, b, a) in &tr.overrides {
            ov_lines.push(v.over(&format!("{label}: {w}"), b, a));
        }
        rows.push(format!("{label}: {} ({})", fmt(&tr.pulses), if fired == want { "as designed" } else { "WRONG" }));
    }
    let cases = v.bar("the rise / jump / inside / fall cases", all_right);
    // (G1's Y12 on f21c9f600: "no level change fires nothing" is a BAR (a dead circuit passes it); its control plants a
    // level detector (E_H's side delay-4 repeater (-1,0,-2) removed, so E_H passes HI's level, not its edge): a steady 15
    // must then fire, through the same trial)
    let ctl_tr = trial(spec, 14, 14, None, 15)?;
    let still_hi = trial(spec, 15, 15, None, 15)?;
    let nochg = v.bar("no level change (14 -> 14, 15 -> 15) fires nothing", ctl_tr.pulses.is_empty() && still_hi.pulses.is_empty());
    let mut lvl = spec.clone();
    // (the design's own frame: the spec is mirrored inside the trial, so the cell is the unmirrored one)
    let side = redsim::pos::BlockPos::new(-1, 0, -2);
    let before = lvl.blocks.len();
    lvl.blocks.retain(|(p, _)| *p != side);
    let removed = before - lvl.blocks.len();
    // (a level detector passes HI's level: the request on a rise never ends while HI stays high, where the edge
    // detector's ends after its pulse)
    let lvl_tr = trial(&lvl, 14, 15, None, 15)?;
    let ctl = format!(
        "{nochg}; CONTROL (E_H's side repeater removed: {removed} block, a level detector): the rise 14 -> 15 requests {} ({})",
        fmt(&lvl_tr.pulses),
        v.control("a level detector (E_H's side repeater removed) holds its request open while HI stays high", removed, lvl_tr.pulses.iter().any(|&(_, w)| w == i64::MAX))
    );
    let base = trial(spec, 14, 15, None, 15)?;
    // (G1's checklist (4): the lag is read strictly; a run without the request is NOT JUDGED)
    let first = base.pulses.first().copied();
    let lag_word = v.bar_n("the load request's delay and width measured", usize::from(first.is_some()), first.is_some_and(|(_, w)| w != i64::MAX && w > 0));
    let (lag, width) = first.unwrap_or((i64::MIN, i64::MIN));
    // IN_WIN's edge around the rise: inside until `k` (the window closing at the rise), and inside from `k`
    let mut cut: Vec<String> = Vec::new();
    let (mut a_req, mut b_req) = (Vec::new(), Vec::new());
    // (every request in the edge rows: one pulse at most, at least 2 gt wide, so the pending latch takes it)
    let (mut cut_ok, mut cut_n) = (true, 0usize);
    for k in -12i64..=12 {
        let a = trial(spec, 14, 15, Some((-100, k)), 15)?;
        let b = trial(spec, 14, 15, Some((k, 100)), 15)?;
        a_req.push((k, !a.pulses.is_empty()));
        b_req.push((k, !b.pulses.is_empty()));
        for p in [&a.pulses, &b.pulses] {
            cut_n += p.len();
            cut_ok &= p.len() <= 1 && p.iter().all(|&(_, w)| w != i64::MAX && w >= 2);
        }
        cut.push(format!("{k}: {} / {}", fmt(&a.pulses), fmt(&b.pulses)));
    }
    // (the window's edges judged: closing at k, the request survives while k is early enough for the AND's side to
    // let a pulse of >= 2 gt through before the pulse's end; opening at k, it survives while the side's block lands
    // after the pulse's start: one transition each (never a request again after it stops, or none again after it
    // starts), and every surviving request a single pulse the pending latch takes; the flips' positions printed against
    // the measured pulse (the side's own latency moves them))
    let one_flip = |v: &[(i64, bool)], from_on: bool| {
        let flips = v.windows(2).filter(|w| w[0].1 != w[1].1).count();
        flips == 1 && v.first().is_some_and(|x| x.1 == from_on) && v.last().is_some_and(|x| x.1 != from_on)
    };
    let a_flip = a_req.iter().find(|x| !x.1).map(|x| x.0);
    let b_flip = b_req.iter().find(|x| x.1).map(|x| x.0);
    // (G1's Y15 on f21c9f600: each flip bounded by the measured pulse (start `lag`, width `width`) and the side's latency
    // (at most 2 gt, a repeater's), never anywhere in -12..12)
    let in_pulse = |k: Option<i64>| k.is_some_and(|k| first.is_some() && k >= lag - 2 && k <= lag + width + 2);
    let edges_ok = first.is_some() && one_flip(&a_req, true) && one_flip(&b_req, false) && in_pulse(a_flip) && in_pulse(b_flip);
    let edges = v.bar("IN_WIN's edges around the rise: one transition each (closing, opening), each inside the measured pulse +- the side's 2-gt latency", edges_ok);
    let cut_word = v.bar_n("IN_WIN's edge rows: every request one pulse of >= 2 gt (the pending latch takes it)", cut_n, cut_ok);
    // (G1 on 6b9be93ea point 3, ironspec 20:55 PT (2)(4); G1's audit defect 2) IN_WIN's level at the AND's side swept
    // 1..15 inside the window at the rise, each row printing the side's measured power and the rear's peak: the AND
    // (subtract) passes rear - side, so with E_H at 15 on the rear only a side of 15 holds the request back; the weak
    // levels are the control (a side of 14 must leak).
    let mut weak: Vec<String> = Vec::new();
    let mut leak_levels: Vec<i32> = Vec::new();
    let mut measured_ok = true;
    let mut leak14 = false;
    for s in 1..=15 {
        let tr = trial(spec, 14, 15, Some((-100, 100)), s)?;
        let leaks = !tr.pulses.is_empty();
        if leaks {
            leak_levels.push(s);
        }
        if s == 14 {
            leak14 = leaks;
        }
        measured_ok &= tr.side_power == s;
        weak.push(format!("{s} (side measured {}, rear peak {}):{}", tr.side_power, tr.rear_peak, if leaks { "LEAKS" } else { "held" }));
    }
    let side_ok = v.bar("each weak-OR row's side reads its label", measured_ok);
    let split_ok = (1..=14).all(|s| leak_levels.contains(&s)) && !leak_levels.contains(&15);
    let split = v.bar("only a side of 15 holds the request (1..14 leak)", split_ok);
    let ctl14 = v.control("a measured side of 14 leaks", 1, leak14);
    lines.push(format!(
        "the high re-phase's circuit ({BLOCKS} blocks: HI compare at 15 with a redstone block on its side, its rising edge E_H (subtract, a delay-4 side), the AND (subtract, IN_WIN on the side); + the window decode (tens 5..9: an OR of five tens taps, ~6) and the load path (tens := 5, the FF to u 9, ~4) in box piece 2): {}: {cases}; the no-change control: {} ({ctl})",
        rows.join("; "),
        fmt(&ctl_tr.pulses)
    ));
    lines.push(format!(
        "  the load request H_LOAD after the level's change: {} ({lag_word}; the counter model's H lag: a request due at a detector update reaches the pending latch that much later)",
        if first.is_some() { format!("+{lag} gt for {width} gt") } else { "NONE".to_string() }
    ));
    lines.push(format!(
        "  IN_WIN's edge at offset k from the rise (inside before k / inside from k: the request's pulses): {}; the closing edge's last request at k {} and the opening edge's first at k {} against the pulse's +{lag}..+{} (one transition each: {edges}; every request one pulse of >= 2 gt: {cut_word})",
        cut.join("; "),
        a_flip.map_or("-".to_string(), |k| (k - 1).to_string()),
        b_flip.map_or("-".to_string(), |k| k.to_string()),
        if first.is_some() { (lag + width).to_string() } else { "-".to_string() }
    ));
    lines.push(format!(
        "  IN_WIN's level at the AND's side, inside the window at the rise (level (measured side, rear peak):request): {} ({side_ok}); the split: {split}; the control, a side of 14: {ctl14}; the requirement: the window decode's OR reaches the AND's side at 15 (a repeater gives 15)",
        weak.join(" ")
    ));
    lines.push(format!("  the overrides (each target's state before -> after): {}", ov_lines.join("; ")));
    Ok(lines)
}
