//! The carousel's bed siding (ironfarm's hand-over, 2026-11-10 07:11; vjeux's single lane): the one
//! shared bed beside the loop's return path. A napper's cart alone leaves its batch at the stand's
//! departure and runs into the siding; the napper is dropped beside the bed, naps, is woken, re-boards
//! the same cart, and the cart waits on the exit slot until ironfarm's release pulse sends it out to
//! rejoin its batch's release run.
//!
//! Built on ironfarm's yard's visit core ([`crate::yard`]: the free cell, the activator, the slow
//! run, the park rail against a cobblestone, the bed beside the free cell, the glass cage, the
//! hatch and the dorm zombie), laid in a local frame (x east, z south): the cart arrives moving +x
//! at the entry (0,0,0); a T at (1,0,0) with its stem south (powered `south_west`: entry -> the bed
//! line; unpowered `south_east`: the bed line -> the exit branch); the bed line south (a booster,
//! the free cell, the activator, the slow run, the park rail, the cobblestone); the exit branch east
//! (a plain rail, the exit slot: a braked powered rail sloping down east and a braked flat one), the
//! exit cell (5,-1,0) where the cart leaves moving +x.
//!
//! The controls (here driven ideally by the bench, as the circuit will): idle, the T powered, the
//! hatch open (its plate across the sight line from the bed to the dorm zombie), the activator
//! powered; the napper lies down (the bed OCCUPIED): the T, the hatch and the activator turn over
//! (the hatch shut: the zombie seen, the napper wakes); the bed free again (the wake): 60 gt later
//! (the boarding cooldown) the park rail is powered for 20 gt once the exit slot is empty (the empty
//! cart kicked off the cobblestone meets the napper in the free cell under 0.2 a tick and takes it
//! north through the T onto the exit branch); a cart onto the exit branch resets the T, the hatch
//! and the activator. The release: a 10-gt pulse into the slot's support (4,-2,0).

use crate::layouts::Builder;
use anyhow::{bail, ensure, Result};
use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::ai::brain::{self, Act};
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;
use redsim::world::World;

/// The siding's cells (the local frame).
#[derive(Clone, Debug)]
pub struct Siding {
    pub entry: BlockPos,
    pub tee: BlockPos,
    pub tee_ctrl: BlockPos,
    pub boost: BlockPos,
    pub free_cell: BlockPos,
    pub activator_rail: BlockPos,
    pub activator: BlockPos,
    pub park_rail: BlockPos,
    pub park: BlockPos,
    pub stop: BlockPos,
    pub bed_head: BlockPos,
    pub bed_foot: BlockPos,
    pub hatch: BlockPos,
    pub hatch_ctrl: BlockPos,
    pub zombie: [f64; 3],
    pub slot_rails: [BlockPos; 2],
    pub release: BlockPos,
    pub exit: BlockPos,
}

/// The villagers' hold (the bench's): cart i's rail cell, north of the siding and over 16 from the
/// bed, its riders made to panic all night by the hold zombies (the loop's riders panic at night
/// everywhere but in their own nap: a rider at REST in its cart gives up the bed after 1200 gt out
/// of reach and races the napper for the bed's one ticket, or backs off 40..400 gt after each
/// failed path; and within 16 of the bed a rider at REST while another napper sleeps forgets the
/// bed for good, its ticket kept by the sleeper: `ValidateNearbyPoi`).
pub fn hold_cell(i: i32) -> BlockPos {
    BlockPos::new(-20 + 2 * i, 0, -20)
}

/// The bench's approach start (the cart teleported there moving +x).
pub const APPROACH_X: f64 = -4.5;

/// The bench's launcher (west of the approach): a cart at rest on its first booster, against the
/// glass stop, is kicked east and boosted twice more.
pub const LAUNCH_X: f64 = -8.5;

pub fn layout() -> Siding {
    let p = BlockPos::new;
    Siding {
        entry: p(0, 0, 0),
        tee: p(1, 0, 0),
        tee_ctrl: p(1, -1, 0),
        boost: p(1, 0, 1),
        free_cell: p(1, 0, 2),
        activator_rail: p(1, 0, 3),
        activator: p(1, -1, 3),
        park_rail: p(1, 0, 6),
        park: p(1, -1, 6),
        stop: p(1, 0, 7),
        bed_head: p(0, 0, 2),
        bed_foot: p(0, 0, 3),
        hatch: p(2, 1, 2),
        hatch_ctrl: p(2, 0, 2),
        zombie: [4.5, 1.0, 2.5],
        slot_rails: [p(4, -1, 0), p(5, -1, 0)],
        release: p(4, -2, 0),
        exit: p(6, -1, 0),
    }
}

fn put(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    if let Some(old) = b.d.blocks.get(&c) {
        bail!("the siding's {s} at {c:?} meets {old}");
    }
    b.set(c[0], c[1], c[2], s);
    Ok(())
}

fn at(p: BlockPos) -> [i32; 3] {
    [p.x, p.y, p.z]
}

