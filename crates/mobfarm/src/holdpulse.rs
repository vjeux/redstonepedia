//! split9's napper hold release (day-1, Nov 12 2026; ironspec's ruling 5 + the launch width, 06:46; the
//! hold's drive from 7b90c3f1, accepted 07:34; ironspec 10:49 / 10:50: (b), an ungated 12-gt pulse and
//! the departure seen by a tripwire): a pulse every cycle, never a latch. ARM3 (the third cart's detector
//! rail, (5,0,7), up >= 20 gt a cycle) is cut to WIDTH gt by an edge comparator E (subtract: ARM3's dust in
//! its rear, ARM3 through a delay-2 and a delay-4 repeater on its side), then four 1-tick repeaters, the
//! last powering the drive's input cell (7,-1,8) from ARM3 + 10. 7b90c3f1's own repeater (8,-1,8) facing
//! west reads that cell and strongly powers the stone (9,-1,8) under the hold rail (9,0,8), so the rail
//! rises at ARM3 + 12: D is the rail's rise (7b90c3f1 08:30: its bench powers the rail at ARM3 + D, the
//! input block at ARM3 + D - 2), and D 12. (No repeater beside the rail: its input would be a face of
//! lane 7's merge T.)
//!
//! WIDTH 12 (ironspec 10:09) = max(6 to launch, 7 to clear) + 3 frozen (a login, <= 3 gt, stops the
//! entities while the redstone runs) + 2 margin. The departure is seen by 7b90c3f1's north-south string
//! (8,1,7) over the napper-only cell (8,0,7), its hooks (8,1,6) facing south and (8,1,8) facing north on
//! glass (G1: glass is a sturdy face but no conductor, so a powered hook's anchor powers nothing): a seated
//! rider presses a string over its cart's cell, an empty cart never does (tick-exact's recorded
//! `tripwire-rider` scene, eec7361f5); the resting napper (x 9.2..9.8) and lane 7's riders (x <= 7.8)
//! never reach the string's cell; the controller's state (the FIFO pop, the K / J(-1) timers) keys to the
//! string's rising edge only (a rolling press stays on until the hooks' 10-gt re-check).
//!
//! The bench drives ARM3 through a repeater for 20 gt every 100 gt (a repeater's output, like a detector
//! rail's, turns on inside a game tick), in one of two geometries: `Line` (the hold, then a straight line
//! east under a string) or `Real` (7b90c3f1's cells: the hold (9,0,8) against the slime stop (9,0,9), the T
//! (9,0,7), the booster (8,0,7) on its redstone block, lane 7's merge T (7,0,7) and track, the merge row
//! west, the string and hooks above, the 12-gt chain laid into (7,-1,8)). It prints: the input cell's and
//! the rail's pulses after ARM3 at every phase; a ridden cart at rest placed at every gt between two
//! pulses (it leaves at the next pulse, never before), the string's rising edge (matched within 20 gt of
//! the departure stamp: the rider's box enters the string's cell before the cart's box clears the hold
//! cell), presses while resting, and (Real) the merge row reached; a cart at rest frozen for `freeze` gt
//! (the part at level 32: redstone running, entities frozen) from every offset around the pulse; the launch
//! threshold under the freeze for widths 8, 10, 12 and 14; (Real) a lane-7 rider through its curve; and
//! the positive control, tick-exact's `tripwire-rider` scene cell for cell, against the game's ticks.

use redsim::block::Block;
use redsim::entity::EntityKind;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

type P = [i32; 3];

/// The design: the rail powered at ARM3 + D (the input cell at ARM3 + D - 2) for WIDTH gt, every CYCLE.
/// D 12 (day-1 08:33, on 7b90c3f1's K re-lay: behind = D + 17, ahead = 41 - D against the bars 21 + 2 /
/// 16 + 2; D 10's 27 / 31 left the tightest margin 4, D 12's 29 / 29 leaves 6 / 11; D 11 is off this
/// chain's 2-gt grid).
pub const D: i64 = 12;
pub const WIDTH: i64 = 12;
pub const CYCLE: i64 = 100;
/// E's extra side delay for WIDTH: a delay-2 repeater (redstone ticks) before the delay-4.
pub const EXTRA: i64 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Geo {
    Line,
    Real,
}

/// The cells a run reads: the drive block (placed and removed by the bench), ARM3's repeater (its output
/// = ARM3), the controller's last repeater (OUT, into the input cell), the hold rail, a hook of the
/// departure string, and where the ridden cart is placed (at rest on the hold).
struct Cells {
    drive: P,
    arm3: P,
    out: P,
    hold: P,
    hook: P,
    place: [f64; 3],
}

fn cells(g: Geo) -> Cells {
    match g {
        Geo::Line => Cells { drive: [0, 0, 6], arm3: [0, 0, 5], out: [0, 0, -1], hold: [2, 1, -2], hook: [3, 2, -1], place: [2.5, 1.0625, -1.5] },
        Geo::Real => Cells { drive: [-1, -1, 8], arm3: [0, -1, 8], out: [6, -1, 8], hold: [9, 0, 8], hook: [8, 1, 6], place: [9.5, 0.0625, 8.5] },
    }
}

const DUST: &str = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";
const STRING_NS: &str = "minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]";

fn farm(blocks: Vec<(BlockPos, Block)>, output: BlockPos, lo: BlockPos, hi: BlockPos) -> FarmSpec {
    FarmSpec { blocks, entities: Vec::new(), output, lo, hi, block_entities: Vec::new(), rock: false, nether: None, land: false }
}

