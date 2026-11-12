//! The return's layout bench (7b90c3f1, F1 + Q2 at 24, option (c); 9fc3155f's budget f1b95a5451,
//! loop frame): the merge, the shared lift and the split laid on the budget's cells, batches of three
//! ridden carts from the station pockets' +z exits through to the lanes' branch flats.
//! - merge (y 0): lanes x 7 / 3 / -1 head +z from z 1 and turn west at z 8 (lane 7's curve
//!   (7,0,8)); x 3 and x -1 join through passive Ts at (3,0,8) and (-1,0,8); the main line runs
//!   west to the lift's wall entry (-4,0,8), glass over it;
//! - shared lift: shaft (-5,y,8), soul sand (-5,-1,8), water to y 22, ceiling (-5,24,8), the exit's
//!   current from (-5,23,7) (back wall (-5,23,6)) pushing +z onto the exit curve (-5,23,9)
//!   (north_east) heading east;
//! - split (y 23 along z 9): powered junctions J(-1) at (-1,23,9) and J(3) at (3,23,9) (east_west
//!   straight, north_west to the branch), lane 7's end curve (7,23,9); each branch (x,23,8), a slope
//!   down (x,22,7) (the branch detector, two cells off its T), the branch flat (x,22,6) where the
//!   lane's intake takes the cart (the bench's sink; the speed there against ironfarm's 0.30);
//! - the split's latches in the budget's merge order (the batch's first cart to lane 7 at the end,
//!   the second to lane 3, the third to lane -1): J(3) armed by the first cart's rise at the main
//!   line's detector (5,23,9) (between J(3) and lane 7's curve, two cells off each), cleared by the
//!   second cart's rise on lane 3's branch detector; J(-1) armed by the second rise of a batch at
//!   (1,23,9) (a copper bulb toggled by each rise there, J(-1) armed on its fall: only the first two
//!   carts pass it), cleared by the third cart's rise on lane -1's branch detector.
//! Printed: each cart's lane against its own, the branch speed, the least gap from each junction's
//! turn to the next cart at it, contacts in 3-D, the shaft's closest pair, the riders.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;

/// The bench's options.
#[derive(Clone, Debug)]
pub struct ReturnOpts {
    /// Batches of three, a batch every 100 gt.
    pub batches: usize,
    /// The gt between a batch's releases (lane 7, lane 3, lane -1).
    pub spacing: i64,
    /// The release speed along +z.
    pub speed: f64,
    pub ticks: i64,
    /// FROM,LEN: the bench at level 32 (freeze32 / thaw32).
    pub login: Option<(i64, i64)>,
    /// A powered rail (on) in place of the split's first flat (-4,23,9).
    pub boost: bool,
    /// One cart's line a tick.
    pub trace: Option<usize>,
    /// The reverse merge order: x -1 first, x 3, x 7 last (the batch's first cart to the split's
    /// first exit).
    pub reverse: bool,
    /// The three releases' offsets in the batch (in merge order); default 0, spacing, 2 spacing.
    pub offsets: Vec<i64>,
    /// One release-time shift per cart (batch-major, merge order), added to its offset: the ± 2
    /// corners.
    pub jitter: Vec<i64>,
    /// The lift one lower and the split flat at the exit's level (y 22), the branches with it.
    pub flat: bool,
    /// L1: the releases run on game ticks through a freeze (default L2: on live time).
    pub game_ticks: bool,
    /// (c)'s napper: held at (8,0,8), released west at 0.4 this many gt after the arming detector's
    /// rise (on the batch's third cart's lane track, two cells off its T); one napper, behind batch 0;
    /// the split's napper window (all junctions straight to the 4th route) opens on the third
    /// cart's branch rise.
    pub napper_delay: Option<i64>,
    /// The redesigned split: the exit at y 22, a slope down to the junctions at y 21 (fast), each
    /// branch climbing back to its flat (x,22,6) (slow).
    pub dip: bool,
    /// The per-batch re-sync: the latches back at idle after this many gt with no rise on the
    /// split's detectors (0: off).
    pub resync: i64,
    /// A seeded misroute: cart K finds J(-1) the other way once.
    pub seed_miss: Option<usize>,
    /// A save, stop and start between two ticks at this tick (the lead's L1 rows).
    pub restart: Option<(i64, crate::liveness::Restart)>,
    /// 9fc3155f's z 7 shift: the merge row, the shared lift and the hold one cell north.
    pub z7: bool,
    /// ironfarm's drop entry (ironspec 01:17): the split's row along z 10 at y 24 feeding each
    /// lane's 2-cell branch (x,24,9) -> (x,24,8) (the detector) -> the merge curve (x,24,7); the
    /// lift's water to 24, the exit at 25.
    pub row10: bool,
    /// The in-chunk split (ironspec 02:13): the row along z 9 at y 25, each lane's branch one
    /// detector slope (x,24,8) down to the merge curve (x,24,7); the lift's water to 24.
    pub row9: bool,
    /// `row9`: the booster on the exit's cell (-5,25,8) instead of the row's first cell.
    pub boost_exit: bool,
    /// `row9 --boost-exit`: the lift's water to 22 and the exit at 23, the cart climbing two slopes
    /// to the row at 25 (the delivery mid-band, ironspec 04:10).
    pub climb2: bool,
    /// `climb2`: the first climbing slope a powered rail (the booster) instead of the exit's cell.
    pub boost_slope: bool,
    /// The station's real departure: three boosters (powered rails, on) on each lane's last three
    /// cells before the merge row (dfb89499); the arming detector at the station's exit detector
    /// (x,0,2).
    pub boosters: bool,
    /// A powered rail (on) right after the split's slope (-2,24,10): the row at the rail cap.
    pub row_boost: bool,
    /// Write the return's own cells as a design file (loop frame) to this path.
    pub out: Option<String>,
}

const LANES: [i32; 3] = [7, 3, -1];

