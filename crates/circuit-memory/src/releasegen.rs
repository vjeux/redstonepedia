//! `mctas circuit memory release-gen`: the single-lane loop's RELEASE CONTROLLER (day-1 lane
//! 2026-11-10; vjeux 07:24 "what's the setup that sends them 3 by 3"; ironfarm 07:27-08:40). Real
//! redstone: the golem pulse in; out: REL (one ~10-gt pulse per release, starting 2 gt after the
//! golem), the release group's window and the advance pulses that ironfarm's queue register needs.
//! `--loop F` builds it into ironfarm's loop file instead, wired to its 21 controls.
//!
//! Core (components at y 1 on supports at y 0, the module's own frame):
//! - TMR: a 3-comparator countdown ring (I2 (0,0) -> cA -> S1 (2,0) -> cB -> S2 (2,2) -> cC -> dust
//!   -> I2), refilled to 15 by each release and losing 1 per 6-gt round.
//! - BUSY = S1 - 2 (subtract comparator B (3,0) on S1, a constant 2 from comparator K (3,-1)
//!   reading a hopper of 23 items), through a repeater (4,0) into GO's west side: high until ~92 gt
//!   after a release.
//! - EXPIRE: a wall torch on S2, lit when the ring is empty (~104 gt after a release with no
//!   golem): the fallback.
//! - GO (5,0) = X - BUSY, X = golem OR REQ OR EXPIRE (dust from (5,1)): its output is REL; REL
//!   refills TMR, so BUSY cuts it ~10 gt later: one self-timed pulse per release.
//! - REQ: a waxed copper bulb (7,1) toggled ON by X's rising edge (subtract comparator E (6,1):
//!   X minus X 6 gt late); its reset line (x 7) carries the delay chain's taps from REL + 8 to
//!   about + 56 continuously, so its rising edge turns REQ off and an edge inside it toggles nothing
//!   (the merge window); a comparator (8,1) reads REQ back into X.
//! - The delay chain from REL along x 5 (z -5 on): its stones' taps make the release group's window
//!   (with REL through the diode R_g (4,-4): x 3 from z -5), the advance line (x 3) and the reset
//!   line.

use crate::schedgen::{B, DUST, P3, rep};
use anyhow::{Result, bail};
use mctas::design::Design;
use std::collections::BTreeMap;
use std::path::Path;

const STONE: &str = "minecraft:stone";
const G: u8 = 1; // a group tap (x 4, into the group line x 3)
const A: u8 = 2; // an advance tap (x 4, into the advance line x 3)
const RS: u8 = 4; // a reset tap (x 6, into the reset line x 7)

/// One cell of the delay chain (x 5, from z -5 toward -z): a repeater, a dust, a stone with taps.
#[derive(Clone, Copy)]
enum It {
    R(u8),
    D,
    S(u8),
}
use It::{D, R, S};

fn comp(facing: &str, mode: &str) -> String {
    format!("minecraft:comparator[facing={facing},mode={mode},powered=false]")
}

