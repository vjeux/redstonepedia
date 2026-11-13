//! The vertical drop, M1 of the compact one-bed farm's design space (vjeux 2026-11-11 21:00; the
//! design space doc v0, artifact 1819304709007587): a batch's three carts wait off-rail on closed
//! trapdoors straight above the three stand pockets and fall into them when the trapdoors open (no
//! descent track, no stand gates, no stagger), family-agnostic: the series layout (today's strip,
//! three V pockets along one row, the batch leaving east down it) or Q2's parallel layout (three
//! side-by-side pockets, each the front of its own lane, each leaving east on its own track).
//!
//! Each pocket's cell is a powered rail ascending west on a switched support: unpowered it brakes a
//! cart to rest (`moveAlongTrack`: < 0.03 stops, else x0.5), powered at REL it lets the cart go down
//! the slope. The waiting batch stands on bottom-half trapdoors in hold cells (glass on the cell's
//! four sides at the trapdoor's level: a fixed x, z); the shaft below is clear down to the pocket,
//! whose rider-height cell holds 9fc3155f's occupancy string (tripwire north-south, hooks on either
//! side, stone one cell out). tick-exact (21:07, 26.3 code): a cart falling onto a rail takes no
//! damage from any height (it snaps when a tick starts with its centre over the rail's column and y
//! below the rail + 2; its vertical speed zeroed), off-rail it falls at 0.04 per tick with x0.95
//! drag (0.76 b/t terminal); a cart resting on a closed trapdoor falls straight when it opens; a
//! cart landing on a collision box (a rider's head) hurts its rider.
//!
//! The bench (T: a world time that is a multiple of 100, the summon tick; every timer on the live
//! clock, paused by a freeze): the departing batch at rest in the pockets leaves at REL (+ each
//! pocket's stagger), the pockets re-brake at REL + `rebrake`; trapdoor k opens at T + `release`
//! (gated, with `occupancy`, on its own pocket's string: not while it has been held on for 20 gt or
//! more) and shuts `pulse` gt after (0: stays open). Printed per run: each waiting cart's position
//! and rider's feet at the release, the trapdoor's tick and why it waited, each cart's fall (the
//! trapdoor to its pocket's rail) and its speed, whether its pocket was braked when it landed, its
//! rest tick against the next summon, contacts (centres under 1.25 with the one hit slow), the
//! closest an arriving cart came to a departing one, the riders' damage, each departing cart's exit,
//! and the layout's footprint and height (static).

use crate::layouts::Builder;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// Where the three pockets are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropLayout {
    /// Today's strip (z 0 here): V pockets along x at (13,-2), (9,-1), (5,0), the batch leaving
    /// east down the row to a straight boosted exit.
    Series,
    /// Q2: three side-by-side pockets at x 0, y 0, z 0 / p / 2p, each leaving east on its own
    /// straight boosted track.
    Parallel(i32),
    /// The F1 + Q2 station in 9fc3155f's budget (explore/farmopt-c3-borders ae287a4cd,
    /// wip/c3/f1q2-budget.txt; written in the file frame = loop + (6,0,6)): level pockets at loop
    /// (-1,0,0), (3,0,0), (7,0,0), each leaving +z into the merge (a boosted stand-in for it here),
    /// the holds over them, the pad under the summoners' overlap.
    Station,
}

impl DropLayout {
    /// Each pocket's rail cell (x, y, z), front first (the series layout's east end first).
    pub fn pockets(&self) -> [(i32, i32, i32); 3] {
        match self {
            DropLayout::Series => [(13, -2, 0), (9, -1, 0), (5, 0, 0)],
            DropLayout::Parallel(p) => [(0, 0, 0), (0, 0, *p), (0, 0, 2 * p)],
            DropLayout::Station => [(5, 0, 6), (9, 0, 6), (13, 0, 6)],
        }
    }
    /// Whether a departing cart at `p` has left (removed), mid-exit.
    fn departed(&self, p: Vec3) -> bool {
        match self {
            DropLayout::Series => p.x > 17.5,
            DropLayout::Parallel(_) => p.x > 3.5,
            DropLayout::Station => p.z > 12.9,
        }
    }
}

