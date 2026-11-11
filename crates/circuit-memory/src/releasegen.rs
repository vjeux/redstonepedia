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
    // (with MCTAS_CHAIN_HEAD_RT below 3 every stone comes that many rt earlier against REL; the
    // restore takes the same rt back, so the bulb sees the reset window at main's time)
    let rs_rt: u8 = 1 + 3u8.saturating_sub(std::env::var("MCTAS_CHAIN_HEAD_RT").ok().and_then(|v| v.parse().ok()).unwrap_or(3u8))
        // (MCTAS_REL_EXIT: the bulb's own repeater (7,0) goes (the reset reaches the bulb over its
        // top, by dust), so the restore takes its 1 rt)
        + if std::env::var("MCTAS_REL_EXIT").is_ok() { 1 } else { 0 };
    for zz in far..=-1 {
        if zz == -5 {
            b.on([9, y, zz], &rep("north", rs_rt))?;
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

/// A dust path from `from` with a 1-rt repeater at each index in `reps` (facing along the path),
/// each dust run laid by `sidinggen::route_d` (its foreign-dust and stair checks), so the path's
/// delay is exactly 2 gt per listed repeater.
fn lay_link(b: &mut B, from: P3, cells: &[P3], reps: &[(usize, u8)]) -> Result<()> {
    let mut start = 0;
    let mut src = from;
    for &(r, d) in reps.iter().chain(std::iter::once(&(cells.len(), 1))) {
        if r > start {
            crate::sidinggen::route_d(b, src, &cells[start..r], 0, 0, 1)?;
        }
        if r < cells.len() {
            let p = if r == 0 { from } else { cells[r - 1] };
            let c = cells[r];
            let facing = match (c[0] - p[0], c[2] - p[2]) {
                (1, 0) => "west",
                (-1, 0) => "east",
                (0, 1) => "north",
                _ => "south",
            };
            if c[1] != p[1] || cells.get(r + 1).is_some_and(|n| n[1] != c[1] || [n[0] - c[0], n[2] - c[2]] != [c[0] - p[0], c[2] - p[2]]) {
                bail!("link: the repeater at {c:?} is not on a straight flat run");
            }
            b.on(c, &rep(facing, d))?;
            src = c;
            start = r + 1;
        }
    }
    Ok(())
}

/// One routed feed with an exact delay: from a free side of one of the line cells `srcs` (each with
/// its dust run since the last refresh) to `end`, `total` rt laid along it (`near`: components it
/// may touch). Returns the path's cells with their runs (later feeds may branch off them). No dust
/// over a cell of `corridor` (its support would cut a rider's sight line to a zombie).
fn feed(b: &mut B, srcs: &[(P3, usize)], end: P3, total: u8, near: &[P3], lo: P3, hi: P3, what: &str, corridor: &std::collections::BTreeSet<P3>) -> Result<Vec<(P3, usize)>> {
    use crate::autoroute::{find, rider_space, Limits};
    let mut avoid = rider_space(&b.blocks);
    for c in corridor {
        avoid.insert([c[0], c[1] + 1, c[2]]);
    }
    // (no support right over the lower dust of an earlier route's stair: a stone there caps the
    // stair, which the router checks for its own route only (found 07:59: the G_P side feed's
    // support (5,2,33) cut the G0 rear feed's step (5,1,33) -> (5,2,32)))
    let is_dust = |p: &P3| b.blocks.get(p).is_some_and(|s| s.contains("redstone_wire"));
    for (p, s) in &b.blocks {
        if s.contains("redstone_wire") && [[1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1]].iter().any(|d| is_dust(&[p[0] + d[0], p[1] + d[1], p[2] + d[2]])) {
            avoid.insert([p[0], p[1] + 2, p[2]]);
        }
    }
    // (the end is the fixed part's input cell: its support is that part's, declared with them)
    avoid.remove(&end);
    avoid.remove(&[end[0], end[1] - 1, end[2]]);
    let mut why: Vec<String> = Vec::new();
    for &(src, run0) in srcs {
        if run0 >= 13 {
            continue;
        }
        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1]] {
            let s = [src[0] + d[0], src[1] + d[1], src[2] + d[2]];
            if b.blocks.get(&s).is_some_and(|x| !x.ends_with(":air")) {
                continue;
            }
            let mut n: Vec<P3> = near.to_vec();
            n.push(src);
            let lim = Limits { lo, hi, avoid: avoid.clone(), near_ok: n.into_iter().collect() };
            let p = match find(b, s, end, &lim) {
                Ok(p) => p,
                Err(e) => {
                    why.push(format!("from {s:?}: {e}"));
                    continue;
                }
            };
            let mut t = new_b();
            t.blocks = b.blocks.clone();
            t.ports = b.ports.clone();
            t.nbt = b.nbt.clone();
            match crate::sidinggen::lay_exact(&mut t, src, &p, run0, total) {
                Ok(reps) => {
                    eprintln!("escapement {what}: {} cells from {src:?} (run {run0}) to {end:?}, {total} rt ({reps:?})", p.len());
                    *b = t;
                    let mut run = run0;
                    let mut out = Vec::new();
                    for c in &p {
                        run = if reps.contains(c) { 0 } else { run + 1 };
                        out.push((*c, run));
                    }
                    return Ok(out);
                }
                Err(e) => why.push(format!("from {s:?} ({} cells): {e}", p.len())),
            }
        }
    }
    bail!("escapement {what}: no feed to {end:?} ({})", why.iter().take(40).cloned().collect::<Vec<_>>().join("; "))
}

