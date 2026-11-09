//! The villager carousel, layout L1 (brief `designs/briefs/farm-iron-carousel.md`, explore/ironspec;
//! vjeux 2026-11-08 22:53: "I'd love to pack tons of villagers and move them around or something so
//! we can have golems spawning constantly"): groups of 3 villagers, one per minecart, on 3
//! concentric rail loops (each group has one villager on each loop). Every loop stops its carts
//! after each of its 4 corners on a brake (an unpowered powered rail on a slope down); the golem's
//! fall through a tripwire in the drop shaft powers every brake at once for about 10 gt, so each
//! cart rolls off its stop, boosters carry it along the side and it brakes on the next stop. The
//! north-west stops are the station, in sight of the zombie within 8 blocks and over the pad;
//! the others wait out of the station's 10-box and every golem's follow box (R + 1 = 18.8 sideways).
//! No night stop yet (the villagers' last sleep is the judge's: the first game day).
//!
//! Coordinates: loop k's track runs on the square from (t, t) to (e, e), t = -3k, e = side + 3k;
//! its corners at y 0, every other rail at y -1 (supports one below). Inside each track lie its
//! repeater lane (1 in) and its bus lane (2 in): the pulse's dust at y -1.

use crate::layouts::Builder;
use std::collections::{BTreeMap, BTreeSet};

const STONE: &str = "cobblestone";
const GLASS: &str = "glass";

/// A rail cell: (x, y, z) and its block.
type Cell = ([i32; 3], String);