/// The strip as the S3 file lays it (x, y, state), z 0 here, the gates' blocks left out.
const STRIP: [(i32, i32, &str); 12] = [
    (4, 1, "rail[shape=ascending_west,waterlogged=false]"),
    (5, 0, "rail[shape=ascending_west,waterlogged=false]"),
    (6, -1, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (7, -1, "rail[shape=ascending_east,waterlogged=false]"),
    (8, 0, "rail[shape=east_west,waterlogged=false]"),
    (9, -1, "rail[shape=ascending_west,waterlogged=false]"),
    (10, -2, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (11, -2, "rail[shape=ascending_east,waterlogged=false]"),
    (12, -1, "rail[shape=east_west,waterlogged=false]"),
    (13, -2, "rail[shape=ascending_west,waterlogged=false]"),
    (14, -3, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (15, -3, "rail[shape=east_west,waterlogged=false]"),
];

/// The series layout's gate slopes (the old G0..G2 powered slopes after each pocket) and their
/// sources: switched with their pockets.
const SERIES_SLOPE_SRC: [(i32, i32, i32); 3] = [(14, -4, 0), (10, -3, 0), (6, -2, 0)];

const TRAPDOOR_SHUT: &str = "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]";
const TRAPDOOR_OPEN: &str = "oak_trapdoor[facing=north,half=bottom,open=true,powered=true,waterlogged=false]";

/// The drop design: the pockets (braked powered slopes on switched stone supports, each with an
/// anchor rail up-west keeping its slope), the departures, the hold cells at `hold_y` (the trapdoor
/// block's y; in the series layout one plane over all three), the occupancy strings, the departing
/// batch at rest mid-cell in the pockets and the waiting batch on the trapdoors.
pub fn design(layout: DropLayout, hold_y: i32, dep: [bool; 3], arr: [bool; 3]) -> String {
    design_with(layout, hold_y, dep, arr, false)
}

/// [`design`], with `pad` (the parallel layout) the golem pad under the summoners' overlap.
pub fn design_with(layout: DropLayout, hold_y: i32, dep: [bool; 3], arr: [bool; 3], pad: bool) -> String {
    design_full(layout, hold_y, dep, arr, pad, false)
}

/// [`design_with`], with `refill` (the station) the lanes' R_1 cells and the next batch on them.
pub fn design_full(layout: DropLayout, hold_y: i32, dep: [bool; 3], arr: [bool; 3], pad: bool, refill: bool) -> String {
    design_extra(layout, hold_y, dep, arr, pad, refill, &Extras::default())
}

/// What a bench adds to a drop design: blocks set last (a scarer's cell and its sight openings,
/// from its owner's search), the brief's zombie posts (each at its boat's cell: glass under, an oak
/// boat with a persistent zombie, leaves 3 above), and the hold's north stop as a bottom slab
/// (collision y 17..17.5: it stops the cart coming off R_1 and passes the hold rider's sight north
/// over it) instead of a glass block.
#[derive(Clone, Debug, Default)]
pub struct Extras {
    pub blocks: Vec<(i32, i32, i32, String)>,
    pub posts: Vec<(i32, i32, i32)>,
    pub slab_stop: bool,
    /// (ironspec 09:32) R_1's built driver laid: a repeater at (x+1,17,1) facing west into R_1,
    /// on glass, fed from (x+2,17,1) (the controller's cell), instead of the side cell's glass.
    pub r_repeater: bool,
}

/// [`design_full`] with [`Extras`].
pub fn design_extra(layout: DropLayout, hold_y: i32, dep: [bool; 3], arr: [bool; 3], pad: bool, refill: bool, extra: &Extras) -> String {
    if layout == DropLayout::Station {
        return station_design(hold_y, dep, arr, pad, refill, extra);
    }
    design_body(layout, hold_y, dep, arr, pad, extra)
}

fn design_body(layout: DropLayout, hold_y: i32, dep: [bool; 3], arr: [bool; 3], pad: bool, extra: &Extras) -> String {
    if layout == DropLayout::Station {
        return station_design(hold_y, dep, arr, pad, false, extra);
    }
    let mut b = Builder::new("drop-bench");
    let pockets = layout.pockets();
    match layout {
        DropLayout::Series => {
            for (x, y, s) in STRIP {
                b.set(x, y, 0, s);
                b.set(x, y - 1, 0, "glass");
            }
            for (x, y, z) in SERIES_SLOPE_SRC {
                b.set(x, y, z, "stone");
            }
            // (the old G0 source sat beside its slope at z +1; here under it: the slope (14,-3)
            // has glass under it from the strip, replaced by the switched stone)
            for x in 16..=19 {
                b.set(x, -3, 0, "powered_rail[powered=true,shape=east_west,waterlogged=false]");
                b.set(x, -4, 0, "redstone_block");
            }
            b.set(20, -3, 0, "rail[shape=east_west,waterlogged=false]");
            b.set(20, -4, 0, "glass");
            b.set(21, -3, 0, "glass");
        }
        DropLayout::Parallel(_) => {
            for (x, y, z) in pockets {
                // the anchor up-west, then east: a plain flat (no power across), four boosters, a
                // plain flat, a stop
                b.set(x - 1, y + 1, z, "rail[shape=ascending_west,waterlogged=false]");
                b.set(x - 1, y, z, "glass");
                b.set(x + 1, y, z, "rail[shape=east_west,waterlogged=false]");
                b.set(x + 1, y - 1, z, "glass");
                for dx in 2..=5 {
                    b.set(x + dx, y, z, "powered_rail[powered=true,shape=east_west,waterlogged=false]");
                    b.set(x + dx, y - 1, z, "redstone_block");
                }
                b.set(x + 6, y, z, "rail[shape=east_west,waterlogged=false]");
                b.set(x + 6, y - 1, z, "glass");
                b.set(x + 7, y, z, "glass");
            }
        }
        DropLayout::Station => unreachable!(),
    }
    for (x, y, z) in pockets {
        b.set(x, y, z, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]");
        b.set(x, y - 1, z, "stone");
        // the hold cell: the trapdoor, glass on its four sides
        b.set(x, hold_y, z, TRAPDOOR_SHUT);
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            b.set(x + dx, hold_y, z + dz, "glass");
        }
        // the occupancy string in the pocket's rider-height cell, hooks on either side, stone out
        // (Q2's lanes need a pitch of 4 for them: closer, a lane's hooks and stone sit in the next
        // lane's cells)
        if matches!(layout, DropLayout::Parallel(p) if p < 4) {
            continue;
        }
        b.set(x, y + 1, z, "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]");
        b.set(x, y + 1, z - 1, "tripwire_hook[attached=true,facing=south,powered=false]");
        b.set(x, y + 1, z + 1, "tripwire_hook[attached=true,facing=north,powered=false]");
        // (glass: no stone top in the summoners' scan offers a golem a stray spot)
        b.set(x, y + 1, z - 2, "glass");
        b.set(x, y + 1, z + 2, "glass");
    }
    if pad && let DropLayout::Parallel(p) = layout {
        // the pad (v4's 44-spot pattern moved under Q2's summoners' overlap, x -8..8, z 0..2p):
        // two x-axis portals 9 wide (x -4..4, y -8..-6) on rows z p/2 and p/2 + 3, obsidian frames
        // (sides x -5 and 5, bottom y -9, top y -5), the floor rows (cobblestone x -5..5 at y -7)
        // on either side of each: golem feet at -6 on 4 rows x 11 = 44 spots, every golem's box
        // over a portal; strings on the portals' tops (hooks at x -5 and 5 on glass at x -6 and 6)
        // keep golems off the frames
        let z0 = p / 2 - 1;
        let rows_portal = [z0 + 1, z0 + 4];
        let rows_floor = [z0, z0 + 2, z0 + 3, z0 + 5];
        for zr in rows_portal {
            for x in -4..=4 {
                b.set(x, -9, zr, "obsidian");
                b.set(x, -5, zr, "obsidian");
                for y in -8..=-6 {
                    b.set(x, y, zr, "nether_portal[axis=x]");
                }
                b.set(x, -4, zr, "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
            }
            for y in -8..=-6 {
                b.set(-5, y, zr, "obsidian");
                b.set(5, y, zr, "obsidian");
            }
            b.set(-5, -4, zr, "tripwire_hook[attached=true,facing=east,powered=false]");
            b.set(5, -4, zr, "tripwire_hook[attached=true,facing=west,powered=false]");
            b.set(-6, -4, zr, "glass");
            b.set(6, -4, zr, "glass");
        }
        for zr in rows_floor {
            for x in -5..=5 {
                b.set(x, -7, zr, "cobblestone");
            }
        }
    }
    let rider = "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}";
    let empty = "{Rotation:[0f,0f]}";
    for (k, (x, y, z)) in pockets.iter().enumerate() {
        b.entity("minecart", [*x as f64 + 0.5, *y as f64 + 0.5625, *z as f64 + 0.5], Some(if dep[k] { rider } else { empty }));
    }
    for (k, (x, _, z)) in pockets.iter().enumerate() {
        b.entity("minecart", [*x as f64 + 0.5, hold_y as f64 + 0.1875, *z as f64 + 0.5], Some(if arr[k] { rider } else { empty }));
    }
    b.set(-8, 0, 0, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, 0]);
    add_extras(&mut b, extra);
    b.text(&["The vertical drop (M1): the waiting batch on trapdoors straight above three braked pockets, the batch in them leaving at REL (supports, trapdoors bench-driven on explicit live times)."])
}

/// The extra blocks and the zombie posts into a design.
fn add_extras(b: &mut Builder, extra: &Extras) {
    for (x, y, z, s) in &extra.blocks {
        b.set(*x, *y, *z, s);
    }
    for (x, y, z) in &extra.posts {
        b.set(*x, *y - 1, *z, "glass");
        b.set(*x, *y + 3, *z, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
        b.entity("oak_boat", [*x as f64 + 0.5, *y as f64, *z as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    }
}

/// The layout's footprint and height (static): the bounding box of its blocks from the pockets'
/// supports up to the hold cells' walls and the waiting riders' heads, the drop per pocket (the
/// waiting rider's feet over the pocket's resting rider's feet).
pub fn footprint(layout: DropLayout, hold_y: i32) -> String {
    let pockets = layout.pockets();
    let (x0, x1) = match layout {
        DropLayout::Series => (4, 21),
        DropLayout::Parallel(_) => (-1, 7),
        DropLayout::Station => return station_footprint(hold_y),
    };
    let zs: Vec<i32> = pockets.iter().map(|p| p.2).collect();
    let (z0, z1) = (zs.iter().min().unwrap() - 2, zs.iter().max().unwrap() + 2);
    let y0 = pockets.iter().map(|p| p.1 - 1).min().unwrap().min(if layout == DropLayout::Series { -4 } else { -1 });
    let y1 = hold_y + 2;
    let feet = hold_y as f64 + 0.1875;
    format!(
        "footprint {} x {} (x {x0}..{x1}, z {z0}..{z1}), height {} (y {y0}..{y1}: the waiting riders' heads at {:.2}); the drop per pocket (the waiting rider's feet {feet:.2} over the resting rider's): {}; the shafts 1 x 1, {} cells clear above each pocket's string",
        x1 - x0 + 1,
        z1 - z0 + 1,
        y1 - y0 + 1,
        feet + 1.95,
        pockets.iter().map(|(_, y, _)| format!("{:.2}", feet - (*y as f64 + 0.5625))).collect::<Vec<_>>().join(", "),
        pockets.iter().map(|(_, y, _)| (hold_y - y - 2).to_string()).collect::<Vec<_>>().join("/")
    )
}

/// The F1 + Q2 station (9fc3155f's budget; written in the file frame = loop + (6,0,6), the
/// budget's loop cells + 6 in x and z): per lane k at loop x -1 / 3 / 7 (file 5 / 9 / 13), z 0
/// (file 6): the level pocket (a powered rail ascending north on its switched support, leaving +z;
/// the anchor rail up-north on glass keeps its slope), the string over it (east-west, hooks at x
/// +-1, glass supports at x +-2 shared between lanes), the 1 x 1 shaft (file y 1..hold-1), the hold
/// (a bottom-half trapdoor at `hold_y`, the 3 x 3 glass ring); with `pad`, the golem pad under the
/// summoners' overlap (v4's pattern: x-axis portals 10 wide, file x 4..13 / loop -2..7, on rows file
/// z 1 and 4 / loop -5 and -2, obsidian frames x 3 and 14, bottom y -9, top y -5; cobblestone floor
/// rows file z 0, 2, 3, 5 / loop -6, -4, -3, -1 at y -7, x 3..14 / loop -3..8: golem feet -6, 48
/// spots; glass over the frames' tops at y -4 so no golem stands on them) and a one-block Nether
/// part (a portal needs one); the merge's cells (loop z 1..9, 7b90c3f1's) a boosted stand-in: a
/// plain flat, four boosters, a plain flat and a stop per lane.
pub fn station_design(hold_y: i32, dep: [bool; 3], arr: [bool; 3], pad: bool, refill: bool, extra: &Extras) -> String {
    // (the flat-launcher station, a07e7a1b3 on: named apart from 026d7b72c's anchor-rail pockets)
    let mut b = Builder::new("f1q2-station-launcher");
    let pockets = DropLayout::Station.pockets();
    for (x, y, z) in pockets {
        // the pocket: a flat powered rail on its switched support (unpowered it holds the cart;
        // powered, the stone at its north end kicks the cart south and it boosts away): no anchor
        // rail behind it (9fc3155f's flee check: a rail cell no rider leaves arms a stamp); glass
        // on the stone so no golem stands on it
        b.set(x, y, z, "powered_rail[powered=false,shape=north_south,waterlogged=false]");
        // (V15, ironspec 14:01: no golem floor or stroll candidate under the pocket: glass, the
        // drive from the side (below); ironspec 15:08/15:11: slime, not glass, for the back: a
        // cart at rest on a powered rail is kicked only away from a redstone conductor, which glass
        // isn't (the bench: V15's glass back held all three carts in their pockets; tick-exact's
        // official kick row, 15:18); slime conducts and isn't solid-render, so (x,1,-1) stays out
        // of the candidates)
        b.set(x, y - 1, z, "glass");
        b.set(x, y, z - 1, "slime_block");
        // (M16bw, ironspec 16:03; G1 on d8394ed6e, ironspec 17:56: the pocket's drive is day-1's
        // repeater (x-1,0,0) into the rail, facing west (its input the riser cell (x-2,0,0)), on
        // glass (x-1,-1,0), laid here as a stand-in until day-1's file; the bench drives its input
        // from a lever stand-in at (x-2,0,0) on glass (x-2,-1,0) (the riser's cells, day-1's: the
        // written file leaves them out), never a block in the support cell)
        b.set(x - 1, y, z, "repeater[delay=1,facing=west,locked=false,powered=false]");
        b.set(x - 1, y - 1, z, "glass");
        b.set(x - 2, y, z, "lever[face=floor,facing=east,powered=false]");
        b.set(x - 2, y - 1, z, "glass");
        // (an unattached string on the launcher's stone: no golem stands on it, and unlike glass it
        // passes the pocket rider's sight north)
        b.set(x, y + 1, z - 1, "tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]");
        b.set(x, y + 1, z, "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
        b.set(x - 1, y + 1, z, "tripwire_hook[attached=true,facing=east,powered=false]");
        b.set(x + 1, y + 1, z, "tripwire_hook[attached=true,facing=west,powered=false]");
        b.set(x - 2, y + 1, z, "glass");
        b.set(x + 2, y + 1, z, "glass");
        b.set(x, hold_y, z, TRAPDOOR_SHUT);
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            b.set(x + dx, hold_y, z + dz, "glass");
        }
        if refill {
            // the lane's front cell R_1 (loop (x,17,1)): a powered rail ascending south (the cart
            // rolls north into the hold) resting on the ring's south glass, powered from the side
            // at (x+1,17,1) (glass off, a redstone block on: never through its support, which
            // touches the trapdoor; glass over it so no golem stands on it), R_2's rail up-south on
            // glass as its anchor (ironfarm's lane); the hold's north wall one higher, so the cart
            // coming off R_1 at ~0.5 b/t drops into the hold rather than over the ring; the hold's
            // string at rider height (x,17,0), hooks at x +-1, glass at x +-2 (ironfarm's release gate)
            b.set(x, hold_y + 1, z + 1, "powered_rail[powered=false,shape=ascending_south,waterlogged=false]");
            if extra.r_repeater {
                // (the built driver: a repeater facing west into R_1 (its FACING the input side,
                // east), on glass; its input (x+2,17,1) the controller's, driven by the bench from a
                // lever stand-in there on glass (x+2,16,1) (ironspec 17:46/17:49: no stand-in block
                // whose golem or path properties differ from the line end's; the written file
                // leaves both to the controller))
                b.set(x + 1, hold_y + 1, z + 1, "repeater[delay=1,facing=east,locked=false,powered=false]");
                b.set(x + 1, hold_y, z + 1, "glass");
                b.set(x + 2, hold_y + 1, z + 1, "lever[face=floor,facing=west,powered=false]");
                b.set(x + 2, hold_y, z + 1, "glass");
            } else {
                b.set(x + 1, hold_y + 1, z + 1, "glass");
            }
            b.set(x + 1, hold_y + 2, z + 1, "glass");
            b.set(x, hold_y + 2, z + 2, "rail[shape=ascending_south,waterlogged=false]");
            b.set(x, hold_y + 1, z + 2, "glass");
            // (the lanes' glass under R_3, a stand-in: R_2's high side; without it R_2 pops at
            // placement, which main's placement check (onechunk's 0f375d1c0) now refuses)
            b.set(x, hold_y + 2, z + 3, "glass");
            b.set(x, hold_y + 1, z - 1, if extra.slab_stop { "smooth_stone_slab[type=bottom,waterlogged=false]" } else { "glass" });
            b.set(x, hold_y + 1, z, "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
            b.set(x - 1, hold_y + 1, z, "tripwire_hook[attached=true,facing=east,powered=false]");
            b.set(x + 1, hold_y + 1, z, "tripwire_hook[attached=true,facing=west,powered=false]");
            // (ironspec 02:59: the hold string's hook supports solid-render instead of glass, a
            // stroll destination (the station's arm-and-never-clear places 24 -> 8), sight lines
            // unchanged (glass blocks sight too); every top exposed (nothing over (x+-2,17,0) in
            // the station; z 0 is outside the lanes' box) and dark at night under the sky, so lit:
            // glowstone, no natural-spawn spot)
            b.set(x - 2, hold_y + 1, z, "glowstone");
            b.set(x + 2, hold_y + 1, z, "glowstone");
        }
        // the departure: a plain rail, then the exit detector (no face on a powered rail or a
        // junction: plain rails north and south of it; its rising edge is the departing cart's
        // exit, the --exit-gate evidence)
        b.set(x, y, z + 1, "rail[shape=north_south,waterlogged=false]");
        b.set(x, y - 1, z + 1, "glass");
        b.set(x, y, z + 2, "detector_rail[powered=false,shape=north_south,waterlogged=false]");
        b.set(x, y - 1, z + 2, "glass");
        if x == 13 {
            // (golem-stamp path (a) (2c), ironspec 14:11: lane x 7 climbs from loop z 3, four
            // ascending powered rails to its curve (7,4,7), each on glass, a redstone torch on
            // glass at (6,1,4) powering the chain; the old line's cells glass (V15); its climb
            // west to the lift is laid below)
            for dz in 3..=6 {
                b.set(x, y + dz - 3, z + dz, "powered_rail[powered=true,shape=ascending_south,waterlogged=false]");
                b.set(x, y + dz - 4, z + dz, "glass");
                if dz >= 4 {
                    b.set(x, y - 1, z + dz, "glass");
                }
            }
            b.set(x, y, z + 5, "glass");
            b.set(x, y, z + 6, "glass");
            b.set(x - 1, y + 1, z + 4, "redstone_torch[lit=true]");
            b.set(x - 1, y, z + 4, "glass");
            continue;
        }
        // (path (a)'s (1), ironspec 14:11: lane x 3's z 3 a booster, powered through z 4's chain;
        // z 1 stays plain: powered, it would chain into the pocket and kill its brake)
        b.set(x, y, z + 3, if x == 9 { "powered_rail[powered=true,shape=north_south,waterlogged=false]" } else { "rail[shape=north_south,waterlogged=false]" });
        b.set(x, y - 1, z + 3, "glass");
        // (three boosters to loop z 6, the station's last cell; loop z 7 is the merge row
        // (7b90c3f1's, shifted from z 8 for the strict pitch 32): the bench takes the cart out at
        // loop z 6.9, still on z 6's rail, its speed the merge row's entry speed)
        for dz in 4..=6 {
            b.set(x, y, z + dz, "powered_rail[powered=true,shape=north_south,waterlogged=false]");
            // (V15, ironspec 14:01: glass under the line, no golem floor; an always-on redstone
            // torch on glass beside its middle rail powers it, the chain the other two)
            b.set(x, y - 1, z + dz, "glass");
        }
        b.set(x - 1, y, z + 5, "redstone_torch[lit=true]");
        b.set(x - 1, y - 1, z + 5, "glass");
    }
    // (golem-stamp path (a) (2c), ironspec 14:11/14:16: lane x 7's climb west from its curve
    // (7,4,7): nine ascending rails (6..-2, 4..12, 7) to y 13 on glass, the first two powered (a
    // redstone torch on glass at (6,4,6) powers (6,4,7), the chain (5,5,7)), the other seven plain:
    // the cart crests at a crawl and brakes to rest near the slope's top, its rider's feet ~12.78
    // (with all nine powered it crested at a stored speed ~1.39, braked to rest at the slope's
    // foot with its box in the column's water, crept on the release and, after a restart as it
    // settled, rose under the lift's wall glass (-4,14,7) and stuck); and the crest (-3,13,7); the
    // braked slope (-4,12,7) beyond it and the crest's glass (-3,12,7) are the lift's (7b90c3f1's,
    // the path-a part); loop frame + 6 here)
    b.set(13, 4, 13, "rail[shape=north_west,waterlogged=false]");
    b.set(13, 3, 13, "glass");
    for i in 0..9 {
        b.set(12 - i, 4 + i, 13, if i < 2 { "powered_rail[powered=true,shape=ascending_west,waterlogged=false]" } else { "rail[shape=ascending_west,waterlogged=false]" });
        b.set(12 - i, 3 + i, 13, "glass");
    }
    b.set(12, 4, 12, "redstone_torch[lit=true]");
    b.set(12, 3, 12, "glass");
    b.set(3, 13, 13, "rail[shape=east_west,waterlogged=false]");
    b.set(3, 12, 13, "glass");
    if pad {
        for zr in [1, 4] {
            for x in 4..=13 {
                b.set(x, -9, zr, "obsidian");
                b.set(x, -5, zr, "obsidian");
                for y in -8..=-6 {
                    b.set(x, y, zr, "nether_portal[axis=x]");
                }
            }
            for y in -8..=-5 {
                b.set(3, y, zr, "obsidian");
                b.set(14, y, zr, "obsidian");
            }
            for x in 3..=14 {
                b.set(x, -4, zr, "glass");
            }
        }
        for zr in [0, 2, 3, 5] {
            for x in 3..=14 {
                b.set(x, -7, zr, "cobblestone");
            }
            // (the pad's seal, ironspec's coupling check 02:04 and ruling 02:24: a zombified piglin
            // in a portal row (its floor the frame's bottom obsidian, feet -8) meets this
            // cobblestone and the floor row's at y -7 on both sides: an adult (1.95) never fit the
            // open cell under the floor row, a baby (0.975) did and dropped out under the pad; its
            // jump reaches feet -6.75 at most, under the floor rows' top -6. Only the cells that
            // touch a portal cell sideways (the portal's width, loop x -2..7: 40 blocks); the end
            // cells at loop x -3 and 8 touch only a frame diagonally)
            for x in 4..=13 {
                b.set(x, -8, zr, "cobblestone");
            }
        }
    }
    let rider = "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}";
    let empty = "{Rotation:[0f,0f]}";
    for (k, (x, y, z)) in pockets.iter().enumerate() {
        b.entity("minecart", [*x as f64 + 0.5, *y as f64 + 0.0625, *z as f64 + 0.5], Some(if dep[k] { rider } else { empty }));
    }
    // (the carts where the lane leaves them, tickexact's lane-handoff recording (ca3cdb2ed), its
    // +x our -z: the hold's cart rolled off R_1 against the ring's north glass, 0.49 into the cell
    // (scene x 6.51); the next one braked on R_1 against that cart's rider, 0.72 down R_1 (scene x
    // 5.72, y 65.3425))
    for (k, (x, _, z)) in pockets.iter().enumerate() {
        b.entity("minecart", [*x as f64 + 0.5, hold_y as f64 + 0.1875, *z as f64 + 0.49], Some(if arr[k] { rider } else { empty }));
    }
    if refill {
        for (x, _, z) in pockets.iter() {
            b.entity("minecart", [*x as f64 + 0.5, hold_y as f64 + 1.3425, *z as f64 + 1.28], Some(rider));
        }
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [0, 1, 0]);
    add_extras(&mut b, extra);
    let mut text = b.text(&["The F1 + Q2 station (dfb89499, 9fc3155f's budget; file = loop + (6,0,6)): three level pockets at loop x -1 / 3 / 7 z 0 under M1 drops from holds at y 16, the golem pad under the summoners' overlap (golem feet -6), the merge a boosted stand-in."]);
    if pad {
        // (a portal needs a Nether part that links: a 2 x 3 x-axis portal in obsidian at the
        // Nether's origin)
        let letter = |id: &str| text.lines().find(|l| l.trim_end().ends_with(&format!(" minecraft:{id}"))).and_then(|l| l.trim_start().chars().next());
        if let (Some(o), Some(p)) = (letter("obsidian"), letter("nether_portal[axis=x]")) {
            let mut nether = String::from("nether-origin: 0 0 0\n");
            nether.push_str(&format!("nether layer 64:\n  .{o}{o}\n"));
            for y in 65..=67 {
                nether.push_str(&format!("nether layer {y}:\n  {o}{p}{p}{o}\n"));
            }
            nether.push_str(&format!("nether layer 68:\n  .{o}{o}\n"));
            text.push_str(&nether);
        }
    }
    text
}

/// The station as a committed design in the loop frame (onechunk's module, farmopt 01:30): the
/// bench's station file (written at loop + (6,0,6)) moved by (-6,0,-6); the next batch on R_1 and
/// R_2's rail and glass left to the lanes' file; `positions:` the villager positions (the pockets'
/// and the holds' rider cells; the shafts are never occupied at a summon tick: the falls run
/// within T+21..T+64); `run-cells:` the cells the runner reads (P0..2 the pockets' rest cells,
/// D0..2 the exit detectors, H0..2 the holds' rest cells, PAD0/PAD1 the pad's golem feet box);
/// `header` written first as comment lines.
/// (golem-stamp path (a), ironspec 13:09) An assembled module in the loop frame (onechunk's: the
/// station, the merge row, the shared lift, the lanes, the split) as the station bench's world: moved
/// into the bench's frame (loop + (6,0,6)), its entities cut to the station's six carts (the pockets'
/// and the holds' riders, the bench's own; the module's other riders, posts and queue are left out:
/// the departing carts' path is the blocks'), its blocks every one.
/// A STAND-IN for golem-stamp path (a)'s lift cells (ironspec 14:11/14:16), laid by `--path-a`
/// only where no record file is given: the record is 7b90c3f1's
/// `wip/lift/f1q2-lift-split-path-a.txt` (83878113f), laid after it with `--part` (G1 on
/// d8394ed6e: its plain merge-end ramp and its lift foot differ from these; on this stand-in the
/// napper ran 3 gt fast). The loop frame: the merge row's west end ramped so lanes x -1 and x 3
/// (and the napper) enter the lift at y 2, powered by a redstone torch on glass at (-3,1,6); the
/// old intake (-4,0,7) closed with glass; the braked slope (-4,12,7) in the lift's east wall, lane
/// x 7's hold (day-1's repeater (-4,12,6) drives it: not laid here), and over it an oak wall sign
/// (-4,13,7) on new glass (-4,13,6) (ironspec 15:08): the cart's 0.7-tall box tips onto the slope
/// through that cell (the lift's wall glass there stopped it on the crest); the sign has no
/// collision box and, waterloggable, holds the column's water; unwaterlogged, so the waiting
/// rider's eye is out of water.
pub const PATH_A_LIFT: [(i32, i32, i32, &str); 11] = [
    (-2, 0, 7, "powered_rail[powered=true,shape=ascending_west,waterlogged=false]"),
    (-3, 0, 7, "glass"),
    (-3, 1, 7, "powered_rail[powered=true,shape=ascending_west,waterlogged=false]"),
    (-4, 0, 7, "glass"),
    (-4, 2, 7, "rail[shape=east_west,waterlogged=false]"),
    (-3, 0, 6, "glass"),
    (-3, 1, 6, "redstone_torch[lit=true]"),
    (-4, 12, 7, "powered_rail[powered=false,shape=ascending_east,waterlogged=false]"),
    // (the slope's high side, ironspec 16:04: 7b90c3f1's cell; it also carries the station's
    // crest (-3,13,7) and is the high side of its last climb slope (-2,12,7))
    (-3, 12, 7, "glass"),
    (-4, 13, 6, "glass"),
    (-4, 13, 7, "oak_wall_sign[facing=south,waterlogged=false]"),
];

/// (ironspec 21:25 (4) / 22:05) A design's declared cart contacts, its header's `# contacts:` and
/// `# stops:` lines (in its own frame; the entries as pushscan's shared `header_entries` splits
/// them): each entry `cells=what`, the cells `x,y,z` joined by '+' (parentheses and spaces
/// allowed); an entry whose `what` says PROVISIONAL (a declaration on bench evidence only,
/// 7b90c3f1's r3) marks its cells provisional (the bool).
pub fn declared_contacts(text: &str) -> Result<Vec<((i32, i32, i32), bool)>, String> {
    let mut out = Vec::new();
    for key in ["contacts:", "stops:"] {
        for entry in crate::pushscan::header_entries(text, key) {
            // (G1 on a3b306a05: strict: an entry without its `=what` or with no cell is refused)
            let Some((cells, what)) = entry.split_once('=') else {
                return Err(format!("`# {key}` entry {entry:?}: no `=what` (the checklist audit (9): parsed strictly)"));
            };
            if what.trim().is_empty() {
                return Err(format!("`# {key}` entry {entry:?}: an empty `=what` (the checklist audit (9))"));
            }
            if cells.split(|c: char| c == '+' || c.is_whitespace()).all(|t| t.trim().is_empty()) {
                return Err(format!("`# {key}` entry {entry:?}: no cell (the checklist audit (9))"));
            }
            // (provisional: the entry says so, or its official row is pending (7b90c3f1's r3
            // wording, "bench only, the official row pending"))
            let provisional = what.to_lowercase().contains("provisional") || what.to_lowercase().contains("official row pending");
            // (the checklist audit (9): every cell token parses as x,y,z (the documented forms:
            // '+' or spaces between cells, parentheses around one), else the header is refused)
            for cell in cells.split(|c: char| c == '+' || c.is_whitespace()).filter(|t| !t.trim().is_empty()) {
                // (G1 16:18: the shared cell grammar, `pushscan::parse_cell_group`, the one
                // `strip_attribution` keeps a leading cell by)
                let Some((x, y, z)) = crate::pushscan::parse_cell_group(cell) else {
                    return Err(format!("`# {key}` entry {entry:?}: the cell {cell:?} is not x,y,z (the checklist audit (9): parsed strictly)"));
                };
                out.push(((x, y, z), provisional));
            }
        }
    }
    Ok(out)
}

/// (ironspec 01:07; G1 on 82f9546f and f7e3a825) A held rail stand-in's write at one tick: the rail's
/// own `powered` state toward `held` (the same block: the guard's list and its survival checked),
/// written without the rail's own updates, then the neighbour updates the build's path makes at that
/// edge, all but the rail's own re-evaluation, which the built rail answers by keeping its state (its
/// driver holds it) and the bench's undriven one by dropping: the driver's front updates
/// (DiodeBlock.updateNeighborsInFront: its front cell and that cell's neighbours but the driver),
/// carrying the driver's kind, a signal source; then the rail's own setBlock (its
/// affectNeighborsAfterRemoval) and PoweredRailBlock.updateState's updates below and, ascending,
/// above, carrying the rail's kind. A plain rail with three or more potential connections among the
/// driver's update cells refuses the run: a signal source's update re-settles a plain rail at exactly
/// 3 (RailBlock.updateState 22-26, tick-exact's citation; 4 is flagged conservatively; G1's note on
/// f7e3a825); one among the rail's own update cells is printed, not refused (the powered rail's kind
/// is no signal source). `driver`: the driver repeater's cell and its front cell (the rail itself, or
/// the block under it). Returns the updated cells, listed, at an edge.
fn held_rail_write(w: &mut redsim::world::World, p: BlockPos, held: bool, driver: (BlockPos, BlockPos)) -> Result<Option<String>, String> {
    let s = w.get(p).to_string();
    if !s.contains("powered_rail") {
        return Err(format!("no powered rail at the held cell {p}: {s}"));
    }
    let want = if held { s.replace("powered=false", "powered=true") } else { s.replace("powered=true", "powered=false") };
    if want == s {
        return Ok(None);
    }
    let (built, b) = (Block::parse(&s).map_err(|e| e.to_string())?, Block::parse(&want).map_err(|e| e.to_string())?);
    let d = property_diff(w, &built, &b);
    if !d.is_empty() {
        return Err(format!("the held rail {p}'s state differs from the rail in {} (ironspec 23:30)", d.join(", ")));
    }
    if !redsim::behavior::can_survive(w, &b, p) {
        return Err(format!("the held rail {p}'s state can't survive there (the guard's survival)"));
    }
    let kind = b.kind();
    let driver_kind = Block::parse("minecraft:repeater[delay=1,facing=north,locked=false,powered=false]").map_err(|e| e.to_string())?.kind();
    w.set_block(p, b, redsim::world::flags::CLIENTS | redsim::world::flags::SKIP_ON_PLACE);
    let (rep, front) = driver;
    let mut signal_cells: Vec<BlockPos> = Vec::new();
    if front != p {
        signal_cells.push(front);
    }
    signal_cells.extend(redsim::Dir::ALL.iter().map(|d| front.rel(*d)).filter(|c| *c != rep && *c != p));
    let mut cells: Vec<BlockPos> = redsim::Dir::ALL.iter().map(|d| p.rel(*d)).collect();
    cells.extend(redsim::Dir::ALL.iter().map(|d| p.rel(redsim::Dir::Down).rel(*d)).filter(|c| *c != p));
    if want.contains("ascending") {
        cells.extend(redsim::Dir::ALL.iter().map(|d| p.rel(redsim::Dir::Up).rel(*d)).filter(|c| *c != p));
    }
    let t_rail = |w: &redsim::world::World, c: BlockPos| w.get(c).to_string().starts_with("minecraft:rail[") && potential_connections(w, c) >= 3;
    if let Some(c) = signal_cells.iter().find(|c| t_rail(w, **c)) {
        return Err(format!(
            "the held rail {p}'s driver updates reach the plain rail {c} ({}), which has {} potential connections: at exactly 3 a signal source's update re-settles it (RailBlock.updateState 22-26, tick-exact's citation); 4 is flagged conservatively (G1's note on f7e3a825)",
            w.get(*c),
            potential_connections(w, *c)
        ));
    }
    for c in &signal_cells {
        w.neighbor_changed(*c, driver_kind);
    }
    for c in &cells {
        w.neighbor_changed(*c, kind);
    }
    let list = |w: &redsim::world::World, v: &[BlockPos]| -> String {
        let mut seen: Vec<BlockPos> = Vec::new();
        v.iter()
            .filter(|c| {
                let new = !seen.contains(c);
                seen.push(**c);
                new
            })
            .map(|c| {
                let note = if t_rail(w, *c) { format!(" (a rail with {} potential connections: the powered rail's kind is no signal source, no re-settle)", potential_connections(w, *c)) } else { String::new() };
                format!("{c} {}{note}", w.get(*c).to_string().trim_start_matches("minecraft:"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    Ok(Some(format!("the driver's ({rep} into {front}): {}; the rail's own: {}", list(w, &signal_cells), list(w, &cells))))
}

/// A plain rail's potential connections (the brief's rule, RailBlock.updateState's count): the
/// horizontal sides (north, south, east, west) with a rail in their column at y - 1 .. y + 1.
pub fn potential_connections(w: &redsim::world::World, c: BlockPos) -> usize {
    [redsim::Dir::North, redsim::Dir::South, redsim::Dir::East, redsim::Dir::West]
        .iter()
        .filter(|d| (-1..=1).any(|dy| redsim::rails::is_rail(&w.get(c.rel(**d).offset(0, dy, 0)))))
        .count()
}

/// (ironspec 21:25 (4); G1 08:18, cc767b78's staleness check) The station's declared stops for the
/// bench's contact verdict (the bench's frame): none. No bench row meets a station cell: each
/// arriving cart falls straight from its hold into its pocket (z 0.490 throughout) and no row rolls a
/// cart into a hold, so a declaration here would never be exercised (stale). The station file's
/// header declares the holds' rest against the ring's north glass (x,16,-1) on tick-exact's
/// lane-handoff row ca3cdb2ed (ironspec 02:41 (b)), whose runs meet it; a bench row that meets a
/// station cell fails as undeclared until a row's evidence declares it.
pub fn station_stops(lo: DropLayout, hold_y: i32) -> Vec<(i32, i32, i32)> {
    let _ = (lo, hold_y);
    Vec::new()
}

/// (ironspec 17:46/18:34/23:30, the bench guard's full list until `recordmatch::guard_intrinsics`)
/// A stand-in block's properties against the built cell's: legacy-solid, a full top face,
/// full-block collision, solid-render, any collision, air, a signal source, golem-excluded, a rail,
/// dangerous, a fluid, and the golem spawn-space role: the ones that differ.
pub fn property_diff(w: &redsim::world::World, built: &Block, written: &Block) -> Vec<&'static str> {
    use redsim::entity::mob::blocks;
    let rail = |b: &Block| b.to_string().contains("rail");
    let props = |b: &Block| {
        let fluid = !redsim::fluid::fluid_state_of(b).is_empty();
        let signal = redsim::behavior::is_signal_source(w, b);
        [
            redsim::behavior::is_solid(b),
            redsim::behavior::sturdy_of(b, redsim::Dir::Up, redsim::behavior::Support::Full),
            redsim::behavior::full_cube(b),
            redsim::entity::mob::ai::goal::stroll::is_solid_render(b),
            redsim::behavior::collides(b),
            b.is_air(),
            signal,
            blocks::golem_excluded(b),
            rail(b),
            blocks::is_dangerous(b, false),
            fluid,
            // (the golem spawn-space role: `NaturalSpawner.isValidEmptySpawnBlock`'s block tests)
            !redsim::behavior::full_cube(b) && !signal && !fluid && !rail(b) && !blocks::is_dangerous(b, false),
        ]
    };
    let names = [
        "legacy-solid",
        "a full top face",
        "full-block collision",
        "solid-render",
        "any collision (a walk's path)",
        "air",
        "a signal source",
        "golem-excluded",
        "a rail",
        "dangerous",
        "a fluid",
        "a golem spawn space",
    ];
    let (a, b) = (props(built), props(written));
    (0..names.len()).filter(|i| a[*i] != b[*i]).map(|i| names[i]).collect()
}

/// (ironspec 18:34) A lever stand-in's reach, checked once the world is built: its update reach
/// (a lever's toggle updates its six neighbours and its support's six: a rail there with three or
/// more potential connections, a T, is refused unless it is in `stable`, the cells declared
/// state-stable) and its power reach (it powers its six neighbours, and its support's if the support
/// conducts: any redstone component or rail it reaches besides `consumer`, the input it stands in
/// for, is refused).
pub fn lever_reach_check(w: &redsim::world::World, p: BlockPos, consumer: BlockPos, stable: &[BlockPos]) -> Result<String, String> {
    let s = w.get(p).to_string();
    if !s.contains("lever") {
        return Err(format!("no lever stand-in at {p}: {s}"));
    }
    let support = lever_support(&s, p);
    let six = |q: BlockPos| [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].map(|(dx, dy, dz)| BlockPos::new(q.x + dx, q.y + dy, q.z + dz));
    let is_rail = |q: BlockPos| w.get(q).to_string().contains("rail");
    let component = |q: BlockPos| {
        let t = w.get(q).to_string();
        ["rail", "repeater", "comparator", "redstone_wire", "redstone_torch", "redstone_wall_torch", "piston", "lamp", "door", "note_block", "dispenser", "dropper", "hopper", "tnt", "observer", "fence_gate", "bell"].iter().any(|k| t.contains(k))
    };
    // (update reach: the toggle's block updates)
    let mut updates: Vec<BlockPos> = six(p).to_vec();
    updates.extend(six(support));
    for q in updates {
        if q == p || !is_rail(q) || stable.contains(&q) {
            continue;
        }
        let around = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dz)| (-1..=1).any(|dy| is_rail(BlockPos::new(q.x + dx, q.y + dy, q.z + dz)))).count();
        if around >= 3 {
            return Err(format!("the lever stand-in at {p}: its toggle updates the rail at {q} ({}), a T with {around} potential connections not declared state-stable (ironspec 18:34)", w.get(q)));
        }
    }
    // (power reach: what it powers, directly or through a conducting support)
    let mut powered: Vec<BlockPos> = six(p).to_vec();
    let conducts = redsim::behavior::conductor_of(&w.get(support));
    if conducts {
        powered.extend(six(support));
    }
    for q in powered {
        if q == p || q == support || q == consumer {
            continue;
        }
        if component(q) {
            return Err(format!("the lever stand-in at {p} also powers {q} ({}), beyond the input it stands in for at {consumer} (ironspec 18:34)", w.get(q)));
        }
    }
    // (the lever against the line end it stands in for: non-solid redstone dust, ironspec 18:24,
    // over the full property list)
    let dust = Block::parse("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]").map_err(|e| e.to_string())?;
    let lever_off = Block::parse(&s.replace("powered=true", "powered=false")).map_err(|e| e.to_string())?;
    let d = property_diff(w, &dust, &lever_off);
    if !d.is_empty() {
        return Err(format!("the lever stand-in at {p} differs from its line end (redstone dust) in {} (ironspec 23:30)", d.join(", ")));
    }
    Ok(format!("{p} on {} ({}) for {consumer}", w.get(support), if conducts { "conducting" } else { "not conducting" }))
}

/// An emulated write (a drive's stand-in), refused when the written block differs from the built
/// cell's in a golem or path property (ironspec 17:46's guard: the stand-in is a lever, or the
/// cell is declared emulated and the statics run with it).
fn emulated_write(w: &mut redsim::world::World, p: BlockPos, b: Block, built: &Block) -> Result<(), String> {
    // (ironspec 04:07: an emulated write that changes nothing fails)
    if w.get(p) == b {
        return Err(format!("an emulated write at {p} changes nothing ({b}): ironspec 04:07"));
    }
    let d = property_diff(w, built, &b);
    if !d.is_empty() {
        return Err(format!(
            "an emulated write at {p} refused (ironspec 17:46): {b} differs from the built cell's {built} in {} (drive it from a lever stand-in, e.g. --r-repeater for the R line)",
            d.join(", ")
        ));
    }
    // (survival, ironspec 20:31: the written block can stand where it is written)
    if !redsim::behavior::can_survive(w, &b, p) {
        return Err(format!("an emulated write at {p} refused (ironspec 20:31): {b} can't survive there"));
    }
    w.set_block(p, b, redsim::world::flags::ALL);
    Ok(())
}

/// A lever stand-in's state: the controller's line end at a drive's input (ironspec 17:46/17:49),
/// refused if the lever can't survive where it stands (ironspec 20:31).
/// The block a lever (its block state `s` at `p`) hangs on.
fn lever_support(s: &str, p: BlockPos) -> BlockPos {
    if s.contains("face=floor") {
        BlockPos::new(p.x, p.y - 1, p.z)
    } else if s.contains("face=ceiling") {
        BlockPos::new(p.x, p.y + 1, p.z)
    } else if s.contains("facing=north") {
        BlockPos::new(p.x, p.y, p.z + 1)
    } else if s.contains("facing=south") {
        BlockPos::new(p.x, p.y, p.z - 1)
    } else if s.contains("facing=east") {
        BlockPos::new(p.x - 1, p.y, p.z)
    } else {
        BlockPos::new(p.x + 1, p.y, p.z)
    }
}

/// (ironspec 20:10 (2): the stand-in guard's sight property) The segment a..b against an
/// axis-aligned box `[x0, x1, y0, y1, z0, z1]` (world units).
fn seg_hits_box(a: Vec3, b: Vec3, bx: [f64; 6]) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q, lo, hi) in [(a.x, b.x, bx[0], bx[1]), (a.y, b.y, bx[2], bx[3]), (a.z, b.z, bx[4], bx[5])] {
        let d = q - p;
        if d.abs() < 1e-12 {
            if p < lo || p > hi {
                return false;
            }
        } else {
            let (u, v) = ((lo - p) / d, (hi - p) / d);
            t0 = t0.max(u.min(v));
            t1 = t1.min(u.max(v));
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// A block's type: its id without the state (`minecraft:oak_trapdoor[open=true,..]` ->
/// `minecraft:oak_trapdoor`).
pub fn block_type(b: &Block) -> String {
    let s = b.to_string();
    s.split('[').next().unwrap_or("").to_string()
}

/// The cells a segment a..b passes through, in order (a voxel walk: Amanatides and Woo).
fn seg_cells(a: Vec3, b: Vec3) -> Vec<BlockPos> {
    let p = [a.x, a.y, a.z];
    let d = [b.x - a.x, b.y - a.y, b.z - a.z];
    let mut cell = [a.x.floor() as i32, a.y.floor() as i32, a.z.floor() as i32];
    let end = [b.x.floor() as i32, b.y.floor() as i32, b.z.floor() as i32];
    let step = d.map(|v| if v > 0.0 { 1 } else if v < 0.0 { -1 } else { 0 });
    let (mut t_max, mut t_delta) = ([f64::INFINITY; 3], [f64::INFINITY; 3]);
    for i in 0..3 {
        if step[i] != 0 {
            let next = if step[i] > 0 { (cell[i] + 1) as f64 } else { cell[i] as f64 };
            t_max[i] = (next - p[i]) / d[i];
            t_delta[i] = 1.0 / d[i].abs();
        }
    }
    let mut out = vec![BlockPos::new(cell[0], cell[1], cell[2])];
    while cell != end && out.len() < 4096 {
        let i = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] { 0 } else if t_max[1] <= t_max[2] { 1 } else { 2 };
        if t_max[i] > 1.0 {
            break;
        }
        cell[i] += step[i];
        t_max[i] += t_delta[i];
        out.push(BlockPos::new(cell[0], cell[1], cell[2]));
    }
    out
}

/// (ironspec 20:10 (2) / 23:30 / 00:27; G1's audit (1); tick-exact's 26.3 citation: a mob's sight is
/// one eye-to-eye segment clipped against collision shapes, fluids ignored) Every villager -> scarer
/// (a zombie or a pillager) and villager -> villager segment, eye to eye (`MobType::def().eye`, each
/// row cited to 26.3's EntityTypes builder; a seated villager stays standing, its feet the cart's
/// position), walked cell by cell against the LIVE world's collision shapes and the reference's
/// (the module of record's cells, every stand-in as its built cell): a cell where the live world
/// clips the segment and the reference doesn't is a crossing. Returns the crossings, the rays judged,
/// and the first villager -> scarer ray's midpoint cell (the positive control's, laid in the world).
pub fn sight_crossings_world(w: &redsim::world::World, reference: &std::collections::HashMap<[i32; 3], Block>) -> (Vec<String>, u32, Option<BlockPos>) {
    use redsim::entity::mob::types::MobType;
    let eye = |e: &redsim::entity::Entity, t: MobType| Vec3::new(e.pos.x, e.pos.y + t.def().eye as f64, e.pos.z);
    let of = |f: &dyn Fn(MobType) -> bool| -> Vec<Vec3> {
        w.entities
            .list
            .iter()
            .filter(|e| !e.removed)
            .filter_map(|e| match &e.kind {
                EntityKind::Mob(m) if f(m.ty) => Some(eye(e, m.ty)),
                _ => None,
            })
            .collect()
    };
    let villagers = of(&|t| t == MobType::Villager);
    let scarers = of(&|t| t == MobType::Zombie || t == MobType::Pillager);
    let mut rays: Vec<(Vec3, Vec3, &str)> = Vec::new();
    for v in &villagers {
        for s in &scarers {
            rays.push((*v, *s, "villager -> scarer"));
        }
    }
    for (i, a) in villagers.iter().enumerate() {
        for b in &villagers[i + 1..] {
            rays.push((*a, *b, "villager -> villager"));
        }
    }
    let air = Block::Air;
    let clips = |s: &Block, a: Vec3, b: Vec3, c: BlockPos| {
        redsim::behavior::collision_boxes(s).iter().any(|bx| {
            let f = |v: i32, o: i32| o as f64 + v as f64 / 16.0;
            seg_hits_box(a, b, [f(bx[0], c.x), f(bx[1], c.x), f(bx[2], c.y), f(bx[3], c.y), f(bx[4], c.z), f(bx[5], c.z)])
        })
    };
    let mut hits = Vec::new();
    for (a, b, kind) in &rays {
        for c in seg_cells(*a, *b) {
            let live = w.get(c);
            let refb = reference.get(&[c.x, c.y, c.z]).unwrap_or(&air);
            // (a cell holding the reference's own block type in another state is the design
            // operating (a trapdoor open, a rail powered), not a stand-in: stand-ins and overrides
            // change the block type (dust -> lever, air -> glass))
            if live == *refb || block_type(&live) == block_type(refb) {
                continue;
            }
            if clips(&live, *a, *b, c) && !clips(refb, *a, *b, c) {
                hits.push(format!("{kind} ({:.2},{:.2},{:.2}) -> ({:.2},{:.2},{:.2}) clipped at {c} by {} (the reference: {}; a stand-in's cell: bench run != design run)", a.x, a.y, a.z, b.x, b.y, b.z, live.to_string().trim_start_matches("minecraft:"), refb.to_string().trim_start_matches("minecraft:")));
            }
        }
    }
    // (the positive control's cell, ironspec 00:27 (A): the midpoint cell of the first villager ->
    // scarer ray, else (a world with no scarer: station-judge's) of the first villager -> villager
    // ray, whose midpoint cell is not already a full collider, where `--sight-control` lays its
    // glass in the world)
    let mid = |(a, b, _): &(Vec3, Vec3, &str)| {
        let m = Vec3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0);
        BlockPos::new(m.x.floor() as i32, m.y.floor() as i32, m.z.floor() as i32)
    };
    let open = |c: &BlockPos| redsim::behavior::collision_boxes(&w.get(*c)) != vec![[0, 16, 0, 16, 0, 16]];
    let control = rays.iter().filter(|r| r.2 == "villager -> scarer").map(mid).find(open).or_else(|| rays.iter().filter(|r| r.2 == "villager -> villager").map(mid).find(open));
    (hits, rays.len() as u32, control)
}

/// (the direct summon check) A pocket's rider at the start of the due summon tick S.
#[derive(Clone, Debug, Default)]
struct RiderPre {
    seated: bool,
    scared: bool,
    /// Its panic trigger ticks at S: it starts at S, or it runs with S <= its end (at its end + 1
    /// it is timed out without a tick: tick-exact's official skip at s + 61 + 62k).
    ticks: bool,
    /// The current panic run's start, relative to S (its end - 60).
    run_start: Option<i64>,
    wants: bool,
    willing: u32,
    /// (ironspec 06:55 (e)) The villagers counted willing in its 10-box: entity id, position,
    /// whether a pocket rider.
    willing_in_box: Vec<(u32, [f64; 3], bool)>,
    /// (G1 on d96461902 (b)) Its LAST_SLEPT's age at S and its brain's activity at S's start.
    slept_age: Option<i64>,
    activity: String,
    /// Its golem search's columns where a golem can spawn (`golem_spawn_cells_from`: the 17 x 17
    /// columns, each scanned from y + 6 down): on the pad (feet at y -6) and elsewhere.
    spots_pad: u32,
    spots_off: u32,
}

impl RiderPre {
    fn eligible(&self) -> bool {
        self.ticks && self.wants && self.willing >= 3
    }
    fn text(&self, k: usize) -> String {
        if !self.seated {
            return format!("P{k} no seated rider");
        }
        format!(
            "P{k} {}{}, {}, {} willing in its box{}, LAST_SLEPT {}, activity {}, its golem search's spots {} on the pad and {} elsewhere (of 289)",
            if !self.scared { "calm" } else if self.ticks { "panicking, its trigger ticking" } else { "panicking, its trigger skipping S" },
            self.run_start.map_or(String::new(), |s| format!(" (its run from S{s:+})")),
            if self.wants { "wanting" } else { "blocked by its own memory" },
            self.willing,
            if self.willing_in_box.iter().any(|w| !w.2) {
                format!(
                    " ({})",
                    self.willing_in_box.iter().map(|(id, p, rider)| format!("{}#{id} ({:.2},{:.2},{:.2})", if *rider { "pocket rider " } else { "NON-POCKET villager " }, p[0], p[1], p[2])).collect::<Vec<_>>().join(", ")
                )
            } else {
                String::new()
            },
            self.slept_age.map_or("unset".to_string(), |a| format!("S - {a}")),
            self.activity,
            self.spots_pad,
            self.spots_off
        )
    }
}

/// The villager seated in a minecart at the pocket cell, by its entity index.
fn pocket_rider(w: &redsim::world::World, (px, py, pz): (i32, i32, i32)) -> Option<usize> {
    use redsim::entity::mob::types::MobType;
    (0..w.entities.list.len()).find(|&i| {
        let e = &w.entities.list[i];
        !e.removed
            && matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager)
            && e.vehicle.is_some_and(|v| {
                w.entities.list.iter().any(|c| {
                    c.id == v
                        && !c.removed
                        && matches!(c.kind, EntityKind::Minecart(_))
                        && (c.pos.x - (px as f64 + 0.5)).abs() < 0.5
                        && (c.pos.z - (pz as f64 + 0.5)).abs() < 0.5
                        && (c.pos.y - py as f64).abs() < 1.0
                })
            })
    })
}

/// The pockets' riders at the start of the due summon tick (`s` its game time).
fn summon_pre(w: &redsim::world::World, pockets: &[(i32, i32, i32)], s: i64) -> Vec<RiderPre> {
    use redsim::entity::mob::ai::brain::{self, tree::Kind};
    use redsim::entity::mob::types::MobType;
    pockets
        .iter()
        .map(|p| {
            let Some(i) = pocket_rider(w, *p) else { return RiderPre::default() };
            let EntityKind::Mob(m) = &w.entities.list[i].kind else { return RiderPre::default() };
            let Some(b) = m.brain.as_deref() else { return RiderPre { seated: true, ..Default::default() } };
            let scared = b.scared();
            let node = if brain::has_exact(w, i) { brain::ex(w, i).nodes.iter().find(|n| matches!(n.kind, Kind::VillagerPanicTrigger)) } else { None };
            let ticks = scared && node.is_none_or(|n| !n.running || s <= n.end);
            let run_start = node.filter(|n| n.running).map(|n| n.end - 60 - s);
            let bb = w.entities.list[i].bb.inflate(10.0, 10.0, 10.0);
            let in_box = w.get_entities(bb, |e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager && m.brain.as_deref().is_some_and(|b| b.wants_golem(s))));
            let willing = in_box.len() as u32;
            // (ironspec 06:55 (e), cc767b78: every villager counted willing in the rider's 10-box,
            // by entity id with its position and whether it is a pocket rider)
            let riders_now: Vec<usize> = pockets.iter().filter_map(|q| pocket_rider(w, *q)).collect();
            let willing_in_box: Vec<(u32, [f64; 3], bool)> = in_box.iter().map(|&j| {
                let e = &w.entities.list[j];
                (e.id, [e.pos.x, e.pos.y, e.pos.z], riders_now.contains(&j))
            }).collect();
            let cells = redsim::entity::mob::spawn::golem_spawn_cells_from(w, w.entities.list[i].block_position());
            let spots_pad = cells.iter().filter(|(c, r)| r.is_ok() && c.y == -6).count() as u32;
            let spots_off = cells.iter().filter(|(c, r)| r.is_ok() && c.y != -6).count() as u32;
            RiderPre { seated: true, scared, ticks, run_start, wants: b.wants_golem(s), willing, willing_in_box, slept_age: b.last_slept.map(|l| s - l), activity: format!("{:?}", b.activity), spots_pad, spots_off }
        })
        .collect()
}

/// The pocket (0..2) a villager sits at, by its id.
fn pocket_of(w: &redsim::world::World, pockets: &[(i32, i32, i32)], id: u32) -> Option<u8> {
    let i = redsim::entity::index_of(w, id)?;
    pockets.iter().position(|p| pocket_rider(w, *p) == Some(i)).map(|k| k as u8)
}

/// (ironspec Nov 12 20:14) The column bar: every pair of carts in the shared column (entries, exits,
/// passes) at least this many game ticks apart; the y-12 23 and the 46-gt window retired.
pub const COLUMN_BAR: i64 = 16;

/// (ironspec 13:04, the drive-integrity bar) A pocket drive's first powered tick against its clock
/// slot REL + offset k in the bench's counting (the post-tick of iteration dt is game tick
/// T + dt + 1; the lever stand-in leads the slot by the repeater's 1 gt): a bench measurement of
/// the station's own clock, pinned (G1 on 407a29682: not a game constant), the constant every drive
/// must keep within --drive-slack: +1, every pocket in all 10 runs at REL 5 and REL 6 of the
/// record7f12 module rows at 0 / 14 / 24 ("pocket k slot+1"; artifact 1695639112231349, md5
/// 3bc27a8613a4774c70f6ec7ea301eecb, with the edge controls).
pub const DRIVE_SLOT_DELTA: i64 = 1;

/// (the checklist audit (5)) The due summon tick S relative to T on the summon's own clock: the
/// first game tick after T's at game time % 100 == 0 (redsim: the bench's iteration dt runs game
/// tick T + dt + 1, so S runs in iteration S - 1 and its end is that iteration's post-tick state).
pub fn s_rel_of(t_zero: i64) -> i64 {
    let r = t_zero.rem_euclid(100);
    if r == 0 { 100 } else { 100 - r }
}

/// (G1 on a3b306a05 (4)) The station's DECLARED held phases (its header's `held:` lines, read by
/// the hold check instead of the CLI's drive window, which must equal them): the slope's rail held
/// powered REL + 96 ..= REL + 113 (L7 = REL + 96, day-1's 18-gt pulse), the napper's rail REL + 64
/// ..= REL + 73 (L7 - 32, its 10-gt pulse).
pub const HELD_SLOPE: (i64, i64) = (96, 113);
pub const HELD_NAPPER: (i64, i64) = (64, 73);

/// A villager's GOLEM_DETECTED_RECENTLY memory (its ttl), None with no such entity.
fn golem_mem(w: &redsim::world::World, id: Option<u32>) -> Option<Option<i64>> {
    let e = w.entities.list.iter().find(|e| Some(e.id) == id && !e.removed)?;
    match &e.kind {
        EntityKind::Mob(m) => Some(m.brain.as_deref().and_then(|b| b.golem_detected)),
        _ => None,
    }
}

/// A lever stand-in switched toward `on`: returns whether it changed (written; `can_survive`
/// checked at every write).
fn set_lever(w: &mut redsim::world::World, p: BlockPos, on: bool) -> Result<bool, String> {
    let s = w.get(p).to_string();
    if !s.contains("lever") {
        return Err(format!("no lever stand-in at {p}: {s}"));
    }
    let t = if on { s.replace("powered=false", "powered=true") } else { s.replace("powered=true", "powered=false") };
    if t != s {
        let b = Block::parse(&t).map_err(|e| e.to_string())?;
        if !redsim::behavior::can_survive(w, &b, p) {
            return Err(format!("the lever stand-in at {p} can't survive there (ironspec 20:31)"));
        }
        w.set_block(p, b, redsim::world::flags::ALL);
        return Ok(true);
    }
    Ok(false)
}

/// (ironspec 04:07) Whether a lever's wanted state changed since its last call (an edge: the lever
/// must then change; the first call only records it).
fn lever_edge(seen: &mut Vec<(BlockPos, bool)>, p: BlockPos, on: bool) -> bool {
    match seen.iter_mut().find(|(q, _)| *q == p) {
        Some((_, prev)) => {
            let edge = *prev != on;
            *prev = on;
            edge
        }
        None => {
            seen.push((p, on));
            false
        }
    }
}

/// (ironspec Nov 13 04:07: a bench override that doesn't change its cell fails the run) The module's
/// cells (its frame + (6,0,6)) with `extra` laid over them in order, as `module_text` lays them: for
/// each index in `checked` (the stand-ins and `--block` entries), the cell's state before and after
/// it is laid (canonical), and the overrides that changed nothing.
pub fn overlay_report(text: &str, extra: &[(i32, i32, i32, String)], checked: &[usize]) -> Result<(Vec<String>, Vec<String>, Vec<([i32; 3], String)>), String> {
    let d = mctas::design::Design::parse(text).map_err(|e| format!("--module: {e}"))?;
    let mut m = d.translated([6, 0, 6]);
    let (mut lines, mut noops, mut befores) = (Vec::new(), Vec::new(), Vec::new());
    for (i, (x, y, z, st)) in extra.iter().enumerate() {
        let st = if st.contains(':') { st.clone() } else { format!("minecraft:{st}") };
        let before = crate::recordmatch::canon(m.blocks.get(&[*x, *y, *z]).map_or("minecraft:air", |s| s.as_str()));
        let after = crate::recordmatch::canon(&st);
        if checked.contains(&i) {
            let l = format!("({x},{y},{z}) {} -> {}", before.trim_start_matches("minecraft:"), after.trim_start_matches("minecraft:"));
            if before == after {
                noops.push(l.clone());
            }
            lines.push(l);
            if !befores.iter().any(|(c, _): &([i32; 3], String)| *c == [*x, *y, *z]) {
                befores.push(([*x, *y, *z], before.clone()));
            }
        }
        if after == "minecraft:air" {
            m.blocks.remove(&[*x, *y, *z]);
        } else {
            m.blocks.insert([*x, *y, *z], st);
        }
    }
    Ok((lines, noops, befores))
}

/// A design cell's block as the farm loader reads it (`circuit::studies::mobfarm::farm_block`: a
/// nether portal is a terrain block, the rest `Block::parse`).
pub fn farm_block(s: &str) -> Result<Block, String> {
    if s.starts_with("minecraft:nether_portal[") {
        return redsim::terrain::land::block_of(s);
    }
    Block::parse(s).map_err(|e| e.to_string())
}

/// (G1's audit (1)) The sight check's reference from a design of record: its cells in the bench's
/// frame (its frame + (6,0,6)), as the farm loader reads them.
pub fn record_reference(text: &str) -> Result<std::collections::HashMap<[i32; 3], Block>, String> {
    let d = mctas::design::Design::parse(text).map_err(|e| format!("--record: {e}"))?.translated([6, 0, 6]);
    let mut reference = std::collections::HashMap::new();
    for (p, s) in &d.blocks {
        reference.insert(*p, farm_block(s).map_err(|e| format!("--record {p:?} {s}: {e}"))?);
    }
    Ok(reference)
}

/// (G1's audit (1)) The sight check's reference for a station layout's world: the design's cells
/// with the bench's lever stand-ins and their glass (each pocket's (x-2,0,0), (x-2,-1,0); R_1's input
/// (x+2,hold+1,1) and (x+2,hold,1)) back to air, as `station_loop_text` writes the station.
pub fn station_reference(text: &str, lo: DropLayout, hold_y: i32) -> Result<std::collections::HashMap<[i32; 3], Block>, String> {
    let d = mctas::design::Design::parse(text).map_err(|e| e.to_string())?;
    let mut reference = std::collections::HashMap::new();
    for (p, s) in &d.blocks {
        reference.insert(*p, farm_block(s).map_err(|e| format!("the reference's {p:?} {s}: {e}"))?);
    }
    if lo == DropLayout::Station {
        // (the bench's stand-ins, out of the reference: the riser's lever (x-2,0,z), the R line's
        // lever (x+2,17,1) and its glass (x+2,16,1); G1 on 834d6329d (T11): the riser's glass
        // (x-2,-1,z) is day-1's BUILT cell (M16bw), so it stays in the reference)
        for (x, _, z) in lo.pockets() {
            for c in [[x - 2, 0, z], [x + 2, hold_y + 1, z + 1], [x + 2, hold_y, z + 1]] {
                reference.remove(&c);
            }
        }
    }
    Ok(reference)
}

pub fn module_text(text: &str, hold_y: i32, extra: &[(i32, i32, i32, String)], napper: bool, posts: bool, keep_all: bool) -> Result<String, String> {
    let d = mctas::design::Design::parse(text).map_err(|e| format!("--module: {e}"))?;
    let mut m = d.translated([6, 0, 6]);
    // (the bench's own blocks over the module's, in the bench's frame: an option's cells)
    for (x, y, z, st) in extra {
        let st = if st.contains(':') { st.clone() } else { format!("minecraft:{st}") };
        if st == "minecraft:air" {
            m.blocks.remove(&[*x, *y, *z]);
        } else {
            m.blocks.insert([*x, *y, *z], st);
        }
    }
    let station_cart = |e: &mctas::design::EntitySpec| {
        let (x, y, z) = (e.pos[0], e.pos[1], e.pos[2]);
        e.kind.ends_with("minecart") && (z - 6.5).abs() < 0.6 && [5.5, 9.5, 13.5].iter().any(|cx| (x - cx).abs() < 0.6) && (y < 1.0 || (y - hold_y as f64).abs() < 1.0)
    };
    // (the summon check: the module's hostile posts, boats seating a zombie or a pillager, stay)
    let post_boats: Vec<mctas::design::EntitySpec> = if posts && !keep_all {
        m.entities.iter().filter(|e| e.kind.ends_with("boat") && e.nbt.as_deref().is_some_and(|n| n.contains("zombie") || n.contains("pillager"))).cloned().collect()
    } else {
        Vec::new()
    };
    let carts: Vec<mctas::design::EntitySpec> = m.entities.iter().filter(|e| station_cart(e)).cloned().collect();
    if carts.len() != 6 {
        return Err(format!("--module: {} station carts found (6 wanted: the pockets' and the holds' at loop x -1/3/7 z 0)", carts.len()));
    }
    // (with the design of record, every one of its entities stays as it is, record-match comparing
    // them: its posts, its queue's cart; else only the station's carts and, for the summon check,
    // the posts)
    if !keep_all {
        m.entities.retain(|e| station_cart(e));
    }
    // (path (a): the napper's cart with its rider at its hold, loop (9,0,8), the hold's rail;
    // released by the bench at REL + --napper-release; a bench input, declared)
    if napper {
        let mut e = carts[0].clone();
        e.pos = [15.5, 0.0625, 14.5];
        m.entities.push(e);
    }
    m.entities.extend(post_boats);
    let s = m.to_text();
    mctas::design::Design::parse(&s).map_err(|e| format!("--module: the moved file does not parse back: {e}"))?;
    Ok(s)
}

pub fn station_loop_text(file_text: &str, hold_y: i32, header: &[String]) -> Result<String, String> {
    let d = mctas::design::Design::parse(file_text).map_err(|e| e.to_string())?;
    let mut m = d.translated([-6, 0, -6]);
    // (the placement order with the lanes' R_2 in place, as the module's bottom-up order lays it:
    // checked before R_2 leaves this file)
    let with_r2 = m.clone();
    let pockets: Vec<(i32, i32, i32)> = DropLayout::Station.pockets().iter().map(|(x, y, z)| (x - 6, *y, z - 6)).collect();
    for (x, _, z) in &pockets {
        // (the lanes' R_2 rail and the glass under their R_3; R_1's high-side glass (x,17,2) stays:
        // 9fc3155f's rail-floor scan, ironspec 20:31 (4c): R_1 can't stand without it in this part)
        m.blocks.remove(&[*x, hold_y + 2, z + 2]);
        m.blocks.remove(&[*x, hold_y + 2, z + 3]);
        // (the bench's lever stand-ins and their glass: the pocket drive's input (x-2,0,0) (the
        // riser's cells, day-1's) and the R line's input (x+2,17,1) (the controller's))
        m.blocks.remove(&[x - 2, 0, *z]);
        m.blocks.remove(&[x - 2, -1, *z]);
        m.blocks.remove(&[x + 2, hold_y + 1, z + 1]);
        m.blocks.remove(&[x + 2, hold_y, z + 1]);
    }
    // (path (a): the crest's support (-3,12,7) is the lift's cell (7b90c3f1's file, ironspec
    // 16:04), a stand-in in the bench's file only)
    m.blocks.remove(&[-3, 12, 7]);
    m.entities.retain(|e| !(e.kind.ends_with("minecart") && e.pos[1] > hold_y as f64 + 1.0));
    m.meta.insert("positions".into(), pockets.iter().flat_map(|(x, y, z)| [format!("{x},{y},{z}"), format!("{x},{hold_y},{z}")]).collect::<Vec<_>>().join(" "));
    let mut cells = Vec::new();
    for (k, (x, y, z)) in pockets.iter().enumerate() {
        cells.push(format!("P{k} {x},{y},{z}"));
        cells.push(format!("D{k} {x},{y},{}", z + 2));
        cells.push(format!("H{k} {x},{hold_y},{z}"));
    }
    cells.push("PAD0 -3,-6,-6".into());
    cells.push("PAD1 8,-6,-1".into());
    m.meta.insert("run-cells".into(), cells.join("; "));
    let mut s = String::new();
    for l in header {
        s.push_str("# ");
        s.push_str(l);
        s.push('\n');
    }
    // (ironspec 03:37: the placement-order cells, the file's list: alone, and with the lanes' R_2;
    // the detectors' reach, G1)
    let (po, reach) = placement_order_rails(&m)?;
    let (po_r2, _) = placement_order_rails(&with_r2)?;
    let list = |po: &[([i32; 3], String, String)]| if po.is_empty() { "every rail takes its stored shape".to_string() } else { po.iter().map(|(p, st, nat)| format!("({},{},{}) stored {st}, placement gives {nat}", p[0], p[1], p[2])).collect::<Vec<_>>().join("; ") };
    s.push_str(&format!("# Placement order (bottom-up, each rail placed facing its own axis): in this file alone: {}; with the lanes' R_2 rails (x,{},2) laid in the same order (the module): {}\n", list(&po), hold_y + 2, list(&po_r2)));
    s.push_str(&format!(
        "# The detector rule (no rail with 3 potential connections within a detector's reach: the 6 neighbours of its cell and of the cell below, and of the cell above for a sloped one): {}\n",
        if reach.is_empty() { "no rail within any detector's reach".to_string() } else { format!("the rails within reach and their potential connections: {}", reach.join(", ")) }
    ));
    s.push_str(&m.to_text());
    // (farmopt 02:30: the parser refuses whitespace-only rows inside a layer: the file reads back
    // as written, its blocks and entities the same)
    let back = mctas::design::Design::parse(&s).map_err(|e| format!("--write-design: the written file does not parse back: {e}"))?;
    if back.blocks != m.blocks || back.entities.len() != m.entities.len() || back.nether_blocks != m.nether_blocks {
        return Err(format!("--write-design: the written file reads back different ({} blocks / {} entities written, {} / {} read)", m.blocks.len(), m.entities.len(), back.blocks.len(), back.entities.len()));
    }
    Ok(s)
}

/// (ironspec 03:37, the placement-order rule) The rails whose stored shape differs from the shape a
/// player's placement gives them: the design placed block by block in the guide's order (bottom-up:
/// y, then z, then x), each rail set as `getStateForPlacement` gives it (north_south, or east_west
/// for an east-west or east/west-ascending rail) and its `onPlace` run (redsim's `RailState.place`,
/// its neighbours reshaped as the game does). (cell, stored, natural); empty when every rail keeps
/// its shape. The fix for a listed cell: another order, or a temporary helper rail removed after.
pub fn placement_order_rails(d: &mctas::design::Design) -> Result<(Vec<([i32; 3], String, String)>, Vec<String>), String> {
    let mut reach_out: Vec<String> = Vec::new();
    use redsim::block::Block;
    use redsim::pos::BlockPos;
    let (Some(lo), Some(hi)) = (d.blocks.keys().copied().reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]), d.blocks.keys().copied().reduce(|a, b| [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])])) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut w = redsim::world::World::new(BlockPos::new(lo[0] - 2, lo[1] - 2, lo[2] - 2), [hi[0] - lo[0] + 5, hi[1] - lo[1] + 5, hi[2] - lo[2] + 5]);
    let mut cells: Vec<([i32; 3], String)> = d.blocks.iter().map(|(p, s)| (*p, s.clone())).collect();
    cells.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
    let shape_of = |s: &str| s.split(['[', ',', ']']).find_map(|kv| kv.strip_prefix("shape=")).map(str::to_string);
    let mut rails = Vec::new();
    for (p, s) in &cells {
        let placed = match shape_of(s) {
            Some(sh) if s.contains("rail") => {
                let axis = if sh.contains("east") && !sh.contains("north") && !sh.contains("south") { "east_west" } else { "north_south" };
                rails.push((*p, s.clone()));
                s.replace(&format!("shape={sh}"), &format!("shape={axis}"))
            }
            _ => s.clone(),
        };
        // (a block redsim does not model is skipped unless it is a rail: the pad's portal blocks
        // are lit, not placed)
        let b = match Block::parse(&placed) {
            Ok(b) => b,
            Err(e) if placed.contains("rail") => return Err(format!("placement order: {placed}: {e}")),
            Err(_) => continue,
        };
        w.set_block_and_update(BlockPos::new(p[0], p[1], p[2]), b);
    }
    let mut out = Vec::new();
    for (p, s) in rails {
        let now = w.get(BlockPos::new(p[0], p[1], p[2])).to_string();
        if shape_of(&now) != shape_of(&s) {
            out.push((p, s, now));
        }
    }
    // (ironspec 03:54 / G1 04:54, the detector rule: no rail with 3 potential connections within a
    // detector's reach: the 6 neighbours of its cell and the 6 of the cell below, and the 6 of the
    // cell above for a sloped one; every rail there is listed with its count, a breach as a cell)
    let mut reach_lines = Vec::new();
    for (p, s) in &cells {
        if !s.contains("detector_rail") {
            continue;
        }
        let here = BlockPos::new(p[0], p[1], p[2]);
        let sloped = s.contains("ascending");
        let mut centres = vec![here, BlockPos::new(p[0], p[1] - 1, p[2])];
        if sloped {
            centres.push(BlockPos::new(p[0], p[1] + 1, p[2]));
        }
        let mut seen: Vec<BlockPos> = Vec::new();
        for c in centres {
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                let q = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                if q == here || seen.contains(&q) {
                    continue;
                }
                seen.push(q);
                let b = w.get(q);
                if redsim::rails::is_rail(&b) {
                    let n = redsim::rails::RailState::new(q, b.clone()).count_potential_connections(&w);
                    reach_lines.push(format!("({},{},{}) {n}", q.x, q.y, q.z));
                    if n >= 3 {
                        out.push(([q.x, q.y, q.z], format!("a rail within the detector's reach at ({},{},{})", p[0], p[1], p[2]), format!("{n} potential connections (the detector rule)")));
                    }
                }
            }
        }
    }
    if !reach_lines.is_empty() {
        reach_out.extend(reach_lines);
    }
    Ok((out, reach_out))
}

