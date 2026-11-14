//! THE CORE's REL loop (dfb89499; day-1's netlist subsystem 1 after the compactness cut, ironspec 14:41c / 14:47c /
//! 15:22c): REL is the slot of a 100-gt RE-PHASE LOOP of repeaters, its taps REL + k. The pad's golem pulse (the
//! tripwire rows at y -4, mod 100) is INJECTED into the loop at the position the locked REL's own pulse passes at that
//! tick, so a golem re-locks REL to the summon (a golem one gate late lands in phase: mod 100). A comparator in
//! subtract mode on the loop is the GATE: its side is an RS latch's output, SET by REL itself and RESET by the loop's
//! tap at +87, so every REL is cut to its set time (6 gt) and a second pulse within the inhibit (up to +95) dies:
//! RELs stay >= 95 gt apart, and an injected pulse up to 5 gt ahead of a lagging loop wins.
//!
//! Layout (local frame, components at y 1 on glass at y 0): the loop is a rectangle, S (REL) at (-2,1,2), the gate
//! C at (-1,1,2) (rear (0,1,2)), the latch (day-1's ctljunction latch, mirrored) at x -1..2, z -2..1; the loop's
//! cells run S -> x -2 south to z 9 -> z 9 east to x 9 -> up x 9 -> z 7 west to x 0 -> x 0 north to (0,1,2).

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};

type P = [i32; 3];

/// The loop's period (gt): the summon gate's (game time % 100).
pub const PERIOD: i64 = 100;
/// The golem line's latency in the bench (the hook's rise to the injection dust): the injection node sits at loop
/// position GOLEM_LAT - 4, so the locked REL is the summon tick + 5 (the station's REL = T + 5).
pub const GOLEM_LAT: i64 = 12;
/// The locked REL's offset after the summon tick.
pub const LOCK: i64 = 5;
/// REL (the gate's output dust).
pub const S: P = [-2, 1, 2];
/// The gate comparator.
pub const GATE: P = [-1, 1, 2];
/// The injection node and the golem line's end (the bench sets a redstone block there).
pub const INJ: P = [-2, 1, 5];
pub const GOLEM_IN: P = [-3, 1, 5];
/// The latch's A block (powered = the inhibit on).
pub const LATCH_A: P = [-1, 1, 0];

/// One loop element, in travel order after S.
#[derive(Clone, Copy, Debug, PartialEq)]
enum El {
    Rep(i64),
    Node,
}

fn dir_name(d: [i32; 2]) -> &'static str {
    match d {
        [1, 0] => "east",
        [-1, 0] => "west",
        [0, 1] => "south",
        [0, -1] => "north",
        _ => panic!("not a unit step: {d:?}"),
    }
}

/// The loop's path after S, in travel order: (cell, is a corner).
fn path() -> Vec<([i32; 2], bool)> {
    let mut v = Vec::new();
    for z in 3..=8 {
        v.push(([-2, z], false));
    }
    v.push(([-2, 9], true));
    for x in -1..=8 {
        v.push(([x, 9], false));
    }
    v.push(([9, 9], true));
    v.push(([9, 8], false));
    v.push(([9, 7], true));
    for x in (1..=8).rev() {
        v.push(([x, 7], false));
    }
    v.push(([0, 7], true));
    for z in (3..=6).rev() {
        v.push(([0, z], false));
    }
    v.push(([0, 2], true));
    v
}

/// The elements on the path's non-corner slots, in order (27 slots): the injection node at +8, the reset tap's node
/// at +87, the corner before the gate at +98.
fn elements() -> Vec<El> {
    use El::*;
    let mut v = vec![Rep(4), Rep(4), Node, Rep(4), Rep(4), Rep(4)];
    v.extend(std::iter::repeat_n(Rep(4), 10));
    v.push(Rep(4));
    v.extend([Rep(4), Rep(4), Rep(4), Rep(4), Rep(4), Rep(1), Rep(1), Rep(1)]);
    v.extend([Node, Rep(4), Rep(4), Rep(3)]);
    v
}

