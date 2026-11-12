//! The controller's units digit (day-1, Nov 12 2026; ironspec's ruling 00:14; the 8-token set from the
//! siding windows on 7b90c3f1's K re-lay, 08:35): ONE 10-dropper ring (dropper -> hopper -> dropper:
//! every transit crosses a hopper, so the firing order never matters), EIGHT tokens and TWO taps. The
//! tokens sit where they reach the main tap's dropper (D9) at u = 0, 1, 2, 4, 5, 6, 7, 8: the picks
//! 1/4/7, the gate edge 6, the sidings 8/2/5 and the reset 0. The u = 0 token is an unstackable item,
//! so a tap's comparator reads 2 there and 1 for a stackable token: the u = 0 event is identified and
//! resets the 3-bulb event counter (events 0..7 = u 0/1/2/4/5/6/7/8). The second tap reads D8, one
//! step before the main one: the unstackable there is u = 9, the FF's stop for trigger 2's load (C 19).
//! (The main tap moved from D8 to D9 so that both taps sit on the ring's outer faces: D7, the step
//! before D8, has the clock's repeater under its south face and D0 behind its north face.) The ring is
//! `mctas circuit memory ring --n 10 --unchecked`'s counter10-ring10-0 (27 blocks, 3 x 6 x 3), its
//! open hopper tops sealed with stone.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};

type P = [i32; 3];

/// The ring's droppers in flow order (D0..D9), local cells (x -1..1, y 0..1, z -5..0).
pub const DROPPERS: [P; 10] = [[0, 1, -2], [0, 1, -4], [1, 0, -4], [0, 0, -5], [-1, 0, -4], [-1, 0, -2], [-1, 0, 0], [0, 1, 0], [1, 0, 0], [1, 0, -2]];
/// The main tap: D9 and its comparator (facing west: input from D9 at (1,0,-2), output east).
pub const TAP: usize = 9;
pub const TAP_CMP: P = [2, 0, -2];
pub const TAP_OUT: P = [3, 0, -2];
/// The u = 9 tap: D8 and its comparator (facing north: input from D8 at (1,0,0), output south).
pub const TAP9: usize = 8;
pub const TAP9_CMP: P = [1, 0, 1];
pub const TAP9_OUT: P = [1, 0, 2];
/// The clock input (the ring's input cell, driven from the south).
pub const INPUT: P = [0, 0, 0];
/// The events (u phases) the tokens mark; u 0 is the unstackable.
pub const EVENTS: [i64; 8] = [0, 1, 2, 4, 5, 6, 7, 8];

/// Each event's token dropper at u = 0 (the token for event e reaches D9 at u = e).
pub fn token_droppers() -> Vec<usize> {
    EVENTS.iter().map(|&e| ((TAP as i64 + 10 - e).rem_euclid(10)) as usize).collect()
}