/// The station's box against the budget's (static).
fn station_footprint(hold_y: i32) -> String {
    format!(
        "the station's blocks: file x 3..15 y -9..{hold_y} z 0..7 (loop x -3..9, z -6..1: the glass rings' z +1 cells sit in the lanes' box, as the budget lists them), the waiting riders' heads at {:.2}; the budget's box: loop x -3..9 y -11..19 z -6..0; the departure: file z 7..12 (loop 1..6), the merge row at loop z 7; each exit detector at loop (x,0,2): faces north (x,0,1) a plain rail, south (x,0,3) a plain rail in lane x -1 and an always-on powered rail in lanes x 3 (its booster) and x 7 (its ramp's foot) (path (a), G1: harmless, never switched), east and west air, down glass, up air (no gate or junction)",
        hold_y as f64 + 0.1875 + 1.95
    )
}

/// One drop run's settings.
#[derive(Clone, Debug)]
pub struct DropOpts {
    pub layout: DropLayout,
    pub hold_y: i32,
    /// REL, gt after T: pocket k powered at REL + stagger[k].
    pub rel: i64,
    pub stagger: [i64; 3],
    /// The pockets re-brake (unpowered) at REL + this.
    pub rebrake: i64,
    /// The trapdoors open at T + this (each gated on its pocket's string with `occupancy`).
    pub release: i64,
    /// The trapdoors shut this long after opening (0: they stay open).
    pub pulse: i64,
    pub occupancy: bool,
    /// (the station) Gate each trapdoor instead on its pocket's exit detector (the merge's first
    /// cell) having risen after that pocket's release: the departing cart has left.
    pub exit_gate: bool,
    /// The trapdoor opens this long after its gate is met (G1's brief rule: the hold's release at
    /// the controller's stage 3, 4 gt after stage 1).
    pub open_delay: i64,
    /// L1 (the lead, G1 / ironspec 00:18): the controller and its timers on game ticks through a
    /// freeze or login (a trapdoor can shut while its cart is frozen on it, R_1 switch while the
    /// next batch is frozen); off: L2, the live clock (paused with the entities).
    pub l1: bool,
    /// The cold start (ironspec 00:27): the departing carts absent (true: that pocket starts
    /// empty); with --exit-gate an empty pocket at its release (its string off) is the trapdoor's
    /// evidence.
    pub dep_absent: [bool; 3],
    /// Restarts (cc767b78's `liveness::restart`, ironspec's reload-order rule): at tick T (between
    /// ticks T - 1 and T) a save/stop/start in that entity order (save, reverse, seed:K); the carts
    /// and riders followed by their new ids (`liveness::follow`). The cold start is the fresh
    /// design's row (`dep_absent`, G1).
    pub restarts: Vec<(i64, crate::liveness::Restart)>,
    /// (ironspec 04:53/04:54, the rest bar's mechanism: each summoner panics at T) Scarers for the
    /// panic window: kind ('p' pillager, 'z' zombie) and its feet (file frame), seated in its boat.
    /// Per summoner the bench prints, against the next summon T: in the scarer's type range
    /// (pillager 15, zombie 8) at every tick in T-19..T, and eye to eye (collider clip) within 16
    /// and the 16-box at every tick in T-38..T (the cached line of sight up to 19 gt older).
    pub panic_posts: Vec<(char, Vec3)>,
    /// The rider's follow range (the living scan inflates its box by it: 16 x (1 +- 0.11485),
    /// 14.16..17.84 by its random spawn bonus; G1).
    pub follow_range: f64,
    /// (option (b)) Search a zombie post's cells for this lane (0..2) in the zombie band.
    pub panic_search: Option<usize>,
    /// (ironspec 09:32) The R line through R_1's built driver (the design's repeater, fed by a
    /// redstone block at (x+2,17,1) for the pulse), not a redstone block in R_1's side cell.
    pub r_repeater: bool,
    /// (golem-stamp path (a)) The module laid (the merge row and the shared lift): the departing
    /// carts roll on past the station's exit, each rider tracked against every next-batch caller's
    /// 10-box.
    pub track_lift: bool,
    /// (2c)'s hold (ironspec 14:16): the lever stand-in at the drive's input (day-1's repeater
    /// (-4,12,6)'s input (-4,12,5)), on from REL + `hold_release` for `hold_width` gt, and the
    /// hold rail's cell (the braked slope), in the bench's frame.
    pub hold_drive: Option<(i32, i32, i32)>,
    pub hold_rail: Option<(i32, i32, i32)>,
    pub hold_release: i64,
    pub hold_width: i64,
    /// Path (a)'s napper (ironspec 14:41: the column order lane -1, lane 3, lane 7, the napper):
    /// its cart at its hold (loop (9,0,8)) released at REL + this: the controller's ARM3 output
    /// EMULATED as a lever stand-in at (7,-1,8) for 10 gt, into the module's repeater (8,-1,8) and
    /// the slime (9,-1,8) under the hold rail (the built path, ironspec 17:16).
    pub napper: Option<i64>,
    /// Cells whose unit boxes no cart or rider box may meet (day-1's side drive (b), ironspec
    /// 15:12), in the bench's frame: the ticks any does, counted.
    pub watch: Vec<(i32, i32, i32)>,
    /// (ironspec 19:24..20:03) The direct summon check: the module's hostile posts kept, the
    /// bench's villagers given LAST_SLEPT as the steady runs do, and at the due summon tick S the
    /// stand's riders (willing and ticking at the tick's start) and its `GolemTry` calls recorded;
    /// every golem born is removed after its tick.
    pub summon_check: bool,
    /// The summon check's control: pocket k's arriving rider removed before the warm-up (the row
    /// must show the miss).
    pub remove_rider: Option<usize>,
    /// (ironspec 21:25 (4)) The declared stops and buffers (cells, the bench's frame) the carts may
    /// meet: any other block a cart's move is stopped by fails the row.
    pub stops: Vec<(i32, i32, i32)>,
    /// The declared contacts on PROVISIONAL declarations (bench evidence only), printed as such.
    pub provisional: Vec<(i32, i32, i32)>,
    /// (ironspec 22:03, the pocket drive's control) Pocket k's lever stand-in never switched on: its
    /// cart must stay in its pocket.
    pub lever_off: Option<usize>,
    /// (ironspec 00:27 (A)) The sight check's positive control: a glass stand-in support laid in the
    /// world at T on the first villager -> scarer ray; the row's own check must flag it.
    pub sight_control: bool,
    /// The hold check's positive control (G1 on 82f9546f (c), rule (A)): a real neighbour update to
    /// the held rail itself at REL + K; the bench's undriven rail drops and the check must fail.
    pub hold_poke: Option<i64>,
    /// (record7e, ironspec 21:43/22:37) The pockets' lever stand-ins at these cells (bench frame):
    /// the design of record's own line ends, each lever replacing its dust on the record's own
    /// support and powering the dust beside it; None: the bench's (x-2,0,0) with day-1's repeater
    /// stand-in (x-1,0,0), each on glass.
    pub pocket_levers: Option<[(i32, i32, i32); 3]>,
    /// The napper's rail held powered (a `held:` stand-in, ironspec 01:07) from REL + K + 1 for the
    /// pulse's 10 gt at this cell (bench frame), instead of the lever stand-in at (7,-1,8).
    pub napper_held: Option<(i32, i32, i32)>,
    /// (G1's audit (1)) The sight check's reference: the module of record's cells (bench frame),
    /// every stand-in as its built cell; the live world is read against it. None: no sight check.
    pub sight_ref: Option<std::sync::Arc<std::collections::HashMap<[i32; 3], Block>>>,
    /// (the checklist audit (7), controls) Departing rider K removed at T without `dep_absent`
    /// (the missing-rider count must fire); every golem spawn spot of the callers blocked at T with
    /// glass (the golem rate must fail).
    pub remove_departing: Option<usize>,
    pub rate_control: bool,
    /// (ironspec 06:55 (c)) The foreign-caller control: every pocket rider dismounted 5 gt before
    /// S (it keeps its brain, its panic and its LAST_SLEPT, standing beside its cart): its calls at
    /// S are not the stand's, so the gate must read a miss while those calls happen.
    pub foreign_caller: bool,
    /// (G1 on a3b306a05 (3)) The reseat control: a fresh villager seated in pocket 0's cart 40 gt
    /// before S, the old rider dismounted beside it.
    pub reseat: bool,
    /// (G1 on a3b306a05 (1)) The fluid bar's control: the first fluid cell left unscheduled.
    pub fluid_control: bool,
    /// (farmopt 09:46) The T-shape bar's control: a lever planted beside the first station T at T
    /// and switched on at T+5, off at T+10.
    pub t_control: bool,
    /// (G1 on 834d6329d (T3)) The t1 activity check's control: rider 0's activity set off its
    /// schedule before the read.
    pub activity_control: bool,
    /// (G1 10:10, the T-shape bar's negative row) The same lever planted and switched on / off
    /// beside the first station T, with the T's constant source laid (form (C)): the T must NOT
    /// re-shape; the row bites when the lever switched and the T held.
    pub t_hold: bool,
    /// (ironspec 13:04) The drive-integrity bar's control: pocket K's drive fired D gt late.
    pub late_drive: Option<(usize, i64)>,
    /// (ironspec 13:04) The drive-integrity bar's slack j: each pocket drive's first powered tick
    /// within j of its clock slot REL + offset k (+ the bench's measured constant).
    pub drive_slack: i64,
    /// (ironfarm 11:56, ironspec's forced-V row) Ridden carts (a villager in each) planted at
    /// T + k: (the cart's position, k).
    pub plant_carts: Vec<([f64; 3], i64)>,
    /// (ironfarm 11:56) Blocks planted at T + k (with their updates): (cell, state, k).
    pub plant_blocks: Vec<(BlockPos, Block, i64)>,
    /// (ironfarm's C-fix rows) Cells no cart may leave westward (each lane's C at the lip): a cart
    /// crossing out of one through its west face fails ("a cart left a watched cell westward"); a
    /// cell no cart entered is NOT JUDGED.
    pub watch_west: Vec<BlockPos>,
    /// (ironfarm's C-fix rows) A cart at or above this y at rest by ironspec 10:42's stall rule with
    /// no rail in its cell nor the cell below fails ("a cart at rest off its rails"); no cart at
    /// rest there is NOT JUDGED.
    pub offrail_y: Option<f64>,
    /// (ironspec 08:23) The run's day time at its start (RunOpts' day0; None: steady's random day
    /// time in 10..12,000): a night value (12,000..23,999) samples the riders' REST entries.
    pub day0: Option<i64>,
    /// (G1 on 44d4bbfae) The placement count's positive control: the world built this many cells
    /// short of the design's top (0: the design's box).
    pub box_short: i32,
    /// The trapdoors' timer on world ticks (a plain redstone clock running through a freeze), not
    /// the live clock: the positive control for the strings.
    pub release_world: bool,
    /// Each pocket re-brakes `rebrake` after its own release (REL + stagger), not after REL: Q2's
    /// lanes, where no other lane's cart crosses a pocket.
    pub rebrake_each: bool,
    /// Scarers for the night-panic coverage print (ironspec 22:39): ('z' a zombie, 'p' a
    /// pillager, its feet), in the design's frame; panic when within 8 (zombie) / 15 (pillager)
    /// and in sight eye to eye (`LivingEntity.hasLineOfSight`, COLLIDER).
    pub scarers: Vec<(char, Vec3)>,
    /// (the station with `design_full`'s refill) The R line: every lane's R_1 powered from its side
    /// (x+1, hold+1, z+1) for (.1) gt starting (.0) gt after the last trapdoor shut again (needs
    /// `pulse`): the next batch rolls into the holds.
    pub refill: Option<(i64, i64)>,
    pub ticks: i64,
    pub seed: i64,
    pub freeze: Option<(i64, i64)>,
    /// Logins (ironfarm's `--login T,D`): more level-32 freezes in the same run, the entities
    /// frozen for D gt from T while the redstone runs.
    pub logins: Vec<(i64, i64)>,
    pub trace: bool,
}

/// (the direct summon check, ironspec 19:24..20:03) One `GolemTry` call at a due summon tick: the
/// game's `spawnGolemIfNeeded` past its threshold (the caller wanting a golem, enough willing in
/// its 10-box).
#[derive(Clone, Debug, Default)]
pub struct SummonCall {
    /// S - T.
    pub s: i64,
    /// The stand (0: this station, the bench's only one).
    pub stand: u8,
    /// The caller's pocket (0..2); None: a villager not seated at a pocket.
    pub pocket: Option<u8>,
    pub willing: u32,
    /// 3 (the panic path) or 5 (gossip), once redsim's `GolemTry` carries it.
    pub threshold: Option<u32>,
    /// The spot tries used, once redsim's `GolemTry` carries them.
    pub tries: Option<u32>,
    /// A golem placed (else a spot miss: trySpawnMob found no spot).
    pub born: bool,
}

/// A due summon tick's outcome for the stand (its three pockets together, one summon per S).
#[derive(Clone, Debug, Default)]
pub struct SummonDue {
    /// S - T.
    pub s: i64,
    /// (ironspec 13:37) The S after a freeze over the first S (S + 100): the recovery bar's S.
    pub recovery: bool,
    /// The pockets' riders at the tick's start seated in their pocket, wanting a golem (LAST_SLEPT
    /// within 24,000, no GOLEM_DETECTED_RECENTLY), with >= 3 willing in their 10-box, and whose
    /// panic trigger ticks at S (it starts at S, or runs with S <= its end: at its end + 1 it is
    /// timed out without a tick, tick-exact's official skip at s + 61 + 62k).
    pub eligible: u8,
    /// The pockets' riders' `GolemTry` calls in the tick (up to the first success).
    pub calls: u8,
    pub born: bool,
    /// Eligible riders that made no call because this S's golem stamped them first.
    pub covered: u8,
    /// Exactly one of: golem born, spot miss, caller blocked by its own memory, fewer than 3
    /// willing, no panicking caller (or no call, unexplained).
    pub class: &'static str,
    /// Per pocket at the tick's start: seated, scared, its trigger ticking, wanting, the willing in
    /// its box.
    pub riders: Vec<String>,
    /// Each pocket rider's golem-search spots at S: on the pad and elsewhere (of 289 columns).
    pub spots_pad: [u32; 3],
    pub spots_off: [u32; 3],
    /// The run's restart before S (T + its tick), if any: a reload re-phases the panic (ironspec
    /// 20:07: a miss there counts against the one-summon-per-disturbance bar).
    pub restart_before: Option<i64>,
    /// (ironspec 06:55 (e), cc767b78) The villagers counted willing in the pocket riders' 10-boxes
    /// at S's start (each once): entity id, position, whether a pocket rider.
    pub willing_in_box: Vec<(u32, [f64; 3], bool)>,
    /// (ironspec 06:55 (c)) GolemTry calls at S by villagers that are not the stand's pocket riders
    /// (never counted as the stand's calls nor its golem).
    pub foreign_calls: u8,
    /// (G1 on a3b306a05 (3)) Every GolemTry at S: the caller's entity id and the pocket it was
    /// attributed to (from the world: the villager seated in that pocket's cart), if any.
    pub calls_by: Vec<(u32, Option<u8>)>,
}

/// One drop run's figures for a grid's summary.
#[derive(Clone, Debug, Default)]
pub struct DropStats {
    /// (ironspec 14:47 (f)) The run had a freeze, a login or a restart (not a normal-operation row).
    pub disturbed: bool,
    /// The farthest a waiting cart sat from its cell's centre at the release (hundredths).
    pub spread: i64,
    /// The longest fall (gt; i64::MAX: a cart never landed).
    pub fall: i64,
    /// The least lead of a landing over its pocket's re-brake (gt; negative: it landed powered).
    pub brake_lead: Option<i64>,
    /// The least rest margin before the next summon (i64::MIN: not all at rest).
    pub rest: i64,
    pub wrong: usize,
    pub contacts: usize,
    pub contact_max: f64,
    /// The closest an arriving cart came to a departing one (hundredths).
    pub closest: Option<i64>,
    pub hurt: usize,
    pub unseated: usize,
    /// Departing carts that never reached the exit.
    pub stuck: usize,
    /// The longest a trapdoor waited past T + release on its pocket's string (gt).
    pub waited: i64,
    /// The waiting riders' lowest feet at the release (hundredths).
    pub feet: i64,
    /// With scarers: the longest stretch (gt) a rider went without a scarer in range and sight, per
    /// phase: [the hold, the fall, the pocket (resting or landed), the departure]; the nearest a
    /// pillager came to a villager it could see (hundredths); the nearest zombie to a rider it
    /// covered (hundredths).
    pub calm: Option<[i64; 4]>,
    pub pillager_near: Option<i64>,
    pub zombie_near: Option<i64>,
    /// The nearest villager to a zombie, in sight or not (hundredths), and the longest uncovered
    /// stretch in each lane's hold (lanes front first: loop x -1, 3, 7).
    pub zombie_any: Option<i64>,
    pub hold_lane: [i64; 3],
    /// With the refill: next-batch carts not at rest in their hold (fallen through or stuck).
    pub refill_fail: usize,
    /// (ironspec 09:32) R_1's launches after their rail's power ended, or none; the longest lag from
    /// the rail's power to its cart's first move; the shortest power a rail got.
    pub r1_late: usize,
    /// (G1 on b4549f35a) The same keyed on leaving R_1's cell (ironfarm's and tick-exact's rows).
    pub r1_cell_late: usize,
    pub r1_lag: Option<i64>,
    pub r1_len: Option<i64>,
    /// The closest zombie-to-villager approach (its distance, and where: the tick, the lane and
    /// phase, the rider's feet, the zombie), for the bridge check.
    pub zombie_any_at: Option<(f64, String)>,
    /// The R line's first tick, the tick the last next-batch cart came to rest in its hold, the
    /// last trapdoor's shut tick (world ticks after T).
    pub r_from: Option<i64>,
    pub next_in: Option<i64>,
    pub shut_last: Option<i64>,
    /// The falling carts' largest horizontal offset from their shaft's centre line (trapdoor
    /// opening to landing), its largest component toward a zombie post's seat (tick-exact's void
    /// condition for the 6.08), and the last departing cart's tick out of the station (loop z 6.9).
    pub fall_off: f64,
    pub fall_toward: Option<f64>,
    pub last_out: Option<i64>,
    /// Villagers in a shaft or on a departure rail at a summon tick (gameTime % 100 == 0, before
    /// and after it): `positions:` lists the pockets and the holds only on that timing (required 0).
    pub summon_off: usize,
    /// (ironspec 11:05's audit) Cells set outside the world's box (dropped), the placement included.
    pub oob: usize,
    /// (golem-stamp path (a)) Per departing lane: the last tick (to T+100) its rider was inside a
    /// next-batch caller's 10-box, inside at T+100, the lift's entry, its feet at y 12.0125.
    pub lift_last: [Option<i64>; 3],
    pub lift_y80: [Option<f64>; 3],
    pub lift_in100: [bool; 3],
    pub lift_entry: [Option<i64>; 3],
    pub lift_y12: [Option<i64>; 3],
    /// Each lane's exit from the lift's column at its top: (tick, y, vertical speed).
    pub lift_top: [Option<(i64, f64, f64)>; 3],
    /// (2c)'s hold: the release tick; the held cart's first rest (tick, x and feet, the bench's
    /// frame) and its largest drift from it while held; whether it entered the column before the
    /// release; its entry after lane 1's pass at y 12 (gt); ticks the hold rail wasn't a dry rail.
    pub hold_on: Option<i64>,
    pub hold_rest: Option<(i64, f64, f64)>,
    pub hold_drift: f64,
    pub hold_overshoot: bool,
    pub hold_gap: Option<i64>,
    pub hold_wet: u32,
    /// The departing carts past their station's exit: the least horizontal clearance between two
    /// whose boxes overlap in y, and the ticks with a push contact (a box inflated by 0.2
    /// horizontally meeting another's, the game's pushEntities test), the first one's tick and lanes.
    pub lift_clear: Option<f64>,
    pub lift_push: u32,
    pub lift_push_at: Option<(i64, usize, usize)>,
    /// The napper (lane 3 in the contact list): off its hold, into the lift, its pass at y 12 and
    /// its exit at the lift's top (ticks after T).
    pub nap_off: Option<i64>,
    pub nap_entry: Option<i64>,
    pub nap_y12: Option<i64>,
    pub nap_top: Option<i64>,
    /// Each departing rider's least air supply (of 300) from its station's exit on, and the
    /// napper's (ironspec 15:08: the bar hurt 0).
    pub lift_air: [Option<i32>; 4],
    /// The first tick a rider was back in a caller's box after it was out (lane 2's dip at its
    /// column entry; ironspec 16:46: after the next summon by >= 2 gt).
    pub lift_dip: [Option<i64>; 3],
    /// (the direct summon check, ironspec 19:24..20:03) Every `GolemTry` at the due summon tick.
    pub summon_calls: Vec<SummonCall>,
    /// The due summon tick's outcome (S = T + 100; the bench is one cycle).
    pub summon_due: Vec<SummonDue>,
    /// Golems born and removed after their tick (the portals' removal is golem-flow's).
    pub golems_removed: u32,
    /// (ironspec 20:10 (2) / 23:30 / 00:27) The sight rays crossing a lever stand-in's support (the
    /// first few, every tick T..S); whether this row is the positive control (its cell laid).
    pub sight_hits: Vec<String>,
    pub sight_control: bool,
    /// The sight crossings over T..S (every tick, at its end).
    pub sight_hit_count: u32,
    /// (G1 on 82f9546f) Departing riders missing from the lift tracking (removed or never spawned)
    /// without `dep_absent`: a failure.
    pub missing_riders: u32,
    /// (G1 on 82f9546f (c)) Ticks whose end found the held slope rail off its window's state (a
    /// failure), the first of them, and the neighbour updates made at its first on and off edges.
    pub hold_breaks: u32,
    pub hold_break_first: Option<String>,
    pub held_edges: Vec<String>,
    /// (record-match) Every lever stand-in's cell with its bench-checked update and power reach
    /// (`lever_reach_check`'s verdict; survival checked at every write).
    pub standin_cells: Vec<([i32; 3], String)>,
    /// (ironspec 00:27) The laid design's fluid cells and the fluid ticks the bench scheduled for
    /// them before the world settled (0 scheduled with a fluid cell: a failure).
    pub fluid_cells: u32,
    pub fluid_scheduled: u32,
    /// (the checklist audit, ironspec 04:38 (5)) The summon's own clock: S - T, the first tick after
    /// T at which game time % 100 == 0; the stand's first GolemTry tick (S - T), which must equal it.
    pub s_rel: i64,
    pub s_try: Option<i64>,
    /// (the audit (2)) Per departing lane: tracked against the LIVE callers' 10-boxes (the arriving
    /// riders' boxes each tick, (6)), and the ticks it was tested (T..S); 0 while tracked: NOT JUDGED.
    pub lift_tracked: [bool; 3],
    pub lift_samples: [u32; 3],
    /// Whether the laid world holds the shared lift (a bubble column): its verdicts are in scope.
    pub lift_in_world: bool,
    /// What the run drove (the scope of the verdicts that depend on it).
    pub track_lift: bool,
    pub hold_driven: bool,
    pub napper_run: bool,
    pub napper_held: bool,
    pub summon_checked: bool,
    /// The sight check's scope (a stand-in or control cell differing from the reference) and the
    /// rays it judged (every tick T..S, after each tick).
    pub sight_scope: bool,
    pub sight_rays: u32,
    /// The held rails' edges made (the rail-T check runs at each; 0 with a held rail: NOT JUDGED).
    pub held_edge_count: u32,
    /// (ironspec 04:07) Scheduled lever edges that found their lever already in the target state.
    pub lever_noops: u32,
    /// (the audit (6)) Departing riders whose GOLEM_DETECTED_RECENTLY this S's summon set (read
    /// from their memory after S's tick): a failure.
    pub dep_stamped: u32,
    /// (G1 item 9) Station riders whose activity at T differs from their schedule's (PANIC allowed
    /// while scared): a failure.
    pub activity_wrong: u32,
    /// A run with no freeze, no login and no restart: one independent sample of the golem rate
    /// (freeze, login and restart rows share their seed's draws).
    pub independent: bool,
    /// (ironspec 08:19) The station riders holding HOME at T (the design's `port home`).
    pub home_holders: u32,
    /// (ironspec 08:23, G1 08:22) The station riders' entries into REST over the run (the first
    /// tick's included), and the first few with their stamps: a printed count until b8949c96's
    /// hook lands.
    pub rest_entries: u32,
    pub rest_list: Vec<String>,
    /// (G1 on a3b306a05 (8)) Id-keyed reads that found no entity (a station rider or caller gone
    /// unexpectedly): a failure, never a skip.
    pub missing_reads: u32,
    /// (G1 on a3b306a05 (5)) GolemTry calls in the world over the run, whether or not the summon
    /// check runs: summons in the world with no check is NOT JUDGED.
    pub summon_events: u32,
    /// (ironspec 09:03) The least distance from a HOME holder's feet to its HOME head's centre.
    pub home_least: Option<f64>,
    /// (G1 on a3b306a05 (3)) The reseat control's fresh villager, the dismounted old rider, and
    /// their attributions at S through the gate's `pocket_of` (fresh, old).
    pub reseat_id: Option<u32>,
    /// (G1 on a3b306a05 (8)) The station riders that left a world with no lift (their later reads
    /// out of that world's scope, printed).
    pub riders_left: u32,
    pub reseat_old: Option<u32>,
    pub reseat_attr: Option<(Option<u8>, Option<u8>)>,
    /// The run's seed (the golem rate's samples: one independent row per seed).
    pub seed: i64,
    /// (farmopt 09:46) The world's Ts at T (plain rails with >= 3 potential connections) and how
    /// many re-shaped during the run: a failure.
    pub t_count: u32,
    pub t_reshaped: u32,
    pub t_list: Vec<String>,
    /// (G1 on 834d6329d (T1)) The run stopped at a held rail's driver edge reaching a rail T.
    pub rail_t_stop: bool,
    /// (G1 10:10) The T-shape controls' lever, when planted.
    pub t_lever: Option<String>,
    /// (G1 12:27) Each lane's hold string's first press after the R line starts.
    pub hold_press: [Option<i64>; 3],
    /// (ironspec 13:00) The least lead of a hold string's last release before the R line rises.
    pub hold_release_lead: Option<i64>,
    /// (G1 on e2d9333bc (B2)) The hold-string lead bar's failures (a lead under 2 gt, no release
    /// before the rise, pressed at the rise; or NOT JUDGED when the R line never rose).
    pub hold_lead_bad: Option<String>,
    /// (ironspec 13:04) Pocket drives off their clock slot (the drive-integrity bar).
    pub drive_off: u32,
    /// (G1 on 407a29682 (N4)) The --lever-off pocket's drive that never fired (the control's own
    /// collateral, apart from the bar's other pockets).
    pub drive_off_control: u32,
    /// (ironfarm's C-fix rows) Carts that left a watched cell westward; carts at rest off their
    /// rails above the off-rail line.
    pub west_leaves: u32,
    pub offrail: u32,
    /// (G1 on 407a29682 (N1)) A watched cell no cart entered; no cart at rest at or above the
    /// off-rail line: each check NOT JUDGED, a failure.
    pub watch_unseen: bool,
    pub offrail_unseen: bool,
    /// (ironspec 21:25 (4)) The carts' block contacts (a move stopped horizontally by a block with a
    /// collision shape): per cell, its block, the ticks, and whether it is a declared stop.
    pub block_contacts: Vec<(BlockPos, String, u32, bool)>,
    /// The ticks with an undeclared contact (a failure).
    pub undeclared_contacts: u32,
    /// The watched cells: ticks a cart's or rider's box met one, the first (tick, cell).
    pub watch_hits: u32,
    pub watch_first: Option<(i64, (i32, i32, i32))>,
    /// The panic window per summoner (lane): the best scarer's index, the margin before T-19 of
    /// its range run, the margin before T-38 of its sight run (negative: the window fails), and
    /// the earliest failing tick in each window (after T).
    pub panic: [Option<(usize, i64, i64, Option<i64>, Option<i64>)>; 3],
    /// The game's trigger per summoner: the sensor phase pairs (of 400) in which it calls the
    /// summon at T, and the wanting villagers in its 10-box; the least caller count (the summoners
    /// that call in every phase pair).
    pub panic_calls: [Option<(usize, usize)>; 3],
    pub panic_callers: Option<usize>,
    /// Per lane (front first: loop x -1, 3, 7), from its launch tick REL + offset: the tick its
    /// departing cart leaves the pocket's rest, the tick it leaves the station (loop z 6.9) and its
    /// speed there.
    pub lane_times: [Option<(i64, i64, f64)>; 3],
    /// The closest a pocket's departing cart was to its pocket's centre (horizontal, hundredths) as
    /// that pocket's trapdoor opened (i64::MAX: already gone).
    pub gap_at_open: Option<i64>,
    /// The least spacing between consecutive launches (gt, world).
    pub launch_gap: Option<i64>,
    /// Trapdoors shut onto their cart (the cart still in the hold), and R line switches while the
    /// entities were frozen.
    pub shut_onto: usize,
    pub r_frozen: usize,
}