/// The loop's cells with each one's position after S (gt), and the reset tap node.
fn loop_cells() -> Result<(Vec<(P, String, i64)>, P), String> {
    let p = path();
    let els = elements();
    let mut out = Vec::new();
    let (mut t, mut k) = (0i64, 0usize);
    let mut prev = [S[0], S[2]];
    let mut tap = None;
    for (c, corner) in p {
        let back = [prev[0] - c[0], prev[1] - c[1]];
        let cell = [c[0], 1, c[1]];
        if corner {
            out.push((cell, "dust".to_string(), t));
        } else {
            let e = *els.get(k).ok_or("more path slots than elements")?;
            k += 1;
            match e {
                El::Rep(d) => {
                    t += d;
                    out.push((cell, format!("minecraft:repeater[delay={d},facing={},locked=false,powered=false]", dir_name(back)), t));
                }
                El::Node => {
                    if t == 87 {
                        tap = Some(cell);
                    }
                    out.push((cell, "dust".to_string(), t));
                }
            }
        }
        prev = c;
    }
    if k != els.len() {
        return Err(format!("{} elements left over", els.len() - k));
    }
    if t != PERIOD - 2 {
        return Err(format!("the loop reaches the gate at +{t}, not +{}", PERIOD - 2));
    }
    Ok((out, tap.ok_or("no reset tap node at +87")?))
}

/// The dust states: each dust cell connected (side) to every neighbour that is dust, a diode on that axis, a torch or
/// a redstone block; a single connection extends through (the game's line).
fn wire(cells: &mut std::collections::BTreeMap<P, String>) {
    let dirs: [([i32; 3], &str, &str); 4] = [([0, 0, -1], "north", "south"), ([0, 0, 1], "south", "north"), ([1, 0, 0], "east", "west"), ([-1, 0, 0], "west", "east")];
    let snapshot = cells.clone();
    for (p, s) in cells.iter_mut() {
        if s != "dust" {
            continue;
        }
        let mut on: Vec<&str> = Vec::new();
        for (d, name, _) in dirs {
            let q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            let Some(n) = snapshot.get(&q) else { continue };
            let axis = |f: &str| f.contains(&format!("facing={name}")) || f.contains(&format!("facing={}", match name {
                "north" => "south",
                "south" => "north",
                "east" => "west",
                _ => "east",
            }));
            let connects = n == "dust" || ((n.contains("repeater[") || n.contains("comparator[")) && axis(n)) || n.contains("redstone_torch") || n.contains("redstone_wall_torch") || n.contains("redstone_block");
            if connects {
                on.push(name);
            }
        }
        if on.len() == 1 {
            let opp = match on[0] {
                "north" => "south",
                "south" => "north",
                "east" => "west",
                _ => "east",
            };
            on.push(opp);
        }
        let st = |n: &str| if on.contains(&n) { "side" } else { "none" };
        *s = format!("minecraft:redstone_wire[east={},north={},power=0,south={},west={}]", st("east"), st("north"), st("south"), st("west"));
    }
}