/// The controller in its own frame. Returns the chain's stones (z, flags) and its end z.
fn core(b: &mut B, chain: &[It], group_to: i32) -> Result<(Vec<(i32, u8)>, i32)> {
    let y = 1;
    // ---- TMR ring
    b.put([0, y, 0], STONE)?; // I2
    b.on([1, y, 0], &comp("west", "compare"))?; // cA: rear I2, out east into S1
    b.put([2, y, 0], STONE)?; // S1
    b.on([2, y, 1], &comp("north", "compare"))?; // cB: rear S1, out south into S2
    b.put([2, y, 2], STONE)?; // S2
    b.on([1, y, 2], &comp("east", "compare"))?; // cC: rear S2, out west
    b.on([0, y, 2], DUST)?;
    b.on([0, y, 1], DUST)?;
    // (the refill into I2 from the north: the comparator at (0,-1), below)
    b.put([2, y, 3], "minecraft:redstone_wall_torch[facing=south,lit=true]")?; // EXPIRE
    // ---- BUSY
    b.on([3, y, 0], &comp("west", "subtract"))?; // B: rear S1
    b.on([3, y, -1], &comp("north", "compare"))?; // K: rear the hopper, out south into B
    b.put([3, y, -2], "minecraft:hopper[enabled=true,facing=down]")?;
    b.nbt.insert([3, y, -2], "{Items:[{Slot:0b,id:\"minecraft:stone\",count:23}]}".into());
    b.on([4, y, 0], &rep("west", 1))?; // B's output into GO's west side at full strength: GO
                                       // subtracts it from X, so BUSY must be 15 to block any X
    // ---- GO and X
    b.on([5, y, 0], &comp("south", "subtract"))?; // rear (5,1), out north (5,-1)
    for c in [[5, y, 1], [5, y, 2], [5, y, 3], [3, y, 3], [5, y, 4]] {
        b.on(c, DUST)?;
    }
    b.ports.insert("input.golem".into(), [5, y, 5]);
    // ---- ARMED: the fallback waits for the first golem (no release by itself at power-on). EXPIRE
    // reaches X only through the gate G_e (4,3) = EXPIRE - NA; NA (4,4) = a redstone block (4,5)
    // minus ARMED; ARMED: a stone (2,4) held by a repeater (2,5) from a one-repeater loop (2,7)
    // (2,6) (1,6) (1,7) (1,8) (2,8), set once by the golem input through (6,5) .. (3,8)
    b.on([4, y, 3], &comp("west", "subtract"))?; // G_e: rear the EXPIRE dust (3,3), out east into X
    b.on([4, y, 4], &comp("south", "subtract"))?; // NA: rear the redstone block, out north into G_e
    b.put([4, y, 5], "minecraft:redstone_block")?;
    b.on([3, y, 4], &rep("west", 1))?; // ARMED (the stone (2,4)) into NA's west side
    b.put([2, y, 4], STONE)?;
    b.on([2, y, 5], &rep("south", 1))?; // the loop (2,6) into the stone
    b.on([2, y, 7], &rep("south", 1))?; // the loop's repeater: (2,8) -> (2,6)
    for c in [[2, y, 6], [1, y, 6], [1, y, 7], [1, y, 8], [2, y, 8], [3, y, 8], [4, y, 8], [5, y, 8], [6, y, 8], [6, y, 7], [6, y, 6], [6, y, 5]] {
        b.on(c, DUST)?;
    }
    // ---- E (X's rising edge) and the REQ bulb
    b.on([6, y, 1], &comp("west", "subtract"))?; // rear X (5,1), out east into the bulb
    b.on([6, y, 2], &rep("south", 3))?; // X (6,3) 6 gt late into E's south side
    b.on([6, y, 3], DUST)?;
    b.put([7, y, 1], "minecraft:waxed_copper_bulb[lit=false,powered=false]")?;
    b.on([8, y, 1], &comp("west", "compare"))?; // reads the bulb, out east (9,1)
    for c in [[9, y, 1], [9, y, 2], [9, y, 3], [8, y, 3]] {
        b.on(c, DUST)?;
    }
    // REQ back into X at full strength, one cell from X's (5,3) like the golem input, so X keeps
    // one level when the golem pulse hands over to REQ (a level change there re-ticks the refill
    // comparator and could cut the refill 2 gt short)
    b.on([7, y, 3], &rep("east", 1))?;
    // ---- REL: GO's output north; the refill route west along z -3. The refill is REL's first 8 gt
    // only, at full strength whatever X's level: R_r (1,-2) squares REL into the stone (0,-2), the
    // refill comparator (0,-1) (rear that stone, side REL 10 gt late through R1 (-1,-3) and R4
    // (-1,-1)) drives I2 directly; so the countdown starts at a fixed time after REL's start, and a
    // change in X's level during the refill cannot re-tick the comparator and cut it short
    for c in [[5, y, -1], [5, y, -2], [5, y, -3], [4, y, -3], [3, y, -3], [2, y, -3], [5, y, -4], [2, y, -2], [-2, y, -3], [-2, y, -2], [-2, y, -1]] {
        b.on(c, DUST)?;
    }
    // (a repeater, not dust, at (1,-3): in the loop that cell sits under a booster's redstone block,
    // which would hold a dust on; it powers R1's stone strongly)
    b.on([1, y, -3], &rep("east", 1))?;
    b.put([0, y, -3], STONE)?; // weakly powered by the REL dust (1,-3): R1's input
    // dust at (-1,-3), lit by the strongly powered stone (0,-3): the refill's side path and, in the
    // loop, the stand's set line start here (no repeater: S1 at release + 6)
    b.on([-1, y, -3], DUST)?;
    b.on([-1, y, -1], &rep("west", 4))?; // R4 into the refill comparator's west side (REL + 10)
    b.on([1, y, -2], &rep("east", 1))?; // R_r
    b.put([0, y, -2], STONE)?;
    b.on([0, y, -1], &comp("north", "subtract"))?; // the refill, into I2
    // ---- the delay chain along x 5 from z -5
    let mut z = -5;
    let mut stones = Vec::new();
    for it in chain {
        match *it {
            R(d) => b.on([5, y, z], &rep("south", d))?,
            D => b.on([5, y, z], DUST)?,
            S(f) => {
                b.put([5, y, z], STONE)?;
                if f & (G | A) != 0 {
                    b.on([4, y, z], &rep("east", 1))?; // out west into x 3
                }
                if f & RS != 0 {
                    b.on([6, y, z], &rep("west", 1))?; // out east into the reset line (x 7)
                }
                stones.push((z, f));
            }
        }
        z -= 1;
    }
    // the release group: REL through R_g (4,-4) (a diode, so nothing flows back into REL) and the
    // group taps
    b.on([4, y, -4], &rep("south", 1))?;
    b.on([4, y, -5], DUST)?;
    for zz in group_to..=-5 {
        b.on([3, y, zz], DUST)?;
    }
    // the bulb's reset line: x 7 from the farthest reset tap to the bulb, restored at z -5, south of
    // every tap, so all taps reach the bulb with the same delay and consecutive windows overlap
    let far = stones.iter().filter(|s| s.1 & RS != 0).map(|s| s.0).min().unwrap_or(-1);
    for zz in far..=-1 {
        if zz == -5 {
            b.on([7, y, zz], &rep("north", 1))?;
        } else {
            b.on([7, y, zz], DUST)?;
        }
    }
    b.on([7, y, 0], &rep("north", 1))?; // into the bulb
    // the advance line: x 3 between the advance taps
    let adv: Vec<i32> = stones.iter().filter(|s| s.1 & A != 0).map(|s| s.0).collect();
    if let (Some(first), Some(last)) = (adv.first(), adv.last()) {
        for zz in *last..=*first {
            b.on([3, y, zz], DUST)?;
        }
    }
    Ok((stones, z))
}

