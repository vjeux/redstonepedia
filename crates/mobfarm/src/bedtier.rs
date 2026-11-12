//! (ironspec 17:00) The one-chunk top tier's bed stop, the minimal through-line (paper v2,
//! https://artifacts.internalmeta.com/a/2132081997402168): an entry from the switch at the top of
//! the departure climb, an always-on activator that drops every arrival, a curve, two braked
//! powered rails sloping down south (the arrival stops on them), the pickup cell beside the bed's
//! head, a flat beside its foot, a flat under a top-half trapdoor, the exit (R's detector on its
//! slope). The rider lands on a ledge, walks off it into the pit and lies down; woken through the
//! hatch at F, it can stand only on the track cells the released cart sweeps.
//!
//! The unit benches, before any wiring (the bench drives F, the hatch and the release): where the
//! eject lands, the napper's path into the bed, the stand-up cell after the wake, the braked stop,
//! the release's speed over the swept cells and the pickup.

use crate::layouts::Builder;
use anyhow::{bail, Result};
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;
use redsim::world::World;

fn put(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    if let Some(old) = b.d.blocks.get(&c) {
        bail!("bed tier: {s} at {c:?} meets {old}");
    }
    b.set(c[0], c[1], c[2], s);
    Ok(())
}

/// The fold (onechunk's C′ envelope, loop frame minus (0,50,0): x -6..0, z -1..9, the bed's head
/// at (-3,0,4), the divert in from the east heading west, the return out east at z 6). The
/// arrival runs west along z -1: the entry (a booster), the activator, the park (one braked slope
/// rail), a plain slope, a capped flat, a curve south, four capped flats down x -2, the pickup
/// cell beside the bed's head, a capped flat, a curve east, two flats, the exit slope, the rider
/// check, the hold, R. The released cart slows on its 0.96 empty drag over about 6.2 blocks to
/// the pickup cell (a cart over 0.2 b/t stops dead against a standing villager).
pub const ENTRY: [i32; 3] = [3, 2, -4];
/// The activator, always on (its redstone block under it); its south side is the landing L.
pub const ACTIVATOR: [i32; 3] = [2, 2, -4];
/// The park: one braked powered rail on the slope down west; its support is the release's cell
/// (the bench sets a redstone block there).
pub const PARK: [[i32; 3]; 2] = [[1, 1, -4], [0, 0, -4]];
pub const PARK_CTRL: [[i32; 3]; 2] = [[1, 0, -4], [0, -1, -4]];
/// The one cell a woken napper can stand on: the pickup cell.
pub const SWEPT: [[i32; 3]; 1] = [[-2, 0, 4]];
pub const BED_HEAD: [i32; 3] = [-3, 0, 4];
/// The hatch: two trapdoors in the zombie box's one opening west of the bed's head (bottom half
/// (-4,0,4), top half (-4,1,4)); open (their controls powered) their plates cut the eye lines;
/// shut they pass.
pub const HATCH: [i32; 3] = [-4, 0, 4];
pub const HATCH_HI: [i32; 3] = [-4, 1, 4];
pub const HATCH_CTRL: [i32; 3] = [-4, 0, 3];
pub const HATCH_CTRL_HI: [i32; 3] = [-4, 2, 4];
/// The rider check's hold: a braked powered rail, powered only while the check's string sees a rider.
pub const HOLD: [i32; 3] = [3, -1, 6];
/// R's detector: past the hold (a held cart never reads as the tier cleared).
pub const EXIT_DET: [i32; 3] = [4, -1, 6];


