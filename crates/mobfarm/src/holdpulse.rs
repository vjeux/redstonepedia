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
    Ok(farm(blocks, BlockPos::new(-4, 0, 7), BlockPos::new(-5, -3, -1), BlockPos::new(12, 4, 14)))
}

fn spec(g: Geo, extra: i64) -> Result<FarmSpec, String> {
    match g {
        Geo::Line => spec_line(extra),
        Geo::Real => spec_real(extra),
    }
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
/// over `freeze` = (from, gt) (redstone running, entities frozen). States are read after each tick.
fn run_once(g: Geo, extra: i64, phase: i64, cycles: i64, place: Option<i64>, freeze: Option<(i64, i64)>) -> Result<Rec, String> {
    let c = cells(g);
    let sp = crate::mirror::spec(&spec(g, extra)?)?;
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
            }
            if g == Geo::Real && x <= 4.5 && (7.0..8.0).contains(&z) {
                rec.merged = true;
            }
        }
    }
    Ok(rec)
}

type Hist = std::collections::BTreeMap<i64, u32>;

fn row(m: &Hist) -> String {
    m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")
}

/// The freeze sweep for one width (`extra`): a cart at rest on the hold from before the pulse, the part
/// frozen `len` gt from every offset (relative to the rail's rise) -3..width+1; per offset: launched on
/// this pulse (left by the rise + 7 to clear + `len` frozen + 2), crept (left on this cycle but later: the
/// slow drift a short powered span gives, ~+51), or missed (left on a later pulse, or never). Returns
/// (launched, crept, missed, departures vs ARM3 when launched, the string's edge vs ARM3, the failing
/// offsets' text).
fn freeze_sweep(g: Geo, extra: i64, len: i64) -> Result<(u32, u32, u32, Hist, Hist, Vec<String>), String> {
    let reference = run_once(g, extra, 0, 3, None, None)?;
    let (Some(&(rise, fall)), Some(&arm3)) = (reference.rail.get(1), reference.arm3.get(1)) else {
        return Err("the reference run saw fewer than two pulses".into());
    };
    let width = fall - rise;
    let (mut ok, mut crept, mut miss) = (0u32, 0u32, 0u32);
    let (mut dep, mut trip) = (Hist::new(), Hist::new());
    let mut bad = Vec::new();
    for off in -3..=(width + 1) {
        // (the cart placed 40 gt before the second pulse: at rest on the hold by its rise)
        let rec = run_once(g, extra, 0, 3, Some(rise - 40), Some((rise + off, len)))?;
        match rec.left {
            Some(l) if l >= rise && l <= rise + 7 + len + 2 => {
                ok += 1;
                *dep.entry(l - arm3).or_default() += 1;
                if let Some(&tr) = rec.trip.iter().find(|&&x| (x - l).abs() <= 20) {
                    *trip.entry(tr - arm3).or_default() += 1;
                }
            }
            Some(l) if l >= rise && l < rise + CYCLE - 20 => {
                crept += 1;
                bad.push(format!("freeze from rise + {off}: crept off at ARM3 + {}", l - arm3));
            }
            Some(l) => {
                miss += 1;
                bad.push(format!("freeze from rise + {off}: left at ARM3 + {}", l - arm3));
            }
            None => {
                miss += 1;
                bad.push(format!("freeze from rise + {off}: never left"));
            }
        }
    }
    Ok((ok, crept, miss, dep, trip, bad))
}

