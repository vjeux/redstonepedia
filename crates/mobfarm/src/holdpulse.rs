//! split9's napper hold release (day-1, Nov 12 2026; ironspec's ruling 5 + the launch width, 06:46; the
//! hold's drive from 7b90c3f1, accepted 07:34): a pulse every cycle, 8 gt long, never a latch. ARM3 (the
//! third cart's detector rail, (5,0,7), up >= 20 gt a cycle) is cut to 8 gt by an edge comparator E
//! (subtract: ARM3's dust in its rear, ARM3 through a delay-4 repeater on its side, as the divert's E), then
//! four 1-tick repeaters, the last powering the drive's input cell (7,-1,8) from ARM3 + 10 (D 10, as
//! 7b90c3f1's bench emulates it). 7b90c3f1's own repeater (8,-1,8) facing west reads that cell and strongly
//! powers the stone (9,-1,8) under the hold rail (9,0,8) (no repeater beside the rail: its input would be a
//! face of lane 7's merge T). The bench drives ARM3 through a repeater for 20 gt every 100 gt (a repeater's
//! output, like a detector rail's, turns on inside a game tick), checks at every phase when the input cell
//! and the rail are powered after ARM3 (and for how long, one pulse a cycle), and places a cart with a
//! villager at rest on the hold rail at every gt between two measured pulses: it must leave the hold cell at
//! the next pulse (a repeater drive >= 6 gt), never before. Arrivals during a pulse come with momentum
//! (7b90c3f1's bench).

use redsim::block::Block;
use redsim::entity::EntityKind;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// The drive block (placed and removed by the bench) and ARM3's repeater (its output = ARM3).
const ARM3_DRIVE: [i32; 3] = [0, 0, 6];
const ARM3: [i32; 3] = [0, 0, 5];
/// The controller's last repeater: its output powers the input cell (0,0,-2) = (7,-1,8).
const OUT: [i32; 3] = [0, 0, -1];
/// 7b90c3f1's repeater (8,-1,8), the stone under the hold (9,-1,8), the hold rail (9,0,8).
const HOLD: [i32; 3] = [2, 1, -2];
/// The design: the input cell powered at ARM3 + D for WIDTH gt, every CYCLE.
pub const D: i64 = 10;
pub const WIDTH: i64 = 8;
pub const CYCLE: i64 = 100;

