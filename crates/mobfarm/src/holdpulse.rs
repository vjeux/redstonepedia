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
    put(9, -1, 8, "minecraft:stone")?;
    put(9, 0, 8, "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
    // the stop (ironspec 10:41: slime) and the glass over it
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
/// through the settle delay (4 delay-4 repeaters + one of `settle_rt` redstone ticks) into the rear of a
/// subtract comparator P whose side is NOT Q (a torch on a block the live Q powers through a repeater):
/// P = the delayed bit AND the live bit (`live` false: the torch left out, the control); H2 samples P,
/// locked by L2 from E's line at E + 2; the AND (7,-1,9) (subtract: rear E through E_d, side NOT H2)
/// fires into the input cell (7,-1,8) at ARM3 + 10 (D 12 unchanged). ARM3's drive and E (with its 12-gt
/// side) sit east of the gate (the bench's own route; the real cells are the controller box's).
pub const GATE_DRIVE: [i32; 3] = [20, -1, 13];
const GATE_ARM3: [i32; 3] = [19, -1, 13];
const GATE_BIT: [i32; 3] = [10, -1, 11];
const GATE_Q: [i32; 3] = [15, -1, 5];
const GATE_DET: [i32; 3] = [7, -3, 4];

fn spec_gate(extra: i64, live: bool, settle_rt: i64) -> Result<FarmSpec, String> {
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
    // block over a rider's path)
    for x in 7..=20 {
        for z in 1..=16 {
            if !((7..=9).contains(&x) && (2..=6).contains(&z)) {
                put(x, -2, z, "minecraft:stone");
            }
        }
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
    // delay-`settle_rt` repeater, 3 delay-4, a 1-tick (with the read, SET, tap and P repeaters, the
    // detector to H2 in 34 + 2 x settle_rt gt: 38 at settle_rt 2, ironspec 11:49)
    put(19, -1, 6, &rep(1, "north"));
    for (x, z) in [(19, 7), (19, 8), (19, 9), (19, 10), (18, 10)] {
        put(x, -1, z, DUST);
    }
    put(17, -1, 10, &rep(settle_rt.clamp(1, 4), "east"));
    for x in 14..=16 {
        put(x, -1, 10, &rep(4, "east"));
    }
    put(13, -1, 10, &rep(1, "east"));
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
    put(8, -1, 10, "minecraft:redstone_wall_torch[facing=north,lit=true]");
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
    Ok(farm(blocks, BlockPos::new(-4, 0, 7), BlockPos::new(-5, -5, 0), BlockPos::new(22, 4, 18)))
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
}

/// One (c′) run: the drive at every cycle start (phase 0) for 20 gt over `cycles` cycles; a ridden cart
/// spawned on the exit rail (7,-3,4) at `x0` moving east at `v` at `t_spawn` (None: no cart); the part
/// frozen over `freeze`; with `napper_arm3` the departing cart's contact with ARM3's cell (5,0,7) drives
/// ARM3 for 20 gt (the second rise).
#[allow(clippy::too_many_arguments)]
fn gate_run(extra: i64, live: bool, settle_rt: i64, cycles: i64, spawn: Option<(i64, f64, f64)>, freeze: Option<(i64, i64)>, napper_arm3: bool) -> Result<GateRec, String> {
    let c = cells(Geo::Real);
    let sp = crate::mirror::checked(&spec_gate(extra, live, settle_rt)?)?;
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
            id = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x0, -2.9375, crate::mirror::zf(4.5)], Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
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
            if in_hold && still && rec.rest.is_none() {
                rec.rest = Some(t);
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
pub fn gate_bench(extra: i64, settle_rt: i64, freeze: i64) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    // (the pulse and the delay, from a run with one arrival)
    let base = gate_run(extra, true, settle_rt, 4, Some((5, 7.3, 0.3)), None, true)?;
    let delay = match (base.det, base.bit) {
        (Some(d), Some(b)) => (b - d).to_string(),
        _ => "-".into(),
    };
    lines.push(format!(
        "(c′) the presence-gated hold release as blocks (ironspec 11:43 / 11:49: the approach detector (7,-3,4) -> a loop latch -> the settle delay -> P = delayed AND live -> H2 sampled at E + 2 (L2) -> the AND (7,-1,9) into the input cell; the fired pulse resets the latch) in the Real geometry with the T's north arm: one arrival's detector-to-H2 delay {delay} gt (the settle delay: a delay-{settle_rt}, 3 delay-4 and a 1-tick repeater + the read, SET, tap and P); every run's spec checked: 0 cells outside its world's box"
    ));
    lines.push(tsnap_line()?);
    let (mut n, mut margin_min, mut margin_max) = (0u32, i64::MAX, i64::MIN);
    let (mut one_fire, mut extra_fire, mut no_fire, mut late, mut q_left) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut dep_e = (i64::MAX, i64::MIN);
    let mut bad = Vec::new();
    for v in [0.1, 0.25, 0.4] {
        for k in 0..CYCLE {
            let ts = 110 + k;
            let r = gate_run(extra, true, settle_rt, 5, Some((ts, 7.3, v)), None, true)?;
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
    let r0 = gate_run(extra, true, settle_rt, 5, Some((110, 7.3, 0.3)), None, false)?;
    let Some(&(f0, _)) = r0.fired.iter().find(|&&(f, _)| r0.det.is_some_and(|d| f > d)) else {
        lines.push("  the freeze sweep: the reference arrival fired nothing (FAIL)".into());
        return Ok(lines);
    };
    let (mut fl, mut fok, mut fmiss) = (0u32, 0u32, 0u32);
    let mut fdep = (i64::MAX, i64::MIN);
    for off in -5..=16 {
        let r = gate_run(extra, true, settle_rt, 5, Some((110, 7.3, 0.3)), Some((f0 + off, freeze)), false)?;
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
    // the control: the live term removed (P = the delayed bit alone): the napper's second ARM3 rise
    let ctl = gate_run(extra, false, settle_rt, 5, Some((110, 7.3, 0.3)), None, true)?;
    let ctl_fires = ctl.fired.iter().filter(|&&(f, _)| ctl.det.is_some_and(|d| f > d)).count();
    let live = gate_run(extra, true, settle_rt, 5, Some((110, 7.3, 0.3)), None, true)?;
    let live_fires = live.fired.iter().filter(|&&(f, _)| live.det.is_some_and(|d| f > d)).count();
    lines.push(format!(
        "  the napper's own second ARM3 rise: with the live term {live_fires} fired pulse(s) for one rest ({}); the control without it {ctl_fires} ({})",
        if live_fires == 1 { "pass" } else { "FAIL" },
        if ctl_fires > 1 { "the phantom pulse, as ironspec predicts: the control fails the row" } else { "NO phantom: the control does not show the hole" }
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
        Geo::Real => "the Real geometry (7b90c3f1's hold (9,0,8), slime stop (9,0,9), T (9,0,7), booster (8,0,7), lane 7's T (7,0,7); the string (8,1,7), hooks (8,1,6) / (8,1,8) on glass)",
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
