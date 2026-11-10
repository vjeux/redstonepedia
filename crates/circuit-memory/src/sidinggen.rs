//! The bed siding's two junctions on farmopt's pocket-stand loop (day-1 lane 2026-11-10; parent
//! 12:11: the chosen napper's cart diverts once a night and rejoins its batch; agreed with ironfarm
//! and farmopt 12:51-13:15).
//!
//! `release-gen --loop L --siding S` adds, to the loop file it writes:
//! - farmopt's wired siding (S's box, local x -7..6, y -4..4, z -3..9, with its dorm zombie) pasted
//!   unturned at (-20, 2, 10): its bed 17+ blocks from every rider (a calm villager within 16 took
//!   the bed in farmopt's bench), its entry booster at (-27, 2, 10), its exit (-14, 1, 10);
//! - the diverter: a branch rail north of the departure corner (9, -3, 0), which makes the corner a
//!   T (west, south, north): unpowered it stays south_west (the loop), powered it turns north_west
//!   (the branch);
//! - the entry route: north to z -2, west along z -2 (y -3) to x -29, south along x -29 climbing to
//!   y 2 on five boosters, east into the siding's booster;
//! - the exit route: from the siding's exit east to x -13, a booster, south along x -13 (y 1) with
//!   a loop west (z 20..23, x -22) for drag, into T1 (-13, 1, 28), its stem north: unpowered
//!   north_east (east along z 28 to slot 14), powered north_west (west, round to T2 (-15, 1, 31),
//!   its stem north: unpowered east along z 31 to slot 13, powered west, round x -17, east along
//!   z 34 to slot 12);
//! - each drop line ends as ironfarm's side drop: three plain rails at y 1 on glass west of the
//!   slot's slope (x -6..-4), a stone stop east of it at (-2, 1, z); the cart rolls off the end into
//!   the empty slot during the long rest.
//!
//! Four controls, bench cells for now (glass = off, a redstone block = on): the corner T's support
//! (9, -4, 0), T1's (-8, 0, 28), T2's (-10, 0, 31), the siding's release input (-16, 0, 10).

use crate::schedgen::{B, P3};
use anyhow::{Result, bail};
use mctas::design::{Design, EntitySpec};
use std::path::Path;

const STONE: &str = "minecraft:stone";
const GLASS: &str = "minecraft:glass";
const RBLOCK: &str = "minecraft:redstone_block";

pub(crate) const SIDING_AT: P3 = [-20, 2, 10];
pub(crate) const DIVERT: P3 = [9, -4, 0];
pub(crate) const T1: P3 = [-13, 0, 28];
pub(crate) const T2: P3 = [-15, 0, 31];
pub(crate) const RELEASE: P3 = [-16, 0, 10];
pub(crate) const HOLD: P3 = [-13, 0, 23];

fn rail(shape: &str) -> String {
    format!("minecraft:rail[shape={shape},waterlogged=false]")
}

fn booster(shape: &str) -> String {
    format!("minecraft:powered_rail[powered=true,shape={shape},waterlogged=false]")
}

fn comp(facing: &str, mode: &str) -> String {
    format!("minecraft:comparator[facing={facing},mode={mode},powered=false]")
}

/// A rail at `p` on a support (stone unless `glass`).
fn lay(b: &mut B, p: P3, s: &str, glass: bool) -> Result<()> {
    b.put(p, s)?;
    b.put([p[0], p[1] - 1, p[2]], if glass { GLASS } else { STONE })
}