fn build(o: &ReturnOpts) -> Result<FarmSpec, String> {
    let glass = Block::parse("minecraft:glass")?;
    let water = Block::parse("minecraft:water[level=0]")?;
    let rail = |s: &str| Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"));
    let det = |s: &str| Block::parse(&format!("minecraft:detector_rail[powered=false,shape={s},waterlogged=false]"));
    let mut b: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, k: &Block| b.push((BlockPos::new(x, y, z), k.clone()));
    // (the merge; `z7`: 9fc3155f's shift (ironspec 01:02), the merge row, the lift and the hold one
    // cell north (z 8 -> z 7), the split unchanged)
    let mz = if o.z7 { 7 } else { 8 };
    for x in LANES {
        for z in 1..mz {
            // ((c)'s arming detector: on the track of the batch's third cart (lane 7 in the reverse
            // order, lane -1 in the forward one), two cells off its T or curve; with `boosters` the
            // station's own exit detector (x,0,2) (dfb89499: plain z 1, the exit detector z 2, plain
            // z 3, three boosters z 4..6 into the z 7 merge row))
            let arm_lane = if o.reverse { 7 } else { -1 };
            let arm_z = if o.boosters { -100 } else { mz - 2 };
            let boost = o.boosters && z >= mz - 3;
            let r = if o.napper_delay.is_some() && x == arm_lane && z == arm_z {
                det("north_south")?
            } else if boost {
                Block::parse("minecraft:powered_rail[powered=true,shape=north_south,waterlogged=false]")?
            } else {
                rail("north_south")?
            };
            put(x, 0, z, &r);
            put(x, -1, z, &if boost { Block::parse("minecraft:redstone_block")? } else { glass.clone() });
        }
    }
    put(7, 0, mz, &rail("north_west")?);
    // ((c)'s y 0 hold (8,0,mz): its line joins lane 7's curve from the curve's unconnected east side)
    if o.napper_delay.is_some() {
        put(8, 0, mz, &rail("east_west")?);
        put(8, -1, mz, &glass);
        put(9, 0, mz, &glass);
    }
    // (the merge's Ts keep their shapes by placement order (ironspec 02:47): each placed after its two
    // intended arms (NW: the lane and the downstream row), with no signal source able to update it;
    // lane 7's curve is a T once the hold's line joins from the east, laid before that line)
    put(7, -1, mz, &glass);
    for x in -4..=6 {
        // (with `boosters` (the reverse order) the arming detector on the main line (5,0,mz), past the
        // boosters (the speed pinned), two cells off lane 7's curve and lane 3's T: only lane 7's carts
        // reach it before the napper)
        let r = if x == 3 || x == -1 { rail("north_west")? } else if o.boosters && o.napper_delay.is_some() && x == 5 { det("east_west")? } else { rail("east_west")? };
        put(x, 0, mz, &r);
        put(x, -1, mz, &glass);
    }
    put(-4, 1, mz, &glass);
    // (the shared lift: water to `top`, the exit one over it; `flat`: the lift one lower and the
    // split flat at the exit's level, the branches with it (9fc3155f: "if the split runs flat at
    // the exit's level instead, the lift and the module stay one lower"))
    let top = if o.row9 && o.climb2 { 22 } else if o.row10 || o.row9 { 24 } else if o.flat || o.dip { 21 } else { 22 };
    let ex = top + 1;
    put(-5, -1, mz, &Block::parse("minecraft:soul_sand")?);
    put(-5, -2, mz, &glass);
    for y in 0..=top {
        put(-5, y, mz, &water);
        put(-6, y, mz, &glass);
        put(-5, y, mz - 1, &glass);
        put(-5, y, mz + 1, &glass);
        if y >= 1 {
            put(-4, y, mz, &glass);
        }
    }
    put(-6, ex, mz, &glass);
    put(-4, ex, mz, &glass);
    put(-5, ex + 1, mz, &glass);
    put(-5, ex, mz - 1, &water);
    for (x, y, z) in [(-5, ex, mz - 2), (-6, ex, mz - 1), (-4, ex, mz - 1), (-5, ex + 1, mz - 1)] {
        put(x, y, z, &glass);
    }
    // (the exit: pushed +z from the top air cell; with the lift at z 7 one straight cell (-5,ex,8)
    // before the curve onto the split)
    if mz == 7 && !o.row10 && !o.row9 {
        put(-5, ex, 8, &rail("north_south")?);
    }
    // (the split along z 9: `jy` the junctions' level: the exit's (sloped / flat) or, `dip`, one
    // under it (the exit (-4,22,9) then a slope down (-3,21,9): fast through the junctions), each
    // branch climbing back up (x,22,7) (its detector) to the flat (x,22,6) (slow at the branches))
    if o.row9 {
        // (`row9` (ironspec 02:13, in-chunk): the exit at y 25 (the lift's water to 24) north to the
        // curve (-5,25,9), a powered rail on a redstone block (-4,25,9), the row along z 9 at y 25:
        // J(-1) (-1,25,9), J(3) (3,25,9), lane 7's end J(7) (7,25,9), the row's detectors (1,25,9)
        // and (5,25,9), the 4th route (8,25,9), (9,25,9); each lane's branch one cell: a detector
        // slope (x,24,8) (ascending south: up to the junction, down north to the lane's merge curve
        // (x,24,7)), the guard; diagonal to its junction, so no face of the junction touches it;
        // the junction's support (x,24,9) glass (no conductor))
        for z in mz + 1..=8 {
            put(-5, ex, z, &rail("north_south")?);
            put(-5, ex - 1, z, &glass);
        }
        put(-5, ex, 9, &rail("north_east")?);
        put(-5, ex - 1, 9, &glass);
        // (the booster: on the row's first cell, or with `boost_exit` on the exit's cell (-5,25,8), a
        // redstone block as the shaft's south wall under it, so the carts roll farther before J(-1);
        // with `climb2 --boost-slope` the first climbing slope is the booster instead)
        if o.climb2 && o.boost_slope {
            put(-4, ex, 9, &Block::parse("minecraft:powered_rail[powered=true,shape=ascending_east,waterlogged=false]")?);
            put(-4, ex - 1, 9, &Block::parse("minecraft:redstone_block")?);
            put(-3, ex + 1, 9, &rail("ascending_east")?);
            put(-3, ex, 9, &glass);
        } else if o.boost_exit {
            put(-5, ex, 8, &Block::parse("minecraft:powered_rail[powered=true,shape=north_south,waterlogged=false]")?);
            put(-5, ex - 1, 8, &Block::parse("minecraft:redstone_block")?);
            if o.climb2 {
                // (`climb2` (ironspec 04:10, the delivery mid-band): the exit two under the row, the
                // booster's cart climbing two slopes (-4,23,9) and (-3,24,9) up to the row at y 25)
                put(-4, ex, 9, &rail("ascending_east")?);
                put(-4, ex - 1, 9, &glass);
                put(-3, ex + 1, 9, &rail("ascending_east")?);
                put(-3, ex, 9, &glass);
            } else {
                put(-4, ex, 9, &rail("east_west")?);
                put(-4, ex - 1, 9, &glass);
            }
        } else {
            put(-4, ex, 9, &Block::parse("minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]")?);
            put(-4, ex - 1, 9, &Block::parse("minecraft:redstone_block")?);
        }
        for x in (if o.climb2 { -2 } else { -3 })..=6 {
            // (G1: the junctions J(-1), J(3) as the game shapes them: NE unpowered (a cart from the west
            // crosses by the free side and goes on east), NW powered (into the branch))
            let r = if x == 1 || x == 5 { det("east_west")? } else if x == -1 || x == 3 { rail("north_east")? } else { rail("east_west")? };
            put(x, 25, 9, &r);
            put(x, 24, 9, &glass);
        }
        put(7, 25, 9, &rail("north_west")?);
        put(7, 24, 9, &glass);
        // (the junctions' switches, emulated: a lever over each (face ceiling, on glass at y 27), the
        // latch's output; J(-1), J(3) laid NE (off), J(7) NW (on: lane 7) when the 4th route makes it
        // a T; the bench toggles the levers, the rails shape themselves)
        for (jx, on) in [(-1, false), (3, false), (7, true)] {
            if jx == 7 && o.napper_delay.is_none() {
                continue;
            }
            put(jx, 26, 9, &Block::parse(&format!("minecraft:lever[face=ceiling,facing=north,powered={on}]"))?);
            put(jx, 27, 9, &glass);
        }
        if o.napper_delay.is_some() {
            for x in 8..=9 {
                put(x, 25, 9, &rail("east_west")?);
                put(x, 24, 9, &glass);
            }
        }
        for x in LANES {
            put(x, 24, 8, &det("ascending_south")?);
            put(x, 23, 8, &glass);
            put(x, 24, 7, &rail("north_south")?);
            put(x, 23, 7, &glass);
        }
        return Ok(FarmSpec {
            blocks: b,
            entities: Vec::new(),
            output: BlockPos::new(10, 0, 0),
            lo: BlockPos::new(-20, -3, -2),
            hi: BlockPos::new(11, 28, 11),
            block_entities: Vec::new(),
            rock: false,
            nether: None,
            land: false,
        });
    }
    if o.row10 {
        // (`row10` (ironspec 01:17, ironfarm's drop entry): the exit at y 25 (the lift's water to
        // 24), north along x -5 to the curve (-5,25,10), a slope down (-3,24,10) onto the row along
        // z 10 at y 24: junctions J(-1) (-1,24,10), J(3) (3,24,10), lane 7's end J(7) (7,24,10),
        // the row's detectors (1,24,10) and (5,24,10); each lane's 2-cell branch (x,24,9) plain
        // (beside the junction, so no detector there) -> (x,24,8) the branch detector -> the lane's
        // merge curve (x,24,7) (the bench's sink); the 4th route (8,24,10), (9,24,10))
        for z in mz + 1..=9 {
            put(-5, ex, z, &rail("north_south")?);
            put(-5, ex - 1, z, &glass);
        }
        put(-5, ex, 10, &rail("north_east")?);
        put(-5, ex - 1, 10, &glass);
        put(-4, ex, 10, &rail("east_west")?);
        put(-4, ex - 1, 10, &glass);
        put(-3, ex - 1, 10, &rail("ascending_west")?);
        put(-3, ex - 2, 10, &glass);
        for x in -2..=6 {
            // (`row_boost`: a powered rail (on, a redstone block under it) right after the slope,
            // the row at the rail cap so each branch takes the cart at 0.26..0.40)
            let r = if o.row_boost && x == -2 {
                Block::parse("minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]")?
            } else if x == 1 || x == 5 {
                det("east_west")?
            } else if x == -1 || x == 3 {
                rail("north_east")?
            } else {
                rail("east_west")?
            };
            put(x, 24, 10, &r);
            put(x, 23, 10, &if o.row_boost && x == -2 { Block::parse("minecraft:redstone_block")? } else { glass.clone() });
        }
        put(7, 24, 10, &rail("north_west")?);
        put(7, 23, 10, &glass);
        for (jx, on) in [(-1, false), (3, false), (7, true)] {
            if jx == 7 && o.napper_delay.is_none() {
                continue;
            }
            put(jx, 25, 10, &Block::parse(&format!("minecraft:lever[face=ceiling,facing=north,powered={on}]"))?);
            put(jx, 26, 10, &glass);
        }
        if o.napper_delay.is_some() {
            for x in 8..=9 {
                put(x, 24, 10, &rail("east_west")?);
                put(x, 23, 10, &glass);
            }
        }
        for x in LANES {
            put(x, 24, 9, &rail("north_south")?);
            put(x, 23, 9, &glass);
            put(x, 24, 8, &det("north_south")?);
            put(x, 23, 8, &glass);
            put(x, 24, 7, &rail("north_south")?);
            put(x, 23, 7, &glass);
        }
        return Ok(FarmSpec {
            blocks: b,
            entities: Vec::new(),
            output: BlockPos::new(10, 0, 0),
            lo: BlockPos::new(-20, -3, -2),
            hi: BlockPos::new(11, 28, 12),
            block_entities: Vec::new(),
            rock: false,
            nether: None,
            land: false,
        });
    }
    put(-5, ex, 9, &rail("north_east")?);
    let jy = if o.dip { ex - 1 } else { ex };
    let first = if o.boost { Block::parse("minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]")? } else { rail("east_west")? };
    put(-4, ex, 9, &first);
    put(-4, ex - 1, 9, &if o.boost { Block::parse("minecraft:redstone_block")? } else { glass.clone() });
    put(-4, ex + 1, 9, &glass);
    for x in -3..=6 {
        let r = if o.dip && x == -3 { rail("ascending_west")? } else if x == 1 || x == 5 { det("east_west")? } else if x == -1 || x == 3 { rail("north_east")? } else { rail("east_west")? };
        put(x, jy, 9, &r);
        put(x, jy - 1, 9, &glass);
    }
    // (lane 7's end: a powered junction J(7) when a napper rides: north_west to lane 7, east_west on
    // to the 4th route (8,jy,9) (the napper's way out of the split; the bench's sink at (9,jy,9)))
    put(7, jy, 9, &rail("north_west")?);
    if o.napper_delay.is_some() {
        put(8, jy, 9, &rail("east_west")?);
        put(8, jy - 1, 9, &glass);
        put(9, jy, 9, &rail("east_west")?);
        put(9, jy - 1, 9, &glass);
    }
    put(7, jy - 1, 9, &glass);
    for x in LANES {
        put(x, jy, 8, &rail("north_south")?);
        put(x, jy - 1, 8, &glass);
        if o.dip {
            put(x, 21, 7, &det("ascending_north")?);
            put(x, 20, 7, &glass);
        } else if o.flat {
            put(x, ex, 7, &det("north_south")?);
            put(x, ex - 1, 7, &glass);
        } else {
            put(x, ex - 1, 7, &det("ascending_south")?);
            put(x, ex - 2, 7, &glass);
        }
        put(x, 22, 6, &rail("north_south")?);
        put(x, 21, 6, &glass);
    }
    Ok(FarmSpec {
        blocks: b,
        entities: Vec::new(),
        output: BlockPos::new(10, 0, 0),
        lo: BlockPos::new(-20, -3, -2),
        hi: BlockPos::new(11, 27, 11),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    })
}