/// The 3 villagers' loops, `stops` stops each (4: one after each corner), the inner loop's side
/// `side` (28: the waiting stops more than 18.8 from every golem on the pad).
pub fn carousel(side: i32) -> anyhow::Result<String> {
    anyhow::ensure!(side >= 28, "the carousel's inner loop needs a side of 28 or more (its waiting stops out of the golems' follow box)");
    let mut b = Builder::new(&format!("iron-carousel-3x4-{side}"));
    let mut dust: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut redstone_blocks: BTreeSet<[i32; 3]> = BTreeSet::new();
    for k in 0..3 {
        let (t, e) = (-3 * k, side + 3 * k);
        for (p, s) in loop_rails(t, e) {
            b.set(p[0], p[1], p[2], &s);
        }
        // the supports: stone, a redstone block under every 8th booster and the climb
        for (p, s) in loop_rails(t, e) {
            let support = [p[0], p[1] - 1, p[2]];
            if b.d.blocks.contains_key(&support) {
                continue;
            }
            let rb = s.contains("powered_rail[powered=true");
            b.set(support[0], support[1], support[2], if rb && booster_powered(t, e, p) { "redstone_block" } else { STONE });
            if rb && booster_powered(t, e, p) {
                redstone_blocks.insert(support);
            }
        }
        bus(&mut b, &mut dust, t, e);
    }
    // the feed: the tripwire hook's block (3, -8, 5) up a dust stair south along x 2 to the
    // basement (y -3, under every rail's support), then to bus 0 (up, north along x 2) and under
    // the west tracks to buses 1 and 2 (up, north along x -1 and x -4), entering each bus at its
    // station end; repeaters where the dust runs low
    let stair = [[2, -8, 5], [2, -7, 6], [2, -6, 7], [2, -5, 8], [2, -4, 9], [2, -3, 10]];
    let feed0 = [[2, -2, 11], [2, -1, 12], [1, -1, 12], [1, -1, 11], [1, -1, 10], [1, -1, 8], [1, -1, 7], [1, -1, 6], [1, -1, 5], [1, -1, 4], [1, -1, 3]];
    let basement = [[0, -3, 10], [-1, -3, 10], [-2, -3, 10], [-3, -3, 10], [-4, -3, 10]];
    let feed1 = [[-1, -2, 9], [-1, -1, 8], [-1, -1, 7], [-1, -1, 6], [-1, -1, 5], [-1, -1, 3], [-1, -1, 2], [-1, -1, 1], [-1, -1, 0]];
    let feed2 = [[-4, -2, 9], [-4, -1, 8], [-4, -1, 7], [-4, -1, 6], [-4, -1, 5], [-4, -1, 3], [-4, -1, 2], [-4, -1, 1], [-4, -1, 0], [-4, -1, -1], [-4, -1, -2], [-4, -1, -3]];
    for p in stair.iter().chain(feed0.iter()).chain(basement.iter()).chain(feed1.iter()).chain(feed2.iter()) {
        anyhow::ensure!(!redstone_blocks.contains(&[p[0], p[1] + 1, p[2]]), "the feed's dust at {p:?} runs under a booster's redstone block");
        dust.insert(*p);
    }
    // (`facing` names the input side)
    for (p, facing) in [([1, -3, 10], "east"), ([1, -1, 9], "south"), ([-1, -1, 4], "south"), ([-4, -1, 4], "south")] {
        b.set(p[0], p[1], p[2], &format!("repeater[delay=1,facing={facing},locked=false,powered=false]"));
        b.set(p[0], p[1] - 1, p[2], STONE);
    }
    station(&mut b);
    carts(&mut b, side);
    // every dust on a block, its shape from its neighbours
    for p in &dust {
        if !b.d.blocks.contains_key(&[p[0], p[1] - 1, p[2]]) {
            b.set(p[0], p[1] - 1, p[2], STONE);
        }
    }
    for p in &dust {
        let s = dust_state(&b.d.blocks, &dust, *p);
        b.set(p[0], p[1], p[2], &s);
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    Ok(b.text(&[
        &format!("Carousel (L1 prototype): 4 groups of 3 villagers, one per minecart, on 3 concentric loops (inner side {side}); the golem's fall"),
        "through a tripwire releases every cart at once to its next stop; the north-west stops are the station over the pad. No night stop yet.",
    ]))
}

/// Loop (t, e)'s rails: 4 sides, each from its start corner: the corner (y 0), the stop (a
/// powered rail sloping down the travel direction, unpowered: a brake), a plain rail (no booster
/// beside the stop: powered rails pass power along a line), boosters at y -1, the climb back up.
fn loop_rails(t: i32, e: i32) -> Vec<Cell> {
    let n = e - t;
    let sides = [([t, t], [1, 0], "south_east"), ([e, t], [0, 1], "south_west"), ([e, e], [-1, 0], "north_west"), ([t, e], [0, -1], "north_east")];
    let dir_name = |d: [i32; 2]| match d {
        [1, 0] => "east",
        [-1, 0] => "west",
        [0, 1] => "south",
        _ => "north",
    };
    let mut out = Vec::new();
    for (start, d, corner) in sides {
        let at = |i: i32| [start[0] + d[0] * i, start[1] + d[1] * i];
        let axis = if d[1] == 0 { "east_west" } else { "north_south" };
        let back = dir_name([-d[0], -d[1]]);
        let c = at(0);
        out.push(([c[0], 0, c[1]], format!("rail[shape={corner},waterlogged=false]")));
        for i in 1..n {
            let c = at(i);
            // one booster per side, right after the plain rail: a cart's speed only grows on
            // boosters (+0.06 a tick, the move clamped to 0.4 but not the motion), and a stop must
            // halve it to nothing within its one rail; the rest plain, the climb plain too
            let s = match i {
                1 => format!("powered_rail[powered=false,shape=ascending_{back},waterlogged=false]"),
                3 => format!("powered_rail[powered=true,shape={axis},waterlogged=false]"),
                _ if i == n - 1 => format!("rail[shape=ascending_{},waterlogged=false]", dir_name(d)),
                _ => format!("rail[shape={axis},waterlogged=false]"),
            };
            out.push(([c[0], -1, c[1]], s));
        }
    }
    out
}

/// Whether the booster at `p` on loop (t, e) stands on a redstone block: the side's one booster
/// (its third rail).
fn booster_powered(t: i32, e: i32, p: [i32; 3]) -> bool {
    // the rail's index on its side
    let i = if p[2] == t && p[0] > t {
        p[0] - t
    } else if p[0] == e && p[2] > t {
        p[2] - t
    } else if p[2] == e && p[0] < e {
        e - p[0]
    } else {
        e - p[2]
    };
    i == 3
}

/// Loop (t, e)'s pulse bus at y -1 in its bus lane (2 in from the track), north, east and south
/// sides from the station, with a repeater 1 in from each stop facing it, branches to the
/// north-east and south-east stops' repeaters, and repeaters along the line (a dust loses 1 a block).
fn bus(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>, t: i32, e: i32) {
    let y = -1;
    let mut path: Vec<[i32; 3]> = Vec::new();
    for x in t + 1..=e - 2 {
        path.push([x, y, t + 2]);
    }
    for z in t + 3..=e - 2 {
        path.push([e - 2, y, z]);
    }
    for x in (t + 2..=e - 3).rev() {
        path.push([x, y, e - 2]);
    }
    path.push([t + 2, y, e - 1]);
    // in-line repeaters: the first 6 after the entry (index 1), then every 13, never at a turn
    let turn = |i: usize| i == 0 || i + 1 >= path.len() || {
        let (a, c) = (path[i - 1], path[i + 1]);
        a[0] != c[0] && a[2] != c[2]
    };
    let mut next = 7usize;
    for i in 0..path.len() {
        let p = path[i];
        if i == next && !turn(i) && i + 2 < path.len() {
            let a = path[i - 1];
            let facing = match [a[0] - p[0], a[2] - p[2]] {
                [-1, 0] => "west",
                [1, 0] => "east",
                [0, -1] => "north",
                _ => "south",
            };
            b.set(p[0], p[1], p[2], &format!("repeater[delay=1,facing={facing},locked=false,powered=false]"));
            b.set(p[0], p[1] - 1, p[2], STONE);
            next = i + 13;
            continue;
        }
        if i == next {
            next += 1;
        }
        dust.insert(p);
    }
    dust.insert([e - 2, y, t + 1]);
    dust.insert([e - 1, y, e - 2]);
    // the stops' repeaters (`facing` names the input side)
    for (p, facing) in [([t + 1, y, t + 1], "south"), ([e - 1, y, t + 1], "west"), ([e - 1, y, e - 1], "north"), ([t + 1, y, e - 1], "east")] {
        b.set(p[0], p[1], p[2], &format!("repeater[delay=1,facing={facing},locked=false,powered=false]"));
        b.set(p[0], p[1] - 1, p[2], STONE);
    }
}

/// The station inside the inner loop's north-west corner: the pad (x 3..8, z 3..8, floor y 0)
/// under water from its 4 corners, walled in glass; its 2x2 hole (x 5..6, z 5..6) capped by wall
/// signs (dry: they stop the water, a golem falls through them); the shaft, a tripwire across it
/// at y -1 (its hooks at x 4 and 7, z 5: the hook's block (3, -1, 5) feeds the buses); the kill at
/// y -22 (the station's villagers ride at y -0.94: 21 below, out of their follow box), a 2x2 of
/// hoppers under open gates and lava; the chest. The zombie in a boat at (1.5, 0, -4.5), within 8
/// of the 3 station carts and in their sight, roofed with leaves.
fn station(b: &mut Builder) {
    for x in 3..=8 {
        for z in 3..=8 {
            if (5..=6).contains(&x) && (5..=6).contains(&z) {
                continue;
            }
            b.set(x, 0, z, STONE);
        }
    }
    for (x, z) in [(3, 3), (8, 3), (3, 8), (8, 8)] {
        b.set(x, 1, z, "water[level=0]");
    }
    for y in 1..=3 {
        for i in 2..=9 {
            for (x, z) in [(i, 2), (i, 9), (2, i), (9, i)] {
                b.set(x, y, z, GLASS);
            }
        }
    }
    for (x, z, facing) in [(5, 5, "east"), (5, 6, "east"), (6, 5, "west"), (6, 6, "west")] {
        b.set(x, 0, z, &format!("oak_wall_sign[facing={facing},waterlogged=false]"));
    }
    // the shaft's ring from y -1 down to the chamber's floor (-23)
    let k = -22;
    for y in k - 1..=-1 {
        for (x, z) in [(4, 5), (4, 6), (7, 5), (7, 6), (5, 4), (6, 4), (5, 7), (6, 7)] {
            b.set(x, y, z, STONE);
        }
    }
    // the tripwire 8 below the pad (the golem has sunk out of the pad's water and falls free: the
    // carts leave as it drops out of every villager's follow box, R + 2.7 below): hooks on blocks
    // at x 3 and 8, the string across
    let tw = -8;
    b.set(3, tw, 5, STONE);
    b.set(8, tw, 5, STONE);
    b.set(4, tw, 5, "tripwire_hook[attached=true,facing=east,powered=false]");
    b.set(7, tw, 5, "tripwire_hook[attached=true,facing=west,powered=false]");
    for x in 5..=6 {
        b.set(x, tw, 5, "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
    }
    // the kill: lava at k + 2 over open gates at k + 1, the golem on hoppers at k - 1
    for x in 5..=6 {
        for z in 5..=6 {
            b.set(x, k + 2, z, "lava[level=0]");
            b.set(x, k + 1, z, "oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]");
        }
    }
    b.set(5, k - 1, 5, "hopper[enabled=true,facing=east]");
    b.set(6, k - 1, 5, "hopper[enabled=true,facing=south]");
    b.set(5, k - 1, 6, "hopper[enabled=true,facing=east]");
    b.set(6, k - 1, 6, "hopper[enabled=true,facing=down]");
    b.set(6, k - 2, 6, "chest[facing=north,type=single,waterlogged=false]");
    b.d.ports.insert("output".into(), [6, k - 2, 6]);
    // the zombie (on glass: no golem spawns there) 3 above the carts' level: out of their attack
    // reach (a zombie's attack box grows only sideways), within 8 of the station's three, in sight;
    // leaves over it
    b.set(1, 2, -5, GLASS);
    b.set(1, 6, -5, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
    b.entity("oak_boat", [1.5, 3.0, -4.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
}

/// Places the 12 carts with their villagers: one on each stop of each loop. (No night stop yet:
/// each villager's HOME is a bed under its first stop, 2.1 below its feet, which it never reaches
/// from its cart; it sleeps only before the first day, so the prototype makes golems one day.)
pub fn carts(b: &mut Builder, side: i32) {
    for k in 0..3 {
        let (t, e) = (-3 * k, side + 3 * k);
        for (p, back) in [([t + 1, t], [-1, 0]), ([e, t + 1], [0, -1]), ([e - 1, e], [1, 0]), ([t, e - 1], [0, 1])] {
            // the cart on the slope's middle: half a block down
            b.entity("minecart", [p[0] as f64 + 0.5, -1.0 + 0.5 + 0.0625, p[1] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}"));
            let facing = match back {
                [-1, 0] => "east",
                [1, 0] => "west",
                [0, -1] => "south",
                _ => "north",
            };
            b.set(p[0], -3, p[1], &format!("red_bed[facing={facing},occupied=false,part=head]"));
            b.set(p[0] + back[0], -3, p[1] + back[1], &format!("red_bed[facing={facing},occupied=false,part=foot]"));
            for q in [[p[0], -4, p[1]], [p[0] + back[0], -4, p[1] + back[1]]] {
                b.set(q[0], q[1], q[2], STONE);
            }
        }
    }
}

/// A dust's state from its neighbours (`RedStoneWireBlock.getConnectionState`): a side to dust
/// beside it (or one down past a non-solid cell, or one up under a non-solid cell), to a
/// repeater in line, to a signal source; one side runs through to the opposite one; none: a cross.
fn dust_state(blocks: &BTreeMap<[i32; 3], String>, dust: &BTreeSet<[i32; 3]>, p: [i32; 3]) -> String {
    let solid = |q: [i32; 3]| blocks.get(&q).is_some_and(|s| {
        let s = s.trim_start_matches("minecraft:");
        s.starts_with("stone") || s.starts_with("cobblestone") || s.starts_with("glass") || s.starts_with("redstone_block") || s.starts_with("oak_leaves")
    });
    let mut side: BTreeMap<&str, &str> = BTreeMap::new();
    for (name, d) in [("north", [0, -1]), ("south", [0, 1]), ("west", [-1, 0]), ("east", [1, 0])] {
        let n = [p[0] + d[0], p[1], p[2] + d[1]];
        let up = [n[0], n[1] + 1, n[2]];
        let down = [n[0], n[1] - 1, n[2]];
        let above_me = [p[0], p[1] + 1, p[2]];
        let s = blocks.get(&n).map(|s| s.trim_start_matches("minecraft:").to_string()).unwrap_or_default();
        let conn = if dust.contains(&n) {
            Some("side")
        } else if s.starts_with("repeater") {
            let facing = s.split("facing=").nth(1).map(|f| f.split(&[',', ']'][..]).next().unwrap_or("")).unwrap_or("");
            let axis_x = d[1] == 0;
            ((facing == "east" || facing == "west") == axis_x).then_some("side")
        } else if s.starts_with("redstone_block") || s.starts_with("tripwire_hook") || s.starts_with("redstone_torch") {
            Some("side")
        } else if dust.contains(&up) && !solid(above_me) {
            Some("up")
        } else if dust.contains(&down) && !solid(n) {
            Some("side")
        } else {
            None
        };
        if let Some(c) = conn {
            side.insert(name, c);
        }
    }
    let opposite = |n: &str| match n {
        "north" => "south",
        "south" => "north",
        "west" => "east",
        _ => "west",
    };
    if side.is_empty() {
        for n in ["north", "south", "west", "east"] {
            side.insert(n, "side");
        }
    } else if side.len() == 1 {
        let only = *side.keys().next().unwrap();
        side.insert(opposite(only), "side");
    }
    let g = |n: &str| side.get(n).copied().unwrap_or("none");
    format!("redstone_wire[east={},north={},power=0,south={},west={}]", g("east"), g("north"), g("south"), g("west"))
}