fn new_b() -> B {
    B { blocks: BTreeMap::new(), ports: BTreeMap::new(), nbt: BTreeMap::new() }
}

/// The module alone (designs/farms/iron/parts/carousel-release/carousel-release-v0.txt): stones
/// at REL + 8, 16, 24 (group), 32, 38 (A1), 46, 54, 58 (A2), 66, 74, 78 (A3).
pub fn generate(out: &Path, _offdelay: &Path) -> Result<()> {
    let mut b = new_b();
    let y = 1;
    let chain = [R(4), S(G | RS), R(4), S(G | RS), R(4), S(G | RS), R(4), S(RS), R(3), S(A | RS), R(4), S(RS), R(4), S(0), R(2), S(A), R(4), S(0), R(4), S(0), R(2), S(A)];
    let (stones, _) = core(&mut b, &chain, -11)?;
    b.on([2, y, -11], DUST)?;
    b.ports.insert("output.group".into(), [1, y, -11]);
    let last_a = stones.iter().filter(|s| s.1 & A != 0).map(|s| s.0).min().unwrap_or(0);
    b.on([3, y, last_a - 1], DUST)?;
    b.ports.insert("output.adv".into(), [3, y, last_a - 2]);
    save(b, out, "carousel-release-controller")
}

fn save(b: B, out: &Path, name: &str) -> Result<()> {
    let mut d = Design::parse(&format!("name: {name}\n"))?;
    d.blocks = b.blocks;
    d.ports = b.ports;
    d.block_entities = b.nbt;
    d.save(out)?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}

/// A block state turned 180 degrees (east <-> west, north <-> south).
fn turn2(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("facing=") {
        o.push_str(&rest[..i + 7]);
        let tail = &rest[i + 7..];
        let (word, len) = [("east", "west"), ("west", "east"), ("north", "south"), ("south", "north")]
            .iter()
            .find(|(a, _)| tail.starts_with(a))
            .map(|(a, b)| (*b, a.len()))
            .unwrap_or(("", 0));
        o.push_str(word);
        rest = &tail[len..];
    }
    o.push_str(rest);
    o
}