struct Cart {
    id: Option<u32>,
    rider: Option<u32>,
    launch_live: i64,
    lane: Option<i32>,
    branch_v: Option<f64>,
    /// The stored speed entering the lane's merge cell (`row9`: after the branch slope).
    merge_v: Option<f64>,
    branch_t: Option<i64>,
    least_air: i32,
    hurt: f32,
    health0: Option<f32>,
    eye_in_wall: u32,
    dismounted: Option<i64>,
    met_shaft: Option<i64>,
    /// The rider's feet at y 12.01 (9fc3155f's summon-box exit), live gt after release.
    reach12: Option<i64>,
    /// The rider's highest head y (feet + 1.95).
    head: f64,
    /// The speed pin (ironspec 01:19): the least stored horizontal speed and the least per-tick
    /// horizontal displacement on the merge row (y 0, from the row to the shaft) and on the split's
    /// row; the last position.
    merge_min: Option<(f64, f64)>,
    split_min: Option<(f64, f64)>,
    prev: Option<(f64, f64, f64)>,
    end: Option<(f64, f64, f64)>,
}

/// The game's `isInWall` for a rider: its eye box (0.48 square at the eye, 1.62 over its feet)
/// meeting a suffocating block's collision shape (soul sand 14/16, full cubes; glass never).
fn in_wall(w: &redsim::world::World, x: f64, feet: f64, z: f64) -> bool {
    let ey = feet + 1.62;
    for cx in (x - 0.24).floor() as i32..=(x + 0.24).floor() as i32 {
        for cz in (z - 0.24).floor() as i32..=(z + 0.24).floor() as i32 {
            let cp = BlockPos::new(cx, ey.floor() as i32, cz);
            let blk = w.get(cp);
            let name = blk.to_string();
            let boxes = redsim::behavior::collision_boxes(&blk);
            let suffocating = name.contains("soul_sand") || (boxes.len() == 1 && boxes[0] == [0, 16, 0, 16, 0, 16] && !name.contains("glass"));
            if suffocating
                && boxes.iter().any(|bx| {
                    x + 0.24 > cx as f64 + bx[0] as f64 / 16.0
                        && x - 0.24 < cx as f64 + bx[1] as f64 / 16.0
                        && ey > cp.y as f64 + bx[2] as f64 / 16.0
                        && ey < cp.y as f64 + bx[3] as f64 / 16.0
                        && z + 0.24 > cz as f64 + bx[4] as f64 / 16.0
                        && z - 0.24 < cz as f64 + bx[5] as f64 / 16.0
                })
            {
                return true;
            }
        }
    }
    false
}

/// One junction driven (G1, CHANGES 3 on 261a90ca3): with `real` the junction's support block under
/// it (x, y - 1, z) is a redstone block while powered and glass while not, and the game's own rail
/// update shapes the T (NW powered, NE unpowered: `RailBlock.updateState` -> `updateDir`); without,
/// the shape set directly (the older splits).
fn drive_j(w: &mut redsim::world::World, real: bool, x: i32, y: i32, z: i32, on: bool) -> Result<(), String> {
    if real {
        // (a lever over the junction (face ceiling, on the glass two over it): its toggle is a signal
        // source's update, so the rail runs its own `updateDir` with the power it now sees)
        let b = Block::parse(&format!("minecraft:lever[face=ceiling,facing=north,powered={on}]"))?;
        w.set_block(BlockPos::new(x, y + 1, z), b, redsim::world::flags::ALL);
    } else {
        let s = if on { "north_west" } else { "north_east" };
        w.set_block(BlockPos::new(x, y, z), Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"))?, redsim::world::flags::ALL);
    }
    Ok(())
}

/// One run's failure counts and margins (the sweeps' verdict per row).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
    pub wrong: usize,
    pub pushes: usize,
    /// The napper not on the 4th route (with `napper_delay`).
    pub napper_off: bool,
    /// Riders hurt, with an eye-in-wall tick, or dismounted.
    pub riders_bad: usize,
    /// With `restart`: the entities the restart brought back under new ids (0: nothing reloaded).
    pub restarted: usize,
    /// The least shaft-entry gap between a batch's carts (live gt).
    pub gap_min: i64,
    /// The napper's shaft-entry gap behind batch 0's third cart and ahead of batch 1's first.
    pub napper_behind: Option<i64>,
    pub napper_ahead: Option<i64>,
    /// The batch carts' stored speed entering their branch (min, max) and entering the lane's merge
    /// cell (min, max).
    pub v_branch: (f64, f64),
    pub v_merge: (f64, f64),
    /// The riders' highest head y.
    pub head: f64,
}