/// The units digit's design: the ring, the seals, the tokens, the two taps.
pub fn design() -> Design {
    let mut d = Design { name: "units-ring10-8tokens".into(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let put = |d: &mut Design, p: P, s: &str| {
        d.blocks.insert(p, s.to_string());
    };
    // (layer 0, rows z -5..0, x -1..1; layer 1 the same; layer -1 floors)
    let rows0 = [["G", "E", "I"], ["A", "S", "d"], ["G", "R", "."], ["A", "S", "B"], ["G", "r", "F"], ["B", ".", "d"]];
    let rows1 = [[".", ".", "."], [".", "D", "H"], [".", "F", "."], ["C", "d", "I"], [".", ".", "."], ["h", "D", "H"]];
    let pal = |c: &str| -> Option<&'static str> {
        Some(match c {
            "A" => "minecraft:dropper[facing=south,triggered=false]",
            "B" => "minecraft:dropper[facing=up,triggered=false]",
            "D" => "minecraft:dropper[facing=east,triggered=false]",
            "E" => "minecraft:dropper[facing=west,triggered=false]",
            "d" => "minecraft:dropper[facing=north,triggered=false]",
            "F" => "minecraft:hopper[enabled=true,facing=north]",
            "G" => "minecraft:hopper[enabled=true,facing=south]",
            "H" => "minecraft:hopper[enabled=true,facing=down]",
            "I" => "minecraft:hopper[enabled=true,facing=west]",
            "h" => "minecraft:hopper[enabled=true,facing=east]",
            "R" => "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]",
            "S" => "minecraft:stone",
            "r" => "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]",
            _ => return None,
        })
    };
    for (iz, row) in rows0.iter().enumerate() {
        for (ix, c) in row.iter().enumerate() {
            if let Some(s) = pal(c) {
                put(&mut d, [ix as i32 - 1, 0, iz as i32 - 5], s);
            }
        }
    }
    for (iz, row) in rows1.iter().enumerate() {
        for (ix, c) in row.iter().enumerate() {
            // (the generator's own comparator at (-1,1,-2) is not used: the taps are D9's and D8's)
            if *c == "C" {
                continue;
            }
            if let Some(s) = pal(c) {
                put(&mut d, [ix as i32 - 1, 1, iz as i32 - 5], s);
            }
        }
    }
    // floors under the dust and the repeater
    put(&mut d, [0, -1, -3], "minecraft:stone");
    put(&mut d, [0, -1, -1], "minecraft:stone");
    // seals over every hopper whose top is open (nothing can fall in; a hopper pulls only from a
    // container above)
    let hoppers: Vec<P> = d.blocks.iter().filter(|(_, s)| s.contains("hopper")).map(|(p, _)| *p).collect();
    for h in hoppers {
        let up = [h[0], h[1] + 1, h[2]];
        if !d.blocks.contains_key(&up) {
            put(&mut d, up, "minecraft:stone");
        }
    }
    // the main tap: a comparator facing west (input D9 at (1,0,-2)), output into (3,0,-2); its floor
    put(&mut d, TAP_CMP, "minecraft:comparator[facing=west,mode=compare,powered=false]");
    put(&mut d, [TAP_CMP[0], -1, TAP_CMP[2]], "minecraft:stone");
    put(&mut d, [TAP_OUT[0], -1, TAP_OUT[2]], "minecraft:stone");
    // the u = 9 tap: a comparator facing north (input D8 at (1,0,0)), output into (1,0,2)
    put(&mut d, TAP9_CMP, "minecraft:comparator[facing=north,mode=compare,powered=false]");
    put(&mut d, [TAP9_CMP[0], -1, TAP9_CMP[2]], "minecraft:stone");
    put(&mut d, [TAP9_OUT[0], -1, TAP9_OUT[2]], "minecraft:stone");
    // the clock: the ring's input cell (0,0,0) is a stone (the generator's judge put its own there)
    // whose neighbours fire; a repeater (facing south: input from z + 1) into it at (0,0,1), fed by a
    // second at (0,0,2) from the bench's drive cell (0,0,3)
    put(&mut d, INPUT, "minecraft:stone");
    put(&mut d, [0, 0, 1], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    put(&mut d, [0, 0, 2], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    put(&mut d, [0, -1, 1], "minecraft:stone");
    put(&mut d, [0, -1, 2], "minecraft:stone");
    put(&mut d, [0, -1, 3], "minecraft:stone");
    // the tokens
    for (i, k) in token_droppers().iter().enumerate() {
        let id = if i == 0 { "minecraft:wooden_shovel" } else { "minecraft:stone" };
        d.block_entities.insert(DROPPERS[*k], format!("{{Items:[{{Slot:0b,id:\"{id}\",count:1}}]}}"));
    }
    // (the mob-farm kind runs by itself: no input port; the bench drives (0,0,2) directly)
    d.ports.insert("output".into(), TAP_OUT);
    d
}

/// The level a tap shows at phase u for the token due there: 2 the unstackable (u 0), 1 a stackable
/// event, 0 none.
fn due(u: i64) -> i32 {
    let u = u.rem_euclid(10);
    if u == 0 {
        2
    } else if EVENTS.contains(&u) {
        1
    } else {
        0
    }
}

/// The bench: `cycles` clock pulses (a redstone block at the driver's input for `width` gt) every
/// `period` gt (+ a jitter from `jitter`, cycling). Both taps read just before each next pulse: the
/// main tap shows the token due at u = the step count (2 at u 0, 1 at an event, 0 otherwise), the u = 9
/// tap the one due a step later. Every tick between pulses is traced: the settle time after a pulse,
/// and between two consecutive token phases whether the main tap's level dropped to 0 (an edge an
/// event counter could count) or held >= 1 (no edge: the counter must be clocked by the step and the
/// tap sampled). The items in the ring at the end (8: none lost).
pub fn bench(spec: &FarmSpec, cycles: i64, period: i64, width: i64, jitter: &[i64]) -> Result<Vec<String>, String> {
    let horizon = cycles * period + 200;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, horizon as u32 + 20, WalkMode::Random) };
    let mut run = Run::start(&crate::mirror::spec(spec)?, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let all = redsim::world::flags::ALL;
    let bp = |p: P| crate::mirror::at(p);
    let drive = crate::mirror::bp(0, 0, 3);
    let (mut ok, mut ok9, mut bad) = (0u64, 0u64, Vec::new());
    let (mut settle_max, mut settle9_max) = (0i64, 0i64);
    let mut levels: Vec<i32> = Vec::new();
    let mut levels9: Vec<i32> = Vec::new();
    let mut pulse_at: Vec<i64> = Vec::new();
    let mut t_next = 50i64;
    for c in 0..cycles {
        pulse_at.push(t_next + jitter.get(c as usize % jitter.len().max(1)).copied().unwrap_or(0));
        t_next += period;
    }
    // (per step: the main tap's least level from the pulse to the next one)
    let mut step_min: Vec<i32> = vec![i32::MAX; pulse_at.len()];
    let mut first_trace: Option<(usize, Vec<i32>)> = None;
    let mut trace_now: Vec<i32> = Vec::new();
    let mut ci = 0usize;
    let (mut last, mut last9) = (0, 0);
    for t in 0..horizon {
        if ci < pulse_at.len() && t == pulse_at[ci] {
            // (read both taps just before the pulse: the state after the previous pulse, u = ci)
            let lv = run.world.best_neighbor_signal(bp(TAP_OUT));
            let lv9 = run.world.best_neighbor_signal(bp(TAP9_OUT));
            levels.push(lv);
            levels9.push(lv9);
            if lv == due(ci as i64) {
                ok += 1;
            } else if bad.len() < 5 {
                bad.push(format!("before pulse {ci} (u {}): main tap {lv}, expected {}", ci % 10, due(ci as i64)));
            }
            if lv9 == due(ci as i64 + 1) {
                ok9 += 1;
            } else if bad.len() < 5 {
                bad.push(format!("before pulse {ci} (u {}): u-9 tap {lv9}, expected {}", ci % 10, due(ci as i64 + 1)));
            }
            // (the step that just ended: trace it if it was the first between two consecutive tokens)
            if ci > 0 && first_trace.is_none() && due(ci as i64 - 1) >= 1 && due(ci as i64) >= 1 {
                first_trace = Some((ci, std::mem::take(&mut trace_now)));
            }
            trace_now.clear();
            run.world.set_block(drive, on.clone(), all);
        }
        if ci < pulse_at.len() && t == pulse_at[ci] + width {
            run.world.set_block(drive, air.clone(), all);
            ci += 1;
        }
        run.tick();
        let lv = run.world.best_neighbor_signal(bp(TAP_OUT));
        let lv9 = run.world.best_neighbor_signal(bp(TAP9_OUT));
        // (the step in progress: pulse k's from its rise to the next pulse)
        if let Some(k) = pulse_at.iter().rposition(|p| *p <= t) {
            step_min[k] = step_min[k].min(lv);
            if lv != last {
                settle_max = settle_max.max(t + 1 - pulse_at[k]);
            }
            if lv9 != last9 {
                settle9_max = settle9_max.max(t + 1 - pulse_at[k]);
            }
            trace_now.push(lv);
        }
        last = lv;
        last9 = lv9;
    }
    // (consecutive token phases: step k moved the ring from u = k to u = k + 1; both phases carry a
    // token when due(k) >= 1 and due(k + 1) >= 1)
    let (mut cons, mut dips) = (0u32, 0u32);
    for k in 0..pulse_at.len().saturating_sub(1) {
        if due(k as i64) >= 1 && due(k as i64 + 1) >= 1 {
            cons += 1;
            if step_min[k] == 0 {
                dips += 1;
            }
        }
    }
    // (the ring's items: eight tokens, none lost; droppers and hoppers both)
    let hoppers: Vec<P> = design().blocks.iter().filter(|(_, s)| s.contains("hopper")).map(|(p, _)| *p).collect();
    let in_droppers: usize = DROPPERS.iter().map(|p| redsim::redstone::container::analog_output(&run.world, bp(*p)).min(1) as usize).sum();
    let in_hoppers: usize = hoppers.iter().map(|p| redsim::redstone::container::analog_output(&run.world, bp(*p)).min(1) as usize).sum();
    let items = in_droppers + in_hoppers;
    let row = |v: &[i32]| v.iter().take(20).map(|l| l.to_string()).collect::<Vec<_>>().join(" ");
    let toks = token_droppers();
    Ok(vec![
        format!(
            "units ring (10 droppers, {} tokens at u {{{}}}: the unstackable at D{}, stones at D{}; the main tap on D{TAP}, the u-9 tap on D{TAP9}): {cycles} pulses {width} gt wide every {period} gt (jitter {:?}): before each pulse the main tap matched (2 at u 0, 1 at an event, 0 otherwise) {ok} of {}, the u-9 tap (the token due a step later; 2 at u 9) {ok9} of {}{}",
            EVENTS.len(),
            EVENTS.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", "),
            toks[0],
            toks[1..].iter().map(|k| k.to_string()).collect::<Vec<_>>().join(", D"),
            jitter,
            levels.len(),
            levels9.len(),
            if bad.is_empty() { String::new() } else { format!(" (FAIL: {})", bad.join("; ")) }
        ),
        format!(
            "  the main tap before the first 20 pulses: {}; the u-9 tap: {}; the latest change after a pulse: main {settle_max} gt, u-9 {settle9_max} gt; containers holding a token at the end {items} ({in_droppers} droppers + {in_hoppers} hoppers; {} expected: none lost)",
            row(&levels),
            row(&levels9),
            EVENTS.len()
        ),
        format!(
            "  between consecutive token phases ({cons} steps: u 0 -> 1 -> 2, 4 -> 5 -> 6 -> 7 -> 8): the main tap dropped to 0 inside {dips} of them, held >= 1 through {} ({}){}",
            cons - dips,
            if dips == cons { "an edge each step: an event counter may count the tap's rises" } else { "no edge between them: the event counter must be clocked by the step, the tap sampled after it settles" },
            first_trace.map_or(String::new(), |(k, tr)| format!("; the tick-by-tick main tap through the step into u {k}: {}", tr.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(" ")))
        ),
    ])
}
