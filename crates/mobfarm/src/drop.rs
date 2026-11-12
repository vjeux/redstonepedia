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
        b.set(x, y - 1, z, "stone");
        b.set(x, y, z - 1, "stone");
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
            b.set(x + 1, hold_y + 1, z + 1, "glass");
            b.set(x + 1, hold_y + 2, z + 1, "glass");
            b.set(x, hold_y + 2, z + 2, "rail[shape=ascending_south,waterlogged=false]");
            b.set(x, hold_y + 1, z + 2, "glass");
            b.set(x, hold_y + 1, z - 1, if extra.slab_stop { "smooth_stone_slab[type=bottom,waterlogged=false]" } else { "glass" });
            b.set(x, hold_y + 1, z, "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
            b.set(x - 1, hold_y + 1, z, "tripwire_hook[attached=true,facing=east,powered=false]");
            b.set(x + 1, hold_y + 1, z, "tripwire_hook[attached=true,facing=west,powered=false]");
            b.set(x - 2, hold_y + 1, z, "glass");
            b.set(x + 2, hold_y + 1, z, "glass");
        }
        // the merge's cells (a stand-in): a plain rail, then the exit detector (no face on a powered
        // rail or a junction: plain rails north and south of it; its rising edge is the departing
        // cart's exit, the --exit-gate evidence), a plain rail, four boosters, a plain rail, a stop
        b.set(x, y, z + 1, "rail[shape=north_south,waterlogged=false]");
        b.set(x, y - 1, z + 1, "glass");
        b.set(x, y, z + 2, "detector_rail[powered=false,shape=north_south,waterlogged=false]");
        b.set(x, y - 1, z + 2, "glass");
        b.set(x, y, z + 3, "rail[shape=north_south,waterlogged=false]");
        b.set(x, y - 1, z + 3, "glass");
        // (three boosters to loop z 6, the station's last cell; loop z 7 is the merge row
        // (7b90c3f1's, shifted from z 8 for the strict pitch 32): the bench takes the cart out at
        // loop z 6.9, still on z 6's rail, its speed the merge row's entry speed)
        for dz in 4..=6 {
            b.set(x, y, z + dz, "powered_rail[powered=true,shape=north_south,waterlogged=false]");
            b.set(x, y - 1, z + dz, "redstone_block");
        }
    }
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
        }
    }
    let rider = "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}";
    let empty = "{Rotation:[0f,0f]}";
    for (k, (x, y, z)) in pockets.iter().enumerate() {
        b.entity("minecart", [*x as f64 + 0.5, *y as f64 + 0.0625, *z as f64 + 0.5], Some(if dep[k] { rider } else { empty }));
    }
    for (k, (x, _, z)) in pockets.iter().enumerate() {
        b.entity("minecart", [*x as f64 + 0.5, hold_y as f64 + 0.1875, *z as f64 + 0.5], Some(if arr[k] { rider } else { empty }));
    }
    if refill {
        // the next batch's carts at rest mid-cell on R_1 (braked)
        for (x, _, z) in pockets.iter() {
            b.entity("minecart", [*x as f64 + 0.5, hold_y as f64 + 1.5625, *z as f64 + 1.5], Some(rider));
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

/// The station's box against the budget's (static).
fn station_footprint(hold_y: i32) -> String {
    format!(
        "the station's blocks: file x 3..15 y -9..{hold_y} z 0..7 (loop x -3..9, z -6..1: the glass rings' z +1 cells sit in the lanes' box, as the budget lists them), the waiting riders' heads at {:.2}; the budget's box: loop x -3..9 y -11..19 z -6..0; the merge's stand-in: file z 7..15 (loop 1..9, the merge's box); each exit detector at loop (x,0,2): faces north (x,0,1) a plain rail, south (x,0,3) a plain rail, east and west air, down glass, up air (no powered rail, gate or junction)",
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

/// One drop run's figures for a grid's summary.
#[derive(Clone, Debug, Default)]
pub struct DropStats {
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

fn summon_boundary(frozen: &[bool]) -> i64 {
    (1..10i64).map(|k| 100 * k).find(|b| (*b as usize) >= frozen.len() || !frozen[*b as usize]).unwrap_or(100)
}

/// One drop run: the report lines and the figures.
pub fn bench(spec: &FarmSpec, o: &DropOpts) -> Result<(Vec<String>, DropStats), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
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
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (rs, stone, td_open, td_shut, glass) = (parse("redstone_block")?, parse("stone")?, parse(TRAPDOOR_OPEN)?, parse(TRAPDOOR_SHUT)?, parse("glass")?);
    for t in 0..400 {
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
    let mut rest_since: [Option<i64>; 3] = [None; 3];
    let mut rebrake_dt: Option<i64> = None;
    let mut rebrake_at: [Option<i64>; 3] = [None; 3];
    let mut calm_run = [0i64; 9];
    let mut all_shut: Option<i64> = None;
    let mut r_from: Option<i64> = None;
    let mut r_to: Option<i64> = None;
    let mut next_rest: [Option<(i64, Vec3)>; 3] = [None; 3];
    let mut next_fell = [false; 3];
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
            reloaded.push(format!("T+{dt} {}", how.label()));
        }
        let frozen_now = o.freeze.iter().chain(o.logins.iter()).any(|(f, l)| dt >= *f && dt < f + l);
        let live = dt - frozen_ticks;
        // (the controller's clock: L1 (G1/ironspec 00:18, the lead) runs the controller and its
        // timers on game ticks through a freeze; L2 (the ring clock) on the live clock)
        let clock = if o.l1 { dt } else { live };
        // the pockets' supports (and the series layout's gate slopes): REL + stagger .. REL + rebrake
        for k in 0..3 {
            let off_at = o.rel + o.rebrake + if o.rebrake_each { o.stagger[k] } else { 0 };
            let on = clock >= o.rel + o.stagger[k] && clock < off_at;
            if rebrake_at[k].is_none() && clock >= off_at {
                rebrake_at[k] = Some(dt);
            }
            let want = if on { rs.clone() } else { stone.clone() };
            let (x, y, z) = pockets[k];
            let mut cells = vec![BlockPos::new(x, y - 1, z)];
            if o.layout == DropLayout::Series {
                let (sx, sy, sz) = SERIES_SLOPE_SRC[k];
                cells.push(BlockPos::new(sx, sy, sz));
            }
            for p in cells {
                if run.world.get(p) != want {
                    run.world.set_block(p, want.clone(), redsim::world::flags::ALL);
                }
            }
        }
        if rebrake_dt.is_none() && clock >= o.rel + o.rebrake {
            rebrake_dt = Some(dt);
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
                let p = BlockPos::new(x + 1, o.hold_y + 1, z + 1);
                let want = if on { rs.clone() } else { glass.clone() };
                if run.world.get(p) != want {
                    run.world.set_block(p, want, redsim::world::flags::ALL);
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
        run.tick();
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
        // (the next batch: in the hold at rest, or fallen through)
        for (j, id) in next_carts.iter().enumerate() {
            let Some((p, v)) = pos(w, *id) else { continue };
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
        let mut sink_now: Vec<usize> = Vec::new();
        for n in 0..6 {
            let Some((p, v)) = st[n] else { continue };
            if n < 3 {
                // (the launch: the first tick the departing cart is off its pocket's centre)
                let (px, _, pz) = pockets[n];
                if launch_at[n].is_none() && ((p.x - (px as f64 + 0.5)).abs() > 0.05 || (p.z - (pz as f64 + 0.5)).abs() > 0.05) {
                    launch_at[n] = Some(dt);
                }
                if exits[n].is_none() && o.layout.departed(p) {
                    exits[n] = Some((dt, (v.x * v.x + v.z * v.z).sqrt()));
                    sink_now.push(n);
                }
                continue;
            }
            let k = n - 3;
            if opened[k].is_none() {
                continue;
            }
            let (_, py, pz) = pockets[k];
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
                        if zombie {
                            zombie_any = Some(zombie_any.map_or(d, |m: f64| m.min(d)));
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
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else { continue };
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
        }
    }
    let boundary = summon_boundary(&frozen_hist);
    let mut stats = DropStats::default();
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
            zombie_any.map_or("-".to_string(), |d| format!("{d:.2}{}", if d < 6.05 { " (INSIDE 6.05)" } else { "" })),
            pillager_near.map_or("-".to_string(), |d| format!("{d:.2}{}", if d < 8.5 { " (INSIDE 8.5)" } else { "" }))
        ));
        stats.calm = Some(calm_max);
        stats.pillager_near = pillager_near.map(|d| (d * 100.0).floor() as i64);
        stats.zombie_near = zombie_near.map(|d| (d * 100.0).floor() as i64);
        stats.zombie_any = zombie_any.map(|d| (d * 100.0).floor() as i64);
        stats.hold_lane = hold_lane;
    }
    Ok((out, stats))
}

/// The grid's summary over many drop runs.
pub fn summary(label: &str, runs: &[DropStats]) -> String {
    let show = |v: i64| if v == i64::MIN { "FAIL".to_string() } else { v.to_string() };
    format!(
        "{label}: {} runs; the widest release spread {:.2}; the longest fall {}; the least landing lead over its pocket's re-brake {}; arriving at rest: least {} before the next summon; wrong pockets {}, contacts {} (the fastest {:.3}), the closest arriving-to-departing {}; riders hurt {}, unseated ticks {}; departing carts stuck {}; the longest wait on a string {} gt; the lowest waiting rider's feet {:.2}{}{}{}{}{}{}",
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
        if runs.iter().any(|s| s.refill_fail > 0) { format!("; REFILL FAILED in {} runs", runs.iter().filter(|s| s.refill_fail > 0).count()) } else { String::new() },
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
                format!("; the holds' longest uncovered stretch per lane (x -1/3/7) {}/{}/{} gt; the nearest zombie to any villager {}", hl[0], hl[1], hl[2], runs.iter().filter_map(|s| s.zombie_any).min().map_or("-".to_string(), |c| format!("{:.2}", c as f64 / 100.0)))
            } else {
                String::new()
            }
        }
    )
}