/// Pastes the siding and lays the routes and junctions into `d`; returns the cells it added.
pub(crate) fn physical(d: &mut Design, siding: &Path) -> Result<Vec<P3>> {
    let mut b = B { blocks: d.blocks.clone(), ports: Default::default(), nbt: Default::default() };
    let before: std::collections::BTreeSet<P3> = d.blocks.keys().copied().collect();
    // ---- the siding
    let s = Design::load(siding)?;
    let inbox = |q: &P3| (-7..=6).contains(&q[0]) && (-4..=4).contains(&q[1]) && (-3..=9).contains(&q[2]);
    let at = |q: &P3| [q[0] + SIDING_AT[0], q[1] + SIDING_AT[1], q[2] + SIDING_AT[2]];
    for (q, st) in s.blocks.iter().filter(|(q, _)| inbox(q)) {
        b.put(at(q), st)?;
    }
    for (q, n) in s.block_entities.iter().filter(|(q, _)| inbox(q)) {
        d.block_entities.insert(at(q), n.clone());
    }
    for e in &s.entities {
        let q = [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32];
        if inbox(&q) {
            let p = [e.pos[0] + SIDING_AT[0] as f64, e.pos[1] + SIDING_AT[1] as f64, e.pos[2] + SIDING_AT[2] as f64];
            d.entities.push(EntitySpec { kind: e.kind.clone(), pos: p, nbt: e.nbt.clone() });
        }
    }
    if b.blocks.get(&RELEASE).map(|s| s.as_str()) != Some(GLASS) {
        bail!("the siding's release input {RELEASE:?} is {:?}, not glass: the siding file changed", b.blocks.get(&RELEASE));
    }
    // the siding releases each napper as soon as it is back in its cart (farmopt's release-held
    // mode, benched by his child: every napper re-boards and leaves): its release input is a
    // redstone block; the timed release is the hold near T1 (ironfarm 14:35)
    b.blocks.insert(RELEASE, RBLOCK.into());
    // the bed's port (R1's home: the runner gives the bed to each napper from it; ironfarm 14:00)
    let home = s.ports.get("home").copied().ok_or_else(|| anyhow::anyhow!("the siding file has no `port home:`"))?;
    d.ports.insert("home".into(), at(&home));
    // ---- the diverter: the branch north of the corner (the corner's support stays the glass
    // control the bench switches)
    match b.blocks.get(&[9, -3, 0]).map(|s| s.as_str()) {
        Some(s) if s.contains("shape=south_west") => {}
        other => bail!("the departure corner (9, -3, 0) is {other:?}, not a south_west rail: the loop file changed"),
    }
    lay(&mut b, [9, -3, -1], &rail("north_south"), false)?;
    // ---- the entry route
    lay(&mut b, [9, -3, -2], &rail("south_west"), false)?;
    for x in -28..=8 {
        lay(&mut b, [x, -3, -2], &rail("east_west"), false)?;
    }
    lay(&mut b, [-29, -3, -2], &rail("south_east"), false)?;
    lay(&mut b, [-29, -3, -1], &rail("north_south"), false)?;
    // the climb: five boosters ascending south, the first one's support a redstone block (it powers
    // all five through the rail line)
    for k in 0..5 {
        let p = [-29, -3 + k, k];
        b.put(p, &booster("ascending_south"))?;
        b.put([p[0], p[1] - 1, p[2]], if k == 0 { RBLOCK } else { STONE })?;
    }
    for z in 5..=9 {
        lay(&mut b, [-29, 2, z], &rail("north_south"), false)?;
    }
    lay(&mut b, [-29, 2, 10], &rail("north_east"), false)?;
    lay(&mut b, [-28, 2, 10], &rail("east_west"), false)?;
    // ---- the exit route to T1: a booster on the first straight cell (the cart leaves the exit
    // slot at 0.28-0.44 and would crawl: ironfarm 13:57), then ~46-58 rails of drag to the drop
    // ends (a loop west of x -13, z 20..23) so it arrives at 0.32-0.35 b/t
    lay(&mut b, [-13, 1, 10], &rail("south_west"), false)?;
    b.put([-13, 1, 11], &booster("north_south"))?;
    b.put([-13, 0, 11], RBLOCK)?;
    // (no drag loop: ironfarm 14:30 measured this loop's side drop clean at 0.10-0.48 b/t; the
    // booster's 0.49-0.53 decays to ~0.43-0.46 over the ~25 rails to the drop ends)
    // ---- the hold near T1: the cart climbs to y 2 at (-13, 1, 21) (ascending south), runs flat to
    // (-13, 2, 22), and rolls down onto a braked powered rail ascending north (-13, 1, 23), where it
    // stops; powered (its support (-13, 0, 23), a control), it rolls on south to T1. The short leg
    // keeps the hold -> roll-off time nearly the same for all three drops.
    for z in 12..=20 {
        lay(&mut b, [-13, 1, z], &rail("north_south"), false)?;
    }
    lay(&mut b, [-13, 1, 21], &rail("ascending_south"), false)?;
    lay(&mut b, [-13, 2, 22], &rail("north_south"), false)?;
    b.put([-13, 1, 23], "minecraft:powered_rail[powered=false,shape=ascending_north,waterlogged=false]")?;
    b.put(HOLD, GLASS)?;
    for z in 24..=27 {
        lay(&mut b, [-13, 1, z], &rail("north_south"), false)?;
    }
    // T1 (-13, 1, 28): stem north, unpowered north_east (east along z 28: slot 14), powered
    // north_west (T2); its support glass (west of the release controller's taps, so the cells under
    // the T's are free for their drivers)
    b.put([-13, 1, 28], &rail("north_east"))?;
    b.put(T1, GLASS)?;
    // T1 west to T2 (-15, 1, 31), stem north: unpowered east along z 31 (slot 13), powered west
    lay(&mut b, [-14, 1, 28], &rail("east_west"), false)?;
    lay(&mut b, [-15, 1, 28], &rail("south_east"), false)?;
    lay(&mut b, [-15, 1, 29], &rail("north_south"), false)?;
    lay(&mut b, [-15, 1, 30], &rail("north_south"), false)?;
    b.put([-15, 1, 31], &rail("north_east"))?;
    b.put(T2, GLASS)?;
    // T2 west round to z 34 (slot 12)
    lay(&mut b, [-16, 1, 31], &rail("east_west"), false)?;
    lay(&mut b, [-17, 1, 31], &rail("south_east"), false)?;
    lay(&mut b, [-17, 1, 32], &rail("north_south"), false)?;
    lay(&mut b, [-17, 1, 33], &rail("north_south"), false)?;
    lay(&mut b, [-17, 1, 34], &rail("north_east"), false)?;
    // ---- the three drop lines (east ends over the slots' slopes at x -3), glass under the last three
    let lines: [(i32, i32); 3] = [(28, -12), (31, -14), (34, -16)];
    for (z, x0) in lines {
        for x in x0..=-4 {
            lay(&mut b, [x, 1, z], &rail("east_west"), x >= -6)?;
        }
        b.put([-2, 1, z], STONE)?;
    }
    // the always-on redstone blocks (the climb's, the exit booster's) must touch nothing that
    // carries a signal besides their own rail
    for rb in [[-29, -4, 0], [-13, 0, 11]] {
        for dlt in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]] {
            let q = [rb[0] + dlt[0], rb[1] + dlt[1], rb[2] + dlt[2]];
            if let Some(s) = b.blocks.get(&q) {
                if ["redstone_wire", "repeater", "comparator", "torch", "rail", "piston", "dropper", "hopper", "observer", "lamp", "bulb", "door", "trapdoor", "fence_gate"].iter().any(|k| s.contains(k)) {
                    bail!("the always-on block {rb:?} touches {q:?} = {s}");
                }
            }
        }
    }
    let added: Vec<P3> = b.blocks.keys().copied().filter(|p| !before.contains(p)).collect();
    d.blocks = b.blocks;
    Ok(added)
}

