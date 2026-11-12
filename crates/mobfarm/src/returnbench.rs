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
    /// (c)'s napper lift re-laid for the z 7 hold (`napper_delay`): the napper from R (4,-19,4)
    /// through its own bubble lift, the staircase and the hold (9,0,8), released D gt after the
    /// arming detector by powering the hold's block.
    pub napper_lift: bool,
    /// `napper_lift`: arm the release on the station's exit detector (7,0,2) (before the boosters)
    /// instead of the main line's (5,0,7) (after them, the speed pinned).
    pub arm_station: bool,
    /// `row9`: ironfarm's S-bend sidings fed from the row by power-forced junctions K at
    /// (x-2,25,9) into F (x-2,25,8), the exit routed west into K(-3) (the lift's water to 22).
    pub k_relay: bool,
    /// The run mirrored in z (z' = 3 - z, onechunk's southern pair): the world built mirrored, every
    /// read, write and spawn through the mirror, the checks in the design frame.
    pub mirror_z: bool,
    /// K and J(-1) at day-1's taps (07:50/08:38) after the napper's own ARM3 (the cycle whose pulse
    /// released it), at D 12 and shifted by D - 12: K set at +93/108/124 (lanes -1/3/7; its driver
    /// switches 4 gt later), K reset at +125/140/156 (6 gt later); for a napper of lane 3 or 7, J(-1)'s
    /// lane-7 re-arm suppressed until +127 (switching 4 gt later).
    pub day1_taps: bool,
    /// `day1_taps`: when J(-1)'s suppression opens, gt after the napper's own ARM3 at D 12 (day-1:
    /// the napper cycle's next release REL', ARM3 + 72..85 over W).
    pub supp_open: i64,
    /// The hold's pulse gated on the napper's presence (day-1 09:37): fired only when the napper
    /// passed the approach detector (7,-3,4) at least X gt before E's rise (ARM3 + 2).
    pub pulse_gate: Option<i64>,
    /// The control for `pulse_gate`: sample the delayed bit alone (no live term).
    pub gate_delayed_only: bool,
    /// For the arrival-phase sweep's bookkeeping: the waiting bit (set on (7,-3,4)) counts as
    /// consumed by the first arming whose E rise is at least this many gt after it was set.
    pub bit_delay: i64,
    /// day-1's tripwire departure sensor over (8,0,7) (string (8,1,7), hooks (8,1,6)/(8,1,8)).
    pub tripwire: bool,
    /// `day1_taps` keyed to the napper's departure off the hold instead of its pulse's ARM3
    /// (ironspec 10:09, option (b)); with `tripwire`, to the string's first rising edge.
    pub dep_keyed: bool,
    /// The string's rising edge after the departure off the hold in the nominal row (gt): the
    /// tripwire-keyed taps are day-1's departure offsets less this.
    pub trip_lag: i64,
    /// `k_relay`: the returning napper's lane (its siding's K).
    pub napper_lane: i32,
    /// `napper_lift`: the tick the napper leaves R (the late-arrival rows).
    pub napper_r: i64,
    /// `napper_lift`: the hold's release pulse length (gt; >= 6 launches a cart from rest).
    pub release_pulse: i64,
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
            let arm_z = if o.napper_lift && o.arm_station { 2 } else if o.boosters { -100 } else { mz - 2 };
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
    if o.napper_delay.is_some() && !o.napper_lift {
        put(8, 0, mz, &rail("east_west")?);
        put(8, -1, mz, &glass);
        put(9, 0, mz, &glass);
    }
    // (`napper_lift` ((c) re-laid for the z 7 hold, ironspec 03:30): R (4,-19,4) heading +x (58f770cc's
    // detector; a plain rail here) into the entry (5,-19,4) and the napper-only bubble lift (6,y,4)
    // (soul sand (6,-20,4), water to -4, glass walls), its top (6,-3,4) pushed east by the source
    // (5,-3,4) onto the exit (7,-3,4); a powered slope (8,-3,4) up east, the curve (9,-2,4) south,
    // powered slopes (9,-2,5) and (9,-1,6) up south to the T (9,0,7) (arms N, S, W; south_west, its
    // unpowered settle: a cart from the north goes on south, one from the south turns west); the hold
    // (9,0,8), a powered rail stopping the napper against stone (9,0,9) (glass over it), released when
    // its block (9,-1,8) is powered (the kick sends it north, west at the T, (8,0,7), lane 7's curve))
    if o.napper_delay.is_some() && o.napper_lift {
        let rb = Block::parse("minecraft:redstone_block")?;
        let pr = |s: &str, on: bool| Block::parse(&format!("minecraft:powered_rail[powered={on},shape={s},waterlogged=false]"));
        put(4, -19, 4, &rail("east_west")?);
        put(4, -20, 4, &glass);
        put(5, -19, 4, &rail("east_west")?);
        put(5, -20, 4, &glass);
        put(6, -20, 4, &Block::parse("minecraft:soul_sand")?);
        for y in -19..=-4 {
            put(6, y, 4, &water);
            put(7, y, 4, &glass);
            put(6, y, 3, &glass);
            put(6, y, 5, &glass);
            if y > -19 {
                put(5, y, 4, &glass);
            }
        }
        put(7, -20, 4, &glass);
        put(6, -20, 3, &glass);
        put(6, -20, 5, &glass);
        put(6, -3, 3, &glass);
        put(6, -3, 5, &glass);
        put(6, -2, 4, &glass);
        put(5, -3, 4, &water);
        put(4, -3, 4, &glass);
        put(5, -3, 3, &glass);
        put(5, -3, 5, &glass);
        put(5, -2, 4, &glass);
        put(7, -3, 4, &rail("east_west")?);
        put(8, -3, 4, &pr("ascending_east", true)?);
        put(8, -4, 4, &rb);
        put(9, -2, 4, &rail("south_west")?);
        put(9, -3, 4, &glass);
        put(9, -2, 5, &pr("ascending_south", true)?);
        put(9, -3, 5, &rb);
        put(9, -1, 6, &pr("ascending_south", true)?);
        put(9, -2, 6, &rb);
        put(9, 0, 7, &rail("south_west")?);
        put(9, -1, 7, &glass);
        // (the hold's line a booster (on a redstone block): the napper joins at the batches' speed;
        // the block touches no face of lane 7's curve T)
        put(8, 0, mz, &pr("east_west", true)?);
        put(8, -1, mz, &rb);
        put(9, 0, 8, &pr("north_south", false)?);
        // (the hold's drive (day-1 07:03, G1 07:02): glass can't be powered, so the hold rail sits on
        // stone (9,-1,8) (a conductor) strongly powered by the repeater (8,-1,8) facing west (its
        // output east into the stone; its input (7,-1,8), day-1's chain; no T touches the repeater,
        // its input cell or the stone: a repeater beside the rail at (8,0,8) would take its input
        // at (7,0,8), a face of lane 7's curve T (7,0,7), whose placement-order shape a signal
        // source's update there would re-settle)
        put(9, -1, 8, &Block::parse("minecraft:stone")?);
        put(8, -1, 8, &Block::parse("minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?);
        put(8, -2, 8, &glass);
        // (`tripwire` (day-1 10:08, option (b)'s departure sensor): a string over the napper-only
        // cell (8,0,7) at (8,1,7), hooks at (8,1,6) facing south and (8,1,8) facing north on glass
        // anchors (8,1,5), (8,1,9); the bench prints when it trips against the napper's departure)
        if o.tripwire {
            // (the anchors glass (G1 10:57): sturdy for a hook, not a conductor, so the hooks' strong
            // power goes nowhere and their reach is their six neighbours only)
            put(8, 1, 5, &glass);
            put(8, 1, 9, &glass);
            put(8, 1, 6, &Block::parse("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?);
            put(8, 1, 8, &Block::parse("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?);
            put(8, 1, 7, &Block::parse("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?);
        }
        // (the stop: a slime block (9fc3155f 10:15: stone's top made (9,1,9) a draw cell for lane 7's
        // riders' panic tail); the launch from rest pushes away from a redstone conductor
        // (moveAlongTrack's 0.02 kick; redsim minecart.rs), so glass can't stop it, slime (a
        // conductor, not solid-render) can)
        put(9, 0, 9, &Block::parse("minecraft:slime_block")?);
        put(9, 1, 9, &glass);
    }
    // (the merge's Ts keep their shapes by placement order (ironspec 02:47): each placed after its two
    // intended arms (NW: the lane and the downstream row), with no signal source able to update it;
    // lane 7's curve is a T once the hold's line joins from the east, laid before that line)
    put(7, -1, mz, &glass);
    for x in -4..=6 {
        // (with `boosters` (the reverse order) the arming detector on the main line (5,0,mz), past the
        // boosters (the speed pinned), two cells off lane 7's curve and lane 3's T: only lane 7's carts
        // reach it before the napper)
        let r = if x == 3 || x == -1 { rail("north_west")? } else if o.boosters && o.napper_delay.is_some() && !(o.napper_lift && o.arm_station) && x == 5 { det("east_west")? } else { rail("east_west")? };
        put(x, 0, mz, &r);
        put(x, -1, mz, &glass);
    }
    put(-4, 1, mz, &glass);
    // (the shared lift: water to `top`, the exit one over it; `flat`: the lift one lower and the
    // split flat at the exit's level, the branches with it (9fc3155f: "if the split runs flat at
    // the exit's level instead, the lift and the module stay one lower"))
    let top = if o.row9 && (o.k_relay || o.climb2) { 22 } else if o.row10 || o.row9 { 24 } else if o.flat || o.dip { 21 } else { 22 };
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
    if o.row9 && o.k_relay {
        // (`k_relay` (ironspec 05:52/06:28, ironfarm's S-bend): each lane's siding fed from the row
        // by its own junction K at (x-2,25,9), power-forced like J (NE off: through; NW on: north into
        // F (x-2,25,8) on glass (x-2,24,8), ironfarm's rest R (x-2,24,7) beyond, not here); the row's
        // detectors gone (their cells are K's); nothing at (-3,24..26,10); K(-3)'s west arm is the
        // exit's approach: the top cell (-5,23,7) pushes the cart south onto the booster (the powered
        // slope (-5,23,8), powered from the west by a redstone block), the curve (-5,24,9), the slope
        // (-4,24,9) into K(-3))
        let rb = Block::parse("minecraft:redstone_block")?;
        // (the exit (ironspec 07:38 and 08:10: no slow corner, every shape natural, the delivery
        // mid-band): the lift's water to 22, the exit at 23; the plain lift's source north of the top
        // cell pushes the cart south onto the booster, the powered slope (-5,23,8) ascending_south on
        // the shaft wall's glass, powered from its west side by the redstone block (-6,23,8) (3 cells
        // from lane -1's seat); the curve (-5,24,9) north_east on glass; the slope (-4,24,9)
        // ascending_east on glass into K(-3): two climbs, every rail within 2 of the seat on glass)
        put(-5, ex, 8, &Block::parse("minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?);
        put(-6, ex, 8, &rb);
        // (the redstone block's top capped with glass (onechunk 08:41: open, a golem spawn spot; the
        // cap is off the riders' boxes and eye path, which stay in x -5..-4))
        put(-6, ex + 1, 8, &glass);
        put(-5, ex + 1, 9, &rail("north_east")?);
        put(-5, ex, 9, &glass);
        put(-4, ex + 1, 9, &rail("ascending_east")?);
        put(-4, ex, 9, &glass);
        for x in -3..=6 {
            let r = if [-3, -1, 1, 3, 5].contains(&x) { rail("north_east")? } else { rail("east_west")? };
            put(x, 25, 9, &r);
            put(x, 24, 9, &glass);
        }
        put(7, 25, 9, &rail("north_west")?);
        put(7, 24, 9, &glass);
        if o.napper_delay.is_some() {
            for x in 8..=9 {
                put(x, 25, 9, &rail("east_west")?);
                put(x, 24, 9, &glass);
            }
        }
        for (jx, on) in [(-3, false), (-1, false), (1, false), (3, false), (5, false), (7, true)] {
            if jx == 7 && o.napper_delay.is_none() {
                continue;
            }
            put(jx, 26, 9, &Block::parse(&format!("minecraft:lever[face=ceiling,facing=north,powered={on}]"))?);
            put(jx, 27, 9, &glass);
        }
        for x in LANES {
            put(x - 2, 25, 8, &rail("north_south")?);
            put(x - 2, 24, 8, &glass);
            put(x, 24, 8, &det("ascending_south")?);
            put(x, 23, 8, &glass);
            put(x, 24, 7, &rail("north_south")?);
            put(x, 23, 7, &glass);
        }
        return Ok(FarmSpec {
            blocks: b,
            entities: Vec::new(),
            output: BlockPos::new(10, 0, 0),
            lo: BlockPos::new(-20, if o.napper_lift { -22 } else { -3 }, -2),
            hi: BlockPos::new(11, 28, 11),
            block_entities: Vec::new(),
            rock: false,
            nether: None,
            land: false,
        });
    }
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
            lo: BlockPos::new(-20, if o.napper_lift { -22 } else { -3 }, -2),
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
    /// Waypoints (absolute ticks, first visit): its lane's D cell (x,24,8); for the napper its lift
    /// at y >= -12, the hold (9,0,8), (4,0,7) after the hold, its K (n-2,25,9).
    w_d: Option<i64>,
    w_lift12: Option<i64>,
    w_hold: Option<i64>,
    w_merge: Option<i64>,
    w_k: Option<i64>,
    /// The stored speed entering its K's cell (the napper; ironfarm's S-bend margin needs > ~0.14).
    v_k: Option<f64>,
    /// The first tick on J(-1)'s cell (-1,25,9).
    w_j1: Option<i64>,
    /// A batch cart seen on the napper hold's route (single-occupant evidence).
    on_hold_route: bool,
    /// The napper: first on day-1's approach detector (7,-3,4), at rest on the hold, off the hold
    /// after its release.
    w_exit: Option<i64>,
    w_rest: Option<i64>,
    w_leave: Option<i64>,
    /// The napper's first tick on the hold's line (8,0,7) (the tripwire's cell).
    w_line: Option<i64>,
    /// The napper's stored speed entering the T (9,0,7) from the staircase.
    v_t: Option<f64>,
    launch_t: i64,
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
    /// Trip legs (gt): [0] the napper's lift at y -12 -> the hold, [1] the hold's release -> (4,0,7),
    /// [2] (4,0,7) -> its K, [3] K turned on -> the napper at K, [4..7] T_D for lanes -1, 3, 7 (D's
    /// cell after the lane cart's own launch, batch 0).
    pub trips: [Option<i64>; 7],
    /// The napper's stored speed entering its K's cell (ironfarm's S-bend: > ~0.14).
    pub nap_v_k: Option<f64>,
    /// J(-1)'s cell (-1,25,9) after the napper's own ARM3: the napper's pass, the next batch's first
    /// cart (day-1's J(-1) re-arm window).
    pub j1: (Option<i64>, Option<i64>),
    /// The through-carts' least stored speed on a K's cell.
    pub thru_v_k: Option<f64>,
    /// Batch carts seen on the napper hold's route (the T (9,0,7), the hold's line, the hold): bar 0.
    pub hold_intrusions: usize,
    /// The through-carts' greatest stored speed on a K's cell.
    pub thru_v_k_hi: Option<f64>,
    /// The napper from day-1's approach detector (7,-3,4) to rest on the hold (gt).
    pub approach: Option<i64>,
    /// The napper from leaving the bed stop (R, its start) to rest on the hold (gt).
    pub stop_to_hold: Option<i64>,
    /// The napper from leaving the bed stop (R) to its K's cell (gt).
    pub stop_to_k: Option<i64>,
    /// The napper's departure off the hold after its own ARM3 (gt).
    pub departure: Option<i64>,
    /// Arming rises whose pulse the gate skipped.
    pub skipped_pulses: usize,
    /// Pulses in the run, and those after the napper's release that found a cart on the hold (bar 0).
    pub pulses_run: usize,
    pub extra_busy: usize,
    /// The string's first rising edge after the napper's departure off the hold (gt).
    pub trip_after_dep: Option<i64>,
    /// The napper's stored speed entering the T (9,0,7) from the staircase, and the entry's tick
    /// against the cycle (its arrival phase: ticks since the latest arming).
    pub v_t: Option<f64>,
    pub t_phase: Option<i64>,
    /// `pulse_gate` (cc767b78's (c') bar): more than one pulse fired during a rest, a departure not on
    /// a fired pulse, a creep, or a fired pulse without that cycle's departure.
    pub release_bad: bool,
    /// The pulse the napper left on differs from the one that consumed its waiting bit (None: it
    /// never left).
    pub pulse_mismatch: Option<bool>,
}

impl Summary {
    pub fn clean(&self) -> bool {
        self.wrong == 0 && self.pushes == 0 && !self.napper_off && self.riders_bad == 0 && self.hold_intrusions == 0 && self.extra_busy == 0 && !self.release_bad
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
    let names = ["the napper's lift at y -12 -> the hold", "the hold's release -> (4,0,7)", "(4,0,7) -> its K", "K on -> the napper at K", "T_D lane -1", "T_D lane 3", "T_D lane 7"];
    let trips: Vec<String> = (0..7)
        .filter_map(|i| {
            let xs: Vec<i64> = ok.iter().filter_map(|s| s.trips[i]).collect();
            (!xs.is_empty()).then(|| format!("{} {}..{}", names[i], xs.iter().min().copied().unwrap_or(0), xs.iter().max().copied().unwrap_or(0)))
        })
        .collect();
    let rng = |xs: Vec<i64>| if xs.is_empty() { "-".to_string() } else { format!("{}..{}", xs.iter().min().copied().unwrap_or(0), xs.iter().max().copied().unwrap_or(0)) };
    let vk: Vec<f64> = ok.iter().filter_map(|s| s.nap_v_k).collect();
    let tk: Vec<f64> = ok.iter().filter_map(|s| s.thru_v_k).collect();
    let tkh: Vec<f64> = ok.iter().filter_map(|s| s.thru_v_k_hi).collect();
    let kj = format!(
        "the napper entering K at stored {}; the through-carts' stored speed on a K's cell {}..{}; batch carts on the napper hold's route (bar 0) {}; the napper from (7,-3,4) to rest on the hold {}; from the stop (R) to rest on the hold {}; its departure off the hold after its own ARM3 {}; pulses the gate skipped {}; J(-1)'s cell after the napper's own ARM3: the napper {}, the next batch's first cart {}",
        if vk.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", vk.iter().copied().fold(f64::INFINITY, f64::min), vk.iter().copied().fold(f64::NEG_INFINITY, f64::max)) },
        if tk.is_empty() { "-".to_string() } else { format!("{:.3}", tk.iter().copied().fold(f64::INFINITY, f64::min)) },
        if tkh.is_empty() { "-".to_string() } else { format!("{:.3}", tkh.iter().copied().fold(f64::NEG_INFINITY, f64::max)) },
        ok.iter().map(|s| s.hold_intrusions).sum::<usize>(),
        rng(ok.iter().filter_map(|s| s.approach).collect()),
        rng(ok.iter().filter_map(|s| s.stop_to_hold).collect()),
        rng(ok.iter().filter_map(|s| s.departure).collect()),
        ok.iter().map(|s| s.skipped_pulses).sum::<usize>(),
        rng(ok.iter().filter_map(|s| s.j1.0).collect()),
        rng(ok.iter().filter_map(|s| s.j1.1).collect())
    );
    let kj = format!(
        "{kj}; pulses per run {}; pulses after the napper's release that found a cart on the hold (bar 0) {}; the string's rising edge after the departure off the hold {}; the napper entering the T (9,0,7) at stored {}, reaching the hold {} gt after the latest arming; the napper from the stop (R) to its K {}",
        rng(ok.iter().map(|s| s.pulses_run as i64).collect()),
        ok.iter().map(|s| s.extra_busy).sum::<usize>(),
        rng(ok.iter().filter_map(|s| s.trip_after_dep).collect()),
        {
            let vt: Vec<f64> = ok.iter().filter_map(|s| s.v_t).collect();
            if vt.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", vt.iter().copied().fold(f64::INFINITY, f64::min), vt.iter().copied().fold(f64::NEG_INFINITY, f64::max)) }
        },
        rng(ok.iter().filter_map(|s| s.t_phase).collect()),
        rng(ok.iter().filter_map(|s| s.stop_to_k).collect())
    );
    format!(
        "{} rows; the least shaft-entry gap {}; the napper's least gap behind {} and ahead {}; branch entry stored {:.3}..{:.3}, merge-cell entry {:.3}..{:.3}; the highest head {:.3}; trips (gt, min..max): {}; {kj}; rows with a wrong lane, a push, the napper off its route or a rider hurt: {}; rows under a nominal bar: {}{}",
        v.len(),
        ok.iter().map(|s| s.gap_min).min().unwrap_or(i64::MIN),
        nap(&|s: &Summary| s.napper_behind),
        nap(&|s: &Summary| s.napper_ahead),
        lo(&|s: &Summary| s.v_branch.0),
        hi(&|s: &Summary| s.v_branch.1),
        lo(&|s: &Summary| s.v_merge.0),
        hi(&|s: &Summary| s.v_merge.1),
        hi(&|s: &Summary| s.head),
        if trips.is_empty() { "-".to_string() } else { trips.join(", ") },
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

/// The late-arrival rows (ironspec 06:45): the napper leaves R at every tick `from..=to`, so it
/// reaches the hold before, at and 1..100 gt after a release pulse; a late one must leave at the
/// next pulse with every bar met (the napper's gaps measured against the batch it follows).
pub fn late_sweep(o: &ReturnOpts, from: i64, to: i64) -> Result<String, String> {
    let jobs: Vec<(String, ReturnOpts)> = (from..=to).map(|r| (format!("R at t {r}"), ReturnOpts { napper_r: r, trace: None, out: None, ..o.clone() })).collect();
    let v = run_rows(jobs)?;
    Ok(format!("late-arrival sweep (the napper leaving R at t {from}..={to}): {}", verdict(&v)))
}

/// The arrival-phase sweep (ironspec 10:05): the napper's start at every tick `r_from..=r_to` (its
/// detection at every phase of the cycle), each without a login and with a `len`-gt login at every
/// offset `off_from..=off_to` after its start; per row the pulse that consumed its waiting bit (the
/// first arming whose E rise (ARM3 + 2) is >= its detection on (7,-3,4) + `bit_delay`) against the
/// pulse it left on (bar: always the same).
pub fn phase_sweep(o: &ReturnOpts, r_from: i64, r_to: i64, off_from: i64, off_to: i64, len: i64) -> Result<String, String> {
    let mut jobs: Vec<(String, ReturnOpts)> = Vec::new();
    for r in r_from..=r_to {
        jobs.push((format!("R at t {r}"), ReturnOpts { napper_r: r, trace: None, out: None, ..o.clone() }));
        for off in off_from..=off_to {
            jobs.push((format!("R at t {r}, login at R + {off} for {len}"), ReturnOpts { napper_r: r, login: Some((r + off, len)), trace: None, out: None, ..o.clone() }));
        }
    }
    let v = run_rows(jobs)?;
    let mism: Vec<&String> = v.iter().filter(|(_, r)| r.as_ref().is_ok_and(|s| s.pulse_mismatch == Some(true))).map(|(k, _)| k).collect();
    let never = v.iter().filter(|(_, r)| r.as_ref().is_ok_and(|s| s.pulse_mismatch.is_none())).count();
    // (the departure band: the rows at both ends, with their margins)
    let deps: Vec<(&String, &Summary)> = v.iter().filter_map(|(k, r)| r.as_ref().ok().filter(|s| s.departure.is_some()).map(|s| (k, s))).collect();
    let end = |s: &Summary| {
        format!(
            "departure ARM3 + {}: the napper {} behind / {} ahead, least shaft gap {}, K entry {}, J(-1) after ARM3: napper {} / next batch {}",
            s.departure.unwrap_or(0),
            s.napper_behind.map_or("-".into(), |g| g.to_string()),
            s.napper_ahead.map_or("-".into(), |g| g.to_string()),
            s.gap_min,
            s.nap_v_k.map_or("-".into(), |x| format!("{x:.3}")),
            s.j1.0.map_or("-".into(), |g| g.to_string()),
            s.j1.1.map_or("-".into(), |g| g.to_string())
        )
    };
    let lo_row = deps.iter().min_by_key(|(_, s)| s.departure.unwrap_or(0));
    let hi_row = deps.iter().max_by_key(|(_, s)| s.departure.unwrap_or(0));
    let band = format!(
        "the departure band over {} departing rows: early end ({}) {}; late end ({}) {}",
        deps.len(),
        lo_row.map_or("-".to_string(), |(k, _)| k.to_string()),
        lo_row.map_or("-".to_string(), |(_, s)| end(s)),
        hi_row.map_or("-".to_string(), |(k, _)| k.to_string()),
        hi_row.map_or("-".to_string(), |(_, s)| end(s))
    );
    Ok(format!(
        "arrival-phase sweep (the napper's start {r_from}..={r_to}, each bare and with a {len}-gt login at start + {off_from}..={off_to}; the waiting bit sampled {} gt late at E's rise): rows where the napper left on a pulse other than the one that consumed its waiting bit {} (bar 0){}; rows where it never left {}; {band}; {}",
        o.pulse_gate.unwrap_or(o.bit_delay),
        mism.len(),
        if mism.is_empty() { String::new() } else { format!(" (e.g. {})", mism.iter().take(6).map(|s| s.as_str()).collect::<Vec<_>>().join("; ")) },
        never,
        verdict(&v)
    ))
}

/// A `len`-gt login (liveness's freeze) starting at every tick `from..=to` (cc767b78 10:00: 3-gt
/// logins inside the hold's pulse), on 16 threads, against every bar.
pub fn login_sweep(o: &ReturnOpts, from: i64, to: i64, len: i64) -> Result<String, String> {
    let jobs: Vec<(String, ReturnOpts)> = (from..=to).map(|f| (format!("login at t {f} for {len}"), ReturnOpts { login: Some((f, len)), trace: None, out: None, ..o.clone() })).collect();
    let v = run_rows(jobs)?;
    Ok(format!("login sweep ({len}-gt login at every tick {from}..={to}): {}", verdict(&v)))
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
        let station = (p.y == 0 || p.y == -1) && p.z < mz && LANES.contains(&p.x);
        let r_stand = p.x == 4 && p.z == 4 && (p.y == -19 || p.y == -20);
        let branch = LANES.contains(&p.x) && (23..=24).contains(&p.y) && ((o.row10 && (7..=9).contains(&p.z)) || (o.row9 && p.z == 7));
        let stop = !o.napper_lift && p.x == 9 && p.y == 0 && p.z == mz;
        if station || branch || stop || r_stand {
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
    // (the header from the bench's own cells and flags (G1, CHANGES 1 on ea7e196b9): every
    // coordinate below is the one `build` lays for these options)
    let top = if o.row9 && (o.k_relay || o.climb2) { 22 } else if o.row10 || o.row9 { 24 } else if o.flat || o.dip { 21 } else { 22 };
    let ex = top + 1;
    let offs: Vec<String> = (if o.offsets.len() == 3 { o.offsets.clone() } else { vec![0, o.spacing, 2 * o.spacing] }).iter().map(|v| v.to_string()).collect();
    let run_line = format!(
        "mctas mobfarm return-bench{}{}{}{}{}{}{}{} --boosters{} --offsets {}{}{}{}{}",
        if o.z7 { " --z7" } else { "" },
        if o.row9 { " --row9" } else if o.row10 { " --row10" } else { "" },
        if o.k_relay { format!(" --k-relay --napper-lane {}{}{}{}", o.napper_lane, if o.day1_taps { " --day1-taps" } else { "" }, if o.day1_taps && o.supp_open != 73 { format!(" --supp-open {}", o.supp_open) } else { String::new() }, if o.dep_keyed { format!(" --dep-keyed{}", if o.tripwire { format!(" --trip-lag {}", o.trip_lag) } else { String::new() }) } else { String::new() }) } else { String::new() },
        if o.boost_exit { " --boost-exit" } else { "" },
        if o.climb2 { " --climb2" } else { "" },
        if o.boost_slope { " --boost-slope" } else { "" },
        if o.row_boost { " --row-boost" } else { "" },
        if o.reverse { " --reverse" } else { "" },
        if o.game_ticks { " --game-ticks" } else { "" },
        offs.join(","),
        o.napper_delay.map_or(String::new(), |d| format!(" --napper-delay {d}")),
        if o.napper_lift && o.arm_station { " --napper-lift --arm-station" } else if o.napper_lift { " --napper-lift" } else { "" },
        if o.napper_lift && o.release_pulse != 8 { format!(" --release-pulse {}", o.release_pulse) } else { String::new() },
        o.pulse_gate.filter(|_| o.napper_lift).map_or(String::new(), |x| format!(" --pulse-gate {x}{}", if o.gate_delayed_only { " --gate-delayed-only" } else { "" })) + if o.tripwire { " --tripwire" } else { "" }
    );
    let lift = {
        format!(
            "# - shared lift: soul sand (-5,-1,7) on glass (-5,-2,7), the bubble column (-5,0..{top},7), glass walls (-6,y,7), (-5,y,6) for y 0..{top}, (-5,y,8) for y 0..{} and (-4,y,7) for y 1..{top} (panes to come; (-4,0,7) is the wall entry){}; the top cell (-5,{ex},7) with glass over it (-5,{},7) and beside it (-6,{ex},7), (-4,{ex},7); the current's source (-5,{ex},6) boxed by glass (-5,{ex},5), (-6,{ex},6), (-4,{ex},6), (-5,{},6).",
            if o.boost_exit { top - 1 } else { top },
            if o.boost_exit { format!(", the redstone block (-5,{top},8) under the exit's booster as the south wall's top cell") } else { String::new() },
            ex + 1,
            ex + 1
        )
    };
    let exit = if o.row9 && o.k_relay {
        format!(
            "# - exit and split (in the chunk; K re-lay): the booster, a powered slope (-5,{ex},8) ascending_south (on) on the shaft wall's glass (-5,{top},8), powered from its west side by the redstone block (-6,{ex},8) (3 from lane -1's seat), capped with glass (-6,{},8) (no spawn spot on it); the curve (-5,{},9) north_east on glass; the slope (-4,{},9) ascending_east on glass into K(-3) (lay each slope after the rail at its high end); every rail within 2 of lane -1's seat (-3,24,7) on glass; the row along z 9 at y 25, x -3..6 on glass at y 24: K(-3) (-3,25,9), J(-1) (-1,25,9), K(1) (1,25,9), J(3) (3,25,9), K(5) (5,25,9), J(7) (7,25,9){}; each K's feed F (x-2,25,8) north_south on glass (x-2,24,8) (ironfarm's S-bend beyond, not here); each lane's branch the detector slope D (x,24,8) ascending_south on glass (x,23,8), into ironfarm's merge cell (x,24,7) (not here); a lever over every J and K (x,26,9) with glass (x,27,9); (x-1,24,9) glass only; nothing of this file in (x-2..x-1, 22..23, 9) or at (x,23,9) (ironfarm's R feed path and the switched cell).",
            ex + 1,
            ex + 1,
            ex + 1,
            if o.napper_delay.is_some() { ", the 4th route (8,25,9), (9,25,9)" } else { "" }
        )
    } else if o.row9 {
        let boost = if o.climb2 && o.boost_slope {
            format!("the exit cell (-5,{ex},8) plain; the curve (-5,{ex},9) north_east; the first climbing slope (-4,{ex},9) a powered rail ascending_east on the redstone block (-4,{top},9), then (-3,{},9) ascending_east", ex + 1)
        } else if o.boost_exit && o.climb2 {
            format!("the exit cell (-5,{ex},8) a powered rail (north_south, on) on the redstone block (-5,{top},8) (the shaft's south wall there); the curve (-5,{ex},9) north_east; two climbing slopes (-4,{ex},9) and (-3,{},9), both ascending_east, to the row", ex + 1)
        } else if o.boost_exit {
            format!("the exit cell (-5,{ex},8) a powered rail (north_south, on) on the redstone block (-5,{top},8); the curve (-5,{ex},9) north_east; (-4,{ex},9) plain")
        } else {
            format!("the exit cell (-5,{ex},8) plain; the curve (-5,{ex},9) north_east; (-4,{ex},9) a powered rail on the redstone block (-4,{top},9)")
        };
        format!(
            "# - exit and split (in the chunk): {boost}; the row along z 9 at y 25 from x {} (glass under it at y 24): J(-1) (-1,25,9), J(3) (3,25,9), J(7) (7,25,9), the row's detectors (1,25,9) and (5,25,9){}; each lane's branch the detector slope (x,24,8) ascending_south on glass (x,23,8), into ironfarm's merge cell (x,24,7) (not here); the junctions' levers (x,26,9) with glass (x,27,9).",
            if o.climb2 { -2 } else { -3 },
            if o.napper_delay.is_some() { ", the 4th route (8,25,9), (9,25,9)" } else { "" }
        )
    } else {
        "# - exit and split: the row along z 10 at y 24 (OUTSIDE the chunk), the slope (-3,24,10), a powered rail (-2,24,10) with --row-boost; J(-1) (-1,24,10), J(3) (3,24,10), J(7) (7,24,10); the row's detectors (1,24,10), (5,24,10); the 4th route (8,24,10), (9,24,10); the lanes' branches (x,24,9) -> the detector (x,24,8) -> (x,24,7) are ironfarm's (not here).".to_string()
    };
    let napper = if o.napper_lift {
        format!(
            "{} the arming detector {}.",
            "# - napper (c), re-laid for the z 7 hold: from R (4,-19,4) (58f770cc's, not here) the entry (5,-19,4) east_west on glass; soul sand (6,-20,4), the bubble column (6,-19..-4,4), glass walls (7,y,4), (6,y,3), (6,y,5) for y -20..-4 and (5,y,4) for y -18..-4; the top cell (6,-3,4) (glass beside it (6,-3,3), (6,-3,5), over it (6,-2,4)), the source (5,-3,4) boxed by glass (4,-3,4), (5,-3,3), (5,-3,5), (5,-2,4); the exit (7,-3,4) east_west; a powered slope (8,-3,4) ascending_east on the redstone block (8,-4,4); the curve (9,-2,4) south_west on glass; powered slopes (9,-2,5) and (9,-1,6) ascending_south on the redstone blocks (9,-3,5), (9,-2,6); the T (9,0,7) south_west on glass (arms N, S, W: from the north on south, from the south west); the hold (9,0,8), a powered rail north_south, off, on stone (9,-1,8) (a conductor), driven by the repeater (8,-1,8) facing west on glass (8,-2,8) (its input (7,-1,8), day-1's chain: the hold rail powered from ARM3 + D for the release pulse (--release-pulse, 8 gt by default) in each cycle whose pulse fires (with --pulse-gate X, day-1's gate: only when the napper's waiting bit, set on the approach detector (7,-3,4) (a detector rail east_west on glass (7,-4,4), read from its side) and sampled X gt late at E's rise (ARM3 + 2), is high, so a napper not yet at rest waits for the next cycle's pulse and every departure is a scheduled one; without it, every cycle; with --tripwire and --dep-keyed, (b): an ungated pulse every cycle, the departure read by a north-south tripwire string (8,1,7) over the napper-only (8,0,7), hooks (8,1,6) facing south and (8,1,8) facing north on glass anchors (8,1,5), (8,1,9), the FIFO pop and the K/J(-1) taps keyed to its rising edge); EMULATED, a redstone block at (7,-1,8) for the pulse), the napper stopped against the slime block (9,0,9) (a conductor, so the launch's kick pushes off it; not solid-render) with glass over it (9,1,9); the hold's line (8,0,7) a powered rail (on) on the redstone block (8,-1,7) into lane 7's curve (7,0,7); declared (push-scan's two pairs at the hold): the staircase, the T (9,0,7), the hold (9,0,8) and the hold's line (8,0,7) are the napper's own route, no batch cart enters it (lane 7's merge T (7,0,7) joins north and west only, its east arm is the hold's line), one napper at a time (the next reaches the hold a lap later, >= ~970 gt by day-1's minimum), so no two carts meet there;",
            if o.arm_station { "the station's own exit detector on lane 7 (7,0,2) (dfb89499's)" } else { "(5,0,7) east_west on the main line (past the boosters, two cells off lane 7's curve and lane 3's T)" }
        )
    } else if o.napper_delay.is_some() {
        "# - napper (c): the hold's line (8,0,7) only (the bench spawns the napper there, EMULATED); the napper lift not in this file.".to_string()
    } else {
        "# - napper: none in this run.".to_string()
    };
    let mut head: Vec<String> = vec![
        "# The return for F1 + Q2 at 24 (7b90c3f1; bench, tool unreviewed): loop frame, paste with no move. Made by `mctas mobfarm return-bench --out` from the bench's own world; every coordinate in this header is generated from the same options.".into(),
        format!("# run line: {run_line}"),
        "# - merge row along z 7: lane 7's curve (7,0,7) north_west (a T once the hold's line joins from (8,0,7)), Ts (3,0,7) and (-1,0,7) north_west, the main line (x,0,7) x -4..6 on glass, glass over the wall entry (-4,1,7); the station's tracks and boosters (z 1..6) are dfb89499's, not here.".into(),
        lift,
        exit,
        napper,
        "# - declared: each D (x,24,8)'s high end is its junction J (x,25,9) (3 potential connections) and J is a face of the cell over D; J is power-forced by its declared switch in both states, so D's press and release re-settle J to the settle shape of its power, a no-op: tick-exact's official D-J probe passes (J at its driver's settle, NE off / NW on, 2 gt after the lever; D ascending_south through every press, release, toggle while pressed and restart in each state; module-check 0 of 14 blocks differ; https://artifacts.internalmeta.com/a/1169573292176966).".to_string(),
        "# - switches EMULATED: a lever over each junction (face ceiling, glass over it) is the latch's output (`drive_j` only toggles the lever, flags ALL; redsim settles the junction); the latches: J(-1) on at idle; lane -1's branch rise turns it off and J(3) on; lane 3's turns J(3) off; the third cart's rise at the row's second detector turns J(-1) on, or opens the napper window (J(7) off on lane 7's branch rise; J(-1) on at the napper's own first rise at the row's first detector).".into(),
        if o.k_relay {
            format!("# - OPEN: day-1's drivers ({}; the hold's pulse into (7,-1,8)); lane -1's seat (-3,24,7): no rail of mine on a solid-render block within 2 of it (the booster's redstone block sits at (-6,{ex},8), beside the booster (-5,{ex},8), 3 from the seat), 9fc3155f to confirm with --clear-rate.", if o.day1_taps { "K set, K reset and J(-1)'s re-arm at day-1's taps after the napper's own ARM3, EMULATED here as the levers at those ticks" } else { "each K on after the batch's third cart passes it and before the napper, EMULATED here by the bench seeing the third cart" })
        } else {
            "# - OPEN: ironfarm's siding feeds (x-2,25,9) sit on this row: (1,25,9) and (5,25,9) are the row's detectors, (-3,25,9) is over the climb slope (-3,24,9) (where the rest's glass support goes); the K re-lay (power-forced junctions at (x-2,25,9)) is approved in direction, pending the rest's contact fix (ironfarm), the siding sensor (x-1,24,7) and day-1's K driver.".to_string()
        },
    ];
    if o.napper_lift {
        head.push("# single-occupant: 9,0,8+9,0,7+8,0,7=one napper released per nap trip, on a fired pulse only (the gate); every arm of the T (9,0,7) carries only that napper's route (in from the staircase, rest on the hold (9,0,8) against the slime stop (9,0,9), out west on (8,0,7)); lane 7's merge T (7,0,7) joins north and west only, so no batch cart enters (return-bench: 0 batch carts on the hold's route over its late, jitter, restart and arrival-phase sweeps); consecutive nappers leave the bed stop >= 312 gt apart (day-1) and the stop -> hold trip spreads by <= 4 gt (75..79, return-bench), against the bar 177 (the hold's longest wait 138 + the 12-gt pulse + clear + 20; ironspec 11:49): 135 to spare".to_string());
    }
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

/// One block mirrored in z (north and south swapped in its state: curves, slopes, facings).
fn mirror_block(b: &Block) -> Result<Block, String> {
    let s = b.to_string();
    let t = s.replace("north", "\u{1}").replace("south", "north").replace('\u{1}', "south").replace("south_north", "north_south");
    Block::parse(&t)
}

/// The spec mirrored in z: z' = 3 - z (onechunk's southern pair), every block's state mirrored.
fn mirror_spec(spec: &FarmSpec) -> Result<FarmSpec, String> {
    let blocks = spec.blocks.iter().map(|(p, b)| Ok((BlockPos::new(p.x, p.y, 3 - p.z), mirror_block(b)?))).collect::<Result<Vec<_>, String>>()?;
    Ok(FarmSpec { blocks, lo: BlockPos::new(spec.lo.x, spec.lo.y, 3 - spec.hi.z), hi: BlockPos::new(spec.hi.x, spec.hi.y, 3 - spec.lo.z), ..spec.clone() })
}

/// One run: the report and its failure counts.
fn return_run(o: &ReturnOpts) -> Result<(String, Summary), String> {
    let mz = if o.z7 { 7 } else { 8 };
    let spec = build(o)?;
    if o.mirror_z && o.out.is_some() {
        return Err("--out with --mirror-z: emit the design unmirrored (the mirrored copy is onechunk's)".into());
    }
    // (`mirror_z`: the world built mirrored, z' = 3 - z, every block's north and south swapped)
    let spec = if o.mirror_z { mirror_spec(&spec)? } else { spec };
    // (ironspec 11:05: `place_raw` skips cells outside the world's box silently; refuse them)
    let oob = spec.blocks.iter().filter(|(p, _)| p.x < spec.lo.x || p.y < spec.lo.y || p.z < spec.lo.z || p.x > spec.hi.x || p.y > spec.hi.y || p.z > spec.hi.z).count();
    if oob > 0 {
        return Err(format!("{oob} cell(s) outside the bench world's box"));
    }
    // (`napper_lift`: the batches start 100 gt in (the napper waits on the hold first), so the run is
    // 100 gt longer than `ticks` (cc767b78 06:54))
    let run_ticks = o.ticks + if o.napper_lift { 100 } else { 0 };
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, (run_ticks + 200) as u32, WalkMode::Random) };
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
    let mut carts: Vec<Cart> = (0..total).map(|_| Cart { id: None, rider: None, launch_live: 0, lane: None, branch_v: None, merge_v: None, branch_t: None, least_air: i32::MAX, hurt: 0.0, health0: None, eye_in_wall: 0, dismounted: None, met_shaft: None, reach12: None, head: f64::NEG_INFINITY, w_d: None, w_lift12: None, w_hold: None, w_merge: None, w_k: None, v_k: None, w_j1: None, on_hold_route: false, w_exit: None, w_rest: None, w_leave: None, w_line: None, v_t: None, launch_t: 0, merge_min: None, split_min: None, prev: None, end: None }).collect();
    let arm_lane = if o.reverse { 7 } else { -1 };
    let (mut arm_was, mut arm_at, mut window, mut napper_past) = (false, None::<i64>, false, false);
    let (mut pulses, mut pulse_count): (Vec<i64>, usize) = (Vec::new(), 0);
    let (mut hold_was_on, mut hold_rise): (bool, Option<i64>) = (false, None);
    let mut met_abs: Vec<Option<i64>> = vec![None; total];
    let shape = |s: &str| Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"));
    // (`mirror_z` (onechunk's z-mirrored pair, z' = 3 - z): every block the bench reads or writes and every
    // spawn goes through the mirror; the carts' states are mapped back to the design frame for the checks)
    let zf = |z: i32| if o.mirror_z { 3 - z } else { z };
    let on = |w: &redsim::world::World, x: i32, y: i32, z: i32| matches!(w.get(BlockPos::new(x, y, if o.mirror_z { 3 - z } else { z })), Block::DetectorRail { powered: true, .. });
    // (the latches: L3 = J(3) to lane 3, Lm = J(-1) to lane -1, the bulb toggled by (1,23,9))
    let order: [i32; 3] = if o.reverse { [-1, 3, 7] } else { LANES };
    let offsets: Vec<i64> = if o.offsets.len() == 3 { o.offsets.clone() } else { vec![0, o.spacing, 2 * o.spacing] };
    // (the reverse order starts with J(-1) turned for the batch's first cart)
    let (mut l3, mut lm, mut bulb) = (false, o.reverse, false);
    if o.reverse {
        drive_j(&mut run.world, o.row9 || o.row10, -1, ex, zf(rz), true)?;
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
    let (mut k_on, mut j_deferred) = (false, false);
    // (`day1_taps`: the napper's own ARM3 (its releasing pulse - D) and the batch whose arming it was)
    let mut nap_arm: Option<i64> = None;
    let mut skipped_pulses: usize = 0;
    let mut arms: Vec<i64> = Vec::new();
    let mut extra_busy: usize = 0;
    let mut rel_at: Option<i64> = None;
    // (each fired pulse's presence sample at E's rise (ARM3 + 2): the napper at rest on the hold, or
    // rolling, and its stored speed)
    let (mut fired_arms, mut samples): (Vec<i64>, Vec<String>) = (Vec::new(), Vec::new());
    let (mut trip_was, mut trips_seen): (bool, Vec<i64>) = (false, Vec::new());
    let mut hold_mid: i64 = 0;
    let mut nap_b_pulse: Option<usize> = None;
    let (mut thru_v_k, mut hold_intrusions): (Option<f64>, usize) = (None, 0);
    let mut thru_v_k_hi: Option<f64> = None;
    let (mut nap_b, mut k_on_at): (Option<usize>, Option<i64>) = (None, None);
    let (mut hold_released, mut hold_wait): (bool, Option<(f64, f64, f64)>) = (false, None);
    let (mut t_offset, mut t_cross, mut t_det) = (0.0f64, 0usize, Vec::<String>::new());
    for t in 0..run_ticks {
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
            // (`napper_lift`: the batches start 100 gt in, so the napper (from R at the start, ~75 gt
            // to the hold) waits on the hold before its release)
            let prelude = if o.napper_lift { 100 } else { 0 };
            let at = prelude + (k as i64 / 3) * 100 + offsets[k % 3] + o.jitter.get(k).copied().unwrap_or(0);
            if c.id.is_none() && (o.game_ticks || !frozen) && (if o.game_ticks { t } else { live }) >= at {
                let x = order[k % 3];
                let nbt = format!("{{Motion:[0.0d,0.0d,{:.4}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", if o.mirror_z { -o.speed } else { o.speed });
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x as f64 + 0.5, 0.0625, if o.mirror_z { 2.5 } else { 1.5 }], Some(&nbt))?;
                c.id = Some(id);
                c.launch_live = live;
                c.launch_t = t;
                c.rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
        }
        // ((c)'s napper: the arming detector's first rise (the batch 0 third cart), the hold's
        // release D gt after it (a repeater chain: game ticks in L1, live in L2), the napper
        // leaving (8,0,8) west at 0.4)
        if let Some(d) = o.napper_delay {
            let a = if o.napper_lift && o.arm_station {
                // (`arm_station`: the station's own exit detector on lane 7 (7,0,2), before the boosters)
                on(&run.world, 7, 0, 2)
            } else if o.boosters {
                on(&run.world, 5, 0, mz)
            } else {
                on(&run.world, arm_lane, 0, mz - 2)
            };
            if a && !arm_was && arm_at.is_none() {
                arm_at = Some(if o.game_ticks { t } else { live });
            }
            // (`napper_lift`: every arming rise schedules the hold's release pulse D gt later (a
            // pulse every cycle, ironspec 06:45: a late arrival waits for its next slot); with
            // `pulse_gate` X (day-1 09:37, ironspec's pulse-tail rule), only when the napper's waiting
            // bit (set on the approach detector (7,-3,4)) delayed by X is high at E's rise (ARM3 + 2):
            // a napper not yet at rest waits for the next cycle's pulse)
            if a && !arm_was && o.napper_lift {
                let now = if o.game_ticks { t } else { live };
                arms.push(now);
                let fire = match o.pulse_gate {
                    None => true,
                    // (live AND delayed (ironspec 11:49): the waiting bit itself (set on (7,-3,4),
                    // reset by the fired pulse, so not after the release) and its copy delayed X)
                    Some(x) if !o.gate_delayed_only => !hold_released && carts[n].w_exit.is_some_and(|we| now + 2 - we >= x),
                    // (the control: the delayed copy alone, still high for X gt after the reset, so the
                    // departing napper's own second ARM3 rise fires a phantom pulse)
                    Some(x) => carts[n].w_exit.is_some_and(|we| now + 2 - we >= x) && rel_at.is_none_or(|r| now + 2 < r + x),
                };
                if fire {
                    pulses.push(now + d);
                    fired_arms.push(now);
                } else {
                    skipped_pulses += 1;
                }
            }
            arm_was = a;
            let clock = if o.game_ticks { t } else { live };
            if let Some(a0) = arm_at
                && carts[n].id.is_none()
                && !o.napper_lift
                && (o.game_ticks || !frozen)
                && clock >= a0 + d
            {
                let nbt = "{Motion:[-0.4d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [8.5, 0.0625, if o.mirror_z { 3.5 - mz as f64 } else { mz as f64 + 0.5 }], Some(nbt))?;
                carts[n].id = Some(id);
                carts[n].launch_live = live;
                carts[n].rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
            // (`napper_lift`: the napper leaves R at `napper_r` (default 0), climbs its lift and the
            // staircase and waits on the hold (9,0,8))
            if o.napper_lift && carts[n].id.is_none() && t >= o.napper_r {
                let nbt = "{Motion:[0.2d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [4.5, -18.9375, if o.mirror_z { -0.5 } else { 4.5 }], Some(nbt))?;
                carts[n].id = Some(id);
                carts[n].launch_live = live;
                carts[n].rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
            // (the release pulse (day-1 07:03): the hold rail powered from ARM3 + D for `release_pulse`
            // gt (8; >= 6 launches from rest) every cycle (EMULATED: day-1's chain end, a redstone
            // block at the repeater's input (7,-1,8) from two gt before: the repeater's delay 1 is one
            // redstone tick, 2 gt; the rail's measured rise is printed)
            if o.napper_lift {
                let hold_on = matches!(run.world.get(BlockPos::new(9, 0, zf(8))), Block::Rail { activator: false, powered: true, .. });
                if fired_arms.iter().any(|a| a + 2 == clock) {
                    let st = carts[n].id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| {
                        let e = &run.world.entities.list[i];
                        let z = if o.mirror_z { 4.0 - e.pos.z } else { e.pos.z };
                        let v = (e.delta_movement.x * e.delta_movement.x + e.delta_movement.z * e.delta_movement.z).sqrt();
                        let on = (e.pos.x.floor() as i32, e.pos.y.floor() as i32, z.floor() as i32) == (9, 0, 8);
                        format!("t {clock}: {} ({v:.3})", if on && v < 0.005 { "at rest on the hold" } else if on { "rolling on the hold" } else { "not on the hold" })
                    });
                    samples.push(st.unwrap_or_else(|| format!("t {clock}: no napper")));
                }
                if o.tripwire {
                    let trip = matches!(run.world.get(BlockPos::new(8, 1, zf(7))), Block::Tripwire { powered: true, .. });
                    if trip && !trip_was && hold_released {
                        trips_seen.push(clock);
                    }
                    trip_was = trip;
                }
                if hold_on && !hold_was_on && hold_rise.is_none() {
                    hold_rise = arm_at.map(|a| clock - a);
                }
                hold_was_on = hold_on;
                if pulses.iter().any(|p| *p - 2 == clock) {
                    run.world.set_block(BlockPos::new(7, -1, zf(8)), Block::parse("minecraft:redstone_block")?, redsim::world::flags::ALL);
                }
                // (a napper on the hold at any tick of a pulse is launched by it: released by that
                // pulse (a napper arriving mid-pulse leaves on the pulse's tail, later than one
                // waiting at the rise; `hold_mid` records how much later))
                if let Some(p) = pulses.iter().copied().find(|p| clock >= *p && clock < *p + o.release_pulse) {
                    let at_hold = carts[n].id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| {
                        let e = &run.world.entities.list[i];
                        (e.pos.x, e.pos.y, if o.mirror_z { 4.0 - e.pos.z } else { e.pos.z })
                    });
                    if !hold_released && at_hold.is_some_and(|(x, y, z)| (x.floor() as i32, y.floor() as i32, z.floor() as i32) == (9, 0, 8)) {
                        hold_released = true;
                        nap_arm = Some(p - o.napper_delay.unwrap_or(0));
                        nap_b_pulse = arms.iter().position(|x| *x == p - o.napper_delay.unwrap_or(0)).or(Some(pulses.iter().position(|q| *q == p).unwrap_or(pulse_count)));
                        hold_mid = clock - p;
                        rel_at = Some(p);
                        carts[n].launch_live = live;
                        carts[n].launch_t = t;
                        hold_wait = at_hold;
                    }
                }
                if pulses.iter().any(|p| *p == clock) {
                    pulse_count += 1;
                    // (every pulse after the napper's departure off the hold (the napper's own roll over
                    // ARM3's detector makes one, day-1 11:04) must find the hold empty: bar 0)
                    if carts[n].w_leave.is_some()
                        && carts.iter().filter_map(|c| c.id).any(|id| {
                            redsim::entity::index_of(&run.world, id).is_some_and(|i| {
                                let e = &run.world.entities.list[i];
                                let z = if o.mirror_z { 4.0 - e.pos.z } else { e.pos.z };
                                (e.pos.x.floor() as i32, e.pos.y.floor() as i32, z.floor() as i32) == (9, 0, 8)
                            })
                        })
                    {
                        extra_busy += 1;
                    }
                }
                if pulses.iter().any(|p| *p - 2 + o.release_pulse == clock) {
                    run.world.set_block(BlockPos::new(7, -1, zf(8)), Block::parse("minecraft:air")?, redsim::world::flags::ALL);
                }
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
        if o.k_relay {
            // (`k_relay`, the latches without the row's detectors: J(-1) on at idle; lane -1's branch
            // rise turns it off and J(3) on; lane 3's turns J(3) off; lane 7's (the third cart, past
            // every K) turns K(n) on for a returning napper still to come (its own lane n's feed
            // (n-2,25,9)) and J(-1) on for the next batch, unless the napper must still pass J(-1)
            // (n east of it): then J(-1) waits until the napper is in F; the napper in F (EMULATED:
            // the bench sees its cart on F, the controller's napper bit) turns K(n) off)
            if rise(3) {
                lm = false;
                l3 = true;
            }
            if rise(2) {
                l3 = false;
            }
            // (the napper in flight: off the hold (or spawned on the merge, emulated), not yet in its
            // siding; the batch ahead of it is the one whose third cart armed its release)
            let in_flight = o.napper_delay.is_some() && carts[n].lane.is_none() && carts[n].id.is_some() && (!o.napper_lift || hold_released);
            if in_flight && nap_b.is_none() {
                nap_b = (0..o.batches).rev().find(|b| carts[b * 3 + 2].id.is_some());
            }
            if o.day1_taps {
                // (day-1's taps (07:50/08:38), after the napper's own ARM3 (the cycle whose pulse
                // released it), at D 12 shifted by D - 12; EMULATED as the levers at those ticks)
                let shift = o.napper_delay.unwrap_or(12) - 12;
                let clock = if o.game_ticks { t } else { live };
                let (set, reset) = match o.napper_lane {
                    -1 => (93, 125),
                    3 => (108, 140),
                    _ => (124, 156),
                };
                // (`dep_keyed` (ironspec 10:09, option (b)): every tap keyed to the napper's departure
                // off the hold (its first tick off (9,0,8), ARM3 + 16 in the nominal row), day-1's
                // dep + 77/92/108, + 109/124/140, J(-1) + 111: the same numbers less 16)
                let nap_ref = if o.dep_keyed && o.tripwire {
                    // (keyed to the string's first rising edge (ironspec 10:49): day-1's dep + offsets
                    // less the string's lag after the departure off the hold)
                    trips_seen.first().map(|tr| tr - o.trip_lag - 16 - shift)
                } else if o.dep_keyed {
                    carts.get(n).and_then(|c| c.w_leave).map(|d| d - 16 - shift)
                } else {
                    nap_arm
                };
                if let Some(a) = nap_ref {
                    if clock == a + set + shift + 4 && !k_on {
                        k_on = true;
                        k_on_at = Some(t);
                        drive_j(&mut run.world, true, o.napper_lane - 2, ex, zf(rz), true)?;
                    }
                    if clock == a + reset + shift + 6 && k_on {
                        k_on = false;
                        drive_j(&mut run.world, true, o.napper_lane - 2, ex, zf(rz), false)?;
                    }
                    if o.napper_lane != -1 && clock == a + 127 + shift + 4 {
                        j_deferred = false;
                        lm = true;
                    }
                }
                if rise(4) {
                    // (lane 7's branch rise re-arms J(-1), except the napper's own batch's (its third
                    // cart's rise lands at ARM3 + 121..125) when its lane is east of J(-1): day-1's
                    // re-arm tap does it then. The previous batch's rise lands at ARM3 + 21..25 of the
                    // napper's cycle and must still re-arm, so the suppression opens at ARM3 + 73,
                    // midway)
                    if o.napper_lane != -1 && nap_ref.is_some_and(|a| clock >= a + o.supp_open + shift && clock < a + 127 + shift + 4) {
                        j_deferred = true;
                    } else {
                        lm = true;
                    }
                }
            } else {
            if rise(4) {
                // (lane 7's branch rise, the third cart: J(-1) on for the next batch, unless the
                // napper must still pass J(-1) (its lane east of it))
                if in_flight && o.napper_lane != -1 {
                    j_deferred = true;
                } else {
                    lm = true;
                }
            }
            // (K(n) on once the batch's third cart is past it (EMULATED: the bench sees the cart; day-1's
            // driver must turn K on inside the window between the third cart and the napper))
            if in_flight
                && !k_on
                && let Some(b) = nap_b
            {
                let c3 = &carts[b * 3 + 2];
                let past = c3.lane.is_some()
                    || c3.id.and_then(|id| redsim::entity::index_of(&run.world, id)).is_some_and(|i| {
                        let p = run.world.entities.list[i].pos;
                        p.y.floor() as i32 == 25 && p.z.floor() as i32 == zf(9) && p.x >= (o.napper_lane - 2) as f64 + 1.0
                    });
                if past {
                    k_on = true;
                    k_on_at = Some(t);
                    drive_j(&mut run.world, true, o.napper_lane - 2, ex, zf(rz), true)?;
                }
            }
            // (J(-1) deferred for a napper bound east of it: on again once the napper is past J(-1)
            // (EMULATED: the bench sees the napper's cart; day-1's controller times it))
            if j_deferred
                && carts[n].id.and_then(|id| redsim::entity::index_of(&run.world, id)).is_some_and(|i| {
                    let p = run.world.entities.list[i].pos;
                    p.y.floor() as i32 == 25 && p.z.floor() as i32 == zf(9) && p.x >= 0.0
                })
            {
                j_deferred = false;
                lm = true;
            }
            if k_on && carts[n].lane.is_some() {
                k_on = false;
                drive_j(&mut run.world, true, o.napper_lane - 2, ex, zf(rz), false)?;
                if j_deferred {
                    j_deferred = false;
                    lm = true;
                }
            }
            }
        } else if o.reverse {
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
                drive_j(&mut run.world, o.row9 || o.row10, 7, ex, zf(rz), false)?;
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
            drive_j(&mut run.world, o.row9 || o.row10, 3, ex, zf(rz), l3)?;
            turns.push((t, 3, l3));
        }
        if lm != pm {
            drive_j(&mut run.world, o.row9 || o.row10, -1, ex, zf(rz), lm)?;
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
                    if pa.x < -4.0 && pb.x < -4.0 && pa.z.floor() as i32 == zf(mz) && pb.z.floor() as i32 == zf(mz) {
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
            // (`mirror_z`: the cart's state in the design frame)
            let s = if o.mirror_z { crate::liftbench::CartState { z: 4.0 - s.z, vz: -s.vz, yaw: -s.yaw, ..s } } else { s };
            let (x, y, z, vx, vz) = (s.x, s.y, s.z, s.vx, s.vz);
            let id = c.id.unwrap_or(0);
            let bp = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
            c.end = Some((x, y, z));
            // (waypoints for day-1 and 9fc3155f: T_D, the napper's trip legs)
            let cell = (bp.x, bp.y, bp.z);
            if k < n && c.w_d.is_none() && cell == (order[k % 3], 24, 8) {
                c.w_d = Some(t);
            }
            if k >= n {
                if c.w_lift12.is_none() && bp.x == 6 && bp.z == 4 && y >= -12.0 {
                    c.w_lift12 = Some(t);
                }
                if c.w_hold.is_none() && cell == (9, 0, 8) {
                    c.w_hold = Some(t);
                }
                // (the napper's entry onto the T (9,0,7) from the staircase: its stored speed)
                if c.v_t.is_none() && !hold_released && cell == (9, 0, mz) {
                    c.v_t = Some((vx * vx + vz * vz).sqrt());
                }
                // (day-1's approach detector (7,-3,4) and the rest on the hold: X for the pulse gate)
                if c.w_exit.is_none() && cell == (7, -3, 4) {
                    c.w_exit = Some(t);
                }
                if c.w_rest.is_none() && cell == (9, 0, 8) && (vx * vx + vz * vz).sqrt() < 0.005 {
                    c.w_rest = Some(t);
                }
                if c.w_leave.is_none() && hold_released && c.w_hold.is_some() && cell != (9, 0, 8) {
                    c.w_leave = Some(t);
                }
                if c.w_line.is_none() && cell == (8, 0, mz) {
                    c.w_line = Some(t);
                }
                if c.w_merge.is_none() && hold_released && cell == (4, 0, mz) {
                    c.w_merge = Some(t);
                }
                if c.w_k.is_none() && o.k_relay && cell == (o.napper_lane - 2, 25, 9) {
                    c.w_k = Some(t);
                    c.v_k = Some((vx * vx + vz * vz).sqrt());
                }
            }
            if c.w_j1.is_none() && cell == (-1, 25, 9) {
                c.w_j1 = Some(t);
            }
            // (the through-carts' least stored speed on a K's cell (ironfarm's S-bend margin), and the
            // single-occupant evidence at the napper hold: a batch cart on the hold's route (the T
            // (9,0,7), the hold's line (8,0,7), the hold (9,0,8)) counts, bar 0)
            if k < n && o.k_relay && [(-3, 25, 9), (1, 25, 9), (5, 25, 9)].contains(&cell) {
                let hs = (vx * vx + vz * vz).sqrt();
                thru_v_k = Some(thru_v_k.map_or(hs, |m: f64| m.min(hs)));
                thru_v_k_hi = Some(thru_v_k_hi.map_or(hs, |m: f64| m.max(hs)));
            }
            if k < n && o.napper_lift && [(9, 0, mz), (8, 0, mz), (9, 0, 8)].contains(&cell) && !c.on_hold_route {
                c.on_hold_route = true;
                hold_intrusions += 1;
            }
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
            // (`k_relay`: a cart reaching a siding's F (x-2,25,8) has taken lane x's siding feed)
            if o.k_relay && bp.y == 25 && bp.z == 8 && LANES.contains(&(bp.x + 2)) && c.lane.is_none() {
                c.lane = Some(bp.x + 2);
                c.merge_v = Some((vx * vx + vz * vz).sqrt());
                c.branch_t = Some(live - c.launch_live);
                sink.push(id);
            }
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
            drive_j(&mut run.world, o.row9 || o.row10, 7, ex, zf(rz), true)?;
        }
    }
    let mut wrong = 0;
    for (k, c) in carts.iter().enumerate() {
        let want = if k >= n { if o.k_relay { o.napper_lane } else { 9 } } else { order[k % 3] };
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
    // (the napper's gaps against the batch it follows into the shaft: behind that batch's third cart,
    // ahead of the next batch's first)
    let nb = met_abs[n].and_then(|m| (0..o.batches).rev().find(|b| met_abs[b * 3 + 2].is_some_and(|t3| t3 < m)));
    let nap_behind = met_abs[n].zip(nb.and_then(|b| met_abs[b * 3 + 2])).map(|(m, t3)| m - t3);
    let nap_ahead = met_abs[n].zip(nb.filter(|b| b * 3 + 3 < n).and_then(|b| met_abs.get(b * 3 + 3).copied().flatten())).map(|(m, f1)| f1 - m);
    let nap_last = nb.is_some_and(|b| b * 3 + 3 >= n);
    let napper_gaps = if o.napper_delay.is_some() {
        format!(
            "; the napper {} behind batch {}'s third cart, {} ahead of the next batch's first",
            nap_behind.map_or("-".into(), |g| g.to_string()),
            nb.map_or("-".into(), |b| b.to_string()),
            nap_ahead.map_or("-".into(), |g| g.to_string())
        )
    } else {
        String::new()
    };
    out.push(format!("shaft entry gaps (live gt): {}{napper_gaps}", gaps.join(", ")));
    // (cc767b78's hold-release line (ironspec's (c') bar): the fired pulses, those fired while the
    // napper rested on the hold, the skipped ones; the departure on a fired pulse (the hold rail
    // powered at the departure tick) or not; creeps (a departure with no pulse powering); FIFO pops
    // (fired pulses) without a departure)
    let nap_c = carts.get(n);
    let (rest_t, leave_t) = (nap_c.and_then(|c| c.w_rest), nap_c.and_then(|c| c.w_leave));
    let rested: Vec<usize> = pulses.iter().enumerate().filter(|(_, p)| rest_t.is_some_and(|r| **p >= r) && leave_t.is_none_or(|l| **p <= l)).map(|(i, _)| i + 1).collect();
    let on_fired = leave_t.and_then(|l| pulses.iter().position(|p| l >= *p && l < *p + o.release_pulse).map(|i| (i + 1, l)));
    let creeps = usize::from(leave_t.is_some() && on_fired.is_none());
    let pops_wo_dep = pulses.len().saturating_sub(usize::from(on_fired.is_some()));
    let pops_list: Vec<String> = pulses.iter().filter(|p| !leave_t.is_some_and(|l| l >= **p && l < **p + o.release_pulse)).map(|p| p.to_string()).collect();
    // (the departing napper's own roll over ARM3's detector (5,0,7): the first arming rise within
    // 40 gt after its departure; with the gate it must fire nothing)
    let nap_rise = leave_t.and_then(|l| arms.iter().copied().find(|a| *a > l && *a <= l + 40));
    let nap_rise_s = match nap_rise {
        None => "none (no second rise)".to_string(),
        Some(a) => match pulses.iter().position(|p| *p == a + o.napper_delay.unwrap_or(0)) {
            Some(i) if fired_arms.contains(&a) => format!("at t {a}: fired #{}", i + 1),
            _ => format!("at t {a}: nothing"),
        },
    };
    let hold_line = format!(
        "hold release: pulses fired {} (while the napper rested: {}), skipped {}; the napper's departure {}; creeps {} (the napper off the hold with no pulse); FIFO pops without a departure {}; fired pulses without that cycle's departure {}{}; the napper's second ARM3 rise {}; presence samples at E's rise: {}",
        pulses.len(),
        if rested.is_empty() { "none".to_string() } else { rested.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(", ") },
        skipped_pulses,
        match on_fired {
            Some((k, l)) => format!("on fired pulse #{k} at t {l} (ARM3 + {})", l - pulses[k - 1] + o.napper_delay.unwrap_or(0)),
            None if leave_t.is_some() => "NOT on a fired pulse".to_string(),
            None => "none (it never left)".to_string(),
        },
        creeps,
        pops_wo_dep,
        pops_list.len(),
        if pops_list.is_empty() { String::new() } else { format!(" (rising at t {})", pops_list.join(", ")) },
        nap_rise_s,
        if samples.is_empty() { "none".to_string() } else { samples.join("; ") }
    );
    let release_bad = o.pulse_gate.is_some() && (rested.len() > 1 || (leave_t.is_some() && on_fired.is_none()) || creeps > 0 || pops_wo_dep > 0);
    if o.napper_lift {
        out.push(format!(
            "(c)'s napper lift: the napper from R at t {}, on the hold when a pulse ({} gt) launched it: {} ({} pulse(s) in the run; it was first seen on the hold {} gt into that pulse); the hold rail's first rise ARM3 + {}",
            o.napper_r,
            o.release_pulse,
            hold_wait.map_or("NOT on the hold at any pulse".into(), |(x, y, z)| format!("at {x:.3},{y:.3},{z:.3}{}", if (x.floor() as i32, y.floor() as i32, z.floor() as i32) == (9, 0, 8) { " (on the hold)" } else { " (NOT on the hold)" })),
            pulse_count,
            hold_mid,
            hold_rise.map_or("-".into(), |r| r.to_string())
        ));
        if o.tripwire {
            out.push(format!(
                "tripwire (8,1,7): tripped at t {:?}; the napper off the hold at t {}, on (8,0,7) from t {}",
                trips_seen,
                carts.get(n).and_then(|c| c.w_leave).map_or("-".into(), |w| w.to_string()),
                carts.get(n).and_then(|c| c.w_line).map_or("-".into(), |w| w.to_string())
            ));
        }
        out.push(hold_line.clone());
        if o.k_relay {
            let nap_c = carts.get(n);
            out.push(format!(
                "K and J(-1) for the napper (after its own ARM3 at {}): entering K({}) at stored {} (tick {}); on J(-1)'s cell at ARM3 + {}, the next batch's first cart on it at ARM3 + {}; the through-carts' least stored speed on a K's cell {}; batch carts on the napper hold's route (the T (9,0,7), (8,0,7), the hold; bar 0) {}",
                nap_arm.map_or("-".into(), |a| a.to_string()),
                o.napper_lane - 2,
                nap_c.and_then(|c| c.v_k).map_or("-".into(), |v| format!("{v:.3}")),
                nap_c.and_then(|c| c.w_k).map_or("-".into(), |w| w.to_string()),
                nap_arm.zip(nap_c.and_then(|c| c.w_j1)).map_or("-".into(), |(a, w)| (w - a).to_string()),
                nap_arm.zip(nap_b_pulse.filter(|b| (b + 1) * 3 < n).and_then(|b| carts.get((b + 1) * 3)).and_then(|c| c.w_j1)).map_or("-".into(), |(a, w)| (w - a).to_string()),
                thru_v_k.map_or("-".into(), |v| format!("{v:.3}")),
                hold_intrusions
            ));
        }
    }
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
        napper_off: o.napper_delay.is_some() && carts.get(n).is_none_or(|c| c.lane != Some(if o.k_relay { o.napper_lane } else { 9 })),
        riders_bad: carts.iter().filter(|c| c.hurt > 0.0 || c.eye_in_wall > 0 || c.dismounted.is_some()).count(),
        restarted,
        gap_min,
        napper_behind: if o.napper_delay.is_some() { nap_behind.or(Some(i64::MIN)) } else { None },
        napper_ahead: if o.napper_delay.is_some() && !nap_last { nap_ahead.or(Some(i64::MIN)) } else { None },
        v_branch: range(&|c: &Cart| c.branch_v),
        v_merge: range(&|c: &Cart| c.merge_v),
        head: carts.iter().map(|c| c.head).fold(f64::NEG_INFINITY, f64::max),
        trips: {
            let nap = carts.get(n);
            let sub = |a: Option<i64>, b: Option<i64>| a.zip(b).map(|(a, b)| a - b);
            let td = |lane: i32| (0..3).find(|i| order[*i] == lane).and_then(|i| carts.get(i)).and_then(|c| c.w_d.map(|d| d - c.launch_t));
            [
                nap.and_then(|c| sub(c.w_hold, c.w_lift12)),
                nap.and_then(|c| if hold_released { sub(c.w_merge, Some(c.launch_t)) } else { None }),
                nap.and_then(|c| sub(c.w_k, c.w_merge)),
                nap.and_then(|c| sub(c.w_k, k_on_at)),
                td(-1),
                td(3),
                td(7),
            ]
        },
        nap_v_k: carts.get(n).and_then(|c| c.v_k),
        j1: (
            nap_arm.zip(carts.get(n).and_then(|c| c.w_j1)).map(|(a, w)| w - a),
            nap_arm.zip(nap_b_pulse.filter(|b| (b + 1) * 3 < n).and_then(|b| carts.get((b + 1) * 3)).and_then(|c| c.w_j1)).map(|(a, w)| w - a),
        ),
        thru_v_k,
        hold_intrusions,
        thru_v_k_hi,
        approach: carts.get(n).and_then(|c| c.w_rest.zip(c.w_exit)).map(|(r, e)| r - e),
        stop_to_hold: carts.get(n).and_then(|c| c.w_rest).map(|r| r - o.napper_r),
        stop_to_k: carts.get(n).and_then(|c| c.w_k).map(|k| k - o.napper_r),
        departure: carts.get(n).and_then(|c| c.w_leave).and_then(|l| {
            // (against the ARM3 of the pulse it left on: the latest arming whose pulse rose before
            // its departure)
            let d = o.napper_delay.unwrap_or(0);
            arms.iter().copied().filter(|a| a + d <= l).max().map(|a| l - a)
        }),
        skipped_pulses,
        pulses_run: pulse_count,
        extra_busy,
        trip_after_dep: trips_seen.first().zip(carts.get(n).and_then(|c| c.w_leave)).map(|(tr, d)| tr - d),
        v_t: carts.get(n).and_then(|c| c.v_t),
        release_bad,
        t_phase: carts.get(n).and_then(|c| c.w_hold).and_then(|h| arms.iter().copied().filter(|a| *a <= h).max().map(|a| h - a)),
        pulse_mismatch: nap_arm.map(|a| {
            // (the consuming arming: the first whose E rise (ARM3 + 2) is >= detection + the bit's delay
            // (the gate's X when gated))
            let det = carts.get(n).and_then(|c| c.w_exit);
            let bd = o.pulse_gate.unwrap_or(o.bit_delay);
            let consumed = det.and_then(|d| arms.iter().copied().find(|&x| x + 2 >= d + bd));
            consumed != Some(a)
        }),
    };
    out.extend(trace);
    Ok((out.join("\n"), summary))
}