/// The Line geometry: the chain north along x 0, the hold rail east-west at (2,1,-2) on its stone, plain
/// rails east, the string over the first rail after the hold between hooks on glass.
fn spec_line(extra: i64) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, s: &str| -> Result<(), String> {
        blocks.push((BlockPos::new(x, y, z), b(s)?));
        Ok(())
    };
    for x in -4..=24 {
        for z in -5..=7 {
            put(x, -1, z, "minecraft:stone")?;
        }
    }
    // (a diode's `facing` points at its input, as in the game)
    // ARM3: a 1-tick repeater from the drive block, into ARM3's dust (E's rear)
    put(0, 0, 5, "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?;
    put(0, 0, 4, DUST)?;
    // ARM3's branch to E's west side: (with `extra`) a delay-`extra` repeater, then the delay-4 repeater
    if extra > 0 {
        for p in [[-1, 0, 4], [-2, 0, 4], [-3, 0, 4], [-3, 0, 3]] {
            put(p[0], p[1], p[2], DUST)?;
        }
        put(-2, 0, 3, &format!("minecraft:repeater[delay={extra},facing=west,locked=false,powered=false]"))?;
    } else {
        for p in [[-1, 0, 4], [-2, 0, 4], [-2, 0, 3]] {
            put(p[0], p[1], p[2], DUST)?;
        }
    }
    put(-1, 0, 3, "minecraft:repeater[delay=4,facing=west,locked=false,powered=false]")?;
    // E: subtract, input from the south (ARM3's dust), output north
    put(0, 0, 3, "minecraft:comparator[facing=south,mode=subtract,powered=false]")?;
    // four 1-tick repeaters north, the last powering the input cell (0,0,-2) = (7,-1,8): a stone block
    for z in [2, 1, 0, -1] {
        put(0, 0, z, "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?;
    }
    put(0, 0, -2, "minecraft:stone")?;
    // 7b90c3f1's repeater (8,-1,8) facing west: input the cell west of it, output east into the stone
    // under the hold rail
    put(1, 0, -2, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?;
    put(2, 0, -2, "minecraft:stone")?;
    // the hold rail (east-west) on that stone, a backstop west of it, plain rails east on stone
    put(2, 1, -2, "minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(1, 1, -2, "minecraft:stone")?;
    for x in 3..=23 {
        put(x, 0, -2, "minecraft:stone")?;
        put(x, 1, -2, "minecraft:rail[shape=east_west,waterlogged=false]")?;
    }
    // the departure string over the first rail after the hold, its hooks on glass north and south
    put(3, 2, 0, "minecraft:glass")?;
    put(3, 2, -4, "minecraft:glass")?;
    put(3, 2, -1, "minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    put(3, 2, -3, "minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    put(3, 2, -2, STRING_NS)?;
    Ok(farm(blocks, BlockPos::new(24, 1, 7), BlockPos::new(-5, -2, -6), BlockPos::new(25, 4, 8)))
}

/// The Real geometry, in 7b90c3f1's frame: the hold, the stop, the T, the booster, lane 7's merge T and
/// track, the merge row west, the departure string; the 12-gt chain laid at y -1 along z 8 into the input
/// cell (7,-1,8) (the bench's route: the real chain's cells are the controller box's).
fn spec_real(extra: i64) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, s: &str| -> Result<(), String> {
        blocks.push((BlockPos::new(x, y, z), b(s)?));
        Ok(())
    };
    // the chain's floor
    for x in -2..=8 {
        for z in 8..=12 {
            put(x, -2, z, "minecraft:stone")?;
        }
    }
    // ARM3: a 1-tick repeater from the drive block (-1,-1,8), into ARM3's dust (1,-1,8) (E's rear)
    put(0, -1, 8, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?;
    put(1, -1, 8, DUST)?;
    // E's south side: ARM3's dust south and east, (with `extra`) a delay-`extra` repeater, the delay-4
    if extra > 0 {
        for p in [[1, -1, 9], [1, -1, 10], [1, -1, 11], [2, -1, 11]] {
            put(p[0], p[1], p[2], DUST)?;
        }
        put(2, -1, 10, &format!("minecraft:repeater[delay={extra},facing=south,locked=false,powered=false]"))?;
    } else {
        for p in [[1, -1, 9], [1, -1, 10], [2, -1, 10]] {
            put(p[0], p[1], p[2], DUST)?;
        }
    }
    put(2, -1, 9, "minecraft:repeater[delay=4,facing=south,locked=false,powered=false]")?;
    // E: subtract, rear from the west (ARM3's dust), output east
    put(2, -1, 8, "minecraft:comparator[facing=west,mode=subtract,powered=false]")?;
    // four 1-tick repeaters east, the last powering the input cell (7,-1,8)
    for x in 3..=6 {
        put(x, -1, 8, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?;
    }
    put(7, -1, 8, "minecraft:stone")?;
    // 7b90c3f1's repeater (8,-1,8) facing west into the stone (9,-1,8) under the hold rail (9,0,8)
    put(8, -1, 8, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?;
    // (ironspec 13:56, V14) the napper arrival's block under the hold rail is slime (a conductor: the
    // repeater (8,-1,8) powers the rail through it; the fired-pulse read comes off it)
    put(9, -1, 8, "minecraft:slime_block")?;
    put(9, 0, 8, "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
    // the stop and the glass over it: slime (ironspec 15:45, as landed in 7b90c3f1's lift-split and composed
    // by onechunk; tick-exact's rail-kick row and this bench show it acts as stone for the launch and the
    // rest: a minecart is not bounced by a slime block it runs into)
    put(9, 0, 9, "minecraft:slime_block")?;
    put(9, 1, 9, "minecraft:glass")?;
    // the T (9,0,7): arms N, S, W at its placement-order shape south_west (a cart from the hold turns west)
    put(9, -1, 7, "minecraft:glass")?;
    put(9, 0, 7, "minecraft:rail[shape=south_west,waterlogged=false]")?;
    // the booster (8,0,7) on its redstone block
    put(8, -1, 7, "minecraft:redstone_block")?;
    put(8, 0, 7, "minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    // lane 7's merge T (7,0,7) north_west and lane 7's track north of it
    put(7, -1, 7, "minecraft:glass")?;
    put(7, 0, 7, "minecraft:rail[shape=north_west,waterlogged=false]")?;
    for z in 1..=6 {
        put(7, -1, z, "minecraft:glass")?;
        put(7, 0, z, "minecraft:rail[shape=north_south,waterlogged=false]")?;
    }
    // the merge row west, ending on stone
    for x in -2..=6 {
        put(x, -1, 7, "minecraft:glass")?;
        put(x, 0, 7, "minecraft:rail[shape=east_west,waterlogged=false]")?;
    }
    put(-3, 0, 7, "minecraft:stone")?;
    // the departure string over (8,0,7), its hooks on glass
    put(8, 1, 5, "minecraft:glass")?;
    put(8, 1, 9, "minecraft:glass")?;
    put(8, 1, 6, "minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    put(8, 1, 8, "minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    put(8, 1, 7, STRING_NS)?;
    // (G1 on 6ca670551 (2)) the T's north arm, the napper's approach as rev 3 lays it: the lift's exit
    // rail (7,-3,4) on glass, the ascending (8,-3,4) on its redstone block (leaning on the curve's glass),
    // the curve (9,-2,4) south_west on glass, the slopes (9,-2,5) and (9,-1,6) on their redstone blocks
    put(7, -4, 4, "minecraft:glass")?;
    put(7, -3, 4, "minecraft:rail[shape=east_west,waterlogged=false]")?;
    put(8, -4, 4, "minecraft:redstone_block")?;
    put(8, -3, 4, "minecraft:powered_rail[powered=true,shape=ascending_east,waterlogged=false]")?;
    put(9, -3, 4, "minecraft:glass")?;
    put(9, -2, 4, "minecraft:rail[shape=south_west,waterlogged=false]")?;
    put(9, -3, 5, "minecraft:redstone_block")?;
    put(9, -2, 5, "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?;
    put(9, -2, 6, "minecraft:redstone_block")?;
    put(9, -1, 6, "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?;
    Ok(farm(blocks, BlockPos::new(-4, 0, 7), BlockPos::new(-5, -5, -1), BlockPos::new(12, 4, 14)))
}

/// (ironspec 12:59, the box item) A (c′) layout in real in-chunk cells, read from a text file: one cell a
/// line `x y z state` (`... live` marks a cell of the live term, left out by the control); `bit x y z` (H2's
/// input dust), `q x y z` (the latch's state dust), `arm3 x y z` (the block the bench swaps to a redstone
/// block for 20 gt at each cycle start: the batch's ARM3; the napper's own pass over the real detector
/// rail (5,0,7) gives its second rise), `out x y z` (the fired pulse's cell, k's input). Laid over the
/// Real track with rev 3's hazards: the lanes' redstone blocks (x -1, 3, 7 at y -1, z 4..6), the merge
/// row's glass, the detector rail (5,0,7).
pub struct GateFile {
    cells: Vec<([i32; 3], String, bool)>,
    bit: [i32; 3],
    q: [i32; 3],
    arm3: [i32; 3],
    out: [i32; 3],
}

pub fn read_gate_file(path: &str) -> Result<GateFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut f = GateFile { cells: Vec::new(), bit: [0; 3], q: [0; 3], arm3: [5, -1, 7], out: [7, -1, 8] };
    let p3 = |w: &[&str]| -> Result<[i32; 3], String> {
        let n = |s: &str| s.parse::<i32>().map_err(|e| format!("{s}: {e}"));
        Ok([n(w[0])?, n(w[1])?, n(w[2])?])
    };
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() || w[0].starts_with('#') {
            continue;
        }
        match w[0] {
            "bit" | "q" | "arm3" | "out" if w.len() == 4 => {
                let p = p3(&w[1..4])?;
                match w[0] {
                    "bit" => f.bit = p,
                    "q" => f.q = p,
                    "arm3" => f.arm3 = p,
                    _ => f.out = p,
                }
            }
            _ if w.len() >= 4 => f.cells.push((p3(&w[0..3])?, w[3].to_string(), w.get(4) == Some(&"live"))),
            _ => return Err(format!("{path}: bad line {line:?}")),
        }
    }
    Ok(f)
}

fn spec_gate_file(f: &GateFile, live: bool) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut cells: std::collections::BTreeMap<[i32; 3], String> = std::collections::BTreeMap::new();
    let real = spec_real(2)?;
    let chain_or_string = |p: &BlockPos| {
        (p.y == -2 && (8..=12).contains(&p.z) && (-2..=8).contains(&p.x)) || (p.y == -1 && (8..=11).contains(&p.z) && (-1..=6).contains(&p.x)) || (p.y == 1 && p.x == 8 && (5..=9).contains(&p.z))
    };
    for (p, blk) in &real.blocks {
        if !chain_or_string(p) {
            cells.insert([p.x, p.y, p.z], blk.to_string());
        }
    }
    for x in [-1, 3, 7] {
        for z in 4..=6 {
            cells.insert([x, -1, z], "minecraft:redstone_block".into());
        }
    }
    for x in -4..=7 {
        cells.entry([x, -1, 7]).or_insert_with(|| "minecraft:glass".into());
    }
    cells.insert([5, 0, 7], "minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]".into());
    for (p, s, is_live) in &f.cells {
        if *is_live && !live {
            continue;
        }
        cells.insert(*p, s.clone());
    }
    let mut blocks = Vec::new();
    for (p, s) in cells {
        if s == "minecraft:air" {
            continue;
        }
        blocks.push((BlockPos::new(p[0], p[1], p[2]), b(&s)?));
    }
    Ok(farm(blocks, BlockPos::new(-4, 0, 7), BlockPos::new(-5, -9, 0), BlockPos::new(11, 4, 11)))
}

/// One run of a file layout (as `gate_run`, the ARM3 drive by swapping `f.arm3`).
fn gate_run_file(f: &GateFile, live: bool, cycles: i64, spawn: Option<(i64, f64, f64)>, freeze: Option<(i64, i64)>) -> Result<GateRec, String> {
    let c = cells(Geo::Real);
    let sp = crate::mirror::checked(&spec_gate_file(f, live)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let orig = run.world.get(bp(f.arm3));
    let powered = |run: &Run, p: P| run.world.get(bp(p)).to_string().contains("powered=true");
    let lit_dust = |run: &Run, p: P| {
        let s = run.world.get(bp(p)).to_string();
        s.contains("redstone_wire") && !s.contains("power=0")
    };
    let mut rec = GateRec::default();
    let mut id: Option<u32> = None;
    let (mut arm3_on, mut fired_on) = (false, false);
    let mut frozen: Option<[i32; 4]> = None;
    let mut last_pos: Option<(f64, f64, f64)> = None;
    for t in 0..(cycles * CYCLE + 40) {
        let in_cycle = t.rem_euclid(CYCLE);
        if t / CYCLE < cycles && in_cycle == 0 {
            run.world.set_block(bp(f.arm3), on.clone(), redsim::world::flags::ALL);
        }
        if in_cycle == 20 {
            run.world.set_block(bp(f.arm3), orig.clone(), redsim::world::flags::ALL);
        }
        if let Some((ts, x0, v)) = spawn
            && t == ts
        {
            id = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x0, -2.9375, crate::mirror::zf(4.5)], Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
        }
        if let Some((fr, len)) = freeze {
            if t == fr {
                frozen = Some(crate::liveness::freeze32(&mut run.world));
            }
            if t == fr + len
                && let Some(pb) = frozen.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        run.tick();
        let a = powered(&run, [5, 0, 7]) || in_cycle < 20 && t / CYCLE < cycles;
        if a && !arm3_on {
            rec.arm3.push(t);
        }
        arm3_on = a;
        let out_now = lit_dust(&run, f.out) || powered(&run, [8, -1, 8]);
        edge(&mut rec.fired, &mut fired_on, out_now, t);
        if rec.det.is_none() && powered(&run, GATE_DET) {
            rec.det = Some(t);
        }
        if rec.bit.is_none() && lit_dust(&run, f.bit) {
            rec.bit = Some(t);
        }
        if let Some(i) = id
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == i && !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
        {
            let (x, y, z) = (e.pos.x, e.pos.y, crate::mirror::zf(e.pos.z));
            let in_hold = x.floor() as i32 == c.hold[0] && z.floor() as i32 == c.hold[2] && (y - 0.0625).abs() < 0.2;
            let still = last_pos.is_some_and(|(px, _, pz)| (px - x).abs() < 0.001 && (pz - z).abs() < 0.001);
            // (tick-exact's official rest, napper-ride cd070f992: in the hold cell, horizontal speed under
            // 0.005; `still` (moved under 0.001) kept for the late placements' bookkeeping only)
            let hs = (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt();
            let _ = still;
            if in_hold && hs < 0.005 && rec.rest.is_none() {
                rec.rest = Some(t);
            }
            if in_hold && rec.entered.is_none() {
                rec.entered = Some(t);
            }
            if rec.entered.is_some() && !in_hold && rec.left.is_none() {
                rec.left = Some(t);
            }
            if rec.rest.is_some() && !in_hold && rec.off.is_none() {
                rec.off = Some(t);
                rec.off_powered = powered(&run, c.hold);
            }
            last_pos = Some((x, y, z));
        }
    }
    rec.q_end = lit_dust(&run, f.q);
    Ok(rec)
}

/// The file layout's rows (ironspec 12:59: the 300 arrivals, the freeze offsets, the ARM3 control, on
/// the real cells); `quick` runs one speed.
pub fn gate_file_bench(path: &str, freeze: i64, quick: bool) -> Result<Vec<String>, String> {
    let f = read_gate_file(path)?;
    let mut lines = Vec::new();
    let base = gate_run_file(&f, true, 4, Some((5, 7.3, 0.3)), None)?;
    let blocks = f.cells.len();
    lines.push(format!(
        "(c′) real cells from {path} ({blocks} cells, {} of the live term): detector -> H2 {} gt; one arrival: rest {:?}, bit {:?}, fired {:?}, departure {:?} (rail powered then {})",
        f.cells.iter().filter(|c| c.2).count(),
        match (base.det, base.bit) {
            (Some(d), Some(b)) => (b - d).to_string(),
            _ => "-".into(),
        },
        base.rest,
        base.bit,
        base.fired,
        base.off,
        base.off_powered
    ));
    let speeds: &[f64] = if quick { &[0.25] } else { &[0.1, 0.25, 0.4] };
    let (mut n, mut one, mut bad, mut late, mut q_left) = (0u32, 0u32, Vec::new(), 0u32, 0u32);
    let (mut mmin, mut mmax, mut dmin, mut dmax) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for &v in speeds {
        for k in 0..CYCLE {
            let r = gate_run_file(&f, true, 5, Some((110 + k, 7.3, v)), None)?;
            n += 1;
            if let (Some(rest), Some(bit)) = (r.rest, r.bit) {
                mmin = mmin.min(bit - rest);
                mmax = mmax.max(bit - rest);
            }
            let fires: Vec<i64> = r.fired.iter().filter(|&&(f0, _)| r.det.is_some_and(|d| f0 > d)).map(|x| x.0).collect();
            if fires.len() == 1 {
                one += 1;
            } else {
                bad.push(format!("v {v} spawn {}: {} fired", 110 + k, fires.len()));
            }
            if let (Some(&f0), Some(o)) = (fires.first(), r.off) {
                dmin = dmin.min(o - (f0 - 8));
                dmax = dmax.max(o - (f0 - 8));
                late += !r.off_powered as u32;
            }
            q_left += r.q_end as u32;
        }
    }
    let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
    lines.push(format!(
        "  arrivals ({n} runs: v {:?} x every gt of a cycle; the batch's ARM3 by the swap, the napper's own from its pass over (5,0,7)): the bit at H2 after rest {}..{} gt (bar >= 2); exactly one fired pulse {one}{}; the departure at E's rise + {}..{}, late {late}; the latch left set {q_left}",
        speeds,
        m(mmin, i64::MAX),
        m(mmax, i64::MIN),
        if bad.is_empty() { String::new() } else { format!(" (FAIL {}: {})", bad.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        m(dmin, i64::MAX),
        m(dmax, i64::MIN)
    ));
    let r0 = gate_run_file(&f, true, 5, Some((110, 7.3, 0.1)), None)?;
    if let (Some(d1), Some(rs1), Some(&(f0, _))) = (r0.det, r0.rest, r0.fired.iter().find(|&&(x, _)| r0.det.is_some_and(|d| x > d))) {
        let (mut ok, mut nbad, mut least) = (0u32, 0u32, i64::MAX);
        for t in (d1 - 3..=rs1).chain(f0 - 5..=f0 + 16) {
            let r = gate_run_file(&f, true, 5, Some((110, 7.3, 0.1)), Some((t, freeze)))?;
            let fires = r.fired.iter().filter(|&&(x, _)| r.det.is_some_and(|d| x > d)).count();
            if let (Some(rest), Some(bit)) = (r.rest, r.bit) {
                least = least.min(bit - rest);
            }
            if fires == 1 && r.off.is_some() && r.off_powered {
                ok += 1;
            } else {
                nbad += 1;
            }
        }
        lines.push(format!("  a {freeze}-gt freeze at every offset of the approach (detector - 3 .. rest) and around the fired pulse (- 5 .. + 16): launched on one fired pulse {ok}, otherwise {nbad}; the bit after rest at least {} gt", m(least, i64::MAX)));
    }
    let ctl = gate_run_file(&f, false, 5, Some((110, 7.3, 0.3)), None)?;
    let lv = gate_run_file(&f, true, 5, Some((110, 7.3, 0.3)), None)?;
    let nf = |r: &GateRec| r.fired.iter().filter(|&&(x, _)| r.det.is_some_and(|d| x > d)).count();
    lines.push(format!(
        "  the napper's own second ARM3 rise (its pass over the detector rail (5,0,7)): with the live term {} fired pulse(s) ({}); the control without it {} ({})",
        nf(&lv),
        if nf(&lv) == 1 { "pass" } else { "FAIL" },
        nf(&ctl),
        if nf(&ctl) > 1 { "the phantom: the control fails the row" } else { "NO phantom" }
    ));
    Ok(lines)
}

/// (ironspec 15:23 (2)) The napper's ride from R to rest with the lift laid, every tick: t (ticks after the
/// summon, as tick-exact's napper-ride counts), the cart's position and motion, the approach detector's
/// state; the rise, the T entry (its cell becoming the T's) and the rest (the hold cell, horizontal speed
/// under 0.005). The summon as tick-exact's: (4.5, -18.9375 - depth, 4.5), Motion (v, 0, 0), a villager
/// riding.
pub fn lift_trace(k: i64, depth: i32, v: f64, ticks: i64) -> Result<Vec<String>, String> {
    GATE_LIFT.with(|c| c.set(depth));
    let res = (|| -> Result<Vec<String>, String> {
        let c = cells(Geo::Real);
        let sp = crate::mirror::checked(&spec_gate(2, true, k)?)?;
        let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
        let mut run = Run::start(&sp, &ro)?;
        while run.settle_tick() {}
        let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [4.5, -18.9375 - depth as f64, 4.5], Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?;
        let mut lines = vec![format!("the napper's ride, every tick (redsim; the summon at (4.5, {:.4}, 4.5) Motion ({v}, 0, 0), the lift {depth} down; t = ticks after the summon, as tick-exact's napper-ride)", -18.9375 - depth as f64)];
        let (mut rise, mut t_entry, mut rest) = (None::<i64>, None::<(i64, [f64; 3], f64)>, None::<(i64, [f64; 3])>);
        for t in 1..=ticks {
            run.tick();
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed) else {
                lines.push(format!("t{t}: no cart"));
                break;
            };
            let p = [e.pos.x, e.pos.y, e.pos.z];
            let m = [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z];
            let hs = (m[0] * m[0] + m[2] * m[2]).sqrt();
            let det = run.world.get(crate::mirror::at(GATE_DET)).to_string().contains("powered=true");
            let cell = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
            if rise.is_none() && det {
                rise = Some(t);
            }
            if t_entry.is_none() && cell == [9, 0, 7] {
                t_entry = Some((t, p, hs));
            }
            if rest.is_none() && cell == c.hold && hs < 0.005 {
                rest = Some((t, p));
            }
            lines.push(format!("t{t}: ({:.4}, {:.4}, {:.4}) motion ({:+.4}, {:+.4}, {:+.4}) horizontal {hs:.4}{}", p[0], p[1], p[2], m[0], m[1], m[2], if det { " DETECTOR" } else { "" }));
            if rest.is_some_and(|(r, _)| t > r + 2) {
                break;
            }
        }
        lines.push(format!(
            "summary: the rise at {}; the T entry at {}; rest at {}; detector -> rest {}",
            rise.map(|t| format!("t{t}")).unwrap_or_else(|| "-".into()),
            t_entry.map(|(t, p, s)| format!("t{t} ({:.4}, {:.4}, {:.4}) {s:.4}", p[0], p[1], p[2])).unwrap_or_else(|| "-".into()),
            rest.map(|(t, p)| format!("t{t} ({:.4}, {:.4}, {:.4})", p[0], p[1], p[2])).unwrap_or_else(|| "-".into()),
            match (rise, rest) {
                (Some(a), Some((b, _))) => format!("{} gt", b - a),
                _ => "-".into(),
            }
        ));
        Ok(lines)
    })();
    GATE_LIFT.with(|c| c.set(0));
    res
}

fn spec(g: Geo, extra: i64) -> Result<FarmSpec, String> {
    match g {
        Geo::Line => spec_line(extra),
        Geo::Real => spec_real(extra),
    }
}

/// (ironspec 11:43 / 11:49, (c′)) The presence-gated hold release, as blocks in the Real geometry (the
/// string dropped): the lift's exit rail (7,-3,4) becomes the approach detector; its read repeater (7,-3,3)
/// sets a loop latch (a repeater R1 into a subtract comparator C whose output dust loops back to R1; the
/// fired pulse, tapped off the hold's stone (9,-1,8), resets it through C's side); the latch's Q runs
/// through the settle delay (the chain sized for k gt from the detector to H2) into the rear of a
/// subtract comparator P whose side is NOT Q (a torch on a block the live Q powers through a repeater):
/// P = the delayed bit AND the live bit (`live` false: the torch left out, the control); H2 samples P,
/// locked by L2 from E's line at E + 2; the AND (7,-1,9) (subtract: rear E through E_d, side NOT H2)
/// fires into the input cell (7,-1,8) at ARM3 + 10 (D 12 unchanged). ARM3's drive and E (with its 12-gt
/// side) sit east of the gate (the bench's own route; the real cells are the controller box's).
pub const GATE_DRIVE: [i32; 3] = [20, -1, 13];
thread_local! {
    /// (ironspec 14:15) A restart (every chunk saved and loaded) before this tick in this order, for the
    /// next `gate_run`.
    static GATE_RESTART: std::cell::Cell<Option<(i64, crate::liveness::Restart)>> = const { std::cell::Cell::new(None) };
    /// (G1 on ed3a09628) The napper lift laid below the exit rail at this extra depth (0: none, the cart
    /// starts on the exit rail): 7b90c3f1's return-bench napper lift (R (4,-19-d,4) and its entry on glass,
    /// the soul sand under the column (6,-19-d..-4,4) with its glass walls, the top's water source (5,-3,4)
    /// pushing the cart east onto the exit rail (7,-3,4)), the napper summoned at R heading east.
    static GATE_LIFT: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    /// (ironspec 14:41) The ungated control: the AND's side (NOT H2) left out, so the pulse fires every
    /// cycle whatever the hold holds.
    static GATE_UNGATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
const GATE_ARM3: [i32; 3] = [19, -1, 13];
const GATE_BIT: [i32; 3] = [10, -1, 11];
const GATE_Q: [i32; 3] = [15, -1, 5];
const GATE_DET: [i32; 3] = [7, -3, 4];

fn spec_gate(extra: i64, live: bool, k: i64) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut cells: std::collections::BTreeMap<[i32; 3], String> = std::collections::BTreeMap::new();
    // the Real geometry's track (its cells as spec_real lays them, without the ungated chain and the string)
    let real = spec_real(extra)?;
    let chain_or_string = |p: &BlockPos| {
        (p.y == -2 && (8..=12).contains(&p.z) && (-2..=8).contains(&p.x)) || (p.y == -1 && (8..=11).contains(&p.z) && (-1..=6).contains(&p.x)) || (p.y == 1 && p.x == 8 && (5..=9).contains(&p.z))
    };
    for (p, blk) in &real.blocks {
        if !chain_or_string(p) {
            cells.insert([p.x, p.y, p.z], blk.to_string());
        }
    }
    let mut put = |x: i32, y: i32, z: i32, s: &str| {
        cells.insert([x, y, z], s.to_string());
    };
    let rep = |delay: i64, facing: &str| format!("minecraft:repeater[delay={delay},facing={facing},locked=false,powered=false]");
    let cmp = |facing: &str| format!("minecraft:comparator[facing={facing},mode=subtract,powered=false]");
    // the floors: y -2 under the gate (x 7..20, z 1..16), except the track's own y -2 cells and the
    // approach's clearance (the detector rail (7,-3,4), the ascending (8,-3,4), the curve and slopes: no
    // block over a rider's path; and none at x 10 beside the curve (9,-2,4) and the slope (9,-2,5): the
    // cart's box swings out to x 10.16 there (tick-exact's official ride, t79), and x 10 is outside the
    // chunk, empty in the module: a floor there pushed the cart back and cost it 4 gt to rest)
    for x in 7..=20 {
        for z in 1..=16 {
            if !((7..=10).contains(&x) && (2..=7).contains(&z)) {
                put(x, -2, z, "minecraft:stone");
            }
        }
    }
    // (G1 on ed3a09628) the napper lift below the exit rail, as 7b90c3f1's return-bench lays it
    let lift = GATE_LIFT.with(|c| c.get());
    if lift > 0 {
        let rb_y = -19 - lift;
        put(4, rb_y, 4, "minecraft:rail[shape=east_west,waterlogged=false]");
        put(4, rb_y - 1, 4, "minecraft:glass");
        put(5, rb_y, 4, "minecraft:rail[shape=east_west,waterlogged=false]");
        put(5, rb_y - 1, 4, "minecraft:glass");
        put(6, rb_y - 1, 4, "minecraft:soul_sand");
        for y in rb_y..=-4 {
            put(6, y, 4, "minecraft:water[level=0]");
            put(7, y, 4, "minecraft:glass");
            put(6, y, 3, "minecraft:glass");
            put(6, y, 5, "minecraft:glass");
            if y > rb_y {
                put(5, y, 4, "minecraft:glass");
            }
        }
        put(7, rb_y - 1, 4, "minecraft:glass");
        put(6, rb_y - 1, 3, "minecraft:glass");
        put(6, rb_y - 1, 5, "minecraft:glass");
        put(6, -3, 3, "minecraft:glass");
        put(6, -3, 5, "minecraft:glass");
        put(6, -2, 4, "minecraft:glass");
        put(5, -3, 4, "minecraft:water[level=0]");
        put(4, -3, 4, "minecraft:glass");
        put(5, -3, 3, "minecraft:glass");
        put(5, -3, 5, "minecraft:glass");
        put(5, -2, 4, "minecraft:glass");
    }
    // the approach detector and its read repeater (north), the SET line along z 1 at y -3, climbing to y -1
    put(GATE_DET[0], GATE_DET[1], GATE_DET[2], "minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]");
    put(7, -4, 3, "minecraft:glass");
    put(7, -3, 3, &rep(1, "south"));
    for (x, z) in [(7, 2), (7, 1), (8, 1), (9, 1), (10, 1), (11, 1)] {
        put(x, -4, z, "minecraft:stone");
        put(x, -3, z, DUST);
    }
    // (the climb: (11,-3,1) -> (12,-2,1) on the step (12,-3,1) -> (13,-1,1) on the step (13,-2,1))
    put(11, -2, 1, "minecraft:air");
    put(12, -3, 1, "minecraft:stone");
    put(12, -2, 1, DUST);
    put(12, -1, 1, "minecraft:air");
    put(13, -1, 1, DUST);
    put(14, -1, 1, &rep(1, "west"));
    for (x, z) in [(15, 1), (16, 1), (17, 1), (18, 1), (19, 1), (20, 1), (20, 2), (20, 3), (20, 4)] {
        put(x, -1, z, DUST);
    }
    // the loop latch: R1 (17,-1,5) into C (16,-1,5), C's output dust round to R1's input; RESET into C's
    // south side
    put(17, -1, 5, &rep(1, "east"));
    put(16, -1, 5, &cmp("east"));
    for (x, z) in [(15, 5), (14, 5), (14, 4), (14, 3), (15, 3), (16, 3), (17, 3), (18, 3), (19, 3), (19, 4), (19, 5), (18, 5)] {
        put(x, -1, z, DUST);
    }
    put(16, -1, 6, &rep(1, "south"));
    // the reset line: the hold's stone (9,-1,8) read by (10,-1,8), along z 8 to (16,-1,7)
    put(10, -1, 8, &rep(1, "west"));
    for (x, z) in [(11, 8), (12, 8), (13, 8), (14, 8), (15, 8), (16, 8), (16, 7)] {
        put(x, -1, z, DUST);
    }
    // the settle delay: the tap (19,-1,6), dust to (18,-1,10), the chain (17..13,-1,10) into P's rear: a
    // delay-a repeater, 3 delay-4, a delay-b (with the read, SET, tap and P repeaters, the detector to H2
    // in 32 + 2 (a + b) gt; ironspec 14:36: k = R_max + F + 2 at even widths, a repeater delay; k 36..48)
    let n = ((k - 32) / 2).max(2);
    let (ra, rb) = ((n - 1).clamp(1, 4), (n - (n - 1).clamp(1, 4)).clamp(1, 4));
    put(19, -1, 6, &rep(1, "north"));
    for (x, z) in [(19, 7), (19, 8), (19, 9), (19, 10), (18, 10)] {
        put(x, -1, z, DUST);
    }
    put(17, -1, 10, &rep(ra, "east"));
    for x in 14..=16 {
        put(x, -1, 10, &rep(4, "east"));
    }
    put(13, -1, 10, &rep(rb, "east"));
    // P (12,-1,10): rear the delay, side NOT Q (the live term), output west into H2's input dust
    // (12-1,10)->(11,-1,10)->(11,-1,11)->(10,-1,11). (A comparator's side reads only dust, diodes and
    // redstone blocks: a torch beside it gives nothing (its direct signal is upward only); each NOT
    // reaches its comparator's side through a repeater or a dust.)
    put(12, -1, 10, &cmp("east"));
    put(11, -1, 10, DUST);
    put(11, -1, 11, DUST);
    put(10, -1, 11, DUST);
    if live {
        // NOT Q: the live Q from (19,-1,8) down to (20,-2,8), along y -2 to (15,-2,11), up to (15,-1,12),
        // a repeater into B3 (13,-1,12), a wall torch on B3's west face (12,-1,12), a repeater (12,-1,11)
        // into P's south side
        for (x, z) in [(20, 8), (20, 9), (20, 10), (20, 11), (19, 11), (18, 11), (17, 11), (16, 11), (15, 11)] {
            put(x, -3, z, "minecraft:stone");
            put(x, -2, z, DUST);
        }
        put(20, -1, 8, "minecraft:air");
        put(15, -1, 12, DUST);
        put(14, -1, 12, &rep(1, "east"));
        put(13, -1, 12, "minecraft:stone");
        put(12, -1, 12, "minecraft:redstone_wall_torch[facing=west,lit=true]");
        put(12, -1, 11, &rep(1, "south"));
    }
    // H2 (9,-1,11) into B2 (8,-1,11); L2 (9,-1,12) locks it from E's line at (9,-1,13); NOT H2: a wall
    // torch on B2's north face (8,-1,10) into the dust (8,-1,9) on the AND's east side
    put(9, -1, 11, &rep(1, "east"));
    put(8, -1, 11, "minecraft:stone");
    put(9, -1, 12, &rep(1, "south"));
    // (ironspec 14:41, the ungated control: no NOT H2, the AND passes E_d every cycle)
    if !GATE_UNGATED.with(|c| c.get()) {
        put(8, -1, 10, "minecraft:redstone_wall_torch[facing=north,lit=true]");
    }
    put(8, -1, 9, DUST);
    // the AND (7,-1,9) into the input cell; E_d (7,-1,10) delay 2 behind a 1-tick repeater (7,-1,11) (a
    // repeater, not dust, beside B2)
    put(7, -1, 9, &cmp("south"));
    put(7, -1, 10, &rep(2, "south"));
    put(7, -1, 11, &rep(1, "south"));
    // E's line: E's output (16,-1,13), (16,-1,14), west along z 14 to (7,-1,14), north to (7,-1,12); L2's
    // branch (9,-1,13)
    for (x, z) in [(16, 13), (16, 14), (15, 14), (14, 14), (13, 14), (12, 14), (11, 14), (10, 14), (9, 14), (8, 14), (7, 14), (7, 13), (7, 12), (9, 13)] {
        put(x, -1, z, DUST);
    }
    // ARM3 (the bench's drive at (20,-1,13) through a repeater), E (17,-1,13) with its 12-gt side
    put(GATE_ARM3[0], GATE_ARM3[1], GATE_ARM3[2], &rep(1, "east"));
    put(18, -1, 13, DUST);
    put(17, -1, 13, &cmp("east"));
    for (x, z) in [(18, 14), (18, 15), (18, 16), (17, 16)] {
        put(x, -1, z, DUST);
    }
    put(17, -1, 15, &rep(extra.max(1), "south"));
    put(17, -1, 14, &rep(4, "south"));
    let mut blocks = Vec::new();
    for (p, s) in cells {
        if s == "minecraft:air" {
            continue;
        }
        blocks.push((BlockPos::new(p[0], p[1], p[2]), b(&s)?));
    }
    Ok(farm(blocks, BlockPos::new(-4, 0, 7), BlockPos::new(-5, if GATE_LIFT.with(|c| c.get()) > 0 { -23 - GATE_LIFT.with(|c| c.get()) } else { -5 }, 0), BlockPos::new(22, 4, 18)))
}

/// One (c′) run's record.
#[derive(Default)]
struct GateRec {
    /// ARM3's rises (the drive's repeater).
    arm3: Vec<i64>,
    /// the fired pulses at the input cell (rise, first unpowered tick).
    fired: Vec<(i64, i64)>,
    /// the approach detector's first rise; the cart's first tick at rest on the hold (its centre in the
    /// hold cell and moving < 0.001 a tick); the bit's first rise at H2's input.
    det: Option<i64>,
    rest: Option<i64>,
    bit: Option<i64>,
    /// the departure (its centre off the hold cell after resting) and the hold rail's state then.
    off: Option<i64>,
    off_powered: bool,
    /// the latch's Q at the end of the run.
    q_end: bool,
    /// (ironspec 14:41, the ungated control) the cart's first tick in the hold cell and its first tick out
    /// of it after (rest or not).
    entered: Option<i64>,
    left: Option<i64>,
}

/// One (c′) run: the drive at every cycle start (phase 0) for 20 gt over `cycles` cycles; a ridden cart
/// spawned on the exit rail (7,-3,4) at `x0` moving east at `v` at `t_spawn` (None: no cart); the part
/// frozen over `freeze`; with `napper_arm3` the departing cart's contact with ARM3's cell (5,0,7) drives
/// ARM3 for 20 gt (the second rise).
#[allow(clippy::too_many_arguments)]
fn gate_run(extra: i64, live: bool, k: i64, cycles: i64, spawn: Option<(i64, f64, f64)>, freeze: Option<(i64, i64)>, napper_arm3: bool) -> Result<GateRec, String> {
    let c = cells(Geo::Real);
    let sp = crate::mirror::checked(&spec_gate(extra, live, k)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let powered = |run: &Run, p: P| run.world.get(bp(p)).to_string().contains("powered=true");
    let lit_dust = |run: &Run, p: P| {
        let s = run.world.get(bp(p)).to_string();
        s.contains("redstone_wire") && !s.contains("power=0")
    };
    let mut rec = GateRec::default();
    let mut id: Option<u32> = None;
    let (mut arm3_on, mut fired_on) = (false, false);
    let mut frozen: Option<[i32; 4]> = None;
    let mut contact: Option<i64> = None;
    let mut last_pos: Option<(f64, f64, f64)> = None;
    for t in 0..(cycles * CYCLE + 40) {
        if let Some((rt, how)) = GATE_RESTART.with(|c| c.get())
            && rt == t
        {
            let remap = crate::liveness::restart(&mut run.world, how)?;
            if let Some(i) = id {
                id = Some(remap.get(&i).copied().unwrap_or(i));
            }
        }
        let in_cycle = t.rem_euclid(CYCLE);
        if t / CYCLE < cycles && in_cycle == 0 {
            run.world.set_block(bp(GATE_DRIVE), on.clone(), redsim::world::flags::ALL);
        }
        if in_cycle == 20 && contact.is_none_or(|tc| t >= tc + 20) {
            run.world.set_block(bp(GATE_DRIVE), air.clone(), redsim::world::flags::ALL);
        }
        if let Some((ts, x0, v)) = spawn
            && t == ts
        {
            let lift = GATE_LIFT.with(|c| c.get());
            // (with the lift: the napper summoned at R heading east at v, as the return-bench and
            // tick-exact's napper-ride do; else on the exit rail at x0)
            let at = if lift > 0 { [4.5, -18.9375 - lift as f64, crate::mirror::zf(4.5)] } else { [x0, -2.9375, crate::mirror::zf(4.5)] };
            id = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", at, Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
        }
        if let Some((f, len)) = freeze {
            if t == f {
                frozen = Some(crate::liveness::freeze32(&mut run.world));
            }
            if t == f + len
                && let Some(pb) = frozen.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        run.tick();
        let a = powered(&run, GATE_ARM3);
        if a && !arm3_on {
            rec.arm3.push(t);
        }
        arm3_on = a;
        edge(&mut rec.fired, &mut fired_on, run.world.best_neighbor_signal(bp([7, -1, 8])) > 0 || powered(&run, [8, -1, 8]), t);
        if rec.det.is_none() && powered(&run, GATE_DET) {
            rec.det = Some(t);
        }
        if rec.bit.is_none() && lit_dust(&run, GATE_BIT) {
            rec.bit = Some(t);
        }
        if let Some(i) = id
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == i && !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
        {
            let (x, y, z) = (e.pos.x, e.pos.y, crate::mirror::zf(e.pos.z));
            let in_hold = x.floor() as i32 == c.hold[0] && z.floor() as i32 == c.hold[2] && (y - 0.0625).abs() < 0.2;
            let still = last_pos.is_some_and(|(px, _, pz)| (px - x).abs() < 0.001 && (pz - z).abs() < 0.001);
            // (tick-exact's official rest, napper-ride cd070f992: in the hold cell, horizontal speed under
            // 0.005; `still` (moved under 0.001) kept for the late placements' bookkeeping only)
            let hs = (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt();
            let _ = still;
            if in_hold && hs < 0.005 && rec.rest.is_none() {
                rec.rest = Some(t);
            }
            if in_hold && rec.entered.is_none() {
                rec.entered = Some(t);
            }
            if rec.entered.is_some() && !in_hold && rec.left.is_none() {
                rec.left = Some(t);
            }
            if rec.rest.is_some() && !in_hold && rec.off.is_none() {
                rec.off = Some(t);
                rec.off_powered = powered(&run, c.hold);
            }
            if napper_arm3 && rec.off.is_some() && contact.is_none() && x - 0.49 < 6.0 && (7.0..8.0).contains(&z) {
                contact = Some(t);
                run.world.set_block(bp(GATE_DRIVE), on.clone(), redsim::world::flags::ALL);
            }
            if let Some(tc) = contact
                && t == tc + 20
            {
                run.world.set_block(bp(GATE_DRIVE), air.clone(), redsim::world::flags::ALL);
            }
            last_pos = Some((x, y, z));
        }
    }
    rec.q_end = lit_dust(&run, GATE_Q);
    Ok(rec)
}

/// The (c′) bench: arrivals at every phase of the detector's contact relative to E's rise (the spawn
/// time stepped by 1 gt over a cycle) at three exit speeds; per run the bit's arrival at H2 against the
/// cart's rest, the fired pulses (exactly one per rest), the departure on the fired pulse (relative to
/// E's rise, the brief's criterion), the napper's own second ARM3 rise firing nothing; a 3-gt freeze at
/// every offset around the fired pulse; the control with the live term removed (the phantom pulse).
pub fn gate_bench(extra: i64, k: i64, freeze: i64) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    // (the pulse and the delay, from a run with one arrival)
    let base = gate_run(extra, true, k, 4, Some((5, 7.3, 0.3)), None, true)?;
    let delay = match (base.det, base.bit) {
        (Some(d), Some(b)) => (b - d).to_string(),
        _ => "-".into(),
    };
    lines.push(format!(
        "(c′) the presence-gated hold release as blocks (ironspec 11:43 / 11:49: the approach detector (7,-3,4) -> a loop latch -> the settle delay -> P = delayed AND live -> H2 sampled at E + 2 (L2) -> the AND (7,-1,9) into the input cell; the fired pulse resets the latch) in the Real geometry with the T's north arm: one arrival's detector-to-H2 delay {delay} gt (the settle lock k {k}: ironspec 14:36, k = R_max + F + 2 at even widths, a repeater delay); every run's spec checked: 0 cells outside its world's box"
    ));
    lines.push(tsnap_line()?);
    let (mut n, mut margin_min, mut margin_max) = (0u32, i64::MAX, i64::MIN);
    let (mut one_fire, mut extra_fire, mut no_fire, mut late, mut q_left) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut dep_e = (i64::MAX, i64::MIN);
    let mut bad = Vec::new();
    for v in [0.1, 0.25, 0.4] {
        for k in 0..CYCLE {
            let ts = 110 + k;
            let r = gate_run(extra, true, k, 5, Some((ts, 7.3, v)), None, true)?;
            n += 1;
            if let (Some(rest), Some(bit)) = (r.rest, r.bit) {
                margin_min = margin_min.min(bit - rest);
                margin_max = margin_max.max(bit - rest);
            }
            let fires_after = r.fired.iter().filter(|&&(f, _)| r.det.is_some_and(|d| f > d)).count();
            match fires_after {
                1 => one_fire += 1,
                0 => {
                    no_fire += 1;
                    bad.push(format!("v {v} spawn {ts}: no fired pulse"));
                }
                _ => {
                    extra_fire += 1;
                    bad.push(format!("v {v} spawn {ts}: {fires_after} fired pulses"));
                }
            }
            if let Some(&(f, _)) = r.fired.iter().find(|&&(f, _)| r.det.is_some_and(|d| f > d)) {
                // (E's rise = the fired pulse's rise - 8: the input cell at E + 8)
                let e_rise = f - 8;
                if let Some(o) = r.off {
                    dep_e = (dep_e.0.min(o - e_rise), dep_e.1.max(o - e_rise));
                    late += !r.off_powered as u32;
                }
            }
            q_left += r.q_end as u32;
        }
    }
    lines.push(format!(
        "  arrivals (a ridden cart from the exit rail at v 0.1 / 0.25 / 0.4, spawned at every gt of a cycle: {n} runs, the napper's own second ARM3 rise driven on its departure): the bit at H2 after the cart's rest by {}..{} gt (ironspec: >= 2{}); exactly one fired pulse {one_fire}, more than one {extra_fire}, none {no_fire}{}; the departure (its centre off the hold cell) at E's rise + {}..{} gt, late (the rail unpowered) {late}; the latch left set {q_left}",
        if margin_min == i64::MAX { "-".to_string() } else { margin_min.to_string() },
        if margin_max == i64::MIN { "-".to_string() } else { margin_max.to_string() },
        if margin_min != i64::MAX && margin_min < 2 { ": FAIL, the settle delay needs a delay-2 more" } else { ": pass" },
        if bad.is_empty() { String::new() } else { format!(" (FAIL: {})", bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        if dep_e.0 == i64::MAX { "-".to_string() } else { dep_e.0.to_string() },
        if dep_e.1 == i64::MIN { "-".to_string() } else { dep_e.1.to_string() }
    ));
    // a freeze at every offset around the fired pulse (a cart that rested)
    let r0 = gate_run(extra, true, k, 5, Some((110, 7.3, 0.3)), None, false)?;
    let Some(&(f0, _)) = r0.fired.iter().find(|&&(f, _)| r0.det.is_some_and(|d| f > d)) else {
        lines.push("  the freeze sweep: the reference arrival fired nothing (FAIL)".into());
        return Ok(lines);
    };
    let (mut fl, mut fok, mut fmiss) = (0u32, 0u32, 0u32);
    let mut fdep = (i64::MAX, i64::MIN);
    for off in -5..=16 {
        let r = gate_run(extra, true, k, 5, Some((110, 7.3, 0.3)), Some((f0 + off, freeze)), false)?;
        match (r.off, r.fired.iter().find(|&&(f, _)| r.det.is_some_and(|d| f > d))) {
            (Some(o), Some(&(f, _))) if r.off_powered => {
                fok += 1;
                fdep = (fdep.0.min(o - (f - 8)), fdep.1.max(o - (f - 8)));
            }
            (Some(_), Some(_)) => fl += 1,
            _ => fmiss += 1,
        }
    }
    lines.push(format!(
        "  a {freeze}-gt freeze at every offset from the fired pulse's rise - 5 to + 16 (a rested napper): launched on the fired pulse {fok}, late {fl}, missed {fmiss}; the departure at E's rise + {}..{} gt",
        if fdep.0 == i64::MAX { "-".to_string() } else { fdep.0.to_string() },
        if fdep.1 == i64::MIN { "-".to_string() } else { fdep.1.to_string() }
    ));
    // (G1's note 4 on 9c30d6dc3) a freeze during the approach (redstone running, the cart frozen: ironspec's
    // + 3 in 38): at every offset from the detector's rise - 3 to the cart's rest, at the slowest exit speed
    let r1 = gate_run(extra, true, k, 5, Some((110, 7.3, 0.1)), None, false)?;
    if let (Some(d1), Some(rs1)) = (r1.det, r1.rest) {
        let (mut ok, mut bad_n, mut least, mut most) = (0u32, Vec::new(), i64::MAX, i64::MIN);
        for off in -3..=(rs1 - d1) {
            let r = gate_run(extra, true, k, 5, Some((110, 7.3, 0.1)), Some((d1 + off, freeze)), false)?;
            let fires = r.fired.iter().filter(|&&(f, _)| r.det.is_some_and(|d| f > d)).count();
            if let (Some(rest), Some(bit)) = (r.rest, r.bit) {
                least = least.min(bit - rest);
                most = most.max(bit - rest);
            }
            if fires == 1 && r.off.is_some() && r.off_powered {
                ok += 1;
            } else {
                bad_n.push(format!("offset {off}: {fires} fired, departure {:?}, rail powered then {}", r.off, r.off_powered));
            }
        }
        lines.push(format!(
            "  a {freeze}-gt freeze during the approach (G1's note 4: redstone running, the cart frozen; v 0.1, the slowest exit) at every offset from the detector's rise - 3 to the cart's rest (+{}): launched on one fired pulse {ok}, otherwise {}{}; the bit at H2 after the cart's rest by {}..{} gt (the functional bar 0: presence sampled at rest; the design margin k - approach max - 3)",
            rs1 - d1,
            bad_n.len(),
            if bad_n.is_empty() { String::new() } else { format!(" (FAIL: {})", bad_n.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) },
            if least == i64::MAX { "-".to_string() } else { least.to_string() },
            if most == i64::MIN { "-".to_string() } else { most.to_string() }
        ));
        // (ironspec 14:15) a restart at every tick of the approach (the detector's rise - 3 .. the rest), in
        // every reload order (tick-exact's top-exit restart case: detector -> rest 36)
        let (mut rok, mut rbad, mut rleast, mut n) = (0u32, Vec::new(), i64::MAX, 0u32);
        let mut dr = (i64::MAX, i64::MIN);
        for rt in (d1 - 3)..=rs1 {
            for how in crate::liveness::Restart::rows() {
                GATE_RESTART.with(|c| c.set(Some((rt, how))));
                let r = gate_run(extra, true, k, 5, Some((110, 7.3, 0.1)), None, false);
                GATE_RESTART.with(|c| c.set(None));
                let r = r?;
                n += 1;
                let fires = r.fired.iter().filter(|&&(x, _)| r.det.is_some_and(|d| x > d)).count();
                if let (Some(rest), Some(bit), Some(d)) = (r.rest, r.bit, r.det) {
                    rleast = rleast.min(bit - rest);
                    dr = (dr.0.min(rest - d), dr.1.max(rest - d));
                }
                if fires == 1 && r.off.is_some() && r.off_powered {
                    rok += 1;
                } else {
                    rbad.push(format!("restart before t{} ({}): {fires} fired, departure {:?}", rt, how.label(), r.off));
                }
            }
        }
        lines.push(format!(
            "  a restart (every chunk saved and loaded) at every tick of the approach (detector - 3 .. rest) in every reload order ({n} runs, v 0.1): detector -> rest {}..{} gt; launched on one fired pulse {rok}, otherwise {}{}; the bit at H2 after the cart's rest at least {} gt (the functional bar 0)",
            if dr.0 == i64::MAX { "-".to_string() } else { dr.0.to_string() },
            if dr.1 == i64::MIN { "-".to_string() } else { dr.1.to_string() },
            rbad.len(),
            if rbad.is_empty() { String::new() } else { format!(" (FAIL: {})", rbad.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) },
            if rleast == i64::MAX { "-".to_string() } else { rleast.to_string() }
        ));
    }
    // the control: the live term removed (P = the delayed bit alone): the napper's second ARM3 rise
    let ctl = gate_run(extra, false, k, 5, Some((110, 7.3, 0.3)), None, true)?;
    let ctl_fires = ctl.fired.iter().filter(|&&(f, _)| ctl.det.is_some_and(|d| f > d)).count();
    let live = gate_run(extra, true, k, 5, Some((110, 7.3, 0.3)), None, true)?;
    let live_fires = live.fired.iter().filter(|&&(f, _)| live.det.is_some_and(|d| f > d)).count();
    lines.push(format!(
        "  the napper's own second ARM3 rise: with the live term {live_fires} fired pulse(s) for one rest ({}); the control without it {ctl_fires} ({})",
        if live_fires == 1 { "pass" } else { "FAIL" },
        if ctl_fires > 1 { "the phantom pulse, as ironspec predicts: the control fails the row" } else { "NO phantom: the control does not show the hole" }
    ));
    // (G1 on ed3a09628, ironspec 14:36) the lift laid: the arrivals from R, and a restart at every tick of
    // the ride, alone and with a login's freeze at the restart
    GATE_LIFT.with(|c| c.set(9));
    let lift = gate_lift_rows(extra, k);
    GATE_LIFT.with(|c| c.set(0));
    lines.extend(lift?);
    // (ironspec 14:41/14:42) the new positive control: the ungated pulse (the AND passes E_d every cycle)
    // must fail the late rows; and E rises within k of a fired pulse (a permanent bar, 0)
    GATE_UNGATED.with(|c| c.set(true));
    let ung = (|| -> Result<(u32, u32, u32, u32), String> {
        let (mut runs, mut empty, mut before, mut both) = (0u32, 0u32, 0u32, 0u32);
        for &v in &[0.1, 0.25, 0.4] {
            for ph in 0..CYCLE {
                let r = gate_run(extra, true, k, 5, Some((110 + ph, 7.3, v)), None, false)?;
                runs += 1;
                let resting_at = |f: i64| r.rest.is_some_and(|rs| rs <= f) && r.left.is_none_or(|l| l > f);
                let e = r.fired.iter().filter(|&&(f, _)| !resting_at(f)).count() as u32;
                let b = (r.left.is_some() && r.rest.is_none_or(|rs| r.left.is_some_and(|l| l <= rs))) as u32;
                empty += e;
                before += b;
                both += (e > 0 || b > 0) as u32;
            }
        }
        Ok((runs, empty, before, both))
    })();
    GATE_UNGATED.with(|c| c.set(false));
    let (runs, empty, before, both) = ung?;
    lines.push(format!(
        "  the positive control (ironspec 14:41: an ungated pulse, the AND's NOT H2 left out, must fail the late rows), over the 300 arrivals: pulses onto an empty hold {empty}; launches before rest (the cart off the hold cell without having come to rest) {before}; runs failing at least one {both} of {runs} ({})",
        if both > 0 { "the control bites" } else { "the control does NOT bite: the rows cannot see the gate" }
    ));
    let (mut within, mut least, mut fires) = (0u32, i64::MAX, 0u32);
    for &v in &[0.1, 0.25, 0.4] {
        for ph in 0..CYCLE {
            let r = gate_run(extra, true, k, 5, Some((110 + ph, 7.3, v)), None, false)?;
            for &(f, _) in &r.fired {
                fires += 1;
                for &a in &r.arm3 {
                    if a > f {
                        least = least.min(a - f);
                        if a - f <= k {
                            within += 1;
                        }
                    }
                }
            }
        }
    }
    lines.push(format!(
        "  E rises within k {k} gt after a fired pulse over the 300 arrivals ({fires} fired pulses): {within} (the permanent bar 0; the least gap from a fired pulse to the next E rise {} gt; the bench drives E once per 100-gt cycle: the release chain's own interval under restarts and logins is the controller's row)",
        if least == i64::MAX { "-".to_string() } else { least.to_string() }
    ));
    Ok(lines)
}

/// (G1 on ed3a09628; ironspec 14:36: k = R_max + F + 2) The lift-top rows with the napper lift laid (the
/// stop 9 down, `GATE_LIFT`): the napper summoned at R east at 0.2 at every gt of a cycle; then a restart
/// (every chunk saved and loaded) before every tick from the summon to the rest, in every reload order,
/// alone and with a 1..3-gt freeze starting at the restart (a login coinciding with it); detector -> rest,
/// the bit at H2 after the cart's rest, one fired pulse, the departure on the powered rail.
fn gate_lift_rows(extra: i64, k: i64) -> Result<Vec<String>, String> {
    let depth = GATE_LIFT.with(|c| c.get());
    let nf = |r: &GateRec| r.fired.iter().filter(|&&(x, _)| r.det.is_some_and(|d| x > d)).count();
    let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
    let ts = 110;
    let r0 = gate_run(extra, true, k, 5, Some((ts, 0.0, 0.2)), None, false)?;
    let (Some(d0), Some(rs0)) = (r0.det, r0.rest) else {
        return Ok(vec![format!("  the lift-top rows (the napper lift laid, the stop {depth} down): the reference ride from R did not come to rest on the hold (detector {:?}, rest {:?}): FAIL", r0.det, r0.rest)]);
    };
    let mut lines = vec![format!(
        "  (G1 on ed3a09628; ironspec 14:36) the napper lift laid (the stop {depth} down: R (4,{},4) and its entry on glass, the soul sand under the bubble column (6,{}..-4,4), the top's water source (5,-3,4); 7b90c3f1's return-bench cells), the napper summoned at R east at 0.2: detector -> rest {} gt with no restart (tick-exact's official ride: 32); the bit at H2 after rest {} gt; fired {}",
        -19 - depth,
        -19 - depth,
        rs0 - d0,
        r0.bit.map(|b| (b - rs0).to_string()).unwrap_or_else(|| "-".into()),
        nf(&r0)
    )];
    // every summon phase across a cycle
    let (mut one, mut late, mut dr, mut br) = (0u32, 0u32, (i64::MAX, i64::MIN), (i64::MAX, i64::MIN));
    for ph in 0..CYCLE {
        let r = gate_run(extra, true, k, 5, Some((ts + ph, 0.0, 0.2)), None, false)?;
        if let (Some(d), Some(rs)) = (r.det, r.rest) {
            dr = (dr.0.min(rs - d), dr.1.max(rs - d));
            if let Some(b) = r.bit {
                br = (br.0.min(b - rs), br.1.max(b - rs));
            }
        }
        one += (nf(&r) == 1) as u32;
        late += (r.off.is_some() && !r.off_powered) as u32;
    }
    lines.push(format!(
        "  the lift's arrivals at every summon phase of a cycle ({CYCLE} runs): detector -> rest {}..{} gt; the bit at H2 after rest {}..{} gt; exactly one fired pulse {one}; late {late}",
        m(dr.0, i64::MAX),
        m(dr.1, i64::MIN),
        m(br.0, i64::MAX),
        m(br.1, i64::MIN)
    ));
    // a restart at every tick of the ride, alone and with a login's 1..3-gt freeze at it
    for f in [0i64, 1, 2, 3] {
        let (mut n, mut one, mut dr, mut least, mut worst) = (0u32, 0u32, (i64::MAX, i64::MIN), i64::MAX, String::new());
        for rt in ts..=(rs0 + 1) {
            for how in crate::liveness::Restart::rows() {
                GATE_RESTART.with(|c| c.set(Some((rt, how))));
                let r = gate_run(extra, true, k, 5, Some((ts, 0.0, 0.2)), if f > 0 { Some((rt, f)) } else { None }, false);
                GATE_RESTART.with(|c| c.set(None));
                let r = r?;
                n += 1;
                if let (Some(d), Some(rs)) = (r.det, r.rest) {
                    if rs - d > dr.1 {
                        worst = format!("restart before t{rt} ({} after the summon), {}", rt - ts, how.label());
                    }
                    dr = (dr.0.min(rs - d), dr.1.max(rs - d));
                    if let Some(b) = r.bit {
                        least = least.min(b - rs);
                    }
                }
                one += (nf(&r) == 1 && r.off.is_some() && r.off_powered) as u32;
            }
        }
        lines.push(format!(
            "  a restart before every tick from the summon to the rest, every reload order{} ({n} runs): detector -> rest {}..{} gt (redsim's worst {}, at {worst}; tick-exact's official: 36 with a restart at the lift's top; rest as tick-exact's: in the hold cell under 0.005 b/gt); launched on one fired pulse {one} of {n}; the bit at H2 after rest at least {} gt (k {k} - the measured worst = {})",
            if f > 0 { format!(", with a {f}-gt login freeze starting at the restart") } else { String::new() },
            m(dr.0, i64::MAX),
            m(dr.1, i64::MIN),
            m(dr.1, i64::MIN),
            m(least, i64::MAX),
            if dr.1 == i64::MIN { "-".to_string() } else { (k - dr.1).to_string() }
        ));
    }
    // (ironspec 15:23 (1)) a restart and a later login stacking: a restart before each tick from the summon
    // to the rise (the save order) x a 1..3-gt freeze starting at every tick from the rise to the rest
    let save = crate::liveness::Restart::rows().into_iter().next().ok_or("no restart order")?;
    let (mut n, mut one, mut dr, mut least, mut worst) = (0u32, 0u32, (i64::MAX, i64::MIN), i64::MAX, String::new());
    for rt in ts..d0 {
        for f in 1..=3i64 {
            for ft in d0..=rs0 {
                GATE_RESTART.with(|c| c.set(Some((rt, save))));
                let r = gate_run(extra, true, k, 5, Some((ts, 0.0, 0.2)), Some((ft, f)), false);
                GATE_RESTART.with(|c| c.set(None));
                let r = r?;
                n += 1;
                if let (Some(d), Some(rs)) = (r.det, r.rest) {
                    if rs - d > dr.1 {
                        worst = format!("restart before t{rt} ({} after the summon), a {f}-gt freeze from t{ft}", rt - ts);
                    }
                    dr = (dr.0.min(rs - d), dr.1.max(rs - d));
                    if let Some(b) = r.bit {
                        least = least.min(b - rs);
                    }
                }
                one += (nf(&r) == 1 && r.off.is_some() && r.off_powered) as u32;
            }
        }
    }
    lines.push(format!(
        "  (ironspec 15:23 (1)) a restart before each tick from the summon to the rise (the save order) x a 1..3-gt login freeze starting at every tick from the rise to the rest ({n} runs): detector -> rest {}..{} gt (the worst at {worst}); launched on one fired pulse {one} of {n}; the bit at H2 after rest at least {} gt (bar >= 2)",
        m(dr.0, i64::MAX),
        m(dr.1, i64::MIN),
        m(least, i64::MAX)
    ));
    Ok(lines)
}

/// One run's record: ARM3's rises, the controller output's and the rail's pulses (rise, first unpowered
/// tick), the cart's departure from the hold cell, the string's rises, whether (Real) the cart reached
/// the merge row west of lane 7's T.
#[derive(Default)]
struct Rec {
    arm3: Vec<i64>,
    out: Vec<(i64, i64)>,
    rail: Vec<(i64, i64)>,
    left: Option<i64>,
    trip: Vec<i64>,
    merged: bool,
    /// (ironspec 11:05 (2)) the departing napper's contact with ARM3's cell (5,0,7) (Real, emulated).
    arm3_contact: Option<i64>,
    /// the ticks a cart's centre was on the hold cell.
    hold_busy: Vec<i64>,
    /// the departures from the hold (a cart leaving the hold cell).
    departures: u32,
    /// (G1 on 6ca670551, the brief's criterion) the first tick the cart's centre is off the hold cell
    /// after it was on it, and whether the hold rail was powered at that tick (unpowered: late).
    off_rail: Option<i64>,
    off_powered: bool,
}

fn edge(v: &mut Vec<(i64, i64)>, was: &mut bool, now: bool, t: i64) {
    if now && !*was {
        v.push((t, i64::MAX));
    }
    if !now && *was
        && let Some(last) = v.last_mut()
    {
        last.1 = t;
    }
    *was = now;
}

/// One run: the drive block placed at every cycle start (t = phase + k * 100, k < `cycles`) for 20 gt; a
/// ridden cart placed at rest on the hold rail at t = `place` (None: no cart); the part frozen at level 32
/// over `freeze` = (from, gt) (redstone running, entities frozen); with `napper_arm3` (Real) the departing
/// cart's contact with ARM3's cell (5,0,7) powers the drive for 20 gt, as the detector rail there would.
/// States are read after each tick.
fn run_once(g: Geo, extra: i64, phase: i64, cycles: i64, place: Option<i64>, freeze: Option<(i64, i64)>) -> Result<Rec, String> {
    run_full(g, extra, phase, cycles, place, freeze, false)
}

fn run_full(g: Geo, extra: i64, phase: i64, cycles: i64, place: Option<i64>, freeze: Option<(i64, i64)>, napper_arm3: bool) -> Result<Rec, String> {
    let c = cells(g);
    let sp = crate::mirror::checked(&spec(g, extra)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let powered = |run: &Run, p: P| run.world.get(bp(p)).to_string().contains("powered=true");
    let mut rec = Rec::default();
    let mut cart: Option<u32> = None;
    let (mut arm3_on, mut out_on, mut rail_on, mut trip_on) = (false, false, false, false);
    let mut frozen: Option<[i32; 4]> = None;
    let horizon = phase + cycles * CYCLE + 40;
    for t in 0..horizon {
        let in_cycle = (t - phase).rem_euclid(CYCLE);
        if t >= phase && (t - phase) / CYCLE < cycles && in_cycle == 0 {
            run.world.set_block(bp(c.drive), on.clone(), redsim::world::flags::ALL);
        }
        if t >= phase && in_cycle == 20 {
            run.world.set_block(bp(c.drive), air.clone(), redsim::world::flags::ALL);
        }
        if place == Some(t) {
            cart = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [c.place[0], c.place[1], crate::mirror::zf(c.place[2])], Some("{Motion:[0.0d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?);
        }
        if let Some((f, len)) = freeze {
            if t == f {
                frozen = Some(crate::liveness::freeze32(&mut run.world));
            }
            if t == f + len
                && let Some(pb) = frozen.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        run.tick();
        let a = powered(&run, c.arm3);
        if a && !arm3_on {
            rec.arm3.push(t);
        }
        arm3_on = a;
        edge(&mut rec.out, &mut out_on, powered(&run, c.out), t);
        edge(&mut rec.rail, &mut rail_on, powered(&run, c.hold), t);
        let tr = powered(&run, c.hook);
        if tr && !trip_on {
            rec.trip.push(t);
        }
        trip_on = tr;
        if let Some(id) = cart
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
        {
            // (the cart's position in the design's frame)
            let (x, z) = (e.pos.x, crate::mirror::zf(e.pos.z));
            let gone = match g {
                Geo::Line => x - 0.49 >= c.hold[0] as f64 + 1.0,
                Geo::Real => z + 0.49 < c.hold[2] as f64,
            };
            if gone && rec.left.is_none() {
                rec.left = Some(t);
                rec.departures += 1;
            }
            let in_hold = x.floor() as i32 == c.hold[0] && z.floor() as i32 == c.hold[2];
            if in_hold {
                rec.hold_busy.push(t);
            } else if !rec.hold_busy.is_empty() && rec.off_rail.is_none() {
                rec.off_rail = Some(t);
                rec.off_powered = rail_on;
            }
            if g == Geo::Real && x <= 4.5 && (7.0..8.0).contains(&z) {
                rec.merged = true;
            }
            // (the napper's own ARM3 rise: its box reaches the detector rail's cell (5,0,7))
            if napper_arm3 && g == Geo::Real && rec.left.is_some() && rec.arm3_contact.is_none() && x - 0.49 < 6.0 && (7.0..8.0).contains(&z) {
                rec.arm3_contact = Some(t);
                run.world.set_block(bp(c.drive), on.clone(), redsim::world::flags::ALL);
            }
        }
        if let Some(tc) = rec.arm3_contact
            && t == tc + 20
        {
            run.world.set_block(bp(c.drive), air.clone(), redsim::world::flags::ALL);
        }
    }
    Ok(rec)
}

type Hist = std::collections::BTreeMap<i64, u32>;

fn row(m: &Hist) -> String {
    m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")
}

/// The freeze sweep for one width (`extra`): a cart at rest on the hold from before the pulse, the part
/// frozen `len` gt from every offset (relative to the rail's rise) -3..width+1; per offset, by the brief's
/// criterion (G1 on 6ca670551): launched (its centre off the hold cell while the hold rail is still
/// powered), late (off the cell with the rail unpowered at that tick: a braked roll-off or the slow
/// creep) or missed (not off on this cycle). Returns (launched, late, missed, departures vs ARM3 when
/// launched, the least spare (the rail's powered ticks left after a launched departure), the string's
/// edge vs ARM3, the failing offsets' text).
#[allow(clippy::type_complexity)]
fn freeze_sweep(g: Geo, extra: i64, len: i64) -> Result<(u32, u32, u32, Hist, i64, Hist, Vec<String>), String> {
    let reference = run_once(g, extra, 0, 3, None, None)?;
    let (Some(&(rise, fall)), Some(&arm3)) = (reference.rail.get(1), reference.arm3.get(1)) else {
        return Err("the reference run saw fewer than two pulses".into());
    };
    let width = fall - rise;
    let (mut ok, mut late, mut miss) = (0u32, 0u32, 0u32);
    let mut spare = i64::MAX;
    let (mut dep, mut trip) = (Hist::new(), Hist::new());
    let mut bad = Vec::new();
    for off in -3..=(width + 1) {
        // (the cart placed 40 gt before the second pulse: at rest on the hold by its rise)
        let rec = run_once(g, extra, 0, 3, Some(rise - 40), Some((rise + off, len)))?;
        match rec.off_rail {
            Some(o) if o >= rise && o < rise + CYCLE - 20 && rec.off_powered => {
                ok += 1;
                *dep.entry(o - arm3).or_default() += 1;
                spare = spare.min(fall - 1 - o);
                if let Some(&tr) = rec.trip.iter().find(|&&x| (x - o).abs() <= 20) {
                    *trip.entry(tr - arm3).or_default() += 1;
                }
            }
            Some(o) if o >= rise && o < rise + CYCLE - 20 => {
                late += 1;
                bad.push(format!("freeze from rise + {off}: off the rail at ARM3 + {}, the rail unpowered (late)", o - arm3));
            }
            Some(o) => {
                miss += 1;
                bad.push(format!("freeze from rise + {off}: off the rail at ARM3 + {}", o - arm3));
            }
            None => {
                miss += 1;
                bad.push(format!("freeze from rise + {off}: never left"));
            }
        }
    }
    Ok((ok, late, miss, dep, spare, trip, bad))
}

/// The Line geometry's control: an unridden villager standing on the rail under the string; whether a
/// hook powers within 40 gt.
fn tripwire_control() -> Result<bool, String> {
    let sp = crate::mirror::checked(&spec_line(EXTRA)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    redsim::entity::spawn(&mut run.world, "minecraft:villager", [3.5, 1.0625, crate::mirror::zf(-1.5)], Some("{PersistenceRequired:1b}"))?;
    for _ in 0..40 {
        run.tick();
        if run.world.get(crate::mirror::at([3, 2, -1])).to_string().contains("powered=true") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The Real geometry's lane-7 check (G1 10:57 (1)): a ridden cart rolling south down lane 7 (from z 2.5 at
/// 0.35) through its merge T (7,0,7) and west along the merge row; the string over (8,0,7) must stay off.
/// Returns (the string's rises, the cart reached the merge row).
fn lane7_rider(extra: i64) -> Result<(usize, bool), String> {
    let sp = crate::mirror::checked(&spec_real(extra)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let v = if crate::mirror::on() { -0.35 } else { 0.35 };
    let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [7.5, 0.0625, crate::mirror::zf(2.5)], Some(&format!("{{Motion:[0.0d,0.0d,{v}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?;
    let (mut rises, mut was, mut merged) = (0usize, false, false);
    for _ in 0..100 {
        run.tick();
        let now = run.world.get(crate::mirror::at([8, 1, 6])).to_string().contains("powered=true");
        if now && !was {
            rises += 1;
        }
        was = now;
        if let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed) {
            let z = crate::mirror::zf(e.pos.z);
            if e.pos.x <= 4.5 && (7.0..8.0).contains(&z) {
                merged = true;
            }
        }
    }
    Ok((rises, merged))
}

/// The positive control (ironspec 10:50): tick-exact's recorded `tripwire-rider` scene (explore/tickexact
/// eec7361f5, crates/mobai/src/fixture.rs), cell for cell and unmirrored: per row x = 2, 6, 10, 14, 18 a
/// north-south rail on glass at y 64 for z 0..12, the string (x,65,6) between hooks (x,65,5) facing south
/// and (x,65,7) facing north on glass at z 4 / 8; x 2 a ridden cart at rest under it, x 6 a ridden cart
/// from z 1.5 at 0.35, x 10 an empty cart at rest, x 14 an empty cart from z 1.5 at 0.35, x 18 a villager
/// standing under it. Per row the hook's on / off ticks (t1 = after the first tick) over 60 ticks, against
/// the game's (tick-exact 10:57: x 2 and x 18 on at t1; x 6 on t18, off t28; x 10 and x 14 never).
fn rider_scene() -> Result<(Vec<String>, bool), String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for x in 0..=32 {
        for z in 0..=16 {
            blocks.push((BlockPos::new(x, 62, z), b("minecraft:stone")?));
        }
    }
    let rows = [2, 6, 10, 14, 18];
    for x in rows {
        for z in 0..=12 {
            blocks.push((BlockPos::new(x, 63, z), b("minecraft:glass")?));
            blocks.push((BlockPos::new(x, 64, z), b("minecraft:rail[shape=north_south,waterlogged=false]")?));
        }
        blocks.push((BlockPos::new(x, 65, 4), b("minecraft:glass")?));
        blocks.push((BlockPos::new(x, 65, 8), b("minecraft:glass")?));
        blocks.push((BlockPos::new(x, 65, 5), b("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?));
        blocks.push((BlockPos::new(x, 65, 7), b("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?));
        blocks.push((BlockPos::new(x, 65, 6), b(STRING_NS)?));
    }
    let sp = farm(blocks, BlockPos::new(30, 63, 15), BlockPos::new(-1, 61, -1), BlockPos::new(33, 68, 17));
    // (unmirrored: the recorded scene's own cells; refused if any lies outside the world's box)
    let off = crate::mirror::out_of_box(&sp);
    if off > 0 {
        return Err(format!("the tripwire-rider scene: {off} cell(s) outside the world's box"));
    }
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let ridden = |v: f64| format!("{{Motion:[0.0d,0.0d,{v}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}");
    let empty = |v: f64| format!("{{Motion:[0.0d,0.0d,{v}d]}}");
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", [2.5, 64.0625, 6.5], Some(&ridden(0.0)))?;
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", [6.5, 64.0625, 1.5], Some(&ridden(0.35)))?;
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", [10.5, 64.0625, 6.5], Some(&empty(0.0)))?;
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", [14.5, 64.0625, 1.5], Some(&empty(0.35)))?;
    redsim::entity::spawn(&mut run.world, "minecraft:villager", [18.5, 64.0, 6.5], Some("{PersistenceRequired:1b}"))?;
    let mut spans: Vec<Vec<(i64, i64)>> = vec![Vec::new(); rows.len()];
    let mut was = vec![false; rows.len()];
    for t in 1..=60i64 {
        run.tick();
        for (i, &x) in rows.iter().enumerate() {
            let now = run.world.get(BlockPos::new(x, 65, 5)).to_string().contains("powered=true");
            edge(&mut spans[i], &mut was[i], now, t);
        }
    }
    // the game's rows (tick-exact 10:57): (on, off) per row; i64::MAX = still on at t60
    let game: [&[(i64, i64)]; 5] = [&[(1, i64::MAX)], &[(18, 28)], &[], &[], &[(1, i64::MAX)]];
    let label = ["ridden at rest", "ridden rolling from z 1.5 at 0.35", "empty at rest", "empty rolling", "a villager standing"];
    let fmt = |v: &[(i64, i64)]| if v.is_empty() { "never".to_string() } else { v.iter().map(|&(a, b)| if b == i64::MAX { format!("on t{a}") } else { format!("on t{a}, off t{b}") }).collect::<Vec<_>>().join("; ") };
    let mut all = true;
    let mut out = Vec::new();
    for i in 0..rows.len() {
        let ok = spans[i].as_slice() == game[i];
        all &= ok;
        out.push(format!("x {} ({}): {} (game {}{})", rows[i], label[i], fmt(&spans[i]), fmt(game[i]), if ok { ", same" } else { ", DIFFERENT" }));
    }
    Ok((out, all))
}

/// (G1 on 6ca670551 (2)) An arriving napper in the Real geometry: a ridden cart on the lift's exit rail
/// (7,-3,4) at `x0` (its entry phase) moving east at `v`, up the ascending (8,-3,4), the curve (9,-2,4),
/// the slopes (9,-2,5), (9,-1,6), through the T (9,0,7) onto the hold, spawned at `t_spawn` (after a
/// pulse), the cycle drive at phase 0. Returns (the string's rises before the next pulse's rise at
/// `next_rise`, its centre on the hold cell the tick before that rise, the string's rises in the 40 gt
/// from that rise).
fn arrive_once(extra: i64, x0: f64, v: f64, t_spawn: i64, next_rise: i64) -> Result<(u32, bool, u32), String> {
    let c = cells(Geo::Real);
    let sp = crate::mirror::checked(&spec_real(extra)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let mut id: Option<u32> = None;
    let (mut before, mut after, mut was, mut rested) = (0u32, 0u32, false, false);
    for t in 0..(next_rise + 40) {
        let in_cycle = t.rem_euclid(CYCLE);
        if t / CYCLE < 3 && in_cycle == 0 {
            run.world.set_block(bp(c.drive), on.clone(), redsim::world::flags::ALL);
        }
        if in_cycle == 20 {
            run.world.set_block(bp(c.drive), air.clone(), redsim::world::flags::ALL);
        }
        if t == t_spawn {
            id = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x0, -2.9375, crate::mirror::zf(4.5)], Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
        }
        run.tick();
        let now = run.world.get(bp(c.hook)).to_string().contains("powered=true");
        if now && !was {
            if t < next_rise {
                before += 1;
            } else {
                after += 1;
            }
        }
        was = now;
        if t == next_rise - 1
            && let Some(i) = id
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == i && !e.removed)
        {
            let (x, z) = (e.pos.x, crate::mirror::zf(e.pos.z));
            rested = x.floor() as i32 == c.hold[0] && z.floor() as i32 == c.hold[2];
        }
    }
    Ok((before, rested, after))
}

/// The bench (WIDTH from `extra`: 8 + 2 x extra gt) in geometry `g`: the pulse at every ARM3 phase 0..19
/// over 5 cycles; a ridden cart at rest placed at every `step` gt between two pulses (it leaves at the
/// next pulse), with the string's edge and presses while resting; the freeze sweep at this width; the
/// launch threshold under the freeze for widths 8, 10, 12, 14; the controls.
pub fn bench(step: i64, extra: i64, freeze: i64, g: Geo) -> Result<Vec<String>, String> {
    let (mut out_rise, mut out_w, mut rail_rise, mut rail_w) = (Hist::new(), Hist::new(), Hist::new(), Hist::new());
    let mut one_a_cycle = true;
    for phase in 0..20 {
        let rec = run_once(g, extra, phase, 5, None, None)?;
        one_a_cycle &= rec.arm3.len() == 5 && rec.out.len() == 5 && rec.rail.len() == 5;
        for (i, &a) in rec.arm3.iter().enumerate() {
            if let Some(&(r, f)) = rec.out.get(i) {
                *out_rise.entry(r - a).or_default() += 1;
                *out_w.entry(f - r).or_default() += 1;
            }
            if let Some(&(r, f)) = rec.rail.get(i) {
                *rail_rise.entry(r - a).or_default() += 1;
                *rail_w.entry(f - r).or_default() += 1;
            }
        }
    }
    // the late arrivals, against phase 0's measured rail pulses
    let reference = run_once(g, extra, 0, 3, None, None)?;
    let (Some(&(_, first_end)), Some(&(next_rise, _)), Some(&next_arm3)) = (reference.rail.first(), reference.rail.get(1), reference.arm3.get(1)) else {
        return Err("the reference run saw fewer than two pulses".into());
    };
    let (mut dep, mut dep_arm3, mut clear_arm3, mut trip_arm3, mut trip_dep) = (Hist::new(), Hist::new(), Hist::new(), Hist::new(), Hist::new());
    let mut bad = Vec::new();
    let (mut n, mut tripped, mut resting, mut merged, mut late) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut a = 0i64;
    while first_end + a < next_rise {
        let place = first_end + a;
        let rec = run_once(g, extra, 0, 3, Some(place), None)?;
        n += 1;
        // (G1 (1): a press while the napper rests on the hold, before the pulse launches it)
        resting += rec.trip.iter().filter(|&&x| x >= place && x < next_rise).count() as u32;
        merged += rec.merged as u32;
        // (the departure: the cart's centre off the hold cell, the brief's criterion; late when the hold
        // rail is unpowered at that tick)
        match rec.off_rail {
            Some(o) if o >= next_rise => {
                *dep.entry(o - next_rise).or_default() += 1;
                *dep_arm3.entry(o - next_arm3).or_default() += 1;
                late += !rec.off_powered as u32;
                if let Some(l) = rec.left {
                    *clear_arm3.entry(l - next_arm3).or_default() += 1;
                }
                // (the rider's box enters the string's cell before the cart's box clears the hold cell:
                // the string's edge is matched within 20 gt of the departure)
                if let Some(&tr) = rec.trip.iter().find(|&&x| (x - o).abs() <= 20) {
                    tripped += 1;
                    *trip_arm3.entry(tr - next_arm3).or_default() += 1;
                    *trip_dep.entry(tr - o).or_default() += 1;
                }
            }
            Some(o) => bad.push(format!("placed {a} gt after the pulse's end: off the rail at {o}, before the next pulse ({next_rise})")),
            None => bad.push(format!("placed {a} gt after the pulse's end: never left")),
        }
        a += step;
    }
    let width = 8 + 2 * extra;
    let geo = match g {
        Geo::Line => "the Line geometry (the hold, a straight line east under the string)",
        Geo::Real => "the Real geometry (7b90c3f1's hold (9,0,8), the slime stop (9,0,9), T (9,0,7), booster (8,0,7), lane 7's T (7,0,7); the string (8,1,7), hooks (8,1,6) / (8,1,8) on glass)",
    };
    let mut lines = vec![
        format!(
            "split9's hold release in {geo} (ARM3's edge cut to {width} gt by a subtract comparator with a delay-{} side, four 1-tick repeaters into the input cell (7,-1,8); 7b90c3f1's repeater (8,-1,8) into the stone under the hold rail; ARM3 a repeater's output, inside the tick as a detector rail's): over 20 ARM3 phases x 5 cycles the input cell is powered at ARM3 + (gt:count) {} (design {}) for (gt:count) {} (design {width}); the hold rail at ARM3 + {} (design D {D}: the rail's rise) for {}; {}; every run's spec checked: 0 cells outside its world's box",
            if extra > 0 { format!("{extra} + delay-4") } else { "4".to_string() },
            row(&out_rise),
            D - 2,
            row(&out_w),
            row(&rail_rise),
            row(&rail_w),
            if one_a_cycle { "one pulse a cycle" } else { "NOT one pulse a cycle" }
        ),
        format!(
            "  a ridden cart at rest placed {n} times between two pulses (0..{} gt after the first pulse's end, every {step}): off the hold rail (its centre off the cell: the brief's departure) at the next rail pulse's rise + (gt:count) {} = that cycle's ARM3 + {}, late (the hold rail unpowered at that tick) {late}; its box clear of the cell at ARM3 + {}; the departure string rose for {tripped} of {n} at ARM3 + {} (string - departure, gt:count: {}); presses while the napper rested {resting}{}{}",
            next_rise - first_end - 1,
            row(&dep),
            row(&dep_arm3),
            row(&clear_arm3),
            if trip_arm3.is_empty() { "-".to_string() } else { row(&trip_arm3) },
            if trip_dep.is_empty() { "-".to_string() } else { row(&trip_dep) },
            if g == Geo::Real { format!("; reached the merge row west of lane 7's T {merged} of {n}") } else { String::new() },
            if bad.is_empty() { " (every one at the next pulse)".to_string() } else { format!(" (FAIL: {})", bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
        ),
    ];
    let (ok, flate, miss, fdep, fspare, ftrip, fbad) = freeze_sweep(g, extra, freeze)?;
    lines.push(format!(
        "  a cart at rest on the hold, the part frozen {freeze} gt (level 32: redstone running, entities frozen; a login) from every offset around the pulse (rail rise - 3 .. its end + 1), by the brief's criterion (the departure = its centre off the hold cell; late = the hold rail unpowered at that tick): launched {ok}, late {flate}, missed {miss}{}; departures at ARM3 + {}; the least spare (the rail's powered ticks left after a departure) {}; the string's rising edge at ARM3 + {} (the state follows the departure that happens)",
        if fbad.is_empty() { String::new() } else { format!(" (FAIL: {})", fbad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        row(&fdep),
        if fspare == i64::MAX { "-".to_string() } else { fspare.to_string() },
        if ftrip.is_empty() { "- (never)".to_string() } else { row(&ftrip) }
    ));
    // (ironspec 10:05, G1 on 6ca670551: the launch threshold under the freeze by the brief's criterion,
    // every departure before the pulse ends; the chain makes even widths, so the exact least width is
    // read from the widest run: the latest departure after the rail's rise, + 1)
    let mut thr: Vec<String> = Vec::new();
    let mut least: Option<i64> = None;
    let mut latest_after_rise = i64::MIN;
    for x in 0..=3 {
        let (ok, late, miss, d, spare, _, _) = freeze_sweep(g, x, freeze)?;
        let w = 8 + 2 * x;
        thr.push(format!("{w} gt: {ok} launched, {late} late, {miss} missed{}", if spare == i64::MAX { String::new() } else { format!(" (least spare {spare})") }));
        if late + miss == 0 && least.is_none() {
            least = Some(w);
        }
        if x == 3 {
            // (the departures vs ARM3; the rail rises at ARM3 + D)
            latest_after_rise = d.keys().copied().max().map_or(i64::MIN, |m| m - D);
        }
    }
    lines.push(format!(
        "  the launch threshold under a {freeze}-gt freeze at every offset, by the brief's criterion: {}; the least even width with none late or missed {}; the exact least width (the latest departure in the 14-gt run at the rail's rise + {}, + 1) {} gt (the design {WIDTH})",
        thr.join("; "),
        least.map_or("none up to 14".to_string(), |w| format!("{w} gt")),
        if latest_after_rise == i64::MIN { "-".to_string() } else { latest_after_rise.to_string() },
        if latest_after_rise == i64::MIN { "-".to_string() } else { (latest_after_rise + 1).to_string() }
    ));
    match g {
        Geo::Line => lines.push(format!(
            "  the string's control (an unridden villager standing on the rail under it): a hook powered {}",
            if tripwire_control()? { "yes" } else { "NO" }
        )),
        Geo::Real => {
            let (rises, merged7) = lane7_rider(extra)?;
            lines.push(format!(
                "  a lane-7 rider through its merge T (7,0,7) and west (G1: its box stays at x <= 7.8): the string rose {rises} times ({}); the rider reached the merge row {}",
                if rises == 0 { "pass" } else { "FAIL" },
                if merged7 { "yes" } else { "NO" }
            ));
            // (ironspec 11:05 (2): ARM3's only consumer is E; the napper's own rise gives a second pulse on
            // an empty hold)
            let rec = run_full(g, extra, 0, 3, Some(first_end + 10), None, true)?;
            let dep = rec.left.unwrap_or(i64::MAX);
            let after: Vec<(i64, i64)> = rec.rail.iter().copied().filter(|&(r, _)| r > dep && r < next_arm3 + CYCLE).collect();
            let busy = after.iter().any(|&(r, f)| rec.hold_busy.iter().any(|&b| b >= r && b < f));
            lines.push(format!(
                "  the napper's own ARM3 rise (its box reaching ARM3's cell (5,0,7) after it leaves; the detector rail emulated for 20 gt): contact at ARM3 + {}; the rail's pulses after its departure, before the next batch's ARM3 (+ {CYCLE}): {}; a cart on the hold during them: {}; departures {} ({})",
                rec.arm3_contact.map_or("- (no contact)".to_string(), |t| (t - next_arm3).to_string()),
                if after.is_empty() { "none".to_string() } else { after.iter().map(|&(r, f)| format!("ARM3 + {} for {}", r - next_arm3, f - r)).collect::<Vec<_>>().join(", ") },
                if busy { "YES" } else { "none" },
                rec.departures,
                if rec.departures == 1 && !busy { "harmless: one extra pulse on an empty hold, no extra release" } else { "FAIL" }
            ));
            // (G1 on 6ca670551 (2): the arriving napper through the T's north arm, at every entry phase)
            let mut per_v: Vec<String> = Vec::new();
            let (mut n_all, mut pressed_all, mut rested_all, mut dep_all) = (0u32, 0u32, 0u32, 0u32);
            for v in [0.1, 0.2, 0.3, 0.4] {
                let (mut pressed, mut rested, mut dep_press) = (0u32, 0u32, 0u32);
                let k_n = 16;
                for k in 0..k_n {
                    let x0 = 7.10 + 0.05 * k as f64;
                    let (b, r, a) = arrive_once(extra, x0, v, first_end + 2, next_rise)?;
                    pressed += (b > 0) as u32;
                    rested += r as u32;
                    dep_press += (a > 0) as u32;
                }
                per_v.push(format!("v {v}: the string pressed before the release in {pressed} of {k_n}, at rest on the hold at the pulse {rested} of {k_n}, the departure's press {dep_press} of {k_n}"));
                n_all += k_n;
                pressed_all += pressed;
                rested_all += rested;
                dep_all += dep_press;
            }
            lines.push(format!(
                "  arriving nappers (G1: the T's north arm laid; a ridden cart from the lift's exit rail (7,-3,4) at v 0.1..0.4 east x 16 entry offsets, up the slopes through the T (9,0,7) onto the hold, between two pulses): {}; in all, the string pressed before the release in {pressed_all} of {n_all} (bar 0: {}), at rest on the hold at the pulse {rested_all} of {n_all}, the departure's press {dep_all} of {n_all}",
                per_v.join("; "),
                if pressed_all == 0 { "pass" } else { "FAIL: the arrival presses the departure string" }
            ));
        }
    }
    let (scene, same) = rider_scene()?;
    lines.push(format!(
        "  the positive control, tick-exact's recorded tripwire-rider scene (explore/tickexact eec7361f5) cell for cell: {}; {}",
        scene.join("; "),
        if same { "every row as the game" } else { "DIFFERENT from the game" }
    ));
    lines.push(tsnap_line()?);
    Ok(lines)
}

/// (ironspec 12:23) The arrivals' positive control: tick-exact's recorded t-snap scene
/// (explore/tickexact ff1ca056a, crates/mobai/src/fixture.rs; t-snap.rec hash a7e7d4d7402d132f) cell
/// for cell: the T (9,64,7) south_west with its north arm (the exit rail (7,61,4), a plain ascending
/// (8,61,4), the curve (9,62,4), the powered slopes (9,62,5) and (9,63,6) on redstone blocks), the hold
/// (9,64,8) unpowered with a stone stop (9,64,9), the string (8,65,7) on hooks (8,65,6) / (8,65,8) over
/// stone; 8 copies 16 apart in x, each a cart with a seated villager on the upper slope at (z, vz).
/// The game's first presses (game frames): copies 0, 1, 3, 4, 5 at t7, t5, t4, t3, t3; copies 2 (0.285
/// into the T), 6 and 7 (0.4) never.
fn tsnap_scene() -> Result<(Vec<String>, bool), String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let starts: [(f64, f64); 8] = [(6.6, 0.05), (6.6, 0.15), (6.6, 0.35), (6.95, 0.048), (6.9, 0.148), (6.8, 0.348), (6.6, 0.6), (6.8, 0.8)];
    let game: [Option<i64>; 8] = [Some(7), Some(5), None, Some(4), Some(3), Some(3), None, None];
    let game_v = [0.174, 0.175, 0.285, 0.068, 0.105, 0.255, 0.400, 0.400];
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for x in 0..128 {
        for z in 0..16 {
            blocks.push((BlockPos::new(x, 59, z), b("minecraft:stone")?));
        }
    }
    for k in 0..starts.len() as i32 {
        let dx = 16 * k;
        let mut put = |x: i32, y: i32, z: i32, s: &str| -> Result<(), String> {
            blocks.push((BlockPos::new(x + dx, y, z), b(s)?));
            Ok(())
        };
        put(7, 60, 4, "minecraft:stone")?;
        put(7, 61, 4, "minecraft:rail[shape=east_west,waterlogged=false]")?;
        put(8, 60, 4, "minecraft:stone")?;
        put(8, 61, 4, "minecraft:rail[shape=ascending_east,waterlogged=false]")?;
        put(9, 61, 4, "minecraft:stone")?;
        put(9, 62, 4, "minecraft:rail[shape=south_west,waterlogged=false]")?;
        put(9, 61, 5, "minecraft:redstone_block")?;
        put(9, 62, 5, "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?;
        put(9, 62, 6, "minecraft:redstone_block")?;
        put(9, 63, 6, "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?;
        put(9, 63, 7, "minecraft:stone")?;
        put(9, 64, 7, "minecraft:rail[shape=south_west,waterlogged=false]")?;
        put(9, 63, 8, "minecraft:stone")?;
        put(9, 64, 8, "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
        put(9, 64, 9, "minecraft:stone")?;
        put(8, 65, 5, "minecraft:stone")?;
        put(8, 65, 9, "minecraft:stone")?;
        put(8, 65, 6, "minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
        put(8, 65, 7, STRING_NS)?;
        put(8, 65, 8, "minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    }
    let sp = farm(blocks, BlockPos::new(1, 60, 14), BlockPos::new(-1, 58, -1), BlockPos::new(129, 68, 17));
    let off = crate::mirror::out_of_box(&sp);
    if off > 0 {
        return Err(format!("the t-snap scene: {off} cell(s) outside the world's box"));
    }
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let mut ids = Vec::new();
    for (k, (z, vz)) in starts.iter().enumerate() {
        let x = 9.5 + 16.0 * k as f64;
        ids.push(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x, 63.0625 + (z - 6.0), *z], Some(&format!("{{Motion:[0.0d,0.0d,{vz}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
    }
    let mut first: Vec<Option<i64>> = vec![None; starts.len()];
    let mut v_in: Vec<Option<f64>> = vec![None; starts.len()];
    for t in 1..=60i64 {
        run.tick();
        for k in 0..starts.len() {
            let dx = 16 * k as i32;
            if first[k].is_none() && run.world.get(BlockPos::new(8 + dx, 65, 6)).to_string().contains("powered=true") {
                first[k] = Some(t);
            }
            if v_in[k].is_none()
                && let Some(e) = run.world.entities.list.iter().find(|e| e.id == ids[k] && !e.removed)
                && e.pos.z >= 7.0
            {
                v_in[k] = Some((e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt());
            }
        }
    }
    let fmt = |f: Option<i64>| f.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into());
    let mut all = true;
    let mut out = Vec::new();
    for k in 0..starts.len() {
        let ok = first[k] == game[k];
        all &= ok;
        out.push(format!(
            "copy {k} (z {}, vz {}; into the T at {} (game {:.3})): first press {} (game {}{})",
            starts[k].0,
            starts[k].1,
            v_in[k].map(|v| format!("{v:.3}")).unwrap_or_else(|| "-".into()),
            game_v[k],
            fmt(first[k]),
            fmt(game[k]),
            if ok { ", same" } else { ", DIFFERENT" }
        ));
    }
    Ok((out, all))
}

/// The t-snap control's line (the arrival rows count only after it).
fn tsnap_line() -> Result<String, String> {
    let (scene, same) = tsnap_scene()?;
    Ok(format!(
        "  the arrivals' positive control, tick-exact's recorded t-snap scene (explore/tickexact ff1ca056a: arrivals at the T from the north at <= ~0.26 swing the rider's box into (8,.,7)) cell for cell: {}; {}",
        scene.join("; "),
        if same { "every copy as the game (PASS: the arrival rows count)" } else { "DIFFERENT from the game (FAIL: the arrival rows do not count)" }
    ))
}