/// The prototype loop's design (the gate, the inhibit latch, the injection node, the reset tap and its line).
pub fn design() -> Result<Design, String> {
    let mut cells: std::collections::BTreeMap<P, String> = std::collections::BTreeMap::new();
    let (lp, tap) = loop_cells()?;
    for (p, s, _) in &lp {
        cells.insert(*p, s.clone());
    }
    // the gate: subtract mode, rear the loop's corner (0,1,2), out into S, its north side the latch's reader
    cells.insert(GATE, "minecraft:comparator[facing=east,mode=subtract,powered=false]".into());
    cells.insert(S, "dust".into());
    // the latch's SET: S -> (-3,1,2) -> (-3,1,1) -> (-3,1,0) -> a delay-2 repeater into A
    for p in [[-3, 1, 2], [-3, 1, 1], [-3, 1, 0]] {
        cells.insert(p, "dust".into());
    }
    cells.insert([-2, 1, 0], "minecraft:repeater[delay=2,facing=west,locked=false,powered=false]".into());
    // the RS NOR latch (day-1's ctljunction latch, mirrored in x): A, its torch east, dust into B; B's torch north,
    // the return dust at z -2 and down into A from the north
    cells.insert(LATCH_A, "minecraft:stone".into());
    cells.insert([0, 1, 0], "minecraft:redstone_wall_torch[facing=east,lit=true]".into());
    cells.insert([1, 1, 0], "dust".into());
    cells.insert([2, 1, 0], "minecraft:stone".into());
    cells.insert([2, 1, -1], "minecraft:redstone_wall_torch[facing=north,lit=false]".into());
    for p in [[2, 1, -2], [1, 1, -2], [0, 1, -2], [-1, 1, -2], [-1, 1, -1]] {
        cells.insert(p, "dust".into());
    }
    // the reader: a delay-2 repeater reading A, out into the gate's north side
    cells.insert([-1, 1, 1], "minecraft:repeater[delay=2,facing=north,locked=false,powered=false]".into());
    // the RESET: the tap repeater beside the +87 node, the line north along x 2, a repeater into B
    let [tx, _, tz] = tap;
    cells.insert([tx + 1, 1, tz], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]".into());
    for z in 2..=tz {
        cells.insert([tx + 2, 1, z], "dust".into());
    }
    cells.insert([tx + 2, 1, 1], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]".into());
    wire(&mut cells);
    let mut d = Design { name: "ctl-core-rel-loop".into(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let keys: Vec<P> = cells.keys().copied().collect();
    for (p, s) in cells {
        d.blocks.insert(p, s);
    }
    for p in keys {
        let s = d.blocks[&p].clone();
        if s.contains("redstone_wire") || s.contains("repeater") || s.contains("comparator") {
            d.blocks.insert([p[0], 0, p[2]], "minecraft:glass".into());
        }
    }
    d.blocks.insert([GOLEM_IN[0], 0, GOLEM_IN[2]], "minecraft:glass".into());
    // (the mob-farm kind's output port: an air cell on a floor inside the design's box, as ctljunction's)
    d.blocks.insert([12, 0, 12], "minecraft:glass".into());
    d.ports.insert("output".into(), [12, 1, 12]);
    Ok(d)
}

/// One scenario: the loop's first pulse injected at `start` (a 4-gt block at the golem input), the golem pulses (10 gt
/// each, the hook's) arriving at summon + 1 + GOLEM_LAT for each summon in `golems`, the core frozen over each of
/// `freezes` (game time runs on; a golem due inside a freeze is lost), the run to `horizon`. Returns the REL rises and
/// widths (game time).
fn scenario(spec: &FarmSpec, starts: &[i64], golems: &[i64], freezes: &[(i64, i64)], horizon: i64) -> Result<Vec<(i64, i64)>, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, horizon as u32 + 20, WalkMode::Random) };
    let mut run = Run::start(&crate::mirror::checked(spec)?, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let all = redsim::world::flags::ALL;
    let bp = crate::mirror::at;
    let mut drive: Vec<(i64, i64)> = starts.iter().map(|&s| (s, s + 4)).collect();
    for &g in golems {
        let a = g + 1 + GOLEM_LAT;
        if !freezes.iter().any(|&(f, n)| a >= f && a < f + n) {
            drive.push((a, a + 10));
        }
    }
    let mut rels: Vec<(i64, i64)> = Vec::new();
    let mut high: Option<i64> = None;
    let mut driven = false;
    for t in 0..horizon {
        if freezes.iter().any(|&(f, n)| t >= f && t < f + n) {
            continue;
        }
        let want = drive.iter().any(|&(a, b)| t >= a && t < b);
        if want != driven {
            run.world.set_block(bp(GOLEM_IN), if want { on.clone() } else { air.clone() }, all);
            driven = want;
        }
        run.tick();
        let s_on = !run.world.get(bp(S)).to_string().contains("power=0");
        match (s_on, high) {
            (true, None) => high = Some(t + 1),
            (false, Some(r)) => {
                rels.push((r, t + 1 - r));
                high = None;
            }
            _ => {}
        }
    }
    Ok(rels)
}