impl Summary {
    pub fn clean(&self) -> bool {
        self.wrong == 0 && self.pushes == 0 && !self.napper_off && self.riders_bad == 0
    }

    /// Every bar (ironspec): clean, the shaft-entry gaps >= 21, the napper >= 28 behind and >= 21
    /// ahead, the heads under the roof's underside at 28, each lane's delivery (stored, entering its
    /// merge cell) inside ironfarm's 0.30..0.48.
    pub fn bars(&self) -> bool {
        self.clean()
            && self.gap_min >= 21
            && self.napper_behind.is_none_or(|g| g >= 28)
            && self.napper_ahead.is_none_or(|g| g >= 21)
            && self.head < 28.0
            && (!self.v_merge.0.is_finite() || (self.v_merge.0 >= 0.30 && self.v_merge.1 <= 0.48))
    }
}

/// One run's report.
pub fn return_bench(o: &ReturnOpts) -> Result<String, String> {
    return_run(o).map(|(s, _)| s)
}

/// Runs `jobs` (a label and the options for each row) on 16 threads; the rows sorted by label.
fn run_rows(jobs: Vec<(String, ReturnOpts)>) -> Result<Vec<(String, Result<Summary, String>)>, String> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::<(usize, String, Result<Summary, String>)>::new());
    std::thread::scope(|s| {
        for _ in 0..16 {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((label, oi)) = jobs.get(i) else { break };
                    let r = return_run(oi).map(|(_, s)| s);
                    if let Ok(mut v) = results.lock() {
                        v.push((i, label.clone(), r));
                    }
                }
            });
        }
    });
    let mut v = results.into_inner().map_err(|e| e.to_string())?;
    v.sort_by_key(|r| r.0);
    Ok(v.into_iter().map(|(_, l, r)| (l, r)).collect())
}

/// The rows' verdict against every bar ([`Summary::bars`]) and the ranges over them: the least
/// shaft-entry gap, the napper's least gaps, the branch and merge-cell speeds, the highest head.
fn verdict(v: &[(String, Result<Summary, String>)]) -> String {
    let ok: Vec<&Summary> = v.iter().filter_map(|(_, r)| r.as_ref().ok()).collect();
    let bad: Vec<String> = v
        .iter()
        .filter(|(_, r)| !r.as_ref().is_ok_and(Summary::bars))
        .map(|(l, r)| match r {
            Ok(s) => format!(
                "{l}: wrong {} pushes {} napper {} riders {} gap {} napper {:?}/{:?} head {:.2}",
                s.wrong,
                s.pushes,
                if s.napper_off { "OFF" } else { "ok" },
                s.riders_bad,
                s.gap_min,
                s.napper_behind,
                s.napper_ahead,
                s.head
            ),
            Err(e) => format!("{l}: error {e}"),
        })
        .collect();
    let lo = |f: &dyn Fn(&Summary) -> f64| ok.iter().map(|s| f(s)).fold(f64::INFINITY, f64::min);
    let hi = |f: &dyn Fn(&Summary) -> f64| ok.iter().map(|s| f(s)).fold(f64::NEG_INFINITY, f64::max);
    let nap = |f: &dyn Fn(&Summary) -> Option<i64>| ok.iter().filter_map(|s| f(s)).min().map_or("-".into(), |g| g.to_string());
    format!(
        "{} rows; the least shaft-entry gap {}; the napper's least gap behind {} and ahead {}; branch entry stored {:.3}..{:.3}, merge-cell entry {:.3}..{:.3}; the highest head {:.3}; rows with a wrong lane, a push, the napper off its route or a rider hurt: {}; rows under a nominal bar: {}{}",
        v.len(),
        ok.iter().map(|s| s.gap_min).min().unwrap_or(i64::MIN),
        nap(&|s: &Summary| s.napper_behind),
        nap(&|s: &Summary| s.napper_ahead),
        lo(&|s: &Summary| s.v_branch.0),
        hi(&|s: &Summary| s.v_branch.1),
        lo(&|s: &Summary| s.v_merge.0),
        hi(&|s: &Summary| s.v_merge.1),
        hi(&|s: &Summary| s.head),
        v.iter().filter(|(_, r)| !r.as_ref().is_ok_and(Summary::clean)).count(),
        bad.len(),
        if bad.is_empty() { String::new() } else { format!(": {}", bad.iter().take(30).cloned().collect::<Vec<_>>().join("; ")) }
    )
}

/// The restart sweep (ironspec's L1 rows, G1): the bench once per tick T in `from..=to` and per
/// order in `liveness::Restart::rows()` (save, reverse, seed:1..8), on 16 threads; the rows against
/// every bar, and the same configuration without a restart.
pub fn restart_sweep(o: &ReturnOpts, from: i64, to: i64) -> Result<String, String> {
    let quiet = |r: Option<(i64, crate::liveness::Restart)>| ReturnOpts { restart: r, trace: None, out: None, ..o.clone() };
    let (_, base) = return_run(&quiet(None))?;
    let rows = crate::liveness::Restart::rows();
    let jobs: Vec<(String, ReturnOpts)> = (from..=to).flat_map(|t| rows.iter().map(move |r| (t, *r))).map(|(t, r)| (format!("t {t} {}", r.label()), quiet(Some((t, r))))).collect();
    let v = run_rows(jobs)?;
    let reloaded = v.iter().filter(|(_, r)| r.as_ref().is_ok_and(|s| s.restarted > 0)).count();
    Ok(format!(
        "restart sweep (liveness::restart, a save/stop/start between two ticks): every tick {from}..={to} x {} orders ({}), {reloaded} rows reloading entities under new ids; the run without a restart {}; {}",
        rows.len(),
        rows.iter().map(|r| r.label()).collect::<Vec<_>>().join(", "),
        if base.bars() { "meets every bar".to_string() } else { format!("FAILS ({base:?})") },
        verdict(&v)
    ))
}

/// The ± 2 corners: every pattern of release shifts in {-2, 0, +2} over batches 0 and 1's carts
/// (3^6 = 729 rows; batch 2 nominal), on 16 threads, against every bar.
pub fn jitter_sweep(o: &ReturnOpts) -> Result<String, String> {
    let mut jobs = Vec::new();
    for code in 0..729usize {
        let mut j = Vec::new();
        let mut c = code;
        for _ in 0..6 {
            j.push([-2i64, 0, 2][c % 3]);
            c /= 3;
        }
        let label = format!("jitter {}", j.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(","));
        jobs.push((label, ReturnOpts { jitter: j, trace: None, out: None, ..o.clone() }));
    }
    let v = run_rows(jobs)?;
    Ok(format!("jitter sweep (each release of batches 0 and 1 shifted -2, 0 or +2 gt): {}", verdict(&v)))
}

/// Whether a block is a signal source (a component that powers its neighbours).
fn is_source(name: &str) -> bool {
    ["detector_rail", "lever", "redstone_block", "redstone_torch", "repeater", "comparator", "observer", "button", "pressure_plate", "tripwire_hook", "daylight_detector", "redstone_wire", "target", "trapped_chest"]
        .iter()
        .any(|k| name.contains(k))
}

