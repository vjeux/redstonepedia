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
/// The north T beside the bed's foot (unpowered for good: south-east) and its spur east (a
/// powered rail, dead end at the stone east of it).
pub const NORTH_T: [i32; 3] = [-2, 0, 3];
pub const NORTH_SPUR: [i32; 3] = [-1, 0, 3];
/// The south T (powered by the rider strings A and B, or by the latch they set and R's pulse
/// resets: north-west, the exit; unpowered: south-west, the spur) and its spur south (a powered
/// rail, dead end at the stone south of it). The latch's output is the T's support (the bench
/// emulates it there), so a rider frozen on the flat under both strings keeps the T powered.
pub const SOUTH_T: [i32; 3] = [1, -1, 6];
pub const SOUTH_SPUR: [i32; 3] = [1, -1, 7];
/// The kick clock: both spur rails and the slope powered KICK_LEN live gt in every KICK_PERIOD (the
/// tier chunk's ring edges: a freeze stops it with the carts).
pub const KICK_PERIOD: i64 = 80;
pub const KICK_LEN: i64 = 40;
/// The slope's window in the same period (from the same phase): 20 gt longer than the spurs'.
pub const SLOPE_LEN: i64 = 60;
/// The slope down from the curve to the south T: a braked powered rail, powered by the rider string
/// OR the kick clock (a second hold: a strand cart rests on it until a kick rolls it on into the spur;
/// a kick boosts the cart coming back west up it).
pub const SLOPE: [i32; 3] = [-1, -1, 6];
/// R's detector.
pub const EXIT_DET: [i32; 3] = [4, -1, 5];


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
        if lap == 3 {
            // (the last lap's end: the re-entry gate, two braked powered rails on its slopes (the
            // south slope (7,-15,7) and, past the corner west, the slope down west (6,-16,8)): the
            // returning cart, fast off the helix, stops on one of them and waits for its gap; the
            // gate's control powers both. Then the corner south into the T on the tail's z 9 flats
            // at (5,-16,9): west of 7b90c3f1's receiver exit detector (8,-16,9), nothing powered on
            // its faces)
            ret.push(([7, y - 2, 7], "gate_s"));
            ret.push(([7, y - 2, 8], "north_west"));
            ret.push(([6, y - 3, 8], "gate"));
            ret.push(([5, y - 3, 8], "south_east"));
            break;
        }
        ret.push(([7, y - 2, 7], "ascending_north"));
        ret.push(([7, y - 2, 8], "north_west"));
        ret.push(([6, y - 3, 8], "ascending_east"));
        ret.push(([5, y - 3, 8], "north_east"));
        ret.push(([5, y - 4, 7], "ascending_south"));
        ret.push(([5, y - 4, 6], "south_east"));
        y -= 4;
    }
    (divert, ret)
}

/// The re-entry gate (a braked powered rail on the slope down west, the returning cart waits on it
/// for its gap; the T past it on the tail's z 9 flats at (5,-16,9)).
pub const GATE: [i32; 3] = [6, -16, 8];
/// The T on the tail's z 9 flats (on the bench a stop past the gate).