/// `mctas circuit memory siding-ctl-gen --out F`: the siding's control module, in the nightly
/// scheduler's own frame (the scheduler stamped at 0 0 0; additions east of it, x >= 2, and under
/// it, y -2), driven by REL alone (WIP, part 1: the scheduler's REL and night inputs, the visit
/// pulse VP out).
/// - REL: `input.rel` (12, -1, -4), a bus west along z -4 at y -1 under the scheduler's input row,
///   up into a dust (-5, 0, -4) and a repeater on the scheduler's input cell (-5, 0, -3).
/// - Night: the scheduler's window cell (-2, 0, 1) powered from below by a torch (-2, -1, 1) on a
///   block (-2, -2, 1) that the day line powers through a repeater (-1, -2, 1) from `input.day`
///   (0, -2, 1) (in the loop: a normal daylight detector, > 0 by day).
/// - VP, the visit pulse (the scheduler's gate output, high from the release's start while the
///   release is a visit): two repeaters off V's stone (-5, 0, 4) east into a block (-2, 0, 4), a
///   dust under it, down to y -2, south along x -1 and east along z 11 to `output.vp`.
pub fn control_blocks(sched: &Path) -> Result<B> {
    use crate::schedgen::{DUST, rep};
    let mut b = B { blocks: Default::default(), ports: Default::default(), nbt: Default::default() };
    b.stamp(sched, [0, 0, 0], 0, None)?;
    // ---- REL into the scheduler (its input cell becomes a repeater); the input block (14, -2, -4)
    // feeds the bus (up to y -1 at x 12) and REL2 south along x 14 for the siding logic
    b.blocks.remove(&[-5, 0, -3]);
    b.on([-5, 0, -3], &rep("north", 1))?;
    b.on([-5, 0, -4], DUST)?;
    b.put([14, -2, -4], STONE)?;
    b.ports.insert("input.rel".into(), [14, -2, -4]);
    b.on([13, -2, -4], DUST)?;
    b.on([12, -1, -4], DUST)?;
    let bus: Vec<P3> = (-4..=11).rev().map(|x| [x, -1, -4]).collect();
    b.wire([12, -1, -4], &bus, 12)?;
    // ---- night: the torch under the window cell, off while the day line powers its block
    b.put([0, -2, 1], STONE)?;
    b.ports.insert("input.day".into(), [0, -2, 1]);
    b.on([-1, -2, 1], &rep("east", 1))?;
    b.put([-2, -2, 1], STONE)?;
    b.put([-2, -1, 1], "minecraft:redstone_torch[lit=true]")?;
    // ---- NV = NOT (D = 0 and night), steady between releases: the gate's side dust (-4, 0, 1)
    // read by a repeater south into a block (-4, 0, 3), a dust under it, down to y -2 and east along
    // z 3 to `output.nv` (the visit gate itself, V's stone, comes ~13 gt after REL: too late for
    // the diverter, so the release is known a visit from NV sampled at its start)
    b.on([-4, 0, 2], &rep("north", 1))?;
    b.put([-4, 0, 3], STONE)?;
    b.on([-4, -1, 3], DUST)?;
    let nv: Vec<P3> = (-3..=3).map(|x| [x, -2, 3]).collect();
    b.wire([-4, -1, 3], &nv, 0)?;
    b.ports.insert("output.nv".into(), [3, -2, 3]);
    // (v itself is not used: the visit is G2 below, the windows hang off it)
    // ---- G2 = REL and not NV (NV is still the release's own value while REL is high: the D ring
    // steps ~19 gt after REL), a 12-gt pulse at the start of every visit release; the windows W0,
    // W1 = W0 + 10, W2 = W0 + 20 down a repeater chain along x 14, each tapped by a block T_k
    // (14, -2, z_k), z_k = 6, 10, 14
    for z in -3..=2 {
        b.on([14, -2, z], DUST)?;
    }
    b.on([4, -2, 3], &rep("west", 1))?;
    for x in 5..=12 {
        b.on([x, -2, 3], DUST)?;
    }
    b.on([13, -2, 3], &rep("west", 1))?;
    b.on([14, -2, 3], &comp("north", "subtract"))?;
    // the chain: four repeaters from G2 to T0 (the windows' offset, set by `w0`), then 10 gt between
    // taps (repeaters 3 + 1 + 1: no dust in the chain, so nothing beside it leaks in)
    let w0 = [1u8, 1, 1, 1];
    for (i, d) in w0.iter().enumerate() {
        b.on([14, -2, 4 + i as i32], &rep("north", *d))?;
    }
    let zk = [8, 12, 16];
    for (k, &z) in zk.iter().enumerate() {
        b.put([14, -2, z], STONE)?;
        if k < 2 {
            b.on([14, -2, z + 1], &rep("north", 3))?;
            b.on([14, -2, z + 2], &rep("north", 1))?;
            b.on([14, -2, z + 3], &rep("north", 1))?;
        }
        // P_k = W_k - NOT l_k: a comparator west of the tap, its side (north) a repeater fed by
        // the ring R''s NOT l_k line (R' holds 2 items in 3 droppers: the empty one is the state,
        // so a comparator on dropper k reads NOT l_k)
        b.on([13, -2, z], &comp("east", "subtract"))?;
        b.on([13, -2, z - 1], &rep("north", 1))?;
        b.on([13, -2, z - 2], DUST)?;
        // its output up a stair to the OR column at y 0
        b.on([12, -2, z], DUST)?;
        b.on([11, -1, z], DUST)?;
    }
    for z in 8..=16 {
        b.on([10, 0, z], DUST)?;
    }
    b.on([10, 0, 17], &rep("north", 1))?;
    b.ports.insert("output.divert".into(), [10, 0, 18]);
    // ---- R', the visit counter mod 3 (the scheduler's own slot ring is not reachable): droppers
    // A (5, 8), B (7, 8), C (6, 9) and hoppers h (6, 8), H (7, 9), j (5, 9), A -> B -> C -> A, dust on
    // all six tops (y -1) fired by `step`; 2 items, A empty at power-on (R' = 0). Its taps (NOT l_k):
    // A from the north -> (5, 6) -> row 5 -> (12, 5..6) -> P0's side; B from the east -> (9, 8) ->
    // x 10 -> row 10 -> P1's side; C from the south -> (6, 11) -> x 6 -> row 14 -> P2's side.
    let item = "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}";
    let ring: [(P3, &str); 6] = [
        ([5, -2, 8], "minecraft:dropper[facing=east,triggered=false]"),
        ([6, -2, 8], "minecraft:hopper[enabled=true,facing=east]"),
        ([7, -2, 8], "minecraft:dropper[facing=south,triggered=false]"),
        ([7, -2, 9], "minecraft:hopper[enabled=true,facing=west]"),
        ([6, -2, 9], "minecraft:dropper[facing=west,triggered=false]"),
        ([5, -2, 9], "minecraft:hopper[enabled=true,facing=north]"),
    ];
    for (p, s) in ring {
        b.put(p, s)?;
        b.put([p[0], -1, p[2]], DUST)?;
    }
    b.nbt.insert([7, -2, 8], item.into());
    b.nbt.insert([6, -2, 9], item.into());
    // (its step comes in from the west through j's top: see the step line below)
    b.on([4, -1, 9], &rep("west", 1))?;
    // A's tap
    b.on([5, -2, 7], &comp("south", "compare"))?;
    b.on([5, -2, 6], &rep("south", 1))?;
    for x in 5..=12 {
        b.on([x, -2, 5], DUST)?;
    }
    b.on([12, -2, 6], DUST)?;
    // B's tap
    b.on([8, -2, 8], &comp("west", "compare"))?;
    b.on([9, -2, 8], &rep("west", 1))?;
    for p in [[10, -2, 8], [10, -2, 9], [10, -2, 10], [11, -2, 10], [12, -2, 10]] {
        b.on(p, DUST)?;
    }
    // C's tap
    b.on([6, -2, 10], &comp("north", "compare"))?;
    b.on([6, -2, 11], &rep("north", 1))?;
    for z in 12..=14 {
        b.on([6, -2, z], DUST)?;
    }
    for x in 7..=12 {
        b.on([x, -2, 14], DUST)?;
    }
    // ---- R''s step, ~150 gt after W0 (after the waiting cart has crossed T1/T2 in this release,
    // long before the next visit's windows): a dust on T0, up two stairs to y 1, a repeater line
    // north along x 16, west along z -3, south along x 3, down two stairs into j's top repeater
    b.on([14, -1, 8], DUST)?;
    b.on([15, 0, 8], DUST)?;
    b.on([16, 1, 8], DUST)?;
    let step_delay: u8 = 4;
    for z in (-2..=7).rev() {
        b.on([16, 1, z], &rep("south", step_delay))?;
    }
    b.on([16, 1, -3], DUST)?;
    for x in (4..=15).rev() {
        b.on([x, 1, -3], &rep("east", if x % 2 == 0 { step_delay } else { 1 }))?;
    }
    b.on([3, 1, -3], DUST)?;
    for z in -2..=6 {
        b.on([3, 1, z], &rep("north", 1))?;
    }
    b.on([3, 1, 7], DUST)?;
    b.on([3, 0, 8], DUST)?;
    b.on([3, -1, 9], DUST)?;
    // ---- router: T1 = (the waiting cart's k != 0) = (R' != 1) = NOT l1, off B's tap line, down to
    // y -4 and south along x 11; T2 = (k = 2) = (R' = 0) = l0, a second comparator on A (west), a
    // torch on the block it powers, south along x 2
    b.on([11, -3, 11], DUST)?;
    let t1: Vec<P3> = (12..=22).map(|z| [11, -4, z]).collect();
    b.wire([11, -3, 11], &t1, 6)?;
    b.ports.insert("output.t1".into(), [11, -4, 23]);
    b.on([4, -2, 8], &comp("east", "compare"))?;
    b.on([3, -2, 8], &rep("east", 1))?;
    b.put([2, -2, 8], STONE)?;
    b.put([2, -2, 9], "minecraft:redstone_wall_torch[facing=south,lit=true]")?;
    let t2: Vec<P3> = (10..=22).map(|z| [2, -2, z]).collect();
    b.wire([2, -2, 9], &t2, 10)?;
    b.ports.insert("output.t2".into(), [2, -2, 23]);
    // ---- the visit-delay register: a ring of droppers D0 (INJ) .. D5 and hoppers, D0 (18, -2, -2)
    // east along z -2 to H2 (23), back west along z -1 to H5 (18) into D0. The REL clock is a dust
    // patch on the tops of D1..D5 and the hoppers between them (hoppers locked while it is high, so
    // every item moves exactly one stage a release); INJ fires on W0 (a visit) through the line
    // along x 16 from T0's east repeater. 3 items in INJ. D5 holds a visit's item during the 4th
    // release after it: SREL = D5 and RELd.
    let reg: [(P3, &str); 12] = [
        ([18, -2, -2], "minecraft:dropper[facing=east,triggered=false]"),
        ([19, -2, -2], "minecraft:hopper[enabled=true,facing=east]"),
        ([20, -2, -2], "minecraft:dropper[facing=east,triggered=false]"),
        ([21, -2, -2], "minecraft:hopper[enabled=true,facing=east]"),
        ([22, -2, -2], "minecraft:dropper[facing=east,triggered=false]"),
        ([23, -2, -2], "minecraft:hopper[enabled=true,facing=south]"),
        ([23, -2, -1], "minecraft:dropper[facing=west,triggered=false]"),
        ([22, -2, -1], "minecraft:hopper[enabled=true,facing=west]"),
        ([21, -2, -1], "minecraft:dropper[facing=west,triggered=false]"),
        ([20, -2, -1], "minecraft:hopper[enabled=true,facing=west]"),
        ([19, -2, -1], "minecraft:dropper[facing=west,triggered=false]"),
        ([18, -2, -1], "minecraft:hopper[enabled=true,facing=north]"),
    ];
    for (p, s) in reg {
        b.put(p, s)?;
    }
    b.nbt.insert([18, -2, -2], "{Items:[{Slot:0b,id:\"minecraft:stone\",count:3}]}".into());
    for p in [[20, -1, -2], [21, -1, -2], [22, -1, -2], [23, -1, -2], [23, -1, -1], [22, -1, -1], [21, -1, -1], [20, -1, -1], [19, -1, -1]] {
        b.put(p, DUST)?;
    }
    b.on([24, -1, -1], &rep("east", 1))?;
    // INJ's trigger: T0 -> a repeater east into a block (16, -2, 8), dust north along x 16, a
    // repeater east into INJ
    b.on([15, -2, 8], &rep("west", 1))?;
    b.put([16, -2, 8], STONE)?;
    let inj: Vec<P3> = (-2..=7).rev().map(|z| [16, -2, z]).collect();
    b.wire([16, -2, 8], &inj, 0)?;
    b.on([17, -2, -2], &rep("west", 1))?;
    // the REL branch: east along z -6, the clock south along x 25 and up into the patch's repeater;
    // RELd on along z -6 to x 27 and south along x 27 (its delay), west along z 2
    // (the branch keeps off (15, -2, -5..-6): the loop's REL line feeds the input block from the
    // north through (14, -2, -6) and a repeater, and must not touch it, or it latches)
    b.on([15, -2, -4], DUST)?;
    b.on([16, -2, -4], DUST)?;
    b.on([16, -2, -5], DUST)?;
    let relb: Vec<P3> = (16..=25).map(|x| [x, -2, -6]).collect();
    b.wire([16, -2, -5], &relb, 8)?;
    for z in -5..=-2 {
        b.on([25, -2, z], DUST)?;
    }
    b.on([25, -1, -1], DUST)?;
    b.on([26, -2, -6], DUST)?;
    b.on([27, -2, -6], DUST)?;
    let reld_delay: u8 = 4;
    for z in -5..=1 {
        b.on([27, -2, z], &rep("north", reld_delay))?;
    }
    b.on([27, -2, 2], DUST)?;
    b.on([26, -2, 2], DUST)?;
    b.on([25, -2, 2], DUST)?;
    b.on([24, -2, 2], DUST)?;
    // SA = D5 - NOT RELd: D5's comparator (south), a repeater, SA; NOT RELd = a torch on the block
    // RELd powers, its dust on SA's east side; SA's output south to `output.srel`
    b.on([19, -2, 0], &comp("north", "compare"))?;
    b.on([19, -2, 1], &rep("north", 1))?;
    b.on([19, -2, 2], &comp("north", "subtract"))?;
    b.on([20, -2, 2], DUST)?;
    b.put([21, -2, 2], "minecraft:redstone_wall_torch[facing=west,lit=true]")?;
    b.put([22, -2, 2], STONE)?;
    b.on([23, -2, 2], &rep("east", 1))?;
    b.on([19, -2, 3], &rep("north", 1))?;
    b.ports.insert("output.srel".into(), [19, -2, 4]);
    Ok(b)
}