/// `--out`: the return's own cells as a design file (loop frame) with the emitter's static gates
/// (ironspec 03:28/03:37): parse(write()) gives the same cells; no cell would pop (a rail without a
/// rigid block under it, a lever without its block); every rail's natural shape (what the game
/// settles it to with today's neighbours and power) against its designed one, the differences
/// listed as placement-order cells; every T (a plain rail with three rail sides) with its arms, its
/// two power states and the signal sources at its faces (only its declared switch: a lever over it,
/// the bench's latch output); every detector's face neighbours and the block under it with any T
/// among their faces. The merge row's Ts and the split's junctions are the bench's own.
fn write_design(o: &ReturnOpts, spec: &FarmSpec, w: &redsim::world::World, path: &str, mz: i32) -> Result<(), String> {
    use redsim::pos::Dir;
    let mut d = mctas::design::Design { name: "f1q2-lift-split".into(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    for (p, blk) in &spec.blocks {
        let station = p.y <= 0 && p.z < mz && LANES.contains(&p.x);
        let branch = LANES.contains(&p.x) && (23..=24).contains(&p.y) && ((o.row10 && (7..=9).contains(&p.z)) || (o.row9 && p.z == 7));
        let stop = p.x == 9 && p.y == 0 && p.z == mz;
        if station || branch || stop {
            continue;
        }
        d.blocks.insert([p.x, p.y, p.z], w.get(*p).to_string());
    }
    let mut gates: Vec<String> = Vec::new();
    let mut fail: Vec<String> = Vec::new();
    let mut place_order: Vec<String> = Vec::new();
    let mut ts: Vec<String> = Vec::new();
    let cell = |p: BlockPos| format!("({},{},{})", p.x, p.y, p.z);
    for (k, name) in &d.blocks {
        let p = BlockPos::new(k[0], k[1], k[2]);
        let s = w.get(p);
        if name.contains("rail") && !redsim::rails::can_survive(w, p) {
            fail.push(format!("{} {name} would pop (no rigid block under it)", cell(p)));
        }
        if name.contains("lever") && matches!(w.get(p.above()), Block::Air) {
            fail.push(format!("{} lever would pop (no block over it)", cell(p)));
        }
        if !redsim::rails::is_rail(&s) {
            continue;
        }
        let stored = redsim::rails::shape_of(&s);
        let declared = name.contains("rail[") && d.blocks.get(&[k[0], k[1] + 1, k[2]]).is_some_and(|b| b.contains("lever"));
        if let Some(j) = redsim::rails::junction_at(w, p) {
            let sources: Vec<String> = Dir::ALL
                .iter()
                .map(|dd| p.rel(*dd))
                .filter(|q| is_source(&w.get(*q).to_string()))
                .map(|q| format!("{} {}", w.get(q).to_string().trim_start_matches("minecraft:"), cell(q)))
                .collect();
            let undeclared: Vec<&String> = sources.iter().filter(|s| !(declared && s.starts_with("lever"))).collect();
            let verdict = if declared {
                format!("switched by its declared lever (NW on, NE off; now {})", stored.name())
            } else if stored == j.unpowered && !j.signal {
                "settled (unpowered)".to_string()
            } else {
                format!("placement order (settles {} unpowered): lay it after its two designed arms, before the third", j.unpowered.name())
            };
            if !undeclared.is_empty() {
                fail.push(format!("T {} has an undeclared source: {}", cell(p), undeclared.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
            }
            ts.push(format!(
                "T {}: arms {}, designed {}, unpowered {}, powered {}; sources at its faces: {}; {verdict}",
                cell(p),
                j.arms.iter().map(|a| a.name()).collect::<Vec<_>>().join("/"),
                stored.name(),
                j.unpowered.name(),
                j.powered.name(),
                if sources.is_empty() { "none".to_string() } else { sources.join(", ") }
            ));
        } else if !declared {
            let natural = redsim::rails::RailState::new(p, s).shape_for(w, w.has_neighbor_signal(p), stored);
            if natural != stored {
                place_order.push(format!("{} designed {}, natural {}: placement order", cell(p), stored.name(), natural.name()));
            }
        }
        if name.contains("detector_rail") {
            for c in [p, p.below()] {
                for dd in Dir::ALL {
                    let q = c.rel(dd);
                    if q != p && redsim::rails::junction_at(w, q).is_some() && !d.blocks.get(&[q.x, q.y + 1, q.z]).is_some_and(|b| b.contains("lever")) {
                        fail.push(format!("detector {}: an undeclared T {} beside {}", cell(p), cell(q), cell(c)));
                    }
                }
            }
        }
    }
    let rows = if o.row9 {
        "# - split (in the chunk): the exit at y 25 north to the curve (-5,25,9); a powered rail on a redstone block (the exit cell (-5,25,8) with --boost-exit, else (-4,25,9)); the row along z 9 at y 25: J(-1) (-1,25,9), J(3) (3,25,9), J(7) (7,25,9), the row's detectors (1,25,9) and (5,25,9), the 4th route (8,25,9), (9,25,9); each lane's branch the detector slope (x,24,8) ascending south (the guard and the latch, diagonal to its junction), into ironfarm's merge cell (x,24,7) (not here); (x,24,9) the junction's glass support."
    } else {
        "# - split: the row along z 10 at y 24 (OUTSIDE the chunk), the slope (-3,24,10), a powered rail (-2,24,10) with --row-boost; J(-1) (-1,24,10), J(3) (3,24,10), J(7) (7,24,10); the row's detectors (1,24,10), (5,24,10); the 4th route (8,24,10), (9,24,10); the lanes' branches (x,24,9) -> the detector (x,24,8) -> (x,24,7) are ironfarm's (not here)."
    };
    let mut head: Vec<String> = vec![
        "# The return for F1 + Q2 at 24 (7b90c3f1; bench, tool unreviewed): loop frame, paste with no move. Made by `mctas mobfarm return-bench --out` from the bench's own world.".into(),
        "# - merge row along z 7: lane 7's curve (7,0,7) (a T once the hold's line joins from (8,0,7)), Ts (3,0,7) and (-1,0,7) NW, the main line to the wall entry (-4,0,7), the arming detector (5,0,7) with the napper; the station's tracks and boosters (z 1..6) are dfb89499's, not here.".into(),
        "# - shared lift: shaft (-5,y,7), soul sand (-5,-1,7), water to y 24, glass walls (panes to come), the exit current's source (-5,25,6) boxed, the exit at y 25.".into(),
        rows.into(),
        "# - switches EMULATED: a lever over each junction (face ceiling, glass over it) is the latch's output; the napper's hold gate (8,0,7) EMULATED (the bench releases the napper D gt after (5,0,7)'s rise). The napper lift (c) is not in this file yet.".into(),
        "# - latches (merge order x -1, 3, 7): J(-1) on at idle; lane -1's branch rise turns it off and J(3) on; lane 3's turns J(3) off; the third cart's rise at the row's second detector turns J(-1) on, or opens the napper window (J(7) off on lane 7's branch rise; J(-1) on at the napper's own first rise at the row's first detector).".into(),
    ];
    head.extend(ts.iter().map(|t| format!("# {t}")));
    head.push(format!("# placement-order cells (not Ts): {}", if place_order.is_empty() { "none".to_string() } else { place_order.join("; ") }));
    let text = format!("{}\n{}", head.join("\n"), d.to_text());
    let back = mctas::design::Design::parse(&text).map_err(|e| format!("parse(write()): {e}"))?;
    if back.blocks != d.blocks {
        fail.push(format!("parse(write()) gives {} cells against {} written", back.blocks.len(), d.blocks.len()));
    }
    gates.push(format!("parse(write()): {} cells, the same", d.blocks.len()));
    if !fail.is_empty() {
        return Err(format!("--out {path}: the static gates fail: {}", fail.join("; ")));
    }
    std::fs::write(path, &text).map_err(|e| format!("writing {path}: {e}"))?;
    eprintln!("--out {path}: {}; 0 would-pop; {} T(s), 0 undeclared sources; placement-order cells {}", gates.join("; "), ts.len(), place_order.len());
    for t in &ts {
        eprintln!("  {t}");
    }
    for p in &place_order {
        eprintln!("  {p}");
    }
    Ok(())
}

/// One run: the report and its failure counts.
fn return_run(o: &ReturnOpts) -> Result<(String, Summary), String> {
    let mz = if o.z7 { 7 } else { 8 };
    let spec = build(o)?;
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, (o.ticks + 200) as u32, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    // (`out`: the return's own cells as a design file in the loop frame (onechunk's assembly), with
    // the emitter's static gates on the bench's settled world (ironspec 03:28): parse(write()), 0
    // would-pop cells, every rail's natural shape against its designed one (the placement-order
    // list), every T's verdict and the signal sources at it, the detectors' neighbours)
    if let Some(path) = &o.out {
        write_design(o, &spec, &run.world, path, mz)?;
    }
    let column = (0..=22).filter(|y| matches!(run.world.get(BlockPos::new(-5, *y, mz)), Block::BubbleColumn { drag: false })).count();
    let mut out = vec![format!(
        "return bench (budget f1b95a5451 cells): {} batch(es) of 3, releases {} gt apart at {:.2}, a batch every 100 gt; the shared lift's bubble cells after settling {column} of 23{}",
        o.batches,
        o.spacing,
        o.speed,
        if o.boost { "; a powered rail at (-4,23,9)" } else { "" }
    )];
    let n = o.batches * 3;
    // (the split's junction level: the exit's (23; `flat` 22) or `dip`'s main line one under it)
    let ex = if o.row9 { 25 } else if o.row10 { 24 } else if o.dip { 21 } else if o.flat { 22 } else { 23 };
    // (the row's z (`row10`: z 10), the branch detectors' z, where the branch speed is read and the sink)
    let rz = if o.row10 { 10 } else { 9 };
    let bdz = if o.row10 || o.row9 { 8 } else { 7 };
    let (vin_y, vin_z, sink_y, sink_z) = if o.row9 { (24, 8, 24, 7) } else if o.row10 { (24, 9, 24, 7) } else { (22, 6, 22, 6) };
    // (the branch detectors' level: `dip`'s climbing slopes (x,21,7); the others' (x,22,7))
    let bdy = if o.row10 || o.row9 { 24 } else if o.dip { 21 } else { 22 };
    let total = n + usize::from(o.napper_delay.is_some());
    let mut carts: Vec<Cart> = (0..total).map(|_| Cart { id: None, rider: None, launch_live: 0, lane: None, branch_v: None, merge_v: None, branch_t: None, least_air: i32::MAX, hurt: 0.0, health0: None, eye_in_wall: 0, dismounted: None, met_shaft: None, reach12: None, head: f64::NEG_INFINITY, merge_min: None, split_min: None, prev: None, end: None }).collect();
    let arm_lane = if o.reverse { 7 } else { -1 };
    let (mut arm_was, mut arm_at, mut window, mut napper_past) = (false, None::<i64>, false, false);
    let mut met_abs: Vec<Option<i64>> = vec![None; total];
    let shape = |s: &str| Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"));
    let on = |w: &redsim::world::World, x: i32, y: i32, z: i32| matches!(w.get(BlockPos::new(x, y, z)), Block::DetectorRail { powered: true, .. });
    // (the latches: L3 = J(3) to lane 3, Lm = J(-1) to lane -1, the bulb toggled by (1,23,9))
    let order: [i32; 3] = if o.reverse { [-1, 3, 7] } else { LANES };
    let offsets: Vec<i64> = if o.offsets.len() == 3 { o.offsets.clone() } else { vec![0, o.spacing, 2 * o.spacing] };
    // (the reverse order starts with J(-1) turned for the batch's first cart)
    let (mut l3, mut lm, mut bulb) = (false, o.reverse, false);
    if o.reverse {
        drive_j(&mut run.world, o.row9 || o.row10, -1, ex, rz, true)?;
    }
    let mut was = [false; 5];
    let (mut last_rise, mut resyncs, mut seeded) = (0i64, Vec::<i64>::new(), false);
    let mut turns: Vec<(i64, i32, bool)> = Vec::new();
    let mut at_j: Vec<(i64, i32, usize)> = Vec::new();
    let mut frozen_ticks = 0i64;
    let mut freeze_box: Option<[i32; 4]> = None;
    let mut touches = crate::liftbench::Touches::default();
    let mut trace: Vec<String> = Vec::new();
    let mut shaft_gap = f64::INFINITY;
    let mut restarted = 0usize;
    let (mut t_offset, mut t_cross, mut t_det) = (0.0f64, 0usize, Vec::<String>::new());
    for t in 0..o.ticks {
        let frozen = o.login.is_some_and(|(f, l)| t >= f && t < f + l);
        // (G1: `liveness::freeze32` with the `levels` assert (pearl-alternative rows only))
        if let Some((f, l)) = o.login {
            if t == f {
                let pb = crate::liveness::freeze32(&mut run.world);
                let (lo, hi) = crate::liveness::levels(&run.world, pb);
                if (lo, hi) != (32, 32) {
                    return Err(format!("the freeze left the part's chunks {pb:?} at levels {lo}..{hi}, not 32"));
                }
                freeze_box = Some(pb);
            }
            if t == f + l
                && let Some(pb) = freeze_box
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        if frozen {
            frozen_ticks += 1;
        }
        let live = t - frozen_ticks;
        for (k, c) in carts.iter_mut().enumerate() {
            if k >= n {
                continue;
            }
            let at = (k as i64 / 3) * 100 + offsets[k % 3] + o.jitter.get(k).copied().unwrap_or(0);
            if c.id.is_none() && (o.game_ticks || !frozen) && (if o.game_ticks { t } else { live }) >= at {
                let x = order[k % 3];
                let nbt = format!("{{Motion:[0.0d,0.0d,{:.4}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", o.speed);
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x as f64 + 0.5, 0.0625, 1.5], Some(&nbt))?;
                c.id = Some(id);
                c.launch_live = live;
                c.rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
        }
        // ((c)'s napper: the arming detector's first rise (the batch 0 third cart), the hold's
        // release D gt after it (a repeater chain: game ticks in L1, live in L2), the napper
        // leaving (8,0,8) west at 0.4)
        if let Some(d) = o.napper_delay {
            let a = if o.boosters { on(&run.world, 5, 0, mz) } else { on(&run.world, arm_lane, 0, mz - 2) };
            if a && !arm_was && arm_at.is_none() {
                arm_at = Some(if o.game_ticks { t } else { live });
            }
            arm_was = a;
            let clock = if o.game_ticks { t } else { live };
            if let Some(a0) = arm_at
                && carts[n].id.is_none()
                && (o.game_ticks || !frozen)
                && clock >= a0 + d
            {
                let nbt = "{Motion:[-0.4d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [8.5, 0.0625, mz as f64 + 0.5], Some(nbt))?;
                carts[n].id = Some(id);
                carts[n].launch_live = live;
                carts[n].rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
        }
        // (the split's latches on the detectors' rises; the junctions set to them; detectors: [0]
        // (5,23,9), [1] (1,23,9), [2] lane 3's branch, [3] lane -1's, [4] lane 7's)
        let now = [on(&run.world, 5, ex, rz), on(&run.world, 1, ex, rz), on(&run.world, 3, bdy, bdz), on(&run.world, -1, bdy, bdz), on(&run.world, 7, bdy, bdz)];
        // (`row9`: each press and release of a detector slope (x,24,8) re-settles its junction
        // (x,25,9) from the power it sees (`BaseRailBlock.affectNeighborsAfterRemoval`: a sloped
        // rail's state change updates the faces of the cell over it; redsim does it, tick-exact's
        // rail-t scene 0 differences): the junctions hold only the shapes their declared switches give)
        let (p3, pm) = (l3, lm);
        let rise = |i: usize| now[i] && !was[i];
        if o.reverse {
            // (the reverse merge order: the batch's first cart to lane -1 (J(-1) turned at idle),
            // the second to lane 3, the third to lane 7 at the end: lane -1's branch rise clears
            // J(-1) and turns J(3); lane 3's clears J(3); lane 7's turns J(-1) for the next batch)
            if rise(3) {
                lm = false;
                l3 = true;
            }
            if rise(2) {
                l3 = false;
            }
            // (the third cart's rise at (5,jy,9), past J(3) (only it reaches there): J(-1) turned for
            // the next batch, or, with the napper still to come, the napper window opens (J(-1) and
            // J(3) stay straight); lane 7's branch rise then sends J(7) on to the 4th route)
            if rise(0) {
                if o.napper_delay.is_some() && carts[n].lane.is_none() && !window && carts[n].id.is_some() {
                    window = true;
                } else if !window {
                    lm = true;
                }
            }
            if rise(4) && window {
                drive_j(&mut run.world, o.row9 || o.row10, 7, ex, rz, false)?;
            }
            // (the napper's rise at (1,jy,9), past J(-1) (the window's first rise there): J(-1)
            // turned at once for the next batch, whose first cart may reach it before the napper is
            // out of the split; later rises there are the next batch's)
            if rise(1) && window && !napper_past {
                lm = true;
                napper_past = true;
            }
        } else {
            if rise(0) {
                l3 = true;
            }
            if rise(1) {
                let before = bulb;
                bulb = !bulb;
                if before && !bulb {
                    lm = true;
                }
            }
            if rise(2) {
                l3 = false;
            }
            if rise(3) {
                lm = false;
            }
        }
        // (the per-batch re-sync (ironspec 00:30): `resync` gt with no rise on any of the split's
        // detectors (longer than a batch's spread, shorter than the gap to the next batch) puts the
        // latches back at idle, so one misroute heals on the next batch)
        if (0..5).any(|i| now[i] && !was[i]) {
            last_rise = t;
        }
        if o.resync > 0 && t - last_rise == o.resync && !window {
            let idle = if o.reverse { (false, true) } else { (false, false) };
            if (l3, lm) != idle || bulb {
                resyncs.push(t);
            }
            (l3, lm) = idle;
            bulb = false;
        }
        // (`seed_miss`: cart K finds J(-1) the other way once, a seeded misroute)
        if let Some(k) = o.seed_miss
            && !seeded
            && let Some(Some(s)) = carts.get(k).map(|c| c.id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| run.world.entities.list[i].pos))
            && s.y.floor() as i32 == ex
            && (s.z.floor() as i32) == rz
            && s.x >= -2.6
            && s.x < -1.6
        {
            lm = !lm;
            seeded = true;
        }
        was = now;
        if l3 != p3 {
            drive_j(&mut run.world, o.row9 || o.row10, 3, ex, rz, l3)?;
            turns.push((t, 3, l3));
        }
        if lm != pm {
            drive_j(&mut run.world, o.row9 || o.row10, -1, ex, rz, lm)?;
            turns.push((t, -1, lm));
        }
        // (`restart`: a save, stop and start between two ticks (the lead's L1 rows), through the
        // shared helper (cc767b78's `liveness::restart`, in the given entity order); the carts and
        // riders followed to their new ids)
        if let Some((rt, how)) = o.restart
            && rt == t
        {
            let moved = crate::liveness::restart(&mut run.world, how)?;
            restarted = moved.iter().filter(|(a, b)| a != b).count();
            for c in carts.iter_mut() {
                c.id = c.id.map(|i| crate::liveness::follow(&run.world, i));
                c.rider = c.rider.map(|i| crate::liveness::follow(&run.world, i));
            }
        }
        run.tick();
        let mut sink: Vec<u32> = Vec::new();
        let mut close_window = false;
        let st: Vec<Option<crate::liftbench::CartState>> = carts.iter().map(|c| c.id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| crate::liftbench::CartState::of(&run.world.entities.list[i]))).collect();
        for a in 0..total {
            for bq in a + 1..total {
                if let (Some(pa), Some(pb)) = (st[a], st[bq]) {
                    if pa.x < -4.0 && pb.x < -4.0 && pa.z.floor() as i32 == mz && pb.z.floor() as i32 == mz {
                        shaft_gap = shaft_gap.min(((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2) + (pa.z - pb.z).powi(2)).sqrt());
                    }
                    if let Some((push, closing)) = crate::liftbench::touch(&pa, &pb) {
                        touches.note(t, a, bq, &pa, push, closing);
                    }
                }
            }
        }
        for (k, c) in carts.iter_mut().enumerate() {
            let Some(s) = st[k] else { continue };
            let (x, y, z, vx, vz) = (s.x, s.y, s.z, s.vx, s.vz);
            let id = c.id.unwrap_or(0);
            let bp = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
            c.end = Some((x, y, z));
            // (the speed pin: on the merge row (y 0, z mz, x -4..7) and on the split's row (y ex, z
            // rz), the least stored speed and the least per-tick displacement)
            let hs = (vx * vx + vz * vz).sqrt();
            let disp = c.prev.map(|(px, py, pz)| if (py - y).abs() < 0.5 { ((x - px).powi(2) + (z - pz).powi(2)).sqrt() } else { f64::INFINITY });
            if let Some(d) = disp
                && d.is_finite()
            {
                let on_merge = bp.y == 0 && bp.z == mz && bp.x >= -4 && bp.x <= 7;
                let on_row = bp.y == ex && bp.z == rz && bp.x >= -2 && bp.x <= 7;
                let slot = if on_merge { Some(&mut c.merge_min) } else if on_row { Some(&mut c.split_min) } else { None };
                if let Some(m) = slot {
                    *m = Some(m.map_or((hs, d), |(a, b)| (a.min(hs), b.min(d))));
                }
            }
            c.prev = Some((x, y, z));
            if o.trace == Some(k) {
                trace.push(format!("  t {t}: cart {k} at {x:.3},{y:.3},{z:.3} v {:.3},{:.3},{:.3} on {}", s.vx, s.vy, s.vz, run.world.get(BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32)).to_string().trim_start_matches("minecraft:")));
            }
            if c.met_shaft.is_none() && bp.x == -5 && bp.z == mz {
                c.met_shaft = Some(live - c.launch_live);
                met_abs[k] = Some(live);
            }
            // (G1 / ironspec: every cart on a T (the merge's (3,0,mz), (-1,0,mz), lane 7's curve with the
            // hold, the split's J(-1), J(3), J(7)): its sideways offset from the row's line, and any
            // branch detector (x,bdy,bdz) of another lane its box meets (a through cart tripping it))
            let on_t = (bp.y == 0 && bp.z == mz && [3, -1, 7].contains(&bp.x)) || (bp.y == ex && bp.z == rz && [3, -1, 7].contains(&bp.x));
            if on_t {
                let line_z = bp.z as f64 + 0.5;
                t_offset = t_offset.max((z - line_z).abs());
                t_cross += 1;
            }
            for lx in LANES {
                let (dx0, dz0) = (lx as f64, bdz as f64);
                let meets = x + 0.49 > dx0 && x - 0.49 < dx0 + 1.0 && z + 0.49 > dz0 && z - 0.49 < dz0 + 1.0 && (y - bdy as f64).abs() < 1.0;
                if meets && k < n && order[k % 3] != lx {
                    t_det.push(format!("cart {k} on lane {lx}'s detector at t {t}"));
                }
            }
            if bp.y == ex && bp.z == rz && (bp.x == 3 || bp.x == -1) && at_j.last().is_none_or(|l| l.2 != k || l.1 != bp.x) {
                at_j.push((t, bp.x, k));
            }
            // (the branch speed where the lane takes it (`row10`: entering (x,24,9), ironfarm's v_in),
            // the sink at the branch's end (`row10`: the lane's merge curve (x,24,7)))
            if bp.y == vin_y && bp.z == vin_z && LANES.contains(&bp.x) && c.branch_v.is_none() {
                c.branch_v = Some((vx * vx + vz * vz).sqrt());
            }
            if bp.y == sink_y && bp.z == sink_z && LANES.contains(&bp.x) && c.lane.is_none() {
                c.lane = Some(bp.x);
                c.merge_v = Some((vx * vx + vz * vz).sqrt());
                c.branch_t = Some(live - c.launch_live);
                sink.push(id);
            }
            // (the 4th route's end (9,ex,9): the napper's way out of the split (lane "9"); the
            // window closes behind it, J(7) back to lane 7, J(-1) turned for the next batch)
            if bp.y == ex && bp.z == rz && bp.x == 9 && c.lane.is_none() {
                c.lane = Some(9);
                // (the napper's speed where the 4th route hands it on toward the bed stop)
                c.merge_v = Some((vx * vx + vz * vz).sqrt());
                c.branch_t = Some(live - c.launch_live);
                sink.push(id);
                if window {
                    window = false;
                    close_window = true;
                }
            }
            let seated = c.rider.is_some_and(|r| redsim::entity::index_of(&run.world, id).is_some_and(|i| run.world.entities.list[i].passengers.contains(&r)));
            if !seated && c.dismounted.is_none() {
                c.dismounted = Some(t);
            }
            if let Some(r) = c.rider
                && let Some(j) = redsim::entity::index_of(&run.world, r)
            {
                let re = &run.world.entities.list[j];
                if c.reach12.is_none() && re.pos.y >= 12.01 {
                    c.reach12 = Some(live - c.launch_live);
                }
                // (the rider's head: a villager's box top, feet + 1.95, against the roof's underside at 28)
                c.head = c.head.max(re.pos.y + 1.95);
                if in_wall(&run.world, re.pos.x, re.pos.y, re.pos.z) {
                    c.eye_in_wall += 1;
                }
                if let EntityKind::Mob(m) = &re.kind {
                    c.least_air = c.least_air.min(m.air);
                    let h0 = *c.health0.get_or_insert(m.health);
                    c.hurt = c.hurt.max(h0 - m.health);
                }
            }
        }
        for id in sink {
            let ps: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == id).map(|e| e.passengers.clone()).unwrap_or_default();
            for e in run.world.entities.list.iter_mut() {
                if e.id == id || ps.contains(&e.id) {
                    e.removed = true;
                }
            }
        }
        if close_window {
            // (J(7) back to lane 7; J(-1) was turned again when the napper passed (1,jy,9), and the
            // next batch's first cart may have cleared it since: left alone)
            drive_j(&mut run.world, o.row9 || o.row10, 7, ex, rz, true)?;
        }
    }
    let mut wrong = 0;
    for (k, c) in carts.iter().enumerate() {
        let want = if k >= n { 9 } else { order[k % 3] };
        if c.lane != Some(want) {
            wrong += 1;
        }
        out.push(format!(
            "{} {k} (batch {}, {}): into the shaft {}; {}{}; branch speed {}; air {}; {}; hurt {:.1}; eye-in-wall ticks {}",
            if k >= n { "napper" } else { "cart" },
            if k >= n { 0 } else { k / 3 },
            if k >= n { "the 4th route".to_string() } else { format!("lane {want}") },
            match (c.met_shaft, c.reach12) {
                (Some(d), Some(r)) => format!("{d} live gt after release (the rider's feet at y 12.01 at {r})"),
                (Some(d), None) => format!("{d} live gt after release"),
                _ => "never".into(),
            },
            match c.lane {
                Some(l) if l == want => if l == 9 { "the 4th route".to_string() } else { format!("lane {l}") },
                Some(l) => format!("lane {l} (WRONG LANE)"),
                None => format!("NEVER at a lane (ends at {})", c.end.map_or("-".into(), |(x, y, z)| format!("{x:.2},{y:.2},{z:.2}"))),
            },
            c.branch_t.map_or(String::new(), |d| format!(", at its branch flat {d} live gt after release")),
            match (c.branch_v, c.merge_v) {
                (Some(v), Some(m)) => format!("{v:.3} (entering the merge cell {m:.3})"),
                (Some(v), None) => format!("{v:.3}"),
                (None, Some(m)) => format!("- (handed on by the 4th route at {m:.3} stored, {:.3}/gt)", (0.75 * m).min(0.4)),
                _ => "-".into(),
            },
            if c.least_air == i32::MAX { "-".into() } else { c.least_air.to_string() },
            if c.lane.is_some() && c.dismounted.is_none() { "seated" } else if c.dismounted.is_some() && c.lane.is_none() { "DISMOUNTED" } else { "seated" },
            c.hurt,
            c.eye_in_wall
        ));
        let f = |m: Option<(f64, f64)>| m.map_or("-".into(), |(s, d)| format!("stored >= {s:.3}, moved >= {d:.3}/gt"));
        out.push(format!(
            "  {} {k} speed pin: merge row {}; split row {}; the rider's highest head y {} (the roof's underside 28.0)",
            if k >= n { "napper" } else { "cart" },
            f(c.merge_min),
            f(c.split_min),
            if c.head.is_finite() { format!("{:.3}", c.head) } else { "-".into() }
        ));
    }
    // (each junction's turns against the next cart at it: the least gap, the latches' margin)
    let mut least: Vec<String> = Vec::new();
    for j in [3, -1] {
        let mut g = i64::MAX;
        for &(tt, jj, _) in turns.iter().filter(|x| x.1 == j) {
            if let Some(&(ta, _, _)) = at_j.iter().find(|a| a.1 == jj && a.0 >= tt) {
                g = g.min(ta - tt);
            }
        }
        least.push(format!("J({j}) {}", if g == i64::MAX { "-".into() } else { format!("{g} gt") }));
    }
    let bv = carts.iter().take(n).filter_map(|c| c.branch_v).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
    let mv = carts.iter().take(n).filter_map(|c| c.merge_v).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
    let rng = |r: (f64, f64)| if r.0.is_finite() { format!("{:.3}..{:.3}", r.0, r.1) } else { "-".into() };
    out.push(format!(
        "split: {wrong} wrong lane(s) of {n}; the batch carts' stored speed entering their branch {}, entering their lane's merge cell {} (ironfarm's delivery band 0.30..0.48, margins {}); the least gap from a junction's turn to the next cart at it: {}; the shaft's closest pair {}",
        rng(bv),
        rng(mv),
        if mv.0.is_finite() { format!("{:.3} / {:.3}", mv.0 - 0.30, 0.48 - mv.1) } else { "-".into() },
        least.join(", "),
        if shaft_gap.is_finite() { format!("{shaft_gap:.3}") } else { "-".into() }
    ));
    out.push(format!("pairs: {}", touches.line()));
    out.push(format!(
        "T crossings (the game's shapes: NE unpowered, NW powered): {t_cross} cart-ticks on a T, the largest sideways offset from the row's line {t_offset:.3}; another lane's branch detector met by a through cart: {}",
        if t_det.is_empty() { "none".to_string() } else { t_det.iter().take(6).cloned().collect::<Vec<_>>().join(", ") }
    ));
    // (the shaft's entry gaps (live gt): between a batch's carts, and the napper's behind batch 0's
    // third cart and ahead of batch 1's first (ironspec: >= 21, >= 28, >= 21))
    let mut gaps: Vec<String> = Vec::new();
    for b in 0..o.batches {
        let e: Vec<Option<i64>> = (0..3).map(|i| met_abs[b * 3 + i]).collect();
        if let (Some(a0), Some(a1), Some(a2)) = (e[0], e[1], e[2]) {
            gaps.push(format!("batch {b} {} / {}", a1 - a0, a2 - a1));
        }
    }
    let napper_gaps = if o.napper_delay.is_some() {
        format!(
            "; the napper {} behind batch 0's third cart, {} ahead of batch 1's first",
            match (met_abs[n], met_abs[2]) { (Some(m), Some(t3)) => format!("{}", m - t3), _ => "-".into() },
            match (met_abs[n], met_abs.get(3).copied().flatten()) { (Some(m), Some(f1)) => format!("{}", f1 - m), _ => "-".into() }
        )
    } else {
        String::new()
    };
    out.push(format!("shaft entry gaps (live gt): {}{napper_gaps}", gaps.join(", ")));
    if o.resync > 0 || o.seed_miss.is_some() || o.restart.is_some() {
        out.push(format!(
            "re-sync after {} gt idle: at {}; seeded misroute {}; restart {}",
            o.resync,
            if resyncs.is_empty() { "none".to_string() } else { resyncs.iter().map(|t| format!("t {t}")).collect::<Vec<_>>().join(", ") },
            o.seed_miss.map_or("none".into(), |k| format!("cart {k} ({})", if seeded { "applied" } else { "NOT applied" })),
            o.restart.map_or("none".into(), |(t, how)| format!("at t {t} ({})", how.label()))
        ));
    }
    let range = |f: &dyn Fn(&Cart) -> Option<f64>| {
        carts.iter().take(n).filter_map(f).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)))
    };
    let gap_min = (0..o.batches)
        .filter_map(|b| match (met_abs[b * 3], met_abs[b * 3 + 1], met_abs[b * 3 + 2]) {
            (Some(a0), Some(a1), Some(a2)) => Some((a1 - a0).min(a2 - a1)),
            _ => None,
        })
        .min()
        .unwrap_or(i64::MIN);
    let summary = Summary {
        wrong,
        pushes: touches.pushes,
        napper_off: o.napper_delay.is_some() && carts.get(n).is_none_or(|c| c.lane != Some(9)),
        riders_bad: carts.iter().filter(|c| c.hurt > 0.0 || c.eye_in_wall > 0 || c.dismounted.is_some()).count(),
        restarted,
        gap_min,
        napper_behind: if o.napper_delay.is_some() { met_abs[n].zip(met_abs[2]).map(|(m, t3)| m - t3).or(Some(i64::MIN)) } else { None },
        napper_ahead: if o.napper_delay.is_some() { met_abs[n].zip(met_abs.get(3).copied().flatten()).map(|(m, f1)| f1 - m).or(Some(i64::MIN)) } else { None },
        v_branch: range(&|c: &Cart| c.branch_v),
        v_merge: range(&|c: &Cart| c.merge_v),
        head: carts.iter().map(|c| c.head).fold(f64::NEG_INFINITY, f64::max),
    };
    out.extend(trace);
    Ok((out.join("\n"), summary))
}