/// MCTAS_R1 (ironfarm 07:15): R1's escapement driven from the register's advance (slot 0's, on the
/// x 2 line 2 gt before its control: A_l) and A3 alone (the A3 tap's z 43 line, y -3). Times at the
/// gates, golem-relative, slot 0's advance at its control A_c = A_l + 2:
/// - G_P (0,2,32), 2 gt at A_c + 6 (each advance): CP, a subtract comparator at (1,2,32) facing
///   east into the gate, its rear A_l + 6, its side A_l + 8 (each through a repeater).
/// - G0 (0,3,31): C0 at (2,3,31) through its driver (1,3,31): its rear the advance at A_l + 8, so
///   A_c + 10 for its 10 gt (ironfarm 07:01: a timed window of 2..14 gt admits the same); its side
///   A3 alone, timed to end at A3_c + 12, so the third advance's window keeps only its tail from
///   A3_c + 16 (ironfarm's K2: the third at G_P + 10).
fn escapement(b: &mut B, yy: i32, zl: i32, ld: &Design) -> Result<()> {
    // (routed among the loop's own blocks: its rails, their riders' space and the escapement's
    // gates are in this corner; only the cells the escapement adds go back into the controller)
    let lp = &ld.blocks;
    let mut t = new_b();
    t.blocks = b.blocks.clone();
    t.ports = b.ports.clone();
    t.nbt = b.nbt.clone();
    for (p, s) in lp {
        t.blocks.entry(*p).or_insert_with(|| s.clone());
    }
    // (the riders' sight lines to the zombie posts: no support of the escapement's in them)
    let zombies: Vec<[f64; 3]> = ld.entities.iter().filter(|e| e.nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie"))).map(|e| e.pos).collect();
    let corridor = crate::autoroute::sight_corridor(lp, &zombies);
    escapement_in(&mut t, yy, zl, &corridor)?;
    for (p, s) in &t.blocks {
        if lp.get(p) != Some(s) || b.blocks.contains_key(p) {
            b.blocks.insert(*p, s.clone());
        }
    }
    for (p, n) in &t.nbt {
        b.nbt.insert(*p, n.clone());
    }
    Ok(())
}

fn escapement_in(b: &mut B, yy: i32, zl: i32, corridor: &std::collections::BTreeSet<P3>) -> Result<()> {
    let cmp = |f: &str| format!("minecraft:comparator[facing={f},mode=subtract,powered=false]");
    let need_free = |b: &B, p: P3| -> Result<()> {
        if b.blocks.get(&p).is_some_and(|s| !s.ends_with(":air") && s != STONE) {
            bail!("escapement: {p:?} holds {:?}", b.blocks.get(&p));
        }
        Ok(())
    };
    let in_corridor = std::cell::RefCell::new(Vec::<String>::new());
    let put = |b: &mut B, p: P3, s: String| -> Result<()> {
        need_free(b, p)?;
        need_free(b, [p[0], p[1] - 1, p[2]])?;
        if corridor.contains(&[p[0], p[1] - 1, p[2]]) {
            in_corridor.borrow_mut().push(format!("{:?} (under {p:?})", [p[0], p[1] - 1, p[2]]));
        }
        b.blocks.insert([p[0], p[1] - 1, p[2]], STONE.into());
        b.blocks.insert(p, s);
        Ok(())
    };
    // A off the x 2 line through a repeater (3, yy, 38) into (4, yy, 38); A3 alone off the z 43 line
    // through (8, -3, 42) into (8, -3, 41). MCTAS_ESC_D4 = the A3 side chain's rt, set from the trace)
    let d4: u8 = std::env::var("MCTAS_ESC_D4").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    // G_P: CP into the gate from its east side (ironfarm 07:54: the tail's zombie post moved to x 6;
    // the east driver's supports still cut 8 slot-0/R lines to that zombie, accepted pending the
    // integrated loop's stamp and panic lines: MCTAS_SIGHT_ACCEPT names them). Its rear A_l + 6:
    // the hub's repeater, the chain's 1 and its own 1; its side A_l + 8: 1 + 2 + 1
    put(b, [1, 2, 32], cmp("east"))?;
    put(b, [2, 2, 32], rep("east", 1))?;
    put(b, [1, 2, 33], rep("south", 1))?;
    put(b, [1, 2, 34], rep("south", 2))?;
    // G0: C0 through its driver into the gate; rear 4 rt from A_l (hub 1, chain 2, its repeater 1);
    // its side A3 alone (the third advance's window cut to its tail, A3_c + 16 on)
    put(b, [1, 3, 31], rep("east", 1))?; // the driver into G0
    put(b, [2, 3, 31], cmp("east"))?;
    put(b, [3, 3, 31], rep("east", 1))?;
    put(b, [4, 3, 31], rep("east", 2))?;
    put(b, [2, 3, 30], rep("north", 1))?;
    put(b, [2, 3, 29], rep("north", d4))?;
    // the hubs
    let x2 = [2, yy, 38];
    if !b.blocks.get(&x2).is_some_and(|s| s.contains("redstone_wire")) {
        bail!("escapement: the x 2 line at {x2:?} is {:?}", b.blocks.get(&x2));
    }
    put(b, [3, yy, 38], rep("west", 1))?;
    let ah = [4, yy, 38];
    put(b, ah, DUST.into())?;
    let a3l = [8, -3, zl];
    if !b.blocks.get(&a3l).is_some_and(|s| s.contains("redstone_wire")) {
        bail!("escapement: the z {zl} line's end {a3l:?} is {:?}", b.blocks.get(&a3l));
    }
    put(b, [8, -3, zl - 1], rep("south", 1))?; // (facing = its input side: the z 43 line)
    let a3h = [8, -3, zl - 2];
    put(b, a3h, DUST.into())?;
    if !in_corridor.borrow().is_empty() {
        eprintln!("escapement: supports in the riders' sight corridor: {}", in_corridor.borrow().join(", "));
    }
    if std::env::var("MCTAS_ESC_MAP").is_ok() {
        // (C: a sight-corridor cell (no support there), # a block, . free; x -5..6 across, z down)
        for y in -4..=4 {
            eprintln!("y {y} (x -5..6 across, z 26..40 down)");
            for z in 26..=40 {
                let row: String = (-5..=6)
                    .map(|x| {
                        let p = [x, y, z];
                        if b.blocks.get(&p).is_some_and(|s| !s.ends_with(":air")) {
                            '#'
                        } else if corridor.contains(&p) {
                            'C'
                        } else {
                            '.'
                        }
                    })
                    .collect();
                eprintln!("  {z:3} {row}");
            }
        }
    }
    let lo = [-4, yy - 3, 24];
    let hi = [10, 5, zl + 1];
    let mut anet: Vec<(P3, usize)> = vec![(ah, 0)];
    let p = feed(b, &anet, [1, 2, 35], 0, &[[1, 2, 34]], lo, hi, "G_P side", corridor)?;
    anet.extend(p);
    let p = feed(b, &anet, [5, 3, 31], 0, &[[4, 3, 31]], lo, hi, "G0 rear", corridor)?;
    anet.extend(p);
    // (the A net's own dust may be joined: the same pulse at the same tick). CP's rear: the feed
    // carries 1 rt onto (3,2,32), its repeater's input; failing that, a chain repeater there and a
    // plain feed onto (4,2,32) (the same 3 rt from A_l either way)
    let mut near: Vec<P3> = vec![[2, 2, 32]];
    near.extend(anet.iter().map(|(c, _)| *c));
    if let Err(e1) = feed(b, &anet, [3, 2, 32], 1, &near, lo, hi, "G_P rear (1 rt on the feed)", corridor) {
        put(b, [3, 2, 32], rep("east", 1))?;
        let mut near: Vec<P3> = vec![[3, 2, 32]];
        near.extend(anet.iter().map(|(c, _)| *c));
        feed(b, &anet, [4, 2, 32], 0, &near, lo, hi, "G_P rear", corridor).map_err(|e2| anyhow::anyhow!("{e1} | {e2}"))?;
    }
    let mut ok = false;
    let mut errs = Vec::new();
    for k in 0..=1u8 {
        match feed(b, &[(a3h, 0)], [2, 3, 28], k, &[[2, 3, 29]], lo, hi, "G0 side (A3)", corridor) {
            Ok(_) => {
                if k > 0 {
                    eprintln!("escapement: the A3 feed took {k} rt (MCTAS_ESC_D4 counts the chain's alone)");
                }
                ok = true;
                break;
            }
            Err(e) => errs.push(e.to_string()),
        }
    }
    if !ok {
        bail!("{}", errs.join(" | "));
    }
    Ok(())
}

/// The permission (ironspec 06:54 / 07:53, MCTAS_PERMIT): NOT-PERMIT into GO's free west side
/// (world (-3,-5,25)) through a lane of 7 repeaters along z 25 from (-9,-5,25) (dust in those cells
/// would join REL's line, the readback or the reset tail; every other approach powers the REQ
/// bulb), from an OR node (-10,-5,25) of three inputs: the stand-emptied latch, the train-arrived
/// latch and the declared `permit_hold` input (a level: departure blocked or no tail room). Each latch
/// stores BLOCK in a loop (a repeater refreshing it, a subtract comparator the clear input breaks):
/// set by REL delayed past REL's end (the launch), cleared by its evidence input; power-on (all off)
/// permits, a reload keeps the loop's state. A golem during NOT-PERMIT stays in REQ; EXPIRE is a
/// level: one launch when the permission returns.
fn permit_fixed(b: &mut B) -> Result<()> {
    let cmp = |f: &str| format!("minecraft:comparator[facing={f},mode=subtract,powered=false]");
    let place = |b: &mut B, p: P3, s: String| -> Result<()> {
        if b.blocks.get(&p).is_some_and(|t| t.contains("glass")) {
            b.blocks.remove(&p);
        }
        for q in [p, [p[0], p[1] - 1, p[2]]] {
            if b.blocks.get(&q).is_some_and(|t| !t.ends_with(":air") && t != STONE) {
                bail!("permission: {q:?} holds {:?}", b.blocks.get(&q));
            }
        }
        b.blocks.insert([p[0], p[1] - 1, p[2]], STONE.into());
        b.blocks.insert(p, s);
        Ok(())
    };
    // the lane (facing = the input side: west)
    for x in -9..=-3 {
        place(b, [x, -5, 25], rep("west", 1))?;
    }
    // the OR node and its three inputs
    place(b, [-10, -5, 25], DUST.into())?;
    place(b, [-11, -5, 25], rep("west", 1))?; // the stand-emptied latch's Q, from (-12,-5,25)
    place(b, [-10, -5, 24], rep("north", 1))?; // the train-arrived latch's Q, from (-10,-5,23)
    place(b, [-10, -5, 26], rep("south", 1))?; // the inverted input.permit, from (-10,-5,27)
    // (input.permit's inverter: placed after the latches, below)
    // the two latches (origin o: Rl at o facing south, the loop o+(0..2, 0, -1..2), Cl at o+(2,0,0)
    // facing north, its clear repeater o+(3,0,0) from the port o+(4,0,0), the set chain o+(1,0,3)
    // (2 rt) and o+(1,0,4) (4 rt) from o+(1,0,5), Q's repeater o+(-1,0,-1) out to o+(-2,0,-1))
    for (o, clear) in [([-15, -5, 28], "input.stand_empty"), ([-15, -5, 19], "input.train_arrived")] {
        let a = |d: [i32; 3]| [o[0] + d[0], o[1] + d[1], o[2] + d[2]];
        place(b, a([0, 0, 0]), rep("south", 1))?;
        for d in [[0, 0, -1], [1, 0, -1], [2, 0, -1], [2, 0, 1], [2, 0, 2], [1, 0, 2], [0, 0, 2], [0, 0, 1]] {
            place(b, a(d), DUST.into())?;
        }
        place(b, a([2, 0, 0]), cmp("north"))?;
        place(b, a([3, 0, 0]), rep("east", 1))?;
        b.ports.insert(clear.into(), a([4, 0, 0]));
        place(b, a([1, 0, 3]), rep("south", 2))?;
        place(b, a([1, 0, 4]), rep("south", 4))?;
        place(b, a([-1, 0, -1]), rep("east", 1))?;
    }
    // input.permit (ironspec 08:54: positive and fail-closed, the inverter at the head): powered =
    // permit. INV (-10,-5,27), a subtract comparator, outputs 15 - permit straight into the OR
    // node's south input (-10,-5,26): its rear a repeater (-10,-5,28) from a redstone block
    // (-10,-5,29) (the constant enters through a diode: a block or dust on the stand-empty port
    // (-11,-5,28) beside it can't lock or feed it), its side a repeater (-11,-5,27) reading a stone
    // (-12,-5,27) that the port's landing dust (-12,-4,27) powers weakly (so the stone can't power
    // the stand-emptied latch's loop dust (-13,-5,27) beside it; a stone over that dust cuts the
    // landing dust's diagonal to it). The router lands on (-12,-4,27); a block over it (-12,-3,27)
    // ties it high (today's stand) or is the bench's drive.
    place(b, [-10, -5, 27], "minecraft:comparator[facing=south,mode=subtract,powered=false]".into())?;
    place(b, [-10, -5, 28], rep("south", 1))?;
    b.blocks.insert([-10, -5, 29], "minecraft:redstone_block".into());
    place(b, [-11, -5, 27], rep("west", 1))?;
    place(b, [-12, -4, 27], DUST.into())?; // on the stone (-12,-5,27) that `place` puts under it
    b.blocks.insert([-12, -6, 27], STONE.into());
    b.blocks.insert([-13, -4, 27], STONE.into());
    b.ports.insert("input.permit".into(), [-12, -4, 27]);
    let open = std::env::var("MCTAS_PERMIT_OPEN").is_ok();
    if !open {
        b.blocks.insert([-12, -3, 27], "minecraft:redstone_block".into());
    }
    eprintln!("permission: input.permit lands on (-12,-4,27) (INV (-10,-5,27) into the OR node, powered = permit){}", if open { ", open" } else { ", tied high by a block at (-12,-3,27) (today's stand)" });
    Ok(())
}

/// The permission's feeds once the REL line is laid: each latch's Q to its OR input, the set off the
/// reset tail (REL-derived, one pulse per launch, beside the free west side; the REL line itself is
/// boxed in by the x -8 lines) or the REL line, into each latch's set chain (12 gt of chain).
fn permit_feeds(b: &mut B, rel: &[(P3, usize)], tail: &[(P3, usize)]) -> Result<()> {
    // (the feeds' fixed ends and their supports reserved from every other feed: through the
    // corridor rule, which keeps dust off the cell over each listed cell)
    let ends: [P3; 5] = [[-12, -5, 25], [-10, -5, 23], [-14, -5, 33], [-14, -5, 24], [-10, -5, 27]];
    let mut none: std::collections::BTreeSet<P3> = ends.iter().flat_map(|e| [[e[0], e[1] - 1, e[2]], [e[0], e[1] - 2, e[2]]]).collect();
    // (the evidence ports' cells and their side cells kept free of feed dust: a stand option may drive
    // a port with a dust or a block, which would power any line beside it (seen: the inverted
    // input's route past (-11,-5,28) took the stand-empty evidence as a hold))
    for p in [[-11, -5, 28], [-11, -5, 19]] {
        for d in [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]] {
            let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            if b.blocks.get(&n).is_none_or(|s| s.ends_with(":air")) {
                none.insert([n[0], n[1] - 1, n[2]]);
            }
        }
    }
    let lo = [-20, -8, 14];
    let hi = [-2, 0, 36];
    let qe = [-17, -5, 27];
    let qa = [-17, -5, 18];
    b.blocks.insert([qe[0], qe[1] - 1, qe[2]], STONE.into());
    b.blocks.insert(qe, DUST.into());
    b.blocks.insert([qa[0], qa[1] - 1, qa[2]], STONE.into());
    b.blocks.insert(qa, DUST.into());
    let srcs: Vec<(P3, usize)> = tail.iter().chain(rel.iter()).copied().collect();
    // (ironspec 08:27: the latches reset (BLOCK set) by the launched train's own cart evidence, not
    // by REL: an empty launch (no carts) then resets nothing and the advances still refill the
    // group, no deadlock. A declared port, `input.train_departed` (the release run's transit
    // detector (-3,1,4) or any single-cart cell past the release group; wired with the stand
    // option), feeds both set chains. MCTAS_PERMIT_SET=tail keeps the REL-derived set (the reset
    // tail) for the comparison)
    let port_set = std::env::var("MCTAS_PERMIT_SET").map(|v| v != "tail").unwrap_or(true);
    let srcs: Vec<(P3, usize)> = if port_set {
        let pd = [-19, -5, 28];
        b.ports.insert("input.train_departed".into(), pd);
        vec![(pd, 0)]
    } else {
        srcs
    };
    let p = feed(b, &srcs, [-14, -5, 33], 0, &[[-14, -5, 32]], lo, hi, "permission set (stand-emptied)", &none)?;
    let mut net: Vec<(P3, usize)> = srcs.clone();
    net.extend(p);
    let near: Vec<P3> = std::iter::once([-14, -5, 23]).chain(net.iter().map(|(c, _)| *c)).collect();
    feed(b, &net, [-14, -5, 24], 0, &near, lo, hi, "permission set (train-arrived)", &none)?;
    // (the Q feeds after the set feeds: those end on fixed cells)
    feed(b, &[(qe, 0)], [-12, -5, 25], 0, &[[-11, -5, 25]], lo, hi, "permission Q stand-emptied", &none)?;
    feed(b, &[(qa, 0)], [-10, -5, 23], 0, &[[-10, -5, 24]], lo, hi, "permission Q train-arrived", &none)?;
    // (input.permit's inverter output into the OR node's third input (-10,-5,26), read from the south)
    if let Some(io) = b.ports.remove("x.permit_inv_out") {
        feed(b, &[(io, 1)], [-10, -5, 27], 0, &[[-10, -5, 26]], lo, hi, "permission input.permit (inverted)", &none)?;
    }
    Ok(())
}