/// The bench: the loop's period and REL's width free-running; a lag of N gt (1..10) under golems every cycle (re-locks
/// within one cycle for N <= 5, the inhibit's reach); a lead; freezes of 1..3 gt at every 10th phase with the open's
/// lost gate; a missing golem moving no phase; two first pulses leaving one REL. Judged through `v`.
pub fn bench(spec: &FarmSpec, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let intervals = |r: &[(i64, i64)]| -> Vec<i64> { r.windows(2).map(|w| w[1].0 - w[0].0).collect() };
    let span = |x: &[i64]| -> String {
        match (x.iter().min(), x.iter().max()) {
            (Some(a), Some(b)) if a == b => a.to_string(),
            (Some(a), Some(b)) => format!("{a}..{b}"),
            _ => "-".into(),
        }
    };
    // (the summon ticks: game time % 100 == 0, from 1000)
    let g0 = 1000i64;
    let horizon = g0 + 1200;
    // the loop's first pulse lands REL at g0 - 100 + LOCK + lag: it enters at the injection node (+8)
    let start_for = |lag: i64| g0 - 100 + LOCK + lag - (PERIOD - 8);
    // 1. free-running
    let r = scenario(spec, &[start_for(0)], &[], &[], horizon)?;
    let iv = intervals(&r);
    let widths: Vec<i64> = r.iter().map(|x| x.1).collect();
    let ok = r.len() >= 10 && iv.iter().all(|&i| i == PERIOD);
    lines.push(format!("free-running: {} RELs, intervals {} (bar {PERIOD}), REL width {} gt: {}", r.len(), span(&iv), span(&widths), v.bar_n("the free-running loop's period is 100 gt", iv.len(), ok)));
    // 2. lags under golems every cycle
    let golems: Vec<i64> = (0..12).map(|k| g0 + 100 * k).collect();
    let mut lag_rows = Vec::new();
    let mut lag_ok = true;
    for lag in 1..=10i64 {
        let r = scenario(spec, &[start_for(lag)], &golems, &[], horizon)?;
        let iv = intervals(&r);
        let after: Vec<i64> = r.iter().map(|x| (x.0 - LOCK).rem_euclid(PERIOD)).collect();
        let locked_from = after.iter().position(|&ph| ph == 0);
        let stays = locked_from.is_some_and(|i| after[i..].iter().all(|&ph| ph == 0));
        let least = iv.iter().min().copied();
        if lag <= 5 && !(stays && least.is_some_and(|l| l >= 95)) {
            lag_ok = false;
        }
        lag_rows.push(format!("lag {lag}: locked from REL #{} {}, least interval {}", locked_from.map_or("-".into(), |i| i.to_string()), if stays { "for good" } else { "NOT" }, least.map_or("-".into(), |l| l.to_string())));
    }
    lines.push(format!("lags under a golem every cycle (the golem pulse at summon + 1 + {GOLEM_LAT}, injected at +8): {}: {}", lag_rows.join("; "), v.bar("a lag of 1..5 gt re-locks within one cycle with every interval >= 95", lag_ok)));
    // 3. a lead (never made by an open: printed)
    let mut lead_rows = Vec::new();
    for lead in 1..=3i64 {
        let r = scenario(spec, &[start_for(-lead)], &golems, &[], horizon)?;
        let after: Vec<i64> = r.iter().map(|x| (x.0 - LOCK).rem_euclid(PERIOD)).collect();
        lead_rows.push(format!("lead {lead}: phases {}", after.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",")));
    }
    lines.push(format!("leads (printed, not judged: an open only lags): {}", lead_rows.join("; ")));
    // 4. freezes (the open's idle ticks) with the open's lost gate
    let mut fz_bad = Vec::new();
    let mut fz_n = 0usize;
    for f in 1..=3i64 {
        for ph in (0..100).step_by(10) {
            let at = g0 + 300 + ph;
            // (tick-exact 14:27: an open costs that cycle's summon gate: the next summon tick after the open is lost)
            let lost = (at / 100 + 1) * 100;
            let gs: Vec<i64> = golems.iter().copied().filter(|&g| g != lost).collect();
            let r = scenario(spec, &[start_for(0)], &gs, &[(at, f)], horizon)?;
            fz_n += 1;
            let iv = intervals(&r);
            let tail: Vec<i64> = r.iter().filter(|x| x.0 > at + 300).map(|x| (x.0 - LOCK).rem_euclid(PERIOD)).collect();
            if tail.is_empty() || tail.iter().any(|&p| p != 0) || iv.iter().any(|&i| i < 95) {
                fz_bad.push(format!("freeze {f} at +{ph}: tail phases {tail:?}, least interval {:?}", iv.iter().min()));
            }
        }
    }
    lines.push(format!("freezes of 1..3 gt at every 10th phase, the open's next gate lost: {} runs, {} not re-locked within 3 cycles{}: {}", fz_n, fz_bad.len(), if fz_bad.is_empty() { String::new() } else { format!(" ({})", fz_bad.join("; ")) }, v.bar_n("every freeze re-locks within 3 cycles, every interval >= 95", fz_n, fz_bad.is_empty())));
    // 5. missing golems move no phase
    let gs: Vec<i64> = golems.iter().copied().filter(|&g| g != g0 + 300 && g != g0 + 400 && g != g0 + 500).collect();
    let r = scenario(spec, &[start_for(0)], &gs, &[], horizon)?;
    let ph: Vec<i64> = r.iter().filter(|x| x.0 > g0).map(|x| (x.0 - LOCK).rem_euclid(PERIOD)).collect();
    lines.push(format!("three missing golems: phases after the first summon {}: {}", ph.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(","), v.bar_n("missing golems move no phase", ph.len(), ph.iter().all(|&p| p == 0))));
    // 6. two first pulses (a double start) leave one REL a cycle
    let r = scenario(spec, &[start_for(0), start_for(0) + 37], &[], &[], horizon)?;
    let iv = intervals(&r);
    lines.push(format!("a double start (two first pulses 37 gt apart): intervals {}: {}", span(&iv), v.bar_n("a double start leaves one REL a cycle (every interval 100 after the first)", iv.len(), iv.iter().skip(1).all(|&i| i == PERIOD))));
    Ok(lines)
}
