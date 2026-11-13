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
#[derive(Clone, Debug, Default)]
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
    /// The napper lift's extra depth (ironspec 12:23: the stop 9 down; R at (4,-19-N,4)).
    pub lift_depth: i32,
    /// The always-on redstone blocks laid as before V15 (no torches: the boosters on redstone
    /// blocks, the hold's support stone).
    pub no_torches: bool,
    /// `napper_lift`: the napper's start speed at R (+x, b/gt; tick-exact's official starts 0.18,
    /// 0.20, 0.22, 0.25).
    pub napper_speed: f64,
    /// ironspec 14:11/14:16 path (a): lane 7 off the merge row (its cart started at rest on the
    /// braked slope (-4,12,mz) in the lift's east wall, released at REL + `lane7_rel` for 10 gt), the
    /// merge row's west end ramped into the lift at y 2 (the lift's foot at y 2), (7,0,mz) a plain rail.
    pub path_a: bool,
    /// ARM3 := REL + c (day-1 14:41): the arming at a fixed tick of each batch's cycle instead of the
    /// detector (5,0,mz) (a plain rail then).
    pub arm_rel: Option<i64>,
    /// `path_a`: lane 7's release, gt after its batch's REL (ironspec: 100).
    pub lane7_rel: i64,
    /// The batch carts spawn this many gt after REL (the emulated station's cart starts at z 1.5 at
    /// `speed`; the real pocket release reaches that later: dfb89499's lane -1 passes y 12 ~11 gt
    /// after this bench's at lag 0). REL (lane 7's release, ARM3 := REL + c) stays the cycle's start.
    pub station_lag: i64,
    /// `path_a`'s y-12 bar on the batch pairs (barred in clean() at G1's ask on 83878113f; the number
    /// is the column bar 14 + 2 = 16, ironspec 19:07 / 20:01 PT): lane 3 -> lane 7 and lane 7 -> the
    /// next batch's lane -1 when no napper is between; with the column order.
    pub y12_min: i64,
    /// The napper's y-12 pairs (after its batch's lane 7, before the next batch's first): the column
    /// bar 14 + 2 = 16 (ironspec 19:07 / 20:01 PT).
    pub y12_nap_min: i64,
    /// `path_a`: lane 7's release pulse length (gt) at its drive (EMULATED).
    pub lane7_pulse: i64,
    /// `day1_taps`: day-1's K-set taps this many gt later (day-1 17:59: +4 under path (a)).
    pub kset_shift: i64,
    /// ironspec 19:38 (3) / day-1: every junction switch through day-1's driver, the rail switching 4 gt
    /// after a set edge and 6 after a reset edge (the detector-triggered ones; the taps carry it).
    pub driver_latency: bool,
    /// ironspec 20:01 PT (2): lane 7's entry at least this many gt after the preceding cart's box has
    /// fully cleared the mouth cell at y 12 (its feet at y 13), in every row.
    pub mouth_min: i64,
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
    /// ironspec 20:16: dfb89499's record station part (a design file, loop frame) composed over the
    /// bench's world instead of the emulated station; the batch carts start at rest in its pockets
    /// (x,0,0) and leave through its own drive (a lever stand-in at (x-2,0,0) on glass, on at
    /// REL + offset - 1 for 16 gt, into day-1's repeater (x-1,0,0)).
    pub station_part: Option<String>,
    /// 9fc3155f / ironspec 22:34 PT (5) (c), priced: block light >= 1 in every lift water cell by two
    /// glowstone blocks in place of two wall glass blocks, the shared lift's west wall (-6,8,mz) and
    /// the napper climb's west wall (5,-16,4) (9fc3155f's statics on record7: (7,-16,4) made a stroll target; glowstone, like glass, is refused by
    /// LEGACY_IRON_GOLEM spawning and conducts no redstone; it is opaque where glass is clear).
    pub light_fix: bool,
    /// `--set X,Y,Z=STATE`: candidate cells laid over the build (the bench frame), each printed on the
    /// run line (ironspec 23:46 PT: b8949c96's B4 row).
    pub set_cells: Vec<String>,
}

const LANES: [i32; 3] = [7, 3, -1];

