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
    // (K = 2 with 23 items: BUSY to ~92 gt; `MCTAS_BUSY_ITEMS` measures others: 22 items read 1,
    // BUSY ~98)
    let busy_items: u32 = std::env::var("MCTAS_BUSY_ITEMS").ok().and_then(|v| v.parse().ok()).unwrap_or(23);
    b.nbt.insert([3, y, -2], format!("{{Items:[{{Slot:0b,id:\"minecraft:stone\",count:{busy_items}}}]}}"));
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
    // ---- v2: the chain's pulse shortener. C (5,-5) = REL (rear, the dust (5,-4)) minus REL 10 gt
    // late (side: R4 (6,-3) off the REL dust, dust (7,-3..-5), R1 (6,-5)): the chain carries a
    // 10-gt copy of REL (ironfarm/ironspec 11:12: 10-gt advance pulses). REL itself stays 12 gt
    // (the refill and BUSY are unchanged)
    b.on([5, y, -5], &comp("south", "subtract"))?;
    b.on([6, y, -3], &rep("west", 4))?;
    for c in [[7, y, -3], [7, y, -4], [7, y, -5]] {
        b.on(c, DUST)?;
    }
    b.on([6, y, -5], &rep("east", 1))?;
    // ---- the delay chain along x 5 from z -6
    let mut z = -6;
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
                    b.on([6, y, z], &rep("west", 1))?; // out east, dust (7,z) (8,z) into the reset line (x 9)
                    b.on([7, y, z], DUST)?;
                    b.on([8, y, z], DUST)?;
                }
                stones.push((z, f));
            }
        }
        z -= 1;
    }
    // the release group: REL through R_g (3,-4) (a diode, so nothing flows back into REL; at x 3, so
    // the shortener's west side (4,-5) stays empty) and the group taps
    b.on([3, y, -4], &rep("south", 1))?;
    for zz in group_to..=-5 {
        b.on([3, y, zz], DUST)?;
    }
    // the bulb's reset line: x 9 from the farthest reset tap, restored at z -5 (south of every tap,
    // so all taps reach the bulb with the same delay and consecutive windows overlap), then west
    // along z -1 and into the bulb
    let far = stones.iter().filter(|s| s.1 & RS != 0).map(|s| s.0).min().unwrap_or(-1);
    for zz in far..=-1 {
        if zz == -5 {
            b.on([9, y, zz], &rep("north", 1))?;
        } else {
            b.on([9, y, zz], DUST)?;
        }
    }
    b.on([8, y, -1], DUST)?;
    b.on([7, y, -1], DUST)?;
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
    // v2 (the shortener C at z -5 gives the first 2 gt): stones at REL + 8, 16, 24 (group), 32,
    // 38 (A1), 46, 48 (the merge window to + 58 with 10-gt copies), 58 (A2), 78 (A3)
    let chain = [R(3), S(G | RS), R(4), S(G | RS), R(4), S(G | RS), R(4), S(RS), R(3), S(A | RS), R(4), S(RS), R(1), S(RS), R(3), R(2), S(A), R(4), R(4), R(2), S(A)];
    let (stones, _) = core(&mut b, &chain, -11)?;
    b.on([2, y, -11], DUST)?;
    b.ports.insert("output.group".into(), [1, y, -11]);
    let last_a = stones.iter().filter(|s| s.1 & A != 0).map(|s| s.0).min().unwrap_or(0);
    b.on([3, y, last_a - 1], DUST)?;
    b.ports.insert("output.adv".into(), [3, y, last_a - 2]);
    save(b, out, "carousel-release-controller")?;
    // the at-rest state (wire shapes, the torch, the two comparators on at rest with their outputs),
    // so the file works placed raw, as a player builds it (the parent / tick-exact 11:33)
    let mut d = Design::load(out)?;
    let cells: Vec<P3> = d.blocks.keys().copied().collect();
    let (states, outs) = settle(out, &cells)?;
    apply_settled(&mut d, &states, &outs);
    d.save(out)?;
    Ok(())
}