/// The Line geometry's control: an unridden villager standing on the rail under the string; whether a
/// hook powers within 40 gt.
fn tripwire_control() -> Result<bool, String> {
    let sp = crate::mirror::spec(&spec_line(EXTRA)?)?;
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
    let sp = crate::mirror::spec(&spec_real(extra)?)?;
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
    let (mut dep, mut dep_arm3, mut trip_arm3) = (Hist::new(), Hist::new(), Hist::new());
    let mut bad = Vec::new();
    let (mut n, mut tripped, mut resting, mut merged) = (0u32, 0u32, 0u32, 0u32);
    let mut a = 0i64;
    while first_end + a < next_rise {
        let place = first_end + a;
        let rec = run_once(g, extra, 0, 3, Some(place), None)?;
        n += 1;
        // (G1 (1): a press while the napper rests on the hold, before the pulse launches it)
        resting += rec.trip.iter().filter(|&&x| x >= place && x < next_rise).count() as u32;
        merged += rec.merged as u32;
        match rec.left {
            Some(l) if l >= next_rise => {
                *dep.entry(l - next_rise).or_default() += 1;
                *dep_arm3.entry(l - next_arm3).or_default() += 1;
                // (the rider's box enters the string's cell before the cart's box clears the hold cell:
                // the string's edge may precede the departure stamp, so it is matched within 20 gt)
                if let Some(&tr) = rec.trip.iter().find(|&&x| (x - l).abs() <= 20) {
                    tripped += 1;
                    *trip_arm3.entry(tr - next_arm3).or_default() += 1;
                }
            }
            Some(l) => bad.push(format!("placed {a} gt after the pulse's end: left at {l}, before the next pulse ({next_rise})")),
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
            "split9's hold release in {geo} (ARM3's edge cut to {width} gt by a subtract comparator with a delay-{} side, four 1-tick repeaters into the input cell (7,-1,8); 7b90c3f1's repeater (8,-1,8) into the stone under the hold rail; ARM3 a repeater's output, inside the tick as a detector rail's): over 20 ARM3 phases x 5 cycles the input cell is powered at ARM3 + (gt:count) {} (design {}) for (gt:count) {} (design {width}); the hold rail at ARM3 + {} (design D {D}: the rail's rise) for {}; {}",
            if extra > 0 { format!("{extra} + delay-4") } else { "4".to_string() },
            row(&out_rise),
            D - 2,
            row(&out_w),
            row(&rail_rise),
            row(&rail_w),
            if one_a_cycle { "one pulse a cycle" } else { "NOT one pulse a cycle" }
        ),
        format!(
            "  a ridden cart at rest placed {n} times between two pulses (0..{} gt after the first pulse's end, every {step}): it left the hold cell at the next rail pulse's rise + (gt:count) {} = that cycle's ARM3 + {}; the departure string rose for {tripped} of {n} at ARM3 + {} (its rising edge: the FIFO pop and the timers' key); presses while the napper rested {resting}{}{}",
            next_rise - first_end - 1,
            row(&dep),
            row(&dep_arm3),
            if trip_arm3.is_empty() { "-".to_string() } else { row(&trip_arm3) },
            if g == Geo::Real { format!("; reached the merge row west of lane 7's T {merged} of {n}") } else { String::new() },
            if bad.is_empty() { " (every one at the next pulse)".to_string() } else { format!(" (FAIL: {})", bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
        ),
    ];
    let (ok, crept, miss, fdep, ftrip, fbad) = freeze_sweep(g, extra, freeze)?;
    lines.push(format!(
        "  a cart at rest on the hold, the part frozen {freeze} gt (level 32: redstone running, entities frozen; a login) from every offset around the pulse (rail rise - 3 .. its end + 1): launched on that pulse (left by the rise + 7 + {freeze} + 2) {ok}, crept off later {crept}, missed it {miss}{}; departures at ARM3 + {}; the string's rising edge at ARM3 + {} (the state follows the departure that happens)",
        if fbad.is_empty() { String::new() } else { format!(" (FAIL: {})", fbad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        row(&fdep),
        if ftrip.is_empty() { "- (never)".to_string() } else { row(&ftrip) }
    ));
    // (ironspec 10:05: the launch threshold under the freeze, against WIDTH)
    let mut thr: Vec<String> = Vec::new();
    let mut least: Option<i64> = None;
    for x in 0..=3 {
        let (ok, crept, miss, _, _, _) = freeze_sweep(g, x, freeze)?;
        let w = 8 + 2 * x;
        thr.push(format!("{w} gt: {ok} launched, {crept} crept, {miss} missed"));
        if crept + miss == 0 && least.is_none() {
            least = Some(w);
        }
    }
    lines.push(format!(
        "  the launch threshold under a {freeze}-gt freeze at every offset: {}; the least width with every offset launched {} (the design {WIDTH})",
        thr.join("; "),
        least.map_or("none up to 14".to_string(), |w| format!("{w} gt"))
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
        }
    }
    let (scene, same) = rider_scene()?;
    lines.push(format!(
        "  the positive control, tick-exact's recorded tripwire-rider scene (explore/tickexact eec7361f5) cell for cell: {}; {}",
        scene.join("; "),
        if same { "every row as the game" } else { "DIFFERENT from the game" }
    ));
    Ok(lines)
}