/// The siding's blocks; `bench`: with the bench's launcher, approach and run-out (`--alone` leaves
/// them out: the siding's own blocks only, for pasting into another bench).
pub fn build(b: &mut Builder, s: &Siding, bench: bool) -> Result<()> {
    let rail = |sh: &str| format!("rail[shape={sh},waterlogged=false]");
    // the approach (the bench's) and the entry, west of it the bench's launcher: a glass stop and
    // three always-on boosters
    // (the stop a conductor, which a stationary cart on a powered rail is kicked away from; glass
    // over it, no golem floor)
    if bench {
        put(b, [-10, 0, 0], "stone")?;
        put(b, [-10, 1, 0], "glass")?;
        for x in -9..=-7 {
            put(b, [x, 0, 0], "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
            put(b, [x, -1, 0], "redstone_block")?;
        }
    }
    for x in if bench { -6 } else { 0 }..=0 {
        put(b, [x, 0, 0], &rail("east_west"))?;
        put(b, [x, -1, 0], "glass")?;
    }
    // the T, idle powered (entry -> bed line)
    put(b, at(s.tee), &rail("south_west"))?;
    put(b, at(s.tee_ctrl), "redstone_block")?;
    // the bed line: the booster, the free cell, the activator, the slow run, the park rail, the stop
    // (the rail between the T and the free cell a booster, always powered: a slow arrival leaves
    // the drop fast enough to park against the cobblestone, where the kick has the park rail's
    // whole length; the returning cart, the napper in it, gains it again on its way to the slot)
    put(b, at(s.boost), "powered_rail[powered=true,shape=north_south,waterlogged=false]")?;
    put(b, [s.boost.x, s.boost.y - 1, s.boost.z], "redstone_block")?;
    put(b, at(s.free_cell), &rail("north_south"))?;
    put(b, [s.free_cell.x, -1, s.free_cell.z], "glass")?;
    put(b, at(s.activator_rail), "activator_rail[powered=true,shape=north_south,waterlogged=false]")?;
    put(b, at(s.activator), "redstone_block")?;
    for z in s.activator_rail.z + 1..s.park_rail.z {
        put(b, [s.free_cell.x, 0, z], &rail("north_south"))?;
        put(b, [s.free_cell.x, -1, z], "glass")?;
    }
    put(b, at(s.park_rail), "powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
    put(b, at(s.park), "glass")?;
    put(b, at(s.stop), "cobblestone")?;
    put(b, [s.stop.x, s.stop.y + 1, s.stop.z], "glass")?;
    put(b, [s.stop.x, s.stop.y - 1, s.stop.z], "glass")?;
    // the exit branch: a plain rail, the slot (a braked powered rail sloping down east: the
    // returning cart stops on it, 0.2 in; the release powers it alone, so the cart leaves at the
    // slope's gravity and boost, not the cap), a plain flat, the exit and the bench's run-out
    put(b, [2, 0, 0], &rail("east_west"))?;
    put(b, [2, -1, 0], "glass")?;
    // (the branch's second rail: the circuit's detector rail, two from the T, whose power would
    // turn the T under a cart still on it)
    put(b, [3, 0, 0], &rail("east_west"))?;
    put(b, [3, -1, 0], "glass")?;
    put(b, at(s.slot_rails[0]), "powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
    put(b, at(s.release), "glass")?;
    put(b, at(s.slot_rails[1]), &rail("east_west"))?;
    put(b, [s.slot_rails[1].x, -2, 0], "glass")?;
    // (the exit cell the siding's; the run-out past it and its end stop the bench's)
    for x in s.exit.x..=if bench { 11 } else { s.exit.x } {
        put(b, [x, -1, 0], &rail("east_west"))?;
        put(b, [x, -2, 0], "glass")?;
    }
    if bench {
        put(b, [12, -1, 0], "glass")?;
    }
    // the bed west of the free cell (head) and the activator (foot), facing north, on glass
    put(b, at(s.bed_foot), "red_bed[facing=north,occupied=false,part=foot]")?;
    put(b, at(s.bed_head), "red_bed[facing=north,occupied=false,part=head]")?;
    put(b, [s.bed_foot.x, -1, s.bed_foot.z], "glass")?;
    put(b, [s.bed_head.x, -1, s.bed_head.z], "glass")?;
    // the cage (ironfarm's yard, its cells relative to the free cell F): glass two high west of the
    // bed, south of the foot, east of the line around the hatch; the bed head's north front open on
    // glass (a walking path from the line ends within reach of the head); glass over the booster
    // (the walkers' glass), over the activator and the slow run, over the bed; the east wall a third
    // block high, glass over the hatch
    let f = s.free_cell;
    let (fx, fz) = (f.x, f.z);
    let mut walls: Vec<[i32; 3]> = Vec::new();
    for dy in 0..=1 {
        walls.extend([[fx - 2, dy, fz + 1], [fx - 2, dy, fz], [fx - 1, dy, fz + 2], [fx + 1, dy, fz - 1], [fx + 1, dy, fz + 1], [fx + 1, dy, fz + 2]]);
    }
    walls.extend([[fx - 1, -1, fz - 1]]);
    walls.extend([[fx, 1, fz - 1], [fx, 1, fz + 1], [fx, 1, fz + 2], [fx, 1, fz + 3]]);
    walls.extend([[fx + 1, 2, fz - 1], [fx + 1, 2, fz + 1], [fx + 1, 2, fz], [fx + 1, 2, fz + 2]]);
    walls.extend([[fx - 1, 2, fz + 1], [fx - 1, 2, fz]]);
    for c in walls {
        put(b, c, "glass")?;
    }
    // the hatch: an iron trapdoor open (its plate across the sight line) while its control below
    // (a redstone block) powers it; shut (glass under it) the line passes over its bottom half
    put(b, at(s.hatch), "iron_trapdoor[facing=east,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(b, at(s.hatch_ctrl), "redstone_block")?;
    // the dorm zombie in a boat on glass east of the hatch (seen from the bed's head over the shut
    // hatch, and from the exit slot directly), roofed against the sun
    let zc = [s.zombie[0].floor() as i32, s.zombie[1] as i32, s.zombie[2].floor() as i32];
    put(b, [zc[0], zc[1] - 1, zc[2]], "glass")?;
    put(b, [zc[0], zc[1] + 3, zc[2]], "oak_leaves[distance=1,persistent=true,waterlogged=false]")?;
    b.entity("oak_boat", s.zombie, Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    Ok(())
}

/// The bench design: the siding, the bench's approach and run-out, the 21 villagers in carts in a
/// far calm hold (each on a braked rail between glass), every villager's HOME the siding's bed.
/// Replace a block the siding already placed (the wiring's changes to the bench-driven controls).
fn swap(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    ensure!(b.d.blocks.contains_key(&c), "the wiring swaps {c:?}, which the siding does not place");
    b.set(c[0], c[1], c[2], s);
    Ok(())
}

/// The siding's circuit (wired: the bench only places the release, ironfarm's input):
/// - S, the nap: an observer in the cage's wall south of the bed's foot, watching it (OCCUPIED,
///   copied from the head), pulses south; dust west and down to a 4-tick repeater (the latch needs
///   4 gt) into N1;
/// - H, the visit: an RS-NOR latch west of the siding at y -3 (N1, its wall torch W1 = not H; N2,
///   its wall torch W2 = H; dust between, no two links touching); set by S, reset by R;
/// - R, the return: the exit branch's rail a detector rail (only the returning cart passes it),
///   read by repeaters north, dust west along z -3 and down into N2;
/// - not H drives the T's support, the activator's and the hatch's control through torches under
///   them (each torch's block fed H by a repeater from the H line at x -1): idle, the T sends
///   arrivals down the bed line, the activator drops riders, the hatch is open; from the nap to
///   the return, the T sends the bed line to the exit branch, the activator is off (the empty cart
///   meets the napper over it), the hatch is shut (the zombie seen);
/// - L, the slot taken: a tripwire across the slot's slope at (3,0,0), hooks north and south;
/// - K, the kick: a wall torch under the park rail's support on BK, BK fed by not H through five
///   4-tick repeaters (the nap's 20 gt: the boarding cooldown past by the meeting) OR by L (the
///   slot taken: the kick waits for the release); K powers the park rail until the visit is over.
pub fn wire(b: &mut Builder, s: &Siding) -> Result<()> {
    let rep = |f: &str, d: u8| format!("repeater[delay={d},facing={f},locked=false,powered=false]");
    let dust = "redstone_wire[east=none,north=none,power=0,south=none,west=none]";
    // the control blocks: solid, each powered by a torch below (or, the T, its own torch)
    swap(b, at(s.tee_ctrl), "stone")?;
    swap(b, at(s.activator), "stone")?;
    swap(b, at(s.hatch_ctrl), "stone")?;
    swap(b, at(s.park), "stone")?;
    // a block, then the given part on top of it (dust, repeaters and torches need one)
    let on = |b: &mut Builder, c: [i32; 3], part: &str| -> Result<()> {
        put(b, [c[0], c[1] - 1, c[2]], "stone")?;
        put(b, c, part)
    };
    // the sinks: torches under the T's support, the activator's support, the hatch's control
    let torch = "redstone_torch[lit=true]";
    on(b, [1, -2, 0], torch)?; // Ta (1,-3,0) under it
    on(b, [1, -2, 3], torch)?; // Aa (1,-3,3)
    on(b, [2, -1, 2], torch)?; // Ha (2,-2,2)
    // their feeds: repeaters into Ta and Aa from the H line at x -1, into Ha from the south
    on(b, [0, -3, 0], &rep("west", 1))?;
    on(b, [0, -3, 3], &rep("west", 1))?;
    on(b, [2, -2, 3], &rep("south", 1))?;
    // the H line: x -1, z 0..5, then east along z 5 and up north into the Ha repeater's back
    // (z 5, not 4: dust beside Aa, which its repeater powers strongly, would feed the line back)
    for z in 0..=5 {
        on(b, [-1, -3, z], dust)?;
    }
    on(b, [0, -3, 5], dust)?;
    on(b, [1, -3, 5], dust)?;
    on(b, [2, -2, 5], dust)?;
    on(b, [2, -2, 4], dust)?;
    // the latch (y -3): N1, W1 (south face), D1 west and north into N2; N2, W2 (east face), D2
    // east and south into N1
    on(b, [-5, -3, 2], "stone")?; // N1 (on its own floor)
    put(b, [-5, -3, 3], "redstone_wall_torch[facing=south,lit=true]")?; // W1
    b.set(-5, -4, 3, "stone");
    for c in [[-6, -3, 3], [-7, -3, 3], [-7, -3, 2], [-7, -3, 1], [-7, -3, 0]] {
        on(b, c, dust)?; // D1
    }
    on(b, [-7, -3, -1], "stone")?; // N2
    put(b, [-6, -3, -1], "redstone_wall_torch[facing=east,lit=false]")?; // W2
    b.set(-6, -4, -1, "stone");
    for c in [[-5, -3, -1], [-5, -3, 0], [-5, -3, 1]] {
        on(b, c, dust)?; // D2
    }
    // H out: a repeater east off D2 into the H line
    on(b, [-4, -3, 0], &rep("west", 1))?;
    on(b, [-3, -3, 0], dust)?;
    on(b, [-2, -3, 0], dust)?;
    // S: the observer south of the bed's foot (replacing the cage's glass), dust west along z 5,
    // down north to the 4-tick repeater into N1
    swap(b, [0, 0, 4], "observer[facing=north,powered=false]")?;
    on(b, [0, 0, 5], dust)?;
    for x in [-1, -2, -3] {
        on(b, [x, 0, 5], dust)?;
    }
    on(b, [-3, -1, 4], dust)?;
    on(b, [-3, -2, 3], dust)?;
    on(b, [-3, -3, 2], dust)?;
    on(b, [-4, -3, 2], &rep("east", 4))?;
    // R: the exit branch's detector rail (its second rail, two from the T), repeaters north, dust
    // west along z -3, down west into a repeater south into N2
    swap(b, [3, 0, 0], "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    on(b, [3, 0, -1], &rep("south", 1))?;
    on(b, [3, 0, -2], &rep("south", 1))?;
    for x in -4..=3 {
        on(b, [x, 0, -3], dust)?;
    }
    on(b, [-5, -1, -3], dust)?;
    on(b, [-6, -2, -3], dust)?;
    on(b, [-7, -3, -3], dust)?;
    on(b, [-7, -3, -2], &rep("north", 1))?;
    // L: the tripwire across the slot's slope (hooks north on stone, south on the dorm zombie's
    // glass: a stone top there would be a golem spot, a cap would cut the bed's sight of the zombie)
    put(b, [4, 0, 0], "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?;
    put(b, [4, 0, -1], "tripwire_hook[attached=true,facing=south,powered=false]")?;
    put(b, [4, 0, -2], "stone")?;
    put(b, [4, 0, 1], "tripwire_hook[attached=true,facing=north,powered=false]")?;
    ensure!(b.d.blocks.get(&[4, 0, 2]).is_some_and(|x| x.to_string().contains("glass")), "the south hook hangs on the dorm zombie's glass at (4,0,2)");
    // K: BK east of the park's torch; the torch under the park rail's support
    on(b, [2, -2, 6], "stone")?; // BK
    put(b, [1, -2, 6], "redstone_wall_torch[facing=west,lit=false]")?;
    b.set(1, -3, 6, "stone");
    // not H, delayed: off D1 south along x -6 through five 4-tick repeaters, east along z 9,
    // up north into BK
    for z in 4..=8 {
        on(b, [-6, -3, z], &rep("north", 4))?;
    }
    for x in -6..=1 {
        on(b, [x, -3, 9], if x == -2 { "repeater[delay=1,facing=west,locked=false,powered=false]" } else { dust })?;
    }
    on(b, [2, -3, 9], dust)?;
    on(b, [2, -2, 8], dust)?;
    on(b, [2, -2, 7], dust)?;
    // L into the same line: a repeater east of the south hook reading it, dust east then south
    // along x 6, down, west along z 8 into (2,-2,8)
    on(b, [5, 0, 1], &rep("west", 1))?;
    for z in 1..=6 {
        on(b, [6, 0, z], dust)?;
    }
    on(b, [6, -1, 7], dust)?;
    on(b, [6, -2, 8], dust)?;
    for x in 3..=5 {
        on(b, [x, -2, 8], dust)?;
    }
    // glass caps on the circuit's solid tops a golem could stand on (N1, N2, BK, the hook's stone)
    for c in [[-5, -2, 2], [-7, -2, -1], [2, -1, 6], [4, 1, -2]] {
        put(b, c, "glass")?;
    }
    // the build's listed last steps (brief farm-iron-carousel, "built in order"): the two torches
    // unlit at rest, after everything else. W2, the latch's H torch: the latch is bistable, and
    // built in the default order its first source is W2 (placed unlit, its block not yet powered):
    // the first neighbour update lights it, setting H with no visit (the first arrival then goes to
    // the exit unnapped); placed last, N2 is already powered through D1 by W1, so it stays unlit, the
    // latch at rest as written. K, the kick torch: placed early (a source) it is lit by the first
    // update, and BK's power arrives later through dust written at its settled power, which tells K
    // nothing (no power change), so K stays lit and the park rail powered; placed last, BK is
    // powered already.
    b.d.meta.insert("build".into(), crate::placing::BuildPlan { last: vec![BlockPos::new(-6, -3, -1), BlockPos::new(1, -2, 6)], fills_last: false }.meta());
    Ok(())
}

/// The give-up, driven by day-1's scheduler (farmopt 2026-11-10: the parent's direction; day-1 16:34
/// agreed the cell and the signal): one input cell F (-5,-3,-4), where day-1's line ends in a
/// dust and pulses 10-12 gt at REL(i+3)+52, 150-170 gt after the visit's arrival, on every visit
/// (its register cannot see the slot or the return). The siding gates it:
/// - V, a visit under way: a waxed copper bulb (0,-1,-1) toggled by the arrival (the entry rail
///   a detector rail, its block below a stone beside the bulb) and by the return (a repeater off
///   the R line's support stone (0,-1,-3) into the bulb): lit from the arrival to the return;
/// - the gate: a comparator at (-5,-3,-3) reading F at its back, subtracting not V at its side (a
///   repeater from a torch on P (-3,-3,-2), P powered by V: dust from the bulb's comparator west and
///   down onto P), into a repeater into the H latch's D2: F sets H only during a visit. A napper
///   asleep by then has set H already (nothing changes); a cart already back has turned V off (the
///   pulse dies at the gate); a napper still awake is sent out: the T to the exit, the hatch shut,
///   the kick 20 gt later.
pub const FORCE_CELL: [i32; 3] = [-5, -3, -4];

fn giveup_sched(b: &mut Builder) -> Result<()> {
    let rep = |f: &str| format!("repeater[delay=1,facing={f},locked=false,powered=false]");
    let dust = "redstone_wire[east=none,north=none,power=0,south=none,west=none]";
    let on = |b: &mut Builder, c: [i32; 3], part: &str| -> Result<()> {
        put(b, [c[0], c[1] - 1, c[2]], "stone")?;
        put(b, c, part)
    };
    // V: the arrival's detector rail and the stone under it, the bulb beside that stone, the
    // return's repeater into the bulb from the north
    swap(b, [0, 0, 0], "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    swap(b, [0, -1, 0], "stone")?;
    put(b, [0, -1, -1], "waxed_copper_bulb[lit=false,powered=false]")?;
    on(b, [0, -1, -2], &rep("north"))?;
    // V out: a comparator west of the bulb, dust west, down north and west onto P (-3,-3,-2)
    on(b, [-1, -1, -1], "comparator[facing=east,mode=compare,powered=false]")?;
    on(b, [-2, -1, -1], dust)?;
    on(b, [-2, -2, -2], dust)?;
    put(b, [-3, -3, -2], "stone")?; // P
    put(b, [-3, -2, -2], dust)?;
    // not V: a torch on P's north face; a repeater west of it into the gate's side
    put(b, [-3, -3, -3], "redstone_wall_torch[facing=north,lit=true]")?;
    on(b, [-4, -3, -3], &rep("east"))?;
    // the gate: F at its back (day-1's dust at FORCE_CELL), out south through a repeater into D2
    on(b, [-5, -3, -3], "comparator[facing=north,mode=subtract,powered=false]")?;
    on(b, [-5, -3, -2], &rep("north"))?;
    // glass caps on its solid tops a golem could stand on (the bulb)
    put(b, [0, 0, -1], "glass")?;
    Ok(())
}

pub fn design() -> Result<String> {
    design_with(false)
}

/// The give-up (farmopt 2026-11-10, ironspec's rule-1 missing piece: a cart whose napper cannot
/// sleep still leaves), north of the R line, all of it at z <= -1:
/// - ARR, the arrival: the entry rail a detector rail (only arrivals pass it), its block below a
///   stone; a repeater north reads it, into a stone, dust under it, repeaters north at y -2 under
///   the R line into the A latch's set;
/// - A, a visit under way: an RS-NOR latch at y -2 (the siding latch's own shape, turned: N_A1 set
///   from the south, its torch W_A1 = not A on its west face; N_A2, its torch W_A2 = A), set by
///   ARR, reset by R (a repeater off the R line's corner, dust north along x 4 at y 0 and down
///   into a repeater into N_A2's north face) or by G (a repeater into its east face);
/// - the bound: day-1's two-stage comparator off-delay (`mctas circuit timer long --k 3`, L = 220
///   gt; designs/farms/iron/parts/carousel-scheduler/offdelay-k3.txt) west of the latch, its input
///   not A (W_A1, through three repeaters west and one north): A held 220 gt and its output stone
///   A2 goes dark;
/// - G, give up: a wall torch on A2 (lit when A2 is dark), dust down to y -4 and east along z -5
///   (two repeaters), then (1) south along x 1, up, and west through five repeaters along z -1 into
///   the H latch's D2 (sets H: the T to the exit, the activator off, the hatch shut, the kick 20 gt
///   later), (2) east and up into A's reset. G is one-way into both: R never reaches D2.
/// In a normal visit the nap sets H long before G; when the return comes after 220 gt (the kick
/// waiting for the slot), G fires with H already set (nothing changes) and R resets everything.
fn giveup(b: &mut Builder) -> Result<()> {
    let rep = |f: &str| format!("repeater[delay=1,facing={f},locked=false,powered=false]");
    let dust = "redstone_wire[east=none,north=none,power=0,south=none,west=none]";
    let on = |b: &mut Builder, c: [i32; 3], part: &str| -> Result<()> {
        put(b, [c[0], c[1] - 1, c[2]], "stone")?;
        put(b, c, part)
    };
    // ARR
    swap(b, [0, 0, 0], "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    swap(b, [0, -1, 0], "stone")?;
    on(b, [0, -1, -1], &rep("south"))?;
    put(b, [0, -1, -2], "stone")?;
    on(b, [0, -2, -2], dust)?;
    for z in [-3, -4, -5] {
        on(b, [0, -2, z], &rep("south"))?;
    }
    // A: N_A1 (0,-2,-6), W_A1 west; D_A1 north and east into N_A2 (3,-2,-8); W_A2 south of it,
    // D_A2 west into N_A1
    on(b, [0, -2, -6], "stone")?;
    put(b, [-1, -2, -6], "redstone_wall_torch[facing=west,lit=true]")?;
    b.set(-1, -3, -6, "stone");
    for c in [[-1, -2, -7], [-1, -2, -8], [0, -2, -8], [1, -2, -8], [2, -2, -8]] {
        on(b, c, dust)?;
    }
    on(b, [3, -2, -8], "stone")?;
    put(b, [3, -2, -7], "redstone_wall_torch[facing=south,lit=false]")?;
    b.set(3, -3, -7, "stone");
    for c in [[3, -2, -6], [2, -2, -6], [1, -2, -6]] {
        on(b, c, dust)?;
    }
    // (G into N_A2 a 4-tick repeater: G's own reset of A, which blocks the comparator below,
    // comes after G has set H)
    on(b, [4, -2, -8], "repeater[delay=4,facing=east,locked=false,powered=false]")?; // G into N_A2
    on(b, [3, -2, -9], &rep("north"))?; // R into N_A2
    // not A to the timer's input: repeaters west, dust, a repeater north into its corner dust
    // (written powered, their state at rest: built from the generator's states, unpowered, they
    // would take 6 gt to carry not A to X below, and G's torch, checked 2 gt after its placement,
    // would light once and set H)
    let rep_on = |f: &str| format!("repeater[delay=1,facing={f},locked=false,powered=true]");
    for x in [-2, -3, -4] {
        on(b, [x, -2, -6], &rep_on("east"))?;
    }
    on(b, [-5, -2, -6], dust)?;
    on(b, [-5, -2, -7], &rep_on("south"))?;
    // the timer (day-1's k = 3, as `circuit timer long --k 3` writes it, its frame moved by
    // (-6, -3, -10): its input port (1,1,3) the repeater above, its output port (-7,1,0) G's torch)
    const TIMER: [(i32, i32, i32, &str); 36] = [
        (-5, 0, -2, "S"), (-4, 0, -2, "S"), (-3, 0, -2, "S"), (-2, 0, -2, "S"),
        (-6, 0, -1, "S"), (-2, 0, -1, "S"),
        (-5, 0, 0, "S"), (-4, 0, 0, "S"), (-3, 0, 0, "S"), (-1, 0, 0, "S"), (1, 0, 0, "S"),
        (0, 0, 1, "S"), (2, 0, 1, "S"),
        (1, 0, 2, "S"), (2, 0, 2, "S"),
        (-6, 1, -2, "S"), (-5, 1, -2, "W"), (-4, 1, -2, "W"), (-3, 1, -2, "W"), (-2, 1, -2, "d"),
        (-6, 1, -1, "s"), (-2, 1, -1, "d"),
        (-6, 1, 0, "S"), (-5, 1, 0, "E"), (-4, 1, 0, "E"), (-3, 1, 0, "E"), (-2, 1, 0, "S"), (-1, 1, 0, "L"), (0, 1, 0, "S"), (1, 1, 0, "E"), (2, 1, 0, "S"),
        (0, 1, 1, "n"), (2, 1, 1, "d"),
        (0, 1, 2, "S"), (1, 1, 2, "d"), (2, 1, 2, "d"),
    ];
    for (x, y, z, k) in TIMER {
        let s = match k {
            "S" => "stone",
            "d" => "redstone_wire[east=side,north=side,power=0,south=side,west=side]",
            "E" => "comparator[facing=east,mode=compare,powered=false]",
            "W" => "comparator[facing=west,mode=compare,powered=false]",
            "n" => "comparator[facing=north,mode=compare,powered=false]",
            "s" => "comparator[facing=south,mode=compare,powered=false]",
            _ => "repeater[delay=1,facing=east,locked=false,powered=false]",
        };
        put(b, [x - 6, y - 3, z - 10], s)?;
    }
    // G = A2 dark AND A: a torch on X (-14,-2,-10), X powered by A2 (a repeater off the timer's
    // output port) or by not A (dust west from the timer's input line and north): G lights only
    // while a visit is under way, so no power-on or refill glitch of the timer can set H, and G
    // ends as soon as A is reset
    on(b, [-13, -2, -10], &rep("east"))?;
    put(b, [-14, -2, -10], "stone")?;
    for x in -14..=-6 {
        on(b, [x, -2, -6], dust)?;
    }
    for z in [-7, -8] {
        on(b, [-14, -2, z], dust)?;
    }
    // (a repeater into X, not dust: X, which the A2 repeater powers strongly, would feed dust
    // beside it, and through the line the timer's own input: the timer would hold itself)
    on(b, [-14, -2, -9], &rep_on("south"))?;
    put(b, [-15, -2, -10], "redstone_wall_torch[facing=west,lit=false]")?;
    // G: dust down to y -4, south, east along z -5 (repeaters at x -10 and 3, facing west: out
    // east)
    on(b, [-16, -2, -10], dust)?;
    on(b, [-16, -3, -9], dust)?;
    for z in -8..=-5 {
        on(b, [-16, -4, z], dust)?;
    }
    for x in -15..=6 {
        on(b, [x, -4, -5], &if x == -10 || x == 3 { rep("west") } else { dust.to_string() })?;
    }
    // (1) south along x 1, up to (1,-3,-3), a repeater north of a stone at (1,-3,-1) (not dust:
    // dust there, beside Ta, which H powers strongly, would feed H back into D2 and hold it), west
    // through five repeaters along z -1 into the H latch's D2 (-5,-3,-1)
    on(b, [1, -4, -4], dust)?;
    on(b, [1, -3, -3], dust)?;
    on(b, [1, -3, -2], &rep("north"))?;
    put(b, [1, -3, -1], "stone")?;
    // (the second from D2 a comparator subtracting H, read from the H-out dust south of it at
    // (-3,-3,0): G sets H only when H is not set already, and stops as soon as it is; a late
    // return, its nap long done, fires G with H set: nothing reaches D2, so G cannot race R's
    // reset)
    for x in -4..=0 {
        on(b, [x, -3, -1], &if x == -3 { "comparator[facing=east,mode=subtract,powered=false]".to_string() } else { rep("east") })?;
    }
    // (and the comparator's north side not A: dust south from W_A1, west, south, down into a
    // repeater into its side: G reaches D2 only while A is still set, so a G that the timer fires
    // just as R resets A (the return 238 gt after the arrival) dies at the comparator)
    for c in [[-1, -2, -5], [-2, -2, -5], [-3, -2, -5], [-3, -2, -4], [-3, -3, -3]] {
        on(b, c, dust)?;
    }
    on(b, [-3, -3, -2], &rep("north"))?;
    // (2) north along x 6, up twice into the reset repeater's back (5,-2,-8)
    on(b, [6, -4, -6], dust)?;
    on(b, [6, -4, -7], dust)?;
    on(b, [6, -3, -8], dust)?;
    on(b, [5, -2, -8], dust)?;
    // R's branch: a repeater north off the R line's corner (3,0,-3), dust east and north along x 4
    // at y 0, west, down twice into the R repeater's back (3,-2,-10)
    on(b, [3, 0, -4], &rep("south"))?;
    on(b, [3, 0, -5], dust)?;
    for z in -12..=-5 {
        on(b, [4, 0, z], dust)?;
    }
    on(b, [3, 0, -12], dust)?;
    on(b, [3, -1, -11], dust)?;
    on(b, [3, -2, -10], dust)?;
    // glass caps on its solid tops a golem could stand on (N_A1, N_A2, the timer's stone under
    // its return corner, the ARR column's stone)
    // (and the timer's stone at z -12, within a golem's search of the bench's hold row at z -20)
    for c in [[0, -1, -6], [3, -1, -8], [-6, -1, -8], [0, 0, -2], [1, -2, -1], [-12, -1, -12]] {
        put(b, c, "glass")?;
    }
    Ok(())
}

/// The bench design, its controls bench-driven or `wired` ([`wire`]).
pub fn design_with(wired: bool) -> Result<String> {
    design_opts(wired, false, false)
}

/// The bench design ([`design_with`]), or with `alone` the siding by itself (farmopt 2026-11-10 for
/// ironfarm's loop bench, `queue-loop-run --siding`): no launcher, approach, run-out, hold, hold
/// zombies or carts, only the siding's own blocks and its dorm zombie. `exit_brake` (day-1's
/// side-drop merge wants at most 0.36 b/t): a braked powered rail just past the exit cell, at x
/// exit+1 on glass (not the exit cell itself: the slot tripwire's line runs down x 6, and the stone
/// under its dust beside the exit cell would power a powered rail there whenever the slot's
/// tripwire fires).
pub fn design_opts(wired: bool, alone: bool, exit_brake: bool) -> Result<String> {
    design_opts_cage(wired, alone, exit_brake, false)
}

/// [`design_opts`], and with `cage_only` the bed's open north front (0,0,1) closed with glass, two
/// high, so the bed head is reachable by path only from the cage's free cell (no villager off the
/// siding finds a path to it for AcquirePoi).
pub fn design_opts_cage(wired: bool, alone: bool, exit_brake: bool, cage_only: bool) -> Result<String> {
    design_full(wired, alone, exit_brake, cage_only, GiveUp::None)
}

/// Which give-up the wired siding has: none, its own timer ([`giveup`]), or day-1's scheduler's
/// force input ([`giveup_sched`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiveUp {
    None,
    Timer,
    Scheduler,
}

/// [`design_opts_cage`], and with `give_up` the wired siding's give-up.
pub fn design_full(wired: bool, alone: bool, exit_brake: bool, cage_only: bool, give_up: GiveUp) -> Result<String> {
    let s = layout();
    let name = format!("carousel-bed-siding{}{}{}{}{}", if wired { "-wired" } else { "" }, match give_up { GiveUp::None => "", GiveUp::Timer => "-giveup", GiveUp::Scheduler => "-force" }, if exit_brake { "-brake" } else { "" }, if cage_only { "-cage" } else { "" }, if alone { "-alone" } else { "" });
    let mut b = Builder::new(&name);
    build(&mut b, &s, !alone)?;
    if cage_only {
        put(&mut b, [s.bed_head.x, 0, s.bed_head.z - 1], "glass")?;
        put(&mut b, [s.bed_head.x, 1, s.bed_head.z - 1], "glass")?;
    }
    if exit_brake {
        let brake = "powered_rail[powered=false,shape=east_west,waterlogged=false]";
        let c = [s.exit.x + 1, s.exit.y, s.exit.z];
        if alone {
            put(&mut b, c, brake)?;
            put(&mut b, [c[0], c[1] - 1, c[2]], "glass")?;
        } else {
            swap(&mut b, c, brake)?;
        }
    }
    if wired {
        wire(&mut b, &s)?;
    }
    if give_up == GiveUp::Scheduler {
        ensure!(wired, "the give-up is part of the wired siding (--wired)");
        giveup_sched(&mut b)?;
        // (the torch unlit... none: not V's torch is lit at rest; the cap after the bulb)
    }
    if give_up == GiveUp::Timer {
        ensure!(wired, "the give-up is part of the wired siding (--wired)");
        giveup(&mut b)?;
        // (its two torches unlit at rest go last too: W_A2, A's torch, the latch bistable as the
        // H latch is; G's torch, last of all. Before them the timer's input repeater and then the
        // glass caps: its ring takes about 12 gt to fill, and holds 15 or 14 at rest depending
        // on how it was filled, so it is filled once, whole, before the build ends)
        let mut last = vec![BlockPos::new(-5, -2, -7)];
        // (then G's line along z -5, which the timer's fill does not touch: time for the fill)
        for x in -15..=6 {
            last.push(BlockPos::new(x, -4, -5));
        }
        for c in [[0, -1, -6], [3, -1, -8], [-6, -1, -8], [0, 0, -2], [1, -2, -1], [-12, -1, -12], [-5, -2, 2], [-7, -2, -1], [2, -1, 6], [4, 1, -2]] {
            last.push(BlockPos::new(c[0], c[1], c[2]));
        }
        last.extend([BlockPos::new(-6, -3, -1), BlockPos::new(1, -2, 6), BlockPos::new(3, -2, -7), BlockPos::new(-15, -2, -10)]);
        b.d.meta.insert("build".into(), crate::placing::BuildPlan { last, fills_last: false }.meta());
    }
    for i in (0..21).filter(|_| !alone) {
        let c = hold_cell(i);
        put(&mut b, at(c), "powered_rail[powered=false,shape=east_west,waterlogged=false]")?;
        put(&mut b, [c.x, c.y - 1, c.z], "glass")?;
        // (no glass between the hold's carts: it would cut the riders' sight of the hold zombies;
        // a cart at rest on its braked rail stays, two blocks from the next)
        b.entity("minecart", [c.x as f64 + 0.5, 0.0625, c.z as f64 + 0.5], Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"));
    }
    // the walkway: none (the held riders panic all night, as the loop's do: they run no REST
    // behaviour, so they neither give up the bed nor take its one ticket)
    // the hold zombies: in boats on glass 3 north of the hold row, every 8 along it (each rider
    // sees one within 8: every held rider panics at night, as on the loop), roofed
    for zx in [-18, -10, -2, 6, 14, 22].into_iter().filter(|_| !alone) {
        put(&mut b, [zx, -1, -23], "glass")?;
        put(&mut b, [zx, 3, -23], "oak_leaves[distance=1,persistent=true,waterlogged=false]")?;
        b.entity("oak_boat", [zx as f64 + 0.5, 0.0, -22.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    // (the interface cells: a mob farm takes no input port)
    b.d.meta.insert("siding".into(), format!("v1{}{} entry={},{},{} exit={},{},{} release={},{},{}{}", if wired { " wired" } else { "" }, if alone { " alone" } else { "" }, s.entry.x, s.entry.y, s.entry.z, s.exit.x, s.exit.y, s.exit.z, s.release.x, s.release.y, s.release.z, if give_up == GiveUp::Scheduler { format!(" force={},{},{}", FORCE_CELL[0], FORCE_CELL[1], FORCE_CELL[2]) } else { String::new() }));
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), at(s.bed_head));
    if alone {
        return Ok(b.text(&[
            "The carousel's bed siding by itself (no bench: no launcher, approach, run-out, hold or carts; for pasting into a loop bench): a napper's cart enters at the origin moving +x,",
            "a T sends it down the bed line, the napper naps in the one bed and is woken by the dorm zombie through the hatch, re-boards its cart, which waits on the exit slot until the release pulse at (4,-2,0); out at (6,-1,0) moving +x.",
        ]));
    }
    Ok(b.text(&[
        "The carousel's bed siding (bench): a napper's cart enters at the origin moving +x, a T sends it down the bed line (the free cell, the activator, the park rail),",
        "the napper naps in the one bed and is woken by the dorm zombie through the hatch, re-boards its cart, which waits on the exit slot until the release pulse.",
    ]))
}

/// The static check: every rider and napper cell's golem search: any spot is a stray.
pub fn check(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let s = layout();
    let run = crate::station::world_of(spec, 1)?;
    let w = &run.world;
    let mut cells: Vec<BlockPos> = vec![s.entry, s.tee, s.boost, s.free_cell, s.activator_rail];
    for z in s.activator_rail.z + 1..=s.park_rail.z {
        cells.push(BlockPos::new(s.free_cell.x, 0, z));
    }
    cells.extend([BlockPos::new(2, 0, 0), BlockPos::new(3, 0, 0), s.slot_rails[0], s.slot_rails[1], s.exit]);
    let mut out = Vec::new();
    let mut strays: std::collections::BTreeSet<[i32; 3]> = Default::default();
    for c in &cells {
        let n = redsim::entity::mob::spawn::golem_spawn_cells_from(w, *c).into_iter().filter(|(_, r)| r.is_ok()).map(|(p, _)| [p.x, p.y, p.z]).collect::<Vec<_>>();
        for p in &n {
            strays.insert(*p);
        }
        out.push(format!("search from {c}: {} spawnable spots", n.len()));
    }
    out.push(format!("strays: {} {:?}", strays.len(), strays.iter().take(12).collect::<Vec<_>>()));
    // the siding's own blocks (not the bench's hold, approach and run-out) and its box
    let own: Vec<&(BlockPos, Block)> = spec
        .blocks
        .iter()
        .filter(|(p, b)| !matches!(b, Block::Air) && p.z > -15 && !(p.z == 0 && p.x < 0 && (-1..=1).contains(&p.y)) && !(p.z == 0 && p.x > s.exit.x && (p.y == -1 || p.y == -2)))
        .collect();
    let lo = own.iter().fold([i32::MAX; 3], |a, (p, _)| [a[0].min(p.x), a[1].min(p.y), a[2].min(p.z)]);
    let hi = own.iter().fold([i32::MIN; 3], |a, (p, _)| [a[0].max(p.x), a[1].max(p.y), a[2].max(p.z)]);
    let mut kinds: std::collections::BTreeMap<String, u32> = Default::default();
    for (_, b) in &own {
        let n = b.to_string();
        let n = n.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
        *kinds.entry(n).or_insert(0) += 1;
    }
    out.push(format!("siding blocks {} in x {}..{} y {}..{} z {}..{}: {kinds:?}", own.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]));
    Ok(out)
}

/// The bench's options.
#[derive(Clone, Debug)]
pub struct BenchOpts {
    pub seed: i64,
    pub nights: u32,
    /// The visits' spacing (gt): uniform in this range.
    pub gap: (i64, i64),
    /// The arriving cart's speed (blocks a tick): uniform in this range.
    pub speed: (f64, f64),
    /// The release after the arrival (gt): uniform in this range.
    pub release: (i64, i64),
    pub trace: bool,
    /// At most this many visits a run (`None`: every one the nights allow).
    pub max_visits: Option<usize>,
    /// The design is wired ([`wire`]): the bench touches no control, only the release (ironfarm's
    /// input) and the arrivals (teleported onto the approach).
    pub wired: bool,
    /// The traces' window (gt after the start).
    pub trace_window: (i64, i64),
    /// Arrivals launched from rest on the bench's launcher (the drive's `tp` moves carts at rest),
    /// not teleported moving at a drawn speed.
    pub launch: bool,
    /// The release input held powered the whole run (day-1's case: a napped cart leaves its slot
    /// as soon as it reaches it): no release pulses; a visit counts as ready and released when its
    /// cart reaches the slot with its villager.
    pub release_held: bool,
    /// Record the run as a drive (acts, cart positions every 25 gt of a visit, the totals).
    pub drive: bool,
    /// How the design is put in the world (the brief's placement checks: mobfarm::placing).
    pub placement: crate::placing::Placement,
    /// The design's listed build steps (its `build:` line), for a build in order.
    pub plan: crate::placing::BuildPlan,
    /// Forced failures (the give-up's test): the first N villagers cannot sleep: just before each
    /// of their visits the bench erases their HOME (the bed's ticket stays taken by the others'
    /// claims, so they cannot claim it again); their carts must still leave.
    pub fail_no_home: u32,
    /// Forced failures, the other kind: every visit by day (from day time 1000 to 12000) instead
    /// of by night: no napper can sleep, every visit must end with its cart on the slot and
    /// released within 500 gt (and every villager still holding HOME at each check).
    pub day_visits: bool,
    /// Drive the force input (FORCE_CELL) as day-1's scheduler would: a 12 gt pulse this many gt
    /// after each visit's arrival (spread over the range by the visit), every visit; only when the
    /// design has the scheduler give-up's gate.
    pub force: Option<(i64, i64)>,
    /// Fault injection (ironspec 2026-11-10): on this visit (its index in the run) the napper
    /// cannot sleep (its HOME erased) and the force input is not pulsed; the next visits are sent
    /// on schedule all the same, as the loop would, with the dropped visit's cart still inside.
    pub drop_force: Option<usize>,
}

/// One visit's record.
#[derive(Clone, Debug, Default)]
pub struct VisitRec {
    pub villager: u32,
    pub cart: u32,
    pub arrived: i64,
    pub dropped: Option<i64>,
    pub slept: Option<i64>,
    pub woken: Option<i64>,
    pub reboarded: Option<i64>,
    pub ready: Option<i64>,
    pub released: Option<i64>,
    pub exit_speed: Option<f64>,
    /// Its speed leaving the exit cell (x past the exit cell's east edge; with `--exit-brake` the
    /// speed past the brake).
    pub leave_speed: Option<f64>,
    pub own_cart: bool,
    /// The night it belongs to (the day count at its dusk).
    pub night: i64,
    /// Ticks waiting on the slot at night, and of those panicking.
    pub wait_ticks: u32,
    pub wait_panic: u32,
    /// The cart's x at rest on the slot; the first and last calm waiting night tick (after ready).
    pub rest_x: Option<f64>,
    /// At ready: the rider's eye sees the dorm zombie's (and the two eye heights).
    pub sees: Option<(bool, f64, f64)>,
    /// The first few distinct states of a calm waiting rider (activity, hostile, vehicle, day).
    pub calm_acts: Vec<String>,
    /// Launched: it has reached the entry (its `arrived` is that tick).
    pub entered: bool,
    pub calm: Option<(i64, i64)>,
    pub problem: Option<String>,
    /// A forced failure (`fail_no_home`): its villager's HOME erased just before the visit.
    pub failed: bool,
    /// When the bench pulsed the force input for it (day-1's scheduler's place).
    pub forced: Option<i64>,
    /// When its cart left past the exit cell (x past its east edge).
    pub exited: Option<i64>,
    /// The fault injection (`drop_force`): its napper cannot sleep and its F is dropped.
    pub force_dropped: bool,
    /// The scheduler give-up's bulb V at the visit's start (before its arrival): lit = the bulb's
    /// phase inverted (a visit before it never returned).
    pub v_before: Option<bool>,
    /// The fault injection: when its cart, left behind in the siding, came onto the exit branch.
    pub unstuck: Option<i64>,
}

/// What the bench measured.
#[derive(Clone, Debug, Default)]
pub struct BenchReport {
    pub visits: Vec<VisitRec>,
    pub errors: Vec<String>,
    /// The run as a drive (`bo.drive`): the viewer replays it.
    pub drive: Option<String>,
    /// Built in order: the cells the build left unlike the file (none: the build ends as written),
    /// and the build's steps, cells and ticks.
    pub built_diff: Vec<String>,
    /// ironspec's check: at every dawn (day time 0) and midday (6000), (time, day time, villagers
    /// holding HOME = the bed).
    pub home_checks: Vec<(i64, i64, u32)>,
    /// Every HOME loss or regain (who, when, where, its activity).
    pub home_events: Vec<String>,
    /// Each regained loss's time without HOME (gt).
    pub home_gaps: Vec<i64>,
    pub build: Option<(usize, usize, i64)>,
    /// The run's forced failures (`BenchOpts::fail_no_home`, `day_visits`).
    pub fail_no_home: u32,
    pub day_visits: bool,
}

fn block(s: &str) -> Block {
    Block::parse(&format!("minecraft:{s}")).expect("a known block")
}

/// A block state in a few letters for the circuit trace: lit/powered/shape/open.
fn short_state(s: &str) -> String {
    let s = s.trim_start_matches("minecraft:");
    let name: String = s.split('[').next().unwrap_or(s).chars().take(5).collect();
    let mut flags = String::new();
    for (k, tag) in [("lit=true", "L"), ("powered=true", "P"), ("open=true", "O"), ("occupied=true", "Z"), ("attached=true", "")] {
        if s.contains(k) {
            flags.push_str(tag);
        }
    }
    if let Some(i) = s.find("shape=") {
        let sh: String = s[i + 6..].chars().take_while(|c| *c != ',' && *c != ']').collect();
        flags.push_str(&format!("({sh})"));
    }
    if let Some(i) = s.find("power=") {
        let p: String = s[i + 6..].chars().take_while(|c| c.is_ascii_digit()).collect();
        flags.push_str(&format!("{p}"));
    }
    format!("{name}{flags}")
}

/// The bench: visits through the nights (each night from day time 12000, `gap` apart, until
/// 23000), each taking the villager napped longest ago from the far hold, its cart teleported to
/// the approach moving +x at `speed`; the controls driven ideally (the module's doc); the release
/// `release` after the arrival; the cart back to its hold cell once out past the exit.
pub fn bench(spec: &FarmSpec, bo: &BenchOpts) -> Result<BenchReport, String> {
    let s = layout();
    let ro = RunOpts { ai: true, ..RunOpts::steady(bo.seed, bo.nights * 24000 + 24000, WalkMode::Random) };
    // (the judge's placement and the settle; the raw paste; or built in order)
    let (mut run, built) = crate::placing::start(spec, &ro, bo.placement, &bo.plan)?;
    let built_diff = if built.is_some() { crate::placing::diff_from_file(&run.world, spec, &ro)? } else { Vec::new() };
    let first = run.world.entities.next_id;
    let mut spawned = Vec::new();
    for (ty, pos, nbt) in &spec.entities {
        spawned.push(redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?);
    }
    run.own = (first..run.world.entities.next_id).collect();
    let t0 = run.world.time;
    if bo.trace {
        eprintln!("t0 {t0} (traces' gt are after it; the visits' arrival times are absolute)");
    }
    // (the drive's times count from the spawn, or from the build's start when built in order)
    let origin = built.as_ref().map_or(t0, |b| b.start);
    let mut drive = crate::drive::Drive::new(bo.drive);
    drive.name_entities(&run.world, &spawned, origin);
    for l in built.iter().flat_map(|b| b.lines.iter()) {
        drive.push(l.clone());
    }
    let drive = drive;
    let bed = s.bed_head;
    // (no bed act: every villager's HOME = the bed comes from the design file's `port home:` line,
    // the state R1's break-and-replace build steps leave; redsim counts the claims as the bed's
    // taken ticket; the bench touches no villager memory)
    // the villagers and their carts (by the hold cell they start on)
    let mut pairs: Vec<(u32, u32, i32)> = Vec::new();
    for i in 0..run.world.entities.list.len() {
        let e = &run.world.entities.list[i];
        if !matches!(e.kind, EntityKind::Minecart(..)) || e.passengers.is_empty() {
            continue;
        }
        let cell = BlockPos::new(e.pos.x.floor() as i32, 0, e.pos.z.floor() as i32);
        if let Some(h) = (0..21).find(|&h| hold_cell(h) == cell) {
            pairs.push((e.id, e.passengers[0], h));
        }
    }
    pairs.sort_by_key(|p| p.2);
    let index_of = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    let mut rng = redsim::rng::LegacyRandom::new(bo.seed ^ 0x7369_6469);
    let set = |w: &mut World, p: BlockPos, st: &str| w.set_block(p, block(st), redsim::world::flags::ALL);
    let teleport = |w: &mut World, cart: u32, pos: Vec3, vel: Vec3| {
        let Some(i) = w.entities.list.iter().position(|e| e.id == cart && !e.removed) else { return };
        w.entities.set_pos(i, pos);
        w.entities.list[i].delta_movement = vel;
        let ps = w.entities.list[i].passengers.clone();
        for pid in ps {
            if let Some(j) = w.entities.list.iter().position(|e| e.id == pid && !e.removed) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    };
    let mut rep = BenchReport { fail_no_home: bo.fail_no_home, day_visits: bo.day_visits, ..Default::default() };
    // (the visits' nights: the first full night after the spawn and the `nights` after it, so a
    // spawn after dusk (a build in order ends at any day time) skips the night already under way;
    // the run ends the day after the last)
    // (with `day_visits` the same schedule by day, from day time 1000: every napper fails)
    let w0: i64 = if bo.day_visits { 1000 } else { 12000 };
    let first_night = (run.world.day_time() - w0).div_euclid(24000) + 1;
    let end = t0 + (bo.nights as i64 + 1) * 24000 + if (run.world.day_time() - w0).rem_euclid(24000) < 12000 { 24000 } else { 0 };
    if bo.release_held {
        set(&mut run.world, s.release, "redstone_block");
        drive.act(t0, format!("set {} {} {} minecraft:redstone_block", s.release.x, s.release.y, s.release.z));
    }
    // the schedule: each night's visit ticks, every villager once a night
    let mut next_visit: Option<i64> = None;
    let mut last_nap: Vec<i64> = vec![i64::MIN; pairs.len()];
    let mut night_of_visit: Vec<i64> = vec![-1; pairs.len()];
    let mut active: Option<usize> = None; // the visit inside (not yet on the exit branch)
    let mut waiting: Vec<usize> = Vec::new(); // visits on the slot (not yet released)
    let mut stuck: Vec<usize> = Vec::new(); // the fault injection's visits left inside
    let mut out: Vec<(usize, i64)> = Vec::new(); // visits on the run-out (released)
    let mut occupied = false;
    let mut kick_at: Option<i64> = None;
    let mut park_off: Option<i64> = None;
    let mut release_off: Option<i64> = None;
    // (the force input driven as day-1's scheduler would, when the design has the gate)
    let gate = BlockPos::new(FORCE_CELL[0], FORCE_CELL[1], FORCE_CELL[2] + 1);
    let force = (bo.force.is_some() && matches!(run.world.get(gate), Block::Comparator { .. })).then(|| bo.force.unwrap_or((150, 170)));
    let mut force_off: Option<i64> = None;
    let mut reset_pending = false;
    let mut homes_seen: Vec<bool> = vec![true; pairs.len()];
    let mut home_now: Vec<bool> = pairs.iter().map(|(_, v, _)| index_of(&run.world, *v).is_some_and(|i| brain::brain(&run.world, i).home == Some(bed))).collect();
    let mut lost_at: Vec<Option<i64>> = vec![None; pairs.len()];
    let mut probe_last: Vec<String> = Vec::new();
    while run.world.time < end {
        let now = run.world.time;
        let day = run.world.day_time().rem_euclid(24000);
        let night = !(10..12000).contains(&day);
        // (the night a visit belongs to: the day count at its night's dusk)
        let night_n = (run.world.day_time() - w0).div_euclid(24000);
        let in_window = (run.world.day_time() - w0).rem_euclid(24000) < 11000;
        // ironspec's check: every villager's HOME = the bed at every dawn and every midday
        if day == 0 || day == 6000 {
            let held = pairs.iter().filter(|(_, v, _)| index_of(&run.world, *v).is_some_and(|i| brain::brain(&run.world, i).home == Some(bed))).count();
            rep.home_checks.push((now, day, held as u32));
        }
        // the next visit: scheduled through the night
        if next_visit.is_none() && in_window && night_n >= first_night && night_n < first_night + bo.nights as i64 {
            next_visit = Some(now + rng.next_int(400) as i64);
        }
        if let Some(tv) = next_visit
            && now >= tv
        {
            if !in_window {
                next_visit = None;
            } else if active.is_none_or(|a| rep.visits[a].force_dropped) && bo.max_visits.is_none_or(|m| rep.visits.len() < m) {
                // (the fault injection: the dropped visit's cart left inside, the next sent as the
                // loop would send it)
                if let Some(a) = active.take() {
                    stuck.push(a);
                }
                // the villager napped longest ago, not out already and not visited this night
                let busy: Vec<u32> = waiting.iter().chain(out.iter().map(|(v, _)| v)).chain(stuck.iter()).map(|&k| rep.visits[k].villager).collect();
                let pick = (0..pairs.len()).filter(|&k| !busy.contains(&pairs[k].1) && night_of_visit[k] != night_n).min_by_key(|&k| last_nap[k]);
                if let Some(k) = pick {
                    let (cart, villager, _) = pairs[k];
                    let v = bo.speed.0 + (bo.speed.1 - bo.speed.0) * rng.next_float() as f64;
                    if bo.launch {
                        // (at rest on the launcher's first booster, against its glass stop: the
                        // drive's `tp` moves a cart at rest)
                        let p = Vec3::new(LAUNCH_X, 0.0625, 0.5);
                        teleport(&mut run.world, cart, p, Vec3::new(0.0, 0.0, 0.0));
                        drive.act(now, format!("tp {} {} {} {}", drive.name(cart), p.x, p.y, p.z));
                    } else {
                        teleport(&mut run.world, cart, Vec3::new(APPROACH_X, 0.0625, 0.5), Vec3::new(v, 0.0, 0.0));
                    }
                    let eta = if bo.launch { 30 } else { ((-APPROACH_X - 0.0) / v).round() as i64 };
                    // (a forced failure: the villager cannot sleep, its HOME erased)
                    let dropped_f = bo.drop_force == Some(rep.visits.len());
                    let failed = (k as u32) < bo.fail_no_home || bo.day_visits || dropped_f;
                    if ((k as u32) < bo.fail_no_home || dropped_f) && let Some(i) = index_of(&run.world, villager) {
                        brain::brain_mut(&mut run.world, i).home = None;
                    }
                    // (the fault injection's napper truly cannot sleep: the bed's ticket taken too,
                    // as R1's end state leaves it, so it cannot claim the bed again either)
                    if dropped_f {
                        redsim::entity::mob::ai::poi::take(&mut run.world, bed);
                    }
                    let v_before = force.is_some().then(|| matches!(run.world.get(BlockPos::new(0, -1, -1)), Block::Bulb { lit: true, .. }));
                    rep.visits.push(VisitRec { villager, cart, arrived: now + eta, night: night_n, failed, force_dropped: dropped_f, v_before, ..Default::default() });
                    active = Some(rep.visits.len() - 1);
                    last_nap[k] = now;
                    night_of_visit[k] = night_n;
                }
                let gap = bo.gap.0 + rng.next_int((bo.gap.1 - bo.gap.0 + 1) as i32) as i64;
                next_visit = Some(now + gap);
            }
        }
        // the controls: the bed's OCCUPIED edges (the observer), the kick once the slot is empty,
        // the reset when a cart is onto the exit branch, the release pulses
        let occ = matches!(run.world.get(bed), Block::Bed { occupied: true, .. });
        if !bo.wired && occ && !occupied {
            set(&mut run.world, s.tee_ctrl, "target[power=0]");
            set(&mut run.world, s.hatch_ctrl, "glass");
            set(&mut run.world, s.activator, "glass");
        }
        if !bo.wired && !occ && occupied {
            kick_at = Some(now + 60);
        }
        occupied = occ;
        if let Some(k) = kick_at
            && now >= k
            && waiting.is_empty()
        {
            set(&mut run.world, s.park, "redstone_block");
            park_off = Some(now + 20);
            kick_at = None;
        }
        if park_off == Some(now) {
            set(&mut run.world, s.park, "glass");
            park_off = None;
        }
        if reset_pending && !bo.wired {
            set(&mut run.world, s.tee_ctrl, "redstone_block");
            set(&mut run.world, s.hatch_ctrl, "redstone_block");
            set(&mut run.world, s.activator, "redstone_block");
        }
        reset_pending = false;
        // releases due
        for &k in &waiting.clone() {
            if bo.release_held {
                break;
            }
            let (arrived, villager, released, ready) = (rep.visits[k].arrived, rep.visits[k].villager, rep.visits[k].released, rep.visits[k].ready);
            let due = arrived + bo.release.0 + ((villager as i64 * 7919) % (bo.release.1 - bo.release.0 + 1));
            // (a fault-injected visit's cart, out late, is released once ready on the slot)
            if now >= due && released.is_none() && (ready.is_some() || !rep.visits[k].force_dropped) {
                if ready.is_none() {
                    rep.visits[k].problem.get_or_insert(format!("not ready at its release (gt {})", due - arrived));
                }
                set(&mut run.world, s.release, "redstone_block");
                drive.act(now, format!("set {} {} {} minecraft:redstone_block", s.release.x, s.release.y, s.release.z));
                release_off = Some(now + 10);
                rep.visits[k].released = Some(now);
                waiting.retain(|&x| x != k);
                out.push((k, now));
            }
        }
        if release_off == Some(now) {
            set(&mut run.world, s.release, "glass");
            drive.act(now, format!("set {} {} {} minecraft:glass", s.release.x, s.release.y, s.release.z));
            release_off = None;
        }
        // day-1's scheduler on the force input (the siding's give-up driven by it): a 12 gt pulse
        // from its visit register's D3 gating REL+20 (explore/day1-counter 7d947146b), on at visit
        // +328..+340, 118-150 gt after the arrival as k shifts the arrival (+190..+210), every
        // visit (its register cannot see the return)
        if let Some((fa, fb)) = force {
            let fc = BlockPos::new(FORCE_CELL[0], FORCE_CELL[1], FORCE_CELL[2]);
            let n = rep.visits.len();
            for k in n.saturating_sub(2)..n {
                let v = &rep.visits[k];
                if v.forced.is_none() && !v.force_dropped && (!bo.launch || v.entered) {
                    let delay = fa + ((v.villager as i64 * 104_729 + v.arrived) % (fb - fa + 1));
                    if now >= v.arrived + delay {
                        set(&mut run.world, fc, "redstone_block");
                        drive.act(now, format!("set {} {} {} minecraft:redstone_block", fc.x, fc.y, fc.z));
                        force_off = Some(now + 12);
                        rep.visits[k].forced = Some(now);
                    }
                }
            }
            if force_off == Some(now) {
                set(&mut run.world, fc, "air");
                drive.act(now, format!("set {} {} {} minecraft:air", fc.x, fc.y, fc.z));
                force_off = None;
            }
        }
        run.tick();
        let t = run.world.time;
        // every villager's HOME every tick: each loss and each regain (ironspec's check reads the
        // dawn and midday counts; these say who, when, where and in which activity)
        for (n, (_, v, _)) in pairs.iter().enumerate() {
            let Some(i) = index_of(&run.world, *v) else { continue };
            let h = brain::brain(&run.world, i).home == Some(bed);
            if h != home_now[n] {
                home_now[n] = h;
                // (a loss's start, and on the regain the time without HOME)
                if h {
                    if let Some(l) = lost_at[n].take() {
                        rep.home_gaps.push(t - l);
                    }
                } else {
                    lost_at[n] = Some(t);
                }
                let w = &run.world;
                let e = &w.entities.list[i];
                let act = if brain::has_exact(w, i) { format!("{:?}", brain::ex(w, i).active) } else { "-".into() };
                let occ = matches!(w.get(bed), Block::Bed { occupied: true, .. });
                rep.home_events.push(format!("t0+{} (day time {}): villager {} {} at {:.2} {:.2} {:.2}, {act}, riding {}, bed occupied {occ}", t - t0, run.world.day_time().rem_euclid(24000), n, if h { "takes HOME = the bed" } else { "loses HOME" }, e.pos.x, e.pos.y, e.pos.z, e.vehicle.is_some()));
            }
        }
        let w = &run.world;
        // the visit inside: its stages
        if let Some(k) = active {
            let (villager, cart) = (rep.visits[k].villager, rep.visits[k].cart);
            // (launched from rest: its arrival is the tick it reaches the entry; the drive's check
            // holds the view's cart to the bench's every 25 gt of the visit)
            if let Some(c) = w.entities.list.iter().find(|x| x.id == cart && !x.removed) {
                if bo.launch && !rep.visits[k].entered && c.pos.x >= s.entry.x as f64 + 0.5 {
                    rep.visits[k].entered = true;
                    rep.visits[k].arrived = t;
                }
                if (t - t0) % 25 == 0 {
                    drive.expect(t, format!("pos {} {:.4} {:.4} {:.4}", drive.name(cart), c.pos.x, c.pos.y, c.pos.z));
                }
            }
            if let Some(i) = index_of(w, villager) {
                let e = &w.entities.list[i];
                let riding = e.vehicle;
                let asleep = redsim::entity::mob::mob(w, i).sleeping_pos.is_some();
                let vr = &mut rep.visits[k];
                if vr.dropped.is_none() && riding.is_none() {
                    vr.dropped = Some(t);
                }
                if vr.dropped.is_some() && vr.slept.is_none() && asleep {
                    vr.slept = Some(t);
                }
                if vr.slept.is_some() && vr.woken.is_none() && !asleep {
                    vr.woken = Some(t);
                }
                if vr.woken.is_some() && vr.reboarded.is_none() && riding.is_some() {
                    vr.reboarded = Some(t);
                    vr.own_cart = riding == Some(cart);
                }
                // (traced: the first visit's cart and napper every 3 gt until the re-board)
                if bo.trace && vr.reboarded.is_none() && t % 3 == 0 && t - vr.arrived < 400 && t - t0 >= bo.trace_window.0 && t - t0 < bo.trace_window.1 {
                    let c = w.entities.list.iter().find(|x| x.id == cart).map(|c| (c.pos, c.delta_movement));
                    if let Some((cp, cv)) = c {
                        eprintln!("gt {} (+{}): cart {:.3} {:.3} {:.3} v {:.3} {:.3} | napper {:.2} {:.2} {:.2} riding {} asleep {asleep} | park {} bed {}", t - t0, t - vr.arrived, cp.x, cp.y, cp.z, cv.x, cv.z, e.pos.x, e.pos.y, e.pos.z, riding.is_some(), w.get(s.park), matches!(w.get(bed), Block::Bed { occupied: true, .. }));
                    }
                }
                // (traced: the first seed's visits between their drop and their nap, every 10 gt)
                if bo.trace && vr.dropped.is_some() && vr.slept.is_none() && t % 10 == 0 && t - vr.arrived > 40 {
                    let b = brain::brain(w, i);
                    let act = brain::has_exact(w, i).then(|| {
                        let x = brain::ex(w, i);
                        format!("{:?} cant_reach {:?}", x.active, x.mem.cant_reach_since)
                    });
                    eprintln!("gt {}: visit {k} (villager {villager}) {:.2} {:.2} {:.2} riding {:?} asleep {asleep} act {act:?} home {:?} hostile {:?} woken {:?} slept {:?} bed {}", t - t0, e.pos.x, e.pos.y, e.pos.z, riding, b.home, b.nearest_hostile, b.last_woken.map(|x| t - x), b.last_slept.map(|x| t - x), w.get(bed));
                }
            }
            // the cart onto the exit branch: the reset, the visit now waiting
            if let Some(c) = w.entities.list.iter().find(|x| x.id == cart && !x.removed)
                && c.pos.x >= 2.0
                && c.pos.x < 3.0
                && c.pos.z < 1.0
                && c.delta_movement.x > 0.0
            {
                reset_pending = true;
                active = None;
                waiting.push(k);
            }
            if t - rep.visits[k].arrived > 2000 && active == Some(k) {
                rep.visits[k].problem.get_or_insert("still inside 2000 gt after its arrival".into());
                active = None;
            }
        }
        // (the fault injection: a dropped visit's cart left inside, onto the exit branch at last)
        let mut freed: Vec<usize> = Vec::new();
        for &k in &stuck {
            if let Some(c) = w.entities.list.iter().find(|x| x.id == rep.visits[k].cart && !x.removed)
                && c.pos.x >= 2.0
                && c.pos.x < 3.0
                && c.pos.z < 1.0
                && c.delta_movement.x > 0.0
            {
                freed.push(k);
            }
        }
        for k in freed {
            stuck.retain(|&x| x != k);
            rep.visits[k].unstuck = Some(t);
            waiting.push(k);
        }
        // (traced, wired: the circuit's parts whenever one changes, for the first 3000 gt)
        if bo.trace && bo.wired && t - t0 >= bo.trace_window.0 && t - t0 < bo.trace_window.1 {
            let probes: Vec<(&str, [i32; 3])> = vec![
                ("obs", [0, 0, 4]),
                ("S4", [-4, -3, 2]),
                ("W1", [-5, -3, 3]),
                ("W2", [-6, -3, -1]),
                ("Tt", [1, -2, 0]),
                ("T", [1, 0, 0]),
                ("Tv", [1, -2, 3]),
                ("Th", [2, -1, 2]),
                ("hatch", [2, 1, 2]),
                ("det", [3, 0, 0]),
                ("trip", [4, 0, 1]),
                ("Hout", [-4, -3, 0]),
                ("Hl", [-1, -3, 2]),
                ("Hl5", [1, -3, 5]),
                ("fTa", [0, -3, 0]),
                ("z9", [-5, -3, 9]),
                ("z9e", [2, -3, 9]),
                ("d27", [2, -2, 7]),
                ("chain", [-6, -3, 8]),
                ("Tk", [1, -2, 6]),
                ("park", [1, 0, 6]),
                // (the give-up's, when it has one)
                ("ARR", [0, 0, 0]),
                ("Aset", [0, -2, -5]),
                ("WA1", [-1, -2, -6]),
                ("WA2", [3, -2, -7]),
                ("A2", [-13, -2, -10]),
                ("G", [-15, -2, -10]),
                ("Rr", [3, -2, -9]),
                ("Gr", [4, -2, -8]),
                ("D2in", [-4, -3, -1]),
                // (the scheduler give-up's)
                ("bulb", [0, -1, -1]),
                ("F", FORCE_CELL),
                ("nV", [-3, -3, -3]),
                ("gate", [-5, -3, -3]),
                ("D2r", [-5, -3, -2]),
            ];
            let now_s: Vec<String> = probes.iter().map(|(n, c)| format!("{n}={}", short_state(&w.get(BlockPos::new(c[0], c[1], c[2])).to_string()))).collect();
            if now_s != probe_last {
                eprintln!("wire gt {}: {}", t - t0, now_s.join(" "));
                probe_last = now_s;
            }
        }
        // (traced: every rider's HOME lost or re-taken, with its state)
        if bo.trace {
            for (n, (_, v, _)) in pairs.iter().enumerate() {
                if let Some(i) = index_of(w, *v) {
                    let h = brain::brain(w, i).home.is_some();
                    if h != homes_seen[n] {
                        homes_seen[n] = h;
                        let e = &w.entities.list[i];
                        let act = brain::has_exact(w, i).then(|| {
                            let x = brain::ex(w, i);
                            format!("{:?} cant_reach {:?}", x.active, x.mem.cant_reach_since.map(|c| c - t0))
                        });
                        eprintln!("home gt {} (day {day}): villager {v} {} at {:.2} {:.2} {:.2} riding {:?} act {act:?} hostile {:?} bed occupied {}", t - t0, if h { "re-takes HOME" } else { "loses HOME" }, e.pos.x, e.pos.y, e.pos.z, e.vehicle, brain::brain(w, i).nearest_hostile, matches!(w.get(bed), Block::Bed { occupied: true, .. }));
                    }
                }
            }
        }
        // (traced: the held riders' activities every 500 gt at night)
        if bo.trace && night && t % 500 == 0 {
            let mut acts: std::collections::BTreeMap<String, u32> = Default::default();
            let mut homes = 0;
            for (_, v, _) in &pairs {
                if let Some(i) = index_of(w, *v) {
                    let a = if brain::has_exact(w, i) { format!("{:?}", brain::ex(w, i).active) } else { "-".into() };
                    *acts.entry(a).or_insert(0) += 1;
                    homes += brain::brain(w, i).home.is_some() as u32;
                }
            }
            eprintln!("gt {} (day {day}): riders {acts:?}, {homes} with HOME", t - t0);
        }
        // the waiting visits: ready (at rest on the slot with its villager), panic at night
        let mut held_out: Vec<usize> = Vec::new();
        for &k in &waiting {
            let (villager, cart) = (rep.visits[k].villager, rep.visits[k].cart);
            let Some(ci) = w.entities.list.iter().position(|x| x.id == cart && !x.removed) else { continue };
            let c = &w.entities.list[ci];
            let on_slot = c.pos.x >= s.slot_rails[0].x as f64 && c.pos.x < s.exit.x as f64 && c.pos.z < 1.0;
            let rest = c.delta_movement.x.abs() < 1e-3 && c.delta_movement.z.abs() < 1e-3;
            let with = c.passengers.contains(&villager);
            // (the release held: the cart runs over the slot without stopping, ready and released
            // as it reaches the slot with its villager)
            if bo.release_held {
                if c.pos.x >= s.slot_rails[0].x as f64 && c.pos.z < 1.0 {
                    if with {
                        rep.visits[k].ready = Some(t);
                        rep.visits[k].released = Some(t);
                    } else {
                        rep.visits[k].problem.get_or_insert("reached the slot without its villager".into());
                    }
                    held_out.push(k);
                }
                continue;
            }
            if rep.visits[k].ready.is_none() && on_slot && rest && with {
                rep.visits[k].ready = Some(t);
                rep.visits[k].rest_x = Some(c.pos.x);
                // (the rider's eye to the dorm zombie's: the sight line the panic needs)
                let zi = w.entities.list.iter().position(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Zombie) && (e.pos.x - s.zombie[0]).abs() < 1.0 && (e.pos.z - s.zombie[2]).abs() < 1.0);
                if let (Some(zi), Some(vi)) = (zi, index_of(w, villager)) {
                    let (v, z) = (&w.entities.list[vi], &w.entities.list[zi]);
                    let from = Vec3::new(v.pos.x, v.pos.y + v.kind.eye_height() as f64, v.pos.z);
                    let to = Vec3::new(z.pos.x, z.pos.y + z.kind.eye_height() as f64, z.pos.z);
                    rep.visits[k].sees = Some((redsim::entity::mob::los::has_line_of_sight(w, from, to), from.y, to.y));
                }
            }
            if let Some(rt) = rep.visits[k].ready
                && night
            {
                rep.visits[k].wait_ticks += 1;
                if let Some(i) = index_of(w, villager)
                    && brain::has_exact(w, i)
                    && brain::ex(w, i).active == Act::Panic
                {
                    rep.visits[k].wait_panic += 1;
                } else {
                    let c = rep.visits[k].calm.get_or_insert((t - rt, t - rt));
                    c.1 = t - rt;
                    if let Some(i) = index_of(w, villager) {
                        let a = if brain::has_exact(w, i) { format!("{:?} hostile {:?} riding {:?} day {day}", brain::ex(w, i).active, brain::brain(w, i).nearest_hostile, w.entities.list[i].vehicle) } else { "-".into() };
                        if rep.visits[k].calm_acts.len() < 3 && !rep.visits[k].calm_acts.contains(&a) {
                            rep.visits[k].calm_acts.push(a);
                        }
                        if bo.trace && t - rt > 30 && t % 20 == 0 && t - rt < 200 {
                            let b = brain::brain(w, i);
                            let e = &w.entities.list[i];
                            eprintln!("calm gt {} villager {villager} at {:.3} {:.3} {:.3} visible {:?} los {:?} hostile {:?} sensors {:?}", t - t0, e.pos.x, e.pos.y, e.pos.z, b.visible.entities.iter().take(4).collect::<Vec<_>>(), b.visible.los, b.nearest_hostile, b.sensors);
                        }
                    }
                }
            }
        }
        for k in held_out {
            waiting.retain(|&x| x != k);
            out.push((k, t));
        }
        // the released carts: the exit speed, then back to the hold
        let mut done: Vec<usize> = Vec::new();
        for &(k, rt) in &out {
            let cart = rep.visits[k].cart;
            let Some(c) = w.entities.list.iter().find(|x| x.id == cart && !x.removed) else { continue };
            if rep.visits[k].exit_speed.is_none() && c.pos.x >= s.exit.x as f64 + 0.5 {
                rep.visits[k].exit_speed = Some(c.delta_movement.x);
            }
            if rep.visits[k].leave_speed.is_none() && c.pos.x >= s.exit.x as f64 + 1.0 {
                rep.visits[k].leave_speed = Some(c.delta_movement.x);
                rep.visits[k].exited = Some(t);
            }
            if c.pos.x >= s.exit.x as f64 + 2.5 || t - rt > 400 {
                if c.pos.x < s.exit.x as f64 + 2.5 {
                    rep.visits[k].problem.get_or_insert(format!("not out 400 gt after its release (at {:.2} {:.2} {:.2})", c.pos.x, c.pos.y, c.pos.z));
                }
                done.push(k);
            }
        }
        for k in done {
            out.retain(|(x, _)| *x != k);
            let cart = rep.visits[k].cart;
            let h = pairs.iter().find(|p| p.0 == cart).map(|p| p.2).unwrap_or(0);
            let c = hold_cell(h);
            let p = Vec3::new(c.x as f64 + 0.5, 0.0625, c.z as f64 + 0.5);
            teleport(&mut run.world, cart, p, Vec3::new(0.0, 0.0, 0.0));
            // (after the tick to t: applied before the next one, the drive's act at t)
            drive.act(run.world.time, format!("tp {} {} {} {}", drive.name(cart), p.x, p.y, p.z));
        }
    }
    rep.errors = run.world.errors.iter().take(6).cloned().collect();
    rep.built_diff = built_diff;
    rep.build = built.as_ref().map(|b| (b.steps, b.cells, t0 - b.start));
    if bo.drive {
        drive.expect_end("golems 0".into());
        let flag = match bo.placement {
            crate::placing::Placement::Judge => String::new(),
            crate::placing::Placement::Raw => " --placed-raw".into(),
            crate::placing::Placement::Built { gap: None } => " --placed-in-order".into(),
            crate::placing::Placement::Built { gap: Some(g) } => format!(" --placed-in-order {g}"),
        };
        let start = match bo.placement {
            crate::placing::Placement::Judge => "start Run::start(spec, RunOpts { ai: true, ..RunOpts::steady(seed, days * 24000, WalkMode::Random) }); settle_tick until false; the design's entity lines spawned in order (eN: line N from 0, eN.pK: its passenger K from 0); t0 = the world's time then",
            crate::placing::Placement::Raw => "start Run::start(spec, RunOpts { ai: true, raw: true, ..RunOpts::steady(seed, days * 24000, WalkMode::Random) }) (the file pasted as written, no settle); the design's entity lines spawned in order (eN: line N from 0, eN.pK: its passenger K from 0); t0 = the world's time then",
            crate::placing::Placement::Built { .. } => "start Run::start(spec with no blocks, RunOpts { ai: true, ..RunOpts::steady(seed, days * 24000, WalkMode::Random) }), the build (the `build:` line), then the design's entity lines spawned in order (eN: line N from 0, eN.pK: its passenger K from 0); t0 = the world's time then",
        };
        let mut header = vec![
            format!("drive v1: mctas mobfarm siding-bench <design> --wired --launch --from {s} --to {s} --nights {}{flag} --drive-out F (arrivals launched from rest on the bench's launcher; the release placed as a block: the loop's input)", bo.nights, s = bo.seed),
            // (mobviewer's badge line, 2026-11-10: the view shows it first while the run plays)
            "caption carts launched in and released by the test bench; the siding's own redstone does the rest".into(),
            format!("seed {}", bo.seed),
            format!("days {}", bo.nights + 1),
            "walk random".into(),
            "ai on".into(),
            start.into(),
            if built.is_some() { "act t: applied when the world's time is the build's start + t, before the run's next tick (the bench's own order)".into() } else { "act t: applied when the world's time is t0 + t, before the run's next tick (the bench's own order)".into() },
            format!("t0 {}", t0),
        ];
        header.extend(crate::placing::header(bo.placement, built.as_ref(), t0, &bo.plan));
        rep.drive = Some(drive.text(&header));
    }
    Ok(rep)
}