/// The napper's legs on C′ (ironspec 19:29; loop frame minus (0,50,0)): the divert from the switch
/// flat (9,-14,-1) (loop (9,36,-1): a T on the tail's x 9 run, powered = its north-west curve, the
/// carts from G1 turned west; unpowered = its south-west curve, a cart from the north passes south)
/// climbing a square spiral (x 4..8, z -5..-1) to the tier's entry (3,2,-4), boosted on every
/// straight but the last four slopes (an unboosted climb brings the cart from the rails' 0.4 to
/// about 0.3, the tier's arrival cap); the return from R (4,-1,6) descending a 3 x 3 helix
/// (x 5..7, z 6..8, four levels a lap, glass supports: a rider's eye never in a solid block) to
/// (7,-16,8) and the re-entry's T on the tail's z 9 flats (7,-16,9). The legs share no cell.
/// Each cell: (pos, rail state, powered rail).
pub fn trip_cells() -> (Vec<([i32; 3], &'static str, bool)>, Vec<([i32; 3], &'static str)>) {
    let divert: Vec<([i32; 3], &'static str, bool)> = vec![
        ([8, -14, -1], "ascending_west", true),
        ([7, -13, -1], "ascending_west", false),
        ([6, -12, -1], "ascending_west", true),
        ([5, -11, -1], "ascending_west", false),
        ([4, -10, -1], "north_east", false),
        ([4, -10, -2], "ascending_north", true),
        ([4, -9, -3], "ascending_north", false),
        ([4, -8, -4], "ascending_north", true),
        ([4, -7, -5], "south_east", false),
        ([5, -7, -5], "ascending_east", true),
        ([6, -6, -5], "ascending_east", false),
        ([7, -5, -5], "ascending_east", true),
        ([8, -4, -5], "south_west", false),
        ([8, -4, -4], "ascending_south", true),
        ([8, -3, -3], "ascending_south", false),
        ([8, -2, -2], "north_west", false),
        ([7, -2, -2], "ascending_west", false),
        ([6, -1, -2], "ascending_west", false),
        ([5, 0, -2], "ascending_west", false),
        ([4, 1, -2], "north_east", false),
        ([4, 1, -3], "ascending_north", false),
        ([4, 2, -4], "south_west", false),
    ];
    let mut ret: Vec<([i32; 3], &'static str)> = vec![([5, -1, 6], "east_west")];
    // the helix: per lap east (slope down), the corner south, south (slope), the corner west,
    // west (slope), the corner north, north (slope), the corner east
    let mut y = -1;
    for lap in 0..4 {
        ret.push(([6, y - 1, 6], "ascending_west"));
        ret.push(([7, y - 1, 6], "south_west"));
        ret.push(([7, y - 2, 7], "ascending_north"));
        if lap == 3 {
            ret.push(([7, y - 3, 8], "ascending_north"));
            break;
        }
        ret.push(([7, y - 2, 8], "north_west"));
        ret.push(([6, y - 3, 8], "ascending_east"));
        ret.push(([5, y - 3, 8], "north_east"));
        ret.push(([5, y - 4, 7], "ascending_south"));
        ret.push(([5, y - 4, 6], "south_east"));
        y -= 4;
    }
    (divert, ret)
}

/// The re-entry's waiting cell (the T on the tail's z 9 flats; on the bench a braked rail).
pub const WAIT: [i32; 3] = [7, -16, 9];
/// The divert's first cell past the switch (the bench's cart starts on it, heading west).
pub const DIVERT0: [i32; 3] = [8, -14, -1];

fn trip_legs(b: &mut Builder) -> Result<()> {
    let (divert, ret) = trip_cells();
    // (one redstone block under one powered rail of each boosted straight: it powers the rail
    // and the powered rails joined to it; the curves break the chain)
    let boosters: [[i32; 3]; 7] = [[8, -14, -1], [6, -12, -1], [4, -10, -2], [4, -8, -4], [5, -7, -5], [7, -5, -5], [8, -4, -4]];
    // (MCTAS_TRIP_POWERED=i,j,..: the divert cells (by index) laid as powered rails, each with a
    // redstone block under it, in place of the default: the tuning knob for the arrival speed)
    let over: Option<Vec<usize>> = std::env::var("MCTAS_TRIP_POWERED").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    for (k, (c, shape, powered)) in divert.iter().enumerate() {
        let (powered, block) = match &over {
            Some(v) => (v.contains(&k), v.contains(&k)),
            None => (*powered, boosters.contains(c)),
        };
        if powered {
            put(b, *c, &format!("powered_rail[powered=true,shape={shape},waterlogged=false]"))?;
        } else {
            put(b, *c, &format!("rail[shape={shape},waterlogged=false]"))?;
        }
        let under = [c[0], c[1] - 1, c[2]];
        put(b, under, if block { "redstone_block" } else { "glass" })?;
    }
    for (c, shape) in &ret {
        put(b, *c, &format!("rail[shape={shape},waterlogged=false]"))?;
        put(b, [c[0], c[1] - 1, c[2]], "glass")?;
    }
    put(b, WAIT, "powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
    put(b, [WAIT[0], WAIT[1] - 1, WAIT[2]], "stone")?;
    put(b, [WAIT[0], WAIT[1], WAIT[2] + 1], "stone")?;
    // (the switch flat itself, the cart's start behind it)
    put(b, [9, -14, -1], "rail[shape=north_west,waterlogged=false]")?;
    put(b, [9, -15, -1], "stone")?;
    Ok(())
}
/// The design with the napper's legs on C′ (`trip`: the divert spiral and the return helix in
/// place of the bench's run-out).
pub fn design_with(bench: bool, trip: bool) -> Result<String> {
    let mut b = Builder::new(if trip { "bed-tier-fold-trip" } else if bench { "bed-tier-fold-bench" } else { "bed-tier-fold" });
    let rail = |s: &str| format!("rail[shape={s},waterlogged=false]");
    let cap = "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]";
    let wall = |b: &mut Builder, c: [i32; 3]| -> Result<()> {
        if !b.d.blocks.contains_key(&c) {
            put(b, c, "glass")?;
        }
        Ok(())
    };
    // the arrival, west along z -1: the entry a booster (always on), the activator (always on),
    // both capped (top-half trapdoors: a rider passes, nobody stands there)
    put(&mut b, ENTRY, "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [ENTRY[0], 1, ENTRY[2]], "redstone_block")?;
    put(&mut b, ACTIVATOR, "activator_rail[powered=true,shape=east_west,waterlogged=false]")?;
    // (the activator's redstone block on its north side, not under it: under it, it would sit
    // beside the park rail and power it; on the north it is also the dismount's first choice,
    // blocked)
    put(&mut b, [ACTIVATOR[0], 1, ACTIVATOR[2]], "stone")?;
    put(&mut b, [ACTIVATOR[0], 2, ACTIVATOR[2] - 1], "redstone_block")?;
    put(&mut b, [ENTRY[0], 3, ENTRY[2]], cap)?;
    put(&mut b, [ACTIVATOR[0], 3, ACTIVATOR[2]], cap)?;
    // the park, two braked powered rails down the slope (every arrival stops on the first; the
    // release powers both: the full second cell's boost narrows the spread of release speeds that
    // the first cell's stop position leaves), a capped flat, the curve south
    for (p, c) in PARK.iter().zip(PARK_CTRL.iter()) {
        put(&mut b, *p, "powered_rail[powered=false,shape=ascending_east,waterlogged=false]")?;
        put(&mut b, *c, "stone")?;
    }
    let za = ENTRY[2];
    put(&mut b, [-1, 0, za], &rail("east_west"))?;
    put(&mut b, [-1, -1, za], "glass")?;
    put(&mut b, [-1, 1, za], cap)?;
    put(&mut b, [-2, 0, za], &rail("south_east"))?;
    put(&mut b, [-2, -1, za], "glass")?;
    put(&mut b, [-2, 1, za], cap)?;
    // (glass two cells over the park: no villager stands on the parked cart's top)
    put(&mut b, [1, 3, za], "glass")?;
    // five capped flats down x -2, the pickup cell (the bed's head west of it), a capped booster,
    // the curve east, two capped flats east along z 6
    for z in za + 1..=5 {
        // (the cell after the pickup a booster, always on: an empty cart, slowing past the pickup
        // cell on its 0.96 drag, would stall on the flats before the hold; its redstone block under
        // it touches no other rail)
        if z == SWEPT[0][2] + 1 {
            put(&mut b, [-2, 0, z], "powered_rail[powered=true,shape=north_south,waterlogged=false]")?;
            put(&mut b, [-2, -1, z], "redstone_block")?;
        } else {
            put(&mut b, [-2, 0, z], &rail("north_south"))?;
            put(&mut b, [-2, -1, z], "glass")?;
        }
        if z != SWEPT[0][2] {
            put(&mut b, [-2, 1, z], cap)?;
        }
    }
    put(&mut b, [-2, 0, 6], &rail("north_east"))?;
    put(&mut b, [-2, -1, 6], "glass")?;
    put(&mut b, [-2, 1, 6], cap)?;
    for x in -1..=0 {
        put(&mut b, [x, 0, 6], &rail("east_west"))?;
        put(&mut b, [x, -1, 6], "glass")?;
        put(&mut b, [x, 1, 6], cap)?;
    }
    // the exit: a plain slope down east, a flat, the hold under the rider check (a tripwire at
    // rider height across the hold's own cell, hooks (3,0,5) and (3,0,7): a seated rider's box
    // crosses its band from the moment the cart nears the hold until it has left it, an empty
    // cart's never does; the string over the hold itself, not a cell before it: the string's
    // 10-gt re-check could drop the power while a fast cart is still on the hold), powered from
    // the south hook's block by dust (3,-1,8), (3,-1,7) pointing north into it, then R
    put(&mut b, [1, -1, 6], &rail("ascending_west"))?;
    put(&mut b, [1, -2, 6], "stone")?;
    put(&mut b, [2, -1, 6], &rail("east_west"))?;
    put(&mut b, [2, -2, 6], "glass")?;
    put(&mut b, [3, 0, 6], "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?;
    put(&mut b, [3, 0, 5], "tripwire_hook[attached=true,facing=south,powered=false]")?;
    put(&mut b, [3, 0, 4], "stone")?;
    put(&mut b, [3, 0, 7], "tripwire_hook[attached=true,facing=north,powered=false]")?;
    put(&mut b, [3, 0, 8], "stone")?;
    let dust = "redstone_wire[east=none,north=none,power=0,south=none,west=none]";
    for c in [[3, -1, 8], [3, -1, 7]] {
        put(&mut b, [c[0], c[1] - 1, c[2]], "stone")?;
        put(&mut b, c, dust)?;
    }
    put(&mut b, HOLD, "powered_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(&mut b, [HOLD[0], -2, HOLD[2]], "glass")?;
    put(&mut b, EXIT_DET, "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(&mut b, [EXIT_DET[0], -2, EXIT_DET[2]], "stone")?;
    // the bed, its head beside the pickup cell, its foot north, roofed by two top-half trapdoors
    // (0.25 of headroom over the bed's top: nobody stands on it; a standing napper's eye line runs
    // under them)
    put(&mut b, BED_HEAD, "red_bed[facing=south,occupied=false,part=head]")?;
    put(&mut b, [-3, 0, 3], "red_bed[facing=south,occupied=false,part=foot]")?;
    put(&mut b, [-3, -1, 3], "glass")?;
    put(&mut b, [-3, -1, 4], "glass")?;
    put(&mut b, [-3, 1, 3], cap)?;
    put(&mut b, [-3, 1, 4], cap)?;
    // the hatch west of the bed's head and the zombie box behind it (one cell, x -5; the west wall
    // on x -6): open (powered by the redstone blocks (-4,0,3) and (-4,2,4)) the plates stand across
    // the cells' east sides and cut every line; shut (F) the sleeper's eye line and a standing
    // napper's (under the bed's roof) pass. The zombie (persistent) in a boat at (-5.0, 0, 4.5),
    // glass all round, leaves over it
    put(&mut b, HATCH, "iron_trapdoor[facing=west,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_HI, "iron_trapdoor[facing=west,half=top,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_CTRL, "redstone_block")?;
    put(&mut b, HATCH_CTRL_HI, "redstone_block")?;
    put(&mut b, [-4, -1, 4], "glass")?;
    put(&mut b, [-5, -1, 4], "glass")?;
    put(&mut b, [-5, 2, 4], "glass")?;
    for y in 0..=1 {
        put(&mut b, [-5, y, 3], "glass")?;
        put(&mut b, [-5, y, 5], "glass")?;
        put(&mut b, [-6, y, 4], "glass")?;
        put(&mut b, [-4, y, 5], "glass")?;
    }
    put(&mut b, [-4, 1, 3], "glass")?;
    put(&mut b, [-5, 3, 4], "oak_leaves[distance=1,persistent=true,waterlogged=false]")?;
    b.entity("oak_boat", [-5.0, 0.0, 4.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    // the pit: glass north of the bed's foot, south of its head, east of the flats and the pickup
    // cell (x -1, z 0..5: the ledge's floor at y 1)
    for y in 0..=1 {
        put(&mut b, [-3, y, 2], "glass")?;
        put(&mut b, [-3, y, 5], "glass")?;
        for z in 0..=5 {
            put(&mut b, [-1, y, z], "glass")?;
        }
    }
    // the landing L (2,2,0) south of the activator (its north side the activator's redstone
    // block: the dismount's first choice, blocked), the ledge: south to (2,2,1), west along z 1 to
    // (-1,2,1), south down x -1 to (-1,2,3), west over the flat's cap (-2,2,3) onto the bed's roof
    // (-3,2,3), (-3,2,4): there the bed's head is within SleepInBed's 2 (a villager off a rail
    // cannot path onto one, so the walk ends there; the open column over the pickup cell west of
    // (-2,2,3) is a way in too). Walls two high round the ledge (the
    // parked cart's top and the caps' tops lead nowhere)
    put(&mut b, [2, 1, -3], "stone")?;
    put(&mut b, [2, 1, -2], "stone")?;
    put(&mut b, [2, 1, -1], "stone")?;
    put(&mut b, [2, 1, 0], "stone")?;
    put(&mut b, [1, 1, 0], "stone")?;
    put(&mut b, [0, 1, 0], "stone")?;
    // (the pickup cell's column (-2,1..2,4) stays open: a napper falling in stands on a rail and so
    // can path to the bed; a napper that lost its HOME re-claims the bed that way)
    let ledge: [[i32; 3]; 12] = [[2, 2, -3], [2, 2, -2], [2, 2, -1], [2, 2, 0], [1, 2, 0], [0, 2, 0], [-1, 2, 0], [-1, 2, 1], [-1, 2, 2], [-1, 2, 3], [-2, 2, 3], [-3, 2, 3]];
    let ledge_end = [-3, 2, 4];
    for c in ledge.iter().chain(std::iter::once(&ledge_end)) {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [c[0] + dx, 2, c[2] + dz];
            if ledge.contains(&n) || n == ledge_end || n == [-2, 2, 4] || n == [ACTIVATOR[0], 2, ACTIVATOR[2]] {
                continue;
            }
            wall(&mut b, n)?;
            wall(&mut b, [n[0], 3, n[2]])?;
        }
    }
    if trip {
        trip_legs(&mut b)?;
    } else if bench {
        // the run-out past R and a stop
        for x in EXIT_DET[0] + 1..=EXIT_DET[0] + 6 {
            put(&mut b, [x, -1, 6], &rail("east_west"))?;
            put(&mut b, [x, -2, 6], "glass")?;
        }
        put(&mut b, [EXIT_DET[0] + 7, -1, 6], "glass")?;
        // (the visitor is spawned by the bench at the entry at the visit's hour, HOME = the bed)
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    Ok(b.text(&[
        "The one-chunk top tier's bed stop folded into C′'s envelope (loop frame minus (0,50,0); the bed's head at (-3,0,4)): a booster entry and an always-on activator heading west, two braked park rails down a slope, a capped flat, a curve south, seven capped flats, the pickup cell beside the bed, a capped booster, a curve east, two capped flats, the exit slope, a flat, the hold under the rider check, R.",
        "The rider lands on the ledge south of the activator, walks round onto the bed's roof and sleeps; woken through the hatch it stands only on the pickup cell and keeps seeing the zombie there; the released cart reaches it under 0.2 b/t and seats it; a riderless cart is held before R.",
    ]))
}

/// The design: the bed tier alone, or (`bench`) with a run-out past the exit.
pub fn design(bench: bool) -> Result<String> {
    design_with(bench, false)
}

/// One visit's unit-bench row.
#[derive(Debug, Default, Clone)]
pub struct Row {
    pub seed: u64,
    pub speed: f64,
    /// The rider's first standing cell after the eject (gt after the entry).
    pub landed: Option<([i32; 3], i64)>,
    /// Where the arriving cart stopped (its centre) and when.
    pub parked: Option<([f64; 3], i64)>,
    pub slept_at: Option<i64>,
    pub woke_at: Option<i64>,
    /// The napper's cell on its first awake tick after the nap.
    pub stood: Option<[i32; 3]>,
    /// Every cell the awake napper stood on between the wake and the pickup.
    pub cells_after_wake: Vec<[i32; 3]>,
    /// The release tick, and the cart's horizontal speed as it crossed each swept cell's centre.
    pub release_at: Option<i64>,
    pub swept_speed: Vec<([i32; 3], f64)>,
    /// The pickup: the cell the napper stood on and the tick.
    pub pickup: Option<([i32; 3], i64)>,
    /// The cart's horizontal speed in the tick it seated the napper.
    pub pickup_speed: Option<f64>,
    pub out_at: Option<i64>,
    /// The cart over R's detector (the tier clear), gt after the entry.
    pub r_at: Option<i64>,
    /// The rider check held the cart on HOLD (still there 40 gt: the alarm), gt after the entry.
    pub held: Option<i64>,
    /// (the trip) The cart onto the tier's entry, gt after the start.
    pub entry_at: Option<i64>,
    pub entry_speed: Option<f64>,
    /// The entry rider check: the cart arrived with a rider.
    pub entry_ridden: Option<bool>,
    /// The hold released a riderless cart: it arrived empty, or (a stranded napper) RIDER_ALARM
    /// was clear and is now set.
    pub released_empty: Option<&'static str>,
    /// Whether the cart left with the napper seated.
    pub seated_out: bool,
    pub problems: Vec<String>,
}

/// The bench's fault and control rows.
#[derive(Debug, Default, Clone)]
pub struct Faults {
    /// The cart sent empty (no villager): the rider check's positive control.
    pub empty: bool,
    /// The villager arrives with no HOME memory (a napper that lost its home: it never walks to
    /// the bed).
    pub no_home: bool,
    /// The waker hidden again once the napper is awake (both hatches back open-plated): the
    /// refuge's positive control (the napper should go back to bed).
    pub hide_after_wake: bool,
    /// A level-32 freeze (the tier's entities stop ticking) from this gt after the entry, for this
    /// many gt.
    pub freeze: Option<(i64, i64)>,
    /// The trip: the cart starts on the switch flat heading west up the divert, and the visit ends
    /// at the re-entry's waiting cell past the return.
    pub trip: bool,
    /// RIDER_ALARM already set (a stranded napper's empty cart released earlier): a second
    /// riderless departure is held.
    pub alarm: bool,
}

fn block(s: &str) -> Block {
    Block::parse(&format!("minecraft:{s}")).expect("a known block")
}

/// One visit: the visitor's cart onto the entry at `speed` (+x) at night (or by day with `day`),
/// F at `f_at` gt after the entry (the hatch's control off: the zombie seen), the release
/// `release_after` gt after F (the park's supports to redstone blocks), until the cart passes
/// the run-out's z 8 or 1200 gt.
#[allow(clippy::too_many_arguments)]
pub fn bench(spec: &FarmSpec, seed: u64, speed: f64, f_at_in: i64, release_after: i64, day: bool, on_stand: bool, after_sleep: Option<i64>, f: &Faults) -> Result<Row, String> {
    // (`after_sleep`: F comes that many gt after the lie-down (the bed's own evidence), or at
    // `f_at_in` at the latest (the give-up))
    let mut f_at = f_at_in;
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, 3 * 24000, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let mut row = Row { seed, speed, ..Default::default() };
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    // (wait for the visit's hour: night 13,800 + seed spread, or day 2,000)
    let want = if day { 2000 } else { 13_800 } + (seed as i64 * 37) % 400;
    let mut guard = 0;
    while run.world.day_time().rem_euclid(24000) != want {
        run.tick();
        guard += 1;
        if guard > 30_000 {
            return Err("never reached the visit's hour".into());
        }
    }
    // the visitor at the entry, HOME = the bed (as the game loads the memory)
    let h = BED_HEAD;
    let nbt = if f.empty {
        "{Rotation:[0f,0f]}".to_string()
    } else if f.no_home {
        "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}".to_string()
    } else {
        format!("{{Rotation:[0f,0f],Passengers:[{{id:\"minecraft:villager\",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}", h[0], h[1], h[2])
    };
    let start = if f.trip { [9.5, -14.0 + 0.0625, -0.5] } else { [ENTRY[0] as f64 + 0.5, ENTRY[1] as f64 + 0.0625, ENTRY[2] as f64 + 0.5] };
    let cart_id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&nbt))?;
    let vill = run.world.entities.list.iter().find(|e| e.id == cart_id).and_then(|e| e.passengers.first().copied());
    if vill.is_none() && !f.empty {
        return Err("no rider".into());
    }
    run.own.push(cart_id);
    run.own.extend(vill);
    // the entry
    {
        let w = &mut run.world;
        let i = idx(w, cart_id).ok_or("cart gone")?;
        w.entities.list[i].pos = Vec3::new(start[0], start[1], start[2]);
        w.entities.list[i].delta_movement = Vec3::new(-speed, 0.0, 0.0);
        let ps = w.entities.list[i].passengers.clone();
        for pid in ps {
            if let Some(j) = idx(w, pid) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    }
    let set = |w: &mut World, c: [i32; 3], st: &str| w.set_block(BlockPos::new(c[0], c[1], c[2]), block(st), redsim::world::flags::ALL);
    let mut was_asleep = false;
    let mut last_cart = [0f64; 3];
    let mut still = 0;
    let mut crossed: Vec<[i32; 3]> = Vec::new();
    let mut hidden = false;
    // (F, the give-up and the release count live time, the farm's cart-driven fast ring: a
    // level-32 freeze stops the ring with the carts, so those delays pause through it)
    let mut live = 0i64;
    let mut slept_live: Option<i64> = None;
    let mut f_fired = false;
    for t in 1..=1200i64 {
        if let Some((at, len)) = f.freeze {
            if t == at {
                run.world.entity_chunks = Some([100, 100, 101, 101]);
            }
            if t == at + len {
                run.world.entity_chunks = None;
            }
        }
        let frozen = f.freeze.is_some_and(|(a, l)| t >= a && t < a + l);
        if !frozen {
            live += 1;
        }
        if let Some(d) = after_sleep
            && let Some(s) = slept_live
            && s + d < f_at
            && live <= s + d
        {
            f_at = s + d;
        }
        // (day-1's final rule: the entry rider check saw no rider, so the give-up comes at once:
        // F as soon as the cart is parked)
        if row.entry_ridden == Some(false) && row.parked.is_some() && !f_fired && live < f_at {
            f_at = live;
        }
        if !frozen && !f_fired && live == f_at {
            f_fired = true;
            set(&mut run.world, HATCH_CTRL, "stone");
            set(&mut run.world, HATCH_CTRL_HI, "stone");
        }
        if f.hide_after_wake && !hidden && row.woke_at.is_some() {
            set(&mut run.world, HATCH_CTRL, "redstone_block");
            set(&mut run.world, HATCH_CTRL_HI, "redstone_block");
            hidden = true;
        }
        let stand_now = on_stand && f_fired && row.release_at.is_none() && vill.and_then(|id| idx(&run.world, id)).is_some_and(|vi| { let v = &run.world.entities.list[vi]; let c = [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32]; v.vehicle.is_none() && redsim::entity::mob::mob(&run.world, vi).sleeping_pos.is_none() && SWEPT.contains(&c) });
        if row.release_at.is_none() && !frozen && (stand_now || live == f_at + release_after) {
            for c in PARK_CTRL {
                set(&mut run.world, c, "redstone_block");
            }
            row.release_at = Some(t);
        }
        let before = idx(&run.world, cart_id).map(|i| run.world.entities.list[i].pos);
        run.tick();
        let w = &run.world;
        let Some(ci) = idx(w, cart_id) else {
            row.problems.push(format!("t {t}: the cart is gone"));
            break;
        };
        let c = &w.entities.list[ci];
        let vi = vill.and_then(|id| idx(w, id));
        if vill.is_some() && vi.is_none() {
            row.problems.push(format!("t {t}: the villager is gone"));
            break;
        }
        let mut riding = false;
        if let Some(vi) = vi {
            let v = &w.entities.list[vi];
            let vcell = [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32];
            riding = v.vehicle.is_some();
            let asleep = redsim::entity::mob::mob(w, vi).sleeping_pos.is_some();
            if !riding && row.landed.is_none() && v.on_ground {
                row.landed = Some((vcell, t));
            }
            if asleep && row.slept_at.is_none() {
                row.slept_at = Some(t);
                slept_live = Some(live);
            }
            if was_asleep && !asleep {
                row.woke_at = Some(t);
                row.stood = Some(vcell);
            }
            if row.woke_at.is_some() && !riding && row.pickup.is_none() && !row.cells_after_wake.contains(&vcell) {
                row.cells_after_wake.push(vcell);
            }
            was_asleep = asleep;
            if std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "1") && (t % 5 == 0 || std::env::var("MCTAS_BT_TRACE_FROM").ok().and_then(|s| s.parse::<i64>().ok()).is_some_and(|f| t >= f && t <= f + 15)) {
                use redsim::entity::mob::ai::brain;
                let b = brain::brain(w, vi);
                let act = brain::has_exact(w, vi).then(|| {
                    let x = brain::ex(w, vi);
                    format!("{:?} cant_reach {:?}", x.active, x.mem.cant_reach_since)
                });
                eprintln!("t {t}: villager {:.2} {:.2} {:.2} riding {riding} asleep {asleep} act {act:?} home {:?} hostile {:?} | cart {:.2} {:.2} {:.2} v {:.3} {:.3} {:.3}", v.pos.x, v.pos.y, v.pos.z, b.home, b.nearest_hostile, c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z);
            }
            if riding && row.release_at.is_some() && row.pickup.is_none() {
                row.pickup = Some((row.cells_after_wake.last().copied().unwrap_or(vcell), t));
                row.pickup_speed = before.map(|bp| ((c.pos.x - bp.x).powi(2) + (c.pos.z - bp.z).powi(2)).sqrt());
            }
            if row.release_at.is_none() && row.landed.is_some() && riding && t > 5 {
                row.problems.push(format!("t {t}: the napper back in a cart before the release (at {vcell:?})"));
                break;
            }
        }
        // the cart: parked (still for 10 gt before the release), its speed over the swept cells
        if vi.is_none() && std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "1") && t % 5 == 0 {
            eprintln!("t {t}: (no villager) | cart {:.2} {:.2} {:.2} v {:.3} {:.3} {:.3}", c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z);
        }
        let cp = [c.pos.x, c.pos.y, c.pos.z];
        let moved = (cp[0] - last_cart[0]).abs() + (cp[2] - last_cart[2]).abs() + (cp[1] - last_cart[1]).abs();
        still = if moved < 1e-4 { still + 1 } else { 0 };
        if still == 10 && row.parked.is_none() && row.release_at.is_none() {
            row.parked = Some((cp, t));
        }
        last_cart = cp;
        if let Some(bp) = before
            && row.release_at.is_some()
        {
            for s in SWEPT {
                let zc = s[2] as f64 + 0.5;
                if (bp.z < zc) != (c.pos.z < zc) && (c.pos.x - (s[0] as f64 + 0.5)).abs() < 0.3 && !crossed.contains(&s) {
                    crossed.push(s);
                    let hv = c.delta_movement.x.hypot(c.delta_movement.z);
                    row.swept_speed.push((s, hv));
                }
            }
        }
        // (R and out: the exit runs east along R's row)
        let on_exit_row = (c.pos.z - (EXIT_DET[2] as f64 + 0.5)).abs() < 0.3;
        if on_exit_row && c.pos.x > EXIT_DET[0] as f64 + 0.5 && row.r_at.is_none() {
            row.r_at = Some(t);
        }
        // the entry rider check (day-1's final rule; its string and latch emulated): at the entry,
        // with a rider or not
        let at_entry = (c.pos.z - (ENTRY[2] as f64 + 0.5)).abs() < 0.3 && c.pos.x < ENTRY[0] as f64 + 0.9 && c.pos.y > 1.5;
        if row.entry_ridden.is_none() && (at_entry || (!f.trip && t == 1)) {
            row.entry_ridden = Some(riding);
        }
        // the hold's other input (emulated: a redstone block beside the hold while the cart is on
        // the flat before it): a riderless cart is released when it arrived empty, or, a stranded
        // napper's, when RIDER_ALARM is clear (which it then sets); else the hold holds it
        let ccell = [c.pos.x.floor() as i32, (c.pos.y + 0.01).floor() as i32, c.pos.z.floor() as i32];
        if row.release_at.is_some() && !riding && ccell == [HOLD[0] - 1, HOLD[1], HOLD[2]] && row.released_empty.is_none() {
            let why = if row.entry_ridden == Some(false) {
                Some("arrived empty")
            } else if !f.alarm {
                Some("stranded napper, RIDER_ALARM was clear (now set)")
            } else {
                None
            };
            if let Some(w) = why {
                row.released_empty = Some(w);
                set(&mut run.world, [HOLD[0], HOLD[1], HOLD[2] - 1], "redstone_block");
            }
        }
        if row.released_empty.is_some() && row.r_at.is_some() {
            set(&mut run.world, [HOLD[0], HOLD[1], HOLD[2] - 1], "air");
        }
        let w = &run.world;
        let Some(ci) = idx(w, cart_id) else {
            row.problems.push(format!("t {t}: the cart is gone"));
            break;
        };
        let c = &w.entities.list[ci];
        // the rider check's hold: the cart still on HOLD for 40 gt after the release is the alarm
        if row.release_at.is_some() && still >= 40 && ccell == HOLD {
            row.held = Some(t - 40);
            break;
        }
        if f.trip && row.entry_at.is_none() && (c.pos.z - (ENTRY[2] as f64 + 0.5)).abs() < 0.3 && c.pos.x < ENTRY[0] as f64 + 0.5 && c.pos.y > 1.5 {
            row.entry_at = Some(t);
            row.entry_speed = Some(c.delta_movement.x.hypot(c.delta_movement.z));
        }
        if f.trip {
            if ccell == WAIT && still >= 5 {
                row.out_at = Some(t - 5);
                row.seated_out = riding;
                break;
            }
        } else if on_exit_row && c.pos.x > EXIT_DET[0] as f64 + 1.5 {
            row.out_at = Some(t);
            row.seated_out = riding;
            break;
        }
    }
    if row.out_at.is_none() && row.held.is_none() {
        row.problems.push("the cart neither out nor held in 1200 gt".into());
    }
    if row.out_at.is_some() && !row.seated_out && row.released_empty.is_none() {
        row.problems.push("a riderless cart left the tier (not held)".into());
    }
    let expect_seated = !f.empty && !f.no_home && !f.hide_after_wake;
    if expect_seated && !row.seated_out {
        row.problems.push(if row.held.is_some() { "held at the exit: the napper not seated".into() } else { "the napper not seated".into() });
    }
    if !f.hide_after_wake {
        for c in &row.cells_after_wake {
            if !SWEPT.contains(c) {
                row.problems.push(format!("the woken napper stood on {c:?}, outside the swept cells"));
            }
        }
    }
    if !run.world.errors.is_empty() {
        row.problems.push(format!("world: {}", run.world.errors.join("; ")));
    }
    Ok(row)
}