/// The controller built into ironfarm's loop file (explore/ironfarm 99d3566d7 cells, 08:36):
/// the module turned 180 degrees west of row B (frame: global = (-2 - x, y - 2, 10 - z)), its
/// lines wired to the 18 queue controls head first, the tail windows (slots 1-2 ON at +88..+110,
/// slot 0 +90..+116 from the departure) and the 21 control glass blocks replaced by stone.
pub fn generate_loop(out: &Path, loop_file: &Path, offdelay: &Path) -> Result<()> {
    // the chain, stretched so the group line (x -5, z 15..25) and the advance line (z 27..) do not
    // touch; the advance taps 2 gt early for the feeder repeaters (+36/56/76 -> +38/58/78 at the
    // controls); reset taps 8..44 keep the merge window to ~+58
    let chain = [
        R(4), S(G | RS), // +8
        R(4), S(G | RS), // +16
        R(4), S(G | RS), // +24
        R(1), D, R(1), D, R(1), D, S(A | RS), // +30 (A1; zero skew pads every slot to row A's path)
        R(4), S(RS), // +38
        R(4), S(RS), // +46
        R(2), S(A), // +50 (A2)
        R(4), S(0), // +58
        R(4), S(0), // +66
        R(2), S(A), // +70 (A3)
    ];
    let mut m = new_b();
    let (stones, _) = core(&mut m, &chain, -15)?;
    let at = |p: P3| -> P3 { [-2 - p[0], p[1] - 2, 10 - p[2]] };
    let mut b = new_b();
    for (p, s) in &m.blocks {
        b.put(at(*p), &turn2(s))?;
    }
    for (p, n) in &m.nbt {
        b.nbt.insert(at(*p), n.clone());
    }
    let golem = at(m.ports["input.golem"]);
    let a3 = at([5, 1, stones.iter().filter(|s| s.1 & A != 0).map(|s| s.0).min().unwrap_or(0)]);
    let yy = -1;
    // ---- the release group (slots 15..17 at x -3, z 19/22/25): dust spurs east from the group line
    for z in [19, 22, 25] {
        b.on([-4, yy, z], DUST)?;
    }
    // (zero skew, ironfarm 10:16: every queue control's advance on one tick; the paths differ, so
    // each feeder's delay pads its path to the slowest (row A, release + 44): the group's advance
    // diode +6, row B's head feeders +6, slots 8-9 +4, the turn slot +2)
    b.on([-5, yy, 26], &rep("south", 4))?; // the advance into the group net (a diode)
    // ---- row B (slots 14..8 at x -3, z 28..46): the advance line on to z 46, feeders from the west
    for z in a3[2] + 1..=46 {
        if z == 41 {
            // restores the first advance pulse (A1 injects at z 27) for the slots past z 40
            b.on([-5, yy, z], &rep("north", 1))?;
        } else {
            b.on([-5, yy, z], DUST)?;
        }
    }
    for z in (28..=46).step_by(3) {
        b.on([-4, yy, z], &rep("west", if z < 41 { 4 } else { 3 }))?;
    }
    // ---- around the north end: slot 7 (-1,-1,47) fed from z 48; row A (x 0, z 45..36) from x 2
    b.on([-5, yy, 47], &rep("north", 1))?;
    b.on([-5, yy, 48], DUST)?;
    for x in -5..=2 {
        b.on([x, yy, 49], DUST)?;
    }
    b.on([-1, yy, 48], &rep("south", 2))?;
    b.on([2, yy, 48], DUST)?;
    b.on([2, yy, 47], &rep("south", 1))?;
    for z in 36..=46 {
        b.on([2, yy, z], DUST)?;
    }
    for z in [45, 42, 39, 36] {
        b.on([1, yy, z], &rep("east", 1))?;
    }
    // ---- the descending tail slots (ironfarm 9bd8f2ff9): slot 2 at (0,-1,33), slot 1 at (0,0,29),
    // slot 0 at (0,1,25), each fed by a repeater at x 1. Net N12 (slots 1-2): x 2 from z 33 at y -1
    // climbing to (2,0,29); net N0 (slot 0): x 6 from z 34 to 27 at y -1, up (6,0,26) (6,1,25),
    // west along z 25 at y 1. The advance reaches each through its own diode off row A's line
    // ((2,-1,34) into N12; (6,-1,35) into N0, branched along z 36), so the windows stay apart; the
    // tail lands 2 gt after the other slots (its diode; ironfarm 10:30: fine)
    b.on([2, yy, 35], DUST)?;
    b.on([2, yy, 34], &rep("south", 1))?;
    for c in [[2, -1, 33], [2, -1, 32], [2, -1, 31], [2, 0, 30], [2, 0, 29]] {
        b.on(c, DUST)?;
    }
    b.on([1, -1, 33], &rep("east", 1))?;
    b.on([1, 0, 29], &rep("east", 1))?;
    for x in 3..=6 {
        b.on([x, yy, 36], DUST)?;
    }
    b.on([6, yy, 35], &rep("south", 1))?;
    for z in 27..=34 {
        b.on([6, yy, z], DUST)?;
    }
    for c in [[6, 0, 26], [6, 1, 25], [5, 1, 25], [4, 1, 25], [3, 1, 25], [2, 1, 25]] {
        b.on(c, DUST)?;
    }
    b.on([1, 1, 25], &rep("east", 1))?;
    // ---- the tail windows (at the controls, t0-relative: slots 1-2 ON +88..+120, slot 0 +88..+128,
    // both merged with the 3rd advance): the chain from A3 (REL + 70) runs under the loop at y -3
    // along z 43, climbs at x 8 to y -1 and runs along x 8 toward -z, stones at golem + 86 (z 34),
    // + 96 (z 31), + 104 (z 29) and + 114 (z 26). N0 taps (delay-2 repeaters at x 7 into N0's x 6
    // line, the last into its climb's support (6,-1,26)): control = stone + 6, ON + 92..+ 132. N12
    // taps (repeaters at x 9 into an OR line along x 10, restored at (10,-1,25), west along z 24, up
    // x 4, into N12 at (2,-1,31) through (3,-1,31)): control = stone + 8, ON + 94..+ 124 (the first
    // three stones)
    let (ax, az) = (a3[0], a3[2]);
    b.on([ax, yy, az + 1], &rep("north", 4))?; // golem + 80
    b.on([ax, yy, az + 2], DUST)?;
    b.on([ax, -2, az + 3], DUST)?;
    b.on([ax, -3, az + 4], DUST)?;
    let zl = az + 4;
    for x in ax + 1..=8 {
        if x == -1 {
            b.on([x, -3, zl], &rep("west", 1))?; // golem + 82
        } else {
            b.on([x, -3, zl], DUST)?;
        }
    }
    b.on([8, -2, zl - 1], DUST)?;
    b.on([8, -1, zl - 2], DUST)?;
    // the chain (x 8, y -1) from z zl - 3 toward -z
    let chain_tail: [(i32, It); 15] = [
        (40, R(2)),
        (39, D), (38, D), (37, D), (36, D), (35, D),
        (34, S(0)), // golem + 86: N12 + N0
        (33, R(4)), (32, R(1)),
        (31, S(0)), // + 96: N12 + N0
        (30, R(4)),
        (29, S(0)), // + 104: N12 + N0
        (28, R(4)), (27, R(1)),
        (26, S(0)), // + 114: N0
    ];
    for (z, it) in chain_tail {
        match it {
            R(d) => b.on([8, yy, z], &rep("south", d))?,
            D => b.on([8, yy, z], DUST)?,
            S(_) => b.put([8, yy, z], STONE)?,
        }
    }
    for z in [34, 31, 29, 26] {
        b.on([7, yy, z], &rep("east", 2))?; // N0 taps (+4)
    }
    for z in [34, 31, 29] {
        b.on([9, yy, z], &rep("west", 1))?; // N12 taps into x 10
    }
    for z in 24..=34 {
        if z == 25 {
            b.on([10, yy, z], &rep("south", 1))?; // restores the OR line
        } else {
            b.on([10, yy, z], DUST)?;
        }
    }
    for x in 4..=9 {
        b.on([x, yy, 24], DUST)?;
    }
    for z in 25..=31 {
        b.on([4, yy, z], DUST)?;
    }
    b.on([3, yy, 31], &rep("east", 1))?; // into N12
    // ---- the stand (S1 S2 S3 at (5|2|-1,-1,0)): copper-bulb latches north of it, read by
    // comparators straight into the controls; set by REL (a tap off the REL route, 3 repeaters to a
    // common line along z -6: t0 + 4) through dust (S1: control at t0 + 6), 3 rt (S2: + 12) and
    // 9 rt (S3: + 24); reset all together by a dust line on top of the bulbs (y 0, z -2) from the
    // reset cell (6,0,-2) (line 4, bench-driven until ironfarm's rule)
    // (the stand's set line starts at the core's dust (-1,-1,13), lit by the REL stone (-2,-1,13))
    for x in 0..=7 {
        b.on([x, yy, 13], DUST)?;
    }
    for z in (-4..=12).rev() {
        if z == 7 {
            b.on([7, yy, z], &rep("south", 1))?; // t0 + 2
        } else {
            b.on([7, yy, z], DUST)?;
        }
    }
    b.on([7, yy, -5], &rep("south", 1))?; // t0 + 4
    for x in -1..=7 {
        b.on([x, yy, -6], DUST)?;
    }
    for z in -5..=-3 {
        b.on([5, yy, z], DUST)?; // S1: dust into its bulb
    }
    b.on([2, yy, -5], &rep("north", 4))?; // S2: 8 gt more (+14; ironfarm 10:30: 8 apart)
    b.on([2, yy, -4], DUST)?;
    b.on([2, yy, -3], DUST)?;
    b.on([-1, yy, -5], &rep("north", 4))?; // S3: 16 gt more (+22)
    b.on([-1, yy, -4], &rep("north", 4))?;
    b.on([-1, yy, -3], DUST)?;
    for x in [5, 2, -1] {
        b.put([x, yy, -2], "minecraft:waxed_copper_bulb[lit=false,powered=false]")?;
        b.on([x, yy, -1], &comp("north", "compare"))?; // reads the bulb, out south into the control
    }
    for x in -1..=5 {
        b.on([x, 0, -2], DUST)?; // the reset line on top of the bulbs
    }
    b.put([5, 0, -3], STONE)?; // keeps the reset line off the set dust below (S1, S2, S3)
    b.put([2, 0, -3], STONE)?;
    b.put([-1, 0, -3], STONE)?;
    b.ports.insert("input.stand_reset".into(), [6, 0, -2]);
    // ---- line 4, the stand brake (ironfarm 09:31): all 3 stand controls OFF 26 gt (redstone terms)
    // after the release run's detector rail (-3,1,4) powers. R0 (-4,1,4) reads the detector into the
    // stone (-5,1,4); dust carries it to the stone (-8,1,1); the repeater (-9,1,1) into an edge
    // comparator (-10,1,1) (side: the same 6 gt late) makes one short reset pulse per cart: the first
    // cart's brakes the stand, the others' find it braked (an off-delay ring merging the carts had a
    // phase-dependent latency, which moved the brake off the 2-tick window); then R4 R3 R1 R1 and
    // dust onto the reset line: 2 + 2 + 2 + 8 + 6 + 2 + 2 + (the bulb's reader) 2 = 26
    b.on([-4, 1, 4], &rep("east", 1))?;
    b.put([-5, 1, 4], STONE)?;
    // the burst merge, repeaters only (deterministic): the detector's stone (-5,1,4) runs west
    // through 4 delay-2 repeaters (each stretches a short pulse to 4 gt); their stones tap north into
    // an OR line along z 2, so the train's 3 carts (about 12 gt apart) make one continuous signal
    // from the first cart + 4 to the last + 20
    let mut x = -6;
    for _ in 0..4 {
        b.on([x, 1, 4], &rep("east", 2))?;
        b.put([x - 1, 1, 4], STONE)?;
        b.on([x - 1, 1, 3], &rep("south", 1))?; // input from the stone at z 4, out into z 2
        x -= 2;
    }
    for x in -13..=-5 {
        b.on([x, 1, 2], DUST)?;
    }
    b.on([-5, 1, 1], DUST)?; // the OR line turns south into the edge input stone
    b.put([-5, 1, 0], STONE)?;
    let _ = offdelay;
    // the edge: comparator (-7,1,0) = the merged signal (repeater (-6,1,0)) minus itself 4 gt later
    // (repeater (-5,1,-1), dust (-5..-7,1,-2), repeater (-7,1,-1) into its north side)
    b.on([-6, 1, 0], &rep("east", 1))?;
    b.on([-7, 1, 0], &comp("east", "subtract"))?;
    b.on([-5, 1, -1], &rep("south", 1))?;
    for x in [-5, -6, -7] {
        b.on([x, 1, -2], DUST)?;
    }
    b.on([-7, 1, -1], &rep("north", 2))?;
    // then R4 (-8,1,0), R1 (-9,1,-1) and the z -4 line (R1 at (-6,1,-4)) onto the reset line:
    // 2 (R0) + 4 + 2 (tap) + 2 + 2 (edge) + 8 + 4 + 2 + (the bulb's reader) 2 = 28: ironfarm's
    // bench +27 (it brakes after the tick it reads, so its carts feel bench +N on tick +N+1; the
    // redstone's +26 behaved like his +25 and missed slot 3's middle cart)
    b.on([-8, 1, 0], &rep("east", 4))?;
    b.on([-9, 1, 0], DUST)?;
    b.on([-9, 1, -1], &rep("south", 2))?;
    b.on([-9, 1, -2], DUST)?;
    b.on([-9, 1, -3], DUST)?;
    // (along z -4: the loop file's output port (the chest) is (-5,1,-3))
    for x in -9..=-2 {
        if x == -6 {
            b.on([x, 1, -4], &rep("west", 1))?;
        } else {
            b.on([x, 1, -4], DUST)?;
        }
    }
    b.on([-2, 1, -3], DUST)?;
    b.on([-2, 1, -2], DUST)?;
    // ---- merge into the loop file: the 21 controls become stone (the stand's stay unwired here)
    let mut d = Design::load(loop_file)?;
    let controls: [P3; 21] = [
        [0, 1, 25], [0, 0, 29], [0, -1, 33], [0, -1, 36], [0, -1, 39], [0, -1, 42], [0, -1, 45], [-1, -1, 47],
        [-3, -1, 46], [-3, -1, 43], [-3, -1, 40], [-3, -1, 37], [-3, -1, 34], [-3, -1, 31], [-3, -1, 28], [-3, -1, 25], [-3, -1, 22], [-3, -1, 19],
        [5, -1, 0], [2, -1, 0], [-1, -1, 0],
    ];
    for c in controls {
        match d.blocks.get(&c).map(|s| s.as_str()) {
            Some("minecraft:glass") => {
                d.blocks.insert(c, STONE.into());
            }
            other => bail!("control {c:?} is {other:?}, not glass: the loop file changed"),
        }
    }
    for (p, s) in &b.blocks {
        if let Some(old) = d.blocks.get(p) {
            if old != s {
                bail!("cell {p:?}: the loop file has {old}, the controller wants {s}");
            }
        }
        d.blocks.insert(*p, s.clone());
    }
    for (p, n) in &b.nbt {
        d.block_entities.insert(*p, n.clone());
    }
    // (no ports: a mob-farm design runs by itself; the runner takes the cells as flags)
    let reset = b.ports.get("input.stand_reset").copied();
    // the probe file next to it: the controller's blocks and the 21 controls alone, the controls as
    // output ports (`release-trace` prints when each one is powered)
    let mut probe = Design::parse("name: carousel-release-loop-probe\n")?;
    probe.blocks = b.blocks.clone();
    probe.block_entities = b.nbt.clone();
    let names = ["q00", "q01", "q02", "q03", "q04", "q05", "q06", "q07", "q08", "q09", "q10", "q11", "q12", "q13", "q14", "q15", "q16", "q17", "s1", "s2", "s3"];
    for (c, n) in controls.iter().zip(names) {
        probe.blocks.insert(*c, STONE.into());
        probe.ports.insert(format!("output.{n}"), *c);
    }
    probe.ports.insert("input.golem".into(), golem);
    probe.save(&out.with_extension("probe.txt"))?;
    d.name = "carousel-release-loop-wired".into();
    d.save(out)?;
    println!("{}: {} blocks ({} the controller's), golem input at {:?}, stand reset at {:?}", out.display(), d.blocks.len(), b.blocks.len(), golem, reset);
    Ok(())
}