/// The design's at-rest block states after the judge's placement and 40 quiet ticks (no input):
/// each cell's state, and every comparator's stored output.
pub(crate) fn settle(design: &Path, cells: &[P3]) -> Result<(BTreeMap<P3, String>, BTreeMap<P3, u8>)> {
    use anyhow::anyhow;
    let c = circuit::Circuit::load(design)?;
    let cc = c.with_consumers().map_err(|e| anyhow!("{e:#}"))?;
    let (from, to) = circuit::run::watch_box(&c, 3);
    let mut states = BTreeMap::new();
    let mut outs = BTreeMap::new();
    let res = clock::sim::drive_live(&cc.clock, &[], 41, from, to, |t, w| {
        if t == 40 {
            for p in cells {
                let bp = redsim::pos::BlockPos::new(p[0], p[1], p[2]);
                states.insert(*p, w.get(bp).to_string());
                if let Some(o) = w.comparator_out.get(&bp) {
                    outs.insert(*p, *o);
                }
            }
            return false;
        }
        true
    });
    if let Err((_, why)) = res {
        bail!("settle: {why}");
    }
    Ok((states, outs))
}

/// Writes the settled states into the design (the cells settled), and each comparator's output as
/// its block entity (`OutputSignal`), so a raw paste starts at rest.
pub(crate) fn apply_settled(d: &mut Design, states: &BTreeMap<P3, String>, outs: &BTreeMap<P3, u8>) {
    for (p, s) in states {
        if let Some(old) = d.blocks.get_mut(p) {
            if !old.starts_with("minecraft:hopper") {
                *old = s.clone();
            }
        }
    }
    for (p, o) in outs {
        if *o > 0 {
            d.block_entities.insert(*p, format!("{{OutputSignal:{o}}}"));
        }
    }
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
pub fn generate_loop(out: &Path, loop_file: &Path, offdelay: &Path, siding: Option<&Path>, siding_ctl: Option<&Path>, pad: Option<&Path>) -> Result<()> {
    // (farmopt's wired pocket stand, `stand_wired=1` in the base's `queue:` line: the release
    // staggered, see the release group below)
    let base_meta = Design::load(loop_file)?.meta;
    let stagger = base_meta.get("queue").is_some_and(|q| q.contains("stand_wired=1"));
    // the chain, stretched so the group line (x -5, z 15..25) and the advance line (z 27..) do not
    // touch; the advance taps 2 gt early for the feeder repeaters (+36/56/76 -> +38/58/78 at the
    // controls); reset taps 8..44 keep the merge window to ~+58. With the stagger a fourth group
    // tap at +30 (global z 25) holds the release on to +44: the front slot's window must outlast
    // the back cart's crossing of its slope (2 x stagger + 30 = 38 gt or more, farmopt 16:30)
    let a1_run = if stagger { [R(1), D, R(2), S(G), D, S(A | RS)] } else { [R(1), D, R(2), D, D, S(A | RS)] };
    let mut chain = vec![
        R(3), S(G | RS), // +8 (after the shortener C at z -5)
        R(4), S(G | RS), // +16
        R(4), S(G | RS), // +24
    ];
    chain.extend(a1_run); // +30 (A1; zero skew pads every slot to row A's path)
    chain.extend([
        R(4), S(RS), // +38
        R(4), S(RS), // +46
        R(1), S(RS), // +48 (10-gt copies: the merge window to + 58)
        R(1), S(A), // +50 (A2)
        R(4), R(4), R(2), S(A), // +70 (A3)
    ]);
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
    if siding_ctl.is_some() {
        // (a') the fallback armed at the first darkness (the parent 18:47: a farm with the bed
        // siding gets no golem before a rider has slept, and no rider sleeps before the loop has
        // moved): an inverted daylight detector under the open sky (glass over it: a golem's
        // legacy spawn takes a daylight detector for a floor, never glass, and the sky light comes
        // through) at (-23, -1, 4), its signal east along z 4 through 14 repeaters of 4 rt into
        // ARMED's loop dust (-8, -1, 4): ARMED set 112 gt after the detector first reads darkness
        // (dusk, rain by day, or the detector placed at night as the build's last step). The delay
        // lets a golem that comes within ~100 gt of that moment release first, so a farm whose
        // golems already flow locks on them with no fallback release just before (built in order
        // with the detector going down at night, the fallback fired at once and the first golem
        // waited out BUSY: ironspec 20:35); a fresh farm (no golem before a nap) starts 112 gt
        // later. A golem still sets ARMED whenever one comes first. ARMED is a block-state latch:
        // an unload and reload keeps it.
        b.put([-23, -1, 4], "minecraft:daylight_detector[inverted=true,power=0]")?;
        b.put([-23, 0, 4], "minecraft:glass")?;
        for x in -22..=-9 {
            b.on([x, -1, 4], &rep("west", 4))?;
        }
    }
    let golem = at(m.ports["input.golem"]);
    let a3 = at([5, 1, stones.iter().filter(|s| s.1 & A != 0).map(|s| s.0).min().unwrap_or(0)]);
    let yy = -1;
    // ---- the release group (slots 15..17 at x -3, z 19/22/25): spurs east from the group line;
    // with farmopt's wired pocket stand (`stand_wired=1`: it counts the train at D2 and needs its
    // carts 11-13 gt apart, farmopt 16:15) the release is staggered: slot 16's spur a repeater 4
    // gt, slot 15's 8 gt (both edges), the head's a dust; the group line then carries the release
    // alone and the advance reaches the three controls from the east (a line along x -1, fed off
    // the advance line at z 27 under the glass at (-3, -1, 27), each through a repeater of 8 gt:
    // with the advance diode +8 gt for row A's zero skew)
    if stagger {
        b.on([-4, yy, 19], DUST)?;
        b.on([-4, yy, 22], &rep("west", 2))?;
        b.on([-4, yy, 25], &rep("west", 4))?;
    } else {
        for z in [19, 22, 25] {
            b.on([-4, yy, z], DUST)?;
        }
    }
    // (zero skew, ironfarm 10:16: every queue control's advance on one tick; the paths differ, so
    // each feeder's delay pads its path to the slowest (row A, release + 44): the group's advance
    // diode +6, row B's head feeders +6, slots 8-9 +4, the turn slot +2)
    if stagger {
        // the advance's own path to the group (see above): a repeater under the glass restores
        // the pulses injected further down the advance line (2 gt: the feeders 6 gt, 8 in all); it
        // climbs to y 0 along x -1 and each feeder powers the slot's rail (-3, 0, z) from the east
        // (into the control block, a repeater would power the head's dust spur beside it: the
        // advance back into the group line, through the stagger repeaters)
        b.on([-4, yy - 1, 27], DUST)?;
        b.on([-3, yy - 1, 27], &rep("west", 1))?;
        b.on([-2, yy - 1, 27], DUST)?;
        b.on([-1, yy, 27], DUST)?;
        for z in (19..=26).rev() {
            b.on([-1, yy + 1, z], DUST)?;
        }
        for z in [19, 22, 25] {
            b.on([-2, yy + 1, z], &rep("east", 3))?;
        }
    } else {
        b.on([-5, yy, 26], &rep("south", 4))?; // the advance into the group net (a diode)
    }
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
    // ---- the descending tail slots (ironfarm 9bd8f2ff9 / a5f46f390): slot 2 at (0,-1,33), slot 1
    // at (0,0,29), slot 0 at (0,1,25), each fed by a repeater at x 1. Each slot has its own net
    // (ironfarm 10:58: slot 2 OFF t0 + 100, slot 1 + 118, slot 0 + 120). The advance reaches each
    // net through its own diode (+2, the tail's skew; ironfarm 10:30: fine): N2 from the hub
    // (2,-1,35) through (2,-1,34); N1 from the z 36 branch through (4,-1,35), down x 4 at y -1 and
    // up to (4..2,0,29); N0 from the z 36 branch through (6,-1,35), x 6 at y -1, up to y 1 along
    // z 25
    b.on([2, yy, 35], DUST)?;
    b.on([2, yy, 34], &rep("south", 1))?;
    for c in [[2, -1, 33], [2, -1, 32]] {
        b.on(c, DUST)?;
    }
    b.on([1, -1, 33], &rep("east", 1))?;
    for x in 3..=6 {
        b.on([x, yy, 36], DUST)?;
    }
    b.on([4, yy, 35], &rep("south", 1))?; // N1's diode
    for z in 25..=34 {
        if z != 29 {
            b.on([4, yy, z], DUST)?; // z 25..28: N1's window from the z 24 line; z 30..34: its advance
        }
    }
    for c in [[4, 0, 29], [3, 0, 29], [2, 0, 29]] {
        b.on(c, DUST)?;
    }
    b.on([1, 0, 29], &rep("east", 1))?;
    b.on([6, yy, 35], &rep("south", 1))?; // N0's diode
    for z in 27..=34 {
        b.on([6, yy, z], DUST)?;
    }
    for c in [[6, 0, 26], [6, 1, 25], [5, 1, 25], [4, 1, 25], [3, 1, 25], [2, 1, 25]] {
        b.on(c, DUST)?;
    }
    b.on([1, 1, 25], &rep("east", 1))?;
    // ---- the tail windows (at the controls, golem-relative; each merged with the tail's 3rd advance
    // at + 84..+ 96): the chain from A3 (REL + 70) runs under the loop at y -3 along z 43, climbs at
    // x 8 to y -1 and runs along x 8 toward -z, stones at golem + 86 (z 34), + 96 (z 31), + 104
    // (z 29), + 106 (z 27).
    // - N0: taps (delay-2 repeaters at x 7) into its x 6 line, control = stone + 6: stones 86/96/106,
    //   ON + 92..+ 124 (t0 + 120).
    // - N1: taps (repeaters at x 9) into an OR line along x 10, restored at (10,-1,25), west along
    //   z 24, up x 4 into N1, control = stone + 6: stones 86/96/104, ON + 92..+ 122 (t0 + 118).
    // - N2: a branch off the z 43 line (golem 82) along x 3 at y -3 (R3 at z 41), up a step to
    //   (2,-2,30), a repeater (2,-2,31) into N2's support (2,-2,32), which powers N2's dust above:
    //   control ON + 92..+ 104 (t0 + 100)
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
    // (v2: 10-gt copies, so the stones overlap by 2 gt: N1 86/94/102/106 → control + 92..+ 122,
    // N0 86/94/102/108 → + 92..+ 124)
    let chain_tail: [(i32, It); 15] = [
        (40, R(2)),
        (39, D), (38, D), (37, D), (36, D), (35, D),
        (34, S(0)), // golem + 86: N1 + N0
        (33, R(4)),
        (32, S(0)), // + 94: N1 + N0
        (31, R(4)),
        (30, S(0)), // + 102: N1 + N0
        (29, R(2)),
        (28, S(0)), // + 106: N1
        (27, R(1)),
        (26, S(0)), // + 108: N0 (into its climb's support (6,-1,26))
    ];
    for (z, it) in chain_tail {
        match it {
            R(d) => b.on([8, yy, z], &rep("south", d))?,
            D if z == 26 => {} // (the chain ends at the stone)
            D => b.on([8, yy, z], DUST)?,
            S(_) => b.put([8, yy, z], STONE)?,
        }
    }
    for z in [34, 32, 30, 26] {
        b.on([7, yy, z], &rep("east", 2))?; // N0 taps (+4)
    }
    for z in [34, 32, 30, 28] {
        b.on([9, yy, z], &rep("west", 1))?; // N1 taps into x 10
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
    // N2's window
    b.on([3, -3, zl - 1], DUST)?;
    b.on([3, -3, zl - 2], &rep("south", 3))?;
    for z in 30..=zl - 3 {
        b.on([3, -3, z], DUST)?;
    }
    b.on([2, -2, 30], DUST)?;
    b.on([2, -2, 31], &rep("north", 1))?;
    // (the pocket stand (farmopt, `stand_stop=1` in the loop's `queue:` line) has its own gates: the
    // S1/S2/S3 latches and line 4 below are dropped for it)
    let base = Design::load(loop_file)?;
    let pocket = base.meta.get("queue").is_some_and(|q| q.contains("stand_stop=1"));
    let before_stand: std::collections::BTreeSet<P3> = b.blocks.keys().copied().collect();
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
    b.on([2, yy, -5], &rep("north", 3))?; // S2: 6 gt more (+12; ironfarm 10:58: 6 apart)
    b.on([2, yy, -4], DUST)?;
    b.on([2, yy, -3], DUST)?;
    b.on([-1, yy, -5], &rep("north", 4))?; // S3: 12 gt more (+18)
    b.on([-1, yy, -4], &rep("north", 2))?;
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
    if pocket {
        b.blocks.retain(|p, _| before_stand.contains(p));
        b.nbt.retain(|p, _| before_stand.contains(p));
        b.ports.remove("input.stand_reset");
    }
    // ---- merge into the loop file: the 21 controls become stone (the stand's stay unwired here)
    let mut d = base;
    let all_controls: [P3; 21] = [
        [0, 1, 25], [0, 0, 29], [0, -1, 33], [0, -1, 36], [0, -1, 39], [0, -1, 42], [0, -1, 45], [-1, -1, 47],
        [-3, -1, 46], [-3, -1, 43], [-3, -1, 40], [-3, -1, 37], [-3, -1, 34], [-3, -1, 31], [-3, -1, 28], [-3, -1, 25], [-3, -1, 22], [-3, -1, 19],
        [5, -1, 0], [2, -1, 0], [-1, -1, 0],
    ];
    let controls: &[P3] = if pocket { &all_controls[..18] } else { &all_controls[..] };
    for &c in controls {
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
    // the bed siding and its junctions (pocket stand only)
    let mut siding_cells: Vec<P3> = Vec::new();
    // (farmopt's wired pocket stand: its REL from the controller's REL)
    if d.meta.get("queue").is_some_and(|q| q.contains("stand_wired=1")) && std::env::var("MCTAS_NO_STAND_REL").is_err() {
        let cells = crate::sidinggen::stand_rel(&mut d)?;
        println!("the wired pocket stand's REL (8,-2,2): {} cells, golem +10..+50 off the controller's REL", cells.len());
        siding_cells.extend(cells);
    }
    if let Some(sp) = siding {
        if !pocket {
            bail!("--siding needs farmopt's pocket-stand loop (stand_stop=1)");
        }
        let phys = crate::sidinggen::physical(&mut d, sp)?;
        println!("siding pasted at {:?}: {} cells added; controls divert {:?}, T1 {:?}, T2 {:?}, holds {:?} (the siding's release {:?} held on)", crate::sidinggen::SIDING_AT, phys.len(), crate::sidinggen::DIVERT, crate::sidinggen::T1, crate::sidinggen::T2, crate::sidinggen::HOLDS, crate::sidinggen::RELEASE);
        siding_cells.extend(phys);
        if let Some(sched) = siding_ctl {
            let more = crate::sidinggen::wire_control(&mut d, sched)?;
            println!("siding control module at {:?}: {} cells (the scheduler, the night window, the windows, R', the visit-delay register, the routes to the 4 controls)", crate::sidinggen::MODULE_AT, more.len());
            siding_cells.extend(more);
        }
    }
    // (farmopt's portal pad, pasted at the origin, its golem pulse wired into the controller)
    let mut pad_cells: std::collections::BTreeSet<P3> = Default::default();
    if let Some(pp) = pad {
        if siding_ctl.is_none() {
            bail!("--pad needs --siding-ctl (the pad's farm is the wired farm)");
        }
        pad_cells = Design::load(pp)?.blocks.keys().copied().collect();
        let cells = crate::sidinggen::wire_pad(&mut d, pp)?;
        println!("the pad pasted at the origin ({} blocks) and its pulse (9,6,-2) wired into the controller's X: {} cells", pad_cells.len(), cells.len());
        siding_cells.extend(cells);
    }
    // ---- golem spawn cells (the brief, part 1: none off the pad wherever villagers ride who may
    // have slept): every block added here with a solid top and air over it gets a glass cover
    // (no golem spawns on glass, nor on a block under one; glass carries no power)
    {
        let base_cells: std::collections::BTreeSet<P3> = Design::load(loop_file)?.blocks.keys().copied().collect();
        // (the siding's own blocks are farmopt's: its floor is where the napper walks to the bed, a
        // glass cover there would block it; it is left as pasted)
        let s0 = crate::sidinggen::SIDING_AT;
        let in_siding = |p: &P3| (s0[0] - 7..=s0[0] + 6).contains(&p[0]) && (s0[1] - 4..=s0[1] + 4).contains(&p[1]) && (s0[2] - 3..=s0[2] + 9).contains(&p[2]);
        let solid = |s: &str| ["minecraft:stone", "minecraft:redstone_block", "minecraft:dropper", "minecraft:observer", "minecraft:target", "minecraft:smooth_stone", "minecraft:waxed_copper_bulb", "minecraft:copper_bulb"].iter().any(|k| s == *k || s.starts_with(&format!("{k}[")));
        let covers: Vec<P3> = d
            .blocks
            .iter()
            .filter(|(p, s)| !base_cells.contains(*p) && !pad_cells.contains(*p) && !(siding.is_some() && in_siding(p)) && solid(s))
            .map(|(p, _)| [p[0], p[1] + 1, p[2]])
            .filter(|up| d.blocks.get(up).is_none_or(|s| s.ends_with(":air")))
            .collect();
        for c in &covers {
            d.blocks.insert(*c, "minecraft:glass".into());
        }
        siding_cells.extend(covers.iter().copied());
        println!("golem covers: {} glass blocks over the added blocks' open tops", covers.len());
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
    // the controller's at-rest state, settled with the whole loop's blocks around it (a booster's
    // redstone block beside a wire counts), written into the loop file and the probe
    let mut sd = Design::parse("name: carousel-release-loop-settle\n")?;
    // (a pad's nether portal cells are left out: the plain redstone world does not model them, and
    // no redstone of the loop's touches them; and the daylight detectors: the file starts as the
    // runners settle it, with its detectors not yet placed (the night line reads day, ARMED unset))
    sd.blocks = d.blocks.iter().filter(|(_, s)| !s.starts_with("minecraft:nether_portal") && !s.starts_with("minecraft:daylight_detector")).map(|(p, s)| (*p, s.clone())).collect();
    sd.block_entities = d.block_entities.clone();
    sd.ports.insert("input.golem".into(), golem);
    let settle_path = out.with_extension("settle.txt");
    sd.save(&settle_path)?;
    let mut cells: Vec<P3> = b.blocks.keys().copied().filter(|p| sd.blocks.contains_key(p)).collect();
    cells.extend(siding_cells.iter().copied().filter(|p| sd.blocks.contains_key(p)));
    let settled = settle(&settle_path, &cells);
    let _ = std::fs::remove_file(&settle_path);
    let (states, outs) = settled?;
    {
        let changed: Vec<String> = siding_cells.iter().filter_map(|p| states.get(p).filter(|s| d.blocks.get(p) != Some(*s)).map(|s| format!("{p:?} {} -> {s}", d.blocks.get(p).map(|x| x.as_str()).unwrap_or("air")))).collect();
        if !changed.is_empty() {
            // (the count by default; each cell with MCTAS_LIST_SETTLED)
            println!("settling changed {} siding/junction cells{}", changed.len(), if std::env::var("MCTAS_LIST_SETTLED").is_ok() { ":" } else { " (MCTAS_LIST_SETTLED lists them)" });
            if std::env::var("MCTAS_LIST_SETTLED").is_ok() {
                for c in &changed {
                    println!("  {c}");
                }
            }
        }
    }
    apply_settled(&mut d, &states, &outs);
    apply_settled(&mut probe, &states, &outs);
    // (with the siding's control module: a probe of the whole file for `release-trace`, its night
    // forced (the daylight detector taken out), no entities, the controls and taps as ports)
    if siding_ctl.is_some() {
        let mut sp = Design::parse("name: carousel-siding-probe\n")?;
        sp.blocks = d.blocks.iter().filter(|(_, s)| !s.starts_with("minecraft:nether_portal")).map(|(p, s)| (*p, s.clone())).collect();
        sp.block_entities = d.block_entities.clone();
        let det = crate::sidinggen::DETECTOR;
        // (night forced: a redstone block for the inverted detector, 15 = night;
        // `MCTAS_PROBE_KEEP_DETECTOR` keeps it, to check the day line: release-trace's frozen day
        // time 0, sunrise, reads 0 under open sky: day, no visit)
        if std::env::var("MCTAS_PROBE_KEEP_DETECTOR").is_err() {
            sp.blocks.insert(det, "minecraft:redstone_block".into());
        }
        let m = crate::sidinggen::module_at;
        let u = crate::sidinggen::unit_at;
        sp.ports.insert("input.golem".into(), golem);
        for (n, q) in [("rel", [-2, -1, 13]), ("rel_in", m([14, -2, -4])), ("g2", m([14, -2, 4])), ("unit_t0", u([20, -6, 9])), ("divert", [9, -3, 0]), ("t1", [-15, 1, 28]), ("t2", [-16, 1, 31]), ("ka", [-21, -4, 31]), ("hold14", [-12, 1, 28]), ("hold13", [-11, 1, 31]), ("hold12", [-12, 1, 34]), ("force", crate::sidinggen::FORCE), ("ns3_in", { let f = crate::sidinggen::FORCE; [f[0] - 2, f[1], f[2] - 1] }), ("fa_rel", { let f = crate::sidinggen::FORCE; [f[0], f[1], f[2] - 2] }), ("thr", { let d = crate::sidinggen::DETECTOR; [d[0], d[1], d[2] + 13] }), ("lock", { let d = crate::sidinggen::DETECTOR; [d[0] + 1, d[1], d[2] + 12] }), ("win", m([-2, 0, 1])), ("nv_mod", m([-4, 0, 3])), ("nv_loc", u([18, -6, 10])), ("g2_loc", u([20, -6, 9]))] {
            sp.ports.insert(format!("output.{n}"), q);
        }
        if std::env::var("MCTAS_SIDING_PROBE_MORE").is_ok() {
            for (n, q) in [("r15", [-3, 0, 25]), ("r16", [-3, 0, 22]), ("r17", [-3, 0, 19]), ("q14", [-3, -1, 28]), ("q15", [-3, -1, 25]), ("q16", [-3, -1, 22]), ("q17", [-3, -1, 19]), ("q00", [0, 1, 25]), ("q01", [0, 0, 29]), ("q02", [0, -1, 33]), ("q03", [0, -1, 36]), ("stand_rel", [8, -2, 2]), ("x_tap", [0, -1, 10]), ("x_b0", [0, -1, 9]), ("x_b3", [8, -1, 9]), ("x_or", [4, -1, 7]), ("x_or8", [8, -1, 6]), ("x_dr", [8, -2, 4]), ("row7", u([23, -6, 7])), ("row1", u([23, -6, 1])), ("row_5", u([23, -6, -5])), ("step_in", u([20, -4, 12])), ("step_end", u([28, -5, 6])), ("ring_top", u([27, -5, 4])), ("p0", u([22, -6, 9])), ("p1", u([22, -6, 3])), ("p2", u([22, -6, -3])), ("or_out", [11, -4, 0]), ("ns5", [-23, -4, 30]), ("bus", [-15, -1, 33])] {
                sp.ports.insert(format!("output.{n}"), q);
            }
        }
        sp.save(&out.with_extension("siding-probe.txt"))?;
        // and the whole file with day forced (the inverted detector taken out: no night signal, no
        // visit ever), for the daytime loop checks (`queue-loop-run` knows nothing of visits)
        let mut dd = d.clone();
        dd.blocks.remove(&det);
        dd.save(&out.with_extension("day.txt"))?;
    }
    probe.save(&out.with_extension("probe.txt"))?;
    // (every daylight detector arms the controller: the inverted one its fallback (a'), the night
    // latch's its visits. A listed place-last step, after every other cell (ironspec 20:35: a
    // player placing one mid-build gets a controller running in a half-built farm for the build's
    // ~2.75 days); the built-in-order check follows the list, and the page's build steps show it.
    // Before them, R'' (its six containers with their two items, then the seven dusts on them):
    // built in order with its stock filled as placed, a pulse of its half-built step line left an
    // item in a locked hopper and the first visit opened two windows; its containers alone placed
    // last left the dusts on them shaped without their supports (R'' never stepped). Containers,
    // then dusts, onto quiet wiring: it comes up at rest)
    let dets: Vec<[i32; 3]> = d.blocks.iter().filter(|(_, b)| b.starts_with("minecraft:daylight_detector")).map(|(p, _)| *p).collect();
    {
        let toks: Vec<String> = d.meta.get("build").map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default();
        let fills = toks.iter().any(|t| t == "fills-last");
        let mut cells: Vec<String> = toks.into_iter().filter(|t| t != "last" && t != "fills-last").collect();
        let rpp: [[i32; 3]; 13] = [[27, -6, 4], [28, -6, 5], [26, -6, 5], [26, -6, 4], [28, -6, 4], [27, -6, 5], [26, -5, 4], [27, -5, 4], [28, -5, 4], [26, -5, 5], [27, -5, 5], [28, -5, 5], [28, -5, 6]];
        let ring: Vec<[i32; 3]> = rpp.iter().copied().filter(|p| d.blocks.get(p).is_some_and(|b| b.starts_with("minecraft:dropper") || b.starts_with("minecraft:hopper") || b.starts_with("minecraft:redstone_wire"))).collect();
        for p in ring.iter().chain(&dets) {
            let c = format!("{},{},{}", p[0], p[1], p[2]);
            if !cells.contains(&c) {
                cells.push(c);
            }
        }
        let mut line = format!("last {}", cells.join(" "));
        if fills {
            line.push_str(" fills-last");
        }
        d.meta.insert("build".into(), line);
    }
    // (the chunk lines the added parts cross (ironspec 21:24/21:25: the carousel at X & 15 = 6,
    // Z & 15 = 6 for loop cell 0, 0, so the lines fall just before x and z = 10 mod 16): each line
    // with the pairs of touching blocks across it, one of them added here (glass covers aside), for
    // the partial-unload faults at each line)
    {
        let base_cells: std::collections::BTreeSet<P3> = Design::load(loop_file)?.blocks.keys().copied().collect();
        let s0 = crate::sidinggen::SIDING_AT;
        let in_siding = |p: &P3| (s0[0] - 7..=s0[0] + 6).contains(&p[0]) && (s0[1] - 4..=s0[1] + 4).contains(&p[1]) && (s0[2] - 3..=s0[2] + 9).contains(&p[2]);
        let real = |s: &String| !s.starts_with("minecraft:glass") && !s.ends_with(":air");
        let mine: std::collections::BTreeSet<P3> = d.blocks.iter().filter(|(p, s)| !base_cells.contains(*p) && !pad_cells.contains(*p) && !(siding.is_some() && in_siding(p)) && real(s)).map(|(p, _)| *p).collect();
        let solid_at = |p: &P3| d.blocks.get(p).is_some_and(real);
        let mut xs: BTreeMap<i32, usize> = BTreeMap::new();
        let mut zs: BTreeMap<i32, usize> = BTreeMap::new();
        for (p, _) in d.blocks.iter().filter(|(_, s)| real(s)) {
            let w = [p[0] - 1, p[1], p[2]];
            if p[0].rem_euclid(16) == 10 && solid_at(&w) && (mine.contains(p) || mine.contains(&w)) {
                *xs.entry(p[0]).or_default() += 1;
            }
            let n = [p[0], p[1], p[2] - 1];
            if p[2].rem_euclid(16) == 10 && solid_at(&n) && (mine.contains(p) || mine.contains(&n)) {
                *zs.entry(p[2]).or_default() += 1;
            }
        }
        let (lo, hi) = mine.iter().fold(([i32::MAX; 3], [i32::MIN; 3]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])], [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])]));
        let show = |m: &BTreeMap<i32, usize>| m.iter().map(|(k, n)| format!("{}|{} ({n})", k - 1, k)).collect::<Vec<_>>().join(", ");
        println!("the added parts ({} blocks, box {:?}..{:?}) cross chunk lines (X & 15 = 6, Z & 15 = 6 at loop 0,0; touching pairs): x {}; z {}", mine.len(), lo, hi, show(&xs), show(&zs));
    }
    // (the chunk alignment header (the brief's chunk rule, 7b4d7caea): the stand area (stand,
    // approach, circuit, pad: loop x -3..9, z -6..7) inside one chunk allows loop cell 0,0 at world
    // X & 15 in 3..6 and Z & 15 in 6..8; the carousel's pick is 6, 6 (ironspec 21:24/21:25,
    // ironfarm's pick), checked against that range here; runners place the design shifted to it)
    {
        let (sx, sz) = ((-3, 9), (-6, 7));
        let (xr, zr) = ((-sx.0, 15 - sx.1), (-sz.0, 15 - sz.1));
        let pick = (6, 6);
        if !(xr.0..=xr.1).contains(&pick.0) || !(zr.0..=zr.1).contains(&pick.1) {
            bail!("the chunk pick {pick:?} is outside the stand area's range x {xr:?}, z {zr:?}");
        }
        d.meta.insert("chunk".into(), format!("0 0 0 {}-{} {}-{}", pick.0, pick.0, pick.1, pick.1));
    }
    d.name = "carousel-release-loop-wired".into();
    d.save(out)?;
    println!("{}: {} blocks ({} the controller's), golem input at {:?}, stand reset at {:?}", out.display(), d.blocks.len(), b.blocks.len(), golem, reset);
    Ok(())
}