fn build(o: &ReturnOpts) -> Result<FarmSpec, String> {
    let glass = Block::parse("minecraft:glass")?;
    let water = Block::parse("minecraft:water[level=0]")?;
    let rail = |s: &str| Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"));
    let det = |s: &str| Block::parse(&format!("minecraft:detector_rail[powered=false,shape={s},waterlogged=false]"));
    // (V15 (ironspec 14:01; 9fc3155f's cells): the always-on redstone blocks under the boosters
    // replaced by redstone torches on glass; `no_torches` lays the blocks as before)
    let torches = !o.no_torches;
    let torch = Block::parse("minecraft:redstone_torch[lit=true]")?;
    let slime = Block::parse("minecraft:slime_block")?;
    let mut b: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, k: &Block| b.push((BlockPos::new(x, y, z), k.clone()));
    // (the merge; `z7`: 9fc3155f's shift (ironspec 01:02), the merge row, the lift and the hold one
    // cell north (z 8 -> z 7), the split unchanged)
    let mz = if o.z7 { 7 } else { 8 };
    for x in LANES {
        // (`path_a` (ironspec 14:11/14:16): lane 7 leaves the station up dfb89499's ramp and climb
        // to the braked slope (-4,12,mz) in the lift's east wall; not modelled here (the bench starts
        // lane 7's cart at rest on that slope))
        if o.path_a && x == 7 {
            continue;
        }
        // (`station_part` (ironspec 20:16): dfb89499's record station composed instead: no emulated
        // station cells)
        if o.station_part.is_some() {
            continue;
        }
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
            put(x, -1, z, &if boost && !torches { Block::parse("minecraft:redstone_block")? } else { glass.clone() });
        }
        // (V15, dfb89499's station half emulated here: each lane's boosters powered by a torch at
        // (x-1,0,mz-2) on glass, beside the middle booster (the chain carries it to the other two))
        if o.boosters && torches {
            put(x - 1, 0, mz - 2, &torch);
            put(x - 1, -1, mz - 2, &glass);
        }
    }
    // (`path_a`: lane 7 off the merge row, so (7,0,mz) a plain east_west rail (2 arms: the hold's
    // line and the row), no longer a T)
    put(7, 0, mz, &if o.path_a { rail("east_west")? } else { rail("north_west")? });
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
        // (`lift_depth` (ironspec 12:23: the stop 9 down): R, the entry and the soul sand that many
        // levels lower, the bubble column that much longer)
        let rb_y = -19 - o.lift_depth;
        put(4, rb_y, 4, &rail("east_west")?);
        put(4, rb_y - 1, 4, &glass);
        put(5, rb_y, 4, &rail("east_west")?);
        put(5, rb_y - 1, 4, &glass);
        put(6, rb_y - 1, 4, &Block::parse("minecraft:soul_sand")?);
        for y in rb_y..=-4 {
            put(6, y, 4, &water);
            put(7, y, 4, &glass);
            put(6, y, 3, &glass);
            put(6, y, 5, &glass);
            if y > rb_y {
                put(5, y, 4, &if o.light_fix && y == -16 { Block::parse("minecraft:glowstone")? } else { glass.clone() });
            }
        }
        put(7, rb_y - 1, 4, &glass);
        put(6, rb_y - 1, 3, &glass);
        put(6, rb_y - 1, 5, &glass);
        put(6, -3, 3, &glass);
        put(6, -3, 5, &glass);
        put(6, -2, 4, &glass);
        put(5, -3, 4, &water);
        // (ironspec 01:07 PT (2a): the napper climb's top-drift current saved settled: (5,-3,4)
        // spreads one cell into the climb's top (6,-3,4), level 1, and stops)
        put(6, -3, 4, &Block::parse("minecraft:water[level=1]")?);
        put(4, -3, 4, &glass);
        put(5, -3, 3, &glass);
        put(5, -3, 5, &glass);
        put(5, -2, 4, &glass);
        // (`pulse_gate`: the exit rail is day-1's approach detector (the waiting bit's source, read from
        // its side; its support (7,-4,4) glass, no T within its update reach))
        put(7, -3, 4, &if o.pulse_gate.is_some() { det("east_west")? } else { rail("east_west")? });
        put(8, -3, 4, &pr("ascending_east", true)?);
        put(8, -4, 4, &rb);
        put(9, -2, 4, &rail("south_west")?);
        put(9, -3, 4, &glass);
        put(9, -2, 5, &pr("ascending_south", true)?);
        put(9, -3, 5, if torches { &glass } else { &rb });
        put(9, -1, 6, &pr("ascending_south", true)?);
        put(9, -2, 6, if torches { &glass } else { &rb });
        // (V15: the climb's chain (9,-2,5) -> (9,-1,6) powered by the torch (8,-2,5) on glass)
        if torches {
            put(8, -2, 5, &torch);
            put(8, -3, 5, &glass);
        }
        put(9, 0, 7, &rail("south_west")?);
        put(9, -1, 7, &glass);
        // (the hold's line a booster: the napper joins at the batches' speed; its power touches no
        // face of lane 7's curve T; V15: on slime (8,-1,7) strongly powered by the torch (8,-2,7) on
        // glass (8,-3,7) under it (a torch beside the rail at (8,0,6) would re-settle lane 7's
        // merge T (7,0,7) by its Manhattan-2 updates, 9fc3155f))
        put(8, 0, mz, &pr("east_west", true)?);
        if torches {
            put(8, -1, mz, &slime);
            put(8, -2, mz, &torch);
            put(8, -3, mz, &glass);
        } else {
            put(8, -1, mz, &rb);
        }
        put(9, 0, 8, &pr("north_south", false)?);
        // (the hold's drive (day-1 07:03, G1 07:02): glass can't be powered, so the hold rail sits on
        // stone (9,-1,8) (a conductor) strongly powered by the repeater (8,-1,8) facing west (its
        // output east into the stone; its input (7,-1,8), day-1's chain; no T touches the repeater,
        // its input cell or the stone: a repeater beside the rail at (8,0,8) would take its input
        // at (7,0,8), a face of lane 7's curve T (7,0,7), whose placement-order shape a signal
        // source's update there would re-settle)
        // (G1 / V15: the hold's support (9,-1,8) is slime, as the stone was a conductor: the repeater's
        // strong power reaches the rail through it; --no-torches keeps it slime too)
        put(9, -1, 8, &slime);
        put(8, -1, 8, &Block::parse("minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?);
        put(8, -2, 8, &glass);
        // (ironspec 18:16 (6) / 18:24: day-1's input (7,-1,8) EMULATED by a conforming stand-in, a
        // floor lever on glass (7,-2,8), toggled for the pulse (not a redstone block: that one was a
        // candidate and an off-pad golem spot); a floor lever, not 9fc3155f's wall lever on (7,-1,7):
        // a wall lever's toggle updates (7,-1,7)'s neighbours, among them V15's T (7,0,7), whose
        // stored shape differs from its unpowered settle; on (7,-2,8) its updates reach no T)
        put(7, -1, 8, &Block::parse("minecraft:lever[face=floor,facing=north,powered=false]")?);
        put(7, -2, 8, &glass);
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
    // (`path_a` (2b): the row's west end ramped, (-2,0,mz) and (-3,1,mz) ascending west on glass,
    // the entry (-4,2,mz) east_west on the wall's glass (-4,1,mz): lanes -1, 3 and the napper enter
    // the lift at y 2; the arming detector (5,0,mz) a plain rail (ARM3 := REL + c, day-1 14:41))
    for x in (if o.path_a { -1 } else { -4 })..=6 {
        // (with `boosters` (the reverse order) the arming detector on the main line (5,0,mz), past the
        // boosters (the speed pinned), two cells off lane 7's curve and lane 3's T: only lane 7's carts
        // reach it before the napper)
        let r = if x == 3 || x == -1 { rail("north_west")? } else if o.boosters && o.napper_delay.is_some() && !(o.napper_lift && o.arm_station) && !o.path_a && o.arm_rel.is_none() && x == 5 { det("east_west")? } else { rail("east_west")? };
        put(x, 0, mz, &r);
        put(x, -1, mz, &glass);
    }
    if o.path_a {
        put(-2, 0, mz, &rail("ascending_west")?);
        put(-2, -1, mz, &glass);
        put(-3, 1, mz, &rail("ascending_west")?);
        put(-3, 0, mz, &glass);
        put(-4, 2, mz, &rail("east_west")?);
        // (G1 on dfb89499's d8394ed6e: the old intake cell (-4,0,mz) defined: glass, the closed wall
        // entry under the entry's glass (-4,1,mz))
        put(-4, 0, mz, &glass);
    }
    put(-4, 1, mz, &glass);
    // (the shared lift: water to `top`, the exit one over it; `flat`: the lift one lower and the
    // split flat at the exit's level, the branches with it (9fc3155f: "if the split runs flat at
    // the exit's level instead, the lift and the module stay one lower"))
    let top = if o.row9 && (o.k_relay || o.climb2) { 22 } else if o.row10 || o.row9 { 24 } else if o.flat || o.dip { 21 } else { 22 };
    let ex = top + 1;
    // (`path_a`: the lift's foot at y 2 (soul sand (-5,1,mz) on glass (-5,0,mz)), the column from
    // y 2; the east wall's y 12 the intake slope)
    let foot = if o.path_a { 2 } else { 0 };
    put(-5, foot - 1, mz, &Block::parse("minecraft:soul_sand")?);
    put(-5, foot - 2, mz, &glass);
    for y in foot..=top {
        put(-5, y, mz, &water);
        put(-6, y, mz, &if o.light_fix && y == 8 { Block::parse("minecraft:glowstone")? } else { glass.clone() });
        put(-5, y, mz - 1, &glass);
        put(-5, y, mz + 1, &glass);
        if y >= foot + 1 && !(o.path_a && (y == 12 || y == 13)) {
            put(-4, y, mz, &glass);
        }
    }
    // (`path_a` (2c)'s intake (ironspec 14:16): the braked powered slope (-4,12,mz) ascending_east
    // in the east wall (lane 7 rests on it; released at REL + `lane7_rel` for `lane7_pulse` gt by
    // day-1's drive at (-4,12,mz-1), the bench's ceiling lever stand-in below); the climb's top
    // (-3,13,mz) dfb89499's (a stand-in here, or `--station-part`'s own))
    if o.path_a {
        put(-4, 12, mz, &Block::parse("minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false]")?);
        // (dfb89499 15:05: the cell over the slope clear for the cart's box: an oak wall sign
        // (-4,13,mz) facing south (no collision box; waterloggable, so the column's water stays in) on
        // new glass (-4,13,mz-1))
        put(-4, 13, mz, &Block::parse("minecraft:oak_wall_sign[facing=south,waterlogged=false]")?);
        put(-4, 13, mz - 1, &glass);
        // (G1 on 5ec2749b3 / onechunk's record7: day-1's drive EMULATED by a ceiling lever (-4,12,mz-1)
        // hung from the sign's glass (-4,13,mz-1), no new support (no glass where the build has air:
        // the sight property), toggled for the pulse in place of day-1's repeater (-4,12,mz-1) and its
        // input (-4,12,mz-2); not this file's)
        put(-4, 12, mz - 1, &Block::parse("minecraft:lever[face=ceiling,facing=north,powered=false]")?);
        if o.station_part.is_none() {
            put(-3, 13, mz, &rail("east_west")?);
        }
        put(-3, 12, mz, &glass);
    }
    put(-6, ex, mz, &glass);
    put(-4, ex, mz, &glass);
    put(-5, ex + 1, mz, &glass);
    put(-5, ex, mz - 1, &water);
    // (ironspec 01:07 PT (2a): the top-drift current saved settled: the source (-5,ex,mz-1) spreads
    // one cell into the top cell, level 1, and stops; laid here so the settle makes no new water)
    put(-5, ex, mz, &Block::parse("minecraft:water[level=1]")?);
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
        // (V15: the side power a redstone torch on glass (-6,ex-1,8); the cap kept as measured
        // (9fc3155f 14:03: removing it is fine statically, pending its print))
        if torches {
            put(-6, ex, 8, &torch);
            put(-6, ex - 1, 8, &glass);
        } else {
            put(-6, ex, 8, &rb);
        }
        // (the redstone block's top capped with glass (onechunk 08:41: open, a golem spawn spot; the
        // cap is off the riders' boxes and eye path, which stay in x -5..-4); V15b: no cap over the
        // torch (9fc3155f 14:41: no candidate either way, 0 new cells))
        if !torches {
            put(-6, ex + 1, 8, &glass);
        }
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
            // (the napper's leg east of J(7): (8,25,9) east_west and (9,25,9) east_west, the shape a rail
            // with only its west neighbour settles to here; record7's (9,25,9) is the way-in's curve
            // north_west (b8949c96's file): a declared stand-in (`# stand-in:`; ironspec 23:50 PT))
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
            lo: BlockPos::new(-20, if o.napper_lift { -22 - o.lift_depth } else { -3 }, -2),
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
            // (the napper's leg east of J(7): (8,25,9) east_west and (9,25,9) east_west, the shape a rail
            // with only its west neighbour settles to here; record7's (9,25,9) is the way-in's curve
            // north_west (b8949c96's file): a declared stand-in (`# stand-in:`; ironspec 23:50 PT))
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
            lo: BlockPos::new(-20, if o.napper_lift { -22 - o.lift_depth } else { -3 }, -2),
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
    // (z7 moves the lift's south wall to z 8, so the curve needs its own support; Run::start refuses
    // a popped cell since 0f375d1c0)
    if mz == 7 {
        put(-5, ex - 1, 9, &glass);
    }
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
    // (`--set`: candidate cells over the build, `X,Y,Z=STATE` each, the bench frame (ironspec 23:46 PT:
    // b8949c96's B4 row))
    for s in &o.set_cells {
        let (c, st) = s.split_once('=').ok_or_else(|| format!("--set {s}: X,Y,Z=STATE"))?;
        let v: Vec<i32> = c.split(',').map(|t| t.trim().parse::<i32>().map_err(|e| format!("--set {s}: {e}"))).collect::<Result<_, _>>()?;
        if v.len() != 3 {
            return Err(format!("--set {s}: X,Y,Z=STATE"));
        }
        put(v[0], v[1], v[2], &Block::parse(st)?);
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
    /// The rider's air each tick in the lift's column (and the side slope's cells), run-length.
    air_trace: Vec<(i32, u32)>,
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
    /// The napper: first in the lift's top cell (6,-3,4); first at rest on the hold with no motion.
    w_top: Option<i64>,
    w_rest0: Option<i64>,
    /// First tick in the lift's column at y >= 12 (path (a)'s side entry level).
    w_y12: Option<i64>,
    /// First tick in the lift's column with its feet at y >= 13 (its box out of the mouth cell).
    w_y13: Option<i64>,
    /// First tick on the exit booster's cell (-5,ex,mz+1): the column's contact point.
    w_top_exit: Option<i64>,
    /// `station_part`: the first tick the cart moves (its departure from the pocket), and its first
    /// tick on its lane's merge-row cell (x,0,mz) with its speed there (ironspec 20:16's observables).
    w_dep: Option<i64>,
    w_row: Option<(i64, f64)>,
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
/// The (2c) switch rule's margins, one convention for every junction line (G1 on 5ec2749b3).
///
/// The bench's loop index t is: the controller's reads (detector states, as the previous tick left
/// them) and writes (the latches' `drive_j`, the taps, the drives), then `run.tick()`, the game tick
/// t (block phase, then each cart reads the rail under it at the start of its entity tick and moves),
/// then the per-cart records (positions at the end of tick t). So a cart recorded in a junction's cell
/// at indices e..=l starts ticks e + 1..=l + 1 there and reads the junction in those ticks, and a
/// switch written at index s is read from tick s. The rule (tick-exact's official boundary row): a
/// switch lands strictly after the old-state cart's last read and no later than the new-state cart's
/// first: settle = (e + 1) - s >= 0 and clear = s - (l + 1) - 1 >= 0 are the physical edges; the bar
/// adds the cause's window (block phase 1 gt, entity phase 2) + 2. A timed tap (the K taps, J(-1)'s
/// re-arm, day-1's driver output) is written at its tick, the block phase's equivalent: its margins as
/// written. A detector press happens in the entity phase of tick p and reaches the bench's controller
/// at index p + 1 (`drive_j` there, read from tick p + 1), so its margins timed from the press tick
/// (`from_press`, ironspec 19:08 / 19:14) are settle + 1 and clear - 1.
pub fn settle_margin(s: i64, first_rec: i64, from_press: bool) -> i64 {
    (first_rec + 1) - s + i64::from(from_press)
}

/// See [`settle_margin`]: the clear side, against the old-state cart's last record in the cell.
pub fn clear_margin(s: i64, last_rec: i64, from_press: bool) -> i64 {
    s - (last_rec + 1) - 1 - i64::from(from_press)
}

#[cfg(test)]
mod switch_margin_tests {
    use super::{clear_margin, settle_margin};

    /// A scripted timeline: an old-state cart recorded in the junction's cell at indices 9..=11 (so it
    /// reads the junction in ticks 10..=12), a new-state cart first recorded there at 19 (first read in
    /// tick 20).
    #[test]
    fn the_2c_edges() {
        // (a tap written at 13, read from tick 13: strictly after the old cart's last read 12: clear 0,
        // the edge; written at 12 the old cart reads the new shape in its last tick: a violation)
        assert_eq!(clear_margin(13, 11, false), 0);
        assert_eq!(clear_margin(12, 11, false), -1);
        // (written at 20, the new cart's first read: settle 0, the edge; at 21 it reads the old shape)
        assert_eq!(settle_margin(20, 19, false), 0);
        assert_eq!(settle_margin(21, 19, false), -1);
        // (a press in tick 15 reaches the controller at index 16: timed from the press tick, settle + 1
        // and clear - 1 against the bench's switch at 16)
        assert_eq!(settle_margin(16, 19, true), 20 - 15);
        assert_eq!(clear_margin(16, 11, true), 15 - 12 - 1);
        assert_eq!(settle_margin(16, 19, true), settle_margin(16, 19, false) + 1);
        assert_eq!(clear_margin(16, 11, true), clear_margin(16, 11, false) - 1);
        // (day-1's driver: a set press in tick 15, seen at index 16, queued to 16 + 3 = 19 = press + 4,
        // read from tick 19 (block phase): margins as written; a reset to 16 + 5 = 21 = press + 6)
        assert_eq!(settle_margin(16 + 3, 19, false), 1);
        assert_eq!(clear_margin(16 + 5, 11, false), 21 - 12 - 1);
    }
}

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
    /// The approach detector's own rise -> the napper at rest on the hold (|v| < 0.005; and |v| = 0),
    /// and the tick it first reached the lift's top cell (6,-3,4) (ironspec 14:36's cross-check).
    pub det_to_rest: Option<i64>,
    pub det_to_rest0: Option<i64>,
    pub t_top: Option<i64>,
    /// ARM3 - REL per batch (the arming's first rise in each batch's cycle; day-1's c; batches 0..8).
    pub arm_c: [Option<i64>; 8],
    /// The napper's y-12 pass in the column after its fired pulse's rail rise (gt).
    pub pulse_to_y12: Option<i64>,
    /// Each cart's first tick in the column at y >= 12, against its batch's REL (the napper: against
    /// the REL of the batch it follows; carts 0..15); the least centre distance between two carts in
    /// the column.
    pub y12_rel: [Option<i64>; 16],
    pub nap_index: usize,
    pub col_min: Option<f64>,
    /// The napper's y-12 pass against the batch carts' nearest before and after it (ironspec's
    /// >= 21 gt both sides at y 12, path (a)).
    pub gap12_behind: Option<i64>,
    pub gap12_ahead: Option<i64>,
    /// `path_a`'s y-12 pairs, least over the batches: [lane -1 -> lane 3 (the foot pair: barred by
    /// the shaft entry's gap, printed here), lane 3 -> lane 7, lane 7 -> the napper, the napper -> the
    /// next batch's lane -1, lane 7 -> the next batch's lane -1 with no napper between]; whether a
    /// barred pair is under `y12_min`, a cart never passed y 12, or the column order is wrong.
    pub y12_pairs: [Option<i64>; 5],
    pub y12_order_bad: bool,
    pub y12_bad: bool,
    /// Why the path (a) y-12 bar failed ("" when it passed; "lane 7's mouth clearance unmeasured"
    /// apart from a margin under its bar: G1 on 5ec2749b3).
    pub y12_why: &'static str,
    /// The most stored speed on J(-1), J(3), J(7) over every cart (ironspec 17:20 (b)).
    pub j_vmax: [f64; 3],
    /// [J(-1), J(3)]: the least settle (switch -> the first cart needing the new state) and clear
    /// (the last cart needing the old state out of the cell -> the switch), ironspec 17:20 (b).
    pub j_settle: [Option<i64>; 2],
    pub j_clear: [Option<i64>; 2],
    pub j_clear_cart: [Option<i64>; 2],
    /// The napper's K under day-1's timed taps: on clear / settle, off margin (the napper out first) /
    /// the next batch cart after off (ironspec 18:16 (3)).
    pub k_clear: Option<i64>,
    pub k_settle: Option<i64>,
    pub k_off_margin: Option<i64>,
    pub k_off_settle: Option<i64>,
    /// Per switch cause (ironspec 19:14): [K set, K reset, J(-1) re-arm, J(-1) set, J(-1) reset,
    /// J(3) set, J(3) reset]: the least settle and clear (a press timed from the press tick).
    pub cause_settle: [Option<i64>; 7],
    pub cause_clear: [Option<i64>; 7],
    /// The run was `path_a` (its foot pair and y-12 bars).
    pub path_a: bool,
    /// ironspec 21:07 / 21:13 PT: across the run's restart (the helper's call, the official load: one
    /// block-only tick): its game-time delta (the official 1) and the moving carts' largest
    /// displacement (cart, distance; None: none moving; held = 0). Printed; barred once the helper
    /// runs the block-only tick (cc767b78's `liveness::restart`, pending).
    pub restart_dt: Option<i64>,
    pub restart_moved: Option<Option<(usize, f64)>>,
    /// ironspec 21:25 PT (4): the cells the carts' moves touched, and those not declared as stops
    /// (`stops()`, the `# stops:` header line).
    pub contact_cells: usize,
    pub contacts_undeclared: usize,
    /// The `# update-only:` declarations' evidence (path (a)): per J(-1) / J(3) / J(7), its branch
    /// detector's changes, the T off its power's settle after one, the T re-shaped by the update.
    pub dj: [(usize, usize, usize); 3],
    /// Rule C (tick-exact's water rows, farmopt 01:00): new water cells the settle made that the file
    /// doesn't declare (`declared_water()`).
    pub water_undeclared: usize,
    /// E rises within k of a fired pulse (ironspec 14:41 (2)).
    pub e_within_k: usize,
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
    /// Presence samples at fired pulses (E's rise) that found the napper rolling, or not on the hold
    /// (G1: both 0 under the settle lock).
    pub samples_rolling: usize,
    pub samples_away: usize,
    /// The pulse the napper left on differs from the one that consumed its waiting bit (None: it
    /// never left).
    pub pulse_mismatch: Option<bool>,
}

impl Summary {
    pub fn clean(&self) -> bool {
        self.wrong == 0 && self.pushes == 0 && !self.napper_off && self.riders_bad == 0 && self.hold_intrusions == 0 && self.extra_busy == 0 && !self.release_bad && !self.y12_bad && self.contacts_undeclared == 0 && self.dj.iter().all(|d| d.1 == 0) && (!self.path_a || self.dj.iter().all(|d| d.0 > 0)) && self.water_undeclared == 0
    }

    /// Every bar: clean; the column bar 14 + 2 = 16 on the shaft-entry (foot) gaps and the napper's
    /// gaps there (ironspec 19:38 / 19:07 PT: 14 is the exit-top contact boundary, 13 touches; the
    /// inherited 21 / 28 / 21 retired; under `path_a` only the foot pair lane -1 -> lane 3 enters at the
    /// foot and the napper's pairs are the y-12 ones in clean()); the heads under the roof's underside
    /// at 28 (geometry); each lane's delivery inside ironfarm's 0.30..0.48 (its part spec, inherited).
    pub fn bars(&self) -> bool {
        self.clean()
            && self.gap_min >= 16
            && (self.path_a || self.napper_behind.is_none_or(|g| g >= 16))
            && (self.path_a || self.napper_ahead.is_none_or(|g| g >= 16))
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
                "{l}: wrong {} pushes {} napper {} riders {} gap {} napper {:?}/{:?} head {:.2}{}",
                s.wrong,
                s.pushes,
                if s.napper_off { "OFF" } else { "ok" },
                s.riders_bad,
                s.gap_min,
                s.napper_behind,
                s.napper_ahead,
                s.head,
                if s.y12_why.is_empty() { String::new() } else { format!(" y-12: {}", s.y12_why) } + &if s.contacts_undeclared > 0 { format!(" undeclared block contacts {}", s.contacts_undeclared) } else { String::new() } + &if s.dj.iter().any(|d| d.1 > 0) { format!(" a T off its settle after its D's change {:?}", s.dj.map(|d| d.1)) } else { String::new() } + if s.path_a && s.dj.iter().any(|d| d.0 == 0) { " D-on-J NOT JUDGED (0 samples)" } else { "" } + &if s.water_undeclared > 0 { format!(" undeclared new water {}", s.water_undeclared) } else { String::new() }
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
        "{kj}; pulses per run {}; pulses after the napper's release that found a cart on the hold (bar 0) {}; the string's rising edge after the departure off the hold {}; the napper entering the T (9,0,7) at stored {}, reaching the hold {} gt after the latest arming; the napper from the stop (R) to its K {}; presence samples at fired pulses: rolling {}, not on the hold {} (bar 0 each); the approach detector's own rise -> rest {} (|v| = 0: {}; the rise read at the start of a tick, the rest at its end: + 1 counted from the tick the detector powers, as the official counts)",
        rng(ok.iter().map(|s| s.pulses_run as i64).collect()),
        ok.iter().map(|s| s.extra_busy).sum::<usize>(),
        rng(ok.iter().filter_map(|s| s.trip_after_dep).collect()),
        {
            let vt: Vec<f64> = ok.iter().filter_map(|s| s.v_t).collect();
            if vt.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", vt.iter().copied().fold(f64::INFINITY, f64::min), vt.iter().copied().fold(f64::NEG_INFINITY, f64::max)) }
        },
        rng(ok.iter().filter_map(|s| s.t_phase).collect()),
        rng(ok.iter().filter_map(|s| s.stop_to_k).collect()),
        ok.iter().map(|s| s.samples_rolling).sum::<usize>(),
        ok.iter().map(|s| s.samples_away).sum::<usize>(),
        rng(ok.iter().filter_map(|s| s.det_to_rest).collect()),
        rng(ok.iter().filter_map(|s| s.det_to_rest0).collect())
    );
    // (ironspec 14:41: c = ARM3 - REL over the rows, the napper's fired pulse -> y-12 pass, each
    // column entrant's y-12 pass against its REL (batch 0's lanes -1/3/7 in the reverse order, the
    // napper), the least distance in the column, E rises within k of a fired pulse)
    let cs: Vec<i64> = ok.iter().flat_map(|s| s.arm_c.iter().flatten().copied().collect::<Vec<_>>()).collect();
    let y12s = |i: usize| rng(ok.iter().filter_map(|s| s.y12_rel.get(i).copied().flatten()).collect());
    let nidx = ok.first().map_or(0, |s| s.nap_index.min(15));
    let cm: Vec<f64> = ok.iter().filter_map(|s| s.col_min).collect();
    // (ironspec 16:46: lane 7's entry after lane 3's y-12 pass, and the window from lane 7's entry to
    // the next batch's lane -1 (cart 3(b+1)), per batch, least over the rows)
    let y = |s: &Summary, i: usize| s.y12_rel.get(i).copied().flatten();
    let l7_after_l3: Vec<i64> = ok.iter().flat_map(|s| (0..s.nap_index / 3).filter_map(|b| Some(y(s, 3 * b + 2)? - y(s, 3 * b + 1)?)).collect::<Vec<_>>()).collect();
    let win: Vec<i64> = ok.iter().flat_map(|s| (0..(s.nap_index / 3).saturating_sub(1)).filter_map(|b| Some(y(s, 3 * (b + 1))? + 100 - y(s, 3 * b + 2)?)).collect::<Vec<_>>()).collect();
    let kj = format!(
        "{kj}; lane 7's y-12 pass after lane 3's, least {}; the window from lane 7's y-12 pass to the next batch's lane -1's, least {} (most {})",
        l7_after_l3.iter().min().map_or("-".into(), |v| v.to_string()),
        win.iter().min().map_or("-".into(), |v| v.to_string()),
        win.iter().max().map_or("-".into(), |v| v.to_string())
    );
    // (G1 on 83878113f: path (a)'s y-12 pairs, least per pair over every row, and the rows with the
    // column order wrong (barred in clean() under --path-a))
    let pl = |i: usize| {
        let v: Vec<i64> = ok.iter().filter_map(|s| s.y12_pairs[i]).collect();
        v.iter().min().map_or("NOT JUDGED (0 samples)".to_string(), |m| format!("{m} ({} rows)", v.len()))
    };
    let kj = format!(
        "{kj}; y-12 pairs, least over the rows: lane -1 -> lane 3 {} (the foot pair), lane 3 -> lane 7 {}, lane 7 -> the napper {}, the napper -> the next lane -1 {}, lane 7 -> the next lane -1 (no napper between) {}; rows with the column order wrong at y 12 {}",
        pl(0),
        pl(1),
        pl(2),
        pl(3),
        pl(4),
        ok.iter().filter(|s| s.y12_order_bad).count()
    );
    let jm = |i: usize| ok.iter().map(|s| s.j_vmax[i]).fold(0.0f64, f64::max);
    let kj = format!("{kj}; the most stored speed on J(-1) / J(3) / J(7) over every cart and row {:.3} / {:.3} / {:.3}", jm(0), jm(1), jm(2));
    // (ironspec 23:56 PT, no vacuous passes: each least prints its sample count, the rows that gave
    // one; none: NOT JUDGED (0 samples))
    let jl = |f: &dyn Fn(&Summary) -> Option<i64>| {
        let v: Vec<i64> = ok.iter().filter_map(|s| f(s)).collect();
        v.iter().min().map_or("NOT JUDGED (0 samples)".to_string(), |m| format!("{m} ({} rows)", v.len()))
    };
    // (ironspec 21:07 / 21:13 PT: across each restart (the helper's call, the official load: one
    // block-only tick): the game-time delta (officially 1) and whether the moving carts held
    // (officially yes); printed: this bench's helper runs no block-only tick yet (delta 0), so the bar
    // waits for cc767b78's `liveness::restart`)
    let dts: Vec<i64> = ok.iter().filter_map(|s| s.restart_dt).collect();
    let kj = if dts.is_empty() {
        kj
    } else {
        let mv: Vec<Option<(usize, f64)>> = ok.iter().filter_map(|s| s.restart_moved).collect();
        format!(
            "{kj}; across the restarts (the load; officially one block-only tick: delta 1, carts held; this bench's restart helper runs no block-only tick yet): the game-time delta {}..{}, moving carts held in {} rows, moved in {} (the most {:.4}), none moving in {}",
            dts.iter().min().copied().unwrap_or(0),
            dts.iter().max().copied().unwrap_or(0),
            mv.iter().filter(|m| m.is_some_and(|(_, d)| d <= 1e-9)).count(),
            mv.iter().filter(|m| m.is_some_and(|(_, d)| d > 1e-9)).count(),
            mv.iter().flatten().map(|(_, d)| *d).fold(0.0f64, f64::max),
            mv.iter().filter(|m| m.is_none()).count()
        )
    };
    // (ironspec 21:25 PT (4): the rows with an undeclared block contact; the D-on-J evidence over the
    // rows: the detector changes per J and the rows with a T off its settle after one)
    let kj = format!(
        "{kj}; block contacts: the most cells touched in a row {}, rows with an undeclared contact {}; D-on-J (the `# update-only:` declarations): detector changes per row J(-1) {}..{}, J(3) {}..{}, J(7) {}..{}, rows with a T off its power's settle after one {}, rows with a T re-shaped by the update {}",
        ok.iter().map(|s| s.contact_cells).max().unwrap_or(0),
        ok.iter().filter(|s| s.contacts_undeclared > 0).count(),
        ok.iter().map(|s| s.dj[0].0).min().unwrap_or(0),
        ok.iter().map(|s| s.dj[0].0).max().unwrap_or(0),
        ok.iter().map(|s| s.dj[1].0).min().unwrap_or(0),
        ok.iter().map(|s| s.dj[1].0).max().unwrap_or(0),
        ok.iter().map(|s| s.dj[2].0).min().unwrap_or(0),
        ok.iter().map(|s| s.dj[2].0).max().unwrap_or(0),
        ok.iter().filter(|s| s.dj.iter().any(|d| d.1 > 0)).count(),
        ok.iter().filter(|s| s.dj.iter().any(|d| d.2 > 0)).count()
    );
    let kj = format!(
        "{kj}; junction settle / clear, least over the rows (ironspec 17:20 (b): >= bar + 2 each): J(-1) settle {}, clear at day-1's timed re-arm {}, clear at its cart-triggered switches {} (the switching cart itself: >= 0 by construction); J(3) settle {}, clear (cart-triggered) {}; the napper's K under day-1's timed taps (ironspec 18:16 (3)): on clear {}, on settle {}, the napper out of K before off {}, off -> the next batch cart {}",
        jl(&|s| s.j_settle[0]),
        jl(&|s| s.j_clear[0]),
        jl(&|s| s.j_clear_cart[0]),
        jl(&|s| s.j_settle[1]),
        jl(&|s| s.j_clear_cart[1]),
        jl(&|s| s.k_clear),
        jl(&|s| s.k_settle),
        jl(&|s| s.k_off_margin),
        jl(&|s| s.k_off_settle)
    );
    let names = ["K(x) set", "K(x) reset", "J(-1) re-arm", "J(-1) set", "J(-1) reset", "J(3) set", "J(3) reset"];
    let kj = format!(
        "{kj}; by cause, least settle / clear over the rows (ironspec 19:14; a press timed from the press tick): {}",
        (0..7).map(|i| format!("{} {} / {}", names[i], jl(&|s: &Summary| s.cause_settle[i]), jl(&|s: &Summary| s.cause_clear[i]))).collect::<Vec<_>>().join(", ")
    );
    let kj = format!(
        "{kj}; ARM3 - REL (c) {}; the napper's fired pulse -> its y-12 pass {} (the rail's rise read at the start of a tick: + 1 counted from the game tick it rises); y-12 passes after REL: carts 0/1/2 {} / {} / {}, the napper {}; the napper's least y-12 gap behind {} and ahead {}; the least distance between two carts in the column {}; E rises within k after a reset {}",
        rng(cs),
        rng(ok.iter().filter_map(|s| s.pulse_to_y12).collect()),
        y12s(0),
        y12s(1),
        y12s(2),
        y12s(nidx),
        ok.iter().filter_map(|s| s.gap12_behind).min().map_or("-".into(), |v| v.to_string()),
        ok.iter().filter_map(|s| s.gap12_ahead).min().map_or("-".into(), |v| v.to_string()),
        if cm.is_empty() { "-".to_string() } else { format!("{:.3}", cm.iter().copied().fold(f64::INFINITY, f64::min)) },
        ok.iter().map(|s| s.e_within_k).sum::<usize>()
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
    ) + &{
        // (ironspec 18:16 (5): the rows holding each least junction / K margin, for a single re-run)
        let at = |f: &dyn Fn(&Summary) -> Option<i64>, what: &str| {
            v.iter()
                .filter_map(|(l, r)| r.as_ref().ok().and_then(|s| f(s)).map(|g| (g, l)))
                .min_by_key(|x| x.0)
                .map(|(g, l)| format!("{what} {g} at {l}"))
        };
        let rows: Vec<String> = [
            at(&|s| s.j_settle[0], "J(-1)'s least settle"),
            at(&|s| s.j_clear[0], "J(-1)'s least clear at the timed re-arm"),
            at(&|s| s.k_clear, "K's least on clear"),
            at(&|s| s.k_settle, "K's least on settle"),
            at(&|s| s.k_off_margin, "the napper's least margin out of K before off"),
            at(&|s| s.cause_settle[3], "J(-1) set's (lane 7's D press) least settle"),
            at(&|s| s.cause_clear[2], "J(-1) re-arm's least clear"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if rows.is_empty() { String::new() } else { format!("; the rows at the least margins: {}", rows.join("; ")) }
    }
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
    ) + &{
        // (G1 on 83878113f: the first rows failing a path (a) y-12 bar, for a single re-run)
        let y12f: Vec<&String> = v.iter().filter(|(_, r)| r.as_ref().is_ok_and(|s| s.y12_bad)).map(|(k, _)| k).collect();
        if y12f.is_empty() { String::new() } else { format!("; rows failing a y-12 bar {} (e.g. {})", y12f.len(), y12f.iter().take(4).map(|s| s.as_str()).collect::<Vec<_>>().join("; ")) }
    })
}

/// A party to a box or push-box contact between two moving entities (ironspec 21:58 PT).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Party {
    /// A minecart, with or without a passenger.
    Cart { ridden: bool },
    /// A mob seated in a minecart.
    SeatedRider,
    /// An entity on its own: neither riding nor a cart.
    Unmounted,
}

/// ironspec 21:58 / 22:07 PT's convention: whether a contact counts against a clean row, `overlap`
/// the entities' boxes themselves overlapping (else only a push box, the 0.2-inflated one, meets the
/// other). Two carts count, and a cart with an unmounted entity; a seated rider in a RIDERLESS cart's
/// push box counts (`Entity.push` moves a cart with no passenger); a seated rider only in a RIDDEN
/// cart's push box is printed, not barred (neither moves: `rideTick` zeroes the rider's motion, and a
/// cart with a passenger is not pushed), but its box OVERLAPPING that cart counts (a cart's move meets
/// the entities in its way, riders included); two seated riders are printed (both motions zeroed).
pub fn contact_counts(a: Party, b: Party, overlap: bool) -> bool {
    match (a, b) {
        (Party::SeatedRider, Party::SeatedRider) => false,
        (Party::Cart { ridden }, Party::SeatedRider) | (Party::SeatedRider, Party::Cart { ridden }) => !ridden || overlap,
        _ => true,
    }
}

#[cfg(test)]
mod contact_tests {
    use super::{Party, contact_counts};

    /// The classification, ridden and riderless, push box and overlap (ironspec 21:58 / 22:07 PT).
    #[test]
    fn the_push_box_convention() {
        let (ridden, riderless) = (Party::Cart { ridden: true }, Party::Cart { ridden: false });
        for overlap in [false, true] {
            // (two carts, ridden or not; a cart and an unmounted entity: count)
            assert!(contact_counts(ridden, ridden, overlap));
            assert!(contact_counts(ridden, riderless, overlap));
            assert!(contact_counts(riderless, riderless, overlap));
            assert!(contact_counts(riderless, Party::Unmounted, overlap));
            assert!(contact_counts(Party::Unmounted, ridden, overlap));
            assert!(contact_counts(Party::SeatedRider, Party::Unmounted, overlap));
            // (a seated rider in a riderless cart's push box: that cart is pushed: counts)
            assert!(contact_counts(Party::SeatedRider, riderless, overlap));
            assert!(contact_counts(riderless, Party::SeatedRider, overlap));
            // (two seated riders: printed, not barred)
            assert!(!contact_counts(Party::SeatedRider, Party::SeatedRider, overlap));
        }
        // (a seated rider only in a ridden cart's push box: printed, not barred; overlapping it: counts)
        assert!(!contact_counts(Party::SeatedRider, ridden, false));
        assert!(!contact_counts(ridden, Party::SeatedRider, false));
        assert!(contact_counts(Party::SeatedRider, ridden, true));
        assert!(contact_counts(ridden, Party::SeatedRider, true));
    }
}

/// ironspec 20:14 PT (b): tick-exact's official lift-pair configuration (`mctas mobai lift-pair` on
/// 83878113f: lane 7's side-slope entry) run in redsim: the path (a) lift; two carts with villager
/// riders on lane 7's route; the leader rolls onto the braked slope (-4,12,mz) from the climb's top and
/// settles; a redstone block at (-4,12,mz-1) (in place of day-1's repeater) releases it for 10 gt; the
/// follower is placed at the leader's rest, at rest, the tick that pulse ends, and released the same
/// way `gap` gt after the leader. Per gap: the least box gap (the largest per-axis separation of the
/// two 0.98 x 0.7 x 0.98 boxes, least over the scene; < 0 overlaps); the least vertical distance while
/// the boxes overlap sideways; the first box contact, the first push-box (0.2-inflated sideways)
/// contact and the first push that acts; the first tick each cart departs from its solo ride (the same
/// scene without the other cart). Ticks after the leader's release; "clean" = no contact of either
/// kind and both rides undisturbed (ironspec 20:14 PT).
pub fn lift_pair(o: &ReturnOpts, from: i64, to: i64) -> Result<String, String> {
    use crate::liftbench::CartState;
    // (per tick: the cart's state and its rider's feet position)
    type Track = Vec<Option<(CartState, Option<[f64; 3]>)>>;
    if o.mirror_z {
        return Err("--lift-pair runs unmirrored".into());
    }
    if from < 10 || to < from {
        return Err("--lift-pair FROM,TO: gaps FROM <= TO, at least 10 (the follower is placed when the leader's 10-gt pulse ends)".into());
    }
    let mz = if o.z7 { 7 } else { 8 };
    let base = ReturnOpts { path_a: true, batches: 0, napper_delay: None, napper_lift: false, trace: None, out: None, login: None, restart: None, ..o.clone() };
    // (the leader's release, absolute; the scene's length)
    const REL: i64 = 40;
    let len = REL + to + 140;
    // (the cell's resting block between the pulses: the lane-7 drive stand-in, the ceiling lever)
    let rep = Block::parse("minecraft:lever[face=ceiling,facing=north,powered=false]")?;
    let rb = Block::parse("minecraft:redstone_block")?;
    let slope_in = BlockPos::new(-4, 12, mz - 1);
    // (tick-exact's official riders are NoAI villagers; redsim refuses NoAI on a villager, so these run
    // with AI: a seated rider's box sits at its cart's position either way, which the scene checks
    // every tick (rider - cart printed; tick-exact's row: 0.0000 every tick); ironspec 22:07 PT (2c))
    let rider = "Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]";
    // (one scene: the leader if `lead`; the follower placed at `rest` at REL + 10 and released at
    // REL + gap if given; each cart's state at each tick's end)
    let scene = |lead: bool, fol: Option<(i64, [f64; 3])>| -> Result<(Track, Track), String> {
        let spec = build(&base)?;
        let ro = RunOpts { ai: true, ..RunOpts::steady(1, (len + 50) as u32, WalkMode::Random) };
        let (mut run, _, _) = start_run(&spec, &ro)?;
        while run.settle_tick() {}
        let (mut lt, mut ft): (Track, Track) = (vec![None; len as usize], vec![None; len as usize]);
        let (mut lid, mut fid): (Option<u32>, Option<u32>) = (None, None);
        for t in 0..len {
            if t == 0 && lead {
                lid = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [-2.5, 13.0625, mz as f64 + 0.5], Some(&format!("{{Motion:[-0.1d,0.0d,0.0d],{rider}}}")))?);
            }
            if lead && t == REL {
                run.world.set_block(slope_in, rb.clone(), redsim::world::flags::ALL);
            }
            if lead && t == REL + 10 {
                run.world.set_block(slope_in, rep.clone(), redsim::world::flags::ALL);
            }
            if let Some((gap, rest)) = fol {
                if t == REL + 10 {
                    fid = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", rest, Some(&format!("{{Motion:[0.0d,0.0d,0.0d],{rider}}}")))?);
                }
                if t == REL + gap {
                    run.world.set_block(slope_in, rb.clone(), redsim::world::flags::ALL);
                }
                if t == REL + gap + 10 {
                    run.world.set_block(slope_in, rep.clone(), redsim::world::flags::ALL);
                }
            }
            run.tick();
            let grab = |id: Option<u32>, w: &redsim::world::World| {
                id.and_then(|i| redsim::entity::index_of(w, i)).map(|i| {
                    let e = &w.entities.list[i];
                    let rider = e.passengers.first().and_then(|r| redsim::entity::index_of(w, *r)).map(|j| {
                        let p = &w.entities.list[j].pos;
                        [p.x, p.y, p.z]
                    });
                    (CartState::of(e), rider)
                })
            };
            lt[t as usize] = grab(lid, &run.world);
            ft[t as usize] = grab(fid, &run.world);
        }
        Ok((lt, ft))
    };
    let (lsolo, _) = scene(true, None)?;
    let r = lsolo[(REL - 1) as usize].ok_or("the leader never reached the slope")?.0;
    let rest = [r.x, r.y, r.z];
    let mut lines = vec![format!(
        "lift-pair (ironspec 20:14 PT (b): tick-exact's configuration in redsim on the record lift file's path (a) geometry; riders seated): the leader settled on the slope at {:.4},{:.4},{:.4} (|v| {:.4}), released by a redstone block at (-4,12,{}) at t 0 for 10 gt; the follower placed there at rest at t 10 and released the same way at t gap; trace convention (ironspec 21:07 PT): t = game ticks after the leader's release, the block written before tick 0 (the leader's first kick in tick 0), each state as it stands at the end of tick t (absolute t + {REL}); a second shared event per cart, its column entry (its first end-of-tick in the column cell, x < -4); \"clean\" = no box or push-box contact that counts (ironspec 21:58 PT: two carts, a cart and an unmounted entity, a seated rider in a riderless cart's push box or overlapping a ridden cart; a seated rider only in a ridden cart's push box is printed only) and both rides as solo:",
        r.x,
        r.y,
        r.z,
        (r.vx * r.vx + r.vy * r.vy + r.vz * r.vz).sqrt(),
        mz - 1
    )];
    // (a box's separation from another (> 0 apart, < 0 overlapping): a cart 0.98 wide x 0.7 tall, a
    // villager rider 0.6 x 1.95, feet at y; `inflate` widens the pair sideways (a cart's push query,
    // its box inflated 0.2 sideways))
    fn sep(a: [f64; 3], wa: f64, ha: f64, b: [f64; 3], wb: f64, hb: f64, inflate: f64) -> f64 {
        let half = (wa + wb) / 2.0 + inflate;
        let dx = (a[0] - b[0]).abs() - half;
        let dz = (a[2] - b[2]).abs() - half;
        let dy = (a[1] - (b[1] + hb)).max(b[1] - (a[1] + ha));
        dx.max(dz).max(dy)
    }
    let kind_names = ["cart-cart", "the leader's rider vs the follower's cart", "the follower's rider vs the leader's cart", "rider-rider"];
    // (ironspec 21:58 / 22:07 PT's convention per kind, for a box overlap and for a push box only:
    // both carts ridden, the riders seated)
    let ridden = Party::Cart { ridden: true };
    let parties = [(ridden, ridden), (Party::SeatedRider, ridden), (Party::SeatedRider, ridden), (Party::SeatedRider, Party::SeatedRider)];
    let counts_box: [bool; 4] = parties.map(|(a, b)| contact_counts(a, b, true));
    let counts_push: [bool; 4] = parties.map(|(a, b)| contact_counts(a, b, false));
    let mut clean: Vec<(i64, bool)> = Vec::new();
    let mut kept: Vec<(i64, Track, Track, Option<i64>)> = Vec::new();
    for gap in from..=to {
        let (pl, pf) = scene(true, Some((gap, rest)))?;
        let (_, fsolo) = scene(false, Some((gap, rest)))?;
        // (the first tick a cart or its rider departs from its solo ride, and by how much)
        let departs = |a: &Track, b: &Track| -> Option<(i64, f64)> {
            (0..len as usize).find_map(|t| match (a[t], b[t]) {
                (Some((x, xr)), Some((y, yr))) => {
                    let mut d = (x.x - y.x).abs().max((x.y - y.y).abs()).max((x.z - y.z).abs());
                    if let (Some(p), Some(q)) = (xr, yr) {
                        d = d.max((p[0] - q[0]).abs()).max((p[1] - q[1]).abs()).max((p[2] - q[2]).abs());
                    }
                    (d > 1e-9).then_some((t as i64 - REL, d))
                }
                (None, None) => None,
                _ => Some((t as i64 - REL, f64::INFINITY)),
            })
        };
        let (dl, df) = (departs(&pl, &lsolo), departs(&pf, &fsolo));
        let mut least: [Option<(f64, i64)>; 4] = [None; 4];
        // (the first box / push-box contact that counts, and the first printed only)
        let mut first_box: Option<(i64, usize)> = None;
        let mut first_pbox: Option<(i64, usize)> = None;
        let mut printed_box: Option<(i64, usize)> = None;
        let mut printed_pbox: Option<(i64, usize)> = None;
        let mut first_push: Option<i64> = None;
        let mut samples = 0usize;
        for t in 0..len as usize {
            if let (Some((a, ar)), Some((b, br))) = (pl[t], pf[t]) {
                samples += 1;
                let tt = t as i64 - REL;
                let (ac, bc) = ([a.x, a.y, a.z], [b.x, b.y, b.z]);
                let mut kinds: Vec<(usize, f64, f64)> = vec![(0, sep(ac, 0.98, 0.7, bc, 0.98, 0.7, 0.0), sep(ac, 0.98, 0.7, bc, 0.98, 0.7, 0.2))];
                if let Some(r) = ar {
                    kinds.push((1, sep(r, 0.6, 1.95, bc, 0.98, 0.7, 0.0), sep(r, 0.6, 1.95, bc, 0.98, 0.7, 0.2)));
                }
                if let Some(r) = br {
                    kinds.push((2, sep(r, 0.6, 1.95, ac, 0.98, 0.7, 0.0), sep(r, 0.6, 1.95, ac, 0.98, 0.7, 0.2)));
                }
                if let (Some(p), Some(q)) = (ar, br) {
                    kinds.push((3, sep(p, 0.6, 1.95, q, 0.6, 1.95, 0.0), f64::INFINITY));
                }
                for (i, s, sp) in kinds {
                    if least[i].is_none_or(|g| s < g.0) {
                        least[i] = Some((s, tt));
                    }
                    if s < 0.0 {
                        let fb = if counts_box[i] { &mut first_box } else { &mut printed_box };
                        if fb.is_none() {
                            *fb = Some((tt, i));
                        }
                    }
                    if sp < 0.0 {
                        let fp = if counts_push[i] { &mut first_pbox } else { &mut printed_pbox };
                        if fp.is_none() {
                            *fp = Some((tt, i));
                        }
                    }
                }
                if let Some((true, _)) = crate::liftbench::touch(&a, &b) {
                    first_push.get_or_insert(tt);
                }
            }
        }
        // (ironspec 23:56 PT: a gap with no tick holding both carts is not judged)
        let ok = samples > 0 && first_box.is_none() && first_pbox.is_none() && dl.is_none() && df.is_none();
        clean.push((gap, ok));
        let centre = [dl.map(|x| x.0), df.map(|x| x.0), first_pbox.map(|x| x.0), first_box.map(|x| x.0), printed_pbox.map(|x| x.0)].into_iter().flatten().min().or(least[0].map(|x| x.1));
        let ct = |v: Option<(i64, usize)>| v.map_or("none".to_string(), |(t, i)| format!("t {t} ({})", kind_names[i]));
        let dep = |v: Option<(i64, f64)>| v.map_or("never".to_string(), |(t, d)| format!("at t {t} (by {d:.4})"));
        // (each cart's column entry: its first end-of-tick with x < -4 in the column cell's z)
        let entry = |tr: &Track| (0..len as usize).find(|&t| tr[t].is_some_and(|(s, _)| s.x < -4.0 && s.z.floor() as i32 == mz)).map_or("-".to_string(), |t| format!("t {}", t as i64 - REL));
        lines.push(format!(
            "  gap {gap}: {} (samples: {samples} ticks holding both carts); column entries: the leader {}, the follower {}; least separations: {}; first box contact that counts {}; first push-box contact that counts {}; printed only (a seated rider only in a ridden cart's push box, or two seated riders: ironspec 21:58 / 22:07 PT) box {}, push box {}; first cart push acting {}; the leader departs from its solo ride {}, the follower {}",
            if samples == 0 { "NOT JUDGED (0 samples)" } else if ok { "CLEAN" } else { "DISTURBED" },
            entry(&pl),
            entry(&pf),
            (0..4).filter_map(|i| least[i].map(|(s, t)| format!("{} {s:.3} at t {t}", kind_names[i]))).collect::<Vec<_>>().join(", "),
            ct(first_box),
            ct(first_pbox),
            ct(printed_box),
            ct(printed_pbox),
            first_push.map_or("none".to_string(), |t| format!("t {t}")),
            dep(dl),
            dep(df)
        ));
        kept.push((gap, pl, pf, centre));
    }
    let least_clean = clean.iter().rev().take_while(|x| x.1).last().map(|x| x.0);
    // (ironspec 22:07 PT (2c): the riders run with AI (redsim refuses NoAI on a villager); a seated
    // rider's feet against its cart's position, every tick of every kept scene)
    let seat = kept
        .iter()
        .flat_map(|(_, pl, pf, _)| pl.iter().chain(pf.iter()))
        .filter_map(|s| s.and_then(|(c, r)| r.map(|r| (r[0] - c.x).abs().max((r[1] - c.y).abs()).max((r[2] - c.z).abs()))))
        .fold(0.0f64, f64::max);
    lines.push(format!(
        "the riders: villagers with AI (redsim refuses NoAI on a villager; tick-exact's official riders are NoAI): a seated rider's feet minus its cart's position, the most over every tick of every gap, {seat:.4} (tick-exact's official row: 0.0000 every tick), so the boxes are the same either way"
    ));
    lines.push(format!(
        "least clean gap (it and every larger gap in {from}..={to} clean): {}",
        least_clean.map_or("none in the range".to_string(), |g| g.to_string())
    ));
    // (ironspec 20:28 PT: the per-tick boxes at the boundary, the last disturbed gap and the first
    // clean one, around the first divergence / contact / least separation)
    let worst = clean.iter().filter(|x| !x.1).map(|x| x.0).max();
    for (gap, pl, pf, centre) in kept.iter().filter(|k| worst.is_some_and(|w| k.0 == w || k.0 == w + 1)) {
        let Some(c) = *centre else { continue };
        lines.push(format!("  per tick at gap {gap} (t: the leader's cart x,y,z / rider feet y; the follower's cart x,y,z / rider feet y; cart-cart separation, push-box separation):"));
        for t in (c - 4)..=(c + 6) {
            let i = (t + REL) as usize;
            if let (Some(Some((a, ar))), Some(Some((b, br)))) = (pl.get(i), pf.get(i)) {
                let (ac, bc) = ([a.x, a.y, a.z], [b.x, b.y, b.z]);
                lines.push(format!(
                    "    t {t}: {:.4},{:.4},{:.4} / {}; {:.4},{:.4},{:.4} / {}; {:.4}, {:.4}",
                    a.x,
                    a.y,
                    a.z,
                    ar.map_or("-".to_string(), |r| format!("{:.4}", r[1])),
                    b.x,
                    b.y,
                    b.z,
                    br.map_or("-".to_string(), |r| format!("{:.4}", r[1])),
                    sep(ac, 0.98, 0.7, bc, 0.98, 0.7, 0.0),
                    sep(ac, 0.98, 0.7, bc, 0.98, 0.7, 0.2)
                ));
            }
        }
    }
    Ok(lines.join("\n"))
}