/// MCTAS_C24 (see its call): slot 15's window cut by A1: the line read at z 23 through a subtract
/// comparator (-5, -1, 24) (its rear the line's own weak dust, so a cut at ~9 empties it), its
/// output dust (-5, -1, 25) into the tap, now 3 rt (2 + 6: the 8-gt stagger kept); its east side
/// (-4, -1, 24) A1 off the feeder line through 4 rt.
fn q15_cut(b: &mut B, yy: i32) -> Result<()> {
    use crate::autoroute::{find, rider_space, Limits};
    let tap = [-4, yy, 25];
    let cmp = [-5, yy, 24];
    let out = [-5, yy, 25];
    let side = [-4, yy, 24];
    if !b.blocks.get(&tap).is_some_and(|s| s.contains("repeater")) {
        bail!("q15 cut: the tap {tap:?} is {:?}", b.blocks.get(&tap));
    }
    for p in [cmp, out, [cmp[0], cmp[1] - 1, cmp[2]], [out[0], out[1] - 1, out[2]]] {
        if b.blocks.get(&p).is_some_and(|s| !s.ends_with(":air") && s != STONE) {
            bail!("q15 cut: {p:?} holds {:?}", b.blocks.get(&p));
        }
    }
    b.blocks.insert(tap, rep("west", 3));
    b.blocks.insert([cmp[0], cmp[1] - 1, cmp[2]], STONE.into());
    b.blocks.insert(cmp, "minecraft:comparator[facing=north,mode=subtract,powered=false]".into());
    b.blocks.insert([out[0], out[1] - 1, out[2]], STONE.into());
    b.blocks.insert(out, DUST.into());
    let rp = cmp;
    let rear = out;
    if b.blocks.get(&side).is_some_and(|s| !s.ends_with(":air")) {
        bail!("q15 cut: the comparator's side {side:?} holds {:?}", b.blocks.get(&side));
    }
    // (the feeder line: x -1, y yy + 1, z 19..26, its dust run from the repeater (-3, yy - 1, 27):
    // (-2, yy - 1, 27), (-1, yy, 27), then z 26 down)
    let avoid = rider_space(&b.blocks);
    let mut why: Vec<String> = Vec::new();
    for z in (19..=26).rev() {
        let f = [-1, yy + 1, z];
        if !b.blocks.get(&f).is_some_and(|s| s.contains("redstone_wire")) {
            continue;
        }
        let run0 = (29 - z) as usize;
        for d in [[1, 0, 0], [0, 0, -1], [-1, 0, 0]] {
            let s = [f[0] + d[0], f[1] + d[1], f[2] + d[2]];
            if b.blocks.get(&s).is_some_and(|x| !x.ends_with(":air")) {
                continue;
            }
            // (x <= 0: the x 0 lane's feeders (x 1, y yy + 2, on stones at y yy + 1) are laid later;
            // the cells round the portal pad's route end (it arrives on (-2, -3, 17) and steps down
            // onto the golem stone's input block (-2, -4, 18)) left to it)
            let mut avoid = avoid.clone();
            for x in -3..=-1 {
                for y in -4..=-2 {
                    for z in 16..=18 {
                        avoid.insert([x, y, z]);
                    }
                }
            }
            let lim = Limits { lo: [-6, yy - 3, 17], hi: [0, yy + 1, 28], avoid, near_ok: [f, cmp, tap].into_iter().collect() };
            let p = match find(b, s, side, &lim) {
                Ok(p) => p,
                Err(e) => {
                    why.push(format!("from {s:?}: {e}"));
                    continue;
                }
            };
            let mut t = new_b();
            t.blocks = b.blocks.clone();
            t.ports = b.ports.clone();
            t.nbt = b.nbt.clone();
            match crate::sidinggen::lay_exact(&mut t, f, &p, run0, 4) {
                Ok(reps) => {
                    eprintln!("q15 cut: the comparator {cmp:?} (rear the line at z 23, out {rear:?} into the tap {tap:?} at 3 rt, {rp:?}); A1 from the feeder line {f:?} by {} cells, 4 rt ({reps:?}) into its side {side:?}", p.len());
                    *b = t;
                    return Ok(());
                }
                Err(e) => why.push(format!("from {s:?} ({} cells): {e}", p.len())),
            }
        }
    }
    bail!("q15 cut: no path from the feeder line into the comparator's side ({})", why.iter().take(8).cloned().collect::<Vec<_>>().join("; "))
}