/// The closest zombie-to-villager approach's place, for the bridge check: the tick after T, the
/// lane (front first: loop x -1, 3, 7), whose rider (this batch or the next) and in which phase,
/// the rider's feet and the zombie's seat (file frame).
fn closest_text(a: (i64, usize, Vec3, Vec3, Option<usize>)) -> String {
    let (dt, slot, p, z, phase) = a;
    let lane = if slot >= 6 { slot - 6 } else { slot % 3 };
    let who = if slot >= 6 { "the next batch's" } else if slot >= 3 { "the arriving" } else { "the departing" };
    let ph = match phase {
        Some(0) => "in its hold",
        Some(1) => "falling",
        Some(2) => "in its pocket",
        Some(3) => "departing",
        _ => "on R_1",
    };
    format!(" (T+{dt}, {who} rider of lane {lane} {ph}, feet {:.4},{:.4},{:.4}; the zombie {:.4},{:.4},{:.4})", p.x, p.y, p.z, z.x, z.y, z.z)
}

fn summon_boundary(frozen: &[bool]) -> i64 {
    (1..10i64).map(|k| 100 * k).find(|b| (*b as usize) >= frozen.len() || !frozen[*b as usize]).unwrap_or(100)
}

/// (option (b)) A candidate zombie post for one lane: its cell (file frame), its open side, its
/// seat and eye, the seat's distance to the rider at rest, the per-tick (in range, in the
/// 16-box, seen through the open face) against the lane's arriving rider, and the nearest any
/// rider came.
struct Cand {
    cell: [i32; 3],
    open: char,
    seat: Vec3,
    eye: Vec3,
    rest_d: f64,
    log: Vec<(bool, bool, bool)>,
    nearest: f64,
}

impl Cand {
    /// Whether the sight line from the zombie's eye to `to` leaves its box through the open face
    /// (not the floor, the roof or a wall): the face the segment crosses first.
    fn exits_open(&self, to: Vec3) -> bool {
        let (x0, y0, z0) = (self.cell[0] as f64, self.cell[1] as f64, self.cell[2] as f64);
        let d = (to.x - self.eye.x, to.y - self.eye.y, to.z - self.eye.z);
        let t = |p: f64, lo: f64, hi: f64, dv: f64| if dv > 0.0 { (hi - p) / dv } else if dv < 0.0 { (lo - p) / dv } else { f64::INFINITY };
        let (tx, ty, tz) = (t(self.eye.x, x0, x0 + 1.0, d.0), t(self.eye.y, y0, y0 + 2.0, d.1), t(self.eye.z, z0, z0 + 1.0, d.2));
        if ty <= tx.min(tz) {
            return false;
        }
        let face = if tx < tz {
            if d.0 > 0.0 { 'e' } else { 'w' }
        } else if d.2 > 0.0 {
            's'
        } else {
            'n'
        };
        face == self.open
    }
}

/// The game's trigger over every sensor phase pair for one scarer's per-tick (in range, in the
/// box, seen) log (as the panic measure's): the pairs calling at T = `tn`, the panic start's
/// earliest and latest, and the window to `dep` (leaving the pocket's rest), least and most.
fn trigger_summary(log: &[(bool, bool, bool)], tn: usize, dep: Option<i64>) -> (usize, Option<i64>, Option<i64>, Option<i64>, Option<i64>) {
    let mem = |t: usize, pl: usize| -> bool {
        let u = t - (t + 20 - pl) % 20;
        log[u].1 && (u..=t).all(|q| log[q].2) && log[t].0
    };
    let (mut calls, mut hmin, mut hmax, mut wmin, mut wmax) = (0usize, None::<i64>, None::<i64>, None::<i64>, None::<i64>);
    for pl in 0..20 {
        for ph in 0..20 {
            let tstar = tn - (tn + 20 - ph) % 20;
            if !mem(tstar, pl) {
                continue;
            }
            let mut h = tstar;
            while h >= 40 && mem(h - 20, pl) {
                h -= 20;
            }
            if h < 40 || (tn >= h + 61 && (tn - h - 61) % 62 == 0) {
                continue;
            }
            calls += 1;
            let h = h as i64;
            hmin = Some(hmin.map_or(h, |x| x.min(h)));
            hmax = Some(hmax.map_or(h, |x| x.max(h)));
            if let Some(d) = dep {
                wmin = Some(wmin.map_or(d - h, |x| x.min(d - h)));
                wmax = Some(wmax.map_or(d - h, |x| x.max(d - h)));
            }
        }
    }
    (calls, hmin, hmax, wmin, wmax)
}

/// (ironspec 11:05's audit, G1 on 44d4bbfae) The design's cells a run's placement drops: those
/// outside the world's box (the cells `place_raw` cannot write), the overworld's and the Nether
/// part's, read structurally (not from redsim's records).
pub fn placement_drops(spec: &FarmSpec, w: &redsim::world::World) -> usize {
    let ow = spec.blocks.iter().filter(|(p, _)| !w.contains(*p)).count();
    let nether = spec.nether.as_ref().zip(w.nether.as_deref()).map_or(0, |(ns, n)| ns.placed().iter().filter(|(p, _)| !n.contains(*p)).count());
    ow + nether
}

/// redsim's records of non-air writes outside the world's box (each one dropped).
pub fn oob_records(w: &redsim::world::World) -> usize {
    w.errors.iter().filter(|e| e.starts_with("setBlock outside the box")).count()
}

/// The spec a run builds its world from: the design's, or (the placement count's positive control)
/// `short` cells short of the design's top.
pub fn spec_for_run(spec: &FarmSpec, short: i32) -> FarmSpec {
    let mut s = spec.clone();
    s.hi.y -= short.max(0);
    s
}