/// ironspec 14:36's cross-check: per napper start speed at R, a restart (each liveness order) at the
/// tick the napper first reaches the lift's top cell (6,-3,4), and at that tick +- 1, 2; the approach
/// detector's own rise -> the napper at rest on the hold (|v| < 0.005, and |v| = 0), against
/// tick-exact's official ride (32; 36 with a restart at the lift's top exit).
pub fn top_restart(o: &ReturnOpts, speeds: &[f64]) -> Result<String, String> {
    let rows = crate::liveness::Restart::rows();
    let mut out = vec![format!(
        "top-exit restart cross-check (the napper from R at each start speed; a restart, every liveness order ({}), at the tick it first reaches the lift's top cell (6,-3,4), and at that tick -2..+2; the approach detector (7,-3,4)'s own rise -> rest on the hold (|v| < 0.005 / |v| = 0); the rise is read at the start of a tick (it powered in the tick before), the rest at a tick's end, so the official count (from the tick the detector powers) is these + 1):",
        rows.iter().map(|r| r.label()).collect::<Vec<_>>().join(", ")
    )];
    let fmt = |v: &[i64]| if v.is_empty() { "-".to_string() } else { format!("{}..{}", v.iter().min().unwrap_or(&0), v.iter().max().unwrap_or(&0)) };
    for &sp in speeds {
        let base = ReturnOpts { napper_speed: sp, restart: None, trace: None, out: None, ..o.clone() };
        let (_, s0) = return_run(&base)?;
        let Some(tt) = s0.t_top else {
            out.push(format!("  start {sp:.2}: the napper never reached the lift's top cell"));
            continue;
        };
        let jobs: Vec<(String, ReturnOpts)> = (-2..=2i64)
            .flat_map(|d| rows.iter().map(move |r| (d, *r)))
            .map(|(d, r)| (format!("{d:+} {}", r.label()), ReturnOpts { restart: Some((tt + d, r)), ..base.clone() }))
            .collect();
        let v = run_rows(jobs)?;
        let pick = |d: i64, f: &dyn Fn(&Summary) -> Option<i64>| -> Vec<i64> { v.iter().filter(|(k, _)| k.starts_with(&format!("{d:+} "))).filter_map(|(_, r)| r.as_ref().ok().and_then(f)).collect() };
        let at0: Vec<String> = v.iter().filter(|(k, _)| k.starts_with("+0 ")).map(|(k, r)| format!("{} {}", k.trim_start_matches("+0 "), r.as_ref().ok().and_then(|s| s.det_to_rest).map_or("-".into(), |x: i64| x.to_string()))).collect();
        let bad = v.iter().filter(|(_, r)| !r.as_ref().is_ok_and(Summary::clean)).count();
        out.push(format!(
            "  start {sp:.2}: no restart {} (|v| = 0: {}), the top cell at t {tt}; restart at t {tt}: {} (each order: {}); at t {tt} -2..+2: {} (|v| = 0: {}); rows failing a bar {bad} of {}",
            s0.det_to_rest.map_or("-".into(), |x| x.to_string()),
            s0.det_to_rest0.map_or("-".into(), |x| x.to_string()),
            fmt(&pick(0, &|s: &Summary| s.det_to_rest)),
            at0.join(", "),
            fmt(&(-2..=2).flat_map(|d| pick(d, &|s: &Summary| s.det_to_rest)).collect::<Vec<_>>()),
            fmt(&(-2..=2).flat_map(|d| pick(d, &|s: &Summary| s.det_to_rest0)).collect::<Vec<_>>()),
            v.len()
        ));
    }
    Ok(out.join("\n"))
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
/// ironspec 21:25 PT (4) / 22:05 PT: the cells a cart's move touches by design (containment, not
/// stops), each group with its reason and its evidence row (the file's `# contacts:` line,
/// `CELLS=REASON (evidence row)` in this file's frame): the lift shaft's walls and caps at its two
/// entries and its top, and the hold's slime stop. A group with no official row (`None`) is bench-only:
/// its contacts print their speed change per row. The bench fails a row whose carts touch any other
/// cell.
fn contact_groups(o: &ReturnOpts, mz: i32) -> Vec<(Vec<(i32, i32, i32)>, &'static str, Option<&'static str>)> {
    let zf = |z: i32| if o.mirror_z { 3 - z } else { z };
    let foot = if o.path_a { 2 } else { 0 };
    let top = if o.row9 && (o.k_relay || o.climb2) { 22 } else if o.row10 || o.row9 { 24 } else if o.flat || o.dip { 21 } else { 22 };
    let c = |v: &[(i32, i32, i32)]| v.iter().map(|&(x, y, z)| (x, y, zf(z))).collect::<Vec<_>>();
    const LIFT_PAIR: &str = "tick-exact's official lift-pair rows (`mctas mobai lift-pair --module path-a`, 3a5d5cdc6): redsim's cart and rider boxes equal tick for tick";
    const SIDE: &str = "tick-exact's official side-slope row: rest feet 12.7767, column entry at release + 6, as redsim; the lift-pair rows enter here";
    let mut g = vec![
        (c(&[(-6, foot, mz), (-6, foot + 1, mz)]), "the foot entry's back wall: a cart off the merge row slides west into it and stops over the column", None),
        (c(&[(-5, foot, mz - 1), (-5, foot + 1, mz - 1), (-5, foot, mz + 1), (-5, foot + 1, mz + 1)]), "the shaft's side walls at the foot: the 0.98-wide box in the 1-wide shaft meets them as it stops and starts to rise", None),
        (c(&[(-4, foot + 1, mz)]), "the foot entry's lintel: the box's east part, still under it when the column starts lifting, is held until the cart is in", None),
        (c(&[(-5, top + 2, mz), (-6, top + 1, mz)]), "the lift's cap and its west wall at the top: the riser stops under the cap (y 23.3) and drifts south onto the exit", Some(LIFT_PAIR)),
    ];
    if o.path_a {
        g.push((c(&[(-6, 12, mz), (-6, 13, mz)]), "lane 7's side entry's back wall: its cart rolls off the slope (-4,12,7) west into it and stops over the column", Some(SIDE)));
        g.push((c(&[(-5, 12, mz - 1), (-5, 13, mz - 1), (-5, 12, mz + 1), (-5, 13, mz + 1), (-4, 13, mz - 1)]), "the shaft's side walls at the side entry and the sign's glass over the slope's north edge", Some(SIDE)));
    }
    if o.napper_lift {
        g.push((c(&[(9, 0, 9)]), "the hold's slime stop: the napper rolls onto the hold rail (9,0,8) against it and rests", Some("tick-exact's official rail-kick row (`mctas mobai rail-kick`)")));
    }
    if o.napper_lift && o.napper_delay.is_some() {
        // (the napper's own lift from the bed stop's R (4,-19-N,4): its foot entry's east wall and its cap)
        const RIDE: &str = "tick-exact's official napper-ride rows (`mctas mobai napper-ride`, every tick from the summon: R_max 36, the pulse -> y-12 63 gt = the bench's 62 + 1)";
        let rb_y = -19 - o.lift_depth;
        g.push((c(&[(7, rb_y, 4), (7, rb_y + 1, 4)]), "the napper lift's foot entry back wall: the napper off the bed stop's R slides east into it and stops over its column", Some(RIDE)));
        g.push((c(&[(6, -2, 4)]), "the napper lift's cap: the napper rises under it and drifts east onto the exit detector (7,-3,4)", Some(RIDE)));
    }
    g
}

/// [`contact_groups`] cell by cell: (cell, reason, evidence row).
fn declared_contacts(o: &ReturnOpts, mz: i32) -> Vec<((i32, i32, i32), &'static str, Option<&'static str>)> {
    contact_groups(o, mz).into_iter().flat_map(|(cs, why, ev)| cs.into_iter().map(move |p| (p, why, ev))).collect()
}

/// tick-exact's water rows via farmopt 01:00, ironspec 01:07 PT (2): the designed currents, saved
/// settled in the file (each source spreads one cell into its lift's exit cell, level 1, and stops;
/// `build` lays the flowing cell itself), with their purpose and evidence (the file's `# water:` line,
/// read by settle-check, the water-settle row and the water-rule static as "designed current").
fn declared_water(o: &ReturnOpts, mz: i32) -> Vec<((i32, i32, i32), &'static str)> {
    let zf = |z: i32| if o.mirror_z { 3 - z } else { z };
    let top = if o.row9 && (o.k_relay || o.climb2) { 22 } else if o.row10 || o.row9 { 24 } else if o.flat || o.dip { 21 } else { 22 };
    let mut v = vec![((-5, top + 1, zf(mz)), "designed current, saved settled (level 1): the shared lift's top-drift current, fed by the source north of the exit cell, carrying each riser south onto the exit booster; bounded by the glass round it, the cap over it and the booster's rail (tick-exact's water-settle row on record7e: one cell, then still; the bench's traces: the riser's z speed 0.027 -> 0.104 in the cell, on the booster 8 gt after the top)")];
    if o.napper_lift && o.napper_delay.is_some() {
        v.push(((6, -3, zf(4)), "designed current, saved settled (level 1): the napper climb's top-drift current, fed by the source (5,-3,4), carrying the napper east onto the exit detector (7,-3,4); bounded by the glass round it and the detector's rail (tick-exact's water-settle row on record7e)"));
    }
    v
}

fn write_design(o: &ReturnOpts, spec: &FarmSpec, w: &redsim::world::World, path: &str, mz: i32) -> Result<(), String> {
    use redsim::pos::Dir;
    let mut d = mctas::design::Design { name: "f1q2-lift-split".into(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    for (p, blk) in &spec.blocks {
        let station = (p.y == 0 || p.y == -1) && p.z < mz && (LANES.contains(&p.x) || (o.boosters && !o.no_torches && p.z == mz - 2 && LANES.contains(&(p.x + 1))));
        let r_stand = p.x == 4 && p.z == 4 && (p.y == -19 - o.lift_depth || p.y == -20 - o.lift_depth);
        let branch = LANES.contains(&p.x) && (23..=24).contains(&p.y) && ((o.row10 && (7..=9).contains(&p.z)) || (o.row9 && p.z == 7));
        let stop = !o.napper_lift && p.x == 9 && p.y == 0 && p.z == mz;
        // (`path_a`: day-1's intake repeater (-4,12,mz-1) and its glass, dfb89499's climb-top
        // stand-in (-3,13,mz): not this file's; the glass under that stand-in (-3,12,mz) is, as the
        // slope's high-side support (would_pop: an ascending rail needs a rigid block on its high side))
        let foreign = (o.path_a && ((p.x == -4 && (p.z == mz - 1 || p.z == mz - 2) && (p.y == 12 || p.y == 11)) || (p.x == -3 && p.z == mz && p.y == 13))) || (o.napper_lift && p.x == 7 && (p.y == -1 || p.y == -2) && p.z == 8);
        if station || branch || stop || r_stand || foreign {
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
            // (ironspec 14:01: a signal source's placement and changes update cells around it (a
            // torch every cell within Manhattan 2, a detector rail its faces and its block-below's
            // faces: each type's pattern from redsim's block model, push-scan's `update_reach`); a T
            // whose stored shape is not its unpowered settle may have none reaching it unless it is
            // its own declared switch (power-forced): every source of the bench's world, the
            // station's emulation included)
            let m2: Vec<String> = spec
                .blocks
                .iter()
                .filter(|(q, blk)| crate::pushscan::update_reach(*q, blk).is_some_and(|(_, cells)| cells.contains(&p)))
                .map(|(q, blk)| format!("{} {}", blk.to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or(""), cell(*q)))
                .collect();
            if !declared && stored != j.unpowered && !m2.is_empty() {
                fail.push(format!("T {} (stored {}, unpowered {}) has signal-source updates reaching it: {}", cell(p), stored.name(), j.unpowered.name(), m2.join(", ")));
            }
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
                "T {}: arms {}, designed {}, unpowered {}, powered {}; sources at its faces: {}; signal-source updates reaching it: {}; {verdict}",
                cell(p),
                j.arms.iter().map(|a| a.name()).collect::<Vec<_>>().join("/"),
                stored.name(),
                j.unpowered.name(),
                j.powered.name(),
                if sources.is_empty() { "none".to_string() } else { sources.join(", ") },
                if m2.is_empty() { "none".to_string() } else { m2.join(", ") }
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
        o.pulse_gate.filter(|_| o.napper_lift).map_or(String::new(), |x| format!(" --pulse-gate {x}{}", if o.gate_delayed_only { " --gate-delayed-only" } else { "" })) + if o.tripwire { " --tripwire" } else { "" } + &if o.napper_lift && o.lift_depth != 9 { format!(" --lift-depth {}", o.lift_depth) } else { String::new() } + &if o.path_a { format!(" --path-a --lane7-rel {}", o.lane7_rel) } else { String::new() } + &if o.station_lag != 0 { format!(" --station-lag {}", o.station_lag) } else { String::new() } + &if o.path_a && o.lane7_pulse != 10 { format!(" --lane7-pulse {}", o.lane7_pulse) } else { String::new() } + &if o.kset_shift != 0 { format!(" --kset-shift {}", o.kset_shift) } else { String::new() } + if o.driver_latency { " --driver-latency" } else { "" } + &o.arm_rel.map_or(String::new(), |c| format!(" --arm-rel {c}")) + if o.no_torches { " --no-torches" } else { "" } + if o.light_fix { " --light-fix" } else { "" } + &o.set_cells.iter().map(|s| format!(" --set '{s}'")).collect::<String>()
    );
    let lift = {
        let y8top = if o.boost_exit { top - 1 } else { top };
        let l7r = o.lane7_rel;
        format!(
            "# - shared lift: {foot_txt}{}; the top cell (-5,{ex},7) with glass over it (-5,{},7) and beside it (-6,{ex},7), (-4,{ex},7); the current's source (-5,{ex},6) boxed by glass (-5,{ex},5), (-6,{ex},6), (-4,{ex},6), (-5,{},6).",
            if o.boost_exit { format!(", the redstone block (-5,{top},8) under the exit's booster as the south wall's top cell") } else { String::new() },
            ex + 1,
            ex + 1,
            foot_txt = if o.path_a {
                format!("soul sand (-5,1,7) on glass (-5,0,7), the bubble column (-5,2..{top},7), glass walls (-6,y,7), (-5,y,6) for y 2..{top}, (-5,y,8) for y 2..{y8top} and (-4,y,7) for y 3..{top} but y 12 (the entry (-4,2,7) on glass (-4,1,7), the merge row's); the intake (2c): the braked powered slope (-4,12,7) ascending_east (off) on the wall's glass (-4,11,7), lane 7's rest, released at REL + {l7r} by day-1's drive at (-4,12,6) (not here; dfb89499's climb tops at (-3,13,7)), the cell over it an oak wall sign (-4,13,7) facing south on glass (-4,13,6) (no collision box; it keeps the column's water in)")
            } else {
                format!("soul sand (-5,-1,7) on glass (-5,-2,7), the bubble column (-5,0..{top},7), glass walls (-6,y,7), (-5,y,6) for y 0..{top}, (-5,y,8) for y 0..{y8top} and (-4,y,7) for y 1..{top} (panes to come; (-4,0,7) is the wall entry)")
            }
        )
    };
    let exit = if o.row9 && o.k_relay {
        format!(
            "# - exit and split (in the chunk; K re-lay): the booster, a powered slope (-5,{ex},8) ascending_south (on) on the shaft wall's glass (-5,{top},8), powered from its west side by {exit_src} (3 from lane -1's seat){cap_part}; the curve (-5,{},9) north_east on glass; the slope (-4,{},9) ascending_east on glass into K(-3) (lay each slope after the rail at its high end); every rail within 2 of lane -1's seat (-3,24,7) on glass; the row along z 9 at y 25, x -3..6 on glass at y 24: K(-3) (-3,25,9), J(-1) (-1,25,9), K(1) (1,25,9), J(3) (3,25,9), K(5) (5,25,9), J(7) (7,25,9){}; each K's feed F (x-2,25,8) north_south on glass (x-2,24,8) (ironfarm's S-bend beyond, not here); each lane's branch the detector slope D (x,24,8) ascending_south on glass (x,23,8), into ironfarm's merge cell (x,24,7) (not here); a lever over every J and K (x,26,9) with glass (x,27,9); (x-1,24,9) glass only; nothing of this file in (x-2..x-1, 22..23, 9) or at (x,23,9) (ironfarm's R feed path and the switched cell).",
            ex + 1,
            ex + 1,
            if o.napper_delay.is_some() { ", the 4th route (8,25,9), (9,25,9)" } else { "" },
            exit_src = if o.no_torches { format!("the redstone block (-6,{ex},8)") } else { format!("the redstone torch (-6,{ex},8) on glass (-6,{top},8) (V15)") },
            cap_part = if o.no_torches { format!(", with the glass cap (-6,{},8) over it (no spawn spot on the block)", ex + 1) } else { String::new() }
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
            format!("# - napper (c), re-laid for the z 7 hold: from R (4,{r},4) (58f770cc's, not here) the entry (5,{r},4) east_west on glass; soul sand (6,{ss},4), the bubble column (6,{r}..-4,4), glass walls (7,y,4), (6,y,3), (6,y,5) for y {ss}..-4 and (5,y,4) for y {w5}..-4; the top cell (6,-3,4) (glass beside it (6,-3,3), (6,-3,5), over it (6,-2,4)), the source (5,-3,4) boxed by glass (4,-3,4), (5,-3,3), (5,-3,5), (5,-2,4); the exit (7,-3,4) {exit74}; a powered slope (8,-3,4) ascending_east on the redstone block (8,-4,4); the curve (9,-2,4) south_west on glass; powered slopes (9,-2,5) and (9,-1,6) ascending_south {climb}; the T (9,0,7) south_west on glass (arms N, S, W: from the north on south, from the south west); the hold (9,0,8), a powered rail north_south, off, on {hold_sup} (9,-1,8) (a conductor), driven by the repeater (8,-1,8) facing west on glass (8,-2,8) (its input (7,-1,8), day-1's chain: the hold rail powered from ARM3 + D for the release pulse (--release-pulse, 8 gt by default) in each cycle whose pulse fires (with --pulse-gate X, day-1's gate: only when the napper's waiting bit, set on the approach detector (7,-3,4) (a detector rail east_west on glass (7,-4,4), read from its side) and sampled X gt late at E's rise (ARM3 + 2), is high, so a napper not yet at rest waits for the next cycle's pulse and every departure is a scheduled one; without it, every cycle); EMULATED, a redstone block at (7,-1,8) for the pulse), the napper stopped against the slime block (9,0,9) (a conductor, so the launch's kick pushes off it; not solid-render) with glass over it (9,1,9); the hold's line (8,0,7) a powered rail (on) {line_pw} into lane 7's curve (7,0,7); declared (push-scan's two pairs at the hold): the staircase, the T (9,0,7), the hold (9,0,8) and the hold's line (8,0,7) are the napper's own route, no batch cart enters it (lane 7's merge T (7,0,7) joins north and west only, its east arm is the hold's line), one napper at a time (the next leaves the bed stop >= 312 gt later, day-1's model figure; the single-occupant line below), so no two carts meet there;",
                r = -19 - o.lift_depth,
                ss = -20 - o.lift_depth,
                w5 = -18 - o.lift_depth,
                climb = if o.no_torches { "on the redstone blocks (9,-3,5), (9,-2,6)" } else { "on glass (9,-3,5), (9,-2,6), powered by the redstone torch (8,-2,5) on glass (8,-3,5) beside (9,-2,5) (the chain carries it to (9,-1,6))" },
                hold_sup = if o.no_torches { "stone" } else { "slime" },
                exit74 = if o.pulse_gate.is_some() { "a detector rail east_west on the lift wall's glass (7,-4,4) (day-1's approach detector: the waiting bit's source, read from its side; no T within its update reach)" } else { "east_west" },
                line_pw = if o.no_torches { "on the redstone block (8,-1,7)" } else { "on slime (8,-1,7), strongly powered by the redstone torch (8,-2,7) on glass (8,-3,7) under it" }
            ),
            if let Some(c) = o.arm_rel { format!("ARM3 := REL + {c} (day-1 14:41: no detector; (5,0,7) a plain rail)") } else if o.arm_station { "the station's own exit detector on lane 7 (7,0,2) (dfb89499's)".to_string() } else { "(5,0,7) east_west on the main line (past the boosters, two cells off lane 7's curve and lane 3's T)".to_string() }
        )
    } else if o.napper_delay.is_some() {
        "# - napper (c): the hold's line (8,0,7) only (the bench spawns the napper there, EMULATED); the napper lift not in this file.".to_string()
    } else {
        "# - napper: none in this run.".to_string()
    };
    let mut head: Vec<String> = vec![
        "# The return for F1 + Q2 at 24 (7b90c3f1; bench, tool unreviewed): loop frame, paste with no move. Made by `mctas mobfarm return-bench --out` from the bench's own world; every coordinate in this header is generated from the same options.".into(),
        format!("# run line: {run_line}"),
        // (ironspec 22:07 PT (2b) / 22:03 PT (8): the frames this file's numbers are in)
        format!(
            "# frames: the run line's `--offsets {} --station-lag {}` is the emulated station, a calibration: at offsets 0,12,24 / lag 11 it reproduces dfb89499's real station at its offsets 0 / 14 / 24 (lane 3's y-12 at REL + 76 both ways); rows that count toward a bar run with `--station-part` (dfb89499's f1q2-station.txt, 3939d2012) at `--offsets 0,14,24`. Ticks after REL as a tick's end shows them: L7 = REL + {} (= REL + {} in day-1's frame, read at the next tick's start), the napper's hold rail REL + {} ({}).",
            o.offsets.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","),
            o.station_lag,
            o.lane7_rel,
            o.lane7_rel + 1,
            o.lane7_rel - 32,
            o.lane7_rel - 31
        ),
        if o.path_a {
            "# - merge row along z 7 (path (a)): lane 7 off it (its own ramp and climb, dfb89499's), so (7,0,7) a plain rail east_west joining the hold's line (8,0,7) to the row; Ts (3,0,7) and (-1,0,7) north_west; the main line (x,0,7) x -1..6 on glass ((5,0,7) a plain rail: ARM3 := REL + c, day-1); the west end ramped (2b): (-2,0,7) and (-3,1,7) ascending_west on glass (-2,-1,7), (-3,0,7), the entry (-4,2,7) east_west on glass (-4,1,7) into the lift at y 2, the old intake cell (-4,0,7) glass (the closed wall entry); the station's tracks and boosters (z 1..6) are dfb89499's, not here (the bench's station torches (x-1,0,5) on glass (x-1,-1,5), x -1/3/7, are EMULATED: dfb89499's cells).".into()
        } else {
            "# - merge row along z 7: lane 7's curve (7,0,7) north_west (a T once the hold's line joins from (8,0,7)), Ts (3,0,7) and (-1,0,7) north_west, the main line (x,0,7) x -4..6 on glass, glass over the wall entry (-4,1,7); the station's tracks and boosters (z 1..6) are dfb89499's, not here (the bench's station torches (x-1,0,5) on glass (x-1,-1,5), x -1/3/7, are EMULATED: dfb89499's cells).".into()
        },
        lift,
        exit,
        napper,
        "# - declared: each D (x,24,8)'s high end is its junction J (x,25,9) (3 potential connections) and J is a face of the cell over D; J is power-forced by its declared switch in both states, so D's press and release re-settle J to the settle shape of its power, a no-op: tick-exact's official D-J probe passes (J at its driver's settle, NE off / NW on, 2 gt after the lever; D ascending_south through every press, release, toggle while pressed and restart in each state; module-check 0 of 14 blocks differ; https://artifacts.internalmeta.com/a/1169573292176966).".to_string(),
        "# - switches EMULATED: a lever over each junction (face ceiling, glass over it) is the latch's output (`drive_j` only toggles the lever, flags ALL; redsim settles the junction); the latches: J(-1) on at idle; lane -1's branch rise turns it off and J(3) on; lane 3's turns J(3) off; the third cart's rise at the row's second detector turns J(-1) on, or opens the napper window (J(7) off on lane 7's branch rise; J(-1) on at the napper's own first rise at the row's first detector).".into(),
        if o.k_relay {
            format!("# - OPEN: day-1's drivers ({}; the hold's pulse into (7,-1,8)); lane -1's seat (-3,24,7): no rail of mine on a solid-render block within 2 of it (the booster's side source sits at (-6,{ex},8), beside the booster (-5,{ex},8), 3 from the seat), 9fc3155f to confirm with --clear-rate.", if o.day1_taps { "K set, K reset and J(-1)'s re-arm at day-1's taps after the napper's own ARM3, EMULATED here as the levers at those ticks" } else { "each K on after the batch's third cart passes it and before the napper, EMULATED here by the bench seeing the third cart" })
        } else {
            "# - OPEN: ironfarm's siding feeds (x-2,25,9) sit on this row: (1,25,9) and (5,25,9) are the row's detectors, (-3,25,9) is over the climb slope (-3,24,9) (where the rest's glass support goes); the K re-lay (power-forced junctions at (x-2,25,9)) is approved in direction, pending the rest's contact fix (ironfarm), the siding sensor (x-1,24,7) and day-1's K driver.".to_string()
        },
    ];
    if o.napper_lift || (o.row9 && o.k_relay) {
        // (the always-on sources (ironspec 14:01, V15; 9fc3155f's push-scan reads `always-on:`))
        let mut ao: Vec<String> = Vec::new();
        if o.napper_lift && o.napper_delay.is_some() {
            if o.no_torches {
                ao.push("8,-1,7=the hold's line booster (8,0,7)'s redstone block".into());
                ao.push("9,-3,5=the napper climb's booster (9,-2,5)'s redstone block".into());
                ao.push("9,-2,6=the napper climb's booster (9,-1,6)'s redstone block".into());
            } else {
                ao.push("8,-2,7=the hold's line booster (8,0,7)'s torch on glass (8,-3,7), under the slime (8,-1,7)".into());
                ao.push("8,-2,5=the napper climb's torch on glass (8,-3,5), beside (9,-2,5) (the chain to (9,-1,6))".into());
            }
            ao.push("8,-4,4=the napper climb's first booster (8,-3,4)'s redstone block".into());
        }
        if o.row9 && o.k_relay {
            ao.push(if o.no_torches { format!("-6,{ex},8=the lift exit booster (-5,{ex},8)'s side redstone block") } else { format!("-6,{ex},8=the lift exit's torch on glass (-6,{top},8), beside the booster (-5,{ex},8)") });
        }
        if !ao.is_empty() {
            head.push(format!("# always-on: {}", ao.join("; ")));
        }
        // (ironspec 21:25 PT (4) / 22:05 PT: the declared block contacts, `CELLS=REASON (evidence row)`)
        head.push(format!(
            "# contacts: {}",
            contact_groups(o, mz)
                .iter()
                .map(|(cs, why, ev)| format!(
                    "{}={why} ({})",
                    cs.iter().map(|(x, y, z)| format!("{x},{y},{z}")).collect::<Vec<_>>().join("+"),
                    ev.unwrap_or("bench only, the official row pending: the bench prints its speed change per row")
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
        // (rule C, tick-exact's water rows via farmopt 01:00: the flowing cells this file's water makes
        // on purpose, `CELLS=REASON (evidence row)`)
        head.push(format!("# water: {}", declared_water(o, mz).iter().map(|((x, y, z), why)| format!("{x},{y},{z}={why}")).collect::<Vec<_>>().join("; ")));
        // (push-scan (b), onechunk's syntax: each branch detector D (x,24,8) beside its J's T (x,25,9),
        // a declared switch source: its rise or fall updates the T, which re-shapes to its power's
        // settle (the bench's D-on-J line checks it at every change of every row); and each J / K T's
        // own lever, the switch the T static already reads (9fc3155f via farmopt 22:34))
        if o.path_a && o.row9 && !o.mirror_z {
            // (9fc3155f 23:22: the D detectors change with every cart, so they are update-only sources
            // (their update re-settles the T they reach); the levers are switches)
            let upd: Vec<String> = [-1, 3, 7]
                .iter()
                .map(|x| format!("{x},24,8=J({x})'s branch detector D, its own north arm's sensor: a rise or fall updates the T ({x},25,9), which re-settles to its power's shape (its lever ({x},26,9): north_west on, north_east off) (return-bench's D-on-J line: every change of every row, the T off its settle after one 0)"))
                .collect();
            head.push(format!("# update-only: {}", upd.join("; ")));
            let mut sw: Vec<String> = Vec::new();
            for x in [-1, 3, 7] {
                sw.push(format!("{x},26,9=J({x})'s lever (day-1's latch output; a ceiling lever under glass ({x},27,9)): on -> the T ({x},25,9) north_west, off -> north_east"));
                sw.push(format!("{},26,9=K({x})'s lever (day-1's timed taps; a ceiling lever under glass ({},27,9)): on -> the T ({},25,9) north_west, off -> north_east", x - 2, x - 2, x - 2));
            }
            if o.napper_lift && o.napper_delay.is_some() && o.pulse_gate.is_some() {
                sw.push("7,-3,4=the napper climb's exit detector, day-1's approach detector E (the waiting bit's source): redundant on the rail it reaches, (8,-3,4) being held on by its redstone block (8,-4,4)".to_string());
            }
            head.push(format!("# switches: {}", sw.join("; ")));
        }
        // (ironspec 23:50 PT: the way-in's curve (9,25,9), north_west in record7 (b8949c96's file), laid
        // here as the shape a rail with only its west neighbour settles to: a declared stand-in)
        if o.row9 && o.k_relay && o.napper_delay.is_some() && !o.mirror_z {
            head.push("# stand-in: 9,25,9=the way-in's curve (north_west in record7, b8949c96's file), here a rail east_west, the shape a rail with only its west neighbour settles to; this bench ends the napper's route at its K(x) and the batch carts' at their J, so no cart of the bench reaches it (the routing rows decide before it); guard: rail for rail (the same golem, path, late-path and sight properties; no power or update reach; it survives)".to_string());
        }
        // (9fc3155f's wet-spawn static via farmopt 22:34: the light in this file's water cells, from
        // redsim's light engine on the bench's world (this file's cells and the bench's stand-ins only;
        // the site and the module around it add their own))
        {
            let mut groups: std::collections::BTreeMap<String, (i32, i32, i32, i32, usize)> = std::collections::BTreeMap::new();
            for (p, b) in &spec.blocks {
                let s = b.to_string();
                if !(s.contains("water") || s.contains("bubble_column")) || s.contains("waterlogged") {
                    continue;
                }
                let key = if p.x == -5 && p.z == mz {
                    "the shared lift's column (-5,y,7)".to_string()
                } else if p.x == 6 && p.z == 4 {
                    "the napper climb's column (6,y,4)".to_string()
                } else {
                    format!("({},{},{})", p.x, p.y, p.z)
                };
                let sky = redsim::daylight::sky_light(w, *p).unwrap_or(-1);
                let blk = redsim::daylight::block_light_cached(w, *p, 1).unwrap_or(-1);
                let e = groups.entry(key).or_insert((i32::MAX, i32::MIN, i32::MAX, i32::MIN, 0));
                *e = (e.0.min(sky), e.1.max(sky), e.2.min(blk), e.3.max(blk), e.4 + 1);
            }
            head.push(format!(
                "# lightning (26.3's LightningBolt: every living entity in x +- 3, y - 3 .. y + 9, z +- 3 of the strike at the MOTION_BLOCKING heightmap; ironspec 22:56 / 22:59 PT): the riders' highest points in this file: the lift top (-5,{},7), feet y 23.3, head 25.25, under the cap glass (-5,{},7) (top y {}); the exit and the split row (x -5..9, y 25, z 9), feet 23.1..25.06, heads <= 27.01, under the glass (x,27,9) over the J / K levers (top y 28); the napper's climb and hold and every rider below y 23 sit >= 4 under the module's structure. A rider is unstrikable only where every motion-blocking top within 3 sideways stands more than 3 above its head (y > 28.25 over the lift top, > 30.01 over the exit and the split row): this file's own tops (y {} / 28) do not reach it: the cover is ONE lightning rod at the farm's highest point (ironspec 23:00 PT; onechunk places it; a rod takes every strike within 128 blocks; its removal is the positive control).",
                top + 1,
                top + 2,
                top + 3,
                top + 3
            ));
            head.push(format!(
                "# light (redsim's light engine on the bench's world: this file's cells and its stand-ins; the site and the module around them add their own; 9fc3155f's wet-spawn static): {}. Spawn rules (26.3): a drowned needs block light 0 and dark enough (raw brightness <= 0..7) and, outside river biomes, y < sea level - 5 = 58 (1 in 40; rivers 1 in 15 at any y); a glow squid y <= sea level - 33 = 30 and raw brightness 0; guardians only in monuments. So these water cells are spawn-proof wherever each sits at y >= 58 outside river biomes; lower, or in a river biome, a cell with block light 0 needs light (ironspec 22:54 PT: (b) the site rule, or (c) `--light-fix`: two glowstone blocks in place of wall glass (-6,8,7) and (5,-16,4), block light >= 1 in every cell).",
                groups.iter().map(|(k, (s0, s1, b0, b1, n))| format!("{k}: {n} cells, sky light {s0}..{s1}, block light {b0}..{b1} (the raw brightness at night, max(sky - 11, block), at least {}; in a thunderstorm, max(sky - 10, block), at least {})", (s0 - 11).max(*b0).max(0), (s0 - 10).max(*b0).max(0))).collect::<Vec<_>>().join("; ")
            ));
        }
    }
    if o.napper_lift {
        head.push(format!("# single-occupant: 9,0,8+9,0,7+8,0,7=one napper released per nap trip, on a fired pulse only (the gate); every arm of the T (9,0,7) carries only that napper's route (in from the staircase, rest on the hold (9,0,8) against the slime stop (9,0,9), out west on (8,0,7)); {t77} (return-bench: 0 batch carts on the hold's route over its late, jitter, restart and arrival-phase sweeps); consecutive nappers leave the bed stop >= 312 gt apart (day-1) and the stop -> hold trip spreads by <= 4 gt ({}, return-bench), against the hold's longest wait {} + the 12-gt pulse + clear: {}, a margin of {} gt (printed only: ironspec 19:07 PT (5) retired the inherited + 20 and measures the occupancy directly, the 0 batch carts above)",
            match o.lift_depth { 0 => "75..79".to_string(), 9 => "88..92".to_string(), d => format!("measured at depths 0 and 9; depth {d} not measured") },
            100 + o.pulse_gate.unwrap_or(38),
            100 + o.pulse_gate.unwrap_or(38) + 19,
            312 - (100 + o.pulse_gate.unwrap_or(38) + 19),
            t77 = if o.path_a { "(7,0,7) is a plain rail joining the hold's line to the row (path (a): lane 7 off the merge row), so no batch cart enters" } else { "lane 7's merge T (7,0,7) joins north and west only, so no batch cart enters" }
        ));
        if o.tripwire {
            head.push("# - --tripwire (option (b), not the adopted release): a north-south tripwire string (8,1,7) over the napper-only (8,0,7), hooks (8,1,6) facing south and (8,1,8) facing north on glass anchors (8,1,5), (8,1,9); with --dep-keyed the taps key to its rising edge".to_string());
        }
    }
    // (ironspec 17:46 / 17:49: every write the bench makes outside this file, declared EMULATED, with
    // its golem / path properties, to be checked against its owner's built cell with the statics run
    // there)
    if o.napper_lift {
        let d = o.lift_depth;
        head.push(format!(
            "# emulated (ironspec 17:46 / 17:49; the bench's writes outside this file, each to be checked against its owner's built cell with the statics run there): dfb89499's station z 1..6 (each lane's rails, its exit detector and boosters on glass{}){}; the bed stop's R (4,{},4) a rail on glass (4,{},4) (58f770cc's); ironfarm's merge cells (x,24,7) rails on glass (x,23,7), x -1/3/7; day-1's hold input (7,-1,8) (a conforming stand-in: a floor lever on glass (7,-2,8), toggled for the pulse; a wall lever on (7,-1,7) would update V15's T (7,0,7)){}. The bench's blocks: rails, the detector, levers and the repeater non-solid with no full top face; glass solid with a full top face and full-block collision, not solid-render, and refused by LEGACY_IRON_GOLEM spawning; the two inputs are non-solid line ends in the build (ironspec 18:24): the hold's a floor lever on glass, lane 7's a ceiling lever under existing glass, the same rise ticks as the redstone blocks they replace. In this file but standing in for day-1's drive: the levers over each J and K (x,26,9) (the latches' outputs, declared switches at each T).",
            if o.boosters && !o.no_torches { ", the torches (x-1,0,5) on glass (x-1,-1,5)" } else { "" },
            if o.path_a { "; dfb89499's climb top (-3,13,7) a rail" } else { "" },
            -19 - d,
            -20 - d,
            if o.path_a { "; day-1's lane 7 drive at (-4,12,6) (a conforming stand-in, onechunk's record7: a ceiling lever hung from the sign's glass (-4,13,6), no new support, toggled for the pulse in place of day-1's repeater (-4,12,6) and its input (-4,12,5))" } else { "" }
        ));
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
/// ironspec 00:27 PT: every bench schedules every fluid cell's tick (`LiquidBlock.onPlace` on each
/// water cell of its world, as the game does when the cell is placed) and prints the count; a world
/// with water and no fluid tick scheduled is an error.
fn start_run(spec: &FarmSpec, ro: &RunOpts) -> Result<(Run, usize, usize), String> {
    let mut run = Run::start(spec, ro)?;
    let water: Vec<BlockPos> = spec.blocks.iter().filter(|(_, b)| matches!(b, Block::Water { .. })).map(|(p, _)| *p).collect();
    for p in &water {
        let s = run.world.get(*p);
        if matches!(s, Block::Water { .. }) {
            redsim::fluid::liquid_on_place(&mut run.world, s, *p);
        }
    }
    let n = run.world.fluid_ticks.count();
    if !water.is_empty() && n == 0 {
        return Err(format!("{} water cells and no fluid tick scheduled (ironspec 00:27 PT)", water.len()));
    }
    Ok((run, water.len(), n))
}

fn mirror_spec(spec: &FarmSpec) -> Result<FarmSpec, String> {
    let blocks = spec.blocks.iter().map(|(p, b)| Ok((BlockPos::new(p.x, p.y, 3 - p.z), mirror_block(b)?))).collect::<Result<Vec<_>, String>>()?;
    Ok(FarmSpec { blocks, lo: BlockPos::new(spec.lo.x, spec.lo.y, 3 - spec.hi.z), hi: BlockPos::new(spec.hi.x, spec.hi.y, 3 - spec.lo.z), ..spec.clone() })
}

/// tick-exact's t-snap positive control (ironspec 12:35): its 8 starts (copy k at x offset 16k, a
/// villager-ridden cart summoned on the slope (9,-1,6) at (9.5, -0.9375 + (z - 6), z) with Motion
/// (0, 0, vz)) through the T and hold, either in tick-exact's geometry (`official`: stone under the
/// T, a stone stop, (8,0,7) air, the tripwire over it) or in this bench's (`mine`: glass under the T,
/// the slime stop with glass over it, the hold's line booster (8,0,7), lane 7's merge T (7,0,7));
/// prints tick-exact's table: t1..t25, each copy's cart and rider, x in the design frame, y in the
/// scene frame (+64), z.
pub fn t_snap(mode: &str, torches: bool) -> Result<String, String> {
    let mine = match mode {
        "official" => false,
        "mine" => true,
        _ => return Err(format!("--t-snap {mode}: official or mine")),
    };
    let starts: [(f64, f64); 8] = [(6.6, 0.05), (6.6, 0.15), (6.6, 0.35), (6.95, 0.048), (6.9, 0.148), (6.8, 0.348), (6.6, 0.6), (6.8, 0.8)];
    let mut b: Vec<(BlockPos, Block)> = Vec::new();
    let glass = Block::parse("minecraft:glass")?;
    let stone = Block::parse("minecraft:stone")?;
    let rb = Block::parse("minecraft:redstone_block")?;
    let rail = |s: &str| Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"));
    let pr = |s: &str, on: bool| Block::parse(&format!("minecraft:powered_rail[powered={on},shape={s},waterlogged=false]"));
    for k in 0..8i32 {
        let ox = 16 * k;
        let mut put = |x: i32, y: i32, z: i32, blk: &Block| b.push((BlockPos::new(x + ox, y, z), blk.clone()));
        put(7, -3, 4, &rail("east_west")?);
        put(7, -4, 4, &glass);
        if mine {
            put(8, -3, 4, &pr("ascending_east", true)?);
            put(8, -4, 4, &rb);
        } else {
            put(8, -3, 4, &rail("ascending_east")?);
            put(8, -4, 4, &glass);
        }
        put(9, -2, 4, &rail("south_west")?);
        put(9, -3, 4, &glass);
        put(9, -2, 5, &pr("ascending_south", true)?);
        put(9, -1, 6, &pr("ascending_south", true)?);
        // (this bench's V15 cells with `torches`: the climb's torch (8,-2,5), glass under the slopes;
        // the hold's support slime (9,-1,8); the hold's line on slime (8,-1,7) over the torch (8,-2,7))
        let v15 = mine && torches;
        if v15 {
            put(9, -3, 5, &glass);
            put(9, -2, 6, &glass);
            put(8, -2, 5, &Block::parse("minecraft:redstone_torch[lit=true]")?);
            put(8, -3, 5, &glass);
        } else {
            put(9, -3, 5, &rb);
            put(9, -2, 6, &rb);
        }
        put(9, 0, 7, &rail("south_west")?);
        put(9, -1, 7, if mine { &glass } else { &stone });
        put(9, 0, 8, &pr("north_south", false)?);
        put(9, -1, 8, &if v15 { Block::parse("minecraft:slime_block")? } else { stone.clone() });
        if mine {
            put(9, 0, 9, &Block::parse("minecraft:slime_block")?);
            put(9, 1, 9, &glass);
            put(8, 0, 7, &pr("east_west", true)?);
            if v15 {
                put(8, -1, 7, &Block::parse("minecraft:slime_block")?);
                put(8, -2, 7, &Block::parse("minecraft:redstone_torch[lit=true]")?);
                put(8, -3, 7, &glass);
            } else {
                put(8, -1, 7, &rb);
            }
            put(7, 0, 7, &rail("north_west")?);
            put(7, -1, 7, &glass);
            put(7, 0, 6, &rail("north_south")?);
            put(7, -1, 6, &glass);
            put(6, 0, 7, &rail("east_west")?);
            put(6, -1, 7, &glass);
        } else {
            put(9, 0, 9, &stone);
            put(8, 1, 5, &stone);
            put(8, 1, 9, &stone);
            put(8, 1, 6, &Block::parse("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?);
            put(8, 1, 8, &Block::parse("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?);
            put(8, 1, 7, &Block::parse("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?);
        }
    }
    let spec = FarmSpec {
        blocks: b,
        entities: Vec::new(),
        output: BlockPos::new(0, 3, 0),
        lo: BlockPos::new(0, -6, 0),
        hi: BlockPos::new(16 * 7 + 12, 4, 12),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    };
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 60, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let mut ids = Vec::new();
    for (k, (z, vz)) in starts.iter().enumerate() {
        let nbt = format!("{{Motion:[0.0d,0.0d,{vz:.4}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}");
        let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [9.5 + 16.0 * k as f64, -0.9375 + (z - 6.0), *z], Some(&nbt))?;
        let rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
        ids.push((id, rider));
    }
    let mut out = vec![format!("# t-snap through {} geometry (return-bench --t-snap {mode}); the table's format: t copy role x(frame) y(scene) z", if mine { "this bench's" } else { "tick-exact's" }), "# t copy role x(frame) y(scene) z".to_string()];
    for t in 1..=25 {
        run.tick();
        for (k, (id, rider)) in ids.iter().enumerate() {
            for (role, e) in [("cart", Some(*id)), ("rider", *rider)] {
                let Some(e) = e else { continue };
                let Some(i) = redsim::entity::index_of(&run.world, e) else { continue };
                let p = run.world.entities.list[i].pos;
                out.push(format!("{t} {k} {role} {:.6} {:.6} {:.6}", p.x - 16.0 * k as f64, p.y + 64.0, p.z));
            }
        }
    }
    Ok(out.join("\n"))
}

/// `station_part` (ironspec 20:16): dfb89499's record station laid over the bench's world, each cell
/// replacing the bench's (the box grown to hold it), its Nether placeholder left out (obsidian and
/// nether_portal: "the bench's PLACEHOLDER, not the build", its header); the pockets' drive
/// stand-ins added: a floor lever (x-2,0,0) on glass (x-2,-1,0) per lane, refused if the part holds
/// either cell. Returns a line for the report.
fn lay_station(spec: &mut FarmSpec, path: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("--station-part {path}: {e}"))?;
    let d = mctas::design::Design::parse(&text).map_err(|e| format!("--station-part {path}: {e}"))?;
    let mut map: std::collections::HashMap<BlockPos, Block> = spec.blocks.drain(..).collect();
    let (mut laid, mut skipped, mut replaced) = (0usize, 0usize, 0usize);
    for (p, s) in &d.blocks {
        if s.starts_with("minecraft:obsidian") || s.starts_with("minecraft:nether_portal") {
            skipped += 1;
            continue;
        }
        let bp = BlockPos::new(p[0], p[1], p[2]);
        spec.lo = BlockPos::new(spec.lo.x.min(bp.x), spec.lo.y.min(bp.y), spec.lo.z.min(bp.z));
        spec.hi = BlockPos::new(spec.hi.x.max(bp.x), spec.hi.y.max(bp.y), spec.hi.z.max(bp.z));
        let b = Block::parse(s)?;
        // (a part cell where the bench has a different block is refused; the same block is shared)
        match map.insert(bp, b) {
            Some(old) if old != b => {
                return Err(format!("--station-part {path}: its cell ({},{},{}) {s} would replace the bench's {}", bp.x, bp.y, bp.z, old));
            }
            Some(_) => replaced += 1,
            None => {}
        }
        laid += 1;
    }
    for x in LANES {
        let (lv, sup) = (BlockPos::new(x - 2, 0, 0), BlockPos::new(x - 2, -1, 0));
        if map.contains_key(&lv) || map.contains_key(&sup) {
            return Err(format!("--station-part: the pocket drive's stand-in cells ({},0,0) / ({},-1,0) are taken in the part", x - 2, x - 2));
        }
        map.insert(lv, Block::parse("minecraft:lever[face=floor,facing=north,powered=false]")?);
        map.insert(sup, Block::parse("minecraft:glass")?);
    }
    // (G1 on 1420794e6 (1): each pocket lever's REACH LINE: the six neighbours of the lever and of its
    // support, the powered components among them (a lever toggled updates both sets; only day-1's
    // repeater (x-1,0,0) is expected))
    let reacts = |b: &Block| {
        let n = b.to_string();
        ["repeater", "comparator", "redstone_wire", "rail", "piston", "door", "redstone_lamp", "note_block", "dispenser", "dropper", "hopper", "tnt", "observer", "bell", "fence_gate", "crafter", "copper_bulb"].iter().any(|k| n.contains(k))
    };
    let mut reach: Vec<String> = Vec::new();
    for x in LANES {
        let mut comps: Vec<String> = Vec::new();
        for c in [BlockPos::new(x - 2, 0, 0), BlockPos::new(x - 2, -1, 0)] {
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                let q = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                if let Some(b) = map.get(&q)
                    && reacts(b)
                    && !comps.iter().any(|s| s.starts_with(&format!("({},{},{})", q.x, q.y, q.z)))
                {
                    comps.push(format!("({},{},{}) {}", q.x, q.y, q.z, b.to_string().trim_start_matches("minecraft:")));
                }
            }
        }
        reach.push(format!("lane {x}'s lever ({},0,0): {}", x - 2, if comps.is_empty() { "none".to_string() } else { comps.join(", ") }));
    }
    let mut v: Vec<(BlockPos, Block)> = map.into_iter().collect();
    v.sort_by_key(|(p, _)| (p.x, p.y, p.z));
    spec.blocks = v;
    Ok(format!(
        "station part {path}: {laid} cells laid ({replaced} of them the bench's own block, shared; a cell with a different block would be refused), {skipped} Nether-placeholder cells left out (out of scope), the pockets' drive levers at (x-2,0,0) on glass (x-2,-1,0) (DECLARED STAND-INS for day-1's line-13 cells); the levers' reach (G1 on 1420794e6 (1): the powered components among the six neighbours of each lever and of its support): {}",
        reach.join("; ")
    ))
}

/// One run: the report and its failure counts.
fn return_run(o: &ReturnOpts) -> Result<(String, Summary), String> {
    let mz = if o.z7 { 7 } else { 8 };
    let spec = build(o)?;
    if o.mirror_z && o.out.is_some() {
        return Err("--out with --mirror-z: emit the design unmirrored (the mirrored copy is onechunk's)".into());
    }
    // (`station_part`: dfb89499's record station composed (ironspec 20:16); the file's own cells
    // come from the bench without it, so --out refuses it)
    let mut station_line = None;
    let spec = if let Some(path) = &o.station_part {
        if o.out.is_some() || o.mirror_z || !o.napper_lift {
            return Err("--station-part: not with --out or --mirror-z; needs --napper-lift (the 100-gt prelude: each batch's carts sit in the pockets 30 gt before its REL)".into());
        }
        let mut s = spec;
        station_line = Some(lay_station(&mut s, path)?);
        s
    } else {
        spec
    };
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
    let (mut run, water_n, fluid_n) = start_run(&spec, &ro)?;
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
    if let Some(l) = &station_line {
        out.push(l.clone());
    }
    // (ironspec 00:27 PT: the fluid cells' ticks, scheduled at the start)
    out.push(format!("fluid: {water_n} water cells, {fluid_n} fluid ticks scheduled at the start (LiquidBlock.onPlace on each water cell, as placing it does; ironspec 00:27 PT: 0 with water fails)"));
    // (rule C, tick-exact's water rows via farmopt 01:00: the water the settle made where the build
    // laid none, each a declared current (`declared_water()`, the file's `# water:` line) or failing)
    let water_new: Vec<(BlockPos, String)> = {
        let laid: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| matches!(b, Block::Water { .. } | Block::BubbleColumn { .. })).map(|(p, _)| *p).collect();
        let (mn, sz) = (run.world.min, run.world.size);
        let mut v = Vec::new();
        for x in mn.x..mn.x + sz[0] {
            for y in mn.y..mn.y + sz[1] {
                for z in mn.z..mn.z + sz[2] {
                    let p = BlockPos::new(x, y, z);
                    let b = run.world.get(p);
                    if matches!(b, Block::Water { .. }) && !laid.contains(&p) {
                        v.push((p, b.to_string()));
                    }
                }
            }
        }
        v
    };
    let dw = declared_water(o, mz);
    // (ironspec 01:07 PT (2b): on a saved-settled design the count must be 0; a nonzero one fails,
    // declared or not (a declaration annotates a saved cell, it never excuses an unsettled save))
    let water_undeclared = water_new.len();
    out.push(format!(
        "new water after the settle (rule C, ironspec 01:07 PT: 0 on a saved-settled design; any fails, the `# water:` currents being saved cells): {}",
        if water_new.is_empty() {
            "none".to_string()
        } else {
            water_new.iter().map(|(p, s)| format!("({},{},{}) {} ({}, FAILS: not in the saved design)", p.x, p.y, p.z, s.trim_start_matches("minecraft:"), if dw.iter().any(|(c, _)| *c == (p.x, p.y, p.z)) { "a `# water:` current" } else { "undeclared" })).collect::<Vec<_>>().join("; ")
        }
    ));
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
    let mut carts: Vec<Cart> = (0..total).map(|_| Cart { id: None, rider: None, launch_live: 0, lane: None, branch_v: None, merge_v: None, branch_t: None, least_air: i32::MAX, hurt: 0.0, health0: None, eye_in_wall: 0, dismounted: None, met_shaft: None, reach12: None, head: f64::NEG_INFINITY, w_d: None, w_lift12: None, w_hold: None, w_merge: None, w_k: None, v_k: None, w_j1: None, on_hold_route: false, w_exit: None, w_rest: None, w_leave: None, w_line: None, w_top: None, w_rest0: None, w_y12: None, w_y13: None, w_top_exit: None, w_dep: None, w_row: None, v_t: None, air_trace: Vec::new(), launch_t: 0, merge_min: None, split_min: None, prev: None, end: None }).collect();
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
    let mut timed_turns: Vec<i64> = Vec::new();
    let mut pending_j: Vec<(i64, i32, bool)> = Vec::new();
    let mut at_j: Vec<(i64, i32, usize)> = Vec::new();
    let mut at_j_v: Vec<f64> = Vec::new();
    let mut j_last: std::collections::HashMap<(i32, usize), i64> = std::collections::HashMap::new();
    let mut frozen_ticks = 0i64;
    let mut freeze_box: Option<[i32; 4]> = None;
    let mut touches = crate::liftbench::Touches::default();
    let mut trace: Vec<String> = Vec::new();
    let mut shaft_gap = f64::INFINITY;
    let mut restarted = 0usize;
    let (mut restart_dt, mut restart_moved): (Option<i64>, Option<Option<(usize, f64)>>) = (None, None);
    // (ironspec 21:25 PT (4): the carts' contacts with blocks, from each move's collision flags: a
    // cell its box meets (1e-3 out) on a flagged tick -> (first t, last t, flagged ticks, carts))
    let mut contacts: std::collections::BTreeMap<(i32, i32, i32), (i64, i64, usize, Vec<usize>, f64)> = std::collections::BTreeMap::new();
    // (each cart's speed at the previous tick's end, for a contact's speed change; the cart-ticks
    // checked, the contacts' samples (ironspec 23:56 PT))
    let mut prev_speed: Vec<Option<f64>> = Vec::new();
    let mut contact_samples = 0usize;
    // (the `# update-only:` declarations' evidence, path (a): per J (x,25,9) with its branch detector D
    // (x,24,8): the D's changes, the T off its power's settle after one, and the T re-shaped in a D
    // change's tick with its lever unchanged (the update re-shaping it))
    let mut dj: [(usize, usize, usize); 3] = [(0, 0, 0); 3];
    let mut dj_prev: [Option<(bool, String, bool)>; 3] = [None, None, None];
    let (mut k_on, mut j_deferred) = (false, false);
    // (`day1_taps`: the napper's own ARM3 (its releasing pulse - D) and the batch whose arming it was)
    let mut nap_arm: Option<i64> = None;
    let mut skipped_pulses: usize = 0;
    let mut arms: Vec<i64> = Vec::new();
    let mut det_rise: Option<i64> = None;
    let mut col_min: Option<(f64, usize, usize, i64)> = None;
    let mut j_vmax = [0.0f64; 3];
    let mut slope_rises: Vec<i64> = Vec::new();
    let mut slope_was = false;
    let mut extra_busy: usize = 0;
    let mut rel_at: Option<i64> = None;
    // (each fired pulse's presence sample at E's rise (ARM3 + 2): the napper at rest on the hold, or
    // rolling, and its stored speed)
    let (mut fired_arms, mut samples): (Vec<i64>, Vec<String>) = (Vec::new(), Vec::new());
    let (mut samples_rolling, mut samples_away): (usize, usize) = (0, 0);
    let (mut trip_was, mut trips_seen): (bool, Vec<i64>) = (false, Vec::new());
    let mut hold_mid: i64 = 0;
    let mut nap_b_pulse: Option<usize> = None;
    let (mut thru_v_k, mut hold_intrusions): (Option<f64>, usize) = (None, 0);
    let mut thru_v_k_hi: Option<f64> = None;
    let (mut nap_b, mut k_on_at): (Option<usize>, Option<i64>) = (None, None);
    let mut k_off_at: Option<i64> = None;
    let mut k_first: Vec<Option<i64>> = vec![None; total];
    let mut k_last: Vec<Option<i64>> = vec![None; total];
    let (mut hold_released, mut hold_wait): (bool, Option<(f64, f64, f64)>) = (false, None);
    let (mut t_offset, mut t_cross, mut t_det) = (0.0f64, 0usize, Vec::<String>::new());
    for t in 0..run_ticks {
        // (J(-1)'s switch this tick came from day-1's timed re-arm tap (ARM3 + 127 + 4), not a cart)
        let mut lm_timed = false;
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
            let at = prelude + o.station_lag + (k as i64 / 3) * 100 + offsets[k % 3] + o.jitter.get(k).copied().unwrap_or(0);
            // (`station_part`: the cart placed at rest in its pocket (x,0,0) 30 gt before its batch's
            // REL (the holds' refill not modelled); it leaves on the pocket's own drive below)
            let at = if o.station_part.is_some() { prelude + (k as i64 / 3) * 100 - 30 } else { at };
            if c.id.is_none() && (o.game_ticks || !frozen) && (if o.game_ticks { t } else { live }) >= at {
                let x = order[k % 3];
                // (`path_a`: lane 7's cart starts at rest on the braked slope (-4,12,mz) (dfb89499's
                // ramp and climb not modelled: it waits there before its release))
                let (pos, nbt) = if o.station_part.is_some() {
                    ([x as f64 + 0.5, 0.0625, 0.5], "{Motion:[0.0d,0.0d,0.0d],Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".to_string())
                } else if o.path_a && x == 7 {
                    // (it arrives from the climb's top (-3,13,mz) rolling west at 0.1 and settles on the
                    // braked slope by itself, near its crest, as tick-exact's official rows rest it)
                    ([-2.5, 13.0625, if o.mirror_z { 3.5 - mz as f64 } else { mz as f64 + 0.5 }], "{Motion:[-0.1d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".to_string())
                } else {
                    (
                        [x as f64 + 0.5, 0.0625, if o.mirror_z { 2.5 } else { 1.5 }],
                        format!("{{Motion:[0.0d,0.0d,{:.4}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", if o.mirror_z { -o.speed } else { o.speed }),
                    )
                };
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", pos, Some(&nbt))?;
                c.id = Some(id);
                // (`station_part`: the launch is the pocket's drive (the lever on at REL + offset - 1))
                c.launch_live = if o.station_part.is_some() { live + (prelude + (k as i64 / 3) * 100 + offsets[k % 3] + o.jitter.get(k).copied().unwrap_or(0) - 1 - at) } else { live };
                c.launch_t = t;
                c.rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
        }
        // (`station_part`: each pocket's drive (dfb89499 19:55): the lever stand-in (x-2,0,0) on at
        // REL + offset - 1 into day-1's repeater (x-1,0,0) (the part's), the pocket's rail rising at
        // REL + offset, off 16 gt later (the re-brake))
        if o.station_part.is_some() {
            let clock = if o.game_ticks { t } else { live };
            let prel = if o.napper_lift { 100 } else { 0 };
            for k in 0..n {
                let on = prel + (k as i64 / 3) * 100 + offsets[k % 3] + o.jitter.get(k).copied().unwrap_or(0) - 1;
                if clock == on || clock == on + 16 {
                    run.world.set_block(BlockPos::new(order[k % 3] - 2, 0, 0), Block::parse(&format!("minecraft:lever[face=floor,facing=north,powered={}]", clock == on))?, redsim::world::flags::ALL);
                }
            }
        }
        // ((c)'s napper: the arming detector's first rise (the batch 0 third cart), the hold's
        // release D gt after it (a repeater chain: game ticks in L1, live in L2), the napper
        // leaving (8,0,8) west at 0.4)
        // (`path_a`: lane 7's release is toggled with the controller's writes, just before the tick:
        // see `lane7_drive` below)
        if let Some(d) = o.napper_delay {
            let a = if let Some(c0) = o.arm_rel {
                // (ARM3 := REL + c (day-1 14:41): the arming at a fixed tick of each cycle)
                let clock = if o.game_ticks { t } else { live };
                let prel = if o.napper_lift { 100 } else { 0 };
                // (`arm_rel` c is ARM3's rise in game ticks after REL: the emulated arming seen at the
                // start of tick REL + c + 1, as a signal risen in tick REL + c; the hold rail then rises
                // in tick REL + c + D)
                (0..o.batches as i64).any(|b| clock == prel + 100 * b + c0 + 1)
            } else if o.napper_lift && o.arm_station {
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
                    Some(x) if !o.gate_delayed_only => !hold_released && det_rise.or(carts[n].w_exit).is_some_and(|we| now + 2 - we >= x),
                    // (the control: the delayed copy alone, still high for X gt after the reset, so the
                    // departing napper's own second ARM3 rise fires a phantom pulse)
                    Some(x) => det_rise.or(carts[n].w_exit).is_some_and(|we| now + 2 - we >= x) && rel_at.is_none_or(|r| now + 2 < r + x),
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
                let nbt = format!("{{Motion:[{:.4}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", o.napper_speed);
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [4.5, -18.9375 - o.lift_depth as f64, if o.mirror_z { -0.5 } else { 4.5 }], Some(&nbt))?;
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
                // (the approach detector (7,-3,4)'s own powered state: its first rise once the
                // napper is out, for the detector -> rest the official ride measures)
                if det_rise.is_none() && carts[n].id.is_some() && matches!(run.world.get(BlockPos::new(7, -3, zf(4))), Block::DetectorRail { powered: true, .. }) {
                    det_rise = Some(t);
                }
                if fired_arms.iter().any(|a| a + 2 == clock) {
                    let st = carts[n].id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| {
                        let e = &run.world.entities.list[i];
                        let z = if o.mirror_z { 4.0 - e.pos.z } else { e.pos.z };
                        let v = (e.delta_movement.x * e.delta_movement.x + e.delta_movement.z * e.delta_movement.z).sqrt();
                        let on = (e.pos.x.floor() as i32, e.pos.y.floor() as i32, z.floor() as i32) == (9, 0, 8);
                        format!("t {clock}: {} ({v:.3})", if on && v < 0.005 { "at rest on the hold" } else if on { "rolling on the hold" } else { "not on the hold" })
                    });
                    samples.push(st.unwrap_or_else(|| format!("t {clock}: no napper")));
                    if let Some(s) = samples.last() {
                        if s.contains("rolling") {
                            samples_rolling += 1;
                        } else if !s.contains("at rest") {
                            samples_away += 1;
                        }
                    }
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
                    run.world.set_block(BlockPos::new(7, -1, zf(8)), Block::parse(&format!("minecraft:lever[face=floor,facing={},powered=true]", if o.mirror_z { "south" } else { "north" }))?, redsim::world::flags::ALL);
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
                    run.world.set_block(BlockPos::new(7, -1, zf(8)), Block::parse(&format!("minecraft:lever[face=floor,facing={},powered=false]", if o.mirror_z { "south" } else { "north" }))?, redsim::world::flags::ALL);
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
                // (day-1 17:59: the K-set taps `kset_shift` later (path (a): +4, K-on 8..11 after the
                // third cart); the reset, PRESENT edge and J(-1) re-arm stay)
                let set = set + o.kset_shift;
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
                        k_off_at = Some(t);
                        drive_j(&mut run.world, true, o.napper_lane - 2, ex, zf(rz), false)?;
                    }
                    if o.napper_lane != -1 && clock == a + 127 + shift + 4 {
                        j_deferred = false;
                        lm = true;
                        lm_timed = true;
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
        // (`driver_latency` (day-1, ironspec 19:38 (3)): day-1's driver switches the rail 4 gt after a
        // set edge and 6 after a reset edge, the edge being the press in tick p; the bench's
        // controller sees that press at t = p + 1 (see `settle_margin`), so the switch is queued to
        // t + 3 (set) / t + 5 (reset): written at p + 4 / p + 6, read from those ticks (block phase);
        // the taps already carry the driver's +4 / +6)
        if l3 != p3 {
            if o.driver_latency {
                pending_j.push((t + if l3 { 3 } else { 5 }, 3, l3));
            } else {
                drive_j(&mut run.world, o.row9 || o.row10, 3, ex, zf(rz), l3)?;
                turns.push((t, 3, l3));
            }
        }
        if lm != pm {
            if o.driver_latency && !lm_timed {
                pending_j.push((t + if lm { 3 } else { 5 }, -1, lm));
            } else {
                drive_j(&mut run.world, o.row9 || o.row10, -1, ex, zf(rz), lm)?;
                turns.push((t, -1, lm));
                if lm_timed {
                    timed_turns.push(t);
                }
            }
        }
        let due: Vec<(i64, i32, bool)> = pending_j.iter().copied().filter(|p| p.0 == t).collect();
        pending_j.retain(|p| p.0 != t);
        for (_, j, st) in due {
            drive_j(&mut run.world, o.row9 || o.row10, j, ex, zf(rz), st)?;
            turns.push((t, j, st));
        }
        // (`restart`: a save, stop and start between two ticks (the lead's L1 rows), through the
        // shared helper (cc767b78's `liveness::restart`, in the given entity order); the carts and
        // riders followed to their new ids)
        // (ironspec 21:07 / 21:13 PT, tick-exact: the official load is one block-only tick (game time
        // + 1, redstone and scheduled ticks run, no entity moves); measured across the helper's call:
        // its game-time delta and each moving cart's position (held = clean, moved = re-run))
        if let Some((rt, how)) = o.restart
            && rt == t
        {
            let g0 = run.world.time;
            let before: Vec<Option<(f64, f64, f64, bool)>> = carts
                .iter()
                .map(|c| c.id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| {
                    let e = &run.world.entities.list[i];
                    let s = crate::liftbench::CartState::of(e);
                    (s.x, s.y, s.z, s.vx * s.vx + s.vy * s.vy + s.vz * s.vz > 1e-12)
                }))
                .collect();
            let moved = crate::liveness::restart(&mut run.world, how)?;
            restarted = moved.iter().filter(|(a, b)| a != b).count();
            for c in carts.iter_mut() {
                c.id = c.id.map(|i| crate::liveness::follow(&run.world, i));
                c.rider = c.rider.map(|i| crate::liveness::follow(&run.world, i));
            }
            restart_dt = Some(run.world.time - g0);
            let mut worst: Option<(usize, f64)> = None;
            for (k, b) in before.iter().enumerate() {
                if let Some((x, y, z, true)) = b
                    && let Some(i) = carts[k].id.and_then(|id| redsim::entity::index_of(&run.world, id))
                {
                    let p = &run.world.entities.list[i].pos;
                    let d = ((p.x - x).powi(2) + (p.y - y).powi(2) + (p.z - z).powi(2)).sqrt();
                    if worst.is_none_or(|w| d > w.1) {
                        worst = Some((k, d));
                    }
                }
            }
            restart_moved = Some(worst);
        }
        // (`lane7_drive`, `path_a`: lane 7's release, the slope (-4,12,mz) powered for `lane7_pulse`
        // gt by the ceiling lever stand-in (-4,12,mz-1), written with the controller's writes before
        // tick R (R = REL + `lane7_rel`): the carts see the rail powered from tick R, as day-1's
        // repeater chain powered it in tick R's block phase (its tap written before tick R - 1); the
        // per-tick records (taken after each tick) first show it at R: the printed slope-rail rise
        // equals `lane7_rel`)
        if o.path_a {
            let clock = if o.game_ticks { t } else { live };
            let prel = if o.napper_lift { 100 } else { 0 };
            for b in 0..o.batches as i64 {
                let r0 = prel + 100 * b + o.lane7_rel;
                if clock == r0 || clock == r0 + o.lane7_pulse {
                    run.world.set_block(BlockPos::new(-4, 12, zf(mz - 1)), Block::parse(&format!("minecraft:lever[face=ceiling,facing={},powered={}]", if o.mirror_z { "south" } else { "north" }, clock == r0))?, redsim::world::flags::ALL);
                }
            }
        }
        run.tick();
        let mut sink: Vec<u32> = Vec::new();
        let mut close_window = false;
        let st: Vec<Option<crate::liftbench::CartState>> = carts.iter().map(|c| c.id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| crate::liftbench::CartState::of(&run.world.entities.list[i]))).collect();
        // (ironspec 21:25 PT (4): the carts' block contacts this tick, with the speed change over it)
        prev_speed.resize(carts.len(), None);
        for (k, c) in carts.iter().enumerate() {
            if let Some(i) = c.id.and_then(|id| redsim::entity::index_of(&run.world, id)) {
                let e = &run.world.entities.list[i];
                let v = &e.delta_movement;
                let speed = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
                if e.horizontal_collision || e.vertical_collision {
                    let dv = prev_speed[k].map_or(0.0, |p| speed - p);
                    for (p, _) in redsim::entity::collide::block_collisions(&run.world, e.bb.inflate(1e-3, 1e-3, 1e-3)) {
                        let en = contacts.entry((p.x, p.y, p.z)).or_insert((t, t, 0, Vec::new(), 0.0));
                        en.1 = t;
                        en.2 += 1;
                        en.4 = en.4.min(dv);
                        if !en.3.contains(&k) {
                            en.3.push(k);
                        }
                    }
                }
                prev_speed[k] = Some(speed);
                contact_samples += 1;
            } else {
                prev_speed[k] = None;
            }
        }
        // (path (a)'s D-on-J evidence: each J's T against its power's settle at each of its D's changes)
        if o.path_a && !o.mirror_z {
            for (j, x) in [-1, 3, 7].into_iter().enumerate() {
                let (ds, ts, ls) = (run.world.get(BlockPos::new(x, 24, 8)).to_string(), run.world.get(BlockPos::new(x, 25, 9)).to_string(), run.world.get(BlockPos::new(x, 26, 9)).to_string());
                let (d_on, l_on) = (ds.contains("powered=true"), ls.contains("powered=true"));
                let shape = ts.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("").to_string();
                if let Some((pd, ps, pl)) = &dj_prev[j]
                    && *pd != d_on
                {
                    dj[j].0 += 1;
                    if shape != if l_on { "north_west" } else { "north_east" } {
                        dj[j].1 += 1;
                    }
                    if *ps != shape && *pl == l_on {
                        dj[j].2 += 1;
                    }
                }
                dj_prev[j] = Some((d_on, shape, l_on));
            }
        }
        // (`path_a`: lane 7's slope rail's rises, as the tick's end shows them)
        if o.path_a {
            let on = matches!(run.world.get(BlockPos::new(-4, 12, zf(mz))), Block::Rail { powered: true, .. });
            if on && !slope_was {
                slope_rises.push(t);
            }
            slope_was = on;
        }
        // (the least centre distance between two carts both in the lift's column (x -5, z mz), and
        // which pair (ironspec 14:41: 0 push contacts in the column, the least distance printed))
        {
            let col: Vec<(usize, f64, f64, f64)> = st
                .iter()
                .enumerate()
                .filter_map(|(k, s)| s.map(|s| (k, s.x, s.y, s.z)))
                .filter(|(_, x, _, z)| x.floor() as i32 == -5 && z.floor() as i32 == zf(mz))
                .collect();
            for i in 0..col.len() {
                for j in i + 1..col.len() {
                    let (a, b) = (col[i], col[j]);
                    let dd = ((a.1 - b.1).powi(2) + (a.2 - b.2).powi(2) + (a.3 - b.3).powi(2)).sqrt();
                    if col_min.is_none_or(|(m, _, _, _)| dd < m) {
                        col_min = Some((dd, a.0, b.0, t));
                    }
                }
            }
        }
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
                if c.w_rest0.is_none() && cell == (9, 0, 8) && vx == 0.0 && vz == 0.0 {
                    c.w_rest0 = Some(t);
                }
                if c.w_top.is_none() && cell == (6, -3, 4) {
                    c.w_top = Some(t);
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
            if c.w_y12.is_none() && bp.x == -5 && bp.z == mz && y >= 12.0 {
                c.w_y12 = Some(t);
            }
            // (ironspec 20:01 PT (2): the tick a cart's box fully clears the mouth cell at y 12 (its
            // feet at y >= 13 in the column), against lane 7's entry)
            if c.w_y13.is_none() && bp.x == -5 && bp.z == mz && y >= 13.0 {
                c.w_y13 = Some(t);
            }
            // (ironspec 20:03 PT: the column bar is measured at the exit top, the contact point: each
            // cart's first tick on the exit booster's cell (-5,top+1,mz+1), the build's lift top + 1:
            // 23 here (r2 compared y against the split row's 25, so the exit-top pairs were never
            // measured: fixed with the no-vacuous print that showed it))
            let lift_ex = if o.row9 && (o.k_relay || o.climb2) { 23 } else if o.row10 || o.row9 { 25 } else if o.flat || o.dip { 22 } else { 23 };
            if c.w_top_exit.is_none() && bp.x == -5 && bp.y == lift_ex && bp.z == mz + 1 {
                c.w_top_exit = Some(t);
            }
            // (`station_part`'s observables: the departure, the merge-row entry and its speed)
            let hv = (s.vx * s.vx + s.vz * s.vz).sqrt();
            if c.w_dep.is_none() && (hv > 1e-9 || s.vy.abs() > 1e-9) {
                c.w_dep = Some(t);
            }
            if c.w_row.is_none() && k < n && bp.y == 0 && bp.z == mz && bp.x == order[k % 3] {
                c.w_row = Some((t, hv));
            }
            // (ironspec 17:20 (b): the stored speeds the geometry produces on each J (J(-1), J(3), J(7))
            // over every cart, for J(-1)'s junction-settle bar)
            if bp.y == ex && bp.z == rz
                && let Some(i) = [-1, 3, 7].iter().position(|&jx| jx == bp.x)
            {
                j_vmax[i] = j_vmax[i].max((s.vx * s.vx + s.vz * s.vz).sqrt());
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
            if bp.y == ex && bp.z == rz && (bp.x == 3 || bp.x == -1) {
                if at_j.last().is_none_or(|l| l.2 != k || l.1 != bp.x) {
                    at_j.push((t, bp.x, k));
                    at_j_v.push((s.vx * s.vx + s.vz * s.vz).sqrt());
                }
                j_last.insert((bp.x, k), t);
            }
            // (ironspec 18:16 (3): every cart's ticks in the napper's K's cell (napper_lane - 2, ex, rz))
            if o.k_relay && bp.y == ex && bp.z == rz && bp.x == o.napper_lane - 2 {
                k_first[k].get_or_insert(t);
                k_last[k] = Some(t);
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
                    // (ironspec 15:08: the riders' air every tick from their entry into the column to
                    // the lift's top exit (path (a): lane 7 by the side slope), as run-length pairs)
                    if (re.pos.x.floor() as i32 == -5 || re.pos.x.floor() as i32 == -4) && re.pos.z.floor() as i32 == zf(mz) && re.pos.y >= 0.0 {
                        match c.air_trace.last_mut() {
                            Some((a, n)) if *a == m.air => *n += 1,
                            _ => c.air_trace.push((m.air, 1)),
                        }
                    }
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
        "split: {wrong} wrong lane(s) of {n}; the batch carts' stored speed entering their branch {}, entering their lane's merge cell {} (ironfarm's delivery band 0.30..0.48, inherited from its part spec, margins {}); the least gap from a junction's turn to the next cart at it: {}; the shaft's closest pair {}",
        rng(bv),
        rng(mv),
        if mv.0.is_finite() { format!("{:.3} / {:.3}", mv.0 - 0.30, 0.48 - mv.1) } else { "-".into() },
        least.join(", "),
        if shaft_gap.is_finite() { format!("{shaft_gap:.3}") } else { "-".into() }
    ));
    out.push(format!("pairs: {}", touches.line()));
    // (ironspec 17:20 (b), cc767b78: per J(-1) / J(3) switch, settle = the switch -> the first cart
    // needing its new state entering the cell, clear = the last cart needing its old state leaving
    // the cell -> the switch; each least with its switch and cart. A batch cart needs its own lane's
    // J set and every J west of it reset; the napper needs every J it reaches reset)
    let need_on = |k: usize, j: i32| k < n && order[k % 3] == j;
    let mut jsc: Vec<String> = Vec::new();
    let mut j_settle: [Option<i64>; 2] = [None; 2];
    let mut j_clear: [Option<i64>; 2] = [None; 2];
    let mut j_clear_cart: [Option<i64>; 2] = [None; 2];
    for (ji, j) in [-1, 3].into_iter().enumerate() {
        let mut best_s: Option<(i64, i64, bool, usize, f64)> = None;
        // (the clear side split: day-1's timed switches (the re-arm tap) against the cart-triggered
        // ones (a D rise: the switching cart has left the junction by construction))
        let mut best_c: Option<(i64, i64, bool, usize)> = None;
        let mut best_cc: Option<(i64, i64, bool, usize)> = None;
        for &(tt, jj, s) in turns.iter().filter(|x| x.1 == j) {
            if let Some((idx, &(ta, _, k))) = at_j.iter().enumerate().find(|(_, a)| a.1 == jj && a.0 >= tt && need_on(a.2, j) == s) {
                let g = settle_margin(tt, ta, false);
                if best_s.is_none_or(|b| g < b.0) {
                    best_s = Some((g, tt, s, k, at_j_v.get(idx).copied().unwrap_or(f64::NAN)));
                }
            }
            if let Some(&(_, _, k)) = at_j.iter().filter(|a| a.1 == jj && a.0 < tt && need_on(a.2, j) != s).last()
                && let Some(&last) = j_last.get(&(jj, k))
            {
                let g = clear_margin(tt, last, false);
                let slot = if j == -1 && timed_turns.contains(&tt) { &mut best_c } else { &mut best_cc };
                if slot.is_none_or(|b| g < b.0) {
                    *slot = Some((g, tt, s, k));
                }
            }
        }
        j_settle[ji] = best_s.map(|b| b.0);
        j_clear[ji] = best_c.map(|b| b.0);
        j_clear_cart[ji] = best_cc.map(|b| b.0);
        let who = |k: usize| if k < n { format!("cart {k} (batch {}, lane {})", k / 3, order[k % 3]) } else { format!("cart {k} (the napper)") };
        let sr = |s: bool| if s { "set" } else { "reset" };
        let cl = |b: Option<(i64, i64, bool, usize)>| b.map_or("-".to_string(), |(g, tt, s, k)| format!("{g} gt ({} out of the cell, then its {} at t {tt})", who(k), sr(s)));
        jsc.push(format!(
            "J({j}): settle {}, clear at its timed switches {}, clear at its cart-triggered switches {}",
            best_s.map_or("-".to_string(), |(g, tt, s, k, v)| format!("{g} gt (its {} at t {tt}, then {} entering at stored {v:.3})", sr(s), who(k))),
            cl(best_c),
            cl(best_cc)
        ));
    }
    out.push(format!("junction settle / clear (ironspec 17:20 (b): each switch against the first cart needing its new state and the last needing its old one, least per junction): {}", jsc.join("; ")));
    // (ironspec 18:16 (3): the napper's K(n-2) under day-1's timed taps: on: clear (the last batch cart
    // through K out of its cell -> K on) and settle (K on -> the napper entering); off: the napper out
    // of K's cell -> K off, and K off -> the next batch cart entering)
    let (k_clear, k_settle, k_off_margin, k_off_settle) = if o.k_relay && o.day1_taps {
        let thru_before = k_on_at.and_then(|on| (0..n).filter(|&k| k_first[k].is_some_and(|f| f < on)).filter_map(|k| k_last[k].map(|l| (l, k))).max());
        let kc = k_on_at.zip(thru_before).map(|(on, (l, k))| (clear_margin(on, l, false), k));
        let ks = k_on_at.zip(k_first.get(n).copied().flatten()).map(|(on, f)| settle_margin(on, f, false));
        let km = k_off_at.zip(k_last.get(n).copied().flatten()).map(|(off, l)| clear_margin(off, l, false));
        let next_after = k_off_at.and_then(|off| (0..n).filter_map(|k| k_first[k].filter(|f| *f > off).map(|f| (f, k))).min());
        let kos = k_off_at.zip(next_after).map(|(off, (f, k))| (settle_margin(off, f, false), k));
        let who = |k: usize| format!("cart {k} (batch {}, lane {})", k / 3, order[k % 3]);
        out.push(format!(
            "K({}) (the split's K T at ({},25,9); ironspec 22:03 PT's names: K(x), apart from k_sw and k_lock) under day-1's timed taps (ironspec 18:16 (3)): on at t {}: clear {} , settle {}; off at t {}: the napper out of K's cell {} before it, the next batch cart into K {} after it",
            o.napper_lane,
            o.napper_lane - 2,
            k_on_at.map_or("-".to_string(), |v| v.to_string()),
            kc.map_or("NOT JUDGED (0 samples)".to_string(), |(g, k)| format!("{g} gt ({} out of the cell, then K on)", who(k))),
            ks.map_or("NOT JUDGED (0 samples)".to_string(), |g| format!("{g} gt (K on, then the napper entering)")),
            k_off_at.map_or("-".to_string(), |v| v.to_string()),
            km.map_or("NOT JUDGED (0 samples)".to_string(), |g| format!("{g} gt")),
            kos.map_or("NOT JUDGED (0 samples)".to_string(), |(g, k)| format!("{g} gt ({})", who(k)))
        ));
        (kc.map(|x| x.0), ks, km, kos.map(|x| x.0))
    } else {
        (None, None, None, None)
    };
    // (ironspec 19:14 / cc767b78: one entry per switch cause, with its phase: the K taps and J(-1)'s
    // re-arm are taps (block); the D presses are entity-phase raw (timed from the press tick: settle
    // + 1, clear - 1 against the bench's toggle) or block-phase through day-1's driver under
    // `driver_latency`. Order: K set, K reset, J(-1) re-arm, J(-1) set, J(-1) reset, J(3) set, J(3) reset)
    let (cause_settle, cause_clear) = {
        let press_phase = if o.driver_latency { "block" } else { "entity" };
        let who = |k: usize| if k < n { format!("cart {k} (batch {}, lane {})", k / 3, order[k % 3]) } else { format!("cart {k} (the napper)") };
        // (the J causes: (name, cause, junction, state, timed))
        let jc: [(&str, &str, i32, bool, bool); 5] = [
            ("J(-1) re-arm", "tap", -1, true, true),
            ("J(-1) set", "lane 7's D press", -1, true, false),
            ("J(-1) reset", "lane -1's D press", -1, false, false),
            ("J(3) set", "lane -1's D press", 3, true, false),
            ("J(3) reset", "lane 3's D press", 3, false, false),
        ];
        let mut cs: [Option<i64>; 7] = [None; 7];
        let mut cc: [Option<i64>; 7] = [None; 7];
        let mut entries: Vec<String> = Vec::new();
        let kname = format!("K({})", o.napper_lane);
        let opt = |v: Option<i64>, what: String| v.map_or("NOT JUDGED (0 samples)".to_string(), |g| format!("{g} gt ({what})"));
        cs[0] = k_settle;
        cc[0] = k_clear;
        cs[1] = k_off_settle;
        cc[1] = k_off_margin;
        entries.push(format!("{kname} set (tap, block): settle {}, clear {}", opt(k_settle, "K on, then the napper in".into()), opt(k_clear, "the last batch cart out, then K on".into())));
        entries.push(format!("{kname} reset (tap, block): settle {}, clear {}", opt(k_off_settle, "K off, then the next batch cart in".into()), opt(k_off_margin, "the napper out, then K off".into())));
        for (i, (name, cause, j, st, timed)) in jc.iter().enumerate() {
            // (a D press without the driver: entity phase, timed from the press tick)
            let from_press = !*timed && !o.driver_latency;
            let mut bs: Option<(i64, i64, usize)> = None;
            let mut bc: Option<(i64, i64, usize)> = None;
            for &(tt, jj, s) in turns.iter().filter(|x| x.1 == *j && x.2 == *st) {
                let is_timed = *j == -1 && timed_turns.contains(&tt);
                if is_timed != *timed {
                    continue;
                }
                if let Some(&(ta, _, k)) = at_j.iter().find(|a| a.1 == jj && a.0 >= tt && need_on(a.2, *j) == s) {
                    let g = settle_margin(tt, ta, from_press);
                    if bs.is_none_or(|b| g < b.0) {
                        bs = Some((g, tt, k));
                    }
                }
                if let Some(&(_, _, k)) = at_j.iter().filter(|a| a.1 == jj && a.0 < tt && need_on(a.2, *j) != s).last()
                    && let Some(&last) = j_last.get(&(jj, k))
                {
                    let g = clear_margin(tt, last, from_press);
                    if bc.is_none_or(|b| g < b.0) {
                        bc = Some((g, tt, k));
                    }
                }
            }
            cs[i + 2] = bs.map(|b| b.0);
            cc[i + 2] = bc.map(|b| b.0);
            let phase = if *timed { "block" } else { press_phase };
            entries.push(format!(
                "{name} ({cause}, {phase}): settle {}, clear {}",
                bs.map_or("-".to_string(), |(g, tt, k)| format!("{g} gt (the switch at t {tt}, then {} in)", who(k))),
                bc.map_or("-".to_string(), |(g, tt, k)| format!("{g} gt ({} out, then the switch at t {tt})", who(k)))
            ));
        }
        out.push(format!("junction settle / clear by cause (ironspec 19:14; a press timed from the press tick): {}", entries.join("; ")));
        (cs, cc)
    };
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
    let met_n = met_abs.get(n).copied().flatten();
    let nb = met_n.and_then(|m| (0..o.batches).rev().find(|b| met_abs[b * 3 + 2].is_some_and(|t3| t3 < m)));
    let nap_behind = met_n.zip(nb.and_then(|b| met_abs[b * 3 + 2])).map(|(m, t3)| m - t3);
    let nap_ahead = met_n.zip(nb.filter(|b| b * 3 + 3 < n).and_then(|b| met_abs.get(b * 3 + 3).copied().flatten())).map(|(m, f1)| f1 - m);
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
    // (ironspec 20:01 PT (2): lane 7's entry into the column against the preceding cart's box fully
    // out of the mouth cell (its feet at y >= 13): the least over the batches, with the batch)
    let mouth: Option<(i64, usize, i64)> = if o.path_a {
        (0..n / 3).filter_map(|b| Some((carts[3 * b + 2].w_y12? - carts[3 * b + 1].w_y13?, b, carts[3 * b + 1].w_y13?))).min_by_key(|x| x.0)
    } else {
        None
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
    // (ironspec 14:41 (2) / 18:01: every E rise (ARM3 + 2), every fired pulse's E rise (the reset of
    // the waiting bit) and its rail rise, and the E rises in (reset, reset + k], k the gate's
    // (0 everywhere: the live term comes out))
    let kk = o.pulse_gate.unwrap_or(42);
    let e_within: Vec<i64> = arms.iter().copied().filter(|a| fired_arms.iter().any(|f| *a > *f && *a <= *f + kk)).map(|a| a + 2).collect();
    let hold_line = format!(
        "{hold_line}; E rises: {}; fired E rises (resets): {}; fired pulse rises: {}; E rises within {kk} gt after a reset: {}{}",
        if arms.is_empty() { "none".to_string() } else { arms.iter().map(|a| format!("t {}", a + 2)).collect::<Vec<_>>().join(", ") },
        if fired_arms.is_empty() { "none".to_string() } else { fired_arms.iter().map(|a| format!("t {}", a + 2)).collect::<Vec<_>>().join(", ") },
        if fired_arms.is_empty() { "none".to_string() } else { pulses.iter().map(|p| format!("t {p}")).collect::<Vec<_>>().join(", ") },
        e_within.len(),
        if e_within.is_empty() { String::new() } else { format!(" (t {})", e_within.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")) }
    );
    // (`day1_taps` or a gate: the release bar holds ungated too (ironspec 14:41's ungated control
    // must fail it: a pulse onto an empty hold, a pop without a departure, a launch before rest))
    let release_bad = (o.pulse_gate.is_some() || o.day1_taps) && (rested.len() > 1 || (leave_t.is_some() && on_fired.is_none()) || creeps > 0 || pops_wo_dep > 0 || (leave_t.is_some() && rested.is_empty()));
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
        // (ironspec 15:08: every rider's air, tick by tick, in the column (run-length: air x ticks))
        for (k, c) in carts.iter().enumerate() {
            if c.air_trace.is_empty() {
                continue;
            }
            out.push(format!(
                "air in the column, cart {k}{}: {} (least {}, hurt {:.1})",
                if k == n { " (the napper)" } else { "" },
                c.air_trace.iter().map(|(a, n)| if *n > 1 { format!("{a}x{n}") } else { a.to_string() }).collect::<Vec<_>>().join(" "),
                c.least_air,
                c.hurt
            ));
        }
        // (`station_part`, ironspec 20:16: each batch cart's observables from the real station's
        // cells, REL-relative: its departure from the pocket, its merge-row entry and speed there,
        // its y-12 pass, for tick-exact's official station ride (within 1 gt / 0.005 b/gt))
        if o.station_part.is_some() {
            let prel = if o.napper_lift { 100 } else { 0 };
            out.push(format!(
                "station (the record part's cells), after each batch's REL: {}",
                carts
                    .iter()
                    .enumerate()
                    .take(n)
                    .map(|(k, c)| {
                        let rel = prel + 100 * (k / 3) as i64;
                        format!(
                            "b{} lane {}: departs REL + {}, the merge row REL + {}, y 12 REL + {}",
                            k / 3,
                            order[k % 3],
                            c.w_dep.map_or("-".to_string(), |t| (t - rel).to_string()),
                            c.w_row.map_or("- (path (a)'s lane 7: its climb)".to_string(), |(t, v)| format!("{} at |v| {v:.3}", t - rel)),
                            c.w_y12.map_or("-".to_string(), |t| (t - rel).to_string())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        if o.path_a {
            out.push(format!(
                "path (a), in game ticks after REL (the tick a state first shows at a tick's end): lane 7's slope rail rises {:?}; ARM3 rises {:?}; the napper's hold rail rises {}; y-12 passes after REL: {}; the napper's y-12 gap behind {} and ahead {}; lane 7's mouth clearance {}; the least distance between two carts in the column {}",
                {
                    let prel = if o.napper_lift { 100 } else { 0 };
                    slope_rises.iter().map(|r| r - prel - 100 * ((r - prel) / 100)).collect::<Vec<_>>()
                },
                {
                    let prel = if o.napper_lift { 100 } else { 0 };
                    (0..o.batches as i64).filter_map(|b| arms.iter().copied().find(|a| *a >= prel + 100 * b && *a < prel + 100 * (b + 1)).map(|a| a - 1 - (prel + 100 * b))).collect::<Vec<_>>()
                },
                {
                    let prel = if o.napper_lift { 100 } else { 0 };
                    on_fired.map_or("- (none fired)".to_string(), |(k, _)| {
                        let p = pulses[k - 1] - 1;
                        let b = nap_b_pulse.map_or((p - prel).div_euclid(100), |b| b as i64);
                        format!("REL + {} (batch {b}'s REL)", p - (prel + 100 * b))
                    })
                },
                carts
                    .iter()
                    .enumerate()
                    .map(|(k, c)| {
                        let prel = if o.napper_lift { 100 } else { 0 };
                        let b = if k < n { (k / 3) as i64 } else { nap_b_pulse.map_or(0, |b| b as i64) };
                        format!("{}{}", if k < n { format!("b{} lane {}", k / 3, order[k % 3]) } else { "napper".to_string() }, c.w_y12.map_or(" -".into(), |y| format!(" +{}", y - (prel + 100 * b))))
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                {
                    let ny = carts.get(n).and_then(|c| c.w_y12);
                    ny.and_then(|t| carts[..n].iter().filter_map(|c| c.w_y12).filter(|y| *y <= t).max().map(|y| t - y)).map_or("-".to_string(), |v| v.to_string())
                },
                {
                    let ny = carts.get(n).and_then(|c| c.w_y12);
                    ny.and_then(|t| carts[..n].iter().filter_map(|c| c.w_y12).filter(|y| *y > t).min().map(|y| y - t)).map_or("- (no batch after it in the run)".to_string(), |v| v.to_string())
                },
                mouth.map_or("- (unmeasured)".to_string(), |(g, b, t13)| format!("{g} gt (batch {b}: lane 3's box out of the mouth cell, feet at y 13, at t {t13}; lane 7 into the column {g} gt later)")),
                col_min.map_or("-".into(), |(d, a, b, tt)| format!("{d:.3} (carts {a} and {b} at t {tt})"))
            ));
        }
        if o.k_relay {
            let nap_c = carts.get(n);
            out.push(format!(
                "K and J(-1) for the napper (after its own ARM3 at {}): entering K({}) at stored {} (tick {}); on J(-1)'s cell at ARM3 + {}, the next batch's first cart on it at ARM3 + {}; the through-carts' least stored speed on a K's cell {}; batch carts on the napper hold's route (the T (9,0,7), (8,0,7), the hold; bar 0) {}",
                nap_arm.map_or("-".into(), |a| a.to_string()),
                o.napper_lane,
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
            // (`path_a`: lane 7 enters at y 12, so the foot pair is lane -1 -> lane 3 only)
            (Some(a0), Some(a1), Some(a2)) => Some(if o.path_a { a1 - a0 } else { (a1 - a0).min(a2 - a1) }),
            _ => None,
        })
        .min()
        .unwrap_or(i64::MIN);
    // (G1 on 83878113f: path (a)'s y-12 pairs barred in clean(), and the column order)
    let (y12_pairs, y12_order_bad, y12_bad, y12_why, y12_n) = {
        fn upd(s: &mut Option<i64>, v: i64) {
            *s = Some(s.map_or(v, |m| m.min(v)));
        }
        let w: Vec<Option<i64>> = carts.iter().map(|c| c.w_y12).collect();
        let nb = n / 3;
        let mut p: [Option<i64>; 5] = [None; 5];
        // (ironspec 23:56 PT: each pair's sample count)
        let mut pc: [usize; 5] = [0; 5];
        let mut missing = false;
        let ny = w.get(n).copied().flatten();
        // (the napper's batch: the last whose lane 7 passed y 12 before it)
        let nap_b = ny.and_then(|t| (0..nb).filter(|&b| w[3 * b + 2].is_some_and(|l7| l7 <= t)).max());
        for b in 0..nb {
            let (a, l3, l7) = (w[3 * b], w[3 * b + 1], w[3 * b + 2]);
            if let (Some(x), Some(y)) = (a, l3) {
                upd(&mut p[0], y - x);
                pc[0] += 1;
            }
            match (l3, l7) {
                (Some(x), Some(y)) => {
                    upd(&mut p[1], y - x);
                    pc[1] += 1;
                }
                _ => missing = true,
            }
            let next = if b + 1 < nb { w[3 * (b + 1)] } else { None };
            if nap_b == Some(b) {
                if let (Some(x), Some(t)) = (l7, ny) {
                    upd(&mut p[2], t - x);
                    pc[2] += 1;
                }
                if let (Some(t), Some(nx)) = (ny, next) {
                    upd(&mut p[3], nx - t);
                    pc[3] += 1;
                }
            } else if let (Some(x), Some(nx)) = (l7, next) {
                upd(&mut p[4], nx - x);
                pc[4] += 1;
            }
        }
        let mut seq: Vec<(i64, usize)> = w.iter().enumerate().filter_map(|(k, t)| t.map(|t| (t, k))).collect();
        seq.sort();
        let expect: Vec<usize> = (0..nb)
            .flat_map(|b| {
                let mut v = vec![3 * b, 3 * b + 1, 3 * b + 2];
                if nap_b == Some(b) {
                    v.push(n);
                }
                v
            })
            .collect();
        let order_bad = seq.iter().map(|(_, k)| *k).collect::<Vec<_>>() != expect;
        // (the column bar 14 + 2 = 16 on every pair (ironspec 19:07 / 20:01 PT): the batch pairs
        // (lane 3 -> lane 7, lane 7 -> the next lane -1 with no napper between) at --y12-min, the
        // napper's (after lane 7, before the next batch's first) at --y12-nap-min; lane 7's mouth
        // clearance at --mouth-min, an unmeasured one failing as "unmeasured", not as a margin)
        let why = if !o.path_a {
            ""
        } else if missing {
            "a cart never passed y 12"
        } else if order_bad {
            "the column order at y 12"
        } else if pc[1] == 0 || (n < carts.len() && pc[2] == 0) || (nap_b.is_some_and(|b| b + 1 < nb) && pc[3] == 0) || (nb >= 2 && (nb - 1) > usize::from(nap_b.is_some_and(|b| b + 1 < nb)) && pc[4] == 0) {
            // (ironspec 23:56 PT: a barred pair with no sample is not judged: the batch pair lane 3 ->
            // 7 always, the napper's lane 7 -> it with a napper, its next one when a batch follows the
            // napper's in the run, lane 7 -> the next lane -1 when some batch gap carries no napper:
            // out of the run's scope otherwise (the napper after the last batch has no next lane -1))
            "NOT JUDGED (0 samples): a barred y-12 pair"
        } else if [p[1], p[4]].iter().any(|v| v.is_some_and(|v| v < o.y12_min)) || [p[2], p[3]].iter().any(|v| v.is_some_and(|v| v < o.y12_nap_min)) {
            "a y-12 pair under its bar"
        } else if mouth.is_none() {
            "lane 7's mouth clearance unmeasured"
        } else if mouth.is_some_and(|m| m.0 < o.mouth_min) {
            "lane 7's mouth clearance under its bar"
        } else {
            ""
        };
        (p, order_bad, !why.is_empty(), why, pc)
    };
    if o.path_a {
        let s = |v: Option<i64>| v.map_or("NOT JUDGED (0 samples)".to_string(), |v| v.to_string());
        // (ironspec 20:03 PT: the same pairs at the exit top, the contact point: each cart's first
        // tick on the exit booster's cell; the column bar 14 + 2 is read here)
        let top: Vec<Option<i64>> = carts.iter().map(|c| c.w_top_exit).collect();
        let tp = |a: usize, b: usize| -> Option<i64> { Some(top.get(b).copied().flatten()? - top.get(a).copied().flatten()?) };
        let nb = n / 3;
        let ny_top = top.get(n).copied().flatten();
        let nap_bt = ny_top.and_then(|t| (0..nb).filter(|&b| top[3 * b + 2].is_some_and(|l7| l7 <= t)).max());
        let mut tops: [Option<i64>; 4] = [None; 4];
        let upd = |slot: &mut Option<i64>, v: Option<i64>| {
            if let Some(v) = v {
                *slot = Some(slot.map_or(v, |m: i64| m.min(v)));
            }
        };
        for b in 0..nb {
            upd(&mut tops[0], tp(3 * b, 3 * b + 1));
            upd(&mut tops[1], tp(3 * b + 1, 3 * b + 2));
            if nap_bt == Some(b) {
                upd(&mut tops[2], tp(3 * b + 2, n));
                if b + 1 < nb {
                    upd(&mut tops[3], tp(n, 3 * (b + 1)));
                }
            }
        }
        out.push(format!(
            "path (a) y-12 pairs (bar >= {}: the column bar 14 + 2): lane -1 -> lane 3 {} (the foot pair: the shaft entry's gap bars it), lane 3 -> lane 7 {}, lane 7 -> the napper {}, the napper -> the next batch's lane -1 {}, lane 7 -> the next batch's lane -1 with no napper between {}; the column order at y 12 {}; the same pairs at the exit top (the contact point, the exit booster's cell): lane -1 -> lane 3 {}, lane 3 -> lane 7 {}, lane 7 -> the napper {}, the napper -> the next lane -1 {}; samples (ironspec 23:56 PT: the pairs measured, in the same order) {} / {} / {} / {} / {}",
            o.y12_min,
            s(y12_pairs[0]),
            s(y12_pairs[1]),
            s(y12_pairs[2]),
            s(y12_pairs[3]),
            s(y12_pairs[4]),
            if y12_order_bad { "WRONG" } else { "-1, 3, 7 per batch, the napper after its batch's lane 7" },
            s(tops[0]),
            s(tops[1]),
            s(tops[2]),
            s(tops[3]),
            y12_n[0],
            y12_n[1],
            y12_n[2],
            y12_n[3],
            y12_n[4]
        ));
    }
    // (ironspec 21:25 PT (4) / 22:05 PT: every cell the carts' moves touched: a declared contact
    // (`declared_contacts()`, the file's `# contacts:` line) or the row fails; a bench-only contact
    // (no official row) prints its speed change)
    let declared = declared_contacts(o, mz);
    let (mut listed, mut undeclared): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for (p, (f, l, n, ks, dv)) in &contacts {
        let name = run.world.get(BlockPos::new(p.0, p.1, p.2)).to_string();
        let name = name.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
        let s = format!("({},{},{}) {name} t {f}..{l} ({n} ticks, carts {})", p.0, p.1, p.2, ks.iter().map(|k| k.to_string()).collect::<Vec<_>>().join("/"));
        match declared.iter().find(|(c, _, _)| c == p) {
            Some((_, _, Some(_))) => listed.push(s),
            Some((_, _, None)) => listed.push(format!("{s}, bench-only: speed change {dv:+.4} b/gt at the most")),
            None => undeclared.push(format!("{s}, speed change {dv:+.4} b/gt at the most")),
        }
    }
    // (ironspec 23:56 PT: no cart-tick checked would be no judgment: it fails as one undeclared)
    let contacts_undeclared = undeclared.len() + usize::from(contact_samples == 0);
    out.push(format!(
        "block contacts (ironspec 21:25 PT (4): the cells the carts' boxes meet on a tick their move collided; the samples: {} cart-ticks checked{}): {} cells; {} declared ({}); undeclared {}",
        contact_samples,
        if contact_samples == 0 { ", NOT JUDGED (0 samples)" } else { "" },
        contacts.len(),
        listed.len(),
        listed.join("; "),
        if undeclared.is_empty() { "none".to_string() } else { undeclared.join("; ") }
    ));
    if o.path_a && !o.mirror_z {
        out.push(format!(
            "D-on-J (the `# update-only:` declarations: each branch detector D (x,24,8)'s rise or fall updates its J's T (x,25,9)): {}",
            ["J(-1)", "J(3)", "J(7)"].iter().zip(dj.iter()).map(|(n, d)| if d.0 == 0 { format!("{n} NOT JUDGED (0 samples: no detector change)") } else { format!("{n} {} detector changes (the samples), the T off its power's settle after one {}, re-shaped by the update {}", d.0, d.1, d.2) }).collect::<Vec<_>>().join("; ")
        ));
    }
    // (cc767b78's wording: the restart's game-time delta and a moving cart's position across the load)
    if let Some(d) = restart_dt {
        out.push(format!(
            "restart game-time delta {d}; a moving cart's position across the load: {} (the official load is one block-only tick: delta 1, carts held; this bench's restart helper runs no block-only tick yet)",
            match restart_moved.flatten() {
                None => "none moving".to_string(),
                Some((_, dd)) if dd <= 1e-9 => "held".to_string(),
                Some((k, dd)) => format!("moved (cart {k}, delta {dd:.4})"),
            }
        ));
    }
    let summary = Summary {
        y12_pairs,
        y12_order_bad,
        y12_bad,
        y12_why,
        j_vmax,
        j_settle,
        j_clear,
        j_clear_cart,
        k_clear,
        k_settle,
        k_off_margin,
        k_off_settle,
        cause_settle,
        cause_clear,
        path_a: o.path_a,
        restart_dt,
        restart_moved,
        contact_cells: contacts.len(),
        contacts_undeclared,
        dj,
        water_undeclared,
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
        det_to_rest: carts.get(n).and_then(|c| c.w_rest).zip(det_rise).map(|(r, d)| r - d),
        det_to_rest0: carts.get(n).and_then(|c| c.w_rest0).zip(det_rise).map(|(r, d)| r - d),
        t_top: carts.get(n).and_then(|c| c.w_top),
        arm_c: {
            let prel = if o.napper_lift { 100 } else { 0 };
            let mut v = [None; 8];
            for b in 0..(o.batches as i64).min(8) {
                v[b as usize] = arms.iter().copied().find(|a| *a >= prel + 100 * b && *a < prel + 100 * (b + 1)).map(|a| a - (prel + 100 * b));
            }
            v
        },
        pulse_to_y12: carts.get(n).and_then(|c| c.w_y12).zip(on_fired.map(|(k, _)| pulses[k - 1])).map(|(y, p)| y - p),
        y12_rel: {
            let prel = if o.napper_lift { 100 } else { 0 };
            let mut v = [None; 16];
            for (k, c) in carts.iter().enumerate().take(16) {
                let b = if k < n { (k / 3) as i64 } else { nap_b_pulse.map_or(0, |b| b as i64) };
                v[k] = c.w_y12.map(|y| y - (prel + 100 * b));
            }
            v
        },
        nap_index: n,
        col_min: col_min.map(|(d, _, _, _)| d),
        gap12_behind: carts.get(n).and_then(|c| c.w_y12).and_then(|ny| carts[..n].iter().filter_map(|c| c.w_y12).filter(|y| *y <= ny).max().map(|y| ny - y)),
        gap12_ahead: carts.get(n).and_then(|c| c.w_y12).and_then(|ny| carts[..n].iter().filter_map(|c| c.w_y12).filter(|y| *y > ny).min().map(|y| y - ny)),
        e_within_k: e_within.len(),
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
        samples_rolling,
        samples_away,
        t_phase: carts.get(n).and_then(|c| c.w_hold).and_then(|h| arms.iter().copied().filter(|a| *a <= h).max().map(|a| h - a)),
        pulse_mismatch: nap_arm.map(|a| {
            // (the consuming arming: the first whose E rise (ARM3 + 2) is >= detection + the bit's delay
            // (the gate's X when gated); detection = the detector's own rise when the gate lays it, as
            // the gate itself reads it)
            let det = det_rise.or(carts.get(n).and_then(|c| c.w_exit));
            let bd = o.pulse_gate.unwrap_or(o.bit_delay);
            let consumed = det.and_then(|d| arms.iter().copied().find(|&x| x + 2 >= d + bd));
            consumed != Some(a)
        }),
    };
    out.extend(trace);
    Ok((out.join("\n"), summary))
}

#[cfg(test)]
mod record_file_tests {
    use super::*;

    /// The design of record's run line (`f1q2-lift-split-path-a.txt`'s header): these options, every
    /// other flag at its CLI default (the lift depth 9 among them, ironspec 20:53 PT).
    fn record_opts(out: &str) -> ReturnOpts {
        ReturnOpts {
            batches: 3,
            spacing: 14,
            speed: 0.4,
            ticks: 450,
            supp_open: 73,
            napper_speed: 0.2,
            y12_min: 16,
            y12_nap_min: 16,
            mouth_min: 2,
            bit_delay: 36,
            z7: true,
            row9: true,
            k_relay: true,
            napper_lane: 3,
            day1_taps: true,
            reverse: true,
            boosters: true,
            game_ticks: true,
            offsets: vec![0, 12, 24],
            napper_delay: Some(12),
            napper_lift: true,
            release_pulse: 12,
            pulse_gate: Some(42),
            lift_depth: 9,
            path_a: true,
            lane7_rel: 95,
            station_lag: 11,
            lane7_pulse: 18,
            kset_shift: 4,
            driver_latency: true,
            // (ironspec 01:07 / 01:08 PT (4): the light fix adopted: the record lift file is the
            // `--light-fix` emission)
            light_fix: true,
            arm_rel: Some(51),
            out: Some(out.to_string()),
            ..Default::default()
        }
    }

    /// ironspec 20:53 PT: the committed path (a) lift file (record7's lift) is the bench's emission
    /// of its own run line, byte for byte, so a verdict run needs no flag beyond that line.
    #[test]
    fn the_record_lift_file_is_the_default_emission() {
        let committed = include_str!("../../../designs/farms/iron/carousel-assembly/wip/lift/f1q2-lift-split-path-a.txt");
        let path = std::env::temp_dir().join(format!("f1q2-lift-split-path-a-{}.txt", std::process::id()));
        let p = path.to_str().expect("a utf-8 temp path");
        let o = record_opts(p);
        let spec = build(&o).expect("the record geometry builds");
        let ro = RunOpts { ai: true, ..RunOpts::steady(1, 100, WalkMode::Random) };
        let (mut run, _, _) = start_run(&spec, &ro).expect("the record world starts");
        while run.settle_tick() {}
        write_design(&o, &spec, &run.world, p, 7).expect("the emitter's static gates pass");
        let emitted = std::fs::read_to_string(&path).expect("the emitted file");
        let _ = std::fs::remove_file(&path);
        assert!(emitted == committed, "the committed record lift file differs from the bench's emission of its run line: re-emit it with that run line");
    }
}