/// The module written alone (its ports driven by the bench: `door drive`).
pub fn control_gen(out: &Path, sched: &Path) -> Result<()> {
    let b = control_blocks(sched)?;
    let mut d = Design::parse("name: carousel-siding-control\n")?;
    d.blocks = b.blocks;
    d.block_entities = b.nbt;
    d.ports = b.ports;
    d.save(out)?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}

/// A wire through `cells` (each the next horizontally, or one up/down and one over: a stair), a
/// repeater every `every` cells on straight flat runs pointing along it; refuses a cell whose
/// neighbours (same level, or a stair's reach) hold dust that is not this wire's, and a stair
/// whose upper landing has something solid over the lower cell.
fn route(b: &mut B, from: P3, cells: &[P3], every: usize) -> Result<()> {
    use crate::schedgen::{DUST, rep};
    let mine: std::collections::BTreeSet<P3> = cells.iter().copied().chain(std::iter::once(from)).collect();
    let mut prev = from;
    // dust cells since the last repeater (starting full: the first flat cell takes one, whatever the
    // source's strength): a repeater goes on the first flat cell (with a flat cell after it) once
    // `every` dust cells have passed, stairs included
    let mut run = every;
    for (i, &c) in cells.iter().enumerate() {
        let (dx, dy, dz) = (c[0] - prev[0], c[1] - prev[1], c[2] - prev[2]);
        if dx.abs() + dz.abs() != 1 || dy.abs() > 1 {
            bail!("route: {prev:?} -> {c:?} is not a step");
        }
        if dy != 0 {
            let lower = if dy > 0 { prev } else { c };
            let over = [lower[0], lower[1] + 1, lower[2]];
            if b.blocks.get(&over).is_some_and(|s| !s.contains("air")) && !mine.contains(&over) {
                bail!("route: the stair {prev:?} -> {c:?} has {:?} over its lower cell", b.blocks.get(&over));
            }
        }
        for n in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1], [1, -1, 0], [-1, -1, 0], [0, -1, 1], [0, -1, -1]] {
            let q = [c[0] + n[0], c[1] + n[1], c[2] + n[2]];
            if !mine.contains(&q) && b.blocks.get(&q).is_some_and(|s| s.contains("redstone_wire")) {
                bail!("route: {c:?} would touch foreign dust at {q:?} ({})", b.blocks[&q]);
            }
        }
        // (a repeater only on a straight flat cell: the next cell one more step the same way)
        let next_straight = cells.get(i + 1).is_some_and(|n| n[1] == c[1] && n[0] - c[0] == dx && n[2] - c[2] == dz);
        let prev_flat = dy == 0;
        if every > 0 && run >= every && prev_flat && next_straight {
            let facing = match (dx, dz) {
                (1, 0) => "west",
                (-1, 0) => "east",
                (0, 1) => "north",
                _ => "south",
            };
            b.on(c, &rep(facing, 1))?;
            run = 0;
        } else {
            b.on(c, DUST)?;
            run += 1;
            if run > 14 {
                bail!("route: dust {c:?} is {run} cells past its last repeater (dead)");
            }
        }
        prev = c;
    }
    Ok(())
}