/// MCTAS_C24 (see its call): A1's own path, from a dust on its tapless stone (own (5, 1, a1z)) to a
/// cell beside the advance link `al` between its two repeaters, through a 2-rt diode placed where
/// the strength before and after it holds (the run after it to the link's second repeater within 15).
fn a1_path(b: &mut B, at: fn(P3) -> P3, a1z: i32, al: &[P3]) -> Result<()> {
    use crate::autoroute::{find, rider_space, Limits};
    let stone = at([5, 1, a1z]);
    if !b.blocks.get(&stone).is_some_and(|s| s == STONE) {
        bail!("C24: A1's stone {stone:?} is {:?}", b.blocks.get(&stone));
    }
    let top = [stone[0], stone[1] + 1, stone[2]];
    if b.blocks.get(&top).is_some_and(|s| !s.ends_with(":air")) {
        bail!("C24: A1's stone's top {top:?} holds {:?}", b.blocks.get(&top));
    }
    let r1 = 3usize;
    let r2 = al.len() - 5;
    let avoid = rider_space(&b.blocks);
    let mut why: Vec<String> = Vec::new();
    for ci in (r1 + 1..r2).rev() {
        let c = al[ci];
        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
            if b.blocks.get(&n).is_some_and(|x| !x.ends_with(":air")) || al.contains(&n) {
                continue;
            }
            let lim = Limits { lo: [-11, -6, 28], hi: [4, 0, 58], avoid: avoid.clone(), near_ok: [stone, top, c].into_iter().collect() };
            // (the dust on the stone is the source by design (the router refuses a dust on a
            // powered support): routed from each of its neighbours, the top dust put first)
            let mut found = None;
            let mut errs: Vec<String> = Vec::new();
            for s in [[top[0] + 1, top[1], top[2]], [top[0] - 1, top[1], top[2]], [top[0] + 1, top[1] + 1, top[2]], [top[0] - 1, top[1] + 1, top[2]], [top[0], top[1] + 1, top[2] + 1], [top[0], top[1] + 1, top[2] - 1]] {
                if b.blocks.get(&s).is_some_and(|x| !x.ends_with(":air")) {
                    continue;
                }
                match find(b, s, n, &lim) {
                    Ok(q) => {
                        let mut p = vec![top];
                        p.extend(q);
                        found = Some(p);
                        break;
                    }
                    Err(e) => errs.push(format!("from {s:?}: {e}")),
                }
            }
            let Some(p) = found else {
                why.push(format!("to {n:?} (beside link cell {ci}): {}", errs.join(", ")));
                continue;
            };
            let mut lay_errs: Vec<String> = Vec::new();
            for i in (2..p.len()).rev() {
                if i > 15 || p.len() - i - 1 + (r2 - ci) > 15 {
                    continue;
                }
                let mut t = new_b();
                t.blocks = b.blocks.clone();
                t.ports = b.ports.clone();
                t.nbt = b.nbt.clone();
                t.blocks.insert(top, DUST.to_string());
                match lay_link(&mut t, top, &p[1..], &[(i - 1, 2)]) {
                    Ok(()) => {
                        eprintln!("C24: A1's path {} cells from its stone {stone:?} (diode 2 rt at {:?}) into the advance link's cell {ci} {c:?} ({} to its second repeater {:?})", p.len(), p[i], p.len() - i - 1 + (r2 - ci), al[r2]);
                        *b = t;
                        return Ok(());
                    }
                    Err(e) => lay_errs.push(format!("at {i}: {e}")),
                }
            }
            why.push(format!("to {n:?}: path {p:?}: {}", lay_errs.join(", ")));
        }
    }
    bail!("C24: A1's path finds no way into the advance link ({})", why.iter().take(8).cloned().collect::<Vec<_>>().join("; "))
}

