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
    layout_at(0, 0)
}

/// The bench's frame (and the check's): the siding's local coordinates (as [`layout`] places it) to
/// the world's: turned `turn` quarter turns about the local origin (`clock::rotate_pos`: one turn
/// takes (x, z) to (-z, x)), then shifted (dx, dz). Block cells turn as cells, points as points
/// (a point in a cell lands in the turned cell: one turn takes the point (u, w) to (1 - w, u)),
/// as the generator's `turn_design` writes them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    pub turn: i32,
    pub dx: i32,
    pub dz: i32,
}

impl Frame {
    fn t(&self) -> i32 {
        self.turn.rem_euclid(4)
    }

    pub fn blk(&self, x: i32, y: i32, z: i32) -> BlockPos {
        let (a, b) = match self.t() {
            1 => (-z, x),
            2 => (-x, -z),
            3 => (z, -x),
            _ => (x, z),
        };
        BlockPos::new(a + self.dx, y, b + self.dz)
    }

    pub fn cell(&self, c: BlockPos) -> BlockPos {
        self.blk(c.x, c.y, c.z)
    }

    /// A world cell in local coordinates (the inverse of `blk`).
    pub fn local_cell(&self, c: BlockPos) -> BlockPos {
        let (a, b) = (c.x - self.dx, c.z - self.dz);
        let (x, z) = match self.t() {
            1 => (b, -a),
            2 => (-a, -b),
            3 => (-b, a),
            _ => (a, b),
        };
        BlockPos::new(x, c.y, z)
    }

    pub fn pt(&self, x: f64, y: f64, z: f64) -> Vec3 {
        let (a, b) = match self.t() {
            1 => (1.0 - z, x),
            2 => (1.0 - x, 1.0 - z),
            3 => (z, 1.0 - x),
            _ => (x, z),
        };
        Vec3::new(a + self.dx as f64, y, b + self.dz as f64)
    }

    /// A local velocity (or offset) in world coordinates.
    pub fn vel(&self, v: Vec3) -> Vec3 {
        match self.t() {
            1 => Vec3::new(-v.z, v.y, v.x),
            2 => Vec3::new(-v.x, v.y, -v.z),
            3 => Vec3::new(v.z, v.y, -v.x),
            _ => v,
        }
    }

    /// A world velocity in local coordinates (the inverse of `vel`).
    pub fn lvel(&self, v: Vec3) -> Vec3 {
        match self.t() {
            1 => Vec3::new(v.z, v.y, -v.x),
            2 => Vec3::new(-v.x, v.y, -v.z),
            3 => Vec3::new(-v.z, v.y, v.x),
            _ => v,
        }
    }

    /// A world point in local coordinates (the inverse of `pt`).
    pub fn local(&self, p: Vec3) -> Vec3 {
        let (a, b) = (p.x - self.dx as f64, p.z - self.dz as f64);
        let (x, z) = match self.t() {
            1 => (b, 1.0 - a),
            2 => (1.0 - a, 1.0 - b),
            3 => (1.0 - b, a),
            _ => (a, b),
        };
        Vec3::new(x, p.y, z)
    }

    /// A local box (min and max corners) in world coordinates.
    pub fn bb(&self, lo: [f64; 3], hi: [f64; 3]) -> redsim::entity::Aabb {
        let (a, b) = (self.pt(lo[0], lo[1], lo[2]), self.pt(hi[0], hi[1], hi[2]));
        redsim::entity::Aabb { min_x: a.x.min(b.x), min_y: lo[1], min_z: a.z.min(b.z), max_x: a.x.max(b.x), max_y: hi[1], max_z: a.z.max(b.z) }
    }

    /// The T's two shapes in this frame: (the bed line to the exit, the entry to the bed line).
    pub fn tee_shapes(&self) -> (String, String) {
        let s = |sh: &str| clock::canon::rotate_state(&format!("minecraft:rail[shape={sh}]"), self.t() as u8);
        let get = |st: String| st.split("shape=").nth(1).map(|x| x.trim_end_matches(']').split(',').next().unwrap_or("").to_string()).unwrap_or_default();
        (get(s("south_east")), get(s("south_west")))
    }
}

/// [`layout`] in a frame (turned and shifted).
pub fn layout_fr(fr: &Frame) -> Siding {
    let l = layout();
    let z = fr.pt(l.zombie[0], l.zombie[1], l.zombie[2]);
    Siding {
        entry: fr.cell(l.entry),
        tee: fr.cell(l.tee),
        tee_ctrl: fr.cell(l.tee_ctrl),
        boost: fr.cell(l.boost),
        free_cell: fr.cell(l.free_cell),
        activator_rail: fr.cell(l.activator_rail),
        activator: fr.cell(l.activator),
        park_rail: fr.cell(l.park_rail),
        park: fr.cell(l.park),
        stop: fr.cell(l.stop),
        bed_head: fr.cell(l.bed_head),
        bed_foot: fr.cell(l.bed_foot),
        hatch: fr.cell(l.hatch),
        hatch_ctrl: fr.cell(l.hatch_ctrl),
        zombie: [z.x, z.y, z.z],
        slot_rails: [fr.cell(l.slot_rails[0]), fr.cell(l.slot_rails[1])],
        release: fr.cell(l.release),
        exit: fr.cell(l.exit),
    }
}

/// (design (b), emulation: MCTAS_SIDING_PARK=7) The park one block further south: the park rail at
/// (1,0,7), the stop at (1,0,8), K and BK at z 7, so the empty cart meets a napper on the free cell
/// under the pickup's 0.2 b/t (a cart faster than 0.2 can jump the pickup band and stop dead
/// against the villager: the dropped-F jam).
pub fn park7() -> bool {
    std::env::var("MCTAS_SIDING_PARK").is_ok_and(|v| v == "7")
}

/// Design (b)'s self-retrying kick (MCTAS_SIDING_RETRY=1, ironspec 2026-11-11 08:40): the bed line
/// past the free cell one block down. The activator a slope rising north to the free cell
/// (1,-1,3), a plain rail (1,-1,4), the park rail (1,-1,5) against K's powered block (1,-1,6), now
/// the stop. A kicked cart that meets the napper inside its boarding cooldown stops on the slope,
/// rolls back down onto the powered park rail and into the stop, and is kicked again while K holds:
/// a level-32 freeze (redstone running, entities frozen) can no longer leave it stalled for good.
pub fn retry() -> bool {
    std::env::var("MCTAS_SIDING_RETRY").is_ok_and(|v| v == "1")
}

/// The free-cell sensor (MCTAS_SIDING_FC=1, on the retry build): see `wire_b`.
pub fn fc() -> bool {
    retry() && std::env::var("MCTAS_SIDING_FC").is_ok_and(|v| v == "1")
}