pub const WAIT: [i32; 3] = [5, -16, 9];
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
        if *shape == "gate" || *shape == "gate_s" {
            put(b, *c, &format!("powered_rail[powered=false,shape={},waterlogged=false]", if *shape == "gate" { "ascending_east" } else { "ascending_north" }))?;
            put(b, [c[0], c[1] - 1, c[2]], "stone")?;
            continue;
        }
        put(b, *c, &format!("rail[shape={shape},waterlogged=false]"))?;
        put(b, [c[0], c[1] - 1, c[2]], "glass")?;
    }
    // (the T's cell on the bench: a plain rail and a stop past it)
    put(b, WAIT, "rail[shape=north_south,waterlogged=false]")?;
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
    // the arrival, west along z -4: the entry a booster (always on), the activator (always on),
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
    // six capped flats down x -2, the north T, the pickup cell (the bed's head west of it), a capped
    // flat, the curve east, the string flat, the south T (ironspec 21:58: two dead-end spurs and
    // periodic kicks, no bed-state memory).
    // - The north T (-2,0,3), beside the bed's foot: arms north (from the park), south (the pickup
    //   cell), east (the north spur (-1,0,3), a powered rail, dead end at the stone (0,0,3)).
    //   Unpowered for good, so south-east: a southbound cart passes straight to the pickup cell;
    //   any northbound cart (a cart pushed back off the napper, a retry that seated it) turns into
    //   the spur and stops against the stone; kicked, it comes back west and turns south.
    // - The south T (0,0,6): arms west (from the pickup cell by the curve and the string flat),
    //   north (the exit, (0,0,5) curving east to R) and south (the south spur (0,0,7), a powered
    //   rail, dead end at the stone (0,0,8)). Powered only by the hook (0,1,6) right over it, the
    //   east end of a rider-height string over the flat before it (-1,1,6): a rider is seen while
    //   its cart's centre is still 1.3 blocks short of the T (6+ gt at 0.2). Powered: north-west,
    //   a ridden cart turns north to the exit. Unpowered: south-west, an unridden cart (a napper
    //   asleep when it passed: a reload put it back to bed) turns south into the spur and stops
    //   against the stone, so R never sees a strand; kicked, it comes back north, turns west (the
    //   T unpowered: no rider), and runs back through the pickup cell into the north spur, seating
    //   the napper if it stands there; the north kick sends it back south, and the string decides.
    // - The kicks: a plain clock powers both spur rails a few gt in every period (the bench
    //   emulates it on their supports). A frozen cart misses a kick and takes the next.
    // (the forward speed: a 2-level hump before the north T, two slopes up, two raised capped
    // flats at y 2, two slopes down to the T: the descent gives back what the T's unconnected
    // pass costs (a cart passing straight through its south-east curve loses about 30%))
    let hump: [([i32; 3], &str); 6] = [
        ([-2, 0, -3], "ascending_south"),
        ([-2, 1, -2], "ascending_south"),
        ([-2, 2, -1], "north_south"),
        ([-2, 2, 0], "north_south"),
        ([-2, 1, 1], "ascending_north"),
        ([-2, 0, 2], "ascending_north"),
    ];
    // ((-2,2,2) above the lower slope's cap: a cap too, not the ledge's wall glass: a cart coming down
    // the upper slope reaches y 2.25 there; the napper on the ledge still has no headroom in it)
    put(&mut b, [-2, 2, 2], cap)?;
    for (c, shape) in hump {
        put(&mut b, c, &rail(shape))?;
        put(&mut b, [c[0], c[1] - 1, c[2]], "glass")?;
        put(&mut b, [c[0], c[1] + 1, c[2]], cap)?;
    }
    put(&mut b, NORTH_T, &rail("south_east"))?;
    put(&mut b, [NORTH_T[0], -1, NORTH_T[2]], "glass")?;
    put(&mut b, [NORTH_T[0], 1, NORTH_T[2]], cap)?;
    put(&mut b, NORTH_SPUR, "powered_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(&mut b, [NORTH_SPUR[0], -1, NORTH_SPUR[2]], "stone")?;
    put(&mut b, [NORTH_SPUR[0] + 1, 0, NORTH_SPUR[2]], "stone")?;
    for z in [SWEPT[0][2], SWEPT[0][2] + 1] {
        put(&mut b, [-2, 0, z], &rail("north_south"))?;
        put(&mut b, [-2, -1, z], "glass")?;
        if z != SWEPT[0][2] {
            put(&mut b, [-2, 1, z], cap)?;
        }
    }
    put(&mut b, [-2, 0, 6], &rail("north_east"))?;
    put(&mut b, [-2, -1, 6], "glass")?;
    put(&mut b, SLOPE, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
    put(&mut b, [-1, -2, 6], "stone")?;
    // (a flat between the slope and the T, so an empty cart on the slope never reaches the T's
    // cell)
    put(&mut b, [0, -1, 6], &rail("east_west"))?;
    put(&mut b, [0, -2, 6], "glass")?;
    // (the rider strings: A at y 1 over the curve, the slope, the flat and the T, hooks (-3,1,6)
    // and (2,1,6): a rider is seen while its cart's centre is still about 4 blocks short of the
    // T; B at y 0 over the T itself, hooks (1,0,5) and (1,0,7): at y -1 a seated rider's box tops
    // out at y 1.0, under A's band, so B holds the T while the cart is in it, and 10 gt more. A's
    // east hook powers the dust (2,0,6) below it, B's south hook the dust (2,0,7) beside it; the
    // dust powers (2,-1,6), beside the T: the T's only source. An empty cart's box tops out at
    // 0.76 on the slope and -0.24 on the flat and the T: under both bands)
    for x in -2..=1 {
        put(&mut b, [x, 1, 6], "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]")?;
    }
    put(&mut b, [-3, 1, 6], "tripwire_hook[attached=true,facing=east,powered=false]")?;
    put(&mut b, [-4, 1, 6], "stone")?;
    put(&mut b, [2, 1, 6], "tripwire_hook[attached=true,facing=west,powered=false]")?;
    put(&mut b, [3, 1, 6], "stone")?;
    put(&mut b, [SOUTH_T[0], 0, 6], "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?;
    put(&mut b, [SOUTH_T[0], 0, 5], "tripwire_hook[attached=true,facing=south,powered=false]")?;
    put(&mut b, [SOUTH_T[0], 0, 4], "stone")?;
    put(&mut b, [SOUTH_T[0], 0, 7], "tripwire_hook[attached=true,facing=north,powered=false]")?;
    put(&mut b, [SOUTH_T[0], 0, 8], "stone")?;
    for z in [6, 7] {
        put(&mut b, [SOUTH_T[0] + 1, 0, z], "redstone_wire[east=none,north=side,power=0,south=side,west=none]")?;
        put(&mut b, [SOUTH_T[0] + 1, -1, z], "stone")?;
    }
    put(&mut b, SOUTH_T, &rail("south_west"))?;
    // (the T's support: the latch's output (set by either string, reset by R's pulse; ~10 blocks,
    // emulated on the bench): the T stays powered from the first sight of a rider until its cart
    // has passed R, through any freeze, where a frozen rider can sit under both strings)
    put(&mut b, [SOUTH_T[0], SOUTH_T[1] - 1, SOUTH_T[2]], "stone")?;
    put(&mut b, SOUTH_SPUR, "powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
    put(&mut b, [SOUTH_SPUR[0], SOUTH_SPUR[1] - 1, SOUTH_SPUR[2]], "stone")?;
    put(&mut b, [SOUTH_SPUR[0], SOUTH_SPUR[1], SOUTH_SPUR[2] + 1], "stone")?;
    // the exit: north out of the T, the curve east, two flats, R
    put(&mut b, [SOUTH_T[0], -1, 5], &rail("south_east"))?;
    put(&mut b, [SOUTH_T[0], -2, 5], "glass")?;
    for x in SOUTH_T[0] + 1..=3 {
        put(&mut b, [x, -1, 5], &rail("east_west"))?;
        put(&mut b, [x, -2, 5], "glass")?;
    }
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
        wall(&mut b, [-3, y, 5])?;
        for z in 0..=5 {
            // ((-1,1,5) is the retry hold's north hook)
            wall(&mut b, [-1, y, z])?;
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
            put(&mut b, [x, -1, EXIT_DET[2]], &rail("east_west"))?;
            put(&mut b, [x, -2, EXIT_DET[2]], "glass")?;
        }
        put(&mut b, [EXIT_DET[0] + 7, -1, EXIT_DET[2]], "glass")?;
        // (the visitor is spawned by the bench at the entry at the visit's hour, HOME = the bed)
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    Ok(b.text(&[
        "The bed stop (local frame; under the station, loop = local + (0,-18,-1); the bed's head at (-3,0,4)): a booster entry and an always-on activator heading west, two braked park rails down a slope, a capped flat, a curve south, a capped 2-level hump, the north T (unpowered for good: its spur east, a dead end), the pickup cell beside the bed, a capped flat, a curve east, the slope down (a braked powered rail on the rider string or the kick clock), a flat, the south T (powered only by the rider strings and a latch they set and R resets: its spur south, a dead end), the curve north and east, two flats, R.",
        "The rider lands on the ledge south of the activator, walks round onto the bed's roof and sleeps; woken through the hatch it stands only on the pickup cell and keeps seeing the zombie there; the released cart reaches it under 0.2 b/t and seats it; the strings see the rider and the south T sends the cart north to R. A cart that finds the napper asleep (a reload put it back to bed) turns into the south spur; the kick clock (80 live gt, 40 on: the ring's edges, so a freeze stops it) sends it back through the pickup cell into the north spur and back again until the napper rides.",
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
    /// The retry hold: each time the cart came to rest on it unridden after the release, each
    /// roll-back off it, the cart's z speed at the pickup (north < 0: seated by a roll-back), the
    /// cart's least z after the release (under -2: back over the hump toward the park).
    pub holds: Vec<i64>,
    pub rollbacks: Vec<i64>,
    pub pickup_vz: Option<f64>,
    pub min_z: Option<f64>,
    /// The south T: its least lead (gt from powered to a ridden cart's centre entering it), a ridden
    /// cart entering it unpowered, flips with a cart's centre in its cell.
    pub min_lead: Option<i64>,
    pub ridden_t_unpowered: bool,
    pub t_flips_with_cart: i64,
    /// A ridden cart in the south spur (the string missed its rider).
    pub ridden_held: bool,
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
    /// A save and reload (the whole world unloaded) from this gt after the entry, for this many gt.
    pub reload: Option<(i64, i64)>,
    /// The trip: the cart starts on the switch flat heading west up the divert, and the visit ends
    /// at the re-entry's waiting cell past the return.
    pub trip: bool,
    /// The release on the bed observer's wake edge after F, this many live gt later (in place of
    /// F + release_after); the give-up `wake_give_up` live gt after F with no wake.
    pub after_wake: Option<i64>,
    pub wake_give_up: i64,
    /// The kick clock's phase (gt added to game time).
    pub kick_phase: i64,
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
    let mut cart_id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&nbt))?;
    let mut vill = run.world.entities.list.iter().find(|e| e.id == cart_id).and_then(|e| e.passengers.first().copied());
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
    let mut was_occ = false;
    let mut wake_release: Option<i64> = None;
    let mut kick_on = false;
    let mut slope_on = false;
    let mut latch = false;
    let mut latch_out = false;
    let mut t_powered_at: Option<i64> = None;
    let mut last_ccell = [i32::MIN; 3];
    for t in 1..=2400i64 {
        if let Some((at, len)) = f.freeze {
            if t == at {
                run.world.entity_chunks = Some([100, 100, 101, 101]);
            }
            if t == at + len {
                run.world.entity_chunks = None;
            }
        }
        // (a save and reload: the whole world unloaded, everything stops, entities back as new
        // objects in the save's order)
        if let Some((at, len)) = f.reload {
            if t == at {
                redsim::reload::unload_all(&mut run.world);
            }
            if t == at + len {
                redsim::reload::load_all_ordered(&mut run.world, redsim::reload::Order::Save);
                cart_id = run.world.renumbered.get(&cart_id).copied().unwrap_or(cart_id);
                vill = vill.map(|v| run.world.renumbered.get(&v).copied().unwrap_or(v));
            }
        }
        let away = f.reload.is_some_and(|(a, l)| t >= a && t < a + l);
        let frozen = away || f.freeze.is_some_and(|(a, l)| t >= a && t < a + l);
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
        // (the release on the bed observer's wake edge, ironspec 20:32 option 4: the bed's head
        // occupied -> free after F, then f.after_wake live gt; the give-up: f.wake_give_up live gt
        // after F with no wake)
        let occ_now = matches!(run.world.get(BlockPos::new(BED_HEAD[0], BED_HEAD[1], BED_HEAD[2])), Block::Bed { occupied: true, .. });
        if let Some(d) = f.after_wake {
            if f_fired && was_occ && !occ_now && !frozen && wake_release.is_none() {
                wake_release = Some(live + d);
            }
            if !occ_now || frozen {
            } else {
                // (asleep again before the delay ran out: the observer's lie-down edge cancels it)
                wake_release = None;
            }
        }
        if !frozen {
            was_occ = occ_now;
        }
        let wake_due = f.after_wake.is_some() && (wake_release.is_some_and(|w| live >= w) || (f_fired && live == f_at + f.wake_give_up));
        let timer_due = f.after_wake.is_none() && live == f_at + release_after;
        if row.release_at.is_none() && !frozen && (stand_now || timer_due || wake_due) {
            for c in PARK_CTRL {
                set(&mut run.world, c, "redstone_block");
            }
            row.release_at = Some(t);
        }
        // (the kick clock, counted in the tier chunk's ring edges (live time): a freeze stops it with
        // the carts, so a cart caught mid-kick is not braked by the spur at the thaw; both spur rails
        // powered KICK_LEN gt in every KICK_PERIOD, shifted by f.kick_phase; on their supports)
        if !away {
            let on = (live + f.kick_phase).rem_euclid(KICK_PERIOD) < KICK_LEN;
            if on != kick_on {
                for s in [NORTH_SPUR, SOUTH_SPUR] {
                    set(&mut run.world, [s[0], s[1] - 1, s[2]], if on { "redstone_block" } else { "stone" });
                }
                kick_on = on;
            }
        }
        // (the T's latch, emulated: set while either string's hook is powered, reset by R (the
        // cart past R's detector); its output on the T's support)
        if !away {
            let a_on = matches!(run.world.get(BlockPos::new(2, 1, 6)), Block::TripwireHook { powered: true, .. });
            let b_on = matches!(run.world.get(BlockPos::new(SOUTH_T[0], 0, 7)), Block::TripwireHook { powered: true, .. });
            if a_on || b_on {
                latch = true;
            }
            if latch && row.r_at.is_some() && !(a_on || b_on) {
                latch = false;
            }
            if latch != latch_out {
                set(&mut run.world, [SOUTH_T[0], SOUTH_T[1] - 1, SOUTH_T[2]], if latch { "redstone_block" } else { "stone" });
                latch_out = latch;
            }
        }
        // (the slope's power: the rider string (its east hook) OR the kick clock, emulated on its
        // support)
        if !away {
            let string_on = matches!(run.world.get(BlockPos::new(2, 1, 6)), Block::TripwireHook { powered: true, .. });
            // (the slope's own window outlasts the spurs' by SLOPE_LEN - KICK_LEN: a cart kicked
            // in the spurs' last tick still climbs it boosted)
            let slope_kick = (live + f.kick_phase).rem_euclid(KICK_PERIOD) < SLOPE_LEN;
            let on = slope_kick || string_on;
            if on != slope_on {
                set(&mut run.world, [SLOPE[0], SLOPE[1] - 1, SLOPE[2]], if on { "redstone_block" } else { "stone" });
                slope_on = on;
            }
        }
        let before = idx(&run.world, cart_id).map(|i| run.world.entities.list[i].pos);
        let t_before = run.world.get(BlockPos::new(SOUTH_T[0], SOUTH_T[1], SOUTH_T[2]));
        run.tick();
        let t_after = run.world.get(BlockPos::new(SOUTH_T[0], SOUTH_T[1], SOUTH_T[2]));
        let t_powered = matches!(t_after, Block::PlainRail { shape: redsim::block::RailShape::NorthWest });
        if t_powered && !matches!(t_before, Block::PlainRail { shape: redsim::block::RailShape::NorthWest }) {
            t_powered_at = Some(t);
        }
        if std::env::var("MCTAS_BT_CIRCUIT").is_ok_and(|x| x == "1") {
            let g = |c: [i32; 3]| format!("{:?}", run.world.get(BlockPos::new(c[0], c[1], c[2])));
            let short = |s: String| s.chars().filter(|ch| !ch.is_whitespace()).collect::<String>();
            let cp = idx(&run.world, cart_id).map(|i| { let e = &run.world.entities.list[i]; format!("{:.2},{:.2},{:.2} riders {}", e.pos.x, e.pos.y, e.pos.z, e.passengers.len()) });
            eprintln!("c {t}: south T {} | wire {} | hook {} | B {} | dust6 {} | dust7 {} | hookB {} | cart {:?}", short(g(SOUTH_T)), short(g([-1, 1, 6])), short(g([1, 1, 6])), short(g([0, 0, 6])), short(g([1, 0, 6])), short(g([1, 0, 7])), short(g([0, 0, 7])), cp);
        }
        if away {
            continue;
        }
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
            if row.woke_at.is_some() && !riding && !asleep && row.pickup.is_none() && !row.cells_after_wake.contains(&vcell) {
                row.cells_after_wake.push(vcell);
            }
            was_asleep = asleep;
            if std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "1") && (t % 5 == 0 || std::env::var("MCTAS_BT_TRACE_FROM").ok().and_then(|s| s.parse::<i64>().ok()).is_some_and(|f| t >= f && t <= f + 15)) {
                use redsim::entity::mob::ai::brain;
                let b = brain::brain(w, vi);
                let act = brain::has_exact(w, vi).then(|| {
                    let x = brain::ex(w, vi);
                    let path = x.mem.path.as_ref().map(|p| format!("next {} of {} {:?} reached {}", p.next_node_index, p.nodes.len(), p.nodes.iter().skip(p.next_node_index).take(3).map(|n| (n.x, n.y, n.z)).collect::<Vec<_>>(), p.reached));
                    format!("{:?} cant_reach {:?} wt {:?} path {:?}", x.active, x.mem.cant_reach_since, x.mem.walk_target.map(|t| t.target), path)
                });
                eprintln!("t {t}: villager {:.2} {:.2} {:.2} riding {riding} asleep {asleep} act {act:?} home {:?} hostile {:?} | cart {:.2} {:.2} {:.2} v {:.3} {:.3} {:.3}", v.pos.x, v.pos.y, v.pos.z, b.home, b.nearest_hostile, c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z);
            }
            if riding && row.release_at.is_some() && row.pickup.is_none() {
                row.pickup = Some((row.cells_after_wake.last().copied().unwrap_or(vcell), t));
                row.pickup_speed = Some(c.delta_movement.x.hypot(c.delta_movement.z));
                row.pickup_vz = before.map(|bp| c.pos.z - bp.z);
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
        let ccell = [c.pos.x.floor() as i32, (c.pos.y + 0.01).floor() as i32, c.pos.z.floor() as i32];
        // (the retry hold: at rest on it unridden after the release; a roll-back off it westward)
        // (the spurs and the south T: a cart at rest in the south spur unridden (a strand held), a
        // ridden cart in it (the string missed), a roll-back out of it, the T's lead over a ridden
        // cart's centre entering it, a flip of the T with a cart's centre in its cell)
        let in_south_spur = ccell == SOUTH_SPUR;
        if in_south_spur && riding {
            row.ridden_held = true;
        }
        if in_south_spur && still == 5 && !riding && row.release_at.is_some() {
            row.holds.push(t);
        }
        if last_ccell == SOUTH_SPUR && ccell == SOUTH_T {
            row.rollbacks.push(t);
        }
        if ccell == SOUTH_T && last_ccell != SOUTH_T && riding {
            if t_powered {
                let lead = t - t_powered_at.unwrap_or(t);
                row.min_lead = Some(row.min_lead.map_or(lead, |m: i64| m.min(lead)));
            } else {
                row.ridden_t_unpowered = true;
            }
        }
        if ccell == SOUTH_T && last_ccell == SOUTH_T && std::mem::discriminant(&t_before) == std::mem::discriminant(&t_after) && t_before != t_after {
            row.t_flips_with_cart += 1;
        }
        last_ccell = ccell;
        if row.release_at.is_some() {
            row.min_z = Some(row.min_z.map_or(c.pos.z, |m| m.min(c.pos.z)));
        }
        if f.trip && row.entry_at.is_none() && (c.pos.z - (ENTRY[2] as f64 + 0.5)).abs() < 0.3 && c.pos.x < ENTRY[0] as f64 + 0.5 && c.pos.y > 1.5 {
            row.entry_at = Some(t);
            row.entry_speed = Some(c.delta_movement.x.hypot(c.delta_movement.z));
        }
        if f.trip {
            if ((ccell[0], ccell[2]) == (GATE[0], GATE[2]) || (ccell[0], ccell[2]) == (7, 7)) && still >= 5 && c.pos.y < -10.0 {
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
    if row.ridden_held {
        row.problems.push("a ridden cart in the south spur".into());
    }
    if row.ridden_t_unpowered {
        row.problems.push("a ridden cart entered the south T unpowered".into());
    }
    if row.t_flips_with_cart > 0 {
        row.problems.push(format!("the south T flipped {} times with a cart's centre in it", row.t_flips_with_cart));
    }
    if row.min_lead.is_some_and(|l| l < 3) {
        row.problems.push(format!("the south T's lead over a ridden cart only {:?} gt", row.min_lead));
    }
    if row.out_at.is_none() && row.held.is_none() {
        row.problems.push("the cart neither out nor held in 2400 gt".into());
    }
    if row.out_at.is_some() && !row.seated_out {
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

/// (ironspec 20:49) A strand's consequence, give-up only: visit 1's napper A with a save and
/// reload at `reload_at` gt after the entry (400 gt); the cart leaves the tier past R (empty or
/// not), and `c_delay` gt after R the picker's next napper C arrives at the entry in a fresh cart.
/// Each visit: F 10 live gt after a lie-down edge (any villager) after its arrival, or at the
/// give-up `give_up` live gt after the arrival; the release 30 live gt after F.
#[derive(Debug, Default, Clone)]
pub struct StrandRow {
    pub a_seated_visit: Option<u8>,
    pub c_seated_visit: Option<u8>,
    pub stranded: bool,
    pub home_lost: Vec<(&'static str, i64)>,
    pub visit2_f: Option<(i64, &'static str)>,
    pub problems: Vec<String>,
}

pub fn strand_bench(spec: &FarmSpec, seed: u64, speed: f64, reload_at: i64, c_delay: i64, give_up: i64) -> Result<StrandRow, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, 3 * 24000, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    let want = 13_800 + (seed as i64 * 37) % 400;
    let mut guard = 0;
    while run.world.day_time().rem_euclid(24000) != want {
        run.tick();
        guard += 1;
        if guard > 30_000 {
            return Err("never reached the visit's hour".into());
        }
    }
    let h = BED_HEAD;
    let nbt = format!("{{Rotation:[0f,0f],Passengers:[{{id:\"minecraft:villager\",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}", h[0], h[1], h[2]);
    let start = [ENTRY[0] as f64 + 0.5, ENTRY[1] as f64 + 0.0625, ENTRY[2] as f64 + 0.5];
    let spawn_visit = |run: &mut redsim::entity::mob::farm::Run| -> Result<(u32, u32), String> {
        let cid = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&nbt))?;
        let vid = run.world.entities.list.iter().find(|e| e.id == cid).and_then(|e| e.passengers.first().copied()).ok_or("no rider")?;
        run.own.push(cid);
        run.own.push(vid);
        let i = run.world.entities.list.iter().position(|e| e.id == cid).ok_or("cart gone")?;
        run.world.entities.list[i].delta_movement = Vec3::new(-speed, 0.0, 0.0);
        let ps = run.world.entities.list[i].passengers.clone();
        for pid in ps {
            if let Some(j) = run.world.entities.list.iter().position(|e| e.id == pid) {
                redsim::entity::position_rider(&mut run.world, i, j);
            }
        }
        Ok((cid, vid))
    };
    let set = |w: &mut World, c: [i32; 3], st: &str| w.set_block(BlockPos::new(c[0], c[1], c[2]), block(st), redsim::world::flags::ALL);
    let mut row = StrandRow::default();
    let (mut cart1, mut a) = spawn_visit(&mut run)?;
    let mut cart2: Option<u32> = None;
    let mut c: Option<u32> = None;
    // visit state
    let mut visit: u8 = 1;
    let mut arrival_live = 0i64;
    let mut f_live: Option<i64> = None;
    let mut f_fired = false;
    let mut released = false;
    let mut live = 0i64;
    let mut was_occ = false;
    let mut r1_at: Option<i64> = None;
    let mut homes = [true, true];
    for t in 1..=4000i64 {
        if t == reload_at {
            redsim::reload::unload_all(&mut run.world);
        }
        if t == reload_at + 400 {
            redsim::reload::load_all_ordered(&mut run.world, redsim::reload::Order::Save);
            let rn = |id: u32, w: &World| w.renumbered.get(&id).copied().unwrap_or(id);
            cart1 = rn(cart1, &run.world);
            a = rn(a, &run.world);
            cart2 = cart2.map(|x| rn(x, &run.world));
            c = c.map(|x| rn(x, &run.world));
        }
        let away = t >= reload_at && t < reload_at + 400;
        if !away {
            live += 1;
        }
        // visit 2: C arrives c_delay gt after visit 1's cart passed R; the park re-braked, the
        // hatches open again
        if visit == 1 && r1_at.is_some_and(|r| t == r + c_delay) {
            for pc in PARK_CTRL {
                set(&mut run.world, pc, "stone");
            }
            set(&mut run.world, HATCH_CTRL, "redstone_block");
            set(&mut run.world, HATCH_CTRL_HI, "redstone_block");
            let (cid, vid) = spawn_visit(&mut run)?;
            cart2 = Some(cid);
            c = Some(vid);
            visit = 2;
            arrival_live = live;
            f_live = None;
            f_fired = false;
            released = false;
        }
        // F: a lie-down edge after the arrival (+10), or the give-up
        let occ = matches!(run.world.get(BlockPos::new(BED_HEAD[0], BED_HEAD[1], BED_HEAD[2])), Block::Bed { occupied: true, .. });
        if !away {
            if occ && !was_occ && f_live.is_none() && live > arrival_live {
                f_live = Some(live + 10);
            }
            was_occ = occ;
            let f_due = f_live.is_some_and(|x| live >= x) || live == arrival_live + give_up;
            if !f_fired && f_due {
                f_fired = true;
                if visit == 2 {
                    row.visit2_f = Some((t, if f_live.is_some_and(|x| live >= x) { "a lie-down" } else { "the give-up" }));
                }
                f_live = Some(live);
                set(&mut run.world, HATCH_CTRL, "stone");
                set(&mut run.world, HATCH_CTRL_HI, "stone");
            }
            if f_fired && !released && f_live.is_some_and(|x| live >= x + 30) {
                released = true;
                for pc in PARK_CTRL {
                    set(&mut run.world, pc, "redstone_block");
                }
            }
        }
        run.tick();
        if away {
            continue;
        }
        let w = &run.world;
        // HOMEs and seats
        for (k, id) in [(0usize, Some(a)), (1usize, c)] {
            let Some(id) = id else { continue };
            let Some(i) = idx(w, id) else { continue };
            let has = redsim::entity::mob::ai::brain::brain(w, i).home.is_some();
            if homes[k] && !has {
                row.home_lost.push((if k == 0 { "A" } else { "C" }, t));
            }
            homes[k] = has;
            let veh = w.entities.list[i].vehicle;
            let on = |cid: Option<u32>| cid.is_some() && veh == cid && released;
            if k == 0 && row.a_seated_visit.is_none() {
                if on(Some(cart1)) && visit == 1 {
                    row.a_seated_visit = Some(1);
                } else if on(cart2) {
                    row.a_seated_visit = Some(2);
                }
            }
            if k == 1 && row.c_seated_visit.is_none() && on(cart2) {
                row.c_seated_visit = Some(2);
            }
        }
        // visit 1's cart past R
        if r1_at.is_none()
            && let Some(ci) = idx(w, cart1)
        {
            let cp = w.entities.list[ci].pos;
            if (cp.z - (EXIT_DET[2] as f64 + 0.5)).abs() < 0.3 && cp.x > EXIT_DET[0] as f64 + 0.5 {
                r1_at = Some(t);
                row.stranded = row.a_seated_visit.is_none();
            }
        }
        // done: visit 2's cart past R
        if let Some(c2) = cart2
            && let Some(ci) = idx(w, c2)
        {
            let cp = w.entities.list[ci].pos;
            if (cp.z - (EXIT_DET[2] as f64 + 0.5)).abs() < 0.3 && cp.x > EXIT_DET[0] as f64 + 0.5 {
                break;
            }
        }
        if visit == 1 && r1_at.is_some() && !row.stranded {
            // (no strand: nothing to follow)
            break;
        }
    }
    if row.stranded && (row.a_seated_visit.is_none() || row.c_seated_visit.is_none()) {
        row.problems.push("not healed: a napper left in the tier after visit 2".into());
    }
    if !row.home_lost.is_empty() {
        row.problems.push(format!("HOME lost {:?}", row.home_lost));
    }
    Ok(row)
}