/// MCTAS_REL_EXIT (see its call in `release_gen_loop`): R_g moved beside GO with REL's own dust
/// line into the release group's line, the reset tail over the bulb's top, the group taps' link
/// into the REL line. `at` places the core's own cells; `g0` is the core's group line take-off.
fn rel_exit(b: &mut B, at: fn(P3) -> P3, g0: P3) -> Result<(Vec<(P3, usize)>, Vec<(P3, usize)>)> {
    use crate::autoroute::{find, rider_space, Limits};
    let clone = |b: &B| {
        let mut t = new_b();
        t.blocks = b.blocks.clone();
        t.ports = b.ports.clone();
        t.nbt = b.nbt.clone();
        t
    };
    // (out: R_g on the core's group line, the reset line past its restore (own (9, -4..-1)) and
    // along own z -1, and the bulb's repeater)
    for p in [[3, 1, -4], [9, 1, -4], [9, 1, -3], [9, 1, -2], [9, 1, -1], [8, 1, -1], [7, 1, -1], [7, 1, 0]] {
        let q = at(p);
        if b.blocks.remove(&q).is_none() {
            bail!("REL exit: nothing at own {p:?} = {q:?} to take out");
        }
    }
    let rel0 = at([5, 1, -1]); // REL's first dust, north of GO
    let rg = at([6, 1, -1]); // R_g's new cell
    let out0 = at([7, 1, -1]); // the REL line's first cell
    if !b.blocks.get(&rel0).is_some_and(|s| s.contains("redstone_wire")) {
        bail!("REL exit: REL's first dust {rel0:?} is {:?}", b.blocks.get(&rel0));
    }
    let facing = match (rel0[0] - rg[0], rel0[2] - rg[2]) {
        (1, 0) => "east",
        (-1, 0) => "west",
        (0, 1) => "south",
        _ => "north",
    };
    // (MCTAS_RG_RT: R_g's delay, default 1; ironspec 08:13's early corner (b) delays every REL
    // consumer but the stand's by 2 gt: the launch through R_g + 1 rt with the chain's head and
    // the module's REL)
    let rg_rt: u8 = std::env::var("MCTAS_RG_RT").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    b.put(rg, &rep(facing, rg_rt))?;
    let rg_sup = [rg[0], rg[1] - 1, rg[2]];
    if b.blocks.get(&rg_sup).is_some_and(|s| !s.ends_with(":air")) && !b.blocks.get(&rg_sup).is_some_and(|s| s == STONE) {
        bail!("REL exit: R_g's support {rg_sup:?} holds {:?}", b.blocks.get(&rg_sup));
    }
    b.blocks.insert(rg_sup, STONE.to_string());
    let bulb = at([7, 1, 1]);
    let top = [bulb[0], bulb[1] + 1, bulb[2]];
    if b.blocks.get(&top).is_some_and(|s| s.contains("glass")) {
        b.blocks.remove(&top);
    }
    // (MCTAS_PERMIT: the permission's lane into GO's free west side and its fixed parts, laid before
    // the routes below so they go round them)
    if std::env::var("MCTAS_PERMIT").is_ok() {
        permit_fixed(b)?;
    }
    // (the reset now starts at its restore: own (9, 1, -5), facing on toward own -z... its output
    // cell is own (9, 1, -4); the bulb takes it through a stone at its own +z... side: the reset's
    // last dust on that stone, (bulb + own (0, 1, 2))... in the world, the bulb's z-1 side)
    let rs_src = at([9, 1, -5]);
    let rs_out = at([9, 1, -4]);
    let rs_ends: Vec<P3> = vec![[bulb[0], bulb[1] + 1, bulb[2] - 1], [bulb[0], bulb[1] + 1, bulb[2] + 1]];
    // (cells the REL line and the group link must not use: around the bulb (a powered support there
    // toggles it), GO's west side and E's and the readback comparator's sides)
    let mut avoid_rel: std::collections::BTreeSet<P3> = std::collections::BTreeSet::new();
    for dx in -1..=1 {
        for dy in 0..=1 {
            for dz in -1..=1 {
                avoid_rel.insert([bulb[0] + dx, bulb[1] + dy, bulb[2] + dz]);
            }
        }
    }
    for p in [[6, 1, 0], [6, 1, 2], [8, 1, 0], [8, 1, 2]] {
        avoid_rel.insert(at(p));
    }
    // (the release group's line: x -5, y -1, z 19..25, its taps at z 19, 22, 25; the REL line ends
    // beside its west side; the spread is the line's cells from the entry to the farthest tap. With
    // MCTAS_C24 slot 15 reads the line at z 23 through its own repeater (-5, -1, 24): the REL line
    // ends at z 23 or north, the farthest cells z 19 and 23)
    let last = if std::env::var("MCTAS_C24").is_ok() { 23 } else { 25 };
    let mut ends: Vec<(P3, P3, usize)> = (15..=last).map(|z| ([-6, -1, z], [-5, -1, z], ((z - 19).max(last - z) + 1) as usize)).collect();
    ends.sort_by_key(|e| e.2);
    let avoid = rider_space(&b.blocks);
    // (the permission's lane repeaters: a route's dust beside a repeater's side neither powers it nor
    // takes from it, and the lane's axis cells are its own OR node and GO)
    let lane: Vec<P3> = if std::env::var("MCTAS_PERMIT").is_ok() { (-9..=-3).map(|x| [x, -5, 25]).collect() } else { Vec::new() };
    let lim = |near: &[P3]| Limits { lo: [-9, -6, 17], hi: [3, 0, 34], avoid: avoid.union(&avoid_rel).copied().collect(), near_ok: near.iter().chain(lane.iter()).copied().collect() };
    let lim_rs = |near: &[P3]| Limits { lo: [-10, -8, 17], hi: [3, 0, 36], avoid: avoid.clone(), near_ok: near.iter().chain(lane.iter()).copied().collect() };
    let mut why: Vec<String> = Vec::new();
    for (end, entry, spread) in ends {
        let path = match find(b, out0, end, &lim(&[rg, entry])) {
            Ok(p) => p,
            Err(e) => {
                why.push(format!("REL line to {end:?}: {e}"));
                continue;
            }
        };
        if path.len() + spread > 15 {
            why.push(format!("REL line to {end:?}: {} cells + {spread} on the line > 15", path.len()));
            continue;
        }
        let mut t = clone(b);
        if let Err(e) = crate::sidinggen::lay_exact(&mut t, rg, &path, 0, 0) {
            why.push(format!("REL line to {end:?}: {e}"));
            continue;
        }
        // the reset tail: from its restore's output onto a stone beside the bulb (k rt in the tail
        // come off the restore, the total the same)
        let mut rs_done = None;
        let mut rs_why: Vec<String> = Vec::new();
        'rs: for &e in &rs_ends {
            let rp = match find(&t, rs_out, e, &lim_rs(&[rs_src, bulb, at([7, 1, 3]), at([6, 1, 2])])) {
                Ok(p) => p,
                Err(err) => {
                    rs_why.push(format!("to {e:?}: {err}"));
                    continue;
                }
            };
            for k in 0..=1u8 {
                let mut u = clone(&t);
                match crate::sidinggen::lay_exact(&mut u, rs_src, &rp, 0, k) {
                    Ok(_) => {
                        if k > 0 {
                            let s = u.blocks.get(&rs_src).cloned().unwrap_or_default();
                            let d: u8 = s.split("delay=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|v| v.parse().ok()).unwrap_or(0);
                            if d <= k {
                                rs_why.push(format!("to {e:?}: the restore {rs_src:?} has {d} rt, the tail needs {k}"));
                                continue;
                            }
                            u.blocks.insert(rs_src, s.replace(&format!("delay={d}"), &format!("delay={}", d - k)));
                        }
                        rs_done = Some((u, rp));
                        break 'rs;
                    }
                    Err(err) => rs_why.push(format!("to {e:?} with {k} rt: {err}")),
                }
            }
        }
        let Some((u, rp)) = rs_done else {
            why.push(format!("REL line to {end:?} ({path:?}): the reset tail finds no way to the bulb ({})", rs_why.join(", ")));
            continue;
        };
        // the group taps' link: from the take-off's east cell into a cell beside the REL line
        // (past R_g's first two cells), one repeater, the merged run within 15 to the far tap
        let gs = [g0[0] + 1, g0[1], g0[2]];
        let mut g_done = None;
        let mut g_why: Vec<String> = Vec::new();
        // (candidates: a cell beside the REL line past R_g's first two cells (a merge; the REL
        // line's rest then carries the window), or a cell beside the group line of its own (its
        // own entry, the spread from there))
        let mut cands: Vec<(P3, P3, usize)> = Vec::new();
        for ci in (2..path.len()).rev() {
            let c = path[ci];
            for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                if u.blocks.get(&n).is_some_and(|x| !x.ends_with(":air")) || path.contains(&n) {
                    continue;
                }
                cands.push((n, c, path.len() - ci + spread));
            }
        }
        for z in 19..=last {
            let n = [-6, -1, z];
            if u.blocks.get(&n).is_some_and(|x| !x.ends_with(":air")) || path.contains(&n) {
                continue;
            }
            cands.push((n, [-5, -1, z], ((z - 19).max(last - z) + 1) as usize));
        }
        'merge: for (n, c, tail) in cands {
            {
                let ci = path.iter().position(|q| *q == c).unwrap_or(path.len());
                let gp = match find(&u, gs, n, &lim(&[g0, c])) {
                    Ok(p) => p,
                    Err(e) => {
                        g_why.push(format!("to {n:?}: {e}"));
                        continue;
                    }
                };
                // (the repeater as late as the strength before it allows: the core's restore puts 15
                // on own z -7, the take-off 14, the path's cell k 13 - k, so the repeater at i reads
                // 14 - i: i <= 13)
                let mut fit = false;
                for i in (0..gp.len()).rev() {
                    if i > 13 {
                        continue;
                    }
                    let run = gp.len() - i - 1 + tail;
                    if run > 15 {
                        g_why.push(format!("to {n:?}: the repeater at {i} leaves {run} to the far tap"));
                        break;
                    }
                    let mut v = clone(&u);
                    if lay_link(&mut v, g0, &gp, &[(i, 1)]).is_ok() {
                        g_done = Some((v, gp.clone(), vec![gp[i]], ci, run));
                        fit = true;
                        break;
                    }
                }
                if fit {
                    break 'merge;
                }
            }
        }
        let Some((v, gp, reps, ci, grun)) = g_done else {
            why.push(format!("REL line to {end:?}: the group link finds no merge within 15 ({})", g_why.iter().take(6).cloned().collect::<Vec<_>>().join(", ")));
            continue;
        };
        eprintln!(
            "REL exit: R_g at {rg:?} (from REL's first dust {rel0:?}); the REL line {} cells to {end:?}, {spread} on the group line: the farthest tap at strength {}; the reset tail {} cells from its restore {rs_src:?} to {:?} (the bulb {bulb:?} beside its support); the group link {} cells (repeater at {reps:?}) {}: the window at the farthest tap at strength {}",
            path.len(),
            16 - path.len() - spread,
            rp.len(),
            rp.last(),
            gp.len(),
            if ci < path.len() { format!("into the REL line's cell {ci} {:?}", path[ci]) } else { format!("into the group line by its own entry {:?}", gp.last()) },
            16 - grun
        );
        let _ = top;
        *b = v;
        let _ = u;
        // (the REL line's cells with their runs off R_g: no repeater on it; the reset tail's off its
        // restore, a repeater on it resetting the run)
        let rel_cells: Vec<(P3, usize)> = path.iter().enumerate().map(|(i, c)| (*c, i + 1)).collect();
        let mut tail_cells: Vec<(P3, usize)> = Vec::new();
        let mut run = 0usize;
        for c in &rp {
            run = if b.blocks.get(c).is_some_and(|s| s.contains("repeater")) { 0 } else { run + 1 };
            tail_cells.push((*c, run));
        }
        return Ok((rel_cells, tail_cells));
    }
    bail!("REL exit: no layout ({})", why.join("; "))
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
    match res {
        Err((_, why)) => bail!("settle: {why}"),
        // (a tick that fails inside the run ends it early with its reason: a silent empty settle
        // otherwise, the file written raw)
        Ok(Some((t, why))) if !why.is_empty() => bail!("settle: tick {t}: {why}"),
        _ => {}
    }
    if states.is_empty() && !cells.is_empty() {
        bail!("settle: the run ended before tick 40 ({} cells unread)", cells.len());
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

/// Column 3 (c)'s controller shift: the links from the core under the lanes to row B's lines
/// carry two 1-rt repeaters, so every controller output is 2 rt (4 gt) later than on main, and the
/// REL taps outside the core add the same (`core_place`).
/// Milestone B's links' extra delay beyond main's, in rt, which the REL taps outside the core take
/// too: with the golem path one rt shorter than main's (the core 2 gt earlier against the golem),
/// every output then lands at main's time after the golem (the probe, which starts at the golem
/// stone, reads every output + 2 gt).
pub(crate) const FAR_SHIFT_RT: u8 = 1;

/// Where a cell of the controller's own frame lands in the loop: turned 180 degrees west of row B
/// (global = (-2 - x, y - 2, 10 - z)); column 3 (c) (`sidinggen::FAR`): under the lanes instead,
/// global = (3 - x, y - 6, 25 - z) (x -6..5, cells at y -5 on supports at y -6, z 17..54: clear of
/// the divert unit (z <= 16) and the module (y <= -10)), its lines linked up to row B's feeders.
pub(crate) fn core_place(p: P3) -> P3 {
    if crate::sidinggen::FAR {
        [3 - p[0], p[1] - 6, 25 - p[2]]
    } else {
        [-2 - p[0], p[1] - 2, 10 - p[2]]
    }
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
    let far = crate::sidinggen::FAR;
    // (milestone B's recovery: the repeater before the fourth group tap and A1 one rt shorter, so
    // the group window's end and the three advance pulses leave the chain 2 gt earlier, which the
    // links' repeaters make back; the reset taps from A1 on move with them, the merge window's
    // later taps 2 gt earlier against REL, still overlapping)
    let r14 = if far { R(1) } else { R(2) };
    let a1_run = if stagger { [R(1), D, r14, S(G), D, S(A | RS)] } else { [R(1), D, r14, D, D, S(A | RS)] };
    // (MCTAS_CHAIN_HEAD_RT, option 1 of day-1's 05:47 report: the chain's first repeater, default 3;
    // 2 moves every chain stone (the group, the resets, the advances) 2 gt earlier against REL, the
    // arrival side's outputs back to main's time)
    let head: u8 = std::env::var("MCTAS_CHAIN_HEAD_RT").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    // (MCTAS_C24, ironspec 06:04 item 1: the advances 24 gt apart with A1 = L+28, A2 = L+52, A3 =
    // L+76 at the controls: A1 10 gt, A2 6 gt and A3 2 gt earlier than on main. A1 comes between
    // the second and third group taps, so its stone has no tap of its own (its west side would
    // feed the group line): `a1_path` takes it off the stone's top by its own route into the
    // advance link. The group taps (+8/16/24, the fourth at +30), the reset taps (the merge
    // window) and the cell count (the group line to own z -15, the advance line from A2's z, the
    // advance link's take-off at own z -23 on it) as before)
    let c24 = far && std::env::var("MCTAS_C24").is_ok();
    let mut chain = if c24 {
        vec![
            R(head), S(G | RS), // G1
            R(4), S(G | RS),    // G2 (+8)
            R(1), S(0),         // A1 (+10: its own path)
            R(3), S(G | RS),    // G3 (+16)
            R(2), S(G | RS),    // G4 (+20, the window's end)
            R(4), S(RS),        // +28
            R(3), S(A),         // A2 (+34)
            R(1), S(RS),        // +36
            R(1), S(RS),        // +38 (the merge window's last copies)
            R(4), R(4), R(2), S(A), // A3 (+58)
        ]
    } else {
        vec![
            R(head), S(G | RS), // +8 (after the shortener C at z -5)
            R(4), S(G | RS), // +16
            R(4), S(G | RS), // +24
        ]
    };
    if !c24 {
    chain.extend(a1_run); // +30 (A1; zero skew pads every slot to row A's path)
    chain.extend([
        R(4), S(RS), // +38
        R(4), S(RS), // +46
        R(1), S(RS), // +48 (10-gt copies: the merge window to + 58)
        R(1), S(A), // +50 (A2)
        R(4), R(4), R(2), S(A), // +70 (A3)
    ]);
    }
    let mut m = new_b();
    let (stones, _) = core(&mut m, &chain, -15)?;
    if far {
        // (milestone B's recovery: a restore on the core's group line between the first group tap
        // and the rest, facing the link's take-off at own z -6: the later taps' pulses reach the
        // link at 14, so one link repeater carries the window; REL through R_g and the first tap
        // reach it unrestored)
        if !m.blocks.get(&[3, 1, -8]).is_some_and(|s| s.contains("redstone_wire")) {
            bail!("the core's group line at own (3, 1, -8) is {:?}, not a dust", m.blocks.get(&[3, 1, -8]));
        }
        m.blocks.insert([3, 1, -8], rep("north", 1));
    }
    let at = core_place;
    let mut b = new_b();
    for (p, s) in &m.blocks {
        b.put(at(*p), &turn2(s))?;
    }
    if far {
        // (column 3 (c): a stone over each of the core's dusts on its first row (z 17), which
        // cuts their diagonals onto the divert unit's top row (y -4, z 16) just north of it)
        let z0 = m.blocks.keys().map(|p| at(*p)[2]).min().unwrap_or(0);
        let row: Vec<P3> = m.blocks.iter().filter(|(_, s)| s.contains("redstone_wire")).map(|(p, _)| at(*p)).filter(|q| q[2] == z0).collect();
        for q in row {
            b.put([q[0], q[1] + 1, q[2]], STONE)?;
        }
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
        if far {
            // (column 3 (c): the detector at the far end's open strip (5, -8, 55) (x 5 at z 55: no
            // part of the farm over it, inside the loop's z <= 55), 13 repeaters north along x 5 at
            // y -8 (eleven of 4 rt, two of 3 rt), dust on north under the core with a 1-rt restore,
            // west along z 22 (a second restore), up a stair north, the last repeater (4 rt) east
            // into ARMED's loop dust (core (6, 1, 6)): 56 rt, the same 112 gt)
            let det = [5, -8, 55];
            b.put(det, "minecraft:daylight_detector[inverted=true,power=0]")?;
            b.put([det[0], det[1] + 1, det[2]], "minecraft:glass")?;
            let armed = at([6, 1, 6]);
            let mut cells: Vec<P3> = (22..det[2]).rev().map(|z| [det[0], det[1], z]).collect();
            for x in (armed[0] - 2..det[0]).rev() {
                cells.push([x, det[1], 22]);
            }
            cells.push([armed[0] - 2, det[1] + 1, 21]);
            cells.push([armed[0] - 2, det[1] + 2, 20]);
            cells.push([armed[0] - 2, det[1] + 3, 19]);
            cells.push([armed[0] - 1, armed[1], armed[2]]);
            if cells[cells.len() - 2] != [armed[0] - 2, armed[1], armed[2]] {
                bail!("the arming line ends at {:?}, not beside ARMED's loop dust {armed:?}", cells[cells.len() - 1]);
            }
            let mut reps: Vec<(usize, u8)> = (0..13).map(|i| (i, if i == 6 || i == 12 { 3 } else { 4 })).collect();
            for c in [[det[0], det[1], 27], [-1, det[1], 22]] {
                reps.push((cells.iter().position(|q| *q == c).ok_or_else(|| anyhow::anyhow!("the arming line has no cell {c:?}"))?, 1));
            }
            reps.push((cells.len() - 1, 4));
            reps.sort();
            let total: u32 = reps.iter().map(|r| r.1 as u32).sum();
            if total != 56 {
                bail!("the arming line has {total} rt, not 56 (112 gt)");
            }
            lay_link(&mut b, det, &cells, &reps)?;
        } else {
        b.put([-23, -1, 4], "minecraft:daylight_detector[inverted=true,power=0]")?;
        b.put([-23, 0, 4], "minecraft:glass")?;
        for x in -22..=-9 {
            b.on([x, -1, 4], &rep("west", 4))?;
        }
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
    if far {
        // (column 3 (c): the core under the lanes; its group line (0, -5, 30..40) and advance line
        // (0, -5, 42..54) reach row B's lines at x -5 through links, laid before those lines (their
        // last cells touch them): off the line one cell east, up a stair north to y -3, west over
        // the core to x -6, north along x -6, up a stair north into the cell beside the line; two
        // 1-rt repeaters each. Every output of the controller is then FAR_SHIFT (4 gt) later than
        // on main, the release group's start (REL through R_g) included; the REL taps (the stand's,
        // the divert unit's, the kick) and the tail windows take the same 4 gt, so the farm's
        // timing is main's shifted by 4 gt after the golem pulse; the core's own (refill, BUSY,
        // the merge window) is unchanged. The advance link leaves the line at own z -23, where all
        // three advance taps' pulses are still 9 or more)
        let g0 = at([3, 1, -6]);
        let a0 = at([3, 1, -23]);
        for (src, name) in [(g0, "group"), (a0, "advance")] {
            if !b.blocks.get(&src).is_some_and(|s| s.contains("redstone_wire")) {
                bail!("the core's {name} line at {src:?} is {:?}, not a dust", b.blocks.get(&src));
            }
        }
        let link = |src: P3, zrun: i32, zin: i32| -> Vec<P3> {
            let mut c = vec![[src[0] + 1, src[1], src[2]], [src[0] + 1, src[1] + 1, src[2] - 1], [src[0] + 1, src[1] + 2, src[2] - 2]];
            for x in (-6..=src[0]).rev() {
                c.push([x, src[1] + 2, zrun]);
            }
            for z in (zin + 2..zrun).rev() {
                c.push([-6, src[1] + 2, z]);
            }
            c.push([-6, src[1] + 3, zin + 1]);
            c.push([-6, src[1] + 4, zin]);
            c
        };
        let gl = link(g0, g0[2] - 2, 22);
        let al = link(a0, a0[2] - 2, 33);
        // (a stone over the line's next cell cuts the climb's diagonal back onto the line)
        for s in [g0, a0] {
            b.put([s[0], s[1] + 1, s[2] - 1], STONE)?;
        }
        // (repeaters: the group link one, 6 cells along its run west (the core's group line is
        // restored, the take-off at 14 whatever tap holds the window), the advance link two (its 23
        // cells and the row's spread need two refreshes), the first right after the climb, the
        // second 4 cells before the line)
        // (MCTAS_GROUP_LINK_REPS=0: the group link without its repeater, REL's rising edge into the
        // release group at main's time if the dust reaches (B = main, ironspec 05:49))
        if std::env::var("MCTAS_GROUP_LINK_REPS").ok().as_deref() == Some("0") {
            eprintln!("group link: {} cells, no repeater", gl.len());
            lay_link(&mut b, g0, &gl, &[])?;
        } else if std::env::var("MCTAS_REL_EXIT").is_err() {
            lay_link(&mut b, g0, &gl, &[(8, 1)])?;
        }
        lay_link(&mut b, a0, &al, &[(3, 1), (al.len() - 5, 1)])?;
        // (MCTAS_REL_EXIT, ironspec 06:04 item 2: the release group's launch edge free of any
        // refresh. R_g leaves the core's group line for the REL dust beside GO: at (-3, -5, 26),
        // fed by REL's first dust (-2, -5, 26), its output a dust line straight to the release
        // group's line (all dust, within 15 to the farthest tap), so REL reaches the controls
        // through R_g alone, as on main. The reset tail that sat there (own (9..7, -1) and the
        // bulb's repeater (7, 0)) goes: the reset reaches the bulb over its top by dust (its
        // restore takes the repeater's rt). The group taps' link joins the REL line past R_g
        // with its one repeater (the window's end as before). Each route is the autorouter's;
        // the REL line's end is tried in turn until the other two lay)
        if c24 && stagger {
            q15_cut(&mut b, yy)?;
        }
        let mut rel_line: Vec<(P3, usize)> = Vec::new();
        let mut reset_tail: Vec<(P3, usize)> = Vec::new();
        if std::env::var("MCTAS_REL_EXIT").is_ok() {
            (rel_line, reset_tail) = rel_exit(&mut b, core_place, g0)?;
        }
        // (MCTAS_C24: A1 off its stone's top by its own route into the advance link between the
        // link's two repeaters, through a 2-rt diode (the tap and the link's first repeater it
        // skips), so it reaches the controls through 3 rt like A2 and A3)
        if c24 {
            let a1z = stones.iter().find(|s| s.1 == 0).map(|s| s.0).ok_or_else(|| anyhow::anyhow!("C24: no A1 stone in the chain"))?;
            a1_path(&mut b, core_place, a1z, &al)?;
        }
        // (MCTAS_PERMIT: the permission's feeds, after the accepted routes)
        if std::env::var("MCTAS_PERMIT").is_ok() {
            if rel_line.is_empty() {
                bail!("MCTAS_PERMIT needs MCTAS_REL_EXIT (its set taps the REL line)");
            }
            permit_feeds(&mut b, &rel_line, &reset_tail)?;
        }
        // (MCTAS_REL_FEED, ironspec 06:02 item 1: R_g moved: REL by its own path from its dust
        // beside GO, one 1-rt repeater as both the diode and the refresh, into the release group's
        // line (x -5, y -1, z 19..25); the core's group line keeps only the taps. Tried from each REL
        // dust cell's free sides to each free cell beside the line; the first that lays wins.
        // MCTAS_REL_FEED_SKIP=n skips the first n that lay (the trace judges the spread))
        if std::env::var("MCTAS_REL_FEED").is_ok() {
            let skip: usize = std::env::var("MCTAS_REL_FEED_SKIP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            let rg = at([3, 1, -4]);
            b.blocks.remove(&rg);
            let srcs: [([i32; 3], usize); 7] = [([5, 1, -1], 1), ([5, 1, -2], 2), ([5, 1, -3], 3), ([4, 1, -3], 4), ([3, 1, -3], 5), ([2, 1, -3], 6), ([2, 1, -2], 7)];
            let mut ends: Vec<P3> = vec![[-5, -1, 18]];
            for z in 19..=25 {
                ends.push([-6, -1, z]);
                ends.push([-5, 0, z]);
            }
            let mut laid = 0usize;
            let mut found = None;
            'feed: for (core_src, run0) in srcs {
                let src = at(core_src);
                if !b.blocks.get(&src).is_some_and(|s| s.contains("redstone_wire")) {
                    continue;
                }
                for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]] {
                    let start = [src[0] + d[0], src[1] + d[1], src[2] + d[2]];
                    if b.blocks.get(&start).is_some_and(|s| !s.ends_with(":air")) {
                        if std::env::var("MCTAS_REL_FEED_WHY").is_ok() {
                            eprintln!("REL feed: {src:?} side {d:?}: {start:?} holds {:?}", b.blocks.get(&start));
                        }
                        continue;
                    }
                    for &end in &ends {
                        if b.blocks.get(&end).is_some_and(|s| !s.ends_with(":air")) {
                            continue;
                        }
                        let lim = crate::autoroute::Limits { lo: [-7, -6, 17], hi: [2, 0, 32], avoid: crate::autoroute::rider_space(&b.blocks), near_ok: [src].into_iter().collect() };
                        let path = match crate::autoroute::find(&b, start, end, &lim) {
                            Ok(p) => p,
                            Err(e) => {
                                if std::env::var("MCTAS_REL_FEED_WHY").is_ok() {
                                    eprintln!("REL feed: {start:?} -> {end:?}: {e}");
                                }
                                continue;
                            }
                        };
                        if path.len() + run0 > 24 {
                            if std::env::var("MCTAS_REL_FEED_WHY").is_ok() {
                                eprintln!("REL feed: {start:?} -> {end:?}: {} cells + {run0}", path.len());
                            }
                            continue;
                        }
                        let mut t = new_b();
                        t.blocks = b.blocks.clone();
                        t.ports = b.ports.clone();
                        t.nbt = b.nbt.clone();
                        if crate::sidinggen::lay_exact(&mut t, src, &path, run0, 1).is_ok() {
                            if laid == skip {
                                eprintln!("REL feed: from {src:?} (REL dust {run0} past GO) by {} cells to {end:?}: {path:?}", path.len());
                                found = Some(t);
                                break 'feed;
                            }
                            laid += 1;
                        }
                    }
                }
            }
            let Some(t) = found else { bail!("REL feed: no path lays with one 1-rt repeater (R_g moved)") };
            b.blocks = t.blocks;
            b.ports = t.ports;
            b.nbt = t.nbt;
        }
        for z in 19..=25 {
            if c24 && stagger && z >= 24 {
                continue; // (the q15 cut's repeater and rear stone, laid before)
            }
            b.on([-5, yy, z], DUST)?;
        }
        // (MCTAS_C24: slot 15's release window closes with A1's pulse (+42 at the control, 30 gt):
        // its tap becomes a subtract comparator (-4, -1, 25) facing the line, its rear a stone
        // (-5, -1, 25) powered by a 3-rt repeater (-5, -1, 24) off the line (3 rt + the
        // comparator's 1: the 8-gt stagger kept), its side (-4, -1, 24) A1 off the advance's feeder
        // line (x -1, 6 gt before the rails) through 7 rt: the cut +40..+50 at the comparator,
        // q15 off at +42; A2 and A3 cut only when the window is off. Laid before the REL exit's
        // routes, which go round it)
    }
    // (MCTAS_R1, ironfarm 07:15: R1's loop, 14 slots: row B (x -3) slots 10..5 at z 28..43, the
    // turn slot 4 at (-1,-1,44), row A (x 0) slots 0..3 at z 33..42, the escapement's G0/G_P/P
    // ahead of slot 0; the register zero-skew, slots 0-2 pulse-only (no tail windows))
    let r1 = far && std::env::var("MCTAS_R1").is_ok();
    let row_b_end = if r1 { 43 } else { 46 };
    let adv0 = if far { 27 } else { a3[2] + 1 };
    for z in adv0..=row_b_end {
        if z == 41 {
            // restores the first advance pulse (A1 injects at z 27) for the slots past z 40
            b.on([-5, yy, z], &rep("north", 1))?;
        } else {
            b.on([-5, yy, z], DUST)?;
        }
    }
    for z in (28..=row_b_end).step_by(3) {
        b.on([-4, yy, z], &rep("west", if z < 41 { 4 } else { 3 }))?;
    }
    // ---- around the north end: slot 7 (-1,-1,47) fed from z 48; row A (x 0, z 45..36) from x 2
    // (R1: all 3 cells nearer, slot 4 at (-1,-1,44), row A z 42..33)
    let nz = if r1 { -3 } else { 0 };
    b.on([-5, yy, 47 + nz], &rep("north", 1))?;
    b.on([-5, yy, 48 + nz], DUST)?;
    for x in -5..=2 {
        b.on([x, yy, 49 + nz], DUST)?;
    }
    b.on([-1, yy, 48 + nz], &rep("south", 2))?;
    b.on([2, yy, 48 + nz], DUST)?;
    b.on([2, yy, 47 + nz], &rep("south", 1))?;
    for z in 36 + nz..=46 + nz {
        b.on([2, yy, z], DUST)?;
    }
    for z in [45, 42, 39, 36] {
        b.on([1, yy, z + nz], &rep("east", 1))?;
    }
    if !r1 {
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
    }
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
    let zl = if far { 43 } else { az + 4 };
    if far {
        // (column 3 (c): A3 at (-2, -5, 54) under the lanes: its 4-rt tap on its free west side
        // (A3 has no reset tap), out west, up a stair south to y -4, east along z 55 round the
        // core's south end, north along x 4 (over the core's empty east half) into the z 43 line's
        // x 4 cell from below; one 3-rt repeater on the way: 4 + 3 rt to the line against main's
        // 4 + 1 (the x -1 repeater, gone here), the controller's FAR_SHIFT of 4 gt)
        let tap = [ax - 1, a3[1], az];
        b.on(tap, &rep("east", 4))?;
        let x4 = 4;
        let mut tl = vec![[ax - 2, a3[1], az], [ax - 2, a3[1] + 1, az + 1]];
        for x in ax - 1..=x4 {
            tl.push([x, a3[1] + 1, az + 1]);
        }
        for z in (zl + 1..=az).rev() {
            tl.push([x4, a3[1] + 1, z]);
        }
        // (a stone over the advance line's end cuts the z 55 run's diagonal onto it)
        b.put([0, a3[1] + 1, az], "minecraft:stone")?;
        let restore = tl.iter().position(|c| c[0] == x4 && c[2] == zl + 7).ok_or_else(|| anyhow::anyhow!("the tail route has no cell at z {}", zl + 7))?;
        lay_link(&mut b, tap, &tl, &[(restore, 3)])?;
        for x in 1..=8 {
            b.on([x, -3, zl], DUST)?;
        }
    } else {
    b.on([ax, yy, az + 1], &rep("north", 4))?; // golem + 80
    b.on([ax, yy, az + 2], DUST)?;
    b.on([ax, -2, az + 3], DUST)?;
    b.on([ax, -3, az + 4], DUST)?;
    for x in ax + 1..=8 {
        if x == -1 {
            b.on([x, -3, zl], &rep("west", 1))?; // golem + 82
        } else {
            b.on([x, -3, zl], DUST)?;
        }
    }
    }
    if r1 {
        // (R1: no tail windows; the A3 tap's route (the z 43 line, y -3, x 1..8) carries A3 alone to
        // the escapement's drive, which tells the third advance apart. ironspec 08:06: the drive is
        // held (no fixed G_P offset works at 24 with today's stand): only with MCTAS_R1_ESC, the
        // gates otherwise left to the bench)
        if std::env::var("MCTAS_R1_ESC").is_ok() {
            escapement(&mut b, yy, zl, &Design::load(loop_file)?)?;
        }
    } else {
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
    }
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
    // (R1's 14 queue controls, tail first: row A z 33..42, the turn slot 4, row B and the release
    // group at the same cells as the 18-slot loop's slots 9..17)
    let r1_controls: [P3; 14] = [[0, -1, 33], [0, -1, 36], [0, -1, 39], [0, -1, 42], [-1, -1, 44], [-3, -1, 43], [-3, -1, 40], [-3, -1, 37], [-3, -1, 34], [-3, -1, 31], [-3, -1, 28], [-3, -1, 25], [-3, -1, 22], [-3, -1, 19]];
    let controls: &[P3] = if r1 { &r1_controls[..] } else if pocket { &all_controls[..18] } else { &all_controls[..] };
    if r1 {
        // (P (0,2,31) always on: its block a redstone block, the escapement's own)
        match d.blocks.get(&[0, 1, 31]).map(|s| s.as_str()) {
            Some("minecraft:glass") => {
                d.blocks.insert([0, 1, 31], "minecraft:redstone_block".into());
            }
            other => bail!("R1: P's block (0,1,31) is {other:?}, not glass"),
        }
    }
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
        let far = crate::sidinggen::FAR;
        let rel_port = if far { core_place([0, 1, -3]) } else { [-2, -1, 13] };
        let dt = crate::sidinggen::DETECTOR;
        // (column 3 (c): thr at (d0 + 1, -17, d2 - 11) facing north, its output the cell north of
        // it, the lock repeater on its west side)
        let (thr_port, lock_port) = if far { ([dt[0] + 1, -17, dt[2] - 12], [dt[0], -17, dt[2] - 11]) } else { ([dt[0], dt[1], dt[2] + 13], [dt[0] + 1, dt[1], dt[2] + 12]) };
        for (n, q) in [("rel", rel_port), ("rel_in", m([14, -2, -4])), ("g2", m([14, -2, 4])), ("unit_t0", u([20, -6, 9])), ("divert", [9, -3, 0]), ("t1", [-15, 1, 28]), ("t2", [-16, 1, 31]), ("ka", [-21, -4, 31]), ("hold14", [-12, 1, 28]), ("hold13", [-11, 1, 31]), ("hold12", [-12, 1, 34]), ("force", crate::sidinggen::FORCE), ("ns3_in", { let f = crate::sidinggen::FORCE; [f[0] - 2, f[1], f[2] - 1] }), ("fa_rel", { let f = crate::sidinggen::FORCE; [f[0], f[1], f[2] - 2] }), ("thr", thr_port), ("lock", lock_port), ("win", m([-2, 0, 1])), ("nv_mod", m([-4, 0, 3])), ("nv_loc", u([18, -6, 10])), ("g2_loc", u([20, -6, 9]))] {
            sp.ports.insert(format!("output.{n}"), q);
        }
        if std::env::var("MCTAS_SIDING_PROBE_MORE").is_ok() {
            // (R1: the cells that stayed keep their old names, so the trace-diff maps them: q14..q17
            // and r15..r17 are R1's slots 10..13 at the same cells, q02/q03 R1's slots 0/1 at the
            // same cells; R1's own: slots 2..4 and the escapement's gates. q00/q01's cells are gone)
            let base_ports: Vec<(&str, P3)> = vec![("r15", [-3, 0, 25]), ("r16", [-3, 0, 22]), ("r17", [-3, 0, 19]), ("q14", [-3, -1, 28]), ("q15", [-3, -1, 25]), ("q16", [-3, -1, 22]), ("q17", [-3, -1, 19]), ("q02", [0, -1, 33]), ("q03", [0, -1, 36])];
            let tail_ports: Vec<(&str, P3)> = if r1 { vec![("r1_q02", [0, -1, 39]), ("r1_q03", [0, -1, 42]), ("r1_q04", [-1, -1, 44]), ("g_p", [0, 2, 32]), ("g0", [0, 3, 31])] } else { vec![("q00", [0, 1, 25]), ("q01", [0, 0, 29])] };
            for (n, q) in base_ports.into_iter().chain(tail_ports) {
                sp.ports.insert(format!("output.{n}"), q);
            }
            if r1 {
                // (the gates are the loop's fence gates, absent from the probe: a stone in each
                // reads its drivers, as the controls' stones do)
                for g in [[0, 2, 32], [0, 3, 31]] {
                    sp.blocks.entry(g).or_insert_with(|| STONE.into());
                }
            }
            for (n, q) in [("stand_rel", [8, -2, 2]), ("x_tap", [0, -1, 10]), ("x_b0", [0, -1, 9]), ("x_b3", [8, -1, 9]), ("x_or", [4, -1, 7]), ("x_or8", [8, -1, 6]), ("x_dr", [8, -2, 4]), ("row7", u([23, -6, 7])), ("row1", u([23, -6, 1])), ("row_5", u([23, -6, -5])), ("step_in", u([20, -4, 12])), ("step_end", u([28, -5, 6])), ("ring_top", u([27, -5, 4])), ("p0", u([22, -6, 9])), ("p1", u([22, -6, 3])), ("p2", u([22, -6, -3])), ("or_out", [11, -4, 0]), ("ns5", [-23, -4, 30]), ("bus", [-15, -1, 33])] {
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
        // (no added block in a rider's space, the two cells over any rail: a block there
        // suffocates the passing rider and stops its cart. Found 2026-11-11: an auto-routed line
        // over the loop's y 4 return track killed three riders in the judge's run)
        if crate::sidinggen::FAR {
            let rs = crate::autoroute::rider_space(&d.blocks);
            let bad: Vec<String> = mine.iter().filter(|p| rs.contains(*p) && d.blocks.get(*p).is_some_and(|s| real(s)) && !siding_cells.contains(*p)).map(|p| format!("{p:?} {}", d.blocks[p])).collect();
            if !bad.is_empty() {
                bail!("{} added blocks in a rider's space (the two cells over a rail): {}", bad.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("; "));
            }
            // (no added block may cut a rider's sight of a zombie: the base is the file without the
            // controller's own blocks (the loop, the siding, the pad as given); MCTAS_SIGHT_CONTROL
            // x,y,z puts a stone there first, the check's positive control)
            let pad_cells: std::collections::BTreeSet<P3> = match pad {
                Some(p) => Design::load(p)?.blocks.keys().copied().collect(),
                None => Default::default(),
            };
            let mut fin = d.blocks.clone();
            for c in std::env::var("MCTAS_SIGHT_CONTROL").unwrap_or_default().split(';').map(|v| v.split(',').filter_map(|t| t.trim().parse::<i32>().ok()).collect::<Vec<_>>()).filter(|c| c.len() == 3) {
                fin.insert([c[0], c[1], c[2]], STONE.into());
            }
            let base: BTreeMap<P3, String> = d.blocks.iter().filter(|(p, _)| !mine.contains(*p) || siding_cells.contains(*p) || pad_cells.contains(*p)).map(|(p, s)| (*p, s.clone())).collect();
            let zombies: Vec<[f64; 3]> = d.entities.iter().filter(|e| e.nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie"))).map(|e| e.pos).collect();
            let (n, cuts) = crate::autoroute::sight_cuts(&base, &fin, &zombies);
            // (MCTAS_SIGHT_ACCEPT "x,y,z;...": the stopping cells whose cuts an owner accepted,
            // pending a run that checks them (ironfarm 07:54: G_P's east driver, pending the
            // integrated loop's stamp and panic lines); every accepted cut is printed)
            let accept: Vec<P3> = std::env::var("MCTAS_SIGHT_ACCEPT").ok().map(|v| v.split(';').filter_map(|c| {
                let n: Vec<i32> = c.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                (n.len() == 3).then(|| [n[0], n[1], n[2]])
            }).collect()).unwrap_or_default();
            let (accepted, cuts): (Vec<_>, Vec<_>) = cuts.into_iter().partition(|(_, _, b)| accept.contains(b));
            if !accepted.is_empty() {
                let show: Vec<String> = accepted.iter().map(|(r, z, b)| format!("rider over {r:?} to the zombie at {z:?}: stopped by {b:?}")).collect();
                println!("sight lines ACCEPTED as cut (MCTAS_SIGHT_ACCEPT, pending the integrated check): {}: {}", accepted.len(), show.join("; "));
            }
            if !cuts.is_empty() {
                let show: Vec<String> = cuts.iter().take(8).map(|(r, z, b)| format!("rider over {r:?} to the zombie at {z:?}: stopped by {b:?} {}", fin.get(b).map(|s| s.as_str()).unwrap_or("?"))).collect();
                bail!("{} rider-zombie sight lines cut by the added blocks (of {n} checked): {}", cuts.len(), show.join("; "));
            }
            println!("sight lines: {n} rider-zombie lines within 8 checked ({} zombies), none cut by the added blocks", zombies.len());
        }
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
    // (the head's declared ports (ironspec 08:48: S3's router wires against them; day-1's port list
    // 2026-11-11, times from input.golem's rise): head.golem (REQ in, any pulse >= 2 gt), head.hold
    // (PERMIT in, powered = hold: GO blocked 14 gt after it rises, REL 20 gt after it falls), head.go
    // (GO's output dust: REL +2..+14), head.stand_rel (the 1-rt tap off the refill dust: +4..+18, for
    // a stand demux), head.expire (G_e's output: ~100 gt after a launch with no golem))
    if crate::sidinggen::FAR {
        d.ports.insert("head.golem".into(), golem);
        d.ports.insert("head.go".into(), core_place([5, 1, -1]));
        d.ports.insert("head.stand_rel".into(), core_place([-3, 1, -1]));
        d.ports.insert("head.expire".into(), core_place([4, 1, 3]));
        if let Some(p) = b.ports.get("input.permit") {
            d.ports.insert("head.permit".into(), *p);
        }
    }
    d.name = "carousel-release-loop-wired".into();
    d.save(out)?;
    println!("{}: {} blocks ({} the controller's), golem input at {:?}, stand reset at {:?}", out.display(), d.blocks.len(), b.blocks.len(), golem, reset);
    Ok(())
}