fn seg_to(cells: &mut Vec<P3>, to: P3) {
    let mut c = *cells.last().expect("seg_to after a start");
    while c != to {
        let s = |a: i32, b: i32| (b - a).signum();
        if c[1] != to[1] {
            c = [c[0] + s(c[0], to[0]), c[1] + s(c[1], to[1]), c[2] + if c[0] == to[0] { s(c[2], to[2]) } else { 0 }];
        } else if c[0] != to[0] {
            c = [c[0] + s(c[0], to[0]), c[1], c[2]];
        } else {
            c = [c[0], c[1], c[2] + s(c[2], to[2])];
        }
        cells.push(c);
    }
}

/// The module's place in the loop (its frame's 0 0 0), under the ground west of the siding.
pub(crate) const MODULE_AT: P3 = [-59, -4, 0];

/// The divert unit, next to the departure corner (REL from the core reaches it in ~6 gt; the far
/// module's own visit pulse comes ~30 gt after REL, too late for the corner): G2' = REL and not
/// (NV or REL 10 gt late) at (20, -6, 10), a 10-gt pulse on a visit release; T0 its block, the
/// chain north along x 20 (T1 = T0 + 10 gt, T2 = T0 + 20 gt, 4 cells apart... 6 apart here), P_k
/// east of each tap (comparators, rear the tap, side NOT l_k from R''); their outputs up two
/// stairs east to an OR column along x 24 at y -4, west along z 0 into the corner's repeater.
/// R'' is a copy of the module's k counter (2 items in 3 droppers), stepped ~120 gt after each
/// visit pulse (in sync with the module's R'); the router (T1 = NOT l1, T2 = l0) hangs off it.
fn divert_unit(b: &mut B, nv_in: P3) -> Result<(P3, P3)> {
    use crate::schedgen::{DUST, rep};
    // REL: a second tap on the core's REL dust (0, -1, 11), east along z 11 at y -1, down to y -6,
    // north into G2''s rear (20, -6, 11)
    b.on([1, -1, 11], &rep("west", 1))?;
    let mut rel = vec![[2, -1, 11]];
    seg_to(&mut rel, [8, -1, 11]);
    seg_to(&mut rel, [13, -6, 11]);
    seg_to(&mut rel, [20, -6, 11]);
    route(b, [1, -1, 11], &rel, 6)?;
    // REL 10 gt late into G2''s west side with NV: a junction dust (18, -6, 10) fed by two repeaters
    b.on([19, -6, 12], DUST)?;
    b.on([18, -6, 12], DUST)?;
    b.on([18, -6, 11], &rep("south", 4))?;
    b.on([18, -6, 10], DUST)?;
    b.on([19, -6, 10], &rep("west", 1))?;
    b.on([20, -6, 10], &comp("south", "subtract"))?;
    // NV's last repeater into the junction from the west
    b.on([17, -6, 10], &rep("west", 1))?;
    let mut nv = vec![nv_in];
    seg_to(&mut nv, [16, -6, 10]);
    route(b, [nv_in[0] - 1, nv_in[1], nv_in[2]], &nv, 6)?;
    // the chain north: T0 (20, -6, 9), five repeaters, T1 (20, -6, 3), five, T2 (20, -6, -3)
    let tz = [9, 3, -3];
    for (k, &z) in tz.iter().enumerate() {
        b.put([20, -6, z], STONE)?;
        if k < 2 {
            for dz in 1..=5 {
                b.on([20, -6, z - dz], &rep("south", 1))?;
            }
        }
        // P_k east of the tap, its side from the north: a repeater fed by the row of NOT l_k
        b.on([21, -6, z], &comp("west", "subtract"))?;
        b.on([21, -6, z - 1], &rep("north", 1))?;
        b.on([21, -6, z - 2], DUST)?;
        // its output east, up two stairs to the OR column (24, -4, z)
        b.on([22, -6, z], DUST)?;
        b.on([23, -5, z], DUST)?;
        b.on([24, -4, z], DUST)?;
    }
    let mut or = vec![[24, -4, 8]];
    seg_to(&mut or, [24, -4, -2]);
    route(b, [24, -4, 9], &or, 0)?;
    let mut out = vec![[24, -4, 0], [23, -4, 0]];
    seg_to(&mut out, [11, -4, 0]);
    route(b, [24, -4, 1], &out, 6)?;
    b.on([10, -4, 0], &rep("east", 1))?;
    // R'': droppers a (26, -6, 4), b (28, -6, 4), c (27, -6, 5), hoppers h (27, -6, 4), H (28, -6, 5),
    // j (26, -6, 5); a -> b -> c -> a; state 0 = c empty at power-on. Rows of NOT l_k west to the
    // P sides (21, -6, z_k - 2): l0 (c, tap south) -> row 7; l1 (a, tap north) -> row 1; l2 (b, tap
    // east) -> row -5.
    let item = "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}";
    let ring: [(P3, &str); 6] = [
        ([26, -6, 4], "minecraft:dropper[facing=east,triggered=false]"),
        ([27, -6, 4], "minecraft:hopper[enabled=true,facing=east]"),
        ([28, -6, 4], "minecraft:dropper[facing=south,triggered=false]"),
        ([28, -6, 5], "minecraft:hopper[enabled=true,facing=west]"),
        ([27, -6, 5], "minecraft:dropper[facing=west,triggered=false]"),
        ([26, -6, 5], "minecraft:hopper[enabled=true,facing=north]"),
    ];
    for (p, s) in ring {
        b.put(p, s)?;
        b.put([p[0], -5, p[2]], DUST)?;
    }
    b.nbt.insert([26, -6, 4], item.into());
    b.nbt.insert([28, -6, 4], item.into());
    // c's tap (state 0) south -> row 7
    b.on([27, -6, 6], &comp("north", "compare"))?;
    b.on([27, -6, 7], &rep("north", 1))?;
    let mut r0 = vec![[27, -6, 8], [26, -6, 8]];
    seg_to(&mut r0, [22, -6, 8]);
    r0.push([22, -6, 7]);
    route(b, [27, -6, 7], &r0, 0)?;
    b.on([21, -6, 7], DUST)?;
    // a's tap (state 1) north -> row 1
    b.on([26, -6, 3], &comp("south", "compare"))?;
    b.on([26, -6, 2], &rep("south", 1))?;
    let mut r1 = vec![[26, -6, 1]];
    seg_to(&mut r1, [22, -6, 1]);
    route(b, [26, -6, 2], &r1, 0)?;
    b.on([21, -6, 1], DUST)?;
    // b's tap (state 2) east -> north along x 31 -> row -5
    b.on([29, -6, 4], &comp("west", "compare"))?;
    b.on([30, -6, 4], &rep("west", 1))?;
    let mut r2 = vec![[31, -6, 4]];
    seg_to(&mut r2, [31, -6, -5]);
    seg_to(&mut r2, [22, -6, -5]);
    route(b, [30, -6, 4], &r2, 8)?;
    b.on([21, -6, -5], DUST)?;
    // R'''s step: off T0's top, a delay line (~120 gt) east and back into j's top
    b.on([20, -5, 9], DUST)?;
    let mut st = vec![[20, -4, 10]];
    seg_to(&mut st, [20, -4, 14]);
    seg_to(&mut st, [34, -4, 14]);
    seg_to(&mut st, [34, -4, 6]);
    seg_to(&mut st, [30, -4, 6]);
    seg_to(&mut st, [29, -5, 6]);
    route(b, [20, -5, 9], &st, 1)?;
    b.on([28, -5, 6], DUST)?;
    // (the router: T1 = NOT l1 = row 1's signal, T2 = l0 = NOT row 7's: returned as their cells)
    Ok(([24, -6, 1], [24, -6, 8]))
}