/// [`layout`] moved `dx` east and `dz` south (the bench's world shifted so the loop's chunk lines
/// fall on the siding as day-1 pastes it: `BenchOpts::shift_x`, `shift_z`).
pub fn layout_at(dx: i32, dz: i32) -> Siding {
    let p = |x: i32, y: i32, z: i32| BlockPos::new(x + dx, y, z + dz);
    let pz = if park7() { 7 } else { 6 };
    let r = retry();
    Siding {
        entry: p(0, 0, 0),
        tee: p(1, 0, 0),
        tee_ctrl: p(1, -1, 0),
        boost: p(1, 0, 1),
        free_cell: p(1, 0, 2),
        activator_rail: if r { p(1, -1, 3) } else { p(1, 0, 3) },
        activator: if r { p(1, -2, 3) } else { p(1, -1, 3) },
        park_rail: if r { p(1, -1, 5) } else { p(1, 0, pz) },
        park: p(1, -1, pz),
        stop: if r { p(1, -1, 6) } else { p(1, 0, pz + 1) },
        bed_head: p(0, 0, 2),
        bed_foot: p(0, 0, 3),
        hatch: p(2, 1, 2),
        hatch_ctrl: p(2, 0, 2),
        zombie: [4.5 + dx as f64, 1.0, 2.5 + dz as f64],
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
    if retry() {
        // (the self-retrying kick: the activator a slope down south from the free cell, a plain
        // rail and the park rail one block down, the stop the park's block (K's, wired); glass over
        // the low rails at y 0 and a top-half iron trapdoor over the slope (the cart passes under
        // it at 0.7, a villager's 1.95 does not fit): no villager stands in the low line)
        put(b, at(s.activator_rail), "activator_rail[powered=true,shape=ascending_north,waterlogged=false]")?;
        put(b, at(s.activator), "redstone_block")?;
        put(b, [1, -1, 4], &rail("north_south"))?;
        put(b, [1, -2, 4], "glass")?;
        put(b, at(s.park_rail), "powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
        put(b, [1, -2, 5], "glass")?;
        put(b, at(s.park), "glass")?;
        put(b, [1, 0, 3], "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")?;
        // (a top trapdoor over the first low rail too: the cart's front comes under (1,0,4) still
        // 0.2 up on the slope, where full glass stopped it)
        put(b, [1, 0, 4], "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")?;
        put(b, [1, 0, 5], "glass")?;
        put(b, [1, 0, 6], "glass")?;
    } else {
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
    }
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
    // (MCTAS_SIDING_FRONT=0: the bed head's north front closed, two high: the free cell the only
    // place an awake napper can stand in the cage (the glass over the bed leaves its top 1.44
    // high), so "the napper awake in the cage" is "the napper on the free cell")
    if std::env::var("MCTAS_SIDING_FRONT").is_ok_and(|v| v == "0") {
        walls.extend([[fx - 1, 0, fz - 1], [fx - 1, 1, fz - 1]]);
    }
    walls.extend([[fx, 1, fz - 1], [fx, 1, fz + 1], [fx, 1, fz + 2], [fx, 1, fz + 3]]);
    walls.extend([[fx + 1, 2, fz - 1], [fx + 1, 2, fz + 1], [fx + 1, 2, fz], [fx + 1, 2, fz + 2]]);
    // (the glass over the bed; MCTAS_SIDING_BEDTOP=1 lifts it one block (tick-exact 03:14, for
    // ironspec's e5): the bed's top standable (1.95 under the glass), so a calm rider with a
    // land path to the bed sets no CANT_REACH)
    let bed_roof = if std::env::var("MCTAS_SIDING_BEDTOP").is_ok_and(|v| v == "1") { 3 } else { 2 };
    walls.extend([[fx - 1, bed_roof, fz + 1], [fx - 1, bed_roof, fz]]);
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
    if b.d.meta.get("turn").is_some_and(|t| t == "2" || t == "3") {
        // (the half turn, day-1's (c) 2026-11-11: a plain rail at a T picks its curve by a fixed
        // direction order (RailState.place: the last matching curve wins), so turned 180 degrees
        // the T's polarity inverts: powered is the bed line to the exit, unpowered the entry to
        // the bed line. So the turned copy's T is powered during a visit only: H's repeater still
        // powers Ta's block (1,-3,0); a wall torch W on its south face is not-H, it powers the block
        // (1,-2,1) above it; a wall torch Ta' on that block's north face, under the T's support, is
        // H and powers the support (+2 blocks))
        put(b, [1, -3, 0], "stone")?;
        put(b, [1, -3, 1], "redstone_wall_torch[facing=south,lit=true]")?;
        put(b, [1, -2, 1], "stone")?;
        put(b, [1, -2, 0], "redstone_wall_torch[facing=north,lit=false]")?;
    } else {
        on(b, [1, -2, 0], torch)?; // Ta (1,-3,0) under it
    }
    // (the self-retrying kick: the activator's control is the slope's support (1,-2,3), Aa one
    // lower at (1,-3,3) on (1,-4,3), its repeater (0,-4,3) reading the H line's support (-1,-4,3))
    if retry() {
        on(b, [1, -3, 3], torch)?;
    } else {
        on(b, [1, -2, 3], torch)?; // Aa (1,-3,3)
    }
    on(b, [2, -1, 2], torch)?; // Ha (2,-2,2)
    // their feeds: repeaters into Ta and Aa from the H line at x -1, into Ha from the south
    on(b, [0, -3, 0], &rep("west", 1))?;
    if retry() {
        on(b, [0, -4, 3], &rep("west", 1))?;
    } else {
        on(b, [0, -3, 3], &rep("west", 1))?;
    }
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
    // (the free-cell sensor FC: no S observer or S line; H is set by F alone, day-1's pulse on
    // every visit, so a bed change with no visit (a stranger leaving the bed) sets nothing)
    if !fc() {
    swap(b, [0, 0, 4], "observer[facing=north,powered=false]")?;
    }
    // (the forced give-up's speed, 2026-11-10: delay moved from the kick chain (H to the kick,
    // which the scheduler's F also waits through) to the nap path (S to H, which F skips); the
    // nap's onset to its kick stays the same. MCTAS_SIDING_MOVE = repeaters moved, 0..3; the
    // free cell a booster with MCTAS_SIDING_BOOST=1)
    let mv: usize = std::env::var("MCTAS_SIDING_MOVE").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    // (MCTAS_SIDING_SLONG=1: the S line longer, four 4-tick repeaters: south from the observer
    // through (0,0,5), (0,0,6), west along z 7 through (-1,0,7), (-2,0,7), north up x -3 to the
    // stair: 16 gt more on the nap path, so the kick chain can shrink to dust and F, which skips
    // the nap path, kicks sooner)
    let slong = !std::env::var("MCTAS_SIDING_SLONG").is_ok_and(|v| v == "0");
    if slong {
        on(b, [0, 0, 5], &rep("north", 4))?;
        on(b, [0, 0, 6], &rep("north", 4))?;
        on(b, [0, 0, 7], dust)?;
        on(b, [-1, 0, 7], &rep("east", 4))?;
        on(b, [-2, 0, 7], &rep("east", 4))?;
        for z in [7, 6, 5] {
            on(b, [-3, 0, z], dust)?;
        }
    } else if !fc() {
        // (the self-retrying kick: the S dust on glass, not stone: dust powers the block under it,
        // and a powered (0,-1,5) powers the low park rail beside it at every S pulse)
        if retry() {
            put(b, [0, -1, 5], "glass")?;
            put(b, [0, 0, 5], dust)?;
        } else {
            on(b, [0, 0, 5], dust)?;
        }
        for (i, x) in [-1, -2, -3].into_iter().enumerate() {
            on(b, [x, 0, 5], &if i < mv.min(2) { rep("east", 4) } else { dust.to_string() })?;
        }
    }
    if !fc() {
        on(b, [-3, -1, 4], dust)?;
        on(b, [-3, -2, 3], dust)?;
        on(b, [-3, -3, 2], dust)?;
        on(b, [-4, -3, 2], &rep("east", 4))?;
    }
    // R: the exit branch's detector rail (its second rail, two from the T), repeaters north, dust
    // west along z -3, down west into a repeater south into N2
    swap(b, [3, 0, 0], "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    // (MCTAS_SIDING_RDELAY=d: the two R repeaters at delay d (default 1): R resets H later, so a
    // wake pulse's echo down a longer S line reaches N1 before the reset, not after it)
    let rd: u8 = std::env::var("MCTAS_SIDING_RDELAY").ok().and_then(|v| v.parse().ok()).unwrap_or(if slong { 4 } else { 1 });
    on(b, [3, 0, -1], &rep("south", rd))?;
    on(b, [3, 0, -2], &rep("south", rd))?;
    // (MCTAS_SIDING_REXTRA=n: n of the R line's dust at (2,0,-3), (1,0,-3) replaced by 4-tick
    // repeaters: R resets H up to 16 gt later, so the wake's echo down a longer nap path reaches N1
    // while H is still set)
    let rx: usize = std::env::var("MCTAS_SIDING_REXTRA").ok().and_then(|v| v.parse().ok()).unwrap_or(if slong { 2 } else { 0 });
    for x in -4..=3 {
        let r = (x == 2 && rx >= 1) || (x == 1 && rx >= 2);
        on(b, [x, 0, -3], &if r { rep("east", 4) } else { dust.to_string() })?;
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
    let kz = s.park.z;
    on(b, [2, -2, kz], "stone")?; // BK
    put(b, [1, -2, kz], "redstone_wall_torch[facing=west,lit=false]")?;
    b.set(1, -3, kz, "stone");
    // not H, delayed: off D1 south along x -6 through five 4-tick repeaters, east along z 9,
    // up north into BK
    // (the default: with the long S line (+16 gt on the nap path) the chain [2,1,0,0,0] (a 2-tick
    // and a 1-tick repeater, three dust): F's forced exit 55-58 gt, a nap's drop to re-board min 69
    // against the 60-gt boarding cooldown; the kick at least ~10 gt after H, since the villager
    // wakes up to ~23 gt after the hatch shuts at H and must be up before the cart passes the free
    // cell. Without it (MCTAS_SIDING_SLONG=0): [4,2,1,1,1], 67-70 gt, re-board min 65)
    let chain_delays: Vec<u8> = std::env::var("MCTAS_SIDING_CHAIN").ok().map(|v| v.split(',').filter_map(|d| d.trim().parse().ok()).collect()).unwrap_or_else(|| if slong { vec![2, 1, 0, 0, 0] } else if mv == 3 { vec![4, 2, 1, 1, 1] } else { Vec::new() });
    for z in 4..=8 {
        // (the moved delay: the last `mv` chain repeaters at 1 tick; MCTAS_SIDING_CHAIN="a,b,c,d,e"
        // sets all five)
        let d = chain_delays.get((z - 4) as usize).copied().unwrap_or(if (z as usize) > 8 - mv.min(3) { 1 } else { 4 });
        // (MCTAS_SIDING_CHAIN 0 = dust in place of the repeater: the chain's signal holds to BK
        // through the one repeater at (-2,-3,9))
        if d == 0 {
            on(b, [-6, -3, z], dust)?;
            continue;
        }
        on(b, [-6, -3, z], &rep("north", d))?;
    }
    if !std::env::var("MCTAS_SIDING_BOOST").is_ok_and(|v| v == "0") {
        swap(b, at(s.free_cell), "powered_rail[powered=true,shape=north_south,waterlogged=false]")?;
        // (the self-retrying kick: no redstone block under the free cell, which would power the low
        // activator beside it at all times; the booster north of it powers it along the line)
        if !retry() {
            swap(b, [s.free_cell.x, -1, s.free_cell.z], "redstone_block")?;
        }
    }
    for x in -6..=1 {
        on(b, [x, -3, 9], if x == -2 { "repeater[delay=1,facing=west,locked=false,powered=false]" } else { dust })?;
    }
    on(b, [2, -3, 9], dust)?;
    on(b, [2, -2, 8], dust)?;
    if kz == 6 {
        on(b, [2, -2, 7], dust)?;
    }
    // L into the same line: a repeater east of the south hook reading it, dust east then south
    // along x 6, down, west along z 8 into (2,-2,8)
    // (MCTAS_SIDING_PARK=7: BK at (2,-2,7) takes L from the east, (4,-2,8) north to (4,-2,7) and a
    // repeater at (3,-2,7) into BK, so the chain's dust at (2,-2,8) has one link and points north
    // into BK)
    on(b, [5, 0, 1], &rep("west", 1))?;
    for z in 1..=6 {
        on(b, [6, 0, z], dust)?;
    }
    on(b, [6, -1, 7], dust)?;
    on(b, [6, -2, 8], dust)?;
    if kz == 6 {
        for x in 3..=5 {
            on(b, [x, -2, 8], dust)?;
        }
    } else {
        for x in 4..=5 {
            on(b, [x, -2, 8], dust)?;
        }
        on(b, [4, -2, 7], dust)?;
        on(b, [3, -2, 7], &rep("east", 1))?;
    }
    // (design (b), the napper-aboard sensing, measured first (ironspec 2026-11-10 23:25):
    // MCTAS_SIDING_SEAT=1 strings a tripwire at (1,2,1), over the booster north of the free cell,
    // between hooks at (1,2,0) and (1,2,2) on glass at (1,2,-1) and (1,2,3). A seated villager's
    // box (its feet on the cart's seat, about 0.25 up) tops out near 2.2 and crosses the string's
    // band (2.06-2.16); a villager standing on a rail tops out at 1.95, no villager can stand on
    // the booster (the glass at (1,1,1)), and an empty cart reaches 0.76)
    if std::env::var("MCTAS_SIDING_SEAT").is_ok_and(|v| v == "1") {
        put(b, [1, 2, -1], "glass")?;
        put(b, [1, 2, 0], "tripwire_hook[attached=true,facing=south,powered=false]")?;
        put(b, [1, 2, 1], "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?;
        put(b, [1, 2, 2], "tripwire_hook[attached=true,facing=north,powered=false]")?;
        put(b, [1, 2, 3], "glass")?;
    }
    // (MCTAS_SIDING_SEAT=2: a stone pressure plate at (1,2,1), on the booster's glass ceiling
    // (1,1,1): its touch box, y 2.0-2.25, meets a seated villager's top (2.0125 on a flat rail)
    // and not a standing one's (1.95); no villager can stand on the booster under the glass)
    if std::env::var("MCTAS_SIDING_SEAT").is_ok_and(|v| v == "2") {
        put(b, [1, 2, 1], "stone_pressure_plate[powered=false]")?;
    }
    // glass caps on the circuit's solid tops a golem could stand on (N1, N2, BK, the hook's stone)
    for c in [[-5, -2, 2], [-7, -2, -1], [2, -1, kz], [4, 1, -2]] {
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
    b.d.meta.insert("build".into(), crate::placing::BuildPlan { last: vec![BlockPos::new(-6, -3, -1), BlockPos::new(1, -2, kz)], fills_last: false }.meta());
    if std::env::var("MCTAS_SIDING_B").is_ok_and(|v| v == "1") {
        ensure!(!slong && kz == 6, "design (b) wired: the short nap path (MCTAS_SIDING_SLONG=0), park at z 6");
        wire_b(b)?;
    }
    Ok(())
}

/// Design (b), wired (MCTAS_SIDING_B=1, on the short nap path), step 1: B and the after-wake kick.
/// - B: a waxed copper bulb under the S stair's foot (-3,-3,3), toggled by every S pulse (the
///   bed observer pulses at the sleep and at the wake); its comparator (-3,-3,4) reads it.
/// - The delay chain now carries not-X = not-H or B: B joins it at (-6,-3,5) through a repeater
///   (-5,-3,5), and the chain is 4-tick repeaters all along (about 44 gt): X = H and not-B rises at
///   the wake, the kick comes about 44 gt after it.
/// - B now: a second bulb under the S dust (0,-1,5) (the same pulses), its comparator (0,-1,6),
///   a repeater (0,-1,7) and dust (0,-1,8) whose block powers a repeater (1,-2,8) into the
///   chain's end: BK = not-X delayed or B now or L, so K = X delayed and not-B and not-L.
fn wire_b(b: &mut Builder) -> Result<()> {
    let rep = |f: &str, d: u8| format!("repeater[delay={d},facing={f},locked=false,powered=false]");
    let dust = "redstone_wire[east=none,north=none,power=0,south=none,west=none]";
    let bulb = "waxed_copper_bulb[lit=false,powered=false]";
    let cmp = |f: &str| format!("comparator[facing={f},mode=compare,powered=false]");
    let sub = |f: &str| format!("comparator[facing={f},mode=subtract,powered=false]");
    // (a block under, unless one is there; then the part, on an empty cell)
    let on = |b: &mut Builder, c: [i32; 3], part: &str| -> Result<()> {
        let below = [c[0], c[1] - 1, c[2]];
        if !b.d.blocks.contains_key(&below) {
            b.set(below[0], below[1], below[2], "stone");
        }
        put(b, c, part)
    };
    // B, a bulb west of the bed head: a second observer replaces the cage's west wall glass at
    // (-1,0,2), watching the bed head as the first watches the foot, its pulses into the bulb
    // (-2,0,2), glass over it (a golem spot otherwise). Not on the S stair: a copper bulb is no
    // conductor and would cut the stair's climb, and the parts keep clear of the stair's cut cells
    // (-3,0,4), (-3,-1,3), (-3,-2,2). Its comparator (-3,0,2) puts B out at (-4,0,2), west and
    // south round the S stair (dust on stone, never beside a stair cell: a stair dust with stone
    // beside it climbs to any dust on top) down to (-4,-3,6), into the chain through a repeater
    // (-5,-3,6) at its merge cell (-6,-3,6), after two repeaters: the chain carries not-H or B,
    // and B stays out of D1 (D1 also powers N2, the H latch's reset: B there would hold H off
    // through every nap). (The first route went down onto N1's glass cap: dust on glass carries
    // power up the stair and not down, so B never reached the chain.)
    // (the free-cell sensor FC replaces B: no observer, bulb or comparator; B's downstream network
    // from (-4,-3,6) on carries not-FC instead, fed at (0,-1,8) below)
    if !fc() {
        swap(b, [-1, 0, 2], "observer[facing=east,powered=false]")?;
        on(b, [-2, 0, 2], bulb)?;
        put(b, [-2, 1, 2], "glass")?;
        on(b, [-3, 0, 2], &cmp("east"))?;
        for c in [[-4, 0, 2], [-5, 0, 2], [-5, 0, 3], [-5, -1, 4], [-5, -2, 5], [-4, -2, 5]] {
            on(b, c, dust)?;
        }
    }
    on(b, [-4, -3, 6], dust)?;
    on(b, [-5, -3, 6], &rep("east", 1))?;
    // the chain: 8-gt repeaters, (-6,-3,6) the merge cell (dust)
    for z in [4, 5, 7, 8] {
        swap(b, [-6, -3, z], &rep("north", 4))?;
    }
    swap(b, [-6, -3, 6], dust)?;
    // (repeater delays count redstone ticks: 4 = 8 gt. After the merge: two 8-gt repeaters down
    // x -6, five on z 9, one 2-gt one: about 60 gt from B's fall (the wake) to the chain's end,
    // design (b)'s D = 60 as emulated)
    for x in -5..=0 {
        swap(b, [x, -3, 9], &rep("west", if x <= -1 { 4 } else { 1 }))?;
    }
    // B now: B itself (one bulb, one resync), into the chain's end through (1,-2,8): B's line
    // branches at its stair's foot (-4,-3,6) down to the N line (-4,-4,7), (-4,-5,8), which carries
    // it both ways: down to N's repeater and up the N stair to (0,-1,8), whose block feeds the
    // repeater (1,-2,8) into the chain's end. (A second bulb under the S dust had no free face for
    // its resync gate.)
    on(b, [0, -1, 8], dust)?;
    on(b, [1, -2, 8], &rep("west", 1))?;
    // (the two-torch tower's top block, under nothing now: FAST's block for SC's side repeater)
    put(b, [0, -2, 7], "stone")?;
    if fc() {
        // (not-FC: a wall torch (1,1,7) on FC's block (1,1,6), dust (1,0,7), (0,0,7) on stone, down
        // south onto (0,-1,8): the whole network once fed B now carries not-FC)
        put(b, [0, -1, 7], "stone")?;
        put(b, [1, 1, 7], "redstone_wall_torch[facing=south,lit=true]")?;
        on(b, [1, 0, 7], dust)?;
        put(b, [0, 0, 7], dust)?;
        // (K holds, ironspec 08:40 item 1: K fed back into SC's side. From the stop (1,-1,6), K's
        // strongly powered block: dust west (0,-1,6), (-1,-1,6), (-2,-1,6), south (-2,-1,7) on the
        // block (-2,-2,7), read by a repeater (-1,-2,7) east into FAST's block (0,-2,7): once lit, K
        // stays lit until not-H (R) or L. Stone (0,0,6) keeps (0,-1,6) off not-FC's (0,0,7))
        for c in [[0, -1, 6], [-1, -1, 6], [-2, -1, 6], [-2, -1, 7]] {
            on(b, c, dust)?;
        }
        on(b, [-1, -2, 7], &rep("west", 1))?;
        put(b, [0, 0, 6], "stone")?;
        put(b, [0, 1, 6], "glass")?;
    } else {
        put(b, [0, -1, 7], "glass")?;
    }
    on(b, [-4, -4, 7], dust)?;
    if !fc() {
    // the dawn/dusk resync (ironspec 05:47, day-1's pulse from its daylight stair, 05:53): a pulse
    // into P (-4,1,4) turns off the wall torch (-4,1,3) on it (not-RS), whose repeater (-4,1,2)
    // feeds the side of G (-4,1,1), a subtract comparator with B at its back (dust up from B's
    // line at (-5,0,2) to (-5,1,1)): G = B and RS, east into dust (-3,1,1), (-2,1,1) and onto the
    // bulb (-2,1,2): a lit bulb at a resync toggles off, an unlit one ignores it. CB's output
    // block (-4,0,2) is stone (strongly powered: B's dust beside it), the repeater on it.
    swap(b, [-4, 0, 2], "stone")?;
    on(b, [-5, 1, 1], dust)?;
    on(b, [-4, 1, 1], &sub("west"))?;
    on(b, [-3, 1, 1], dust)?;
    on(b, [-2, 1, 1], dust)?;
    swap(b, [-2, 1, 2], dust)?;
    put(b, [-4, 1, 2], &rep("south", 1))?;
    put(b, [-4, 1, 4], "stone")?;
    put(b, [-4, 1, 3], "redstone_wall_torch[facing=north,lit=true]")?;
    put(b, [-4, 2, 4], "glass")?;
    b.d.meta.insert("resync".into(), "-4 1 4".into());
    }
    // step 2's FAST unit is described above; the old comment block follows for the record
    // (superseded: N fed B now from a second bulb, FA = H - N at (-1,-6,7))
    // - L into BK from the east (its dust no longer joins the chain's end).
    // step 2, the fast path (an F visit, no nap: the kick on F itself), a unit at y -6:
    // - FA (0,-6,6), a subtract comparator facing north: back F, side N, out south into the
    //   two-torch tower's block (0,-6,7) -> (0,-5,7), (0,-3,7) -> (0,-2,7), SC's side repeater.
    //   FAST = F - N (ironspec 05:47 item 2's B faults: with FAST = H - N, a missed sleep pulse
    //   left N off and kicked at H's rise, inside the boarding cooldown: the cart stalled against
    //   the standing napper for good). F comes 150-170 gt after the arrival, long past the
    //   cooldown, and K still needs H (BK has not-H).
    // - N, "a nap this visit": a repeater (1,-6,6) facing east into FA's side, fed B at its back
    //   (2,-6,6) and locked by H through the repeater (1,-6,5) into its side: H rises while the
    //   napper sleeps (about 26 gt after the onset), so a napped visit locks N on, an F visit off;
    //   H falling at the return unlocks it. (Not a dropper latch: droppers are conductors.)
    // - F: tapped beside its input cell by two repeaters (-4,-3,-4), (-3,-3,-4), down to y -5,
    //   south down x -3 (a repeater at z 0), east along z 5 at y -6 into FA's back (0,-6,5).
    // - H: off the H line at (-1,-3,1) down east (0,-4,1), (1,-5,1), (1,-6,2), south to the lock.
    // - B: the B line's N branch (-4,-4,7), (-4,-5,8), (-4,-6,9), east along z 9 at y -6
    //   (repeaters at x -3 and 1), north up x 2 to N's back (2,-6,6); up the stair from (-4,-5,8)
    //   to (0,-1,8) it is B now at the chain's end.
    // - SC (2,-2,7), a subtract comparator before BK: back the chain's end (not-X delayed or B
    //   now), side FAST through (1,-2,7): BK = SC or L or not-H, so K = (X delayed and not-B now,
    //   or FAST) and not-L and H.
    on(b, [-1, -2, 8], dust)?;
    on(b, [-2, -3, 8], dust)?;
    on(b, [-3, -4, 8], dust)?;
    on(b, [-4, -5, 8], dust)?;
    on(b, [-4, -6, 9], dust)?;
    for x in -3..=2 {
        on(b, [x, -6, 9], &if x == -3 || x == 1 { rep("west", 1) } else { dust.to_string() })?;
    }
    on(b, [2, -6, 8], dust)?;
    on(b, [2, -6, 7], dust)?;
    on(b, [2, -6, 6], dust)?;
    on(b, [1, -6, 6], &rep("east", 1))?;
    on(b, [0, -6, 6], &sub("north"))?;
    on(b, [1, -6, 5], &rep("north", 1))?;
    on(b, [0, -4, 1], dust)?;
    on(b, [1, -5, 1], dust)?;
    for z in 2..=4 {
        on(b, [1, -6, z], dust)?;
    }
    // (F's line keeps to z >= -4, the force cell's own row: with any block at z -5 or beyond,
    // the bench's riders came to the siding with CANT_REACH stamps of ~19,000 gt and released
    // HOME on the approach, 30-40 HOME changes over 16 seeds against 0, a single stone anywhere
    // on z -5 enough (y -4 or -8, x -10 or 3) and a world shift not: a bench effect of the
    // design's extent, under investigation)
    on(b, [-4, -3, -4], &rep("west", 1))?;
    on(b, [-3, -3, -4], &rep("west", 1))?;
    on(b, [-2, -3, -4], dust)?;
    on(b, [-2, -4, -3], dust)?;
    on(b, [-2, -5, -2], dust)?;
    for z in -2..=4 {
        on(b, [-3, -5, z], &if z == 0 { rep("north", 1) } else { dust.to_string() })?;
    }
    for x in -3..=0 {
        on(b, [x, -6, 5], dust)?;
    }
    // (B now into the torch tower's top block (0,-4,7): FAST = F - N held off while the napper
    // sleeps now. N locks B at H's rise, and when F lands on the sleep's own tick, H rises from
    // F before B reaches N: N locked off and FAST kicked a sleeping napper)
    for x in -3..=-2 {
        on(b, [x, -4, 7], dust)?;
    }
    // (a repeater into the block, not dust: T_low powers that block strongly, which would feed
    // dust beside it)
    on(b, [-1, -4, 7], &rep("west", 1))?;
    put(b, [0, -6, 7], "stone")?;
    put(b, [0, -5, 7], "redstone_torch[lit=true]")?;
    put(b, [0, -4, 7], "stone")?;
    put(b, [0, -3, 7], "redstone_torch[lit=false]")?;
    swap(b, [2, -2, 7], &sub("south"))?;
    on(b, [1, -2, 7], &rep("west", 1))?;
    swap(b, [3, -2, 8], "glass")?;
    on(b, [4, -2, 7], dust)?;
    on(b, [4, -2, 6], dust)?;
    on(b, [3, -2, 6], &rep("east", 1))?;
    // (SC and the L repeater power BK strongly, and a strongly powered block powers the dust
    // beside it: the H line's step (2,-2,5) north of BK would take BK's power into H, and dust at
    // (1,-3,4) would take Aa's block's (the wire's own note). The H line's end goes round the
    // east instead: (1,-3,5), (2,-3,5), (3,-3,5), up north to (3,-2,4), west to (2,-2,4); (2,-2,5)
    // is glass)
    swap(b, [2, -2, 5], "glass")?;
    swap(b, [2, -3, 5], dust)?;
    b.set(2, -4, 5, "stone");
    on(b, [3, -3, 5], dust)?;
    on(b, [3, -2, 4], dust)?;
    // (not-H into BK: the chain's 44 gt also delay H's fall, so after each return K would light
    // for 44 gt with H off and the next arrival meet a powered park (it bounces, stops short of
    // the stop and is kicked too slow to take its napper). Ha's strongly powered hatch control
    // (2,0,2) feeds dust east and south into the L line at (6,0,4): BK = SC or L or not-H)
    for c in [[3, 0, 2], [3, 0, 3], [3, 0, 4], [4, 0, 4], [5, 0, 4]] {
        on(b, c, dust)?;
    }
    // (the free-cell sensor FC, MCTAS_SIDING_FC=1 on the retry build, ironspec 09:19: a tripwire
    // string at y 1 over the T, the booster and the free cell, every cell the napper can stand on,
    // between hooks at (1,1,-1) (on glass at (1,1,-2)) and (1,1,5) (on stone at (1,1,6), FC's block);
    // the booster's cap gone, so glass caps over the T's sides (0,1,0), (2,1,0), glass at its north
    // (1,0,-1), and glass at y 2 over the trapdoors. A standing villager's box crosses the band at
    // 1.06-1.16; a baby (0.98) and the cart (0.76) do not)
    if fc() {
        swap(b, [1, 1, 1], "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?;
        for z in [0, 2, 3, 4] {
            if b.d.blocks.contains_key(&[1, 1, z]) {
                swap(b, [1, 1, z], "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?;
            } else {
                put(b, [1, 1, z], "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?;
            }
        }
        put(b, [1, 1, -1], "tripwire_hook[attached=true,facing=south,powered=false]")?;
        put(b, [1, 1, -2], "glass")?;
        swap(b, [1, 1, 5], "tripwire_hook[attached=true,facing=north,powered=false]")?;
        put(b, [1, 1, 6], "stone")?;
        for c in [[0, 1, 0], [2, 1, 0], [1, 0, -1], [1, 2, -1], [1, 2, 3], [1, 2, 4], [1, 2, 6]] {
            put(b, c, "glass")?;
        }
    }
    // (the declared z >= -4 rule's positive control, ironspec 08:13: MCTAS_SIDING_EXTRA_STONE=x,y,z
    // adds one stone at that local cell; at z -5 the bench's HOME rows fail)
    if let Ok(v) = std::env::var("MCTAS_SIDING_EXTRA_STONE") {
        let c: Vec<i32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        ensure!(c.len() == 3, "MCTAS_SIDING_EXTRA_STONE wants x,y,z");
        b.set(c[0], c[1], c[2], "stone");
    }
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
    if std::env::var("MCTAS_SIDING_V").is_ok_and(|v| v == "bulb") {
        put(b, [0, -1, -1], "waxed_copper_bulb[lit=false,powered=false]")?;
        on(b, [0, -1, -2], &rep("north"))?;
        on(b, [-1, -1, -1], "comparator[facing=east,mode=compare,powered=false]")?;
        on(b, [-2, -1, -1], dust)?;
        on(b, [-2, -2, -2], dust)?;
        put(b, [-3, -3, -2], "stone")?; // P
        put(b, [-3, -2, -2], dust)?;
        put(b, [-3, -3, -3], "redstone_wall_torch[facing=north,lit=true]")?;
        put(b, [0, 0, -1], "glass")?;
    } else {
        // V, a set/reset latch of two droppers and one item (parent 2026-11-10: self-recovering
        // from a missed or an extra pulse): DA (0,-1,-1), beside the arrival's stone, fires on
        // ARR and drops the item north into DB; DB (0,-1,-2), beside the R line's support stone
        // (0,-1,-3), fires on R and drops it back south. A second ARR or R finds its dropper empty
        // and does nothing. The item in DA = no visit: a comparator reading DA, a repeater
        // (strength 1 to 15), dust west and down north into the repeater into the gate's side
        put(b, [0, -1, -1], "dropper[facing=north,triggered=false]")?;
        put(b, [0, -1, -2], "dropper[facing=south,triggered=false]")?;
        b.d.block_entities.insert([0, -1, -1], "{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:1}]}".into());
        on(b, [-1, -1, -1], "comparator[facing=east,mode=compare,powered=false]")?;
        on(b, [-2, -1, -1], &rep("east"))?;
        on(b, [-3, -1, -1], dust)?;
        on(b, [-3, -2, -2], dust)?;
        on(b, [-3, -3, -3], dust)?;
        // (glass caps on the droppers' tops)
        put(b, [0, 0, -1], "glass")?;
        put(b, [0, 0, -2], "glass")?;
    }
    on(b, [-4, -3, -3], &rep("east"))?;
    // the gate: F at its back (day-1's dust at FORCE_CELL), out south through a repeater into D2
    on(b, [-5, -3, -3], "comparator[facing=north,mode=subtract,powered=false]")?;
    on(b, [-5, -3, -2], &rep("north"))?;
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
    design_turned(wired, alone, exit_brake, cage_only, give_up, 0)
}

/// The rotation of a design by `turn` quarter turns (0 or 2) about its origin: block cells and
/// states turned (`clock::rotate_pos`, `clock::canon::rotate_state`), entity points p -> 1 - p
/// (so a point in cell x lands in cell -x), the ports, and the `siding:` and `build:` meta lines'
/// cells; a `turn:` meta line records it for the bench and the check.
fn turn_design(d: &mut mctas::design::Design, turn: u8) {
    if turn % 4 == 0 {
        return;
    }
    let rp = |c: [i32; 3]| clock::rotate_pos(c, turn);
    d.blocks = std::mem::take(&mut d.blocks).into_iter().map(|(c, s)| (rp(c), clock::canon::rotate_state(&s, turn))).collect();
    d.block_entities = std::mem::take(&mut d.block_entities).into_iter().map(|(c, s)| (rp(c), s)).collect();
    for e in d.entities.iter_mut() {
        // (a point (u, w) in a cell goes, each quarter turn, to (1 - w, u): in the turned cell)
        for _ in 0..turn % 4 {
            e.pos = [1.0 - e.pos[2], e.pos[1], e.pos[0]];
        }
    }
    for c in d.ports.values_mut() {
        *c = rp(*c);
    }
    let turn_cells = |line: &str| -> String {
        line.split_whitespace()
            .map(|t| {
                let (k, v) = t.split_once('=').map(|(k, v)| (format!("{k}="), v)).unwrap_or((String::new(), t));
                match v.split(',').map(|x| x.parse::<i32>()).collect::<Result<Vec<_>, _>>() {
                    Ok(c) if c.len() == 3 => {
                        let r = rp([c[0], c[1], c[2]]);
                        format!("{k}{},{},{}", r[0], r[1], r[2])
                    }
                    _ => t.to_string(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    for key in ["siding", "build"] {
        if let Some(l) = d.meta.get(key).cloned() {
            d.meta.insert(key.into(), turn_cells(&l));
        }
    }
    d.meta.insert("turn".into(), turn.to_string());
}

/// [`design_full`] turned `turn` quarter turns (0 or 2) about the siding's origin, the T's control
/// inverted for the half turn (`wire`).
pub fn design_turned(wired: bool, alone: bool, exit_brake: bool, cage_only: bool, give_up: GiveUp, turn: u8) -> Result<String> {
    ensure!(turn <= 3, "--turn 0, 1, 2 or 3 quarter turns");
    let s = layout();
    let name = format!("carousel-bed-siding{}{}{}{}{}", if wired { "-wired" } else { "" }, match give_up { GiveUp::None => "", GiveUp::Timer => "-giveup", GiveUp::Scheduler => "-force" }, if exit_brake { "-brake" } else { "" }, if cage_only { "-cage" } else { "" }, if alone { "-alone" } else { "" });
    let mut b = Builder::new(&name);
    if turn != 0 {
        // (read by `wire`: the T's control inverted for turns 2 and 3, where the T's curve rule
        // inverts its polarity)
        b.d.meta.insert("turn".into(), turn.to_string());
    }
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
        // (the dropper latch's item a build step of its own after every placement (fills-last):
        // merged into DA's placement, the comparator reading DA got no update and its line stayed
        // dark, unlike the file; then the glass caps, so the not-V line (about 7 gt from the fill)
        // has settled before the build's two torches end it, at any gap)
        let kz = layout().park.z;
        let mut last: Vec<BlockPos> = [[0, 0, -1], [0, 0, -2], [-5, -2, 2], [-7, -2, -1], [2, -1, kz], [4, 1, -2]].iter().map(|c| BlockPos::new(c[0], c[1], c[2])).collect();
        last.extend([BlockPos::new(-6, -3, -1), BlockPos::new(1, -2, kz)]);
        if turn == 2 || turn == 3 {
            // (the turned copy's T inverter, W then Ta', last: placed early, Ta' lights at its
            // first update and powers the T at rest until W is placed)
            last.extend([BlockPos::new(1, -3, 1), BlockPos::new(1, -2, 0)]);
        }
        b.d.meta.insert("build".into(), crate::placing::BuildPlan { last, fills_last: true }.meta());
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
    // (the brief's "No derails", ironspec 2026-11-11: the declared states of the siding's rails,
    // the activator and the hatch at rest (the bench's launcher boosters and hold brakes too), and
    // its flights: none, no cart leaves the rails on purpose)
    b.d.meta.insert("declared".into(), crate::declared::format(&declared_of(&b, &s, wired)));
    b.d.meta.insert("flights".into(), String::new());
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), at(s.bed_head));
    if turn != 0 {
        // (the build's last cells: the turned copy's T torches Ta' and W after the list as
        // written; then the whole design turned)
        turn_design(&mut b.d, turn);
    }
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

/// The siding's declared states at rest (`declared:`, [`crate::declared`]): every rail of the
/// siding by its id (the T a plain rail, the detectors detector rails), the always-on boosters
/// powered (`!`), the park rail unpowered (the slot's braked rail by its id only: its power is the
/// loop's release input, held or pulsed), the activator powered (the
/// wired siding's rest: it drops arrivals), the hatch open; in the bench design also the
/// launcher's boosters powered and the hold's brakes unpowered (both `!`).
fn declared_of(b: &Builder, s: &Siding, wired: bool) -> Vec<crate::declared::Decl> {
    use crate::declared::Decl;
    let id_at = |c: BlockPos| b.d.blocks.get(&[c.x, c.y, c.z]).map(|st| st.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string());
    let on: &[(&str, &str)] = &[("powered", "true")];
    let off: &[(&str, &str)] = &[("powered", "false")];
    let mut cells: Vec<(BlockPos, &[(&str, &str)], bool)> = vec![(s.entry, &[], false), (s.tee, &[], false), (s.boost, on, true)];
    if id_at(s.free_cell).as_deref() == Some("powered_rail") {
        cells.push((s.free_cell, on, true));
    } else {
        cells.push((s.free_cell, &[], false));
    }
    cells.push((s.activator_rail, if wired { on } else { &[] }, false));
    for z in s.activator_rail.z + 1..s.park_rail.z {
        cells.push((BlockPos::new(s.free_cell.x, s.park_rail.y, z), &[], false));
    }
    cells.extend([(s.park_rail, off, false), (BlockPos::new(2, 0, 0), &[][..], false), (BlockPos::new(3, 0, 0), &[][..], false), (s.slot_rails[0], &[][..], false), (s.slot_rails[1], &[][..], false), (s.exit, &[][..], false)]);
    cells.push((s.hatch, &[("open", "true")], false));
    let mut out: Vec<Decl> = cells.iter().filter_map(|(c, p, a)| id_at(*c).map(|id| Decl::new([c.x, c.y, c.z], &id, p, *a))).collect();
    // (the bench's own: the launcher's boosters, the approach and run-out rails, the hold's brakes)
    let mut extra: Vec<(BlockPos, String)> = b.d.blocks.iter().filter(|(c, st)| st.contains("rail") && !out.iter().any(|d| d.cell == **c)).map(|(c, st)| (BlockPos::new(c[0], c[1], c[2]), st.clone())).collect();
    extra.sort_by_key(|(c, _)| (c.z, c.x, c.y));
    for (c, st) in extra {
        let id = st.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
        if id == "powered_rail" {
            out.push(Decl::new([c.x, c.y, c.z], &id, if st.contains("powered=true") { on } else { off }, true));
        } else {
            out.push(Decl::new([c.x, c.y, c.z], &id, &[], false));
        }
    }
    out
}

/// The static check: every rider and napper cell's golem search: any spot is a stray. `turn`: the
/// design's quarter turns (its `turn:` meta line).
/// (ironspec 09:57, the cage proof) Every standing position a villager (0.6 x 1.95) reaches from the
/// corridor cells (the T, the booster, the free cell): from a cell centre to each of the 8
/// neighbouring cell centres, up at most 1.25 (a jump; fences and walls are 1.5 in their collision
/// boxes, so they stop it) or down any fall, the box free at the start height raised to the target
/// height and at the target, landing on the first floor below. `toggle`: every trapdoor's open
/// state flipped first (the hatch and the trapdoor caps in their other state); `remove`: cells set
/// to air first (a positive control). Returns every reached position, local, with its floor.
pub fn cage(spec: &FarmSpec, toggle: bool, remove: &[[i32; 3]]) -> Result<Vec<([i32; 3], f64)>, String> {
    let fr = Frame { turn: 0, dx: 0, dz: 0 };
    let mut run = crate::station::world_of(spec, 1)?;
    let w = &mut run.world;
    for c in remove {
        let p = fr.blk(c[0], c[1], c[2]);
        w.set_block(p, Block::Air, 2);
    }
    if toggle {
        let o = fr.blk(-8, -8, -6);
        for y in 0..14 {
            for z in 0..17 {
                for x in 0..16 {
                    let p = BlockPos::new(o.x + x, o.y + y, o.z + z);
                    let st = w.get(p).to_string();
                    if st.contains("trapdoor") {
                        let t = if st.contains("open=true") { st.replace("open=true", "open=false") } else { st.replace("open=false", "open=true") };
                        if let Ok(b) = Block::parse(t.trim_start_matches("minecraft:")) {
                            w.set_block(p, b, 2);
                        }
                    }
                }
            }
        }
    }
    let half = 0.3;
    let bx = |x: f64, y: f64, z: f64| redsim::entity::vec3::Aabb { min_x: x - half, min_y: y, min_z: z - half, max_x: x + half, max_y: y + 1.95, max_z: z + half };
    let free = |w: &World, x: f64, y: f64, z: f64| !redsim::entity::collide::collides_with_blocks(w, bx(x, y + 1.0e-4, z));
    // (the landing floor under (x, from, z): the box dropped until it meets a block, at most 24 down)
    let land = |w: &World, x: f64, from: f64, z: f64| -> Option<f64> {
        let mut y = from;
        if !free(w, x, y, z) {
            return None;
        }
        for _ in 0..(24 * 16) {
            if !free(w, x, y - 1.0 / 16.0, z) {
                return Some(y);
            }
            y -= 1.0 / 16.0;
        }
        None
    };
    let mut seen: std::collections::BTreeMap<[i32; 3], f64> = Default::default();
    let mut todo: Vec<(f64, f64, f64)> = Vec::new();
    for c in [[1, 0, 0], [1, 0, 1], [1, 0, 2]] {
        let p = fr.blk(c[0], c[1], c[2]);
        if let Some(y) = land(w, p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5) {
            todo.push((p.x as f64 + 0.5, y, p.z as f64 + 0.5));
        }
    }
    while let Some((x, y, z)) = todo.pop() {
        let l = fr.local(redsim::entity::vec3::Vec3::new(x, y, z));
        let key = [l.x.floor() as i32, l.y.floor() as i32, l.z.floor() as i32];
        if seen.get(&key).is_some_and(|&h| h <= y + 1.0e-6) || seen.len() > 4000 {
            continue;
        }
        seen.insert(key, y);
        for (dx, dz) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            let (tx, tz) = (x + dx, z + dz);
            // (try every start height from the jump's top down to the current floor: the box must be
            // free there at the start and the target column)
            let mut top = y + 1.25;
            while top >= y - 1.0e-6 {
                if free(w, x, top, z) && free(w, tx, top, tz) && (dx == 0.0 || dz == 0.0 || (free(w, tx, top, z) && free(w, x, top, tz))) {
                    if let Some(h) = land(w, tx, top, tz) {
                        todo.push((tx, h, tz));
                    }
                    break;
                }
                top -= 1.0 / 16.0;
            }
        }
    }
    Ok(seen.into_iter().map(|(k, v)| (k, v - fr.blk(0, 0, 0).y as f64)).collect())
}

pub fn check(spec: &FarmSpec, turn: i32) -> Result<Vec<String>, String> {
    let fr = Frame { turn, dx: 0, dz: 0 };
    let s = layout_fr(&fr);
    let run = crate::station::world_of(spec, 1)?;
    let w = &run.world;
    let mut cells: Vec<BlockPos> = vec![s.entry, s.tee, s.boost, s.free_cell, s.activator_rail];
    for z in 4..=layout().park_rail.z {
        cells.push(fr.blk(1, layout().park_rail.y, z));
    }
    cells.extend([fr.blk(2, 0, 0), fr.blk(3, 0, 0), s.slot_rails[0], s.slot_rails[1], s.exit]);
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
    // the siding's own blocks (not the bench's hold, approach and run-out) and its box, judged in
    // the siding's own frame
    let ex = layout().exit.x;
    let own: Vec<&(BlockPos, Block)> = spec
        .blocks
        .iter()
        .filter(|(p, b)| {
            let l = fr.local_cell(*p);
            !matches!(b, Block::Air) && l.z > -15 && !(l.z == 0 && l.x < 0 && (-1..=1).contains(&l.y)) && !(l.z == 0 && l.x > ex && (l.y == -1 || l.y == -2))
        })
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
    /// Fault injection at the stateful elements (ironspec 2026-11-10), on visit K: `arr-miss`
    /// (DB pulsed just after the arrival: as if ARR never moved the item), `arr-extra` (DA pulsed
    /// mid-visit), `r-miss` (after the return: DA pulsed and the H latch set again, as if R never
    /// fired), `r-extra` (mid-visit, before the nap: DB pulsed and the H latch reset), `f-double`
    /// (F pulsed again 30 gt after the first). Each pulse a redstone block for 4 gt beside the
    /// element.
    pub fault: Option<(String, usize)>,
    /// The world moved this many blocks east: at 2 the loop's chunk lines (ironspec 2026-11-10:
    /// X & 15 = 6, Z & 15 = 6 at the loop's cell 0, 0) fall on the siding as day-1 pastes it (the
    /// siding's (−5,−3,−4) at the loop's (−25,−1,6)): siding x −3|−2 and z −1|0.
    pub shift_x: i32,
    /// The world moved this many blocks south (with `shift_x`; the one-chunk placement's 3 puts
    /// the siding's z -3..9 inside one chunk).
    pub shift_z: i32,
    /// The design's quarter turns (its `turn:` meta line, 0 or 2): the bench's frame turns with it.
    pub turn: i32,
    /// A partial unload (tick-exact's `reload::unload_some` / `load_some`; farmopt and ironspec
    /// 2026-11-10): the forced chunk box set at the start, the chunks dropped when visit K reaches
    /// the state, back N gt later.
    pub unload: Option<Unload>,
    /// (ironspec 2026-11-11, V as day-1's divert gate, emulated) No visit is sent while V (the
    /// visit latch) is set: the loop's divert holds the cart back.
    pub divert_gate: bool,
    /// (the gate's deadlock guard, emulated: day-1's scheduler's reset pulse VR) Every scheduled
    /// visit slot, sent or held back, pulses the R line (DB and the H latch's reset, as R does) at
    /// its arrival time + this many gt.
    pub v_reset: Option<i64>,
    /// Design (b) emulated on the wired siding (ironspec's emulation-first rule): the bench drives
    /// the park support itself (the design's kick torch K taken out) by (b)'s rule, on the
    /// siding's real latch H, T, hatch, F gate and slot detector L: the kick while H, the slot
    /// free and the bed empty, and either the bed empty for this many gt since the last wake (the
    /// after-wake kick; a re-sleep restarts it) or no nap since this visit's arrival (the fast
    /// path, an F visit). B (the bed occupied) and N (a nap since the arrival) ideal. With
    /// `v_reset`, the reset pulse is gated by B (VR and not-B: a napper asleep keeps H).
    pub b_emul: Option<i64>,
    /// With `b_emul`: ironspec's two changes (02:05): no kick unless a villager stands on the free
    /// cell (FC, ideal), and VR fired only while the siding holds no cart (at the park or the free
    /// cell).
    pub b_fc: bool,
    /// The design's declared states (its `declared:` line, [`crate::declared`]), checked at the
    /// start and, the always-on cells, at t0 + 20; and its declared flights (`flights:`), every
    /// cart watched for off-rail stretches against them ([`crate::flight::RailWatch`]). In the
    /// design's own coordinates (the bench shifts them with the world).
    pub declared: Vec<crate::declared::Decl>,
    pub flights: Vec<crate::flight::Flight>,
    /// The dawn/dusk resync's input block (the design's `resync:` line, local): the bench powers it
    /// for 12 gt at day times 1,000 and 11,000 (day-1's daylight-stair pulse).
    pub resync: Option<[i32; 3]>,
}

/// A partial unload of the bench's world ([`BenchOpts::unload`]): the forced chunk box (chunk
/// coordinates, `[x0, z0, x1, z1]`), the chunks dropped, the state of visit `visit` it starts at
/// (`arrival`, `nap`: the sleep onset, `fpending`: 10 gt before the earliest force pulse, `kick`:
/// the park rail powered, `exit`: the cart onto the exit branch; `none`: only the forced box, the
/// control) and for how long.
#[derive(Clone, Debug)]
pub struct Unload {
    pub forced: [i32; 4],
    pub drop: [i32; 4],
    pub state: String,
    pub n: i64,
    pub visit: usize,
    /// The whole part saved and unloaded (`reload::unload_all` / `load_all`: the entities back as
    /// new objects with new ids) in place of the partial unload's dropped chunks.
    pub whole: bool,
    /// The whole load's entity order (tick-exact 76dae3d14: `reload::load_all_ordered`; save,
    /// reverse or seed:K). A partial load comes back in save order only (`load_some`).
    pub order: redsim::reload::Order,
}

/// The forced box and the dropped chunks of a partial unload at one of the loop's chunk lines, in
/// the bench's world shifted 2 east ([`BenchOpts::shift_x`]): the siding's chunks are x −1..0 and
/// z −1..0, the x line between chunk x −1 and 0 (the siding's x −3|−2), the z line between chunk
/// z −1 and 0 (its z −1|0). `side` is the side dropped (`lo`: west or north), `level` its level:
/// 32 (the other side forced) or 33 (the other side at 32, the forced box one chunk beyond it).
pub fn unload_boxes(line: &str, side: &str, level: u8) -> Result<([i32; 4], [i32; 4]), String> {
    // (the one-chunk placement, ironspec and farmopt 2026-11-10 22:58: day-1 pastes the siding at
    // ox in [-15,-13], oz in [13,16], inside one chunk; on the bench shifted (8, 3) the siding is
    // chunk (0, 0) and the lines left are the junction tracks': `w` between chunk x -1 and 0 (the
    // approach, the bench's launcher), `e` between 0 and 1 (the run-out); the forced row z 0)
    if line == "w" || line == "e" {
        let (forced, drop) = match (line, side, level) {
            ("w", "lo", 32) => ((-1, 1), (-1, -1)),
            ("w", "lo", 33) => ((-1, 1), (-1, 0)),
            ("w", "hi", 32) => ((-1, 1), (0, 1)),
            ("e", "lo", 32) => ((-1, 1), (-1, 0)),
            ("e", "hi", 32) => ((-1, 1), (1, 1)),
            ("e", "hi", 33) => ((-1, 1), (0, 1)),
            // (w hi 33 and e lo 33 put the siding's chunk at 33 and the far junction's at 34: its
            // entities saved and unloaded, which redsim does not model)
            ("w", "hi", 33) | ("e", "lo", 33) => return Err(format!("{line} {side} 33 leaves the far junction's chunk at 34 (not modeled)")),
            _ => return Err(format!("side lo or hi and level 32 or 33, not {side} {level}")),
        };
        return Ok(([forced.0, 0, forced.1, 0], [drop.0, 0, drop.1, 0]));
    }
    let (forced, drop) = match (side, level) {
        ("lo", 32) => ((-1, 0), (-1, -1)),
        ("lo", 33) => ((-1, 1), (-1, 0)),
        ("hi", 32) => ((-1, 0), (0, 0)),
        ("hi", 33) => ((-2, 0), (-1, 0)),
        _ => return Err(format!("side lo or hi and level 32 or 33, not {side} {level}")),
    };
    match line {
        "x" => Ok(([forced.0, -1, forced.1, 0], [drop.0, -1, drop.1, 0])),
        "z" => Ok(([-1, forced.0, 0, forced.1], [-1, drop.0, 0, drop.1])),
        _ => Err(format!("line x or z, not {line}")),
    }
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
    /// When its cart came onto the exit branch (x past the T's east edge, moving east).
    pub branch: Option<i64>,
    /// When the bench sent its cart (onto the launcher or the approach).
    pub sent: i64,
    /// The siding's T rail: when it turned to bed-exit after the arrival, and back to entry-bed
    /// after the cart left (and the cart's x then).
    pub tee_out: Option<i64>,
    pub tee_back: Option<(i64, f64)>,
    /// (farmopt 03:55, the open top) The first tick after the wake the napper, awake and not
    /// riding, stood off the free cell; the first tick its kicked cart reached the free cell.
    pub left_fc: Option<i64>,
    pub kick_at_fc: Option<i64>,
    /// The napper asleep again with its cart gone (out past the exit): a fail.
    pub asleep_cart_gone: bool,
    /// (farmopt 06:08, a standard line) The cart reached the slot without its napper: the tick,
    /// the napper's position (local) and whether it slept then.
    pub riderless: Option<(i64, [f64; 3], bool)>,
    /// (ironspec 08:40, the self-retrying kick) The kicked cart's bumps (moving north, it stopped short
    /// on the slope with its napper not aboard), its retries (kicked again from the park after a
    /// bump), the napper's position (local) at each bump, and the cart's last local z speed.
    pub bumps: u32,
    pub retries: u32,
    pub bump_at: Vec<[f64; 3]>,
    pub last_vz: f64,
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
    /// The partial unload: when the chunks were dropped and when they came back.
    pub unload: Option<(i64, Option<i64>)>,
    /// The unload's day times (dropped, back; -1 not back).
    pub unload_day: (i64, i64),
    /// The divert gate (`BenchOpts::divert_gate`): the scheduled visits it held back (when), and
    /// the reset pulses VR fired (when).
    pub held_back: Vec<i64>,
    pub v_resets: Vec<i64>,
    /// The napper-aboard sensor (`MCTAS_SIDING_SEAT=1`, the tripwire at (1,2,1)): its turns on
    /// with a seated villager in its band, its turns on without one (and what was there), the
    /// ticks a seated villager crossed the band with the string unpowered, the ticks it was on, a
    /// few examples.
    pub seat: Option<SeatLog>,
    /// ironspec's hard line (2026-11-11, ironfarm's shared check `merge::MergeTally`): merged
    /// carts, two carts the game's push pairs but never separates (under 0.01 apart horizontally).
    pub merged: Vec<(u32, u32)>,
    pub merged_first: Vec<String>,
    /// Design (b) emulated (`BenchOpts::b_emul`): the kicks it gave by the slow path (the bed
    /// empty D gt after the last wake) and the fast path (no nap this visit), and the kicks it
    /// would have given with the bed occupied (none by construction: a check of the rule).
    pub b_slow: u32,
    pub b_fast: u32,
    pub b_kick_in_bed: u32,
    /// The VR pulses held back by the gating (`b_emul`: the bed occupied, or a cart inside).
    pub v_resets_blocked: u32,
    /// The declared states' mismatches (at the start, and the always-on cells at t0 + 20).
    pub declared_bad: Vec<String>,
    /// ironspec's invariant (2026-11-11 02:41): the windows a villager is calm in REST with the
    /// bed out of reach (riding, not panicking, at night): each closed window's phase (of its
    /// visit, at the window's start) and length in gt; the most villagers exposed at one tick and
    /// the ticks with two or more.
    pub exposure: Vec<(String, i64)>,
    pub exposed_max: usize,
    pub exposed_two_ticks: u64,
    /// ironspec 2026-11-11 02:59: while the bed is occupied, the closest any other villager came
    /// to the bed's centre (ValidateNearbyPoi's 16), and where (the villager, its phase).
    pub bed_min_dist: Option<(f64, String)>,
    /// The position audit (ironspec 03:36): ticks the napper was awake and not riding inside the
    /// siding, of those off the free cell, and the first few such positions.
    pub audit_awake_ticks: u64,
    pub audit_off_ticks: u64,
    pub audit_off: Vec<String>,
    /// Boards per kick: the visits' carts onto the exit branch with their napper aboard, and
    /// without (a kick that left without its villager).
    pub kick_boarded: u32,
    pub kick_empty: u32,
    /// The standard stamp line (ironspec via farmopt, 03:52): the oldest CANT_REACH stamp seen
    /// (gt), by where the villager was: in the siding (entry to exit), on the bench's approach, in
    /// the bench's enclosed hold (its riders' flee paths never reach: a bench artifact).
    pub stamp_max: [i64; 3],
    /// (ironspec 05:47 item 1(b)) Villagers dropped at the activator while the bed was occupied.
    pub dropped_bed_taken: u32,
    /// (ironspec 05:23, the farm-wide edge-count check) Rising edges of the arrival detector
    /// (0,0,0) and the return detector (3,0,0), against the carts that entered and went out.
    pub edges: [u32; 2],
    pub carts_in_out: [u32; 2],
    /// (ironspec 07:00 item 1(a)) Design (b): ticks the bulb B disagreed with the bed for more than
    /// 20 gt running (lit with the bed free, or unlit with it occupied), how many such runs, and
    /// the first run's start (t - t0) and kind.
    pub b_wrong: (u32, i64, Option<(i64, bool)>),
    /// (ironspec 08:13, the z -5 HOME finding) The riders' walk state, ticks by where they are (the
    /// siding, the bench's approach, its hold) and by kind (reaching, partial, a walk target with no
    /// path, no walk target), and a few approach samples (walk target and path end, local).
    pub walk_kinds: [[u64; 4]; 3],
    pub walk_samples: Vec<String>,
    /// The off-the-rails watch's derails and its report line.
    pub derails: Vec<String>,
    pub rail_line: String,
    /// The F pulses fired for slots the divert gate held back (day-1's scheduler fires F on
    /// every slot).
    pub held_f_fired: u32,
}

/// The napper-aboard sensor's measurement ([`BenchReport::seat`]).
#[derive(Clone, Debug, Default)]
pub struct SeatLog {
    pub on_seated: u32,
    pub on_other: u32,
    pub missed: u32,
    pub on_ticks: u32,
    pub seen_standing: u32,
    /// Every seated villager's box top seen (min, max) and its feet above its cart's (min, max).
    pub seated_top: Option<(f64, f64)>,
    pub seated_feet: Option<(f64, f64)>,
    pub examples: Vec<String>,
}

fn block(s: &str) -> Block {
    Block::parse(&format!("minecraft:{s}")).expect("a known block")
}

/// A block state in a few letters for the circuit trace: lit/powered/shape/open.
fn short_state(s: &str) -> String {
    let s = s.trim_start_matches("minecraft:");
    let name: String = s.split('[').next().unwrap_or(s).chars().take(5).collect();
    let mut flags = String::new();
    for (k, tag) in [("lit=true", "L"), ("powered=true", "P"), ("triggered=true", "T"), ("open=true", "O"), ("occupied=true", "Z"), ("attached=true", "")] {
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

/// A farm spec moved `dx` east (blocks, block entities, entities, its box and output; the
/// villagers' loaded HOME memory, `pos:[I;x,y,z]` in their SNBT, too).
fn shift_spec(spec: &FarmSpec, dx: i32, dz: i32) -> FarmSpec {
    let mv = |p: BlockPos| BlockPos::new(p.x + dx, p.y, p.z + dz);
    let mv_nbt = |s: &str| -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(i) = rest.find("pos:[I;") {
            let (head, tail) = rest.split_at(i + "pos:[I;".len());
            out.push_str(head);
            let end = tail.find(']').unwrap_or(tail.len());
            let v: Vec<i64> = tail[..end].split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() == 3 {
                out.push_str(&format!("{},{},{}", v[0] + dx as i64, v[1], v[2] + dz as i64));
            } else {
                out.push_str(&tail[..end]);
            }
            rest = &tail[end..];
        }
        out.push_str(rest);
        out
    };
    let mut o = spec.clone();
    o.blocks = spec.blocks.iter().map(|(p, b)| (mv(*p), *b)).collect();
    o.block_entities = spec.block_entities.iter().map(|(p, s)| (mv(*p), s.clone())).collect();
    o.entities = spec.entities.iter().map(|(t, p, n)| (t.clone(), [p[0] + dx as f64, p[1], p[2] + dz as f64], n.as_deref().map(mv_nbt))).collect();
    o.output = mv(spec.output);
    o.lo = mv(spec.lo);
    o.hi = mv(spec.hi);
    o
}

/// The bench: visits through the nights (each night from day time 12000, `gap` apart, until
/// 23000), each taking the villager napped longest ago from the far hold, its cart teleported to
/// the approach moving +x at `speed`; the controls driven ideally (the module's doc); the release
/// `release` after the arrival; the cart back to its hold cell once out past the exit.
pub fn bench(spec: &FarmSpec, bo: &BenchOpts) -> Result<BenchReport, String> {
    let (dx, dz) = (bo.shift_x, bo.shift_z);
    // (the frame: the design turned (its `turn:` line) and the world shifted (dx, dz) so the loop's
    // chunk lines fall on the siding as day-1 pastes it; positions are compared in the siding's
    // own frame through it)
    let fr = Frame { turn: bo.turn, dx, dz };
    let s = layout_fr(&fr);
    let moved;
    let spec = if dx != 0 || dz != 0 {
        if bo.drive {
            return Err("a shifted bench writes no drive (its coordinates are not the design's)".into());
        }
        moved = shift_spec(spec, dx, dz);
        &moved
    } else {
        spec
    };
    let hold_at = |h: i32| fr.cell(hold_cell(h));
    let force_cell = fr.blk(FORCE_CELL[0], FORCE_CELL[1], FORCE_CELL[2]);
    let resync_cell = bo.resync.map(|c| fr.blk(c[0], c[1], c[2]));
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
    // (the partial unload's forced chunk box, from the start: outside it the chunks sit at 31 plus
    // their distance to it; a whole-part unload keeps the world's box)
    if let Some(u) = &bo.unload
        && !u.whole
    {
        run.world.entity_chunks = Some(u.forced);
    }
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
        if let Some(h) = (0..21).find(|&h| hold_at(h) == cell) {
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
    let gate = fr.blk(FORCE_CELL[0], FORCE_CELL[1], FORCE_CELL[2] + 1);
    let force = (bo.force.is_some() && matches!(run.world.get(gate), Block::Comparator { .. })).then(|| bo.force.unwrap_or((150, 170)));
    let mut force_off: Option<i64> = None;
    // (the F pulses of the slots the divert gate held back)
    let mut held_f: Vec<i64> = Vec::new();
    let mut reset_pending = false;
    let mut homes_seen: Vec<bool> = vec![true; pairs.len()];
    let mut home_now: Vec<bool> = pairs.iter().map(|(_, v, _)| index_of(&run.world, *v).is_some_and(|i| brain::brain(&run.world, i).home == Some(bed))).collect();
    let mut lost_at: Vec<Option<i64>> = vec![None; pairs.len()];
    // (the siding's T rail's shape: its turns logged against the latest visit)
    let tee_shape = |w: &World| -> String {
        let st = w.get(s.tee).to_string();
        st.split("shape=").nth(1).map(|x| x.split([',', ']']).next().unwrap_or("").to_string()).unwrap_or_default()
    };
    let mut tee_prev = tee_shape(&run.world);
    let mut kick_seen: Option<i64> = None;
    // (the divert gate's reset pulses VR, emulated: when they fire; and their cells, the R line's
    // two ends as R pulses them: DB beside its dropper, the H latch's reset input)
    let mut vr_at: Vec<i64> = Vec::new();
    let (vr_db, vr_n2) = (fr.blk(1, -1, -2), fr.blk(-8, -3, -1));
    // (the napper-aboard sensor, when the design has it: the string's band, or the plate's touch
    // box, and its last state)
    let seat_cell = fr.blk(1, 2, 1);
    let plate = run.world.get(seat_cell).to_string().contains("pressure_plate");
    let seat_band = if plate {
        fr.bb([1.0 + 1.0 / 16.0, 2.0, 1.0 + 1.0 / 16.0], [1.0 + 15.0 / 16.0, 2.25, 1.0 + 15.0 / 16.0])
    } else {
        fr.bb([1.0, 2.0 + 1.0 / 16.0, 1.0], [2.0, 2.0 + 2.5 / 16.0, 2.0])
    };
    let mut seat_prev = false;
    // (ironspec's merged-carts hard line: ironfarm's shared tally, fed every tick)
    let mut merges = crate::merge::MergeTally::default();
    // (the brief's "No derails" (ironspec 2026-11-11): the declared states at the start, the
    // always-on ones again at t0 + 20, and every cart watched against the declared flights; in
    // the shifted world, moved with it. Positive controls: MCTAS_DECLARED_CONTROL=1 declares the
    // park rail powered (it rests unpowered: must fail); MCTAS_RAILWATCH_CONTROL=1 lifts the
    // first visit's cart off its rails at its arrival (must count a derail))
    let mut decls: Vec<crate::declared::Decl> = bo.declared.iter().map(|d| crate::declared::Decl { cell: [d.cell[0] + dx, d.cell[1], d.cell[2] + dz], ..d.clone() }).collect();
    if std::env::var("MCTAS_DECLARED_CONTROL").is_ok_and(|v| v == "1") {
        decls.push(crate::declared::Decl::new([s.park_rail.x, s.park_rail.y, s.park_rail.z], "powered_rail", &[("powered", "true")], true));
    }
    let railwatch_control = std::env::var("MCTAS_RAILWATCH_CONTROL").is_ok_and(|v| v == "1");
    rep.declared_bad.extend(crate::declared::check(&run.world, &decls, false).into_iter().map(|b| format!("t0: {b}")));
    let mut rail_watch = crate::flight::RailWatch::new(bo.flights.iter().map(|f| f.shifted(dx, dz)).collect());
    let mut exp_since: Vec<Option<(i64, &str)>> = vec![None; pairs.len()];
    // (design (b) emulated: its state; the design's kick torch K taken out, the bench drives the
    // park support)
    #[derive(Default)]
    struct BEmul {
        since: Option<i64>,
        n: bool,
        arr_prev: bool,
        kick: bool,
        h_prev: bool,
        cart_seen: Option<i64>,
        aboard_set: Option<i64>,
        act_off: bool,
    }
    let mut be = BEmul::default();
    if bo.b_emul.is_some() {
        set(&mut run.world, fr.blk(1, -2, layout().park.z), "air");
    }
    let mut seat_seated_last: i64 = i64::MIN;
    if plate || matches!(run.world.get(seat_cell), Block::Tripwire { .. }) {
        rep.seat = Some(SeatLog::default());
    }
    let mut probe_last: Vec<String> = Vec::new();
    let mut det_prev = [false; 2];
    let mut b_wrong_since: Option<i64> = None;
    while run.world.time < end {
        let now = run.world.time;
        let day = run.world.day_time().rem_euclid(24000);
        let night = !(10..12000).contains(&day);
        // (the night a visit belongs to: the day count at its night's dusk)
        let night_n = (run.world.day_time() - w0).div_euclid(24000);
        let in_window = (run.world.day_time() - w0).rem_euclid(24000) < 11000;
        // ironspec's check: every villager's HOME = the bed at every dawn and every midday
        if (day == 0 || day == 6000) && run.world.away.is_none() {
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
            if run.world.away.is_some() {
                // (the whole part unloaded: the loop's carts are away too; the next visit waits)
                next_visit = Some(now + 1);
            } else if !in_window {
                next_visit = None;
            } else if {
                // (the bench never drops a cart onto another: a cart still on the launcher or the
                // approach start holds the next arrival back, as the loop's own track would; else
                // the bench itself makes merged carts)
                let at = if bo.launch { LAUNCH_X } else { APPROACH_X };
                run.world.entities.list.iter().any(|e| {
                    let l = fr.local(e.pos);
                    !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && (l.x - at).abs() < 1.5 && (l.z - 0.5).abs() < 1.0 && (l.y - 0.0625).abs() < 1.0
                })
            } {
                next_visit = Some(now + 20);
            } else if bo.divert_gate && {
                // (the divert gate, emulated: V set, or B, the bed occupied (ideal: the bed's own
                // state), holds the scheduled visit back; ironspec 2026-11-11: V or B. With design
                // (b) wired, B is its bulb (-2,0,2): day-1's gate reads the siding's B, wrong or
                // right, ironspec 05:47 item 1(b))
                let v = match run.world.get(fr.blk(0, -1, -1)) {
                    Block::Bulb { lit, .. } => lit,
                    _ => matches!(run.world.get(fr.blk(-1, -1, -1)), Block::Comparator { powered: false, .. }),
                };
                let b_wired = match (bo.resync.is_some(), run.world.get(fr.blk(-2, 0, 2))) {
                    (true, Block::Bulb { lit, .. }) => Some(lit),
                    _ => None,
                };
                v || b_wired.unwrap_or_else(|| matches!(run.world.get(bed), Block::Bed { occupied: true, .. }))
            } {
                rep.held_back.push(now);
                if let Some(at) = bo.v_reset {
                    vr_at.push(now + 30 + at);
                }
                // (day-1's scheduler fires F on every visit slot, a diverted one too (ironspec
                // 02:05): the held-back slot's F at its would-be arrival plus the usual delay)
                if let Some((fa, fb)) = force {
                    held_f.push(now + 30 + fa + now.rem_euclid(fb - fa + 1));
                }
                let gap = bo.gap.0 + rng.next_int((bo.gap.1 - bo.gap.0 + 1) as i32) as i64;
                next_visit = Some(now + gap);
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
                        let p = fr.pt(LAUNCH_X, 0.0625, 0.5);
                        teleport(&mut run.world, cart, p, Vec3::new(0.0, 0.0, 0.0));
                        drive.act(now, format!("tp {} {} {} {}", drive.name(cart), p.x, p.y, p.z));
                    } else {
                        teleport(&mut run.world, cart, fr.pt(APPROACH_X, 0.0625, 0.5), fr.vel(Vec3::new(v, 0.0, 0.0)));
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
                    // (V before the arrival: the bulb lit, or the dropper latch's DA empty, its
                    // comparator at (-1,-1,-1) off)
                    let v_before = force.is_some().then(|| match run.world.get(fr.blk(0, -1, -1)) {
                        Block::Bulb { lit, .. } => lit,
                        _ => matches!(run.world.get(fr.blk(-1, -1, -1)), Block::Comparator { powered: false, .. }),
                    });
                    rep.visits.push(VisitRec { villager, cart, arrived: now + eta, sent: now, night: night_n, failed, force_dropped: dropped_f, v_before, ..Default::default() });
                    if let Some(at) = bo.v_reset {
                        vr_at.push(now + eta + at);
                    }
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
        // (the fault injection's pulses: (cell, on until))
        if let Some((kind, fk)) = &bo.fault
            && let Some(v) = rep.visits.get(*fk)
        {
            let rb = |w: &mut World, c: [i32; 3], on: bool| w.set_block(fr.blk(c[0], c[1], c[2]), block(if on { "redstone_block" } else { "air" }), redsim::world::flags::ALL);
            let (da, db, n1, n2) = ([1, -1, -1], [1, -1, -2], [-6, -3, 2], [-8, -3, -1]);
            let mut pulse = |w: &mut World, cells: &[[i32; 3]], at: i64| {
                if now == at {
                    for &c in cells {
                        rb(w, c, true);
                    }
                }
                if now == at + 4 {
                    for &c in cells {
                        rb(w, c, false);
                    }
                }
            };
            let entered = !bo.launch || v.entered;
            match kind.as_str() {
                "arr-miss" if entered => pulse(&mut run.world, &[db], v.arrived + 15),
                "arr-extra" if entered => pulse(&mut run.world, &[da], v.arrived + 60),
                "r-extra" if entered => pulse(&mut run.world, &[db, n2], v.arrived + 8),
                "r-miss" => {
                    // (after R's own pulse has settled: the long nap path delays R by its 4-tick
                    // repeaters and the R line's two extra ones, so an earlier pulse is undone)
                    if let Some(r) = v.ready {
                        pulse(&mut run.world, &[da, n1], r + 40);
                    }
                }
                "f-double" => {
                    if let Some(f) = v.forced {
                        let fc = force_cell;
                        if now == f + 30 {
                            set(&mut run.world, fc, "redstone_block");
                        }
                        if now == f + 42 {
                            set(&mut run.world, fc, "air");
                        }
                    }
                }
                // (tick-exact 02:28 for ironspec's item 1: a night-time sleep failure with the bed
                // free. no-drop: the activator a plain rail for the visit, so the napper stays
                // aboard and parks with its cart, its walk to the bed unreachable; bed-taken: the
                // bed marked occupied with nobody in it for the visit. Both undone once the cart
                // is out; both written to the drive)
                "no-drop" | "bed-taken" => {
                    let cells: Vec<(BlockPos, String, String)> = if kind == "no-drop" {
                        let r = |st: &str| clock::canon::rotate_state(st, fr.turn.rem_euclid(4) as u8);
                        vec![(s.activator_rail, r(if retry() { "rail[shape=ascending_north,waterlogged=false]" } else { "rail[shape=north_south,waterlogged=false]" }), r(if retry() { "activator_rail[powered=false,shape=ascending_north,waterlogged=false]" } else { "activator_rail[powered=false,shape=north_south,waterlogged=false]" }))]
                    } else {
                        [s.bed_head, s.bed_foot].iter().map(|&c| {
                            let st = run.world.get(c).to_string().trim_start_matches("minecraft:").to_string();
                            (c, st.replace("occupied=false", "occupied=true"), st.replace("occupied=true", "occupied=false"))
                        }).collect()
                    };
                    let on_at = if kind == "no-drop" { v.sent + 1 } else { v.arrived + 2 };
                    if now == on_at {
                        for (c, on, _) in &cells {
                            set(&mut run.world, *c, on);
                            drive.act(now, format!("set {} {} {} minecraft:{on}", c.x, c.y, c.z));
                        }
                    }
                    if v.exited == Some(now - 1) {
                        for (c, _, off) in &cells {
                            set(&mut run.world, *c, off);
                            drive.act(now, format!("set {} {} {} minecraft:{off}", c.x, c.y, c.z));
                        }
                    }
                }
                // (farmopt 05:47 / ironspec 05:47 item 2, B's bulb faults: the bench flips the bulb
                // once (its lit state) at a phase: right after the sleep's or the wake's own toggle
                // (a missed pulse), or as an extra pulse before the arrival, mid-nap or inside the
                // after-wake delay; written to the drive)
                "b-sleep" | "b-wake" | "b-extra" | "b-nap" | "b-delay" => {
                    let at = match kind.as_str() {
                        "b-sleep" => v.slept.map(|x| x + 3),
                        "b-wake" => v.woken.map(|x| x + 3),
                        "b-extra" => Some(v.sent + 5),
                        "b-nap" => v.slept.map(|x| x + 10),
                        _ => v.woken.map(|x| x + 30),
                    };
                    if at == Some(now) {
                        let c = fr.blk(-2, 0, 2);
                        let st = run.world.get(c).to_string().trim_start_matches("minecraft:").to_string();
                        if st.starts_with("waxed_copper_bulb") {
                            let flipped = if st.contains("lit=true") { st.replace("lit=true", "lit=false") } else { st.replace("lit=false", "lit=true") };
                            set(&mut run.world, c, &flipped);
                            drive.act(now, format!("set {} {} {} minecraft:{flipped}", c.x, c.y, c.z));
                        }
                    }
                }
                _ => {}
            }
        }
        // (ironspec 07:00 item 1(a): design (b)'s bulb against the bed, a run of disagreement over
        // 20 gt counted once; the bulb and the bed change a few ticks apart at every sleep and wake)
        if bo.resync.is_some() && run.world.away.is_none() {
            let lit = matches!(run.world.get(fr.blk(-2, 0, 2)), Block::Bulb { lit: true, .. });
            let occ = matches!(run.world.get(bed), Block::Bed { occupied: true, .. });
            if lit != occ {
                let since = *b_wrong_since.get_or_insert(now);
                if now - since == 20 {
                    rep.b_wrong.0 += 1;
                    rep.b_wrong.2.get_or_insert((since - t0, lit));
                }
                if now - since >= 20 {
                    rep.b_wrong.1 += 1;
                }
            } else {
                b_wrong_since = None;
            }
        }
        // (the dawn/dusk resync, bench-driven as day-1's daylight-stair pulse: P a redstone block
        // for 12 gt at day times 1,000 and 11,000)
        {
            let det = |w: &World, c: BlockPos| matches!(w.get(c), Block::DetectorRail { powered: true, .. });
            let now_d = [det(&run.world, fr.blk(0, 0, 0)), det(&run.world, fr.blk(3, 0, 0))];
            for i in 0..2 {
                if now_d[i] && !det_prev[i] && run.world.away.is_none() {
                    rep.edges[i] += 1;
                }
            }
            det_prev = now_d;
        }
        if let Some(p) = resync_cell {
            let dt = run.world.day_time().rem_euclid(24000);
            if dt == 1000 || dt == 11000 {
                set(&mut run.world, p, "redstone_block");
                drive.act(now, format!("set {} {} {} minecraft:redstone_block", p.x, p.y, p.z));
            }
            if dt == 1012 || dt == 11012 {
                set(&mut run.world, p, "stone");
                drive.act(now, format!("set {} {} {} minecraft:stone", p.x, p.y, p.z));
            }
        }
        if let Some((fa, fb)) = force {
            let fc = force_cell;
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
            if held_f.contains(&now) && force_off.is_none() {
                set(&mut run.world, fc, "redstone_block");
                drive.act(now, format!("set {} {} {} minecraft:redstone_block", fc.x, fc.y, fc.z));
                force_off = Some(now + 12);
                rep.held_f_fired += 1;
            }
            if force_off == Some(now) {
                set(&mut run.world, fc, "air");
                drive.act(now, format!("set {} {} {} minecraft:air", fc.x, fc.y, fc.z));
                force_off = None;
            }
        }
        // (the divert gate's reset pulse VR: a redstone block at both ends of the R line for 4 gt;
        // under design (b)'s emulation gated by B (not fired while the bed is occupied) and, with
        // `b_fc`, by the siding holding no cart (ironspec 02:05: a cart at the park or the free
        // cell, carts sensed, never riders))
        let cart_inside = || {
            // (any cart from the entry over the T and down the bed line to the park: the wired
            // sensor's detector rails at the free cell and the park hold 20 gt past a cart,
            // covering the stretch between them)
            run.world.entities.list.iter().any(|e| {
                let p = fr.local(e.pos);
                !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && ((p.x >= 0.0 && p.x < 2.0 && p.z >= 0.0 && p.z < 1.0) || (p.x >= 1.0 && p.x < 2.0 && p.z >= 1.0 && p.z < layout().park_rail.z as f64 + 1.0))
            })
        };
        if (bo.b_fc || bo.resync.is_some()) && (cart_inside() || run.world.get(s.entry).to_string().contains("powered=true")) {
            // (the arrival detector ARR counts too: a VR landing on the arrival tick clears the
            // V that ARR has just set, before the cart is past the entry cell)
            be.cart_seen = Some(now);
        }
        // (design (b) wired: day-1's VR as declared, gated by the siding's B (its bulb) and by the
        // siding holding no cart (ARR and the carts from the entry to the park), 20 gt held)
        let b_bulb = bo.resync.is_some() && matches!(run.world.get(fr.blk(-2, 0, 2)), Block::Bulb { lit: true, .. });
        let vr_blocked = (bo.b_emul.is_some() && (matches!(run.world.get(bed), Block::Bed { occupied: true, .. }) || (bo.b_fc && be.cart_seen.is_some_and(|c| now - c <= 20)))) || (bo.resync.is_some() && (b_bulb || be.cart_seen.is_some_and(|c| now - c <= 20)));
        // (a one-tick dump for tracing: MCTAS_BENCH_DUMP_AT=gt1,gt2,... prints every cart and mob
        // inside the siding's box at those ticks after t0, local, with riders and sleep)
        if bo.trace && let Ok(v) = std::env::var("MCTAS_BENCH_DUMP_AT") && v.split(',').filter_map(|x| x.trim().parse::<i64>().ok()).any(|g| g == now - t0) {
            for e in run.world.entities.list.iter().filter(|e| !e.removed) {
                let l = fr.local(e.pos);
                if l.x < -8.0 || l.x > 8.0 || l.z < -5.0 || l.z > 10.0 || l.y < -8.0 || l.y > 5.0 {
                    continue;
                }
                let what = match &e.kind {
                    EntityKind::Minecart(_) => "cart".to_string(),
                    EntityKind::Mob(_) => format!("mob{}", if let Some(i) = index_of(&run.world, e.id) { if redsim::entity::mob::mob(&run.world, i).sleeping_pos.is_some() { " asleep" } else { "" } } else { "" }),
                    _ => continue,
                };
                eprintln!("dump gt {}: {what} #{} at {:.2} {:.2} {:.2} v {:.3} {:.3} vehicle {:?}", now - t0, e.id, l.x, l.y, l.z, e.delta_movement.x, e.delta_movement.z, e.vehicle);
            }
        }
        if bo.trace && vr_at.contains(&now) {
            eprintln!("VR gt {}: {} (DA {} | DB {})", now - t0, if vr_blocked { "held back" } else { "fires" }, run.world.get(fr.blk(0, -1, -1)), run.world.get(fr.blk(0, -1, -2)));
        }
        if vr_at.contains(&now) && vr_blocked {
            rep.v_resets_blocked += 1;
        }
        if vr_at.contains(&now) && !vr_blocked {
            set(&mut run.world, vr_db, "redstone_block");
            set(&mut run.world, vr_n2, "redstone_block");
            rep.v_resets.push(now);
        }
        if rep.v_resets.last() == Some(&(now - 4)) {
            set(&mut run.world, vr_db, "air");
            set(&mut run.world, vr_n2, "air");
        }
        // (the partial unload: the chunks dropped when visit K reaches the state, back N gt later;
        // or the whole part saved and unloaded, back as new entities whose ids the bench follows;
        // both written to the drive as tick-exact's acts)
        if let Some(u) = &bo.unload
            && let Some(v) = rep.visits.get(u.visit)
            && (!bo.launch || v.entered || u.state == "sent")
        {
            if kick_seen.is_none() && now > v.arrived && v.entered | !bo.launch && run.world.away.is_none() && run.world.get(s.park_rail).to_string().contains("powered=true") {
                kick_seen = Some(now);
            }
            let bx = |b: [i32; 4]| format!("{},{},{},{}", b[0], b[1], b[2], b[3]);
            match rep.unload {
                None => {
                    // (a state may carry an offset, `kick+5`: that many gt after it, to sample a
                    // window densely: farmopt 06:09, the kick-to-pickup window tick by tick)
                    let (sname, soff) = match u.state.split_once('+') {
                        Some((n, d)) => (n, d.parse::<i64>().unwrap_or(0)),
                        None => (u.state.as_str(), 0),
                    };
                    let at = match sname {
                        "sent" => Some(v.sent + 10),
                        "arrival" => Some(v.arrived),
                        "arriving" => Some(v.arrived + 8),
                        "drop" => v.dropped,
                        // (the napper's walk to the bed, and the cart at the park: ironspec's long-freeze phases)
                        "walk" => v.dropped.map(|x| x + 4),
                        "park" => v.dropped.map(|x| x + 10),
                        "nap" => v.slept,
                        "asleep" => v.slept.map(|x| x + 10),
                        "waking" => v.woken,
                        // (inside design (b)'s after-wake delay: 30 gt after the wake)
                        "delay" => v.woken.map(|x| x + 30),
                        "fpending" => force.map(|(fa, _)| v.arrived + fa - 10),
                        "kick" => kick_seen,
                        "reboard" => kick_seen.map(|x| x + 9),
                        "exit" => v.branch,
                        _ => None,
                    }
                    .map(|a| a + soff);
                    if let Some(at) = at
                        && now >= at
                    {
                        if u.whole {
                            redsim::reload::unload_all(&mut run.world);
                            drive.act(now, "unload".into());
                        } else {
                            redsim::reload::unload_some(&mut run.world, u.drop);
                            drive.act(now, format!("unload {}", bx(u.drop)));
                        }
                        rep.unload = Some((now, None));
                        rep.unload_day = (run.world.day_time().rem_euclid(24000), -1);
                    }
                }
                Some((from, None)) if now >= from + u.n => {
                    if u.whole {
                        redsim::reload::load_all_ordered(&mut run.world, u.order);
                        drive.act(now, if u.order == redsim::reload::Order::Save { "load".into() } else { format!("load order={}", u.order.name()) });
                        let w = &run.world;
                        let re = |id: u32| w.renumbered.get(&id).copied().unwrap_or(id);
                        for (&o, &n) in &w.renumbered {
                            drive.renamed(o, n);
                        }
                        for p in pairs.iter_mut() {
                            (p.0, p.1) = (re(p.0), re(p.1));
                        }
                        for v in rep.visits.iter_mut() {
                            (v.cart, v.villager) = (re(v.cart), re(v.villager));
                        }
                        run.own = run.own.iter().map(|&i| re(i)).collect();
                    } else {
                        redsim::reload::load_some(&mut run.world, u.drop);
                        drive.act(now, format!("load {}", bx(u.drop)));
                    }
                    rep.unload = Some((from, Some(now)));
                    rep.unload_day.1 = run.world.day_time().rem_euclid(24000);
                }
                _ => {}
            }
        }
        // (design (b) emulated: the park support driven by (b)'s rule, `BenchOpts::b_emul`)
        if let Some(d) = bo.b_emul
            && run.world.away.is_none()
        {
            let w = &run.world;
            let wire_on = |c: BlockPos| matches!(w.get(c), Block::Wire { power, .. } if power > 0);
            let h = wire_on(fr.blk(-1, -3, 2));
            // (L: the slot's south tripwire hook itself; its line merges into the chain's)
            let l = w.get(fr.blk(4, 0, 1)).to_string().contains("powered=true");
            let b = matches!(w.get(bed), Block::Bed { occupied: true, .. });
            let arr = w.get(s.entry).to_string().contains("powered=true");
            if arr && !be.arr_prev {
                be.n = false;
            }
            be.arr_prev = arr;
            be.n |= b;
            // (ironspec 2026-11-11 02:05: no kick unless the napper stands on the free cell: FC,
            // ideal, a villager not riding in the free cell's column)
            let fcell = layout().free_cell;
            let fc = w.entities.list.iter().any(|e| {
                let p = fr.local(e.pos);
                !e.removed && e.vehicle.is_none() && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager) && p.x >= fcell.x as f64 && p.x < fcell.x as f64 + 1.0 && p.z >= fcell.z as f64 && p.z < fcell.z as f64 + 1.0 && p.y > -0.5 && p.y < 1.0
            });
            // (or the napper still aboard its parked cart: never dropped (an F before the drop
            // turns the activator off), it leaves with its cart: the kick sends no cart off
            // without its villager either way; a seat sensor over the park in the wiring)
            let pz = layout().park_rail.z as f64;
            let aboard = w.entities.list.iter().any(|e| {
                let p = fr.local(e.pos);
                !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && p.x >= 1.0 && p.x < 2.0 && p.z >= pz && p.z < pz + 1.0
            });
            if h && !b {
                be.since.get_or_insert(now);
            } else {
                be.since = None;
            }
            let slow = be.since.is_some_and(|x| now - x >= d);
            // (the napper aboard its parked cart (never dropped): with `b_fc` it leaves at once,
            // whatever the bed (B may be someone else's or a stale flag): H set for it through the
            // latch's set input (the wiring: the seat sensor over the park into the S line))
            let aboard_kick = bo.b_fc && aboard;
            let nofast = std::env::var("MCTAS_BEMUL_NOFAST").is_ok_and(|v| v == "1");
            let kick = h && !l && ((!b && (fc || !bo.b_fc) && (slow || (!be.n && !nofast))) || aboard_kick);
            if bo.b_fc && aboard && !h && be.aboard_set.is_none() {
                let n1 = fr.blk(-6, -3, 2);
                set(&mut run.world, n1, "redstone_block");
                drive.act(now, format!("set {} {} {} minecraft:redstone_block", n1.x, n1.y, n1.z));
                be.aboard_set = Some(now);
            }
            if let Some(a) = be.aboard_set
                && now == a + 4
            {
                let n1 = fr.blk(-6, -3, 2);
                set(&mut run.world, n1, "air");
                drive.act(now, format!("set {} {} {} minecraft:air", n1.x, n1.y, n1.z));
            }
            if be.aboard_set.is_some_and(|a| now > a + 4) && !aboard {
                be.aboard_set = None;
            }
            // (the drop gated by B: the activator off while the bed is occupied, so a napper
            // arriving at an occupied bed is not dropped beside it (the wiring: B into the
            // activator torch's block, with H))
            if bo.b_fc && b != be.act_off {
                be.act_off = b;
                let r = |st: &str| clock::canon::rotate_state(st, fr.turn.rem_euclid(4) as u8);
                let sh = if retry() { "ascending_north" } else { "north_south" };
                let st = if b { r(&format!("rail[shape={sh},waterlogged=false]")) } else { r(&format!("activator_rail[powered=false,shape={sh},waterlogged=false]")) };
                set(&mut run.world, s.activator_rail, &st);
                drive.act(now, format!("set {} {} {} minecraft:{st}", s.activator_rail.x, s.activator_rail.y, s.activator_rail.z));
            }
            if bo.trace && (h != be.h_prev || kick != be.kick || (now - t0 >= bo.trace_window.0 && now - t0 < bo.trace_window.1 && (now - t0) % 50 == 0)) {
                eprintln!("b-emul gt {}: H {h} L {l} B {b} N {} FC {fc} since {:?} kick {kick}", now - t0, be.n, be.since.map(|x| now - x));
            }
            be.h_prev = h;
            if kick && !be.kick {
                if slow {
                    rep.b_slow += 1;
                } else {
                    rep.b_fast += 1;
                }
            }
            if kick != be.kick {
                be.kick = kick;
                let st = if kick { "redstone_block" } else { "stone" };
                set(&mut run.world, s.park, st);
                drive.act(now, format!("set {} {} {} minecraft:{st}", s.park.x, s.park.y, s.park.z));
            }
            if be.kick && matches!(run.world.get(bed), Block::Bed { occupied: true, .. }) {
                rep.b_kick_in_bed += 1;
            }
        }
        // (the off-the-rails watch's positive control: the first visit's cart lifted 3 blocks at
        // its arrival + 2)
        if railwatch_control
            && let Some(v) = rep.visits.first()
            && now == v.arrived + 2
            && let Some(i) = run.world.entities.list.iter().position(|e| e.id == v.cart && !e.removed)
        {
            let (p, vel) = (run.world.entities.list[i].pos, run.world.entities.list[i].delta_movement);
            teleport(&mut run.world, v.cart, Vec3::new(p.x, p.y + 3.0, p.z), vel);
        }
        run.tick();
        let t = run.world.time;
        merges.observe(&run.world, &format!("t0+{}", t - t0));
        if t == t0 + 20 {
            rep.declared_bad.extend(crate::declared::check(&run.world, &decls, true).into_iter().map(|b| format!("t0+20: {b}")));
        }
        for (k, p) in pairs.iter().enumerate() {
            rail_watch.observe(&run.world, k, p.0, Some(p.1), &format!("t0+{}", t - t0));
        }
        // (the exposure, tick-exact's definition (2026-11-11 03:14), so the two lines agree: deep
        // night (day time 13,670..22,330), REST active, CANT_REACH present, HOME held, riding or
        // not, with or without a walk target)
        {
            let w = &run.world;
            // (the bed's other villagers: while it is occupied, every villager but the sleeper,
            // its distance to the bed's centre)
            if matches!(w.get(bed), Block::Bed { occupied: true, .. }) {
                let c = Vec3::new(bed.x as f64 + 0.5, bed.y as f64 + 0.5, bed.z as f64 + 0.5);
                for p in pairs.iter() {
                    if let Some(i) = index_of(w, p.1) {
                        let e = &w.entities.list[i];
                        if redsim::entity::mob::mob(w, i).sleeping_pos.is_some() {
                            continue;
                        }
                        let d = ((e.pos.x - c.x).powi(2) + (e.pos.y - c.y).powi(2) + (e.pos.z - c.z).powi(2)).sqrt();
                        if rep.bed_min_dist.as_ref().is_none_or(|(m, _)| d < *m) {
                            let l = fr.local(e.pos);
                            rep.bed_min_dist = Some((d, format!("villager {} at {:.2} {:.2} {:.2} (local), riding {}, t0+{}", p.1, l.x, l.y, l.z, e.vehicle.is_some(), t - t0)));
                        }
                    }
                }
            }
            let dtn = w.day_time().rem_euclid(24000);
            let night_now = (13670..=22330).contains(&dtn);
            let mut n_exp = 0usize;
            for (k, p) in pairs.iter().enumerate() {
                // (ironspec 08:13: the riders' walk state by region, and approach samples)
                if let Some(i) = index_of(w, p.1) {
                    let l = fr.local(w.entities.list[i].pos);
                    let region = if l.x >= -0.5 && l.x <= 7.0 && l.z >= -0.5 && l.z <= 8.0 { 0 } else if l.x < -0.5 && (l.z - 0.5).abs() < 1.0 { 1 } else { 2 };
                    let kind = crate::queue::rider_walk_kind(w, p.1).map_or(3, |x| x as usize);
                    rep.walk_kinds[region][kind] += 1;
                    if region >= 1 && kind < 3 && rep.walk_samples.len() < 16 && brain::has_exact(w, i) {
                        let ex = brain::ex(w, i);
                        let bl = |b: BlockPos| {
                            let q = fr.local(Vec3::new(b.x as f64 + 0.5, b.y as f64, b.z as f64 + 0.5));
                            format!("{:.0},{:.0},{:.0}", q.x - 0.5, q.y, q.z - 0.5)
                        };
                        let path = ex.mem.path.as_ref().map(|pa| format!("path to {} ending {} ({} nodes, {})", bl(pa.target), pa.nodes.last().map(|n| bl(BlockPos::new(n.x, n.y, n.z))).unwrap_or_default(), pa.nodes.len(), if pa.can_reach() { "reaching" } else { "partial" })).unwrap_or("no path".into());
                        let s = format!("{} villager {} at {:.2},{:.2},{:.2}: {path}", if region == 1 { "approach" } else { "hold" }, p.1, l.x, l.y, l.z);
                        if !rep.walk_samples.iter().any(|x| x.split(':').nth(1) == s.split(':').nth(1)) {
                            rep.walk_samples.push(s);
                        }
                    }
                }
                // (the stamp line: each villager's CANT_REACH age, by where it is)
                if let Some(i) = index_of(w, p.1)
                    && brain::has_exact(w, i)
                    && let Some(s0) = brain::ex(w, i).mem.cant_reach_since
                {
                    let l = fr.local(w.entities.list[i].pos);
                    let region = if l.x >= -0.5 && l.x <= 7.0 && l.z >= -0.5 && l.z <= 8.0 { 0 } else if l.x < -0.5 && (l.z - 0.5).abs() < 1.0 { 1 } else { 2 };
                    rep.stamp_max[region] = rep.stamp_max[region].max(t - s0);
                }
                let exposed = night_now
                    && index_of(w, p.1).is_some_and(|i| brain::has_exact(w, i) && brain::ex(w, i).active == Act::Rest && brain::ex(w, i).mem.cant_reach_since.is_some() && brain::brain(w, i).home.is_some());
                match (exposed, exp_since[k]) {
                    (true, None) => {
                        // (the phase, from the villager's latest visit at the window's start)
                        let ph = match rep.visits.iter().rev().find(|v| v.villager == p.1) {
                            Some(v) if v.dropped.is_none() && v.exited.is_none() => "before the drop-off",
                            Some(v) if v.slept.is_none() && v.reboarded.is_none() && v.exited.is_none() => "on foot in the cage before the nap",
                            Some(v) if v.reboarded.is_none() && v.exited.is_none() => "on foot after the wake",
                            Some(v) if v.reboarded.is_some() && v.exited.is_none() && v.slept.is_some() => "after the nap (re-boarded to out)",
                            Some(v) if v.reboarded.is_some() && v.exited.is_none() => "the stuck path (no nap, sent out by F)",
                            Some(v) if v.exited.is_some() && t - v.exited.unwrap() < 400 => "out, on the bench's run-out and back in its hold (no loop zombies; not counted at once)",
                            _ => "elsewhere (the bench's hold or approach start; not counted at once)",
                        };
                        exp_since[k] = Some((t, ph));
                    }
                    (false, Some((s0, ph))) => {
                        rep.exposure.push((ph.to_string(), t - s0));
                        exp_since[k] = None;
                    }
                    _ => {}
                }
            }
            // (counted at once: the siding's own windows; the bench's run-out past the exit and
            // its hold have no loop zombies (a released cart is teleported back to the hold, its
            // rider calm until the hold zombies are seen again), the loop's release run does)
            n_exp = exp_since.iter().filter(|e| e.is_some_and(|(_, ph)| !ph.starts_with("out,") && !ph.starts_with("elsewhere"))).count();
            rep.exposed_max = rep.exposed_max.max(n_exp);
            rep.exposed_two_ticks += (n_exp >= 2) as u64;
            if bo.trace && n_exp >= 2 && rep.exposed_two_ticks <= 3 {
                let who: Vec<String> = pairs.iter().enumerate().filter_map(|(k, p)| exp_since[k].map(|(s0, ph)| {
                    let c = w.entities.list.iter().find(|e| e.id == p.0 && !e.removed).map(|c| fr.local(c.pos)).unwrap_or(Vec3::new(f64::NAN, 0.0, f64::NAN));
                    format!("villager {} since {} ({ph}) cart at {:.2} {:.2} {:.2}", p.1, t - s0, c.x, c.y, c.z)
                })).collect();
                eprintln!("exposed two at gt {}: {}", t - t0, who.join("; "));
            }
        }
        if let Some(sl) = rep.seat.as_mut() {
            let w = &run.world;
            let on = w.get(seat_cell).to_string().contains("powered=true");
            // (who is in the band now: seated villagers, and any other entity)
            let mut seated: Vec<String> = Vec::new();
            let mut others: Vec<String> = Vec::new();
            for e in w.entities.list.iter().filter(|e| !e.removed && e.bb.intersects(seat_band)) {
                let villager = matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager);
                let cart = e.vehicle.and_then(|v| w.entities.list.iter().find(|x| x.id == v && !x.removed)).map(|c| {
                    let l = fr.local(c.pos);
                    format!(" in a cart at {:.3} {:.4} {:.3}", l.x, l.y, l.z)
                }).unwrap_or_default();
                let le = fr.local(e.pos);
                let what = format!("{} at {:.2} {:.4} {:.2} top {:.4}{}{cart}", if villager { "villager" } else { "entity" }, le.x, le.y, le.z, e.bb.max_y, if e.vehicle.is_some() { " seated" } else { "" });
                if villager && e.vehicle.is_some() {
                    seated.push(what);
                } else {
                    others.push(what);
                }
            }
            // (a standing villager under the string's cell: near it but not in the band)
            let near = w.entities.list.iter().any(|e| !e.removed && e.vehicle.is_none() && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager) && e.bb.min_x < seat_band.max_x && e.bb.max_x > seat_band.min_x && e.bb.min_z < seat_band.max_z + 1.0 && e.bb.max_z > seat_band.min_z);
            if near {
                sl.seen_standing += 1;
            }
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager)) {
                if let Some(c) = e.vehicle.and_then(|v| w.entities.list.iter().find(|x| x.id == v && !x.removed)) {
                    let (top, feet) = (e.bb.max_y, e.pos.y - c.pos.y);
                    sl.seated_top = Some(sl.seated_top.map_or((top, top), |(a, b)| (a.min(top), b.max(top))));
                    sl.seated_feet = Some(sl.seated_feet.map_or((feet, feet), |(a, b)| (a.min(feet), b.max(feet))));
                }
            }
            if !seated.is_empty() {
                seat_seated_last = t;
            }
            if on {
                sl.on_ticks += 1;
            }
            if on && !seat_prev {
                if !seated.is_empty() || t - seat_seated_last <= 2 {
                    sl.on_seated += 1;
                    if sl.examples.len() < 3 {
                        sl.examples.push(format!("on at t0+{}: {}", t - t0, seated.join("; ")));
                    }
                } else {
                    sl.on_other += 1;
                    sl.examples.push(format!("ON WITHOUT A SEATED VILLAGER at t0+{}: {}", t - t0, if others.is_empty() { "nothing in the band".to_string() } else { others.join("; ") }));
                }
            }
            // (a seated villager in the band two ticks running with the string off: missed)
            if !seated.is_empty() && !on && !seat_prev && t - seat_seated_last == 0 {
                sl.missed += 1;
                if sl.examples.len() < 12 {
                    sl.examples.push(format!("missed at t0+{}: {}", t - t0, seated.join("; ")));
                }
            }
            seat_prev = on;
        }
        {
            let sh = tee_shape(&run.world);
            if sh != tee_prev {
                if let Some(k) = rep.visits.len().checked_sub(1) {
                    let v = &mut rep.visits[k];
                    // (the T's shapes: the bed line to the exit, and the entry to the bed line;
                    // turned with the design)
                    let (to_exit, to_bed) = fr.tee_shapes();
                    if sh == to_exit && v.tee_out.is_none() {
                        v.tee_out = Some(t);
                    } else if sh == to_bed && v.tee_out.is_some() && v.tee_back.is_none() {
                        let x = run.world.entities.list.iter().find(|e| e.id == v.cart && !e.removed).map_or(f64::NAN, |c| fr.local(c.pos).x);
                        v.tee_back = Some((t, x));
                    }
                }
                tee_prev = sh;
            }
        }
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
                if bo.launch && !rep.visits[k].entered && fr.local(c.pos).x >= 0.5 {
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
                    // (the bed taken by another: occupied with the napper not in it)
                    if !asleep && matches!(w.get(bed), Block::Bed { occupied: true, .. }) {
                        rep.dropped_bed_taken += 1;
                    }
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
                // (farmopt 03:55: the first tick after the wake off the free cell, and the first
                // tick the kicked cart (moving north) is in the free cell)
                {
                    let fcell = layout().free_cell;
                    let in_fc = |p: Vec3| p.x >= fcell.x as f64 && p.x < fcell.x as f64 + 1.0 && p.z >= fcell.z as f64 && p.z < fcell.z as f64 + 1.0;
                    if vr.woken.is_some() && vr.left_fc.is_none() && vr.reboarded.is_none() && riding.is_none() && !asleep && !in_fc(fr.local(e.pos)) {
                        vr.left_fc = Some(t);
                    }
                    if vr.kick_at_fc.is_none()
                        && vr.dropped.is_some()
                        && let Some(c) = w.entities.list.iter().find(|x| x.id == cart && !x.removed)
                        && in_fc(fr.local(c.pos))
                        && fr.lvel(c.delta_movement).z < 0.0
                    {
                        vr.kick_at_fc = Some(t);
                    }
                    if vr.exited.is_some() && asleep && riding.is_none() {
                        vr.asleep_cart_gone = true;
                        vr.problem.get_or_insert("asleep again with its cart gone".into());
                    }
                }
                // (ironspec 03:36, the position audit: every tick the napper is awake and not
                // riding inside the siding (dropped, not yet out), it stands on the free cell)
                if vr.dropped.is_some() && vr.exited.is_none() && riding.is_none() && !asleep {
                    let l = fr.local(e.pos);
                    let fcell = layout().free_cell;
                    let on_fc = l.x >= fcell.x as f64 && l.x < fcell.x as f64 + 1.0 && l.z >= fcell.z as f64 && l.z < fcell.z as f64 + 1.0;
                    rep.audit_awake_ticks += 1;
                    if !on_fc {
                        rep.audit_off_ticks += 1;
                        if rep.audit_off.len() < 4 {
                            rep.audit_off.push(format!("visit {k} villager {villager} at {:.2} {:.2} {:.2} (local) t0+{}", l.x, l.y, l.z, t - t0));
                        }
                    }
                }
                // (ironspec 08:40, the self-retrying kick: bumps and retries, the napper at each bump)
                if vr.dropped.is_some() && vr.reboarded.is_none() && riding.is_none()
                    && let Some(c) = w.entities.list.iter().find(|x| x.id == cart && !x.removed)
                {
                    let lp = fr.local(c.pos);
                    let vz = fr.local(c.pos.add(c.delta_movement.x, 0.0, c.delta_movement.z)).z - lp.z;
                    if vr.last_vz < -0.05 && vz >= -0.005 && lp.z < 4.2 {
                        vr.bumps += 1;
                        if vr.bump_at.len() < 8 {
                            let l = fr.local(e.pos);
                            vr.bump_at.push([l.x, l.y, l.z]);
                        }
                    }
                    if vr.bumps > 0 && vr.last_vz >= -0.005 && vz < -0.01 && lp.z > 4.5 {
                        vr.retries += 1;
                    }
                    vr.last_vz = vz;
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
                && fr.local(c.pos).x >= 2.0
                && fr.local(c.pos).x < 3.0
                && fr.local(c.pos).z < 1.0
                && fr.lvel(c.delta_movement).x > 0.0
            {
                // (boards per kick: its napper aboard as it leaves the bed line)
                if c.passengers.contains(&villager) {
                    rep.kick_boarded += 1;
                } else {
                    rep.kick_empty += 1;
                }
                reset_pending = true;
                active = None;
                waiting.push(k);
                rep.visits[k].branch = Some(t);
            }
            if t - rep.visits[k].arrived > 2000 && active == Some(k) {
                rep.visits[k].problem.get_or_insert("still inside 2000 gt after its arrival".into());
                active = None;
            }
        }
        // (traced: the fault injection's stuck carts and their villagers every 3 gt in the window)
        if bo.trace && t % 3 == 0 && t - t0 >= bo.trace_window.0 && t - t0 < bo.trace_window.1 {
            for &k in &stuck {
                let (cart, villager) = (rep.visits[k].cart, rep.visits[k].villager);
                let c = w.entities.list.iter().find(|x| x.id == cart && !x.removed).map(|c| (c.pos, c.delta_movement, c.passengers.clone()));
                let vp = index_of(w, villager).map(|i| (w.entities.list[i].pos, w.entities.list[i].boarding_cooldown, w.entities.list[i].vehicle));
                if let (Some((cp, cv, ps)), Some((vp, cd, veh))) = (c, vp) {
                    eprintln!("stuck gt {}: cart {:.2} {:.2} {:.2} v {:.3} {:.3} riders {:?} | villager {:.2} {:.2} {:.2} boarding cooldown {cd} vehicle {veh:?}", t - t0, cp.x, cp.y, cp.z, cv.x, cv.z, ps, vp.x, vp.y, vp.z);
                }
            }
            if let Some(k) = active {
                let cart = rep.visits[k].cart;
                if let Some(c) = w.entities.list.iter().find(|x| x.id == cart && !x.removed) {
                    eprintln!("active gt {}: visit {k} cart {:.2} {:.2} {:.2} v {:.3} {:.3} riders {:?}", t - t0, c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.z, c.passengers);
                }
            }
        }
        // (the fault injection: a dropped visit's cart left inside, onto the exit branch at last)
        let mut freed: Vec<usize> = Vec::new();
        for &k in &stuck {
            if let Some(c) = w.entities.list.iter().find(|x| x.id == rep.visits[k].cart && !x.removed)
                && fr.local(c.pos).x >= 2.0
                && fr.local(c.pos).x < 3.0
                && fr.local(c.pos).z < 1.0
                && fr.lvel(c.delta_movement).x > 0.0
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
                ("s0", [0, 0, 5]),
                ("B1", [-2, 0, 2]),
                ("D1", [-6, -3, 3]),
                ("P", [-4, 1, 4]),
                ("Gt", [-4, 1, 3]),
                ("Gr", [-4, 1, 2]),
                ("G1", [-4, 1, 1]),
                ("g21", [-2, 1, 1]),
                ("g22", [-2, 1, 2]),
                ("CB1", [-3, 0, 2]),
                ("b42", [-4, 0, 2]),
                ("b51", [-5, -1, 2]),
                ("b62", [-6, -2, 2]),
                ("D1n", [-7, -3, 0]),
                ("Dr", [-3, -3, 4]),
                ("CN", [-3, -3, 5]),
                ("FA", [0, -6, 6]),
                ("Fs", [1, -6, 6]),
                ("B2", [0, -1, 5]),
                ("C2", [0, -1, 6]),
                ("e28", [2, -2, 8]),
                ("SCs", [1, -2, 7]),
                ("Lr", [3, -2, 6]),
                ("Ttop", [0, -3, 7]),
                ("s1", [-1, 0, 5]),
                ("s3", [-3, 0, 5]),
                ("s4", [-3, -1, 4]),
                ("s5", [-3, -2, 3]),
                ("s6", [-3, -3, 2]),
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
                ("park", if retry() { [1, -1, 5] } else { [1, 0, 6] }),
                ("act", if retry() { [1, -1, 3] } else { [1, 0, 3] }),
                ("Aa", if retry() { [1, -3, 3] } else { [1, -2, 3] }),
                ("AaR", if retry() { [0, -4, 3] } else { [0, -3, 3] }),
                ("Hs3", [-1, -4, 3]),
                ("FCh", [1, 1, 5]),
                ("FCs", [1, 1, 2]),
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
            let now_s: Vec<String> = probes.iter().map(|(n, c)| format!("{n}={}", short_state(&w.get(fr.blk(c[0], c[1], c[2])).to_string()))).collect();
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
            let lc = fr.local(c.pos);
            let on_slot = lc.x >= 4.0 && lc.x < layout().exit.x as f64 && lc.z < 1.0;
            let rest = c.delta_movement.x.abs() < 1e-3 && c.delta_movement.z.abs() < 1e-3;
            let with = c.passengers.contains(&villager);
            // (the release held: the cart runs over the slot without stopping, ready and released
            // as it reaches the slot with its villager)
            if bo.release_held {
                if lc.x >= 4.0 && lc.z < 1.0 {
                    if with {
                        rep.visits[k].ready = Some(t);
                        rep.visits[k].released = Some(t);
                    } else {
                        rep.visits[k].problem.get_or_insert("reached the slot without its villager".into());
                        if rep.visits[k].riderless.is_none() {
                            let at = index_of(w, villager).map(|i| {
                                let p = fr.local(w.entities.list[i].pos);
                                ([p.x, p.y, p.z], redsim::entity::mob::mob(w, i).sleeping_pos.is_some())
                            });
                            rep.visits[k].riderless = Some((t - t0, at.map(|a| a.0).unwrap_or([f64::NAN; 3]), at.is_some_and(|a| a.1)));
                        }
                    }
                    held_out.push(k);
                }
                continue;
            }
            if rep.visits[k].ready.is_none() && on_slot && rest && with {
                rep.visits[k].ready = Some(t);
                rep.visits[k].rest_x = Some(lc.x);
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
            let (lc, lv) = (fr.local(c.pos), fr.lvel(c.delta_movement));
            let ex = layout().exit.x as f64;
            if rep.visits[k].exit_speed.is_none() && lc.x >= ex + 0.5 {
                rep.visits[k].exit_speed = Some(lv.x);
            }
            if rep.visits[k].leave_speed.is_none() && lc.x >= ex + 1.0 {
                rep.visits[k].leave_speed = Some(lv.x);
                rep.visits[k].exited = Some(t);
            }
            if lc.x >= ex + 2.5 || t - rt > 400 {
                if lc.x < ex + 2.5 {
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
            let p = fr.pt(c.x as f64 + 0.5, 0.0625, c.z as f64 + 0.5);
            teleport(&mut run.world, cart, p, Vec3::new(0.0, 0.0, 0.0));
            // (after the tick to t: applied before the next one, the drive's act at t)
            drive.act(run.world.time, format!("tp {} {} {} {}", drive.name(cart), p.x, p.y, p.z));
        }
    }
    rep.errors = run.world.errors.iter().take(6).cloned().collect();
    rep.built_diff = built_diff;
    rep.build = built.as_ref().map(|b| (b.steps, b.cells, t0 - b.start));
    rep.merged = merges.pairs.clone();
    rep.merged_first = merges.first.clone();
    rep.derails = rail_watch.derails.clone();
    rep.rail_line = rail_watch.line();
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
        // (an unload case: the bench's args and, for a partial one, the forced chunk box its world
        // had from the start: chunks outside it sit at 31 plus their distance to it)
        if let Some(u) = &bo.unload {
            header.push(format!(
                "unload case: {} on visit {} for {} gt{}",
                if u.whole { format!("whole part, phase {}", u.state) } else { format!("chunks {:?} dropped, phase {}", u.drop, u.state) },
                u.visit,
                u.n,
                if u.whole { String::new() } else { format!("; forced chunk box {:?} (World::entity_chunks) set at t0", u.forced) }
            ));
        }
        rep.drive = Some(drive.text(&header));
    }
    rep.carts_in_out = [rep.visits.iter().filter(|v| v.arrived > 0).count() as u32, rep.visits.iter().filter(|v| v.exited.is_some()).count() as u32];
    Ok(rep)
}
