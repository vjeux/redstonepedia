//! The controller's units digit (day-1, Nov 12 2026; ironspec's ruling 00:14): ONE 10-dropper ring
//! (dropper -> hopper -> dropper: every transit crosses a hopper, so the firing order never matters),
//! FOUR tokens and ONE tap. The tokens sit where they reach the tapped dropper at u = 0, 3, 6, 7 (the
//! picks 0/3/6 and the siding phase 7); the u = 0 token is an unstackable item, so the tap's
//! comparator reads 2 there and 1 for the stackable tokens: the u = 0 event is identified and resets
//! the 2-bulb event counter (events 0..3 = u 0/3/6/7). The ring is `mctas circuit memory ring --n 10
//! --unchecked`'s counter10-ring10-0 (27 blocks, 3 x 6 x 3), its open hopper tops sealed with stone
//! (every 10-ring fails that judge only on item intrusion through open tops).

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};

type P = [i32; 3];

/// The ring's droppers in flow order (D0..D9), local cells (x -1..1, y 0..1, z -5..0).
pub const DROPPERS: [P; 10] = [[0, 1, -2], [0, 1, -4], [1, 0, -4], [0, 0, -5], [-1, 0, -4], [-1, 0, -2], [-1, 0, 0], [0, 1, 0], [1, 0, 0], [1, 0, -2]];
/// The tapped dropper (D8) and its comparator (facing north: input from D8, output south).
pub const TAP: usize = 8;
pub const TAP_CMP: P = [1, 0, 1];
pub const TAP_OUT: P = [1, 0, 2];
/// The clock input (the ring's input cell, driven from the south).
pub const INPUT: P = [0, 0, 0];

/// The tokens' droppers at u = 0: D_T (the unstackable), D_{T-3}, D_{T-6}, D_{T-7}.
pub fn token_droppers() -> [usize; 4] {
    [TAP, (TAP + 7) % 10, (TAP + 4) % 10, (TAP + 3) % 10]
}

/// The units digit's design: the ring, the seals, the tokens, the tap.
pub fn design() -> Design {
    let mut d = Design { name: "units-ring10-4tokens".into(), ..Default::default() };
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
            // (the generator's own comparator at (-1,1,-2) is not used: the tap is D8's)
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
    // the tap: a comparator facing north (input D8 at (1,0,0)), output into (1,0,2); its floor
    put(&mut d, TAP_CMP, "minecraft:comparator[facing=north,mode=compare,powered=false]");
    put(&mut d, [TAP_CMP[0], -1, TAP_CMP[2]], "minecraft:stone");
    put(&mut d, [TAP_OUT[0], -1, TAP_OUT[2]], "minecraft:stone");
    // the clock: the ring's input cell (0,0,0) is a stone (the generator's judge put its own there)
    // whose neighbours fire; a repeater (facing south: input from z + 1) into it at (0,0,1), fed by a
    // second at (0,0,2) from the bench's drive cell (0,0,3) (away from the tap's readout cell (1,0,2))
    put(&mut d, INPUT, "minecraft:stone");
    put(&mut d, [0, 0, 1], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    put(&mut d, [0, 0, 2], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    put(&mut d, [0, -1, 1], "minecraft:stone");
    put(&mut d, [0, -1, 2], "minecraft:stone");
    put(&mut d, [0, -1, 3], "minecraft:stone");
    // the tokens
    let toks = token_droppers();
    for (i, k) in toks.iter().enumerate() {
        let id = if i == 0 { "minecraft:wooden_shovel" } else { "minecraft:stone" };
        d.block_entities.insert(DROPPERS[*k], format!("{{Items:[{{Slot:0b,id:\"{id}\",count:1}}]}}"));
    }
    // (the mob-farm kind runs by itself: no input port; the bench drives (0,0,2) directly)
    d.ports.insert("output".into(), TAP_OUT);
    d
}

/// The bench: `cycles` clock pulses (a redstone block at the driver's input for `width` gt) every
/// `period` gt (+ a jitter from `jitter`, cycling), the tap's level read just before each next pulse:
/// 2 at u = 0, 1 at u = 3, 6, 7, 0 otherwise. Every step counted (the tokens' droppers after each
/// pulse), the settle time after a pulse, and the items in the ring (none lost).
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
    let expect = |u: i64| -> i32 { match u.rem_euclid(10) { 0 => 2, 3 | 6 | 7 => 1, _ => 0 } };
    let (mut ok, mut bad) = (0u64, Vec::new());
    let mut settle_max = 0i64;
    let mut levels: Vec<i32> = Vec::new();
    let mut pulse_at: Vec<i64> = Vec::new();
    let mut t_next = 50i64;
    for c in 0..cycles {
        pulse_at.push(t_next + jitter.get(c as usize % jitter.len().max(1)).copied().unwrap_or(0));
        t_next += period;
    }
    let mut ci = 0usize;
    let mut last_level = 0;
    for t in 0..horizon {
        if ci < pulse_at.len() && t == pulse_at[ci] {
            // (read the tap just before the pulse: the state after the previous pulse, u = ci)
            let lv = run.world.best_neighbor_signal(bp(TAP_OUT));
            levels.push(lv);
            if lv == expect(ci as i64) {
                ok += 1;
            } else if bad.len() < 5 {
                bad.push(format!("before pulse {ci} (u {}): tap {lv}, expected {}", ci % 10, expect(ci as i64)));
            }
            run.world.set_block(drive, on.clone(), all);
        }
        if ci < pulse_at.len() && t == pulse_at[ci] + width {
            run.world.set_block(drive, air.clone(), all);
            ci += 1;
        }
        run.tick();
        let lv = run.world.best_neighbor_signal(bp(TAP_OUT));
        if lv != last_level {
            last_level = lv;
            let last_change = t + 1;
            if let Some(p) = pulse_at.iter().rev().find(|p| **p <= t) {
                settle_max = settle_max.max(last_change - p);
            }
        }
    }
    // (the ring's items: four tokens, none lost; droppers and hoppers both, so a token caught in a
    // hopper shows too (G1 on d36a61b59))
    let hoppers: Vec<P> = design().blocks.iter().filter(|(_, s)| s.contains("hopper")).map(|(p, _)| *p).collect();
    let in_droppers: usize = DROPPERS.iter().map(|p| redsim::redstone::container::analog_output(&run.world, bp(*p)).min(1) as usize).sum();
    let in_hoppers: usize = hoppers.iter().map(|p| redsim::redstone::container::analog_output(&run.world, bp(*p)).min(1) as usize).sum();
    let items = in_droppers + in_hoppers;
    let lv_str: Vec<String> = levels.iter().take(20).map(|l| l.to_string()).collect();
    Ok(vec![
        format!("units ring (10 droppers, 4 tokens: the unstackable at D{TAP}, stones at D{}, D{}, D{}; the tap on D{TAP}): {cycles} pulses {width} gt wide every {period} gt (jitter {:?}): the tap's level before each pulse matched (2 at u 0, 1 at u 3/6/7, 0 otherwise) {ok} of {}{}", token_droppers()[1], token_droppers()[2], token_droppers()[3], jitter, levels.len(), if bad.is_empty() { String::new() } else { format!(" (FAIL: {})", bad.join("; ")) }),
        format!("  the tap's levels before the first 20 pulses: {}; the latest change after a pulse {settle_max} gt (the step settles within {settle_max} gt of the clock); containers holding a token at the end {items} ({in_droppers} droppers + {in_hoppers} hoppers; 4 expected: none lost)", lv_str.join(" ")),
    ])
}