fn spec() -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for x in -3..=24 {
        for z in -5..=7 {
            blocks.push((BlockPos::new(x, -1, z), b("minecraft:stone")?));
        }
    }
    let dust = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";
    // (a diode's `facing` points at its input, as in the game)
    // ARM3: a 1-tick repeater from the drive block, into ARM3's dust (E's rear)
    blocks.push((BlockPos::new(ARM3[0], ARM3[1], ARM3[2]), b("minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?));
    blocks.push((BlockPos::new(0, 0, 4), b(dust)?));
    // ARM3's branch to the delay-4 repeater on E's west side
    for p in [[-1, 0, 4], [-2, 0, 4], [-2, 0, 3]] {
        blocks.push((BlockPos::new(p[0], p[1], p[2]), b(dust)?));
    }
    blocks.push((BlockPos::new(-1, 0, 3), b("minecraft:repeater[delay=4,facing=west,locked=false,powered=false]")?));
    // E: subtract, input from the south (ARM3's dust), output north
    blocks.push((BlockPos::new(0, 0, 3), b("minecraft:comparator[facing=south,mode=subtract,powered=false]")?));
    // four 1-tick repeaters north, the last powering the input cell (0,0,-2) = (7,-1,8): a stone block
    for z in [2, 1, 0, -1] {
        blocks.push((BlockPos::new(0, 0, z), b("minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?));
    }
    blocks.push((BlockPos::new(0, 0, -2), b("minecraft:stone")?));
    // 7b90c3f1's repeater (8,-1,8) facing west: input the cell west of it, output east into the stone
    // under the hold rail
    blocks.push((BlockPos::new(1, 0, -2), b("minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?));
    blocks.push((BlockPos::new(HOLD[0], 0, HOLD[2]), b("minecraft:stone")?));
    // the hold rail (east-west) on that stone, a backstop west of it, plain rails east on stone
    blocks.push((BlockPos::new(HOLD[0], HOLD[1], HOLD[2]), b("minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]")?));
    blocks.push((BlockPos::new(HOLD[0] - 1, HOLD[1], HOLD[2]), b("minecraft:stone")?));
    for x in (HOLD[0] + 1)..=23 {
        blocks.push((BlockPos::new(x, 0, HOLD[2]), b("minecraft:stone")?));
        blocks.push((BlockPos::new(x, 1, HOLD[2]), b("minecraft:rail[shape=east_west,waterlogged=false]")?));
    }
    Ok(FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(24, 1, 7),
        lo: BlockPos::new(-4, -2, -6),
        hi: BlockPos::new(25, 4, 8),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    })
}

/// One run's record: ARM3's rises, the controller output's and the rail's pulses (rise, first unpowered
/// tick), the cart's departure.
#[derive(Default)]
struct Rec {
    arm3: Vec<i64>,
    out: Vec<(i64, i64)>,
    rail: Vec<(i64, i64)>,
    left: Option<i64>,
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
/// ridden cart placed on the hold rail at t = `place` (None: no cart). States are read after each tick.
fn run_once(phase: i64, cycles: i64, place: Option<i64>) -> Result<Rec, String> {
    let sp = crate::mirror::spec(&spec()?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: [i32; 3]| crate::mirror::at(p);
    let powered = |run: &Run, p: [i32; 3]| run.world.get(bp(p)).to_string().contains("powered=true");
    let mut rec = Rec::default();
    let mut cart: Option<u32> = None;
    let (mut arm3_on, mut out_on, mut rail_on) = (false, false, false);
    let horizon = phase + cycles * CYCLE + 40;
    for t in 0..horizon {
        let in_cycle = (t - phase).rem_euclid(CYCLE);
        if t >= phase && (t - phase) / CYCLE < cycles && in_cycle == 0 {
            run.world.set_block(bp(ARM3_DRIVE), on.clone(), redsim::world::flags::ALL);
        }
        if t >= phase && in_cycle == 20 {
            run.world.set_block(bp(ARM3_DRIVE), air.clone(), redsim::world::flags::ALL);
        }
        if place == Some(t) {
            cart = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [HOLD[0] as f64 + 0.5, 1.0625, crate::mirror::zf(HOLD[2] as f64 + 0.5)], Some("{Motion:[0.0d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?);
        }
        run.tick();
        let a = powered(&run, ARM3);
        if a && !arm3_on {
            rec.arm3.push(t);
        }
        arm3_on = a;
        edge(&mut rec.out, &mut out_on, powered(&run, OUT), t);
        edge(&mut rec.rail, &mut rail_on, powered(&run, HOLD), t);
        if let (Some(id), None) = (cart, rec.left)
            && run.world.entities.list.iter().find(|e| e.id == id && !e.removed && matches!(e.kind, EntityKind::Minecart(_))).is_some_and(|e| e.pos.x - 0.49 >= HOLD[0] as f64 + 1.0)
        {
            rec.left = Some(t);
        }
    }
    Ok(rec)
}

/// The bench: the pulse at every ARM3 phase 0..19 over 5 cycles (the input cell's and the rail's rise after
/// ARM3's, their lengths, one a cycle); then a ridden cart at rest placed at every `step` gt between the
/// first two measured rail pulses (the first's end .. the next's rise): it must leave at the next pulse.
pub fn bench(step: i64) -> Result<Vec<String>, String> {
    type Hist = std::collections::BTreeMap<i64, u32>;
    let (mut out_rise, mut out_w, mut rail_rise, mut rail_w) = (Hist::new(), Hist::new(), Hist::new(), Hist::new());
    let mut one_a_cycle = true;
    for phase in 0..20 {
        let rec = run_once(phase, 5, None)?;
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
    let reference = run_once(0, 3, None)?;
    let (Some(&(_, first_end)), Some(&(next_rise, _)), Some(&next_arm3)) = (reference.rail.first(), reference.rail.get(1), reference.arm3.get(1)) else {
        return Err("the reference run saw fewer than two pulses".into());
    };
    let (mut dep, mut dep_arm3) = (Hist::new(), Hist::new());
    let mut bad = Vec::new();
    let mut n = 0u32;
    let mut a = 0i64;
    while first_end + a < next_rise {
        let rec = run_once(0, 3, Some(first_end + a))?;
        n += 1;
        match rec.left {
            Some(l) if l >= next_rise => {
                *dep.entry(l - next_rise).or_default() += 1;
                *dep_arm3.entry(l - next_arm3).or_default() += 1;
            }
            Some(l) => bad.push(format!("placed {a} gt after the pulse's end: left at {l}, before the next pulse ({next_rise})")),
            None => bad.push(format!("placed {a} gt after the pulse's end: never left")),
        }
        a += step;
    }
    let row = |m: &Hist| m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ");
    Ok(vec![
        format!(
            "split9's hold release (ARM3's edge cut to {WIDTH} gt by a subtract comparator with a delay-4 side, four 1-tick repeaters into the input cell (7,-1,8); 7b90c3f1's repeater (8,-1,8) into the stone under the hold rail; ARM3 a repeater's output, inside the tick as a detector rail's): over 20 ARM3 phases x 5 cycles the input cell is powered at ARM3 + (gt:count) {} (design {D}) for (gt:count) {} (design {WIDTH}); the hold rail at ARM3 + {} for {}; {}",
            row(&out_rise),
            row(&out_w),
            row(&rail_rise),
            row(&rail_w),
            if one_a_cycle { "one pulse a cycle" } else { "NOT one pulse a cycle" }
        ),
        format!(
            "  a ridden cart at rest placed {n} times between two pulses (0..{} gt after the first pulse's end, every {step}): it left the hold cell at the next rail pulse's rise + (gt:count) {} = that cycle's ARM3 + {}; touches 0 (one cart: the batch is 7b90c3f1's bench){}",
            next_rise - first_end - 1,
            row(&dep),
            row(&dep_arm3),
            if bad.is_empty() { " (every one at the next pulse)".to_string() } else { format!(" (FAIL: {})", bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
        ),
    ])
}