/// Stamps the control module into the loop at `MODULE_AT` and wires it: REL from the release
/// controller's REL stone (-2, -1, 13), a normal daylight detector for the night window, and the
/// four controls (the corner T's support, T1's, T2's through repeaters, the siding's exit slot
/// through a repeater into its rail). Returns the cells added.
pub(crate) fn wire_control(d: &mut Design, sched: &Path) -> Result<Vec<P3>> {
    use crate::schedgen::{DUST, rep};
    let m = control_blocks(sched)?;
    let at = |q: P3| [q[0] + MODULE_AT[0], q[1] + MODULE_AT[1], q[2] + MODULE_AT[2]];
    let before: std::collections::BTreeSet<P3> = d.blocks.keys().copied().collect();
    let mut b = B { blocks: d.blocks.clone(), ports: Default::default(), nbt: Default::default() };
    for (q, s) in &m.blocks {
        b.put(at(*q), s)?;
    }
    for (q, n) in &m.nbt {
        d.block_entities.insert(at(*q), n.clone());
    }
    // night: the detector over the day line (module (2, -1, 1)), a dust under it, a repeater into
    // the day block
    b.put(at([2, -1, 1]), "minecraft:daylight_detector[inverted=false,power=0]")?;
    b.on(at([2, -2, 1]), DUST)?;
    b.on(at([1, -2, 1]), &rep("east", 1))?;
    // REL in: a dust beside the REL stone, down east to y -6, north along x 4, west along z -10,
    // south along x -45 into the module's feed dust and a repeater into its input block
    // (REL is on the core's dust line (-1..0, -1, 13) -> (0, -1, 11): a repeater reads it from
    // the south side)
    b.on([0, -1, 14], &rep("north", 1))?;
    let mut rel = vec![[0, -1, 15]];
    seg_to(&mut rel, [5, -6, 15]);
    seg_to(&mut rel, [5, -6, -10]);
    seg_to(&mut rel, [-45, -6, -10]);
    seg_to(&mut rel, at([14, -2, -7]));
    route(&mut b, [0, -1, 14], &rel, 6)?;
    // (the feed dust touches the module's REL branch on purpose: the same signal)
    b.on(at([14, -2, -6]), DUST)?;
    b.on(at([14, -2, -5]), &rep("north", 1))?;
    // divert: down east to y -8, east along z 18, north along x 12, up a stair north to y -4, into
    // a repeater west into the corner T's support (glass -> stone)
    let mut dv = vec![at([10, 0, 18])];
    seg_to(&mut dv, [-45, -8, 18]);
    seg_to(&mut dv, [12, -8, 18]);
    seg_to(&mut dv, [12, -8, 9]);
    for k in 1..=4 {
        dv.push([12, -8 + k, 9 - k]);
    }
    seg_to(&mut dv, [12, -4, 0]);
    dv.push([11, -4, 0]);
    route(&mut b, at([10, 0, 17]), &dv, 6)?;
    b.on([10, -4, 0], &rep("east", 1))?;
    b.blocks.insert(DIVERT, STONE.into());
    // T1 and T2: each T's support driven from below by two torches (a block, a torch on it, a
    // block, a torch under the support: the same sense as the line), the line into the lowest
    // block through a repeater from the west. T1: east along z 22 (y -8) to x -20, up a stair east
    // to (-16, -4, 22), east and south along x -15 (y -4) to (-15, -4, 28)
    let tower = |b: &mut B, sup: P3| -> Result<()> {
        b.put([sup[0], sup[1] - 4, sup[2]], STONE)?;
        b.put([sup[0], sup[1] - 3, sup[2]], "minecraft:redstone_torch[lit=true]")?;
        b.put([sup[0], sup[1] - 2, sup[2]], STONE)?;
        b.put([sup[0], sup[1] - 1, sup[2]], "minecraft:redstone_torch[lit=false]")?;
        b.blocks.insert(sup, STONE.into());
        b.on([sup[0] - 1, sup[1] - 4, sup[2]], &rep("west", 1))
    };
    let mut t1 = vec![at([11, -4, 23])];
    seg_to(&mut t1, [-20, -8, 23]);
    for k in 1..=4 {
        t1.push([-20 + k, -8 + k, 23]);
    }
    seg_to(&mut t1, [-15, -4, 23]);
    seg_to(&mut t1, [-15, -4, 28]);
    route(&mut b, at([11, -4, 22]), &t1, 6)?;
    tower(&mut b, T1)?;
    // T2: south to z 25, east along z 25 (y -6) to x -17, south along x -17 to z 29, up a stair
    // south to (-17, -4, 31)
    let mut t2 = vec![at([2, -2, 23])];
    t2.push([t2[0][0], t2[0][1], 24]);
    t2.push([t2[0][0], t2[0][1], 25]);
    seg_to(&mut t2, [-17, -6, 25]);
    seg_to(&mut t2, [-17, -6, 29]);
    t2.push([-17, -5, 30]);
    t2.push([-17, -4, 31]);
    route(&mut b, at([2, -2, 22]), &t2, 6)?;
    tower(&mut b, T2)?;
    // siding release: east along z 4 (y -6), up a stair east to (-15, 1, 4), south along x -15 to
    // z 8 (clear of the siding's north dust line at y 2), west to (-16, 1, 8), a repeater north into
    // the exit slot's rail (-16, 1, 10)
    let mut sr = vec![at([19, -2, 4])];
    seg_to(&mut sr, [-22, -6, 4]);
    for k in 1..=7 {
        sr.push([-22 + k, -6 + k, 4]);
    }
    seg_to(&mut sr, [-15, 1, 8]);
    sr.push([-16, 1, 8]);
    route(&mut b, at([19, -2, 3]), &sr, 6)?;
    b.on([-16, 1, 9], &rep("north", 1))?;
    if std::env::var("MCTAS_SIDING_CELLS").is_ok() {
        for q in [[-17, -6, 24], [-17, -6, 25], [-17, -6, 26], [-17, -6, 27], [-17, -6, 28], [-17, -6, 29], [-17, -7, 26], [-47, -8, 23], [-46, -8, 23], [-45, -8, 23], [-44, -8, 23], [-43, -8, 23], [-42, -8, 23], [-41, -8, 23], [-48, -9, 23]] {
            println!("cell {q:?}: {:?}", b.blocks.get(&q));
        }
    }
    let added: Vec<P3> = b.blocks.keys().copied().filter(|p| !before.contains(p)).collect();
    d.blocks = b.blocks;
    Ok(added)
}