/// One drop run: the report lines and the figures.
pub fn bench(spec_in: &FarmSpec, o: &DropOpts) -> Result<(Vec<String>, DropStats), String> {
    let steady = RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random);
    let ro = RunOpts { ai: true, day0: o.day0.unwrap_or(steady.day0), ..steady };
    let spec_run = spec_for_run(spec_in, o.box_short);
    let spec = &spec_run;
    let mut run = Run::start(spec, &ro)?;
    // (ironspec 14:57: a bench turns the natural spawner off explicitly, its scope printed, never a
    // light-read default: the bench runs no terrain site and no natural spawning)
    run.world.mob.natural = None;
    // (G1 10:10, the T-shape bar's negative row: the first station T's constant source, form (C)
    // (ironspec 10:24: slime under the T, a lit torch under the slime, on glass; build: torch last),
    // laid BEFORE the settle, so the T holds its powered shape at T and through the lever's edges)
    let mut t_hold_overrides: Vec<String> = Vec::new();
    if o.t_hold {
        let (px, _, pz) = o.layout.pockets()[0];
        let t = BlockPos::new(px, 0, pz + 7);
        let below = |k: i32| BlockPos::new(t.x, t.y - k, t.z);
        for (c, s) in [(below(3), "minecraft:glass"), (below(1), "minecraft:slime_block"), (below(2), "minecraft:redstone_torch[lit=true]")] {
            let before = run.world.get(c);
            let after = Block::parse(s).map_err(|e| e.to_string())?;
            run.world.set_block(c, after.clone(), redsim::world::flags::ALL);
            // (G1 on 407a29682 (N5): each source cell read back)
            if run.world.get(c) != after {
                return Err(format!("--t-hold-control: the source cell {c} reads back {} (wanted {})", run.world.get(c), after));
            }
            t_hold_overrides.push(format!("{c} {} -> {}", before.to_string().trim_start_matches("minecraft:"), s.trim_start_matches("minecraft:")));
        }
    }
    let place_drop = placement_drops(spec_in, &run.world);
    let rec_place = oob_records(&run.world);
    // (ironspec 00:27: every fluid cell of the laid design gets its tick scheduled, as the game's
    // placement of it schedules one (FlowingFluid's tick delay), before the world settles: a water
    // layout that is not stable as laid shows; the count printed, 0 with water in the design a
    // failure)
    let (mut fluid_cells, mut fluid_scheduled) = (0u32, 0u32);
    let mut lift_in_world = false;
    {
        let w = &mut run.world;
        let (min, size) = (w.min, w.size);
        let mut cells: Vec<(BlockPos, redsim::fluid::Fluid)> = Vec::new();
        for x in min.x..min.x + size[0] {
            for y in min.y..min.y + size[1] {
                for z in min.z..min.z + size[2] {
                    let p = BlockPos::new(x, y, z);
                    let f = redsim::fluid::fluid_state(w, p);
                    if !f.is_empty() {
                        cells.push((p, f.fluid));
                        // (the shared lift: a bubble column; its verdicts are then in scope)
                        if matches!(w.get(p), Block::BubbleColumn { .. }) {
                            lift_in_world = true;
                        }
                    }
                }
            }
        }
        // (G1 on a3b306a05 (1): the count of scheduled cells is read back from the SCHEDULER, each
        // fluid cell's pending tick, not from the loop that scheduled it; `--fluid-control` leaves the
        // first cell unscheduled, and the bar must fail)
        let mut first = true;
        for (p, f) in &cells {
            fluid_cells += 1;
            if o.fluid_control && first {
                first = false;
                continue;
            }
            first = false;
            let delay = redsim::fluid::tick_delay(w, *f);
            w.schedule_fluid_tick(*p, *f, delay);
        }
        fluid_scheduled = cells.iter().filter(|(p, f)| w.fluid_ticks.has_scheduled_tick(*p, *f)).count() as u32;
    }
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    // (path (a): the napper's cart at its hold, loop (9,0,8), apart from the station's carts)
    let mut napper_cart: Option<u32> = if o.napper.is_some() {
        run.world.entities.list.iter().find(|e| matches!(e.kind, EntityKind::Minecart(_)) && (e.pos.x - 15.5).abs() < 0.2 && (e.pos.z - 14.5).abs() < 0.2).map(|e| e.id)
    } else {
        None
    };
    if o.napper.is_some() && napper_cart.is_none() {
        return Err("--napper-release: no cart at the napper's hold (loop (9,0,8))".into());
    }
    // (ironspec 14:47 (g): the station's carts by their declared cells: each pocket's column, from the
    // pocket up to its hold and the R line's queue over R_1 (loop z 0..2.5); the design of record's
    // other carts (its queue's cart, loop (9.5,-13.44,-3.5)) sit in no pocket column, whatever the
    // frame. G1 on e2d9333bc (B13), the bounds: x within 0.6 of the column's centre (a cart's
    // half-width 0.49, plus 0.11 for its rest spread); z from the pocket cell's north face - 0.1 (a
    // pocket or hold cart's box starts at its cell's z, the hold cart at 0.49 against the north
    // glass) to + 2.5 (R_1's queue cart at z 1.30 / slot 2 at 1.5, + its half-width); y from the
    // pocket's y - 1 (the pocket cart at y + 0.0625) to hold_y + 3 (R_1's cart at y 17.34 and the
    // slot-2 cart one up the slope))
    let in_station = |p: Vec3| {
        o.layout != DropLayout::Station || o.layout.pockets().iter().any(|(x, y, z)| (p.x - (*x as f64 + 0.5)).abs() < 0.6 && p.z >= *z as f64 - 0.1 && p.z < *z as f64 + 2.5 && p.y > *y as f64 - 1.0 && p.y < (o.hold_y + 3) as f64)
    };
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_)) && Some(e.id) != napper_cart && in_station(e.pos)).map(|e| e.id).collect();
    if carts.len() != 6 && carts.len() != 9 {
        return Err(format!("{} carts", carts.len()));
    }
    let mut next_carts: Vec<u32> = carts[6.min(carts.len())..].to_vec();
    let mut next_riders: Vec<Option<u32>> = next_carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    let mut carts: Vec<u32> = carts[..6].to_vec();
    let mut riders: Vec<Option<u32>> = carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    // (the cold start: departing carts absent, removed with their riders before the warm-up)
    for n in 0..3 {
        if o.dep_absent[n] {
            for e in run.world.entities.list.iter_mut() {
                if e.id == carts[n] || Some(e.id) == riders[n] {
                    e.removed = true;
                }
            }
        }
    }
    // (the villagers' sleep, G1 on d96461902 (b): the day before the summon, the bench's
    // villagers slept the night before as the steady runs set it: LAST_SLEPT = the summon day's
    // dawn + 10 (they wake at dawn; Run::start sets LAST_SLEPT only on the entities it spawns
    // itself); each pocket rider's LAST_SLEPT age and activity print at S; the WILLING count rests
    // on it (`wants_golem`: slept within SLEPT_WITHIN, no golem memory) and on the panic from the
    // posts; G1 on 834d6329d (T2): set as in the design's world WHATEVER the flag, so a run
    // without --summon-check summons as the design does, and the summon's scope (computed from
    // the world's GolemTry events) reads NOT JUDGED there; the control: pocket k's arriving
    // rider, the one in the hold over it, removed before the warm-up)
    {
        let slept = run.world.time - (ro.day0 - 10).rem_euclid(24000);
        let own = run.own.clone();
        for e in run.world.entities.list.iter_mut() {
            if own.contains(&e.id)
                && let EntityKind::Mob(m) = &mut e.kind
                && let Some(b) = m.brain.as_deref_mut()
                && b.last_slept.is_none()
            {
                b.last_slept = Some(slept);
            }
        }
    }
    if let Some(k) = o.remove_rider {
        let (px, _, pz) = *o.layout.pockets().get(k).ok_or("--remove-rider: a pocket 0..2")?;
        let held = (0..carts.len()).find(|&n| {
            run.world.entities.list.iter().find(|e| e.id == carts[n]).is_some_and(|e| (e.pos.x - (px as f64 + 0.5)).abs() < 0.6 && (e.pos.z - (pz as f64 + 0.5)).abs() < 0.6 && e.pos.y > 5.0) && riders[n].is_some()
        });
        let Some(n) = held else { return Err(format!("--remove-rider {k}: no rider in the hold over pocket {k}")) };
        let (c, r) = (carts[n], riders[n].unwrap_or_default());
        for e in run.world.entities.list.iter_mut() {
            if e.id == r {
                e.removed = true;
                e.vehicle = None;
            }
            if e.id == c {
                e.passengers.retain(|p| *p != r);
            }
        }
        riders[n] = None;
    }
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (rs, stone, td_open, td_shut, glass) = (parse("redstone_block")?, parse("stone")?, parse(TRAPDOOR_OPEN)?, parse(TRAPDOOR_SHUT)?, parse("glass")?);
    // (each pocket's support between drives in the other layouts: the design's own block, the
    // drive EMULATED as a redstone block in that cell (the stand-in guard checks it against the
    // built block); the station drives its pockets from lever stand-ins instead)
    let support_off: Vec<Block> = o.layout.pockets().iter().map(|(x, y, z)| {
        let b = run.world.get(BlockPos::new(*x, *y - 1, *z));
        if b.is_air() { stone.clone() } else { b }
    }).collect();
    // (ironspec 18:34: every lever stand-in's update and power reach, checked on the built world;
    // the stand-ins' own golem, path and late-path properties match their line ends (a lever
    // where the built input is dust or a repeater's input cell))
    let mut stand_ins: Vec<String> = Vec::new();
    // (each lever with its input and whether its support is the build's own cell: the pockets'
    // levers stand on M16bw's riser glass (x-2,-1,0), day-1's built cell; the others' supports are
    // stand-ins, the cells the sight property checks, ironspec 20:10 (2))
    let mut levers: Vec<(BlockPos, BlockPos, bool)> = Vec::new();
    if o.layout == DropLayout::Station {
        for (k, (x, y, z)) in o.layout.pockets().into_iter().enumerate() {
            match o.pocket_levers {
                // (the record's line ends: each lever's input the one dust beside it)
                Some(l) => {
                    let p = BlockPos::new(l[k].0, l[k].1, l[k].2);
                    let dust: Vec<BlockPos> = [redsim::Dir::North, redsim::Dir::South, redsim::Dir::East, redsim::Dir::West]
                        .iter()
                        .map(|d| p.rel(*d))
                        .filter(|c| run.world.get(*c).to_string().starts_with("minecraft:redstone_wire"))
                        .collect();
                    if dust.len() != 1 {
                        return Err(format!("--pocket-lever {p}: {} dust beside it (one wanted: the line it ends)", dust.len()));
                    }
                    levers.push((p, dust[0], true));
                }
                None => levers.push((BlockPos::new(x - 2, y, z), BlockPos::new(x - 1, y, z), true)),
            }
            if o.r_repeater && o.refill.is_some() {
                levers.push((BlockPos::new(x + 2, o.hold_y + 1, z + 1), BlockPos::new(x + 1, o.hold_y + 1, z + 1), false));
            }
        }
    }
    if o.napper.is_some() && o.napper_held.is_none() {
        levers.push((BlockPos::new(13, -1, 14), BlockPos::new(14, -1, 14), false));
    }
    for (p, c, _built) in &levers {
        stand_ins.push(lever_reach_check(&run.world, *p, *c, &[])?);
    }
    let air_b = parse("air")?;
    // (G1 item 9 / on a3b306a05 (7), ironspec 09:28: each station rider's activity at t1, the first
    // tick after Run::start spawned the villagers with the day already set (its day0): G1 on
    // 834d6329d (T3): the warm-up's first tick runs here, before the read (the loop below goes on
    // from its second), against its schedule's at the world's day time (WORK and MEET fall back
    // to IDLE without a job site or meeting point, as the brain's registerBrainGoals does), PANIC
    // allowed while scared; a rider whose id is not found fails (no silent skip); the HOME holders
    // named (ironspec 08:19); `--activity-control` sets rider 0's activity off its schedule before
    // the read, which must fail)
    run.tick();
    if o.activity_control {
        let day = run.world.day_time();
        let want = redsim::entity::mob::villager::scheduled(day);
        let wrong = if want == Some(redsim::entity::mob::villager::Activity::Rest) { redsim::entity::mob::villager::Activity::Idle } else { redsim::entity::mob::villager::Activity::Rest };
        let r0 = riders.first().copied().flatten().ok_or("--activity-control: no rider 0")?;
        let e = run.world.entities.list.iter_mut().find(|e| e.id == r0 && !e.removed).ok_or("--activity-control: rider 0 not found")?;
        let EntityKind::Mob(m) = &mut e.kind else { return Err("--activity-control: rider 0 is no mob".into()) };
        let b = m.brain.as_deref_mut().ok_or("--activity-control: rider 0 has no brain")?;
        b.activity = wrong;
    }
    let mut activity_wrong = 0u32;
    let mut activity_first: Option<String> = None;
    let activity_line: String;
    let home_holders: u32;
    let mut missing_reads = 0u32;
    let mut rider_left: Vec<Option<String>> = vec![None; riders.len()];
    {
        let day = run.world.day_time();
        let want = match redsim::entity::mob::villager::scheduled(day) {
            Some(redsim::entity::mob::villager::Activity::Work | redsim::entity::mob::villager::Activity::Meet) | None => redsim::entity::mob::villager::Activity::Idle,
            Some(a) => a,
        };
        let mut seen = Vec::new();
        let mut homes = Vec::new();
        for (n, r) in riders.iter().enumerate() {
            let Some(id) = r else { continue };
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id && !e.removed) else {
                missing_reads += 1;
                seen.push(format!("rider {n} NOT FOUND"));
                continue;
            };
            let EntityKind::Mob(m) = &e.kind else { continue };
            let Some(b) = m.brain.as_deref() else {
                missing_reads += 1;
                continue;
            };
            if let Some(h) = b.home {
                homes.push(format!("rider {n} {h}"));
            }
            let ok = b.activity == want || (b.scared() && b.activity == redsim::entity::mob::villager::Activity::Panic);
            seen.push(format!("rider {n} {}{}", b.activity.name(), if b.scared() { " (scared)" } else { "" }));
            if !ok {
                activity_wrong += 1;
                if activity_first.is_none() {
                    activity_first = Some(format!("rider {n} {} where the schedule at day time {} says {}", b.activity.name(), day.rem_euclid(24000), want.name()));
                }
            }
        }
        activity_line = format!(
            "the riders' activities at t1 (the first tick after the spawn; day time {}, the schedule's {}): {}; HOME holders: {}",
            day.rem_euclid(24000),
            want.name(),
            seen.join(", "),
            if homes.is_empty() { "none".to_string() } else { homes.join(", ") }
        );
        home_holders = homes.len() as u32;
    }
    // (the warm-up's first tick ran before the t1 read above: the loop goes on from its second)
    for t in 1..400 {
        run.tick();
        if t >= 60 && run.world.time % 100 == 0 {
            break;
        }
    }
    // (the design's zombie posts, seated: scarers beside --scarers)
    let scarers_all: Vec<(char, Vec3)> = o.scarers.iter().cloned().chain(crate::fleecheck::zombies_seated(&run.world, spec).into_iter().map(|p| ('z', p))).collect();
    let pockets = o.layout.pockets();
    let pos = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement));
    let feet_of = |w: &redsim::world::World, r: Option<u32>| r.and_then(|r| w.entities.list.iter().find(|e| e.id == r && !e.removed)).map(|e| e.pos.y);
    let string_on = |w: &redsim::world::World, k: usize| {
        let (x, y, z) = pockets[k];
        w.get(BlockPos::new(x, y + 1, z)).to_string().contains("powered=true")
    };
    let pocket_powered = |w: &redsim::world::World, k: usize| {
        let (x, y, z) = pockets[k];
        w.get(BlockPos::new(x, y, z)).to_string().contains("powered=true")
    };
    let mut frozen_ticks = 0i64;
    let mut frozen_hist: Vec<bool> = Vec::new();
    let mut opened: [Option<i64>; 3] = [None; 3];
    let mut opened_live: [i64; 3] = [0; 3];
    let mut reloaded: Vec<String> = Vec::new();
    let mut empty_at_release = [false; 3];
    let mut was_frozen = false;
    let mut shut_onto: Vec<String> = Vec::new();
    let mut r_was = false;
    let mut r_switch_frozen: Vec<String> = Vec::new();
    let mut shut_again = [false; 3];
    let mut from: [Option<Vec3>; 3] = [None; 3];
    let mut feet: [Option<f64>; 3] = [None; 3];
    let mut waited = [0i64; 3];
    // (a string already on at T has been held since the batch came to rest, long before)
    let mut on_since: [Option<i64>; 3] = [0, 1, 2].map(|k| string_on(&run.world, k).then_some(-1000));
    let mut landed: [Option<(i64, f64, bool)>; 3] = [None; 3];
    let mut fall_v = [0.0f64; 3];
    let mut panic_log: Vec<[Vec<(bool, bool, bool)>; 3]> = Vec::new();
    let mut panic_pos: Vec<[Option<Vec3>; 3]> = Vec::new();
    // (ironspec 13:09, golem-stamp path (a): the departing riders through the merge row and the lift)
    let mut lift_last: [Option<i64>; 3] = [None; 3];
    let mut lift_in100 = [false; 3];
    let mut lift_y80: [Option<f64>; 3] = [None; 3];
    let mut lift_y100: [Option<f64>; 3] = [None; 3];
    let mut lift_merge: [Option<i64>; 3] = [None; 3];
    let mut lift_entry: [Option<i64>; 3] = [None; 3];
    let mut lift_y12: [Option<i64>; 3] = [None; 3];
    let mut lift_top: [Option<(i64, f64, f64)>; 3] = [None; 3];
    let mut hold_on: Option<i64> = None;
    let mut hold_rest: Option<(i64, f64, f64)> = None;
    let mut hold_first: Option<i64> = None;
    let mut hold_track: Vec<(i64, Vec3, Vec3)> = Vec::new();
    let mut hold_drift = 0.0f64;
    let mut hold_overshoot = false;
    let mut hold_wet = 0u32;
    let mut lift_clear: Option<f64> = None;
    let mut lift_push = 0u32;
    let mut lift_push_at: Option<(i64, usize, usize)> = None;
    // (the napper's drive through its built path (ironspec 17:16): the controller's line end a
    // lever stand-in at (7,-1,8) (ironspec 17:56) into the module's repeater (8,-1,8) (delay 1,
    // facing west), which powers the slime (9,-1,8) under the hold rail; tick-exact's official
    // ride: the rail at pulse + 1)
    let nap_drive = BlockPos::new(13, -1, 14);
    let (mut nap_off, mut nap_entry, mut nap_y12, mut nap_top): (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = (None, None, None, None);
    let mut lift_air: [Option<i32>; 4] = [None; 4];
    let mut lift_dip: [Option<i64>; 3] = [None; 3];
    let mut watch_hits = 0u32;
    let mut watch_first: Option<(i64, (i32, i32, i32))> = None;
    let mut lift_track: [Vec<(i64, Vec3)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    // (farmopt 05:55, option (b): a zombie post's cells for lane k inside the zombie band (its seat
    // 6.05..7.95 from the rider at rest, no rider ever nearer than 6.05) that make the rider panic
    // earlier in its fall: each candidate a glass box open on one side (its own cells and the open
    // face air, its floor, roof and walls air or a plain block), its sight through the open face)
    let mut cands: Vec<Cand> = Vec::new();
    if let Some(k) = o.panic_search {
        let (px, py, pz) = pockets[k];
        let rest = Vec3::new(px as f64 + 0.5, py as f64 + 0.0625, pz as f64 + 0.5);
        let w = &run.world;
        let air = |x: i32, y: i32, z: i32| w.get(BlockPos::new(x, y, z)).is_air();
        let plain = |x: i32, y: i32, z: i32| {
            let b = w.get(BlockPos::new(x, y, z));
            b.is_air() || matches!(b, Block::Simple(_))
        };
        for cx in px - 9..=px + 9 {
            for cy in py - 7..=py + 9 {
                for cz in pz - 9..=pz + 9 {
                    let seat = Vec3::new(cx as f64 + 0.5, cy as f64 + 0.1875 - 0.7, cz as f64 + 0.5);
                    let d = ((seat.x - rest.x).powi(2) + (seat.y - rest.y).powi(2) + (seat.z - rest.z).powi(2)).sqrt();
                    if !(6.05..=7.95).contains(&d) || !air(cx, cy, cz) || !air(cx, cy + 1, cz) || !plain(cx, cy - 1, cz) || !plain(cx, cy + 2, cz) {
                        continue;
                    }
                    let sides = [('e', cx + 1, cz), ('w', cx - 1, cz), ('n', cx, cz - 1), ('s', cx, cz + 1)];
                    // (inside the module's chunk: the box's cells at file x and z 0..15)
                    if cx - 1 < 0 || cx + 1 > 15 || cz - 1 < 0 || cz + 1 > 15 {
                        continue;
                    }
                    for (side, ox, oz) in sides {
                        if !air(ox, cy, oz) || !air(ox, cy + 1, oz) {
                            continue;
                        }
                        if !sides.iter().filter(|(s2, _, _)| *s2 != side).all(|(_, wx, wz)| plain(*wx, cy, *wz) && plain(*wx, cy + 1, *wz)) {
                            continue;
                        }
                        cands.push(Cand { cell: [cx, cy, cz], open: side, seat, eye: Vec3::new(seat.x, seat.y + 1.74, seat.z), rest_d: d, log: Vec::new(), nearest: f64::INFINITY });
                    }
                }
            }
        }
    }
    let mut summon_off: Vec<String> = Vec::new();
    // (ironspec 02:14, tick-exact's void condition: the falling carts' largest horizontal offset
    // from their shaft's centre line, from the trapdoor's opening to the landing, and its largest
    // component toward a zombie post's seat)
    let mut fall_off: f64 = 0.0;
    let mut fall_toward: f64 = f64::NEG_INFINITY;
    let mut rest_since: [Option<i64>; 3] = [None; 3];
    let mut rest_at_s: [Option<Vec3>; 3] = [None; 3];
    let mut rebrake_dt: Option<i64> = None;
    let mut rebrake_at: [Option<i64>; 3] = [None; 3];
    let mut calm_run = [0i64; 9];
    let mut all_shut: Option<i64> = None;
    let mut r_from: Option<i64> = None;
    let mut r_to: Option<i64> = None;
    let mut next_rest: [Option<(i64, Vec3)>; 3] = [None; 3];
    let mut next_fell = [false; 3];
    // (ironspec 09:32: R_1's launch against its drive: each lane's rail powered from .. to (read
    // from the rail's own state after each tick), each next cart's first move after the R line came
    // on (its position before the R line's first tick, then more than 0.01 off it: a cart at rest
    // keeps its position exactly (the hold's drift prints 0.0000), and a powered rail's first push
    // from rest moves it more than 0.01 in its first tick, so 0.01 separates rest from the launch
    // with no tick of lag (the checklist audit (5): a tolerance's reason))
    let mut next_start: Vec<Option<Vec3>> = vec![None; 3];
    let mut next_moved: Vec<Option<i64>> = vec![None; 3];
    // (G1 on b4549f35a: ironfarm's and tick-exact's rows key on leaving the rest's cell: each next
    // cart's first tick off R_1's cell (x, hold + 1, z + 1))
    let mut next_off_cell: Vec<Option<i64>> = vec![None; 3];
    let mut r1_on: [Option<i64>; 3] = [None; 3];
    let mut r1_off: [Option<i64>; 3] = [None; 3];
    let mut next_closest: Option<f64> = None;
    let mut gap_at_open: [Option<f64>; 3] = [None; 3];
    let mut ready_at: [Option<i64>; 3] = [None; 3];
    let mut launch_at: [Option<i64>; 3] = [None; 3];
    let mut exit_seen = [false; 3];
    let mut exit_was = [false; 3];
    let mut calm_max = [0i64; 4];
    let mut calm_y: [Option<(f64, f64)>; 3] = [None; 3];
    let mut pillager_near: Option<f64> = None;
    let mut zombie_near: Option<f64> = None;
    let mut zombie_any: Option<f64> = None;
    let mut zombie_any_at: Option<(i64, usize, Vec3, Vec3, Option<usize>)> = None;
    let mut hold_lane = [0i64; 3];
    let mut exits: [Option<(i64, f64)>; 3] = [None; 3];
    let mut prev_v = [0.0f64; 6];
    let mut touching = [[false; 6]; 6];
    let mut contacts: Vec<String> = Vec::new();
    let mut contact_max = 0.0f64;
    let mut closest: Option<(f64, i64, usize, usize)> = None;
    let mut hurt = 0usize;
    let mut health: [Option<f32>; 6] = [None; 6];
    let mut unseated = 0usize;
    let mut out: Vec<String> = Vec::new();
    if !stand_ins.is_empty() {
        out.push(format!(
            "the drives' stand-ins (ironspec 17:46/18:34: levers at the line ends, golem, path and late-path properties as the built inputs', no T in their update reach, nothing powered beyond their input): {}",
            stand_ins.join("; ")
        ));
    }
    // (the direct summon check: T's game time, the due summon tick S after it on the summon's own
    // clock, game time % 100 == 0 (the checklist audit (5)))
    let t_zero = run.world.time;
    let s_rel: i64 = s_rel_of(t_zero);
    // (G1 on a3b306a05 (4): the CLI's drive windows must equal the station's declared held phases;
    // the hold check reads the declaration)
    if o.hold_drive.is_some() && (o.hold_release + 1, o.hold_release + o.hold_width) != HELD_SLOPE {
        return Err(format!(
            "--hold-release {} / --hold-width {}: the drive window REL + {} .. REL + {} differs from the station's declared held: phase REL + {} .. REL + {} (refused, G1 on a3b306a05 (4))",
            o.hold_release,
            o.hold_width,
            o.hold_release + 1,
            o.hold_release + o.hold_width,
            HELD_SLOPE.0,
            HELD_SLOPE.1
        ));
    }
    if let (Some(k), Some(_)) = (o.napper, o.napper_held) {
        if (k + 1, k + 10) != HELD_NAPPER {
            return Err(format!("--napper-release {k}: the napper's held window REL + {} .. REL + {} differs from the declared REL + {} .. REL + {} (refused)", k + 1, k + 10, HELD_NAPPER.0, HELD_NAPPER.1));
        }
    }
    // (the audit (5): a run whose ticks end before the last judged tick (S, the hold window's end,
    // the napper's pulse's end) is refused, as is a hold window outside the run)
    let mut last_judged = s_rel + 1;
    if o.hold_drive.is_some() {
        if o.hold_width <= 0 || o.rel + o.hold_release < 0 {
            return Err(format!("--hold-width {} / --hold-release {}: the window REL + R + 1 .. REL + R + W must lie in the run (W > 0)", o.hold_width, o.hold_release));
        }
        last_judged = last_judged.max(o.rel + o.hold_release + o.hold_width + 2);
    }
    if let Some(k) = o.napper {
        last_judged = last_judged.max(o.rel + k + 12);
    }
    if o.ticks < last_judged {
        return Err(format!("--ticks {}: the run ends before its last judged tick T+{last_judged} (the audit (5))", o.ticks));
    }
    if let Some(k) = o.hold_poke {
        if o.hold_drive.is_none() || k < o.hold_release + 1 || k > o.hold_release + o.hold_width {
            return Err(format!("--hold-poke {k}: outside the held window REL + {} .. REL + {} (the control must poke inside it)", o.hold_release + 1, o.hold_release + o.hold_width));
        }
    }
    if o.lever_off.is_some_and(|k| k > 2) || o.remove_rider.is_some_and(|k| k > 2) {
        return Err("--lever-off / --remove-rider: a pocket 0..2".into());
    }
    // (the reference world's cells whose block TYPE differs from the laid world's at T (the
    // stand-ins and overrides): the sight check's scope)
    let sight_scope = o.sight_control
        || o.sight_ref.as_deref().is_some_and(|r| {
            let air = Block::Air;
            let (min, size) = (run.world.min, run.world.size);
            (min.x..min.x + size[0]).any(|x| (min.y..min.y + size[1]).any(|y| (min.z..min.z + size[2]).any(|z| block_type(&run.world.get(BlockPos::new(x, y, z))) != block_type(r.get(&[x, y, z]).unwrap_or(&air)))))
        });
    let mut sight_rays = 0u32;
    let mut sight_control_cell: Option<String> = None;
    let mut s_try: Option<i64> = None;
    let mut lift_samples = [0u32; 3];
    let mut held_edge_count = 0u32;
    let mut lever_seen: Vec<(BlockPos, bool)> = Vec::new();
    let mut lever_noops = 0u32;
    let mut dep_stamped = 0u32;
    let mut summon_events = 0u32;
    let mut rail_t_stop: Option<String> = None;
    let mut hold_press: [Option<i64>; 3] = [None; 3];
    let mut hold_release: [Option<i64>; 3] = [None; 3];
    let mut hold_was: [bool; 3] = [false; 3];
    let mut drive_fire: [Option<i64>; 3] = [None; 3];
    // (ironfarm's C-fix rows: every minecart's last position, its still ticks, the west leaves and
    // the off-rail rests)
    let mut cart_last: std::collections::HashMap<u32, Vec3> = std::collections::HashMap::new();
    let mut cart_still: std::collections::HashMap<u32, i64> = std::collections::HashMap::new();
    let mut west_leaves: Vec<String> = Vec::new();
    let mut offrail: Vec<(u32, String)> = Vec::new();
    // (G1 on 407a29682 (N1): each watched cell's carts and exits; the carts at rest at or above the
    // off-rail line, on rails or not)
    let mut watch_in: Vec<(BlockPos, Vec<u32>)> = o.watch_west.iter().map(|c| (*c, Vec::new())).collect();
    let mut watch_exits: Vec<String> = Vec::new();
    let mut rest_seen: Vec<u32> = Vec::new();
    let mut planted: Vec<String> = Vec::new();
    let mut plants_done = 0usize;
    // (G1 on e2d9333bc (B4): a plant at T + k past the run never fires: refused before the run)
    if let Some(k) = o.plant_carts.iter().map(|(_, k)| *k).chain(o.plant_blocks.iter().map(|(_, _, k)| *k)).find(|k| *k < 0 || *k >= o.ticks) {
        return Err(format!("--plant-cart / --plant-block at T+{k}: outside the run (T+0..T+{})", o.ticks.saturating_sub(1)));
    }
    let mut t_shapes: Vec<(BlockPos, String)> = Vec::new();
    let mut t_reshaped: Vec<(BlockPos, String)> = Vec::new();
    let mut t_control_lever: Option<BlockPos> = None;
    let mut home_least: Option<f64> = None;
    let mut reseat_id: Option<u32> = None;
    let mut reseat_old: Option<u32> = None;
    let mut reseat_attr: Option<(Option<u8>, Option<u8>)> = None;
    let mut act_prev: Vec<Option<redsim::entity::mob::villager::Activity>> = Vec::new();
    let mut act_timeline: Vec<String> = Vec::new();
    let mut rest_entries = 0u32;
    let mut rest_list: Vec<String> = Vec::new();
    let mut summon_calls: Vec<SummonCall> = Vec::new();
    let mut summon_due: Vec<SummonDue> = Vec::new();
    let mut golems_removed = 0u32;
    let mut sight_hits: Vec<String> = Vec::new();
    let mut sight_control = false;
    let mut sight_hit_count = 0u32;
    let mut rider_missing = [false; 3];
    let mut hold_expect: Vec<(BlockPos, bool)> = Vec::new();
    let mut hold_breaks = 0u32;
    let mut hold_break_first: Option<String> = None;
    let mut held_edges: Vec<String> = Vec::new();
    let mut block_contacts: Vec<(BlockPos, String, u32, bool)> = Vec::new();
    let mut undeclared_contacts = 0u32;
    for dt in 0..o.ticks {
        // (a restart between ticks dt - 1 and dt, before the controller acts in tick dt: the
        // shared helper, its entities back under new ids, followed)
        for (_, how) in o.restarts.iter().filter(|(t, _)| *t == dt) {
            crate::liveness::restart(&mut run.world, *how)?;
            for c in carts.iter_mut().chain(next_carts.iter_mut()) {
                *c = crate::liveness::follow(&run.world, *c);
            }
            for r in riders.iter_mut().chain(next_riders.iter_mut()).flatten() {
                *r = crate::liveness::follow(&run.world, *r);
            }
            if let Some(nc) = napper_cart.as_mut() {
                *nc = crate::liveness::follow(&run.world, *nc);
            }
            reloaded.push(format!("T+{dt} {}", how.label()));
        }
        let frozen_now = o.freeze.iter().chain(o.logins.iter()).any(|(f, l)| dt >= *f && dt < f + l);
        let live = dt - frozen_ticks;
        // (the controller's clock: L1 (G1/ironspec 00:18, the lead) runs the controller and its
        // timers on game ticks through a freeze; L2 (the ring clock) on the live clock)
        let clock = if o.l1 { dt } else { live };
        // the pockets' supports (and the series layout's gate slopes): REL + stagger .. REL + rebrake
        // (the station: day-1's repeater (x-1,0,0) into each pocket rail, its input a lever
        // stand-in (x-2,0,0) switched one tick ahead (the repeater's delay in the bench: the rail
        // at the lever + 1), so the rail's power keeps REL + stagger .. REL + rebrake; G1 on
        // d8394ed6e, ironspec 17:56: no block in the glass support cell)
        for k in 0..3 {
            let off_at = o.rel + o.rebrake + if o.rebrake_each { o.stagger[k] } else { 0 };
            let on = clock >= o.rel + o.stagger[k] && clock < off_at;
            if rebrake_at[k].is_none() && clock >= off_at {
                rebrake_at[k] = Some(dt);
            }
            let (x, y, z) = pockets[k];
            if o.layout == DropLayout::Station {
                // (the drive-integrity bar's control: `--late-drive-control K,D` fires pocket K's
                // drive D gt late)
                let late = o.late_drive.filter(|(lk, _)| *lk == k).map_or(0, |(_, d)| d);
                let lead = clock + 1 >= o.rel + o.stagger[k] + late && clock + 1 < off_at + late && o.lever_off != Some(k);
                let lever = o.pocket_levers.map_or(BlockPos::new(x - 2, y, z), |l| BlockPos::new(l[k].0, l[k].1, l[k].2));
                let edge = lever_edge(&mut lever_seen, lever, lead);
                if !set_lever(&mut run.world, lever, lead)? && edge {
                    lever_noops += 1;
                }
                continue;
            }
            let want = if on { rs.clone() } else { support_off[k].clone() };
            let mut cells = vec![BlockPos::new(x, y - 1, z)];
            if o.layout == DropLayout::Series {
                let (sx, sy, sz) = SERIES_SLOPE_SRC[k];
                cells.push(BlockPos::new(sx, sy, sz));
            }
            for p in cells {
                if run.world.get(p) != want {
                    emulated_write(&mut run.world, p, want.clone(), &support_off[k])?;
                }
            }
        }
        if rebrake_dt.is_none() && clock >= o.rel + o.rebrake {
            rebrake_dt = Some(dt);
        }
        // (2c)'s hold (ironspec 14:16): day-1's output on the controller's clock, REL + release
        // for its width. No stand-in at its line end (-4,12,5): every support a lever could hang on
        // there blocked some rider's sight ray between T and S (ironspec 23:30), so the bench holds
        // the braked slope's own rail powered from REL + release + 1 for the width, the tick and the
        // span day-1's repeater (-4,12,6) gave it from a lever on (-4,12,5): a declared `held:`
        // stand-in (ironspec 01:07; the rail's own state, the same block: the guard's list and its
        // survival pass). G1 on 82f9546f (b): at each edge the bench makes the neighbour updates the
        // build's path makes (26.3: the repeater's updateNeighborsInFront, the rail's setBlock with
        // its affectNeighborsAfterRemoval and PoweredRailBlock.updateState's updates below and, on an
        // ascending rail, above), all but the rail's own re-evaluation, which in the build keeps its
        // state (its repeater drives it) and here would drop it; (c) the hold check after the tick
        if let Some((hx, hy, hz)) = o.hold_drive {
            let on = clock >= o.rel + o.hold_release && clock < o.rel + o.hold_release + o.hold_width;
            if on && hold_on.is_none() {
                hold_on = Some(dt);
            }
            let held = clock >= o.rel + o.hold_release + 1 && clock < o.rel + o.hold_release + o.hold_width + 1;
            let (rx, ry, rz) = o.hold_rail.unwrap_or((hx, hy, hz + 2));
            let p = BlockPos::new(rx, ry, rz);
            // (its driver: day-1's repeater (-4,12,6), the drive's cell + 1 south, its front the rail;
            // G1 on 834d6329d (T1): a driver edge reaching a rail T stops the run there as a
            // failure in its list, not a refused run)
            let w_res = match held_rail_write(&mut run.world, p, held, (BlockPos::new(hx, hy, hz + 1), p)) {
                Err(e) if e.contains("potential connections: at exactly 3") => {
                    rail_t_stop = Some(format!("T+{dt}: {e}"));
                    break;
                }
                r => r?,
            };
            if let Some(cells) = w_res {
                held_edge_count += 1;
                if held_edges.len() < 2 {
                    held_edges.push(format!("the slope T+{dt} {}: neighbour updates to {cells}", if held { "on" } else { "off" }));
                }
            }
            // (the hold check's window: the station's DECLARED phase, HELD_SLOPE)
            hold_expect.push((p, (o.rel + HELD_SLOPE.0..=o.rel + HELD_SLOPE.1).contains(&clock)));
            // (the hold check's positive control, rule (A): `--hold-poke K` gives the rail itself a
            // real neighbour update at REL + K, which the bench's undriven rail answers by dropping)
            if o.hold_poke.is_some_and(|k| clock == o.rel + k) {
                let kind = run.world.get(p).kind();
                run.world.neighbor_changed(p, kind);
            }
        }
        if let Some(k) = o.napper {
            let on = clock >= o.rel + k && clock < o.rel + k + 10;
            match o.napper_held {
                // (record7e: the napper's line end (7,-1,8) is air with no support but the glass
                // (7,-1,7), whose lever ironspec 18:31 retired, so the napper's rail is a held
                // stand-in like the slope's: powered from REL + K + 1 for the pulse's 10 gt, the rail's
                // rise through the repeater (8,-1,8) and the slime (9,-1,8) at the pulse + 1)
                Some((nx, ny, nz)) => {
                    let p = BlockPos::new(nx, ny, nz);
                    let held = clock >= o.rel + k + 1 && clock < o.rel + k + 11;
                    // (its driver: the module's repeater (8,-1,8), whose front is the slime (9,-1,8)
                    // under the rail, which powers it)
                    let front = p.rel(redsim::Dir::Down);
                    let w_res = match held_rail_write(&mut run.world, p, held, (front.rel(redsim::Dir::West), front)) {
                        Err(e) if e.contains("potential connections: at exactly 3") => {
                            rail_t_stop = Some(format!("T+{dt}: {e}"));
                            break;
                        }
                        r => r?,
                    };
                    if let Some(cells) = w_res {
                        held_edge_count += 1;
                        if held_edges.len() < 4 {
                            held_edges.push(format!("the napper T+{dt} {}: neighbour updates to {cells}", if held { "on" } else { "off" }));
                        }
                    }
                    // (the declared phase, HELD_NAPPER)
                    hold_expect.push((p, (o.rel + HELD_NAPPER.0..=o.rel + HELD_NAPPER.1).contains(&clock)));
                }
                None => {
                    let edge = lever_edge(&mut lever_seen, nap_drive, on);
                    if !set_lever(&mut run.world, nap_drive, on)? && edge {
                        lever_noops += 1;
                    }
                }
            }
        }
        // the trapdoors: open at T + release unless the pocket's string has been held 20 gt
        for k in 0..3 {
            let (x, _, z) = pockets[k];
            let cell = BlockPos::new(x, o.hold_y, z);
            let occupied = if o.exit_gate { !exit_seen[k] } else { o.occupancy && on_since[k].is_some_and(|s| clock - s >= 20) };
            // (the cold start's evidence: the pocket empty at its own release, its string off)
            if o.exit_gate && !exit_seen[k] && clock == o.rel + o.stagger[k] && !string_on(&run.world, k) {
                exit_seen[k] = true;
                empty_at_release[k] = true;
            }
            let due = if o.release_world || o.l1 { dt >= o.release } else { !frozen_now && live >= o.release };
            let t_now = if o.release_world { dt } else { clock };
            // (the release a stage later: the trapdoor opens `open_delay` after its gate is met)
            let ready = !occupied && due && t_now >= *ready_at[k].get_or_insert(t_now) + o.open_delay;
            if !due || occupied {
                ready_at[k] = None;
            }
            if opened[k].is_none() && due {
                if occupied {
                    waited[k] = t_now - o.release + 1;
                } else if ready {
                    opened[k] = Some(dt);
                    opened_live[k] = clock;
                    from[k] = pos(&run.world, carts[3 + k]).map(|(p, _)| p);
                    // (the closest case: where this pocket's departing cart was as its trapdoor
                    // opened; gone: removed past the exit)
                    let (px, _, pz) = pockets[k];
                    gap_at_open[k] = Some(pos(&run.world, carts[k]).map_or(f64::INFINITY, |(p, _)| ((p.x - (px as f64 + 0.5)).powi(2) + (p.z - (pz as f64 + 0.5)).powi(2)).sqrt()));
                    feet[k] = feet_of(&run.world, riders[3 + k]);
                    run.world.set_block(cell, td_open.clone(), redsim::world::flags::ALL);
                }
            }
            if o.pulse > 0 && opened[k].is_some() && !shut_again[k] && clock >= opened_live[k] + o.pulse {
                shut_again[k] = true;
                // (a trapdoor shut onto its cart: the arriving cart still in the hold cell, frozen
                // on the open trapdoor: it stays up and misses its drop)
                if let Some((p, _)) = pos(&run.world, carts[3 + k])
                    && p.y > o.hold_y as f64 - 0.3
                {
                    shut_onto.push(format!("T+{dt} lane {k}{}", if frozen_now { " (frozen)" } else { "" }));
                }
                run.world.set_block(cell, td_shut.clone(), redsim::world::flags::ALL);
            }
        }
        // the R line (the refill): every R_1 powered from its side once every trapdoor has shut
        // again, + the delay, for the length
        if let Some((delay, len)) = o.refill
            && !next_carts.is_empty()
        {
            if all_shut.is_none() && (0..3).all(|k| shut_again[k]) {
                all_shut = Some(clock);
            }
            let on = all_shut.is_some_and(|s| clock >= s + delay && clock < s + delay + len);
            if on && r_from.is_none() {
                r_from = Some(dt);
            }
            if !on && r_from.is_some() && r_to.is_none() {
                r_to = Some(dt);
            }
            if on != r_was && frozen_now {
                r_switch_frozen.push(format!("T+{dt} {}", if on { "on" } else { "off" }));
            }
            r_was = on;
            for (x, _, z) in pockets {
                // (the real drive: the controller's line end at the repeater's input, a lever
                // stand-in (ironspec 17:46); else the old emulation, a redstone block in R_1's side
                // cell, which the stand-in guard refuses (it differs from the stub's glass))
                if o.r_repeater {
                    let lp = BlockPos::new(x + 2, o.hold_y + 1, z + 1);
                    let edge = lever_edge(&mut lever_seen, lp, on);
                    if !set_lever(&mut run.world, lp, on)? && edge {
                        lever_noops += 1;
                    }
                } else {
                    let p = BlockPos::new(x + 1, o.hold_y + 1, z + 1);
                    let want = if on { rs.clone() } else { glass.clone() };
                    if run.world.get(p) != want {
                        emulated_write(&mut run.world, p, want, &glass)?;
                    }
                }
            }
        }
        if frozen_now {
            frozen_ticks += 1;
        }
        // (the freeze: one entity freeze over every overlapping window, thawed when none is left)
        if frozen_now && !was_frozen {
            if run.world.entity_chunks.is_none() {
                let (mn, sz) = (run.world.min, run.world.size);
                run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
            }
            redsim::reload::unload_some(&mut run.world, [0, 0, 0, 0]);
        }
        if !frozen_now && was_frozen {
            redsim::reload::load_some(&mut run.world, [0, 0, 0, 0]);
        }
        was_frozen = frozen_now;
        frozen_hist.push(frozen_now);
        // (ironspec 02:24: `positions:` lists the pockets and the holds only, on the timing
        // argument (summons only at gameTime % 100 == 0): count every villager in a shaft or on
        // a departure rail at a summon tick, before and after that tick; required 0)
        let at_summon = |w: &redsim::world::World, out: &mut Vec<String>| {
            let all: Vec<(usize, Option<u32>)> = (0..6).map(|n| (n % 3, riders[n])).chain(next_riders.iter().enumerate().map(|(j, r)| (j, *r))).collect();
            for (k, r) in all {
                let Some(r) = r else { continue };
                let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else { continue };
                let (px, py, pz) = pockets[k];
                let in_col = e.pos.x >= px as f64 && e.pos.x < px as f64 + 1.0;
                let shaft = in_col && e.pos.z >= pz as f64 && e.pos.z < pz as f64 + 1.0 && e.pos.y >= py as f64 + 1.0 && e.pos.y < o.hold_y as f64;
                let leg = in_col && e.pos.z >= pz as f64 + 1.0 && e.pos.y < py as f64 + 1.0;
                if shaft || leg {
                    out.push(format!("lane {k} {} at {:.2},{:.2},{:.2}", if shaft { "in its shaft" } else { "on its departure rail" }, e.pos.x, e.pos.y, e.pos.z));
                }
            }
        };
        if dt % 100 == 0 {
            let mut v = Vec::new();
            at_summon(&run.world, &mut v);
            summon_off.extend(v.into_iter().map(|s| format!("T+{dt} (before): {s}")));
        }
        // (the direct summon check, ironspec 19:24..20:07: the stand's riders at the start of the
        // due summon tick S, its GolemTry calls after it (up to the first success); every golem
        // born is removed after its tick, the portals' removal being golem-flow's; S on the summon's
        // own clock, game time % 100 == 0 (the checklist audit (5)))
        let s_time = run.world.time + 1;
        // (ironspec 13:37: a freeze over S puts that S out of the summon check's scope (printed with
        // its count and reason), and the NEXT S, S + 100, must pass: the recovery bar)
        let recovery_s = summon_due.first().is_some_and(|d| d.class.starts_with("frozen")) && s_time == t_zero + s_rel + 100;
        let pre = (o.summon_check && (s_time == t_zero + s_rel || recovery_s)).then(|| summon_pre(&run.world, &pockets, s_time));
        // (the checklist audit (6): the stamps themselves: each departing rider's
        // GOLEM_DETECTED_RECENTLY before S, read again after S's tick)
        let dep_golem_pre: Option<[Option<Option<i64>>; 3]> = (o.summon_check && s_time == t_zero + s_rel).then(|| [0, 1, 2].map(|n| golem_mem(&run.world, riders[n])));
        // (the sight check's positive control, ironspec 00:27 (A), G1's audit (1): `--sight-control`
        // lays a glass block in the WORLD at T on the first live villager -> scarer ray; the check
        // reads only the world against the reference, so it must flag it from then on; a cell
        // already a full collider would change nothing (04:07) and a run with no such ray can't
        // run the control: both refuse)
        if o.sight_control && dt == 0 {
            let Some(reference) = o.sight_ref.as_deref() else {
                return Err("--sight-control: no reference world (the module's cells, the stand-ins as built) to read the sight against".into());
            };
            let (_, _, control) = sight_crossings_world(&run.world, reference);
            let Some(c) = control else {
                return Err("--sight-control: no villager -> scarer or villager -> villager ray at T to lay the control's glass on".into());
            };
            let before = run.world.get(c);
            if redsim::behavior::collision_boxes(&before) == vec![[0, 16, 0, 16, 0, 16]] {
                return Err(format!("--sight-control: the cell {c} on the ray is already a full collider ({before}): the control would change nothing (ironspec 04:07)"));
            }
            run.world.set_block(c, glass.clone(), redsim::world::flags::ALL);
            sight_control_cell = Some(format!("{c} {} -> glass", before.to_string().trim_start_matches("minecraft:")));
            sight_control = true;
        }
        // (G1 on a3b306a05: the sight window starts at the END of T, the state before the first
        // tick, and the post-tick reads cover the ends of game ticks T+1 .. S)
        if dt == 0 {
            if let Some(reference) = o.sight_ref.as_deref() {
                let (hits, rays, _) = sight_crossings_world(&run.world, reference);
                sight_rays += rays;
                sight_hit_count += hits.len() as u32;
                sight_hits.extend(hits.into_iter().take(12).map(|h| format!("T (its end): {h}")));
            }
        }
        // (the checklist audit (7), the missing-rider control: departing rider K removed at T, not
        // declared absent; the rate control: every golem spawn spot of the stand's callers blocked
        // with glass at T, each block an override that must change its cell (04:07))
        if o.foreign_caller && dt == s_rel - 5 {
            let mut off = 0u32;
            for q in pockets {
                if let Some(i) = pocket_rider(&run.world, q) {
                    redsim::entity::stop_riding(&mut run.world, i);
                    off += 1;
                }
            }
            if off == 0 {
                return Err("--foreign-caller-control: no pocket rider to dismount at S - 5".into());
            }
        }
        // (G1 on a3b306a05 (3), the attribution by id while riders are seated: `--reseat-control`
        // dismounts pocket 0's rider 40 gt before S (it keeps its brain and panic, standing beside
        // the cart) and seats a FRESH villager (a new id, LAST_SLEPT as the others') in that cart;
        // at S the fresh one's call must be attributed to P0 and the old rider's counted foreign)
        if o.reseat && dt == s_rel - 40 {
            let i = pocket_rider(&run.world, pockets[0]).ok_or("--reseat-control: no rider in pocket 0 at S - 40")?;
            let cart = run.world.entities.list[i].vehicle.ok_or("--reseat-control: pocket 0's rider has no vehicle")?;
            let cp = run.world.entities.list.iter().find(|e| e.id == cart).map(|e| e.pos).ok_or("--reseat-control: pocket 0's cart is gone")?;
            reseat_old = Some(run.world.entities.list[i].id);
            redsim::entity::stop_riding(&mut run.world, i);
            let fresh = redsim::entity::spawn(&mut run.world, "minecraft:villager", [cp.x, cp.y, cp.z], None)?;
            let slept = run.world.time - (ro.day0 - 10).rem_euclid(24000);
            if let Some(e) = run.world.entities.list.iter_mut().find(|e| e.id == fresh)
                && let EntityKind::Mob(m) = &mut e.kind
                && let Some(b) = m.brain.as_deref_mut()
            {
                b.last_slept = Some(slept);
            }
            // (seated as an NBT-loaded passenger is: vehicle and passengers set, the cart then
            // positions its rider each tick; redsim's `ride` models only the /ride command's armor
            // stand)
            let fi = redsim::entity::index_of(&run.world, fresh).ok_or("--reseat-control: the fresh villager is gone")?;
            let ci = redsim::entity::index_of(&run.world, cart).ok_or("--reseat-control: pocket 0's cart is gone")?;
            run.world.entities.list[fi].vehicle = Some(cart);
            run.world.entities.list[ci].passengers.push(fresh);
            reseat_id = Some(fresh);
        }
        if dt == 0 {
            if let Some(k) = o.remove_departing {
                let id = riders.get(k).copied().flatten().ok_or("--missing-control: no departing rider there")?;
                let e = run.world.entities.list.iter_mut().find(|e| e.id == id && !e.removed).ok_or("--missing-control: the rider is already gone")?;
                e.removed = true;
            }
            if o.rate_control {
                let mut laid = 0u32;
                for (px, py, pz) in pockets {
                    let start = BlockPos::new(px, py, pz);
                    for (c, r) in redsim::entity::mob::spawn::golem_spawn_cells_from(&run.world, start) {
                        if r.is_ok() && run.world.get(c) != glass {
                            run.world.set_block(c, glass.clone(), redsim::world::flags::ALL);
                            laid += 1;
                        }
                    }
                }
                if laid == 0 {
                    return Err("--rate-control: no golem spawn spot to block (the control would change nothing)".into());
                }
            }
        }
        // (farmopt 09:46, the pocket-exit Ts: every plain rail with >= 3 potential connections in
        // the world at T (a T) is recorded with its shape, checked at every tick's end, any change
        // a runtime re-shape, a failure; `--t-control` plants a lever beside the first station T at
        // T (on glass) and switches it on at T+5: a signal source's update, which must re-shape it)
        if dt == 0 {
            let (min, size) = (run.world.min, run.world.size);
            for x in min.x..min.x + size[0] {
                for y in min.y..min.y + size[1] {
                    for z in min.z..min.z + size[2] {
                        let p = BlockPos::new(x, y, z);
                        let s = run.world.get(p).to_string();
                        if s.starts_with("minecraft:rail[") && potential_connections(&run.world, p) >= 3 {
                            t_shapes.push((p, s));
                        }
                    }
                }
            }
            if o.t_control || o.t_hold {
                let target = t_shapes.iter().map(|(p, _)| *p).find(|p| pockets.iter().any(|(px, _, _)| p.x == *px && p.y == 0)).ok_or("--t-control: no station T (a plain rail with 3 potential connections on a pocket's exit) to plant beside")?;
                // (G1 on e2d9333bc (B7): the T-hold control's constant source sits under the same T
                // the lever is planted beside)
                if o.t_hold {
                    let (px, _, pz) = o.layout.pockets()[0];
                    if target != BlockPos::new(px, 0, pz + 7) {
                        return Err(format!("--t-hold-control: the lever's T {target} is not the T over the constant source ({},0,{})", px, pz + 7));
                    }
                }
                let lp = target.rel(redsim::Dir::South);
                let sp = lp.rel(redsim::Dir::Down);
                if !run.world.get(lp).is_air() {
                    return Err(format!("--t-control: the lever cell {lp} south of the T {target} is not air ({})", run.world.get(lp)));
                }
                let mut ov = Vec::new();
                if run.world.get(sp).is_air() {
                    run.world.set_block(sp, glass.clone(), redsim::world::flags::ALL);
                    ov.push(format!("{sp} air -> glass"));
                }
                let lever = Block::parse("minecraft:lever[face=floor,facing=north,powered=false]").map_err(|e| e.to_string())?;
                run.world.set_block(lp, lever, redsim::world::flags::ALL);
                ov.push(format!("{lp} air -> lever[face=floor] (on at T+5, off at T+10)"));
                // (G1 on 834d6329d (T8): the control's overrides printed, before -> after)
                out.push(format!("the T-shape control's overrides beside the T {target} (G1 on 834d6329d (T8)): {}", ov.join(", ")));
                if o.t_hold {
                    out.push(format!("the T-hold control's constant source under the T, laid before the settle (form (C), torch last): {}", t_hold_overrides.join(", ")));
                }
                t_control_lever = Some(lp);
            }
        }
        // (the lever toggled ON at T+5 (it powers the T: the powered preference, north_west, no
        // change) and OFF at T+10: the update of an unpowered T re-shapes it to its unpowered
        // preference, the control's bite; 7b90c3f1's two-row control, on then off)
        if dt == 5 || dt == 10 {
            if let Some(lp) = t_control_lever {
                if !set_lever(&mut run.world, lp, dt == 5)? {
                    return Err(format!("--t-control: the lever at {lp} changed nothing (ironspec 04:07)"));
                }
            }
        }
        // (ironfarm 11:56: the planted carts and blocks, at T + k before the tick; G1 on e2d9333bc
        // (B4): each write read back: a no-op, a mismatch or a cart without its rider refuses)
        for (p, k) in o.plant_carts.iter().filter(|(_, k)| *k == dt) {
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", *p, Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"))?;
            // (G1 on 407a29682 (N8): the planted cart's position read back)
            let at = run.world.entities.list.iter().find(|e| e.id == id).map(|e| e.pos);
            if at.is_none_or(|a| (a.x - p[0]).abs() > 1e-9 || (a.y - p[1]).abs() > 1e-9 || (a.z - p[2]).abs() > 1e-9) {
                return Err(format!("--plant-cart {:.4},{:.4},{:.4}@{k}: the cart (id {id}) reads back at {at:?}", p[0], p[1], p[2]));
            }
            let rider = run.world.entities.list.iter().find(|e| e.id == id).and_then(|e| e.passengers.first().copied());
            if rider.is_none_or(|r| !run.world.entities.list.iter().any(|e| e.id == r && e.vehicle == Some(id))) {
                return Err(format!("--plant-cart {:.2},{:.2},{:.2}@{k}: the cart (id {id}) has no villager riding it", p[0], p[1], p[2]));
            }
            planted.push(format!("a ridden cart (id {id}, its rider {}) at ({:.2},{:.2},{:.2}) at T+{k}", rider.unwrap_or(0), p[0], p[1], p[2]));
            plants_done += 1;
        }
        for (c, b, k) in o.plant_blocks.iter().filter(|(_, _, k)| *k == dt) {
            let before = run.world.get(*c);
            if before == *b {
                return Err(format!("--plant-block {c}={}@{k}: the cell already holds that state (a no-op)", b.to_string().trim_start_matches("minecraft:")));
            }
            run.world.set_block(*c, b.clone(), redsim::world::flags::ALL);
            let after = run.world.get(*c);
            if after != *b {
                return Err(format!("--plant-block {c}={}@{k}: read back {} (outside the world's box, or the world changed it)", b.to_string().trim_start_matches("minecraft:"), after.to_string().trim_start_matches("minecraft:")));
            }
            planted.push(format!("{c} {} -> {} at T+{k} (read back)", before.to_string().trim_start_matches("minecraft:"), after.to_string().trim_start_matches("minecraft:")));
            plants_done += 1;
        }
        let ev0 = run.world.mob.events.len();
        // (each cart's motion before the tick: a contact is charged to the blocks on the side it
        // was moving toward)
        let before: Vec<(u32, Vec3)> = carts.iter().copied().chain(napper_cart).filter_map(|id| run.world.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (id, e.delta_movement))).collect();
        run.tick();
        // (G1 12:27: the detector-ended R_1 pulse's evidence: each lane's hold string (x,17,0)
        // pressed again after the R line starts, the new hold cart's rider: its first press tick;
        // ironspec 13:00: each hold string's last release before the R line rises, >= 2 gt before)
        for (k, (x, _, z)) in pockets.iter().enumerate() {
            let pressed = run.world.get(BlockPos::new(*x, o.hold_y + 1, *z)).to_string().contains("powered=true");
            // (G1 on 407a29682 (N6): the rise tick itself still updates, so a cart landing on a
            // string in that tick reads "pressed at the rise", and a release in it a lead under 2)
            if r_from.is_none_or(|a| a == dt) {
                if hold_was[k] && !pressed {
                    hold_release[k] = Some(dt + 1);
                }
                // (G1 on e2d9333bc (B2): pressed again before the rise, the earlier release no
                // longer counts: a release must follow)
                if !hold_was[k] && pressed {
                    hold_release[k] = None;
                }
                hold_was[k] = pressed;
            }
        }
        if let Some(a) = r_from {
            for (k, (x, _, z)) in pockets.iter().enumerate() {
                if hold_press[k].is_none() && dt >= a && run.world.get(BlockPos::new(*x, o.hold_y + 1, *z)).to_string().contains("powered=true") {
                    hold_press[k] = Some(dt + 1);
                }
            }
        }
        // (ironspec 13:04, the drive-integrity bar: each pocket drive's first powered tick from T (G1
        // on e2d9333bc (B8): not from REL, so an early drive is seen), against its clock slot REL +
        // offset k)
        for k in 0..3 {
            if drive_fire[k].is_none() && pocket_powered(&run.world, k) {
                drive_fire[k] = Some(dt + 1);
            }
        }
        // (the Ts' shapes at this tick's end: a change is a runtime re-shape)
        for (p, s0) in &t_shapes {
            let now = run.world.get(*p).to_string();
            if now != *s0 && !t_reshaped.iter().any(|(q, _)| q == p) {
                t_reshaped.push((*p, format!("{p} {} -> {} at T+{}", s0.trim_start_matches("minecraft:"), now.trim_start_matches("minecraft:"), dt + 1)));
            }
        }
        // (ironfarm's C-fix rows: every minecart this tick: a crossing out of a watched cell through
        // its west face, and a still cart above the off-rail line with no rail in its cell)
        if !o.watch_west.is_empty() || o.offrail_y.is_some() {
            let carts_now: Vec<(u32, Vec3, f64)> = run
                .world
                .entities
                .list
                .iter()
                .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
                .map(|e| (e.id, e.pos, (e.delta_movement.x * e.delta_movement.x + e.delta_movement.y * e.delta_movement.y + e.delta_movement.z * e.delta_movement.z).sqrt()))
                .collect();
            for (id, p, _v) in carts_now {
                let last = cart_last.get(&id).copied();
                let pc = BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                // (G1 on 407a29682 (N1): every watched cell's carts counted, and the face each left
                // by, so a "none" can't come from a cell no cart entered)
                if let Some(w) = watch_in.iter_mut().find(|w| w.0 == pc) {
                    if !w.1.contains(&id) {
                        w.1.push(id);
                    }
                }
                if let Some(q) = last {
                    let qc = BlockPos::new(q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32);
                    if qc != pc && o.watch_west.contains(&qc) {
                        let face = if pc.x < qc.x { "west" } else if pc.x > qc.x { "east" } else if pc.z < qc.z { "north" } else if pc.z > qc.z { "south" } else if pc.y > qc.y { "up" } else { "down" };
                        watch_exits.push(format!("cart {id} left {qc} by its {face} face at T+{}", dt + 1));
                        if face == "west" && !west_leaves.iter().any(|l| l.starts_with(&format!("cart {id} "))) {
                            west_leaves.push(format!("cart {id} left {qc} westward at T+{} (at ({:.2},{:.2},{:.2}))", dt + 1, p.x, p.y, p.z));
                        }
                    }
                }
                cart_last.insert(id, p);
                if let Some(y0) = o.offrail_y {
                    // (at rest by ironspec 10:42's stall rule: no horizontal move of 0.003 and no
                    // vertical move (a fall, or a rise in a bubble column, is a move) for more than 5
                    // ticks, frozen ticks not counted (G1 (N9)); on its rails by the game's own test,
                    // AbstractMinecart.tick: the cart's cell, or the cell below it when that holds a
                    // rail, is a rail)
                    let rest = last.is_some_and(|q| ((p.x - q.x).powi(2) + (p.z - q.z).powi(2)).sqrt() < 0.003 && (p.y - q.y).abs() < 1e-9);
                    let prev = cart_still.get(&id).copied().unwrap_or(0);
                    let still = if frozen_now { prev } else if rest && p.y >= y0 { prev + 1 } else { 0 };
                    cart_still.insert(id, still);
                    let on_rail = redsim::rails::is_rail(&run.world.get(pc)) || redsim::rails::is_rail(&run.world.get(pc.rel(redsim::Dir::Down)));
                    if still >= 6 && !rest_seen.contains(&id) {
                        rest_seen.push(id);
                    }
                    if still >= 6 && !on_rail && !offrail.iter().any(|(c, _)| *c == id) {
                        offrail.push((id, format!("cart {id} at rest off its rails at ({:.2},{:.2},{:.2}) from T+{} (its cell {pc} {}, the cell below {})", p.x, p.y, p.z, dt + 1 - 5, run.world.get(pc).to_string().trim_start_matches("minecraft:"), run.world.get(pc.rel(redsim::Dir::Down)).to_string().trim_start_matches("minecraft:"))));
                    }
                }
            }
        }
        // (G1 on a3b306a05 (5): the summon's scope from the WORLD, not the flag: every GolemTry
        // this tick counted whether or not --summon-check runs)
        {
            let from = if run.world.mob.events.len() >= ev0 { ev0 } else { 0 };
            summon_events += run.world.mob.events[from..].iter().filter(|ev| matches!(ev, redsim::entity::mob::MobEvent::GolemTry { .. })).count() as u32;
        }
        // (G1 on 82f9546f (c), the hold check: the held rail powered at the end of every tick inside
        // its window and unpowered outside it; a real neighbour update re-evaluating the bench's
        // undriven rail would drop it while the built one stays powered: a bench artefact, a failure;
        // the checklist audit (4): the window is the DECLARED one (the `held:` line's phase), not the
        // drive's own formula)
        for (p, intended) in std::mem::take(&mut hold_expect) {
            let now = run.world.get(p).to_string().contains("powered=true");
            if now != intended {
                hold_breaks += 1;
                if hold_break_first.is_none() {
                    hold_break_first = Some(format!("T+{dt}: {p} {} where the declared window says {}", if now { "powered" } else { "unpowered" }, if intended { "powered" } else { "unpowered" }));
                }
            }
        }
        // (the sight check, every tick T..S read at the tick's END (G1's audit (5)): every
        // villager -> scarer and villager -> villager segment against the live world's collision
        // shapes and the reference's (G1's audit (1): the verdict only through the world))
        // (ironspec 08:23 / G1 08:22, the at-entry bar's station half: every station rider's
        // activity every tick (its transitions printed), each entry into REST recorded with its
        // GOLEM_DETECTED_RECENTLY stamp, the first tick's REST (a night world open's blip)
        // included; a printed count until b8949c96's SetWalkTargetFromBlockMemory hook lands,
        // labelled, not judged)
        if rider_left.len() < riders.len() {
            rider_left.resize(riders.len(), None);
        }
        for n in 0..riders.len() {
            let Some(id) = riders[n] else { continue };
            // (G1 on a3b306a05 (8): an id-keyed read that finds no entity fails the row; G1 on
            // 834d6329d (T4): the one exception is a departing rider the bench itself sank past the
            // exit (set at the sink, n < 3): its later reads are out of that world's scope)
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed) else {
                if rider_left[n].is_none() {
                    missing_reads += 1;
                }
                continue;
            };
            let EntityKind::Mob(m) = &e.kind else { continue };
            let Some(b) = m.brain.as_deref() else {
                missing_reads += 1;
                continue;
            };
            // (ironspec 09:03 / G1: each run's least distance from a HOME holder's feet to its
            // HOME head block's centre, printed: the exposure bar's reach (strictly < 16))
            if let Some(h) = b.home {
                let c = Vec3::new(h.x as f64 + 0.5, h.y as f64 + 0.5, h.z as f64 + 0.5);
                let d = ((e.pos.x - c.x).powi(2) + (e.pos.y - c.y).powi(2) + (e.pos.z - c.z).powi(2)).sqrt();
                home_least = Some(home_least.map_or(d, |m: f64| m.min(d)));
            }
            let a = b.activity;
            if act_prev.len() <= n {
                act_prev.resize(n + 1, None);
            }
            let prev = act_prev[n];
            if a == redsim::entity::mob::villager::Activity::Rest && prev != Some(a) {
                rest_entries += 1;
                if rest_list.len() < 8 {
                    rest_list.push(format!("rider {n} T+{dt} at ({:.2},{:.2},{:.2}), its stamp {}", e.pos.x, e.pos.y, e.pos.z, b.golem_detected.map_or("none".to_string(), |t| format!("ttl {t}"))));
                }
            }
            if prev != Some(a) && act_timeline.len() < 48 {
                act_timeline.push(format!("rider {n} {}@T+{dt}", a.name()));
            }
            act_prev[n] = Some(a);
        }
        if dt < s_rel {
            if let Some(reference) = o.sight_ref.as_deref() {
                let (hits, rays, _) = sight_crossings_world(&run.world, reference);
                sight_rays += rays;
                sight_hit_count += hits.len() as u32;
                if sight_hits.len() < 12 {
                    sight_hits.extend(hits.into_iter().take(12).map(|h| format!("T+{}: {h}", dt + 1)));
                }
            }
        }
        // (ironspec 21:25 (4): the carts' block contacts, each tick: a cart whose move was stopped
        // horizontally, against the blocks with a collision shape its box meets when widened by
        // 0.05 on the side of its motion (the checklist audit (5)/(9): the stop leaves the box
        // against the face (collide() clamps it there), so 0.05 reaches the face's block and not the
        // next; 1e-6 is a motion component's sign (none: no side); the y inset 0.01 keeps the cells
        // under the box's bottom and over its top out. Interim: the contacts move to ironfarm's
        // shared face filter when it lands; until then these contact results are not cited)
        for (id, v) in &before {
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id && !e.removed) else { continue };
            if !e.horizontal_collision {
                continue;
            }
            let bb = e.bb;
            let side = |lo: f64, hi: f64, d: f64| if d > 1e-6 { ((hi + 0.05).floor() as i32, (hi + 0.05).floor() as i32) } else if d < -1e-6 { ((lo - 0.05).floor() as i32, (lo - 0.05).floor() as i32) } else { (lo.floor() as i32, (hi - 1e-9).floor() as i32) };
            let ((x0, x1), (z0, z1)) = (side(bb.min_x, bb.max_x, v.x), side(bb.min_z, bb.max_z, v.z));
            let (y0, y1) = ((bb.min_y + 0.01).floor() as i32, (bb.max_y - 0.01).floor() as i32);
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        let c = BlockPos::new(x, y, z);
                        let b = run.world.get(c);
                        if !redsim::behavior::collides(&b) {
                            continue;
                        }
                        let declared = o.stops.contains(&(x, y, z));
                        if !declared {
                            undeclared_contacts += 1;
                        }
                        match block_contacts.iter_mut().find(|k| k.0 == c) {
                            Some(k) => k.2 += 1,
                            None => block_contacts.push((c, b.to_string(), 1, declared)),
                        }
                    }
                }
            }
        }
        if o.summon_check {
            let from = if run.world.mob.events.len() >= ev0 { ev0 } else { 0 };
            // (every GolemTry this tick; the stand's first one, at any tick, must fall on the
            // summon's own clock, S (the checklist audit (5)))
            let tries: Vec<(i64, u32, u32, bool)> = run.world.mob.events[from..]
                .iter()
                .filter_map(|ev| match ev {
                    redsim::entity::mob::MobEvent::GolemTry { t, villager, willing, golem, .. } => Some((*t, *villager, *willing, golem.is_some())),
                    _ => None,
                })
                .collect();
            if s_try.is_none() {
                s_try = tries.iter().find(|(_, v, _, _)| pocket_of(&run.world, &pockets, *v).is_some()).map(|(t, _, _, _)| t - t_zero);
            }
            // (G1 on 834d6329d (T4): a caller whose id is not found can't be attributed: a failed
            // read, not a "foreign" call)
            missing_reads += tries.iter().filter(|(_, v, _, _)| redsim::entity::index_of(&run.world, *v).is_none()).count() as u32;
            let now: Vec<(u32, u32, bool)> = tries.iter().filter(|(t, ..)| *t == s_time).map(|(_, v, n, b)| (*v, *n, *b)).collect();
            let calls: Vec<SummonCall> = now
                .iter()
                .map(|(v, n, born)| SummonCall { s: s_time - t_zero, stand: 0, pocket: pocket_of(&run.world, &pockets, *v), willing: *n, threshold: None, tries: None, born: *born })
                .collect();
            for e in run.world.entities.list.iter_mut() {
                if !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::IronGolem) {
                    e.removed = true;
                    golems_removed += 1;
                }
            }
            if let Some(pre) = pre {
                // (the reseat control's attribution at S, through the gate's own `pocket_of`: the
                // fresh villager is P0's rider, the dismounted old one no pocket's)
                if let (Some(f), Some(old)) = (reseat_id, reseat_old) {
                    reseat_attr = Some((pocket_of(&run.world, &pockets, f), pocket_of(&run.world, &pockets, old)));
                }
                // (the stamps read after S's tick: a departing rider stamped by this S's summon is
                // the S - 3 bar's own failure, read from its memory, not inferred from boxes)
                if let Some(before) = dep_golem_pre {
                    for n in 0..3 {
                        if o.dep_absent[n] || rider_left.get(n).is_some_and(|l| l.is_some()) {
                            continue;
                        }
                        // (G1 on a3b306a05 (8): a rider not found before or after S fails the row; a
                        // rider the bench sank past the exit in a world with no lift is out of its
                        // scope, printed)
                        match (before[n], golem_mem(&run.world, riders[n])) {
                            (Some(b), Some(Some(a))) => {
                                if b.is_none_or(|b| a > b) {
                                    dep_stamped += 1;
                                }
                            }
                            (Some(_), Some(None)) => {}
                            _ => missing_reads += 1,
                        }
                    }
                }
                let stand_calls = calls.iter().filter(|c| c.pocket.is_some()).count();
                // (G1's audit (4): only the stand's callers' golems count, not a departing rider's
                // or the napper's)
                let born = calls.iter().any(|c| c.born && c.pocket.is_some());
                let eligible = pre.iter().filter(|r| r.eligible()).count();
                let class = if born {
                    "golem born"
                } else if stand_calls > 0 {
                    "spot miss"
                } else if frozen_now {
                    "frozen at S (no entity ticks: no summon this S)"
                } else if !pre.iter().any(|r| r.seated && r.scared) {
                    "no panicking caller"
                } else if !pre.iter().any(|r| r.ticks) {
                    "no caller ticking (the trigger's skip)"
                } else if !pre.iter().any(|r| r.ticks && r.wants) {
                    "caller blocked by its own memory"
                } else if !pre.iter().any(|r| r.eligible()) {
                    "fewer than 3 willing"
                } else {
                    "no call (unexplained)"
                };
                summon_due.push(SummonDue {
                    s: s_time - t_zero,
                    recovery: recovery_s,
                    eligible: eligible as u8,
                    calls: stand_calls as u8,
                    born,
                    covered: if born { eligible.saturating_sub(stand_calls) as u8 } else { 0 },
                    class,
                    riders: pre.iter().enumerate().map(|(k, r)| r.text(k)).collect(),
                    spots_pad: [0, 1, 2].map(|k| pre.get(k).map_or(0, |r| r.spots_pad)),
                    spots_off: [0, 1, 2].map(|k| pre.get(k).map_or(0, |r| r.spots_off)),
                    restart_before: o.restarts.iter().map(|r| r.0).filter(|t| *t <= s_time - t_zero).max(),
                    willing_in_box: {
                        let mut v: Vec<(u32, [f64; 3], bool)> = Vec::new();
                        for w in pre.iter().flat_map(|r| r.willing_in_box.iter()) {
                            if !v.iter().any(|x| x.0 == w.0) {
                                v.push(*w);
                            }
                        }
                        v
                    },
                    foreign_calls: calls.iter().filter(|c| c.pocket.is_none()).count() as u8,
                    calls_by: now.iter().zip(calls.iter()).map(|((v, _, _), c)| (*v, c.pocket)).collect(),
                });
            }
            summon_calls.extend(calls);
        }
        // (the panic window: per summoner and scarer, after this tick: in the scarer's type range;
        // eye to eye within 16 and the villager's box inflated by 16 meeting the scarer's)
        if !o.panic_posts.is_empty() {
            let mut row: [Vec<(bool, bool, bool)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
            let mut prow: [Option<Vec3>; 3] = [None; 3];
            for k in 0..3 {
                let Some(r) = riders[3 + k] else { continue };
                let Some(e) = run.world.entities.list.iter().find(|e| e.id == r && !e.removed) else { continue };
                prow[k] = Some(e.pos);
                let eye = Vec3::new(e.pos.x, e.pos.y + 1.62, e.pos.z);
                for (kind, s) in &o.panic_posts {
                    let range = if *kind == 'z' { 8.0 } else { 15.0 };
                    let d = ((e.pos.x - s.x).powi(2) + (e.pos.y - s.y).powi(2) + (e.pos.z - s.z).powi(2)).sqrt();
                    let fr = o.follow_range;
                    let inbox = (e.pos.x - s.x).abs() < fr + 0.6 && (e.pos.z - s.z).abs() < fr + 0.6 && s.y < e.pos.y + 1.95 + fr && s.y + 1.95 > e.pos.y - fr;
                    let seye = Vec3::new(s.x, s.y + if *kind == 'z' { 1.74 } else { 1.6575 }, s.z);
                    let los16 = d <= 16.0 && redsim::entity::mob::los::has_line_of_sight(&run.world, eye, seye);
                    row[k].push((d <= range, inbox, los16));
                }
            }
            panic_log.push(row);
            panic_pos.push(prow);
        }
        // (the watched cells: any cart's or villager's box meeting a cell's unit box)
        if !o.watch.is_empty() {
            for e in run.world.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_) | EntityKind::Mob(_))) {
                let b = e.bb;
                if let Some(c) = o.watch.iter().find(|(x, y, z)| {
                    b.min_x < (*x + 1) as f64 && b.max_x > *x as f64 && b.min_y < (*y + 1) as f64 && b.max_y > *y as f64 && b.min_z < (*z + 1) as f64 && b.max_z > *z as f64
                }) {
                    watch_hits += 1;
                    if watch_first.is_none() {
                        watch_first = Some((dt, *c));
                    }
                }
            }
        }
        // (ironspec 13:09, golem-stamp path (a): in the module (the merge row and the shared lift
        // laid), each departing rider's box against every next-batch caller's 10-box, the game's
        // test: the caller (the arriving rider at its pocket's rest) inflated by 10; the rider's
        // feet past y 12.0125 clear it vertically, every chunk cell below it is in pocket x -1's)
        if o.track_lift {
            // (the checklist audit (6): the callers' LIVE boxes this tick, the arriving riders'
            // boxes inflated by 10 (the summon's getEntitiesOfClass(Villager, caller.bb.inflate(10)));
            // G1 on a3b306a05: a caller's id not found is a failed read, and a tick without all three
            // callers is no sample (the bar NOT JUDGED short of its samples))
            let mut callers: Vec<redsim::entity::Aabb> = Vec::new();
            for k in 3..6 {
                match riders.get(k).copied().flatten().and_then(|id| run.world.entities.list.iter().find(|e| e.id == id && !e.removed)) {
                    Some(e) => callers.push(e.bb.inflate(10.0, 10.0, 10.0)),
                    None => missing_reads += 1,
                }
            }
            for n in 0..3 {
                if o.dep_absent[n] {
                    continue;
                }
                // (a departing rider missing, removed or never spawned, without dep_absent: a
                // failure, G1 on 4a09c256)
                let Some(r) = riders[n] else {
                    rider_missing[n] = true;
                    continue;
                };
                let Some(e) = run.world.entities.list.iter().find(|e| e.id == r && !e.removed) else {
                    rider_missing[n] = true;
                    continue;
                };
                let p = e.pos;
                let inside = callers.iter().any(|cb| e.bb.intersects(*cb));
                // (the window: iterations 0 .. S - 1, the ends of game ticks T+1 .. S)
                if dt < s_rel && callers.len() == 3 {
                    lift_samples[n] += 1;
                }
                if inside && dt < s_rel {
                    lift_last[n] = Some(dt);
                }
                // (path (a): a rider back in a caller's box after it was out, lane 2's dip at its
                // column entry: the first tick)
                if inside && lift_y12[n].is_some() && lift_dip[n].is_none() {
                    lift_dip[n] = Some(dt);
                }
                if dt == s_rel - 1 {
                    lift_in100[n] = inside;
                }
                if dt == 80 {
                    lift_y80[n] = Some(p.y);
                }
                if dt == s_rel - 1 {
                    lift_y100[n] = Some(p.y);
                }
                if lift_merge[n].is_none() && p.z >= pockets[n].2 as f64 + 7.0 {
                    lift_merge[n] = Some(dt);
                }
                if lift_entry[n].is_none() && run.world.get(BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)).to_string().contains("bubble_column") {
                    lift_entry[n] = Some(dt);
                }
                if lift_y12[n].is_none() && p.y >= 12.0125 {
                    lift_y12[n] = Some(dt);
                }
                if lift_y12[n].is_none() {
                    lift_track[n].push((dt, p));
                }
                if lift_merge[n].is_some() {
                    if let EntityKind::Mob(m) = &e.kind {
                        lift_air[n] = Some(lift_air[n].map_or(m.air, |a: i32| a.min(m.air)));
                    }
                }
                // (the cart: its exit from the column at the top; at the hold, its rest)
                let Some(c) = e.vehicle.and_then(|v| run.world.entities.list.iter().find(|x| x.id == v && !x.removed)) else { continue };
                let cb = BlockPos::new(c.pos.x.floor() as i32, c.pos.y.floor() as i32, c.pos.z.floor() as i32);
                let in_col = run.world.get(cb).to_string().contains("bubble_column");
                if lift_entry[n].is_some() && lift_top[n].is_none() && !in_col && c.pos.y >= 15.0 {
                    lift_top[n] = Some((dt, c.pos.y, c.delta_movement.y));
                }
                if let Some((hx, hy, hz)) = o.hold_rail.filter(|_| n == 2) {
                    if in_col && hold_on.is_none() {
                        hold_overshoot = true;
                    }
                    if cb == BlockPos::new(hx, hy, hz) && hold_on.is_none() {
                        // (at rest: the braked rail halves the cart's motion every tick, so under
                        // 0.01 a tick it is stopped within one tick of the reading (the audit (5)))
                        let still = c.delta_movement.x.abs() + c.delta_movement.z.abs() < 0.01;
                        if hold_first.is_none() {
                            hold_first = Some(dt);
                        }
                        match hold_rest {
                            None if still => hold_rest = Some((dt, c.pos.x, p.y)),
                            Some((_, x0, _)) => hold_drift = hold_drift.max((c.pos.x - x0).abs()),
                            _ => {}
                        }
                    }
                    if dt <= 160 && dt % 2 == 0 {
                        hold_track.push((dt, c.pos, c.delta_movement));
                    }
                }
            }
            // (the departing carts past their station's exit, and the napper (index 3): clearance
            // and push contacts)
            let mut carts: Vec<(usize, redsim::entity::vec3::Aabb)> = (0..3)
                .filter(|n| !o.dep_absent[*n] && lift_merge[*n].is_some())
                .filter_map(|n| {
                    let r = riders[n]?;
                    let e = run.world.entities.list.iter().find(|e| e.id == r && !e.removed)?;
                    let c = run.world.entities.list.iter().find(|x| Some(x.id) == e.vehicle && !x.removed)?;
                    Some((n, c.bb))
                })
                .collect();
            if let Some(nc) = napper_cart.and_then(|id| run.world.entities.list.iter().find(|e| e.id == id && !e.removed)) {
                let cb = BlockPos::new(nc.pos.x.floor() as i32, nc.pos.y.floor() as i32, nc.pos.z.floor() as i32);
                let in_col = run.world.get(cb).to_string().contains("bubble_column");
                // (off its hold: more than 0.01 off its rest's z, as the R line's launch (above))
                if nap_off.is_none() && (nc.pos.z - 14.5).abs() > 0.01 {
                    nap_off = Some(dt);
                }
                if nap_entry.is_none() && in_col {
                    nap_entry = Some(dt);
                }
                if nap_entry.is_some() && nap_y12.is_none() && nc.pos.y >= 12.0 {
                    nap_y12 = Some(dt);
                }
                if nap_entry.is_some() && nap_top.is_none() && !in_col && nc.pos.y >= 15.0 {
                    nap_top = Some(dt);
                }
                if let Some(rid) = nc.passengers.first() {
                    if let Some(EntityKind::Mob(m)) = run.world.entities.list.iter().find(|x| x.id == *rid).map(|x| &x.kind) {
                        lift_air[3] = Some(lift_air[3].map_or(m.air, |a: i32| a.min(m.air)));
                    }
                }
                carts.push((3, nc.bb));
            }
            for i in 0..carts.len() {
                for j in i + 1..carts.len() {
                    let (a, b) = (carts[i].1, carts[j].1);
                    let gy = (a.min_y - b.max_y).max(b.min_y - a.max_y);
                    if gy >= 0.0 {
                        continue;
                    }
                    let gx = (a.min_x - b.max_x).max(b.min_x - a.max_x);
                    let gz = (a.min_z - b.max_z).max(b.min_z - a.max_z);
                    let clear = gx.max(gz);
                    lift_clear = Some(lift_clear.map_or(clear, |c: f64| c.min(clear)));
                    if gx < 0.2 && gz < 0.2 {
                        lift_push += 1;
                        if lift_push_at.is_none() {
                            lift_push_at = Some((dt, carts[i].0, carts[j].0));
                        }
                    }
                }
            }
            if let Some((hx, hy, hz)) = o.hold_rail {
                let s = run.world.get(BlockPos::new(hx, hy, hz)).to_string();
                // (the hold rail and the cell above it, the lift wall's opening: dry)
                let above = run.world.get(BlockPos::new(hx, hy + 1, hz)).to_string();
                if !s.contains("powered_rail") || s.contains("waterlogged=true") || above.contains("minecraft:water") || above.contains("waterlogged=true") {
                    hold_wet += 1;
                }
            }
        }
        // (option (b)'s candidates: lane k's arriving rider against each, after this tick; the
        // nearest any rider comes to each seat)
        if let Some(k) = o.panic_search {
            let pos_of = |id: Option<u32>| id.and_then(|r| run.world.entities.list.iter().find(|e| e.id == r && !e.removed)).map(|e| e.pos);
            let rider = pos_of(riders[3 + k]);
            let all: Vec<Vec3> = riders.iter().filter_map(|r| pos_of(*r)).collect();
            let fr = o.follow_range;
            for c in cands.iter_mut() {
                for p in &all {
                    c.nearest = c.nearest.min(((p.x - c.seat.x).powi(2) + (p.y - c.seat.y).powi(2) + (p.z - c.seat.z).powi(2)).sqrt());
                }
                let Some(p) = rider else {
                    c.log.push((false, false, false));
                    continue;
                };
                let d = ((p.x - c.seat.x).powi(2) + (p.y - c.seat.y).powi(2) + (p.z - c.seat.z).powi(2)).sqrt();
                let inbox = (p.x - c.seat.x).abs() < fr + 0.6 && (p.z - c.seat.z).abs() < fr + 0.6 && c.seat.y < p.y + 1.95 + fr && c.seat.y + 1.95 > p.y - fr;
                let reye = Vec3::new(p.x, p.y + 1.62, p.z);
                let los = d <= 16.0 && c.exits_open(reye) && redsim::entity::mob::los::has_line_of_sight(&run.world, reye, c.eye);
                c.log.push((d <= 8.0, inbox, los));
            }
        }
        if dt % 100 == 0 {
            let mut v = Vec::new();
            at_summon(&run.world, &mut v);
            summon_off.extend(v.into_iter().map(|s| format!("T+{dt} (after): {s}")));
        }
        let live_after = dt + 1 - frozen_ticks;
        let clock_after = if o.l1 { dt + 1 } else { live_after };
        for k in 0..3 {
            if string_on(&run.world, k) {
                on_since[k].get_or_insert(clock_after);
            } else {
                on_since[k] = None;
            }
            // (the exit detector: risen after this pocket's release)
            if o.layout == DropLayout::Station {
                let (x, y, z) = pockets[k];
                let on = run.world.get(BlockPos::new(x, y, z + 2)).to_string().contains("powered=true");
                if on && !exit_was[k] && clock_after > o.rel + o.stagger[k] {
                    exit_seen[k] = true;
                }
                exit_was[k] = on;
            }
        }
        let w = &run.world;
        // (R_1's rails: powered after this tick?)
        if o.refill.is_some() && o.layout == DropLayout::Station {
            for (k, (x, _, z)) in pockets.iter().enumerate() {
                let pw = w.get(BlockPos::new(*x, o.hold_y + 1, *z + 1)).to_string().contains("powered=true");
                if pw && r1_on[k].is_none() {
                    r1_on[k] = Some(dt);
                }
                if !pw && r1_on[k].is_some() && r1_off[k].is_none() {
                    r1_off[k] = Some(dt);
                }
            }
        }
        // (the next batch: in the hold at rest, or fallen through)
        for (j, id) in next_carts.iter().enumerate() {
            let Some((p, v)) = pos(w, *id) else { continue };
            if j < 3 {
                if r_from.is_none() {
                    next_start[j] = Some(p);
                } else if next_moved[j].is_none() && next_start[j].is_some_and(|s| ((p.x - s.x).powi(2) + (p.y - s.y).powi(2) + (p.z - s.z).powi(2)).sqrt() > 0.01) {
                    next_moved[j] = Some(dt);
                }
                if r_from.is_some() && next_off_cell[j].is_none() {
                    let (rx, _, rz) = pockets[j];
                    let cell = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                    if cell != (rx, o.hold_y + 1, rz + 1) {
                        next_off_cell[j] = Some(dt);
                    }
                }
            }
            let s3 = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
            if r_from.is_some() {
                if p.y < o.hold_y as f64 - 0.5 {
                    next_fell[j] = true;
                }
                if next_rest[j].is_none() && s3 < 1e-3 && p.y >= o.hold_y as f64 && p.y < o.hold_y as f64 + 0.5 {
                    next_rest[j] = Some((dt, p));
                }
                for id2 in carts.iter() {
                    if let Some((q, _)) = pos(w, *id2) {
                        let d = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
                        if next_closest.is_none_or(|c| d < c) {
                            next_closest = Some(d);
                        }
                    }
                }
            }
        }
        let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(w, *id)).collect();
        // (G1 on 834d6329d (T4): the napper's cart not found fails the row too)
        if napper_cart.is_some_and(|nc| pos(w, nc).is_none()) {
            missing_reads += 1;
        }
        let mut sink_now: Vec<usize> = Vec::new();
        for n in 0..6 {
            // (G1 on 834d6329d (T4): a station cart not found fails the row, but for a departing one
            // the bench sank past the exit in a world with no lift)
            let Some((p, v)) = st[n] else {
                if !(n < 3 && exits[n].is_some() && !o.track_lift) && !(o.dep_absent.get(n).copied().unwrap_or(false)) {
                    missing_reads += 1;
                }
                continue;
            };
            if n < 3 {
                // (the launch: the first tick the departing cart is more than 0.05 off its pocket's
                // centre: the bench's "+2" convention against tick-exact's official ride, whose
                // position first changes at +1 by under 0.05 and passes 0.05 at +2 (NOTES: the
                // official ride matches redsim to the tick, +-0.0005); the checklist audit (5))
                let (px, _, pz) = pockets[n];
                if launch_at[n].is_none() && ((p.x - (px as f64 + 0.5)).abs() > 0.05 || (p.z - (pz as f64 + 0.5)).abs() > 0.05) {
                    launch_at[n] = Some(dt);
                }
                if exits[n].is_none() && o.layout.departed(p) {
                    exits[n] = Some((dt, (v.x * v.x + v.z * v.z).sqrt()));
                    // (the module: the merge row and the lift laid, the cart rolls on)
                    if !o.track_lift {
                        sink_now.push(n);
                    }
                }
                continue;
            }
            let k = n - 3;
            if opened[k].is_none() {
                continue;
            }
            let (px, py, pz) = pockets[k];
            if landed[k].is_none() {
                let (dx, dz) = (p.x - (px as f64 + 0.5), p.z - (pz as f64 + 0.5));
                fall_off = fall_off.max((dx * dx + dz * dz).sqrt());
                for (kind, s) in &scarers_all {
                    if *kind != 'z' {
                        continue;
                    }
                    let (ux, uz) = (s.x - (px as f64 + 0.5), s.z - (pz as f64 + 0.5));
                    let l = (ux * ux + uz * uz).sqrt();
                    if l > 1e-9 {
                        fall_toward = fall_toward.max((dx * ux + dz * uz) / l);
                    }
                }
            }
            if landed[k].is_none() && p.y < py as f64 + 1.0 {
                landed[k] = Some((dt, fall_v[k], pocket_powered(w, k)));
            }
            fall_v[k] = -v.y;
            let s3 = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
            if s3 < 1e-3 && (p.z - (pz as f64 + 0.5)).abs() < 0.6 && p.y < py as f64 + 1.0 {
                rest_since[k].get_or_insert(dt);
            } else {
                rest_since[k] = None;
            }
            // (the arriving cart's position at S, printed: 9fc3155f's pocket positions)
            if dt == s_rel {
                rest_at_s[k] = Some(p);
            }
            for m in 0..3 {
                if let Some((pd, _)) = st[m] {
                    let d = ((p.x - pd.x).powi(2) + (p.y - pd.y).powi(2) + (p.z - pd.z).powi(2)).sqrt();
                    if closest.is_none_or(|c| d < c.0) {
                        closest = Some((d, dt, k, m));
                    }
                }
            }
        }
        let live_cart = |n: usize| n < 3 || opened[n - 3].is_some();
        let yaws: Vec<f32> = carts.iter().map(|id| w.entities.list.iter().find(|e| e.id == *id && !e.removed).map_or(0.0, |e| e.y_rot)).collect();
        // (the night-panic coverage: each rider's phase, whether a scarer is in range and sight;
        // the next batch's riders count in the hold phase once in their holds, and always for the
        // pillager's distance; a pinned pillager 'P' anywhere in its 1 x 1 cell (its centre +-0.2):
        // a rider is covered only if it is from every such position, and the pillager's nearest
        // seen villager is over them all)
        if !scarers_all.is_empty() {
            let mut who: Vec<(usize, u32, Option<usize>)> = Vec::new();
            for n in 0..6 {
                let Some(r) = riders[n] else { continue };
                let Some((p, _)) = st[n] else { continue };
                let phase = if n < 3 {
                    if exits[n].is_some() {
                        continue;
                    }
                    let (px, py, pz) = pockets[n];
                    if (p.z - (pz as f64 + 0.5)).abs() < 0.6 && (p.x - (px as f64 + 0.5)).abs() < 0.6 && p.y < py as f64 + 1.0 { 2 } else { 3 }
                } else {
                    let k = n - 3;
                    if opened[k].is_none() {
                        0
                    } else if landed[k].is_none() {
                        1
                    } else {
                        2
                    }
                };
                who.push((n, r, Some(phase)));
            }
            for (j, id) in next_carts.iter().enumerate() {
                let Some(r) = next_riders.get(j).copied().flatten() else { continue };
                let Some((p, _)) = pos(w, *id) else { continue };
                let in_hold = p.y >= o.hold_y as f64 - 0.1 && p.y < o.hold_y as f64 + 0.5;
                who.push((6 + j, r, in_hold.then_some(0)));
            }
            for (slot, r, phase) in who {
                let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else { continue };
                let eye = Vec3::new(e.pos.x, e.pos.y + 1.62, e.pos.z);
                let mut covered = false;
                for (kind, s) in &scarers_all {
                    let zombie = *kind == 'z';
                    let samples: Vec<Vec3> = if *kind == 'P' {
                        [(0.0, 0.0), (0.2, 0.2), (0.2, -0.2), (-0.2, 0.2), (-0.2, -0.2)].iter().map(|(dx, dz)| Vec3::new(s.x + dx, s.y, s.z + dz)).collect()
                    } else {
                        vec![*s]
                    };
                    let range = if zombie { 8.0 } else { 15.0 };
                    let mut every = true;
                    for q in &samples {
                        let d = ((e.pos.x - q.x).powi(2) + (e.pos.y - q.y).powi(2) + (e.pos.z - q.z).powi(2)).sqrt();
                        if zombie && zombie_any.is_none_or(|m: f64| d < m) {
                            zombie_any = Some(d);
                            // (where: the tick, the lane (slot 3..5 arriving, 6..8 the next
                            // batch), the rider's feet, the zombie, the rider's phase)
                            zombie_any_at = Some((dt, slot, e.pos, *q, phase));
                        }
                        if d > 16.0 {
                            every = false;
                            continue;
                        }
                        // (eyes: a zombie's 1.74, a pillager's 1.6575: 0.85 x its 1.95, G1)
                        let sees = redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(q.x, q.y + if zombie { 1.74 } else { 1.6575 }, q.z));
                        if sees && !zombie {
                            pillager_near = Some(pillager_near.map_or(d, |m: f64| m.min(d)));
                        }
                        if sees && d <= range {
                            if zombie {
                                zombie_near = Some(zombie_near.map_or(d, |m: f64| m.min(d)));
                            }
                        } else {
                            every = false;
                        }
                    }
                    if every {
                        covered = true;
                    }
                }
                let Some(phase) = phase else { continue };
                if covered {
                    calm_run[slot] = 0;
                } else {
                    calm_run[slot] += 1;
                    calm_max[phase] = calm_max[phase].max(calm_run[slot]);
                    if phase == 0 {
                        let lane = if slot >= 6 { slot - 6 } else { slot - 3 };
                        hold_lane[lane] = hold_lane[lane].max(calm_run[slot]);
                    }
                    if phase == 1 && (3..6).contains(&slot) {
                        calm_y[slot - 3] = Some(calm_y[slot - 3].map_or((e.pos.y, e.pos.y), |(a, b): (f64, f64)| (a.min(e.pos.y), b.max(e.pos.y))));
                    }
                }
            }
        }
        for a in 0..6 {
            for c in 0..6 {
                if a == c || !live_cart(a) || !live_cart(c) {
                    continue;
                }
                let (Some((pa, va)), Some((pc, vc))) = (st[a], st[c]) else { continue };
                let d = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2) + (pa.z - pc.z).powi(2)).sqrt();
                let sc = (vc.x * vc.x + vc.z * vc.z).sqrt();
                // (lift3's box test, G1: the carts' 0.98 x 0.7 boxes, one inflated 0.2 sideways,
                // meeting, and the line between the centres within ~37 degrees of either cart's yaw:
                // |cos| >= 0.8, AbstractMinecart.push)
                let (hx, hz) = (pc.x - pa.x, pc.z - pa.z);
                let hl = (hx * hx + hz * hz).sqrt();
                let along = |y: f32| {
                    let r = (y as f64).to_radians();
                    hl < 1e-9 || ((hx * r.cos() + hz * r.sin()) / hl).abs() >= 0.8
                };
                let now = hx.abs() < 1.18 && hz.abs() < 1.18 && (pa.y - pc.y).abs() < 0.7 && (along(yaws[a]) || along(yaws[c]));
                let _ = d;
                // (sc: the boxes' clearance; under 0.05 (a twentieth of the 0.2 inflation G1's
                // contact rule uses) the carts are touching, not passing: the checklist audit (5))
                if now && !touching[a][c] && sc < 0.05 && dt > 0 {
                    let sa = prev_v[a].max((va.x * va.x + va.y * va.y + va.z * va.z).sqrt());
                    contact_max = contact_max.max(sa);
                    contacts.push(format!("T+{dt}: {} cart {} into {} cart {} at {sa:.3}", if a < 3 { "departing" } else { "arriving" }, a % 3, if c < 3 { "departing" } else { "arriving" }, c % 3));
                }
                touching[a][c] = now;
            }
        }
        for n in 0..6 {
            prev_v[n] = st[n].map_or(0.0, |(_, v)| (v.x * v.x + v.y * v.y + v.z * v.z).sqrt());
            let Some(r) = riders[n] else { continue };
            // (G1 on 834d6329d (T4): a rider not found here fails too, but for the one the bench sank)
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else {
                if rider_left.get(n).is_none_or(|l| l.is_none()) {
                    missing_reads += 1;
                }
                continue;
            };
            let hp = if let EntityKind::Mob(m) = &e.kind { m.health } else { 0.0 };
            if health[n].is_some_and(|h0| hp < h0) {
                hurt += 1;
            }
            health[n] = Some(hp);
            if e.vehicle.is_none() {
                unseated += 1;
            }
        }
        if o.trace {
            let strings: String = (0..3).map(|k| if string_on(w, k) { '1' } else { '0' }).collect();
            out.push(format!("T+{dt} strings {strings}: {}", st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}/{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt(), v.y))).collect::<Vec<_>>().join(" | ")));
        }
        for n in sink_now {
            let ps: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == carts[n]).map(|e| e.passengers.clone()).unwrap_or_default();
            for e in run.world.entities.list.iter_mut() {
                if e.id == carts[n] || ps.contains(&e.id) {
                    e.removed = true;
                }
            }
            // (the bench sinks a departed cart and its rider in a world with no lift: that rider's
            // later reads are out of the world's scope, printed (G1 on a3b306a05 (8)))
            if let Some(slot) = rider_left.get_mut(n)
                && slot.is_none()
            {
                *slot = Some(format!("rider {n} departed at T+{dt} (sunk by the bench past the exit, a world with no lift)"));
            }
        }
    }
    let boundary = summon_boundary(&frozen_hist);
    let mut stats = DropStats::default();
    stats.summon_calls = summon_calls;
    stats.summon_due = summon_due;
    stats.golems_removed = golems_removed;
    stats.sight_hits = sight_hits;
    stats.sight_control = sight_control;
    stats.sight_hit_count = sight_hit_count;
    stats.missing_riders = rider_missing.iter().filter(|m| **m).count() as u32;
    stats.hold_breaks = hold_breaks;
    stats.hold_break_first = hold_break_first;
    stats.held_edges = held_edges;
    stats.standin_cells = levers.iter().zip(stand_ins.iter()).map(|((p, _, _), d)| ([p.x, p.y, p.z], d.clone())).collect();
    stats.fluid_cells = fluid_cells;
    stats.fluid_scheduled = fluid_scheduled;
    stats.s_rel = s_rel;
    stats.s_try = s_try;
    stats.lift_tracked = [0, 1, 2].map(|n| o.track_lift && !o.dep_absent[n]);
    stats.lift_samples = lift_samples;
    stats.lift_in_world = lift_in_world;
    stats.track_lift = o.track_lift;
    stats.hold_driven = o.hold_drive.is_some();
    stats.napper_run = o.napper.is_some();
    stats.napper_held = o.napper.is_some() && o.napper_held.is_some();
    stats.summon_checked = o.summon_check;
    stats.sight_scope = sight_scope;
    stats.sight_rays = sight_rays;
    stats.held_edge_count = held_edge_count;
    stats.lever_noops = lever_noops;
    stats.dep_stamped = dep_stamped;
    stats.activity_wrong = activity_wrong;
    stats.independent = o.freeze.is_none() && o.logins.is_empty() && o.restarts.is_empty();
    stats.home_holders = home_holders;
    stats.rest_entries = rest_entries;
    stats.rest_list = rest_list.clone();
    stats.missing_reads = missing_reads;
    stats.summon_events = summon_events;
    stats.home_least = home_least;
    stats.reseat_id = reseat_id;
    let left: Vec<String> = rider_left.iter().flatten().cloned().collect();
    stats.riders_left = left.len() as u32;
    if !left.is_empty() {
        out.push(format!("the station riders that left the bench world (a world with no lift: their later reads out of its scope, printed, not judged; G1 on a3b306a05 (8)): {}", left.join("; ")));
    }
    stats.reseat_old = reseat_old;
    stats.reseat_attr = reseat_attr;
    stats.seed = o.seed;
    stats.t_count = t_shapes.len() as u32;
    stats.t_reshaped = t_reshaped.len() as u32;
    out.push(format!(
        "the Ts (plain rails with >= 3 potential connections at T; each shape checked at every tick's end, farmopt 09:46): {}{}",
        if t_shapes.is_empty() { "none".to_string() } else { t_shapes.iter().map(|(p, s)| format!("{p} {}", s.trim_start_matches("minecraft:rail"))).collect::<Vec<_>>().join(", ") },
        if t_reshaped.is_empty() { "; none re-shaped".to_string() } else { format!("; RE-SHAPED: {}", t_reshaped.iter().map(|(_, l)| l.clone()).collect::<Vec<_>>().join("; ")) }
    ));
    stats.t_list = t_reshaped.iter().map(|(_, l)| l.clone()).collect();
    if let Some(r) = &rail_t_stop {
        out.push(format!("the run STOPPED at a held rail's edge (G1 on 834d6329d (T1): a failure in its list): {r}"));
    }
    stats.rail_t_stop = rail_t_stop.is_some();
    stats.disturbed = o.freeze.is_some() || !o.logins.is_empty() || !o.restarts.is_empty();
    stats.t_lever = t_control_lever.map(|p| format!("{p}"));
    // (ironfarm's C-fix rows: the planted cells, the west leaves, the off-rail rests, and where every
    // cart above the off-rail line ended)
    if !planted.is_empty() {
        out.push(format!("planted (ironfarm 11:56): {}", planted.join("; ")));
    }
    // (G1 on e2d9333bc (B4): every plant ran, unless the run stopped at a rail-T edge first)
    if rail_t_stop.is_none() && plants_done != o.plant_carts.len() + o.plant_blocks.len() {
        return Err(format!("--plant-cart / --plant-block: {plants_done} of {} plants ran", o.plant_carts.len() + o.plant_blocks.len()));
    }
    if !o.watch_west.is_empty() {
        out.push(format!(
            "the watched cells (ironfarm's C-fix rows; G1 on 407a29682 (N1): each cell's carts and the face each left by; a cell no cart entered reads NOT JUDGED, a failure): {}; the exits: {}; left westward: {}",
            watch_in.iter().map(|(c, ids)| format!("{c} entered by {} cart(s){}", ids.len(), if ids.is_empty() { ": NOT JUDGED" } else { "" })).collect::<Vec<_>>().join(", "),
            if watch_exits.is_empty() { "none".to_string() } else { watch_exits.join("; ") },
            if west_leaves.is_empty() { "none".to_string() } else { west_leaves.join("; ") }
        ));
    }
    if let Some(y0) = o.offrail_y {
        let ends: Vec<String> = run
            .world
            .entities
            .list
            .iter()
            .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && e.pos.y >= y0)
            .map(|e| format!("cart {} at ({:.2},{:.2},{:.2}) on {}", e.id, e.pos.x, e.pos.y, e.pos.z, run.world.get(BlockPos::new(e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32)).to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or("")))
            .collect();
        out.push(format!(
            "the carts at rest at or above y {y0} (ironspec 10:42's stall rule: no horizontal move of 0.003 and no vertical move for more than 5 ticks, frozen ticks not counted; G1 on 407a29682 (N1): none at rest reads NOT JUDGED, a failure): {}{}; of them off their rails (the game's rail test: the cart's cell or the one below it a rail): {}; the carts there at the end: {}",
            rest_seen.len(),
            if rest_seen.is_empty() { ": NOT JUDGED" } else { "" },
            if offrail.is_empty() { "none".to_string() } else { offrail.iter().map(|(_, l)| l.clone()).collect::<Vec<_>>().join("; ") },
            if ends.is_empty() { "none".to_string() } else { ends.join("; ") }
        ));
    }
    stats.west_leaves = west_leaves.len() as u32;
    stats.offrail = offrail.len() as u32;
    stats.watch_unseen = watch_in.iter().any(|(_, ids)| ids.is_empty());
    stats.offrail_unseen = o.offrail_y.is_some() && rest_seen.is_empty();
    // (G1 12:27) the hold strings' first press after the R line starts, per lane
    if o.refill.is_some() {
        out.push(format!(
            "the hold strings around the R line (the detector-ended R_1 pulse, G1 12:27 / ironspec 13:00: each string's last release before the R line rises, >= 2 gt before it, and its first press after, where the pulse would end): the R line rises at {}; {}",
            r_from.map_or("never".to_string(), |a| format!("T+{a}")),
            (0..3)
                .map(|k| format!(
                    "lane {k}: released T+{} ({} before the rise), pressed again T+{}",
                    hold_release[k].map_or("-".to_string(), |t| t.to_string()),
                    hold_release[k].zip(r_from).map_or("-".to_string(), |(t, a)| format!("{} gt", a - t)),
                    hold_press[k].map_or("-".to_string(), |t| t.to_string())
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
        stats.hold_press = hold_press;
        stats.hold_release_lead = (0..3).filter_map(|k| hold_release[k].zip(r_from).map(|(t, a)| a - t)).min();
        // (G1 on e2d9333bc (B2): the lead is a bar (ironspec 13:00: each hold string's last release
        // >= 2 gt before the R line rises; 16:28: the measured leads adopted): a lead under 2, a
        // string with no release before the rise, or one still pressed at the rise fails; an R line
        // that never rose leaves it NOT JUDGED, a failure)
        stats.hold_lead_bad = match r_from {
            None => Some("NOT JUDGED (the R line never rose): the hold-string lead bar".to_string()),
            Some(a) => {
                let bad: Vec<String> = (0..3)
                    .filter_map(|k| {
                        if hold_was[k] {
                            Some(format!("lane {k} pressed at the rise"))
                        } else {
                            match hold_release[k] {
                                None => Some(format!("lane {k} no release before the rise")),
                                Some(t) if a - t < 2 => Some(format!("lane {k} released {} gt before the rise", a - t)),
                                Some(_) => None,
                            }
                        }
                    })
                    .collect();
                (!bad.is_empty()).then(|| bad.join(", "))
            }
        };
        if let Some(b) = &stats.hold_lead_bad {
            out.push(format!("the hold-string lead bar FAILS: {b}"));
        }
    }
    // (ironspec 13:04, the drive-integrity bar: each pocket drive's first powered tick against its
    // clock slot REL + offset k; the bench's constant DRIVE_SLOT_DELTA, the slack j = --drive-slack)
    if o.layout == DropLayout::Station {
        let deltas: Vec<Option<i64>> = (0..3).map(|k| drive_fire[k].map(|t| t - (o.rel + o.stagger[k]))).collect();
        let off: Vec<(usize, String)> = (0..3)
            .filter_map(|k| match deltas[k] {
                Some(d) if (d - DRIVE_SLOT_DELTA).abs() <= o.drive_slack => None,
                Some(d) => Some((k, format!("pocket {k} fired at its slot {:+}", d - DRIVE_SLOT_DELTA))),
                None => Some((k, format!("pocket {k} never fired"))),
            })
            .collect();
        out.push(format!(
            "the pocket drives (the drive-integrity bar, ironspec 13:04: each first powered tick within {} gt of its clock slot REL + offset k + {DRIVE_SLOT_DELTA}): {}{}",
            o.drive_slack,
            (0..3).map(|k| deltas[k].map_or(format!("pocket {k} -"), |d| format!("pocket {k} slot{:+}", d - DRIVE_SLOT_DELTA))).collect::<Vec<_>>().join(", "),
            if off.is_empty() { String::new() } else { format!("; OFF ITS CLOCK: {}", off.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join("; ")) }
        ));
        // (G1 on 407a29682 (N4): only the --lever-off pocket's own unfired drive is that control's
        // collateral; any other pocket off its clock stays the bar's failure)
        let control_pocket = |k: usize| o.lever_off == Some(k) && deltas[k].is_none();
        stats.drive_off = off.iter().filter(|(k, _)| !control_pocket(*k)).count() as u32;
        stats.drive_off_control = off.iter().filter(|(k, _)| control_pocket(*k)).count() as u32;
    }
    out.push(format!(
        "the least distance from a HOME holder's feet to its HOME head block's centre over the run (the exposure bar's reach, strictly < 16; ironspec 09:03): {}",
        home_least.map_or("no HOME holder".to_string(), |d| format!("{d:.2}"))
    ));
    out.push(format!(
        "the station riders' activities every tick (their transitions; ironspec 08:23): {}; REST entries (with their stamps; a printed count until b8949c96's SetWalkTargetFromBlockMemory hook lands, not judged): {}{}",
        if act_timeline.is_empty() { "none".to_string() } else { act_timeline.join(", ") },
        rest_entries,
        if rest_list.is_empty() { String::new() } else { format!(": {}", rest_list.join("; ")) }
    ));
    if !activity_line.is_empty() {
        out.push(format!("{activity_line}{}", activity_first.as_ref().map_or(String::new(), |f| format!("; WRONG: {f} (a failure, G1 item 9)"))));
    }
    if let Some(c) = &sight_control_cell {
        out.push(format!("the sight check's positive control: glass laid at T in the world, {c}"));
    }
    out.push(format!(
        "the arriving carts at S (T+{s_rel}, bench frame; loop = bench - (6,0,6)): {}",
        (0..3).map(|k| rest_at_s[k].map_or(format!("lane {k} -"), |p| format!("lane {k} ({:.4},{:.4},{:.4})", p.x, p.y, p.z))).collect::<Vec<_>>().join(", ")
    ));
    out.push(format!(
        "the laid design's fluid cells (ironspec 00:27; G1 on a3b306a05 (1): each one's pending tick read back from the scheduler): {fluid_cells}, {fluid_scheduled} with their tick pending{}",
        if fluid_scheduled < fluid_cells { format!(": {} WITHOUT, a failure", fluid_cells - fluid_scheduled) } else { String::new() }
    ));
    out.push("natural spawning: OFF, set explicitly (ironspec 14:57: the bench's scope is the station's own mobs and its scarers; no terrain site, no light-read default)".to_string());
    // (ironspec 15:37: a bench whose verdicts depend on a controller gate emulates it per day-1's
    // canonical gate list (rev 15:37) or names it as a stand-in; none is silently missing)
    if o.layout == DropLayout::Station {
        out.push(format!(
            "controller gates (day-1's canonical list rev 15:37, ironspec 15:37): G8 F1 (REL = the three slot-1 strings pressed AND RE_ARMED): a STAND-IN, the bench's REL given (REL T+{}), every hold full at the release by construction, RE_ARMED not modeled; G9 (REL-keyed timings off the gated REL): every bench timing keys off that given REL; G15 the keyed hold pulse: {}; G5 the hold's at-most-one gate: out of scope (one napper a run); the R line (the refill from the cycle's release): {}; lane x 7's slope tap: {}; G1..G4, G6, G7, G10..G14, G16..G22 (the stop, the sidings, the ring, the returning napper, the shutter): out of the station bench's scope (none in its world)",
            o.rel,
            o.napper.map_or("not in this run (no napper)".to_string(), |n| format!("a STAND-IN, the napper hold's pulse driven at REL + {n} (its held window the declared drop::HELD_NAPPER REL + {}..{})", HELD_NAPPER.0, HELD_NAPPER.1)),
            o.refill.map_or("not in this run (no refill)".to_string(), |(a, b)| format!("a STAND-IN, the R line's input levers on at the last shut + {a} for {b} gt")),
            o.hold_drive.map_or("not in this run (no slope hold)".to_string(), |_| format!("a STAND-IN, the slope's hold released at REL + {} (the declared drop::HELD_SLOPE REL + {}..{})", o.hold_release, HELD_SLOPE.0, HELD_SLOPE.1))
        ));
        // (G1 on e2d9333bc (B6): the standing labels of every station figure, printed with the run)
        out.push("every figure of this run carries: (the controller's REL, refill, hold and slope pulses as stand-ins at their declared phases: G8 / G15 not emulated) (ironspec 15:46); (declared phases: day-1's controller cells not laid) (ironspec 16:06)".to_string());
    }
    stats.block_contacts = block_contacts;
    stats.undeclared_contacts = undeclared_contacts;
    if !stats.block_contacts.is_empty() {
        out.push(format!(
            "the carts' block contacts (ironspec 21:25: a move stopped by a block; declared: the design's stops): {}",
            stats.block_contacts.iter().map(|(c, b, n, d)| format!("{c} {b} {n} ticks{}", if !*d { " UNDECLARED" } else if o.provisional.contains(&(c.x, c.y, c.z)) { " (declared PROVISIONAL: bench evidence only)" } else { " (declared)" })).collect::<Vec<_>>().join("; ")
        ));
    }
    for d in &stats.summon_due {
        out.push(format!(
            "the direct summon check at S = T+{}: {} ({} calls from the stand, {} riders willing and ticking at the tick's start, {} covered by this S's golem); {}",
            d.s,
            d.class,
            d.calls,
            d.eligible,
            d.covered,
            d.riders.join("; ")
        ));
    }
    for c in &stats.summon_calls {
        out.push(format!(
            "  GolemTry at S = T+{}: the caller {}, {} willing, {}",
            c.s,
            c.pocket.map_or("not at a pocket".to_string(), |k| format!("at pocket {k}")),
            c.willing,
            if c.born { "a golem born" } else { "a spot miss" }
        ));
    }
    if o.hold_drive.is_some() {
        out.push(format!(
            "the held slope rail (a declared `held:` stand-in, ironspec 01:07; G1 on 82f9546f): {}; the hold check (powered at the end of every tick from REL+{} to REL+{}, unpowered outside it, a failure): {} breaks{}",
            if stats.held_edges.is_empty() { "no edge".to_string() } else { stats.held_edges.join("; ") },
            o.hold_release + 1,
            o.hold_release + o.hold_width,
            stats.hold_breaks,
            stats.hold_break_first.as_ref().map_or(String::new(), |f| format!(" (the first {f})"))
        ));
    }
    if stats.sight_scope || o.sight_ref.is_some() {
        out.push(format!(
            "the sight check (ironspec 20:10/23:30/00:27, G1's audit (1): every villager -> scarer and villager -> villager segment, eye to eye, walked through the live world's collision shapes against the reference's, every tick T..S at the tick's end; a crossing fails): {} rays judged, {} crossings{}; {}",
            stats.sight_rays,
            stats.sight_hit_count,
            if stats.sight_hits.is_empty() { String::new() } else { format!(": {}", stats.sight_hits.join("; ")) },
            if stats.sight_control {
                "this row is the positive control (glass laid in the world at T on a villager -> scarer ray, else a villager -> villager ray): it must fail".to_string()
            } else if !stats.sight_scope {
                "out of scope (computed: the laid world equals the reference at T, no stand-in cell differs)".to_string()
            } else if stats.sight_rays == 0 {
                "NOT JUDGED (0 rays): a failure".to_string()
            } else {
                "judged".to_string()
            }
        ));
    }
    let wrong: Vec<String> = (0..3)
        .filter_map(|k| {
            let (x, _, z) = pockets[k];
            match pos(&run.world, carts[3 + k]) {
                Some((p, _)) if (p.x - (x as f64 + 0.5)).abs() < 0.3 && (p.z - (z as f64 + 0.5)).abs() < 0.3 && rest_since[k].is_some() => None,
                Some((p, _)) => Some(format!("arriving cart {k} at {:.2},{:.2},{:.2}", p.x, p.y, p.z)),
                None => Some(format!("arriving cart {k} gone")),
            }
        })
        .collect();
    stats.spread = (0..3)
        .filter_map(|k| from[k].map(|p| (((p.x - (pockets[k].0 as f64 + 0.5)).abs().max((p.z - (pockets[k].2 as f64 + 0.5)).abs())) * 100.0).round() as i64))
        .max()
        .unwrap_or(0);
    stats.fall = (0..3).map(|k| landed[k].zip(opened[k]).map_or(i64::MAX, |(l, d)| l.0 - d)).max().unwrap_or(i64::MAX);
    stats.brake_lead = (0..3).filter_map(|k| landed[k].zip(rebrake_at[k]).map(|(l, r)| l.0 - r)).min();
    let last_rest = (0..3).map(|k| rest_since[k].unwrap_or(i64::MAX)).max().unwrap_or(i64::MAX);
    stats.rest = if last_rest == i64::MAX { i64::MIN } else { boundary - last_rest };
    stats.wrong = wrong.len();
    stats.contacts = contacts.len();
    stats.contact_max = contact_max;
    stats.closest = closest.map(|c| (c.0 * 100.0).floor() as i64);
    stats.hurt = hurt;
    stats.unseated = unseated;
    stats.stuck = (0..3).filter(|k| exits[*k].is_none() && !o.dep_absent[*k]).count();
    stats.waited = *waited.iter().max().unwrap_or(&0);
    stats.feet = (0..3).filter_map(|k| feet[k]).map(|y| (y * 100.0).floor() as i64).min().unwrap_or(0);
    out.push(format!(
        "drop ({:?}, hold y {}; REL T+{} +{:?}, re-brake REL+{}{}; trapdoors at T+{}{}{}): released {}; riders' feet {}; falls {}; at rest {} (the next summon T+{boundary}); the closest arriving-to-departing {}",
        o.layout,
        o.hold_y,
        o.rel,
        o.stagger,
        o.rebrake,
        rebrake_dt.map_or(String::new(), |t| format!(" = T+{t}")),
        o.release,
        if o.occupancy { " gated on the pocket strings" } else { "" },
        if o.pulse > 0 { format!(", shut after {} gt", o.pulse) } else { String::new() },
        (0..3).map(|k| match (opened[k], from[k]) {
            (Some(t), Some(p)) => format!("cart {k} T+{t}{}{} at {:.3},{:.3},{:.3}", if waited[k] > 0 { format!(" (waited {} gt on its gate)", waited[k]) } else { String::new() }, if empty_at_release[k] { " (its pocket empty at the release)" } else { "" }, p.x, p.y, p.z),
            _ => format!("cart {k} NEVER (waited {} gt)", waited[k]),
        }).collect::<Vec<_>>().join(", "),
        (0..3).map(|k| feet[k].map_or("-".to_string(), |y| format!("{y:.3}"))).collect::<Vec<_>>().join(", "),
        (0..3).map(|k| match (landed[k], opened[k]) {
            (Some((t, v, pw)), Some(d)) => format!("cart {k} {} gt to T+{t} at {v:.3} b/gt{}", t - d, if pw { " ONTO A POWERED POCKET" } else { "" }),
            _ => format!("cart {k} never landed"),
        }).collect::<Vec<_>>().join(", "),
        (0..3).map(|k| rest_since[k].map_or(format!("cart {k} NO"), |t| format!("cart {k} T+{t}"))).collect::<Vec<_>>().join(", "),
        closest.map_or("-".to_string(), |(d, t, a, m)| format!("{d:.2} (arriving cart {a} to departing cart {m} at T+{t})"))
    ));
    out.push(format!(
        "pockets: {}; departures {}; contacts {} (the fastest {:.3}){}; riders hurt {hurt}, unseated ticks {unseated}",
        if wrong.is_empty() { "every arriving cart at rest mid-cell in its own pocket".to_string() } else { format!("WRONG POCKET: {}", wrong.join("; ")) },
        (0..3).map(|k| if o.dep_absent[k] { format!("cart {k} absent (cold start)") } else { exits[k].map_or(format!("cart {k} STUCK"), |(t, v)| format!("cart {k} out T+{t} at {v:.3}")) }).collect::<Vec<_>>().join(", "),
        contacts.len(),
        contact_max,
        if contacts.is_empty() { String::new() } else { format!(" ({})", contacts.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    if !next_carts.is_empty() {
        out.push(format!(
            "refill (the R line{}): {}; the closest a next cart came to another {}",
            match (r_from, r_to) {
                (Some(a), Some(b)) => format!(" T+{a}..T+{b}"),
                (Some(a), None) => format!(" from T+{a}"),
                _ => " NEVER (the trapdoors never all shut)".to_string(),
            },
            (0..next_carts.len().min(3))
                .map(|j| if next_fell[j] {
                    format!("next cart {j} FELL THROUGH")
                } else {
                    next_rest[j].map_or(format!("next cart {j} NOT IN its hold"), |(t, p)| format!("next cart {j} at rest in its hold T+{t} at {:.3},{:.3},{:.3}", p.x, p.y, p.z))
                })
                .collect::<Vec<_>>()
                .join(", "),
            next_closest.map_or("-".to_string(), |d| format!("{d:.2}"))
        ));
        stats.refill_fail = (0..next_carts.len().min(3)).filter(|j| next_fell[*j] || next_rest[*j].is_none()).count();
        // (ironspec 09:32: each lane's R_1 against its drive: a launch only after the rail's power
        // ended is bad (ironfarm's bar for R), as is none)
        let mut late = 0usize;
        let mut cell_late = 0usize;
        let mut lag: Option<i64> = None;
        let mut len: Option<i64> = None;
        let per: Vec<String> = (0..next_carts.len().min(3))
            .map(|j| {
                let (on, off, mv, oc) = (r1_on[j], r1_off[j], next_moved[j], next_off_cell[j]);
                if let (Some(a), Some(b)) = (on, off) {
                    len = Some(len.map_or(b - a, |x: i64| x.min(b - a)));
                }
                if let (Some(a), Some(m)) = (on, mv) {
                    lag = Some(lag.map_or(m - a, |x: i64| x.max(m - a)));
                }
                let bad = match (mv, off) {
                    (None, _) => true,
                    (Some(m), Some(b)) => m >= b,
                    (Some(_), None) => false,
                };
                let bad_cell = match (oc, off) {
                    (None, _) => true,
                    (Some(c), Some(b)) => c >= b,
                    (Some(_), None) => false,
                };
                late += bad as usize;
                cell_late += bad_cell as usize;
                format!(
                    "lane {j}: the rail powered {}, its cart moves {}{}, off R_1's cell {}{}",
                    match (on, off) {
                        (Some(a), Some(b)) => format!("T+{a}..T+{b} ({} gt)", b - a),
                        (Some(a), None) => format!("from T+{a}"),
                        _ => "NEVER".to_string(),
                    },
                    mv.map_or("NEVER".to_string(), |m| format!("at T+{m}")),
                    if bad { " (LATE: after its power ended, or never)" } else { "" },
                    oc.map_or("NEVER".to_string(), |c| format!("at T+{c}{}", off.map_or(String::new(), |b| format!(" ({} gt before its power ended)", b - c)))),
                    if bad_cell { " (LATE by the cell: after its power ended, or never)" } else { "" }
                )
            })
            .collect();
        out.push(format!(
            "R_1's launch ({}): {}",
            if o.r_repeater { "the built drive: a repeater facing west into R_1, fed by the controller's block at (x+2,17,1)" } else { "the emulated drive: a redstone block in R_1's side cell" },
            per.join("; ")
        ));
        stats.r1_late = late;
        stats.r1_cell_late = cell_late;
        stats.r1_lag = lag;
        stats.r1_len = len;
    }
    stats.gap_at_open = (0..3).filter_map(|k| gap_at_open[k]).map(|g| if g.is_finite() { (g * 100.0).floor() as i64 } else { i64::MAX }).min();
    // (the launch spacing: consecutive launches in launch order, world ticks)
    let mut ls: Vec<i64> = launch_at.iter().filter_map(|t| *t).collect();
    ls.sort();
    stats.launch_gap = ls.windows(2).map(|w| w[1] - w[0]).min();
    stats.shut_onto = shut_onto.len();
    stats.r_frozen = r_switch_frozen.len();
    out.push(format!(
        "trapdoors shut onto a cart: {}{}; R line switches while frozen: {}{}{}{}",
        shut_onto.len(),
        if shut_onto.is_empty() { String::new() } else { format!(" ({})", shut_onto.join(", ")) },
        r_switch_frozen.len(),
        if r_switch_frozen.is_empty() { String::new() } else { format!(" ({})", r_switch_frozen.join(", ")) },
        if reloaded.is_empty() { String::new() } else { format!("; restarts {}", reloaded.join(", ")) },
        if run.world.errors.is_empty() { String::new() } else { format!("; the world's model errors: {}", run.world.errors.join("; ")) }
    ));
    out.push(format!(
        "launches {} (spacing in launch order {})",
        (0..3).map(|n| launch_at[n].map_or(format!("pocket {n} NEVER"), |t| format!("pocket {n} T+{t}"))).collect::<Vec<_>>().join(", "),
        ls.windows(2).map(|w| (w[1] - w[0]).to_string()).collect::<Vec<_>>().join(", ")
    ));
    if !scarers_all.is_empty() {
        out.push(format!(
            "night panic ({} scarers): the longest stretch without a scarer in range and sight: the hold {} gt (per lane x -1/3/7: {}), the fall {} gt{}, the pocket {} gt, the departure {} gt; the nearest zombie to a rider it covers {}, to any villager in sight or not {}; the nearest pillager to a villager it sees {}",
            scarers_all.len(),
            calm_max[0],
            hold_lane.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("/"),
            calm_max[1],
            if calm_y.iter().any(|c| c.is_some()) { format!(" (uncovered feet {})", (0..3).map(|k| calm_y[k].map_or(format!("lane {k} -"), |(a, b)| format!("lane {k} y {a:.1}..{b:.1}"))).collect::<Vec<_>>().join(", ")) } else { String::new() },
            calm_max[2],
            calm_max[3],
            zombie_near.map_or("-".to_string(), |d| format!("{d:.2}")),
            zombie_any.map_or("-".to_string(), |d| format!("{d:.4}{}{}", if d < 6.05 { " (INSIDE 6.05)" } else { "" }, zombie_any_at.map_or(String::new(), |a| closest_text(a)))),
            pillager_near.map_or("-".to_string(), |d| format!("{d:.2}{}", if d < 8.5 { " (INSIDE 8.5)" } else { "" }))
        ));
        stats.calm = Some(calm_max);
        stats.pillager_near = pillager_near.map(|d| (d * 100.0).floor() as i64);
        stats.zombie_near = zombie_near.map(|d| (d * 100.0).floor() as i64);
        stats.zombie_any = zombie_any.map(|d| (d * 100.0).floor() as i64);
        stats.zombie_any_at = zombie_any.zip(zombie_any_at).map(|(d, a)| (d, closest_text(a)));
        stats.hold_lane = hold_lane;
    }
    stats.r_from = r_from;
    stats.next_in = if next_carts.is_empty() { None } else { (0..next_carts.len().min(3)).map(|j| next_rest[j].map(|r| r.0)).collect::<Option<Vec<_>>>().and_then(|v| v.into_iter().max()) };
    stats.shut_last = (0..3).filter_map(|k| opened[k].map(|t| t + o.pulse)).max();
    stats.fall_off = fall_off;
    stats.fall_toward = if fall_toward.is_finite() { Some(fall_toward) } else { None };
    stats.summon_off = summon_off.len();
    // (golem-stamp path (a), the module: each departing rider against the next batch's callers)
    if o.track_lift {
        stats.lift_last = lift_last;
        stats.lift_y80 = lift_y80;
        stats.lift_in100 = lift_in100;
        stats.lift_entry = lift_entry;
        stats.lift_y12 = lift_y12;
        stats.lift_top = lift_top;
        stats.hold_on = hold_on;
        stats.hold_rest = hold_rest;
        stats.hold_drift = hold_drift;
        stats.hold_overshoot = hold_overshoot;
        stats.hold_gap = match (lift_entry[2], lift_y12[1]) {
            (Some(a), Some(b)) if o.hold_rail.is_some() => Some(a - b),
            _ => None,
        };
        stats.hold_wet = hold_wet;
        stats.lift_clear = lift_clear;
        stats.lift_push = lift_push;
        stats.lift_push_at = lift_push_at;
        stats.nap_off = nap_off;
        stats.nap_entry = nap_entry;
        stats.nap_y12 = nap_y12;
        stats.nap_top = nap_top;
        stats.lift_air = lift_air;
        stats.lift_dip = lift_dip;
        stats.watch_hits = watch_hits;
        stats.watch_first = watch_first;
        if !o.watch.is_empty() {
            out.push(format!(
                "the watched cells ({} of them): a cart's or villager's box met one on {} ticks{}",
                o.watch.len(),
                watch_hits,
                watch_first.map_or(String::new(), |(t, (x, y, z))| format!(", the first at T+{t}, ({},{},{}) loop", x - 6, y, z - 6))
            ));
        }
        if let Some(k) = o.napper {
            out.push(format!(
                "the napper (released REL+{k}): off its hold T+{}, into the lift T+{}, its pass at y 12 T+{}, the lift's top T+{}; lane 7's entry into the column T+{}: the napper's pass at y 12 {} gt after it (the column bar 16, ironspec 20:14, judged)",
                nap_off.map_or("-".to_string(), |t| t.to_string()),
                nap_entry.map_or("-".to_string(), |t| t.to_string()),
                nap_y12.map_or("-".to_string(), |t| t.to_string()),
                nap_top.map_or("-".to_string(), |t| t.to_string()),
                lift_entry[2].map_or("-".to_string(), |t| t.to_string()),
                match (nap_y12, lift_entry[2]) {
                    (Some(a), Some(b)) => (a - b).to_string(),
                    _ => "-".to_string(),
                }
            ));
        }
        out.push(format!(
            "the lift's top exits: {}{}; the riders' least air from their exits on (of 300): {}; the departing carts past their exits: the least clearance between two overlapping in y {}, push contacts {} (a box inflated 0.2 meeting another's){}",
            (0..3)
                .filter(|n| !o.dep_absent[*n])
                .map(|n| lift_top[n].map_or(format!("lane {n} -"), |(t, y, vy)| format!("lane {n} T+{t} at y {y:.3}, rising {vy:.3}")))
                .collect::<Vec<_>>()
                .join(", "),
            if o.hold_rail.is_some() {
                format!(
                    "; the hold (2c, the braked slope): released T+{}, lane 2's cart on the hold's rail from T+{}, at rest {}, its drift while held {:.4}, in the column before the release: {}, its entry {} gt after lane 1's pass at y 12 (the column bar 16, ironspec 20:14, judged), the hold rail not a dry powered rail on {} ticks{}",
                    hold_on.map_or("-".to_string(), |t| t.to_string()),
                    hold_first.map_or("-".to_string(), |t| t.to_string()),
                    hold_rest.map_or("never".to_string(), |(t, x, f)| format!("from T+{t} at x {:.3} (loop), its rider's feet {f:.3}", x - 6.0)),
                    hold_drift,
                    if hold_overshoot { "YES (overshoot)" } else { "no" },
                    stats.hold_gap.map_or("-".to_string(), |g| g.to_string()),
                    hold_wet,
                    if o.trace {
                        format!(
                            "; its cart from T+72 every 2 gt, loop frame: {}",
                            hold_track.iter().filter(|(t, _, _)| *t >= 72).map(|(t, p, v)| format!("T+{t} {:.3},{:.3} v {:.3},{:.3}", p.x - 6.0, p.y, v.x, v.y)).collect::<Vec<_>>().join(" ")
                        )
                    } else {
                        String::new()
                    }
                )
            } else {
                String::new()
            },
            (0..4).map(|n| lift_air[n].map_or(format!("{} -", if n == 3 { "the napper".to_string() } else { format!("lane {n}") }), |a| format!("{} {a}", if n == 3 { "the napper".to_string() } else { format!("lane {n}") }))).collect::<Vec<_>>().join(", "),
            lift_clear.map_or("-".to_string(), |c| format!("{c:.3}")),
            lift_push,
            lift_push_at.map_or(String::new(), |(t, a, b)| format!(", the first at T+{t}, lanes {a} and {b}"))
        ));
        out.push(format!(
            "the departing riders through the merge row and the shared lift (the bar: each rider's box out of every live caller's 10-box (the arriving riders' boxes inflated by 10, each tick) after its move on S - 3, S on the summon's own clock): {}",
            (0..3)
                .filter(|n| !o.dep_absent[*n])
                .map(|n| format!(
                    "lane {n}: off its rest T+{}, the station's exit T+{}, the merge row T+{}, the lift T+{}, its feet at y 12.0125 T+{}; inside a caller's box last at T+{} (the slack over S - 3 = T+97, physical: {}); its feet at T+80 {}, at T+100 {} ({}){}",
                    launch_at[n].map_or("-".to_string(), |t| t.to_string()),
                    exits[n].map_or("-".to_string(), |e| e.0.to_string()),
                    lift_merge[n].map_or("-".to_string(), |t| t.to_string()),
                    lift_entry[n].map_or("-".to_string(), |t| t.to_string()),
                    lift_y12[n].map_or("-".to_string(), |t| t.to_string()),
                    lift_last[n].map_or("-".to_string(), |t| t.to_string()),
                    lift_last[n].map_or("-".to_string(), |t| (96 - t).to_string()),
                    lift_y80[n].map_or("-".to_string(), |y| format!("{y:.3}")),
                    lift_y100[n].map_or("-".to_string(), |y| format!("{y:.3}")),
                    if lift_in100[n] { "INSIDE at the summon: stamped" } else { "out" },
                    if o.trace {
                        format!(
                            "; its path {}",
                            lift_track[n].iter().filter(|(t, _)| t % 4 == 0).map(|(t, p)| format!("T+{t} {:.2},{:.2},{:.2}", p.x - 6.0, p.y, p.z - 6.0)).collect::<Vec<_>>().join(" ")
                        )
                    } else {
                        String::new()
                    }
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    // (ironspec 11:05's audit, G1 on 44d4bbfae: the placement's drops read structurally (the
    // design's cells outside the world's box), redsim's records beside them; the run's writes
    // outside the box from its records)
    let rec_all = oob_records(&run.world);
    stats.oob = place_drop + rec_all.saturating_sub(rec_place);
    out.push(format!(
        "cells outside the world's box (dropped): the design's at the placement {place_drop} (redsim's records of them {rec_place}), the run's writes {}{}{}",
        rec_all.saturating_sub(rec_place),
        if o.box_short > 0 { format!(" (THE CONTROL: the world built {} short of the design's top)", o.box_short) } else { String::new() },
        if stats.oob > 0 { format!(": {}", run.world.errors.iter().filter(|e| e.starts_with("setBlock outside the box")).take(4).cloned().collect::<Vec<_>>().join(" | ")) } else { String::new() }
    ));
    // (the panic at the next summon T = T+100, two readings. (1) ironspec's conservative windows:
    // per summoner the scarer whose runs reach furthest back: in range from t_A on (needed from
    // T-19), seen eye to eye in the 16-box from t_B on (needed from T-38). (2) the game's trigger
    // (G1 / ironspec 05:04): per summoner and per sensor phase pair (the living scan every 20 gt at
    // phase pL, the hostiles scan every 20 gt at phase pH), NEAREST_HOSTILE at each hostiles scan t
    // = a scarer in the 16-box at the last living scan u <= t, seen (within 16, eye to eye) at every
    // tick u..t (the cached sight), in its type's range at t; VillagerPanicTrigger (duration 60)
    // runs from the first scan h of a continuous presence, h..h+60, stops at h+61, restarts at h+62:
    // the summoner calls spawnGolemIfNeeded at T if the last scan <= T found a scarer and T is not
    // h + 61 + 62k. The game's bar: >= 1 caller in every phase combination; the brief's: all three.)
    if !o.panic_posts.is_empty() && panic_log.len() > 100 {
        let tn = 100usize;
        let ns = o.panic_posts.len();
        let mut lines = Vec::new();
        let mut robust = 0usize;
        for k in 0..3 {
            let a = |t: usize, s: usize| panic_log[t][k].get(s).is_some_and(|x| x.0);
            let b = |t: usize, s: usize| panic_log[t][k].get(s).is_some_and(|x| x.1 && x.2);
            let mut best: Option<(usize, i64, i64, Option<i64>, Option<i64>)> = None;
            for s in 0..ns {
                let run_from = |f: &dyn Fn(usize) -> bool| -> i64 {
                    let mut t = tn as i64;
                    while t >= 0 && f(t as usize) {
                        t -= 1;
                    }
                    t + 1
                };
                let (ta, tb) = (run_from(&|t| a(t, s)), run_from(&|t| b(t, s)));
                let (ma, mb) = ((tn as i64 - 19) - ta, (tn as i64 - 38) - tb);
                let fa = (tn - 19..=tn).find(|t| !a(*t, s)).map(|t| t as i64);
                let fb = (tn - 38..=tn).find(|t| !b(*t, s)).map(|t| t as i64);
                if best.is_none_or(|x| ma.min(mb) > x.1.min(x.2)) {
                    best = Some((s, ma, mb, fa, fb));
                }
            }
            stats.panic[k] = best;
            // the game's trigger over every phase pair
            let mem = |t: usize, pl: usize| -> bool {
                let u = t - (t + 20 - pl) % 20;
                (0..ns).any(|s| panic_log[u][k].get(s).is_some_and(|x| x.1) && (u..=t).all(|q| panic_log[q][k].get(s).is_some_and(|x| x.2)) && panic_log[t][k].get(s).is_some_and(|x| x.0))
            };
            let mut calls = 0usize;
            let mut unknown = 0usize;
            let mut h_latest: Option<i64> = None;
            let mut durations: Vec<i64> = Vec::new();
            let mut windows: Vec<i64> = Vec::new();
            let mut h_least: Option<i64> = None;
            let mut fail_eg: Option<String> = None;
            for pl in 0..20 {
                for ph in 0..20 {
                    let tstar = tn - (tn + 20 - ph) % 20;
                    let ok = if !mem(tstar, pl) {
                        false
                    } else {
                        let mut h = tstar;
                        while h >= 40 && mem(h - 20, pl) {
                            h -= 20;
                        }
                        // (the turn's panic: from h to the first hostiles scan without a scarer)
                        let mut e = tstar;
                        while e + 20 < panic_log.len() && mem(e + 20, pl) {
                            e += 20;
                        }
                        let dur = (e + 20 - h) as i64;
                        durations.push(dur);
                        // (its window in the pocket: from h to leaving its rest, the next cycle's
                        // launch, launch_at + 100 in L1)
                        if let Some(l) = launch_at[k] {
                            windows.push(l + 100 - h as i64);
                        }
                        if h < 40 {
                            // (present since before the bench's window: the trigger's phase is
                            // unknown, T can be its dead tick h + 61 + 62k)
                            unknown += 1;
                            false
                        } else {
                            let dead = tn >= h + 61 && (tn - h - 61) % 62 == 0;
                            if !dead && tn < h + 61 {
                                h_least = Some(h_least.map_or(h as i64, |x: i64| x.min(h as i64)));
                            }
                            if !dead {
                                h_latest = Some(h_latest.map_or(h as i64, |x: i64| x.max(h as i64)));
                            }
                            !dead
                        }
                    };
                    if ok {
                        calls += 1;
                    } else if fail_eg.is_none() {
                        fail_eg = Some(format!(
                            "living phase {pl}, hostiles phase {ph}: {}",
                            if !mem(tstar, pl) { "no scarer at the last hostiles scan" } else if unknown > 0 { "a scarer seen since before T+40: the trigger's phase unknown, T can be its dead tick" } else { "T falls on the trigger's restart gap" }
                        ));
                    }
                }
            }
            if calls == 400 {
                robust += 1;
            }
            // the arriving batch's riders in this summoner's 10-box at T, by position (G1: their
            // wanting state is not read: in the station every one slept last night and none is
            // golem-locked at T, so all of them want)
            let wanting = panic_pos.get(tn).map_or(0, |row| {
                let Some(c) = row[k] else { return 0 };
                row.iter().flatten().filter(|q| (q.x - c.x).abs() < 10.6 && (q.z - c.z).abs() < 10.6 && q.y < c.y + 1.95 + 10.0 && q.y + 1.95 > c.y - 10.0).count()
            });
            stats.panic_calls[k] = Some((calls, wanting));
            if let Some((s, ma, mb, fa, fb)) = best {
                lines.push(format!(
                    "lane {k}: calls in {calls} of 400 sensor phase pairs{}{}{}, {wanting} riders in its 10-box (by position; the state not read: all slept, none locked at T in the station); its panic window in the pocket (its first hostiles scan with a scarer to leaving its rest at T+{}) {}; its panic this turn (to the first scan without a scarer, the bench's track end included) {}; windows (scarer {s}): in range {} (margin {ma}), seen in the 16-box {} (margin {mb})",
                    if unknown > 0 { format!(" ({unknown} with a scarer seen since before T+40: the trigger's phase unknown)") } else { String::new() },
                    h_least.map_or(String::new(), |h| format!(", its trigger started at T+{h} at the earliest (T is its dead tick only from a start at T+39: margin {}){}", h - 39, h_latest.map_or(String::new(), |l| format!(" and by T+{l} at the latest over both sensors' phases")))),
                    fail_eg.map_or(String::new(), |e| format!(" (fails e.g. {e})")),
                    launch_at[k].map_or("?".to_string(), |l| (l + 100).to_string()),
                    match (windows.iter().min(), windows.iter().max()) {
                        (Some(a), Some(b)) => format!("{a}..{b} gt over the phase pairs (the shortest {a})"),
                        _ => "none".to_string(),
                    },
                    match (durations.iter().min(), durations.iter().max()) {
                        (Some(a), Some(b)) => format!("{a}..{b} gt"),
                        _ => "none".to_string(),
                    },
                    fa.map_or("through T+81..100".to_string(), |t| format!("FAILS at T+{t}")),
                    fb.map_or("through T+62..100".to_string(), |t| format!("FAILS at T+{t}"))
                ));
            }
        }
        stats.panic_callers = Some(robust);
        out.push(format!(
            "the panic at the next summon (T+100; each rider's own two sensor phases, 20 x 20 pairs each; follow range {:.2}): {}; the least caller count {robust} (the riders calling at all 400 of their own pairs; the game's bar >= 1, margin 2); all three call in every phase pair: {} (the retired all-three rule)",
            o.follow_range,
            lines.join("; "),
            if robust == 3 { "yes" } else { "NO" }
        ));
    }
    if !summon_off.is_empty() {
        out.push(format!("villagers in a shaft or on a departure rail at a summon tick: {}", summon_off.join("; ")));
    }
    // (option (b), farmopt 05:55: lane k's zombie post cells in the zombie band, ranked by the
    // shortest panic window in the pocket they give)
    if let Some(k) = o.panic_search {
        let tn = 100usize;
        let dep = launch_at[k].map(|l| l + 100);
        let landing = landed[k].map(|(t, _, _)| t);
        let considered = cands.len();
        let mut rows: Vec<(i64, String)> = Vec::new();
        let mut clear = 0usize;
        for c in &cands {
            if c.nearest < 6.05 || c.log.len() <= tn {
                continue;
            }
            clear += 1;
            let (calls, hmin, hmax, wmin, wmax) = trigger_summary(&c.log, tn, dep);
            if calls < 400 {
                continue;
            }
            let mut first = tn as i64;
            while first > 0 && c.log[first as usize].0 && c.log[first as usize].2 {
                first -= 1;
            }
            let first = first + 1;
            let w0 = wmin.unwrap_or(0);
            // (the nearest pad spawn cell's golem, feet at loop y -6 on the floor rows z -6, -4, -3,
            // -1, x -3..8: a zombie within melee reach of one can hit it after a reload)
            let pad = [0i32, 2, 3, 5]
                .iter()
                .flat_map(|z| (3..=14).map(move |x| (x, *z)))
                .map(|(x, z)| ((x as f64 + 0.5 - c.seat.x).powi(2) + (-6.0 - c.seat.y).powi(2) + (z as f64 + 0.5 - c.seat.z).powi(2)).sqrt())
                .fold(f64::INFINITY, f64::min);
            rows.push((
                w0,
                format!(
                    "({},{},{}) open {}: its seat {:.2} from the rider at rest, the nearest any rider comes {:.2}, the nearest pad golem {pad:.2}; lane {k}'s rider in view and in range from T+{first}{}; its panic starts T+{}..{} (the dead-tick margin {}: a start at T+39 puts T on its dead tick); its window in the pocket {}..{} gt",
                    c.cell[0] - 6,
                    c.cell[1],
                    c.cell[2] - 6,
                    c.open,
                    c.rest_d,
                    c.nearest,
                    landing.map_or(String::new(), |l| if first < l { format!(" ({} gt before landing at T+{l})", l - first) } else { format!(" (after landing at T+{l})") }),
                    hmin.unwrap_or(-1),
                    hmax.unwrap_or(-1),
                    hmin.map_or("-".to_string(), |h| (h - 39).to_string()),
                    w0,
                    wmax.unwrap_or(0)
                ),
            ));
        }
        rows.sort_by(|a, b| b.0.cmp(&a.0));
        out.push(format!(
            "option (b), lane {k} (loop frame cells; a zombie in a boat in a glass box open on one side, its sight through the open face; the band: its seat 6.05..7.95 from the rider at rest, no rider of this station bench ever nearer than 6.05 (the module's other riders, the lanes, the S-bend, the merge row and the queue, are 9fc3155f's check); every one calling at all 400 phase pairs): {considered} candidates in the band with room, {clear} with no station rider ever nearer than 6.05, {} calling at all 400; the best by the shortest window: {}",
            rows.len(),
            if rows.is_empty() { "none".to_string() } else { rows.iter().take(12).map(|(_, s)| s.clone()).collect::<Vec<_>>().join(" | ") }
        ));
    }
    stats.last_out = (0..3).filter(|n| !o.dep_absent[*n]).map(|n| exits[n].map(|(t, _)| t)).collect::<Option<Vec<_>>>().and_then(|v| v.into_iter().max());
    // (ironspec 04:10: the measured departure variation per lane, from its own launch tick REL +
    // offset: the tick its cart leaves the pocket's rest, the tick it leaves the station (loop z
    // 6.9, the merge row's entry) and its speed there)
    for n in 0..3 {
        if o.dep_absent[n] {
            continue;
        }
        let base = o.rel + o.stagger[n];
        if let (Some(l), Some((t, v))) = (launch_at[n], exits[n]) {
            stats.lane_times[n] = Some((l - base, t - base, v));
        }
    }
    Ok((out, stats))
}

/// The grid's summary over many drop runs.
pub fn summary(label: &str, runs: &[DropStats]) -> String {
    let show = |v: i64| if v == i64::MIN { "FAIL".to_string() } else { v.to_string() };
    format!(
        "{label}: {} runs; the widest release spread {:.2}; the longest fall {}; the least landing lead over its pocket's re-brake {}; arriving at rest: least {} before the next summon; wrong pockets {}, contacts {} (the fastest {:.3}), the closest arriving-to-departing {}; riders hurt {}, unseated ticks {}; departing carts stuck {}; the longest wait on a string {} gt; the lowest waiting rider's feet {:.2}{}{}{}{}{}{}{}",
        runs.len(),
        runs.iter().map(|s| s.spread).max().unwrap_or(0) as f64 / 100.0,
        runs.iter().map(|s| s.fall).max().map_or("-".to_string(), |f| if f == i64::MAX { "NEVER LANDED".to_string() } else { format!("{f} gt") }),
        runs.iter().filter_map(|s| s.brake_lead).min().map_or("-".to_string(), |v| format!("{v} gt")),
        show(runs.iter().map(|s| s.rest).min().unwrap_or(i64::MIN)),
        runs.iter().map(|s| s.wrong).sum::<usize>(),
        runs.iter().map(|s| s.contacts).sum::<usize>(),
        runs.iter().map(|s| s.contact_max).fold(0.0, f64::max),
        runs.iter().filter_map(|s| s.closest).min().map_or("-".to_string(), |c| format!("{:.2}", c as f64 / 100.0)),
        runs.iter().map(|s| s.hurt).sum::<usize>(),
        runs.iter().map(|s| s.unseated).sum::<usize>(),
        runs.iter().map(|s| s.stuck).sum::<usize>(),
        runs.iter().map(|s| s.waited).max().unwrap_or(0),
        runs.iter().map(|s| s.feet).min().unwrap_or(0) as f64 / 100.0,
        if runs.iter().any(|s| s.calm.is_some()) {
            let mx = |i: usize| runs.iter().filter_map(|s| s.calm.map(|c| c[i])).max().unwrap_or(0);
            format!(
                "; night panic: the longest uncovered stretch the hold {} gt, the fall {} gt, the pocket {} gt, the departure {} gt; the nearest zombie {}, the nearest pillager to a seen villager {}",
                mx(0),
                mx(1),
                mx(2),
                mx(3),
                runs.iter().filter_map(|s| s.zombie_near).min().map_or("-".to_string(), |c| format!("{:.2}", c as f64 / 100.0)),
                runs.iter().filter_map(|s| s.pillager_near).min().map_or("-".to_string(), |c| format!("{:.2}", c as f64 / 100.0))
            )
        } else {
            String::new()
        },
        if runs.iter().any(|s| s.refill_fail > 0) { format!("; REFILL FAILED in {} runs", runs.iter().filter(|s| s.refill_fail > 0).count()) } else { String::new() } + &match (runs.iter().filter_map(|s| s.r1_len).min(), runs.iter().filter_map(|s| s.r1_lag).max()) {
            (Some(l), Some(g)) => format!("; R_1: the shortest rail power {l} gt, the longest lag to its cart's first move {g} gt, late launches (after the power, or none) in {} runs, by the cell (off R_1's cell after the power, or never) in {} runs", runs.iter().filter(|s| s.r1_late > 0).count(), runs.iter().filter(|s| s.r1_cell_late > 0).count()),
            _ => String::new(),
        } + &format!("; cells set outside the world's box (dropped): {} in {} runs", runs.iter().map(|s| s.oob).sum::<usize>(), runs.iter().filter(|s| s.oob > 0).count())
            + &if runs.iter().any(|s| s.lift_last.iter().any(|l| l.is_some())) {
                format!(
                    "; the departing riders against the next batch's callers (the bar: out of every caller box after its move on S - 3 = T+97, physical: the panic path summons at game time % 100 == 0 and stamps every villager in a caller's 10-box): {}",
                    (0..3)
                        .map(|n| format!(
                            "lane {n} inside last at T+{} at the latest (the slack over S - 3 {}), inside at the summon T+100 in {} runs, the lift's entry T+{}..{}, its feet at y 12.0125 T+{}..{}",
                            runs.iter().filter_map(|s| s.lift_last[n]).max().map_or("-".to_string(), |t| t.to_string()),
                            runs.iter().filter_map(|s| s.lift_last[n]).max().map_or("-".to_string(), |t| (96 - t).to_string()),
                            runs.iter().filter(|s| s.lift_in100[n]).count(),
                            runs.iter().filter_map(|s| s.lift_entry[n]).min().map_or("-".to_string(), |t| t.to_string()),
                            runs.iter().filter_map(|s| s.lift_entry[n]).max().map_or("-".to_string(), |t| t.to_string()),
                            runs.iter().filter_map(|s| s.lift_y12[n]).min().map_or("-".to_string(), |t| t.to_string()),
                            runs.iter().filter_map(|s| s.lift_y12[n]).max().map_or("-".to_string(), |t| t.to_string())
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                String::new()
            },
        match runs.iter().filter_map(|s| s.gap_at_open).min() {
            Some(i64::MAX) => "; at every trapdoor's opening its pocket's departing cart was gone".to_string(),
            Some(g) => format!("; the closest a pocket's departing cart was to its pocket as the trapdoor opened: {:.2}", g as f64 / 100.0),
            None => String::new(),
        },
        runs.iter().filter_map(|s| s.launch_gap).min().map_or(String::new(), |g| format!("; the least launch spacing {g} gt")),
        format!("; trapdoors shut onto a cart {} (in {} runs), R line switches while frozen {}", runs.iter().map(|s| s.shut_onto).sum::<usize>(), runs.iter().filter(|s| s.shut_onto > 0).count(), runs.iter().map(|s| s.r_frozen).sum::<usize>()),
        {
            let hl: Vec<i64> = (0..3).map(|k| runs.iter().map(|s| s.hold_lane[k]).max().unwrap_or(0)).collect();
            if runs.iter().any(|s| s.calm.is_some()) {
                format!(
                    "; the holds' longest uncovered stretch per lane (x -1/3/7) {}/{}/{} gt; the nearest zombie to any villager {}",
                    hl[0],
                    hl[1],
                    hl[2],
                    runs.iter().filter_map(|s| s.zombie_any_at.clone()).min_by(|a, b| a.0.total_cmp(&b.0)).map_or("-".to_string(), |(d, at)| format!("{d:.4}{at}"))
                )
            } else {
                String::new()
            }
        },
        {
            // (the R line's timing: the last trapdoor's shut, the R line's first tick, the next
            // batch all in its holds; earliest..latest over the runs)
            let span = |f: &dyn Fn(&DropStats) -> Option<i64>| {
                let v: Vec<i64> = runs.iter().filter_map(f).collect();
                match (v.iter().min(), v.iter().max()) {
                    (Some(a), Some(b)) => format!("T+{a}..T+{b}"),
                    _ => "-".to_string(),
                }
            };
            let r_line = if runs.iter().any(|s| s.r_from.is_some()) {
                format!("; the last trapdoor shut {}, the R line from {}, the next batch in its holds by {}", span(&|s| s.shut_last), span(&|s| s.r_from), span(&|s| s.next_in))
            } else {
                String::new()
            };
            // (tick-exact's void condition for the 6.08; the last departing rider against the
            // next golem's lock: out of the station by T+a..b, so many gt before T+95 for the merge
            // and the lift to carry its feet to 14.54)
            let toward = runs.iter().filter_map(|s| s.fall_toward).fold(f64::NEG_INFINITY, f64::max);
            let off = runs.iter().map(|s| s.fall_off).fold(0.0, f64::max);
            let outs: Vec<i64> = runs.iter().filter_map(|s| s.last_out).collect();
            // (the measured departure variation per lane)
            let lanes: Vec<String> = (0..3)
                .filter_map(|k| {
                    let v: Vec<(i64, i64, f64)> = runs.iter().filter_map(|s| s.lane_times[k]).collect();
                    if v.is_empty() {
                        return None;
                    }
                    let (l0, l1) = (v.iter().map(|x| x.0).min().unwrap(), v.iter().map(|x| x.0).max().unwrap());
                    let (e0, e1) = (v.iter().map(|x| x.1).min().unwrap(), v.iter().map(|x| x.1).max().unwrap());
                    let (s0, s1) = (v.iter().map(|x| x.2).fold(f64::INFINITY, f64::min), v.iter().map(|x| x.2).fold(0.0, f64::max));
                    Some(format!("lane {k} leaves its rest +{l0}..+{l1}, the station +{e0}..+{e1} at {s0:.3}..{s1:.3}"))
                })
                .collect();
            let lanes = if lanes.is_empty() { String::new() } else { format!("; departures from each lane's launch tick (REL + offset): {}", lanes.join(", ")) };
            // (the panic window: per summoner the least margins over the runs)
            let pan: Vec<String> = (0..3)
                .filter_map(|k| {
                    let v: Vec<(usize, i64, i64, Option<i64>, Option<i64>)> = runs.iter().filter_map(|s| s.panic[k]).collect();
                    if v.is_empty() {
                        return None;
                    }
                    let (ma, mb) = (v.iter().map(|x| x.1).min().unwrap(), v.iter().map(|x| x.2).min().unwrap());
                    let fails = v.iter().filter(|x| x.1 < 0 || x.2 < 0).count();
                    let ea = v.iter().filter_map(|x| x.3).min();
                    let eb = v.iter().filter_map(|x| x.4).min();
                    let calls = runs.iter().filter_map(|s| s.panic_calls[k]).map(|c| c.0).min();
                    Some(format!(
                        "lane {k}: calls in at least {} of 400 phase pairs, in range margin {ma}{}, seen margin {mb}{}, failing window runs {fails}",
                        calls.map_or("-".to_string(), |c| c.to_string()),
                        ea.map_or(String::new(), |t| format!(" (earliest fail T+{t})")),
                        eb.map_or(String::new(), |t| format!(" (earliest fail T+{t})"))
                    ))
                })
                .collect();
            let least = runs.iter().filter_map(|s| s.panic_callers).min();
            let all3 = runs.iter().filter(|s| s.panic_callers == Some(3)).count();
            let lanes = if pan.is_empty() {
                lanes
            } else {
                format!(
                    "{lanes}; the panic at the next summon: {}; the least caller count {} (the stand's bar >= 1, margin 2), all three calling in every phase pair in {all3} of {} runs (the retired all-three rule)",
                    pan.join(", "),
                    least.map_or("-".to_string(), |c| c.to_string()),
                    runs.len()
                )
            };
            format!(
                "{r_line}; the falling carts' largest horizontal offset from their shaft's centre line {off:.4}{}{}; villagers in a shaft or on a departure rail at a summon tick (T+0/100/200, before and after) {} (in {} runs; required 0){lanes}",
                if toward.is_finite() { format!(" (toward a zombie post's seat {toward:+.4}{})", if toward >= 0.03 { ": AT OR OVER 0.03, THE POST MOVES" } else { "" }) } else { String::new() },
                match (outs.iter().min(), outs.iter().max()) {
                    (Some(a), Some(b)) => format!("; the last departing rider out of the station (loop z 6.9, feet 0.06) at T+{a}..T+{b}: {} gt before T+95 for the merge and the lift to lift its feet to 14.54", 95 - b),
                    _ => String::new(),
                },
                runs.iter().map(|s| s.summon_off).sum::<usize>(),
                runs.iter().filter(|s| s.summon_off > 0).count()
            )
        }
    )
}

#[cfg(test)]
mod tests {
    /// A design header's declared cart contacts: ironfarm's `cells=what` form, cells joined by '+',
    /// and the parenthesized form; the text after '=' is not read.
    #[test]
    fn declared_contacts_reads_both_forms() {
        let t = "# contacts: -1,24,5+3,24,5+7,24,5=the drop entry's backstop, a stop: (evidence (3,4,5) here); -6,2,7+-6,3,7=the foot entry's back wall, PROVISIONAL (bench evidence only)\n# stops: (-6,12,7) (\u{2212}6,13,7)=walls\nname: x\n";
        assert_eq!(
            super::declared_contacts(t).unwrap(),
            vec![((-1, 24, 5), false), ((3, 24, 5), false), ((7, 24, 5), false), ((-6, 2, 7), true), ((-6, 3, 7), true), ((-6, 12, 7), false), ((-6, 13, 7), false)]
        );
        // (strict: a malformed cell refuses the header; no header, no contacts; an entry without its
        // `=what` or its cells refuses too)
        assert!(super::declared_contacts("# contacts: 1,2+3,4,5=a typo\n").is_err());
        assert!(super::declared_contacts("# contacts: 1,2,x=a typo\n").is_err());
        assert!(super::declared_contacts("# contacts: 1,2,3\n").is_err());
        assert!(super::declared_contacts("# contacts: 1,2,3=\n").is_err());
        assert!(super::declared_contacts("# contacts: =a wall\n").is_err());
        assert_eq!(super::declared_contacts("name: x\n").unwrap(), vec![]);
    }

    /// (ironspec 14:47 (a)) The station file as written: its `# contacts:` line (with "; " inside
    /// its evidence parentheses and a trailing clause with no `=`) reads as ONE entry of three
    /// cells, through the bench's strict reader and main's bounded and strict readers alike.
    #[test]
    fn the_station_files_contacts_line_is_one_entry() {
        let t = include_str!("../../../designs/farms/iron/carousel-assembly/wip/station/f1q2-station.txt");
        let cells = vec![((-1, 16, -1), false), ((3, 16, -1), false), ((7, 16, -1), false)];
        assert_eq!(super::declared_contacts(t).unwrap(), cells);
        let bounded = crate::contacts::declared_contacts_bounded(t).unwrap();
        assert_eq!(bounded.len(), 1, "{bounded:?}");
        assert_eq!(bounded[0].cells, vec![redsim::pos::BlockPos::new(-1, 16, -1), redsim::pos::BlockPos::new(3, 16, -1), redsim::pos::BlockPos::new(7, 16, -1)]);
        assert!(bounded[0].reason.contains("no other station cell is a declared contact"), "{:?}", bounded[0].reason);
        let strict = crate::contacts::declared_contacts_strict(t).unwrap();
        assert_eq!(strict.len(), 3, "{strict:?}");
        assert!(strict.iter().all(|(_, r)| *r == strict[0].1), "{strict:?}");
        // (G1 on e2d9333bc (B12): both readers keep the same reason)
        assert_eq!(strict[0].1, bounded[0].reason);
    }

    /// (the checklist audit (3)) The stand-in guard refuses a stand-in that differs from its built
    /// cell: a lever for dust passes, glass for air and a redstone block for dust don't.
    #[test]
    fn guard_refuses_a_differing_stand_in() {
        let w = redsim::world::World::new(redsim::pos::BlockPos::new(0, 0, 0), [1, 1, 1]);
        let b = |s: &str| redsim::block::Block::parse(s).unwrap();
        let dust = b("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]");
        assert!(super::property_diff(&w, &dust, &b("minecraft:lever[face=floor,facing=east,powered=false]")).is_empty());
        assert!(!super::property_diff(&w, &b("minecraft:air"), &b("minecraft:glass")).is_empty());
        assert!(!super::property_diff(&w, &dust, &b("minecraft:redstone_block")).is_empty());
    }

    /// (the checklist audit (5)) The summon's own clock: S is the first tick after T at game time
    /// % 100 == 0 (the bench's own function, `s_rel_of`).
    #[test]
    fn summon_clock() {
        assert_eq!(super::s_rel_of(2_400), 100);
        assert_eq!(super::s_rel_of(2_437), 63);
        assert_eq!(super::s_rel_of(2_499), 1);
        assert_eq!(super::s_rel_of(-1), 1);
    }

    /// The voxel walk visits every cell a segment crosses, end to end.
    #[test]
    fn seg_cells_walks_the_segment() {
        let v = |x: f64, y: f64, z: f64| redsim::entity::Vec3::new(x, y, z);
        let c = super::seg_cells(v(0.5, 0.5, 0.5), v(3.5, 0.5, 0.5));
        assert_eq!(c.len(), 4);
        let d = super::seg_cells(v(0.5, 0.5, 0.5), v(2.5, 2.5, 0.5));
        assert_eq!(d.first(), Some(&redsim::pos::BlockPos::new(0, 0, 0)));
        assert_eq!(d.last(), Some(&redsim::pos::BlockPos::new(2, 2, 0)));
    }
}
