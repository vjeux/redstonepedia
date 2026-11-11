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
pub(crate) const T1: P3 = [-15, 0, 28];
pub(crate) const T2: P3 = [-17, 0, 31];
pub(crate) const RELEASE: P3 = [-16, 0, 10];
pub(crate) const HOLDS: [P3; 3] = [[-12, 0, 28], [-12, 0, 31], [-12, 0, 34]];

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
    // the siding's own build list (`build: last x,y,z ...`, its cells placed after all others),
    // moved into the loop's frame; joined with the drop ends' below
    let mut last: Vec<P3> = Vec::new();
    if let Some(bl) = s.meta.get("build") {
        let cells = bl.strip_prefix("last ").ok_or_else(|| anyhow::anyhow!("the siding's `build: {bl}` is not a `last` list"))?;
        for c in cells.split_whitespace() {
            let v: Vec<i32> = c.split(',').map(|n| n.parse()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("the siding's build cell {c}: {e}"))?;
            if v.len() != 3 {
                bail!("the siding's build cell {c} is not x,y,z");
            }
            last.push(at(&[v[0], v[1], v[2]]));
        }
    }
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
    // ---- the exit route on to T1 (-15, 1, 28): x -13 south to z 25, west along z 26 to x -15,
    // south into T1 (the tree sits west of the release controller's taps, so the cells under the
    // T's and the holds are free for their drivers)
    for z in 12..=25 {
        lay(&mut b, [-13, 1, z], &rail("north_south"), false)?;
    }
    lay(&mut b, [-13, 1, 26], &rail("north_west"), false)?;
    lay(&mut b, [-14, 1, 26], &rail("east_west"), false)?;
    lay(&mut b, [-15, 1, 26], &rail("south_east"), false)?;
    lay(&mut b, [-15, 1, 27], &rail("north_south"), false)?;
    // T1: stem north, unpowered north_east (east: slot 14's line), powered north_west (T2)
    b.put([-15, 1, 28], &rail("north_east"))?;
    b.put(T1, GLASS)?;
    // T1 west to T2 (-17, 1, 31), stem north: unpowered east (slot 13's line), powered west
    lay(&mut b, [-16, 1, 28], &rail("east_west"), false)?;
    lay(&mut b, [-17, 1, 28], &rail("south_east"), false)?;
    lay(&mut b, [-17, 1, 29], &rail("north_south"), false)?;
    lay(&mut b, [-17, 1, 30], &rail("north_south"), false)?;
    b.put([-17, 1, 31], &rail("north_east"))?;
    b.put(T2, GLASS)?;
    // T2 west round to z 34 (slot 12's line)
    lay(&mut b, [-18, 1, 31], &rail("east_west"), false)?;
    lay(&mut b, [-19, 1, 31], &rail("south_east"), false)?;
    lay(&mut b, [-19, 1, 32], &rail("north_south"), false)?;
    lay(&mut b, [-19, 1, 33], &rail("north_south"), false)?;
    lay(&mut b, [-19, 1, 34], &rail("north_east"), false)?;
    // ---- the three drop lines, each with its hold (ironfarm 14:42: one hold per line, a short
    // leg from the hold to the drop): flat east to x -15, a climb ascending east (-14, 1, z), flat
    // (-13, 2, z), the hold (-12, 1, z) (a braked powered rail ascending west: the cart rolls down
    // onto it and stops; its support (-12, 0, z) is the kick control), flat east (-11..-4, 1, z)
    // (glass under the last three), the stone stop (-2, 1, z)
    let lines: [(i32, i32); 3] = [(28, -14), (31, -16), (34, -18)];
    for (z, x0) in lines {
        for x in x0..=-15 {
            lay(&mut b, [x, 1, z], &rail("east_west"), false)?;
        }
        lay(&mut b, [-14, 1, z], &rail("ascending_east"), false)?;
        lay(&mut b, [-13, 2, z], &rail("east_west"), false)?;
        b.put([-12, 1, z], "minecraft:powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
        b.put([-12, 0, z], GLASS)?;
        for x in -11..=-4 {
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
    // the drop ends go in after every other rail (ironfarm 15:13): placed by (y, x, z) they would
    // come before the slots' peaks, and each slot's slope, with one link so far, would take its
    // drop end (seen built in order: the slopes turned ascending_west)
    let mut cells: Vec<String> = [[-4, 1, 28], [-4, 1, 31], [-4, 1, 34]].iter().chain(last.iter()).map(|q| format!("{},{},{}", q[0], q[1], q[2])).collect();
    // (a build list the loop file already has comes first)
    if let Some(prev) = d.meta.get("build") {
        let prev = prev.strip_prefix("last ").ok_or_else(|| anyhow::anyhow!("the loop's `build: {prev}` is not a `last` list"))?;
        cells.splice(0..0, prev.split_whitespace().map(|c| c.to_string()));
    }
    cells.dedup();
    d.meta.insert("build".into(), format!("last {}", cells.join(" ")));
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
    // (RELd and SA moved out: the hold's kick unit near T1 takes REL from the core; the module
    // only says which release is the drop's, steadily)
    // ---- NS5 = NOT (a visit's item in D5): D5's comparator (south), a repeater into a block, a
    // torch on it, out east along z 3 (`output.ns5`); high except from the clock of the 4th
    // release after a visit to the clock of the 5th
    b.on([19, -2, 0], &comp("north", "compare"))?;
    b.put([19, -2, 1], STONE)?;
    b.put([19, -2, 2], "minecraft:redstone_wall_torch[facing=south,lit=true]")?;
    for x in 20..=27 {
        b.on([x, -2, 2], DUST)?;
    }
    b.ports.insert("output.ns5".into(), [27, -2, 2]);
    // ---- NV out: a dust on NV's block (-4, 0, 3), up two stairs to y 3 (over the D ring), east
    // along z 3 to `output.nv` (28, 4, 3): at y 4, two above the D ring's loop dust (y 1): a line
    // at y 3 strongly powers its support stones at y 2, which sit right over the loop dust and so
    // count for the ring's droppers' quasi-connectivity (they jammed the ring)
    b.on([-4, 1, 3], DUST)?;
    b.on([-3, 2, 3], DUST)?;
    b.on([-2, 3, 3], DUST)?;
    let nvo: Vec<P3> = (-1..=28).map(|x| [x, 4, 3]).collect();
    b.wire([-2, 3, 3], &nvo, 10)?;
    b.ports.insert("output.nv".into(), [28, 4, 3]);
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
    route_d(b, from, cells, every, every, 1)
}

/// `route` with the dust count since the source's last repeater (`run0`: 0 right after a repeater,
/// `every` for "take one on the first flat cell") and the repeaters' delay.
fn route_d(b: &mut B, from: P3, cells: &[P3], every: usize, run0: usize, delay: u8) -> Result<()> {
    use crate::schedgen::{DUST, rep};
    let mine: std::collections::BTreeSet<P3> = cells.iter().copied().chain(std::iter::once(from)).collect();
    let mut prev = from;
    // dust cells since the last repeater (starting full: the first flat cell takes one, whatever the
    // source's strength): a repeater goes on the first flat cell (with a flat cell after it) once
    // `every` dust cells have passed, stairs included
    let mut run = run0;
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
            b.on(c, &rep(facing, delay))?;
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

/// The wired pocket stand's REL (farmopt's gates, `queue-loop --stand-wired`: its input cell
/// (8, -2, 2), on from the slot's +10 for 40 gt opens the three gates and resets the latch),
/// driven from the release controller's REL (the core's dust (0, -1, 11), REL at golem +4..+16): a
/// tap north (0, -1, 10) into block B0 (0, -1, 9), a delay line east along z 9 (B1 = B0 + 10 gt,
/// B2 = + 20, B3 = + 28), each block tapped south into an OR line along z 7 (four 12-gt copies:
/// golem +10..+50), down to (8, -2, 4) and a repeater north into the input cell (glass -> stone).
pub(crate) fn stand_rel(d: &mut Design) -> Result<Vec<P3>> {
    use crate::schedgen::{DUST, rep};
    let cell: P3 = [8, -2, 2];
    if d.blocks.get(&cell).map(|s| s.as_str()) != Some(GLASS) {
        bail!("the pocket stand's REL cell {cell:?} is {:?}, not glass: the loop file changed", d.blocks.get(&cell));
    }
    let before: std::collections::BTreeSet<P3> = d.blocks.keys().copied().collect();
    let mut b = B { blocks: d.blocks.clone(), ports: Default::default(), nbt: Default::default() };
    b.on([0, -1, 10], &rep("south", 1))?;
    b.put([0, -1, 9], STONE)?;
    b.on([1, -1, 9], &rep("west", 4))?;
    b.on([2, -1, 9], &rep("west", 1))?;
    b.put([3, -1, 9], STONE)?;
    b.on([4, -1, 9], &rep("west", 4))?;
    b.on([5, -1, 9], &rep("west", 1))?;
    b.put([6, -1, 9], STONE)?;
    b.on([7, -1, 9], &rep("west", 4))?;
    b.put([8, -1, 9], STONE)?;
    for x in [0, 3, 6, 8] {
        b.on([x, -1, 8], &rep("north", 1))?;
    }
    for x in 0..=8 {
        b.on([x, -1, 7], DUST)?;
    }
    b.on([8, -1, 6], DUST)?;
    b.on([8, -1, 5], DUST)?;
    b.on([8, -2, 4], DUST)?;
    b.on([8, -2, 3], &rep("south", 1))?;
    b.blocks.insert(cell, STONE.into());
    let added: Vec<P3> = b.blocks.keys().copied().filter(|p| !before.contains(p)).collect();
    d.blocks = b.blocks;
    Ok(added)
}

/// The R'' step line's repeater delay (the line has ~25 repeaters).
fn step_delay() -> u8 {
    3
}

/// The module's place in the loop (its frame's 0 0 0), under the ground west of the siding.
pub(crate) const MODULE_AT: P3 = [-59, -4, 0];
/// The night window's daylight detector, under the open sky north of the module.
pub(crate) const DETECTOR: P3 = [-56, 1, -26];

/// The divert unit, next to the departure corner (REL from the core reaches it in ~6 gt; the far
/// module's own visit pulse comes ~30 gt after REL, too late for the corner): G2' = REL and not
/// (NV or REL 10 gt late) at (20, -6, 10), a 10-gt pulse on a visit release; T0 its block, the
/// chain north along x 20 (T1 = T0 + 10 gt, T2 = T0 + 20 gt, 4 cells apart... 6 apart here), P_k
/// east of each tap (comparators, rear the tap, side NOT l_k from R''); their outputs up two
/// stairs east to an OR column along x 24 at y -4, west along z 0 into the corner's repeater.
/// R'' is a copy of the module's k counter (2 items in 3 droppers), stepped ~120 gt after each
/// visit pulse (in sync with the module's R'); the router (T1 = NOT l1, T2 = l0) hangs off it.
fn divert_unit(b: &mut B) -> Result<(P3, P3)> {
    use crate::schedgen::{DUST, rep};
    // REL: a second tap on the core's REL dust (0, -1, 11), east along z 11 at y -1 to x 8, south to
    // z 13, down a stair east to y -6, east along z 13, north to (20, -6, 12), a repeater into G2''s
    // rear (full strength for the windows' long OR path; one repeater mid-way)
    b.on([1, -1, 11], &rep("west", 1))?;
    let mut rel = vec![[2, -1, 11]];
    seg_to(&mut rel, [8, -1, 11]);
    seg_to(&mut rel, [8, -1, 13]);
    seg_to(&mut rel, [13, -6, 13]);
    seg_to(&mut rel, [20, -6, 13]);
    rel.push([20, -6, 12]);
    route_d(b, [1, -1, 11], &rel, 13, 0, 1)?;
    b.on([20, -6, 11], &rep("south", 1))?;
    // REL 10 gt late into G2''s west side with NV: a junction dust (18, -6, 10) fed by two
    // repeaters, REL's from the south (18, -6, 11) off a branch (18, -6, 12), NV's from the west
    // (17, -6, 10) (NV comes along z 10 into (16, -6, 10))
    b.on([18, -6, 12], DUST)?;
    b.on([18, -6, 11], &rep("south", 4))?;
    b.on([18, -6, 10], DUST)?;
    b.on([19, -6, 10], &rep("west", 2))?;
    b.on([20, -6, 10], &comp("south", "subtract"))?;
    b.on([17, -6, 10], &rep("west", 1))?;
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
        // (its feed dust (21, -6, z - 2) is the end of R'''s row)
        // its output east, up two stairs to the OR column (24, -4, z)
        b.on([22, -6, z], DUST)?;
        b.on([23, -5, z], DUST)?;
        b.on([24, -4, z], DUST)?;
    }
    for z in -2..=8 {
        b.on([24, -4, z], DUST)?;
    }
    let mut out = vec![[23, -4, 0]];
    seg_to(&mut out, [11, -4, 0]);
    route(b, [24, -4, 0], &out, 12)?;
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
    // (along z 8 only to x 25, then z 7: clear of P0's output stair at z 9)
    let mut r0 = vec![[27, -6, 8], [26, -6, 8], [25, -6, 8], [25, -6, 7]];
    seg_to(&mut r0, [21, -6, 7]);
    route(b, [27, -6, 7], &r0, 0)?;
    // a's tap (state 1) north -> row 1
    b.on([26, -6, 3], &comp("south", "compare"))?;
    b.on([26, -6, 2], &rep("south", 1))?;
    let mut r1 = vec![[26, -6, 1]];
    seg_to(&mut r1, [21, -6, 1]);
    route(b, [26, -6, 2], &r1, 0)?;
    // b's tap (state 2) east -> north along x 31 -> row -5
    b.on([29, -6, 4], &comp("west", "compare"))?;
    b.on([30, -6, 4], &rep("west", 1))?;
    let mut r2 = vec![[31, -6, 4]];
    seg_to(&mut r2, [31, -6, -5]);
    seg_to(&mut r2, [21, -6, -5]);
    route(b, [30, -6, 4], &r2, 8)?;
    // R'''s step: off T0's top, a delay line east and back into j's top, ~140 gt after the visit
    // pulse: after this visit's own windows (+0..+30), and after the cart of the visit 4 releases
    // back has gone through T1/T2 (the router reads R'' as "the next visit's k" until then)
    b.on([20, -5, 9], DUST)?;
    let mut st = vec![[20, -4, 10]];
    seg_to(&mut st, [20, -4, 14]);
    seg_to(&mut st, [34, -4, 14]);
    seg_to(&mut st, [34, -4, 6]);
    seg_to(&mut st, [30, -4, 6]);
    seg_to(&mut st, [29, -5, 6]);
    route_d(b, [20, -5, 9], &st, 1, 1, step_delay())?;
    b.on([28, -5, 6], DUST)?;
    // (the router: T1 = NOT l1 = row 1's signal, T2 = l0 = NOT row 7's: returned as their cells)
    Ok(([24, -6, 1], [25, -6, 8]))
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
    // night: an INVERTED daylight detector under the open sky north of the module (DETECTOR, y 1:
    // nothing of the farm over it; its output is 15 - (sky light - darkening): 0 by day, 3 from
    // about day time 12,500 to 23,640), three dusts south (so the repeater after them needs 3 or
    // more), a repeater into a block, a torch on the block's south face: the line is ON by day and
    // also while the detector is missing (a runner places detectors after its settle: the module
    // must not start a visit then), down a stair south to y -7, south along x -56, up into the day
    // line's dust (module (2, -2, 1)) and its repeater into the day block (the detector had been
    // under the module: no sky, the night window open all day; ironfarm 15:37)
    b.put(DETECTOR, "minecraft:daylight_detector[inverted=true,power=0]")?;
    for dz in 1..=3 {
        b.on([DETECTOR[0], DETECTOR[1], DETECTOR[2] + dz], DUST)?;
    }
    b.on([DETECTOR[0], DETECTOR[1], DETECTOR[2] + 4], &rep("north", 1))?;
    b.put([DETECTOR[0], DETECTOR[1], DETECTOR[2] + 5], STONE)?;
    b.put([DETECTOR[0], DETECTOR[1], DETECTOR[2] + 6], "minecraft:redstone_wall_torch[facing=south,lit=true]")?;
    let mut dl = vec![[DETECTOR[0], DETECTOR[1], DETECTOR[2] + 7]];
    for k in 1..=8 {
        dl.push([DETECTOR[0], DETECTOR[1] - k, DETECTOR[2] + 7 + k]);
    }
    // (at y -7 under the module's REL bus (module y -1, z -4), then up a stair into (3, -2, 1))
    seg_to(&mut dl, at([3, -3, -1]));
    dl.push(at([3, -2, 0]));
    dl.push(at([3, -2, 1]));
    route_d(&mut b, [DETECTOR[0], DETECTOR[1], DETECTOR[2] + 6], &dl, 6, 0, 1)?;
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
    // ---- NV, the module's steady "the next release is a visit", to the divert unit: from its port
    // (28, 4, 3) down a stair south to y -9, east along z 12, north to z 10, up a stair east into
    // (16, -6, 10)
    let nv_port = at([28, 4, 3]);
    let mut nv = vec![[nv_port[0], nv_port[1], 4], [nv_port[0], nv_port[1], 5]];
    for k in 1..=9 {
        nv.push([nv_port[0], nv_port[1] - k, 5 + k]);
    }
    seg_to(&mut nv, [13, -9, 14]);
    seg_to(&mut nv, [13, -9, 10]);
    nv.push([14, -8, 10]);
    nv.push([15, -7, 10]);
    nv.push([16, -6, 10]);
    route(&mut b, nv_port, &nv, 6)?;
    let (t1_row, t2_row) = divert_unit(&mut b)?;
    b.blocks.insert(DIVERT, STONE.into());
    // ---- T1 and T2 from the unit's ring: each T's support driven from below by two torches (a
    // block, a torch on it, a block, a torch under the support: the same sense as the line), the
    // line into the lowest block through a repeater from the west
    let tower = |b: &mut B, sup: P3| -> Result<()> {
        b.put([sup[0], sup[1] - 4, sup[2]], STONE)?;
        b.put([sup[0], sup[1] - 3, sup[2]], "minecraft:redstone_torch[lit=true]")?;
        b.put([sup[0], sup[1] - 2, sup[2]], STONE)?;
        b.put([sup[0], sup[1] - 1, sup[2]], "minecraft:redstone_torch[lit=false]")?;
        b.blocks.insert(sup, STONE.into());
        b.on([sup[0] - 1, sup[1] - 4, sup[2]], &rep("west", 1))
    };
    // T1 = NOT l1: off row 1 at (24, -6, 1), north, down a stair to y -11, west along z -5, south
    // along x -17, up a stair south to (-17, -4, 27), into (-17, -4, 28)
    let mut t1 = vec![[t1_row[0], -6, 0]];
    for k in 1..=5 {
        t1.push([t1_row[0], -6 - k, -k]);
    }
    seg_to(&mut t1, [-17, -11, -5]);
    seg_to(&mut t1, [-17, -11, 20]);
    for k in 1..=7 {
        t1.push([-17, -11 + k, 20 + k]);
    }
    t1.push([-17, -4, 28]);
    route(&mut b, t1_row, &t1, 6)?;
    tower(&mut b, T1)?;
    // T2 = l0 = NOT (row 7's NOT l0): off row 7 at (24, -6, 8), a repeater south into a block
    // (24, -6, 10), a torch on it, down a stair south to y -13, west along z 19, south along x -19,
    // up a stair south to (-19, -4, 30), into (-19, -4, 31)
    b.on([t2_row[0], -6, 9], &rep("north", 1))?;
    b.put([t2_row[0], -6, 10], STONE)?;
    b.put([t2_row[0], -6, 11], "minecraft:redstone_wall_torch[facing=south,lit=true]")?;
    let mut t2 = vec![[t2_row[0], -6, 12]];
    for k in 1..=7 {
        t2.push([t2_row[0], -6 - k, 12 + k]);
    }
    seg_to(&mut t2, [-19, -13, 19]);
    seg_to(&mut t2, [-19, -13, 21]);
    for k in 1..=9 {
        t2.push([-19, -13 + k, 21 + k]);
    }
    t2.push([-19, -4, 31]);
    route(&mut b, [t2_row[0], -6, 11], &t2, 6)?;
    tower(&mut b, T2)?;
    // ---- the holds' kick: REL from the core's REL stone (-2, -1, 13) through (-2, -1, 14), down a
    // stair south to y -4, west along z 18 to x -21, a delay (`kick_delay` repeaters of 4 gt)
    // south along x -21, the gate KA (-21, -4, 30): REL late minus NS5 (the module's NOT "a
    // visit's item in D5": high except from the 4th release after a visit to the 5th), its output
    // south, east along z 37 up to y -1, into the bus along x -13 (y -1, z 30..36) that feeds the
    // three holds' drivers: a dust up a stair onto (-12, 0, z + 2), a repeater north into the
    // hold's support (-12, 0, z)
    let mut kr = vec![[-2, -1, 14], [-2, -1, 15]];
    for k in 1..=3 {
        kr.push([-2, -1 - k, 15 + k]);
    }
    seg_to(&mut kr, [-21, -4, 18]);
    route(&mut b, [-2, -1, 13], &kr, 6)?;
    let kick_delay = 6;
    for k in 0..kick_delay {
        b.on([-21, -4, 19 + k], &rep("north", 4))?;
    }
    for z in 19 + kick_delay..=29 {
        b.on([-21, -4, z], DUST)?;
    }
    b.on([-21, -4, 30], &comp("north", "subtract"))?;
    // NS5 into KA's west side through a repeater (-22, -4, 30), from the module's port: east one,
    // south along x -30 (y -6) to z 30, east to x -25, up a stair east to (-23, -4, 30)
    let ns5_port = at([27, -2, 2]);
    let mut ns = vec![[ns5_port[0] + 1, ns5_port[1], ns5_port[2]]];
    seg_to(&mut ns, [-28, -6, 2]);
    seg_to(&mut ns, [-28, -6, 30]);
    seg_to(&mut ns, [-25, -6, 30]);
    ns.push([-24, -5, 30]);
    ns.push([-23, -4, 30]);
    route(&mut b, ns5_port, &ns, 14)?;
    b.on([-22, -4, 30], &rep("west", 1))?;
    let mut ko = vec![[-21, -4, 31], [-21, -4, 32], [-21, -4, 33], [-21, -4, 34], [-21, -4, 35], [-21, -4, 36], [-21, -4, 37]];
    seg_to(&mut ko, [-18, -4, 37]);
    for k in 1..=3 {
        ko.push([-18 + k, -4 + k, 37]);
    }
    seg_to(&mut ko, [-15, -1, 30]);
    route(&mut b, [-21, -4, 30], &ko, 6)?;
    // each hold's driver: the bus dust (-15, -1, z + 2) up a stair onto (-14, 0, z + 2), a repeater
    // east into a block (-12, 0, z + 2), a repeater north into the hold's support (-12, 0, z) (a
    // block, not a dust, next to the other holds' supports: their kicks stay apart)
    for &h in &HOLDS {
        b.blocks.insert(h, STONE.into());
        b.on([h[0], 0, h[2] + 1], &rep("south", 1))?;
        b.put([h[0], 0, h[2] + 2], STONE)?;
        b.on([h[0] - 1, 0, h[2] + 2], &rep("west", 1))?;
        b.on([h[0] - 2, 0, h[2] + 2], DUST)?;
    }
    // (the divert unit's ring items)
    for (q, n) in &b.nbt {
        d.block_entities.insert(*q, n.clone());
    }
    let added: Vec<P3> = b.blocks.keys().copied().filter(|p| !before.contains(p)).collect();
    d.blocks = b.blocks;
    Ok(added)
}
