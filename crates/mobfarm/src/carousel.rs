//! The villager carousel, layout L1 (brief `designs/briefs/farm-iron-carousel.md`, explore/ironspec;
//! vjeux 2026-11-08 22:53: "I'd love to pack tons of villagers and move them around or something so
//! we can have golems spawning constantly"): groups of 3 villagers, one per minecart, on 3
//! concentric rail loops (each group has one villager on each loop). Every loop stops its carts
//! on a brake (an unpowered powered rail on a slope down) a few rails after each of its 4 corners;
//! the golem's fall through a tripwire in the drop shaft powers every brake at once for about
//! 10 gt, so each cart rolls off its stop, a booster carries it along the side and it brakes on
//! the next stop. The north stops are the station, in sight of the zombie within 8 blocks and over
//! the pad; the others wait out of the station's 10-box and every golem's follow box.
//!
//! The night (each stop a bed stop, [`stop`]), safe for empty carts: an empty cart slows by 0.96 a
//! tick against a ridden one's 0.997 (`applyNaturalSlowdown`), so it coasts about 10 rails and
//! strands mid-side; no empty cart ever travels here. From dusk to dawn an activator rail just
//! before each brake drops the arriving cart's villager onto the stop's bed and the cart stays
//! parked on the brake, empty, all night: the night's one advance is the dusk pulse that drops (the
//! night signal's rising edge, [`night_pulse`]). The villager sleeps (a HOME it cannot reach from
//! the bed's cage it gives up after 1200 gt and claims this bed); at dawn it stands up in a pocket
//! just past the brake, and the pulse of its bed's waking (an observer on one bed) releases the
//! carts, each picking its villager up as it rolls off its brake.
//!
//! Coordinates: loop k's track runs on the square from (t, t) to (e, e), t = -3k, e = side + 3k;
//! each side from its start corner: the corner and two rails at y 0 (the second the activator), the
//! brake sloping to y -1 (on mud), the pocket, the booster, plain rails, the climb back to the
//! next corner. Inside each track lie its repeater lane (1 in: the beds) and its bus lane (2 in:
//! the pulse's dust at y -1, the brakes' repeaters under the beds).

use crate::layouts::Builder;
use std::collections::{BTreeMap, BTreeSet};

const STONE: &str = "cobblestone";
const GLASS: &str = "glass";

/// A side's rails by index from its start corner: the activator, the brake, the pocket, the booster.
const ACT: i32 = 2;
const BRAKE: i32 = 3;
const POCKET: i32 = 4;
const BOOST: i32 = 5;

/// A side of a loop: its start corner c (x, z), its direction d, the loop's inside n (the right of
/// the travel: the loops run clockwise).
#[derive(Clone, Copy)]
struct Side {
    c: [i32; 2],
    d: [i32; 2],
    n: [i32; 2],
}

impl Side {
    /// The cell (x, z) i rails along the side and `off` toward the inside.
    fn at(&self, i: i32, off: i32) -> [i32; 2] {
        [self.c[0] + self.d[0] * i + self.n[0] * off, self.c[1] + self.d[1] * i + self.n[1] * off]
    }
}

/// Loop (t, e)'s sides: north (east from (t, t)), east, south, west.
fn sides(t: i32, e: i32) -> [Side; 4] {
    [
        Side { c: [t, t], d: [1, 0], n: [0, 1] },
        Side { c: [e, t], d: [0, 1], n: [-1, 0] },
        Side { c: [e, e], d: [-1, 0], n: [0, -1] },
        Side { c: [t, e], d: [0, -1], n: [1, 0] },
    ]
}

fn name(d: [i32; 2]) -> &'static str {
    match d {
        [1, 0] => "east",
        [-1, 0] => "west",
        [0, 1] => "south",
        _ => "north",
    }
}

/// The 3 villagers' loops, 4 stops each (one after each corner), the inner loop's side `side`
/// (28: the waiting stops more than 18.8 from every golem on the pad).
pub fn carousel(side: i32) -> anyhow::Result<String> {
    anyhow::ensure!(side >= 28, "the carousel's inner loop needs a side of 28 or more (its waiting stops out of the golems' follow box)");
    let mut b = Builder::new(&format!("iron-carousel-3x4-{side}"));
    let mut dust: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut redstone_blocks: BTreeSet<[i32; 3]> = BTreeSet::new();
    for k in 0..3 {
        let (t, e) = (-3 * k, side + 3 * k);
        let rails = loop_rails(t, e);
        for (p, s, _) in &rails {
            b.set(p[0], p[1], p[2], s);
        }
        // the supports: stone, a redstone block under each booster, mud under each brake
        for (p, _, support) in &rails {
            let q = [p[0], p[1] - 1, p[2]];
            if b.d.blocks.contains_key(&q) {
                continue;
            }
            b.set(q[0], q[1], q[2], support);
            if *support == "redstone_block" {
                redstone_blocks.insert(q);
            }
        }
        bus(&mut b, &mut dust, t, e);
        for s in sides(t, e) {
            stop(&mut b, &mut dust, s, k == 2);
        }
    }
    // the feed: the tripwire hook's block (3, -8, 5) through the night gate (a repeater, locked
    // from dusk to dawn: [`night_gate`]) up a dust stair south along x 1 to the basement (y -3,
    // under every rail's support), then to bus 0 (up, north along x 2) and under the west tracks
    // to buses 1 and 2 (up, north along x -1 and x -4), entering each bus at its station end;
    // repeaters where the dust runs low
    let stair = [[1, -8, 5], [1, -7, 6], [1, -6, 7], [1, -5, 8], [1, -4, 9], [2, -4, 9], [2, -3, 10]];
    let feed0 = [[2, -2, 11], [2, -1, 12], [1, -1, 12], [1, -1, 11], [1, -1, 10], [1, -1, 8], [1, -1, 7], [1, -1, 6], [1, -1, 5], [1, -1, 4], [1, -1, 3]];
    let basement = [[0, -3, 10], [-1, -3, 10], [-2, -3, 10], [-3, -3, 10], [-4, -3, 10]];
    let feed1 = [[-1, -2, 9], [-1, -1, 8], [-1, -1, 7], [-1, -1, 6], [-1, -1, 5], [-1, -1, 3], [-1, -1, 2], [-1, -1, 1], [-1, -1, 0]];
    let feed2 = [[-4, -2, 9], [-4, -1, 8], [-4, -1, 7], [-4, -1, 6], [-4, -1, 5], [-4, -1, 3], [-4, -1, 2], [-4, -1, 1], [-4, -1, 0], [-4, -1, -1], [-4, -1, -2], [-4, -1, -3]];
    for p in stair.iter().chain(feed0.iter()).chain(basement.iter()).chain(feed1.iter()).chain(feed2.iter()) {
        anyhow::ensure!(!redstone_blocks.contains(&[p[0], p[1] + 1, p[2]]), "the feed's dust at {p:?} runs under a booster's redstone block");
        dust.insert(*p);
    }
    // (`facing` names the input side; the feed's repeaters at 4 ticks: they stretch the night's
    // 2 gt observer pulses to the tripwire's length)
    for (p, facing) in [([1, -3, 10], "east"), ([1, -1, 9], "south"), ([-1, -1, 4], "south"), ([-4, -1, 4], "south")] {
        b.set(p[0], p[1], p[2], &format!("repeater[delay=4,facing={facing},locked=false,powered=false]"));
        b.set(p[0], p[1] - 1, p[2], STONE);
    }
    night_gate(&mut b, &mut dust);
    night_pulse(&mut b, &mut dust);
    station(&mut b);
    curtain_dust(&mut dust);
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
        "through a tripwire releases every cart at once to its next stop; the north stops are the station over the pad. At dusk each cart drops its",
        "villager onto its stop's bed and waits there empty; at dawn it picks the villager up from a pocket past the brake.",
    ]))
}

/// Loop (t, e)'s rails with each one's support: per side from its start corner, the corner and a
/// rail at y 0, the activator (y 0), the brake (a powered rail sloping down to y -1, unpowered,
/// on mud: 14 pixels high, it leaves the cell above no floor, so neither a dropped villager nor a
/// waking one is put there, `getBlockFloorHeight`), the
/// pocket, the booster (on a redstone block: a cart's speed grows only on boosters, and a brake
/// must halve it to nothing within its one rail), plain rails, the climb back up to the corner.
fn loop_rails(t: i32, e: i32) -> Vec<([i32; 3], String, &'static str)> {
    let n = e - t;
    let corners = ["south_east", "south_west", "north_west", "north_east"];
    let mut out = Vec::new();
    for (s, corner) in sides(t, e).into_iter().zip(corners) {
        let axis = if s.d[1] == 0 { "east_west" } else { "north_south" };
        let back = name([-s.d[0], -s.d[1]]);
        let fwd = name(s.d);
        let c = s.at(0, 0);
        out.push(([c[0], 0, c[1]], format!("rail[shape={corner},waterlogged=false]"), STONE));
        for i in 1..n {
            let p = s.at(i, 0);
            let (y, rail, support) = match i {
                1 => (0, format!("rail[shape={axis},waterlogged=false]"), STONE),
                ACT => (0, format!("activator_rail[powered=false,shape={axis},waterlogged=false]"), STONE),
                BRAKE => (-1, format!("powered_rail[powered=false,shape=ascending_{back},waterlogged=false]"), "mud"),
                BOOST => (-1, format!("powered_rail[powered=true,shape={axis},waterlogged=false]"), "redstone_block"),
                _ if i == n - 1 => (-1, format!("rail[shape=ascending_{fwd},waterlogged=false]"), STONE),
                _ => (-1, format!("rail[shape={axis},waterlogged=false]"), STONE),
            };
            out.push(([p[0], y, p[1]], rail, support));
        }
    }
    out
}

/// A stop's bed and its cage (side `s`; `outer`: the outer loop, nothing outside its track):
/// - the bed in the repeater lane, its foot beside the brake, its head beside the pocket: the
///   cart arriving over the powered activator drops its villager onto the foot (the dismount's
///   first free floor: every cell nearer is glass, covered or floorless);
/// - glass over the bus lane beside the bed and the activator (a walking villager has no headroom
///   there, a waking one no room to stand), 2 high in the repeater lane beside the activator and
///   beyond the head, over the rail before the activator (at y 1: carts and riders pass) and over
///   the booster, and 2 high outside (over the next loop's bus; the outer loop's from y -1): the
///   villager reaches only the bed, the brake's cell (its cart there), the pocket and the
///   activator's cell, so a HOME elsewhere is given up after 1200 gt for this bed; waking, it stands
///   up in the pocket (the brake's cell has no floor over its mud), where its cart picks it up;
/// - the activator's night power: a redstone wall torch over it on a cobblestone block in the
///   repeater lane (y 1, on the glass), a non-inverted daylight sensor beside that block's dust (y
///   2) keeping it off while its output is 1 or more (by day, in rain and thunder too).
fn stop(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>, s: Side, outer: bool) {
    let mut set = |p: [i32; 2], y: i32, block: &str| b.set(p[0], y, p[1], block);
    let facing = name(s.d);
    set(s.at(BRAKE, 1), -1, &format!("red_bed[facing={facing},occupied=false,part=foot]"));
    set(s.at(POCKET, 1), -1, &format!("red_bed[facing={facing},occupied=false,part=head]"));
    for i in ACT..=BOOST {
        set(s.at(i, 2), 0, GLASS);
    }
    for i in [ACT, BOOST] {
        for y in -1..=0 {
            set(s.at(i, 1), y, GLASS);
        }
    }
    set(s.at(1, 0), 1, GLASS);
    set(s.at(BOOST, 0), 0, GLASS);
    for i in 1..=BOOST {
        for y in (if outer { -1 } else { 0 })..=1 {
            set(s.at(i, -1), y, GLASS);
        }
    }
    let a = s.at(ACT, 1);
    set(a, 1, STONE);
    set(s.at(ACT, 0), 1, &format!("redstone_wall_torch[facing={},lit=true]", name([-s.n[0], -s.n[1]])));
    set(s.at(1, 1), 2, "daylight_detector[inverted=false,power=0]");
    dust.insert([a[0], 2, a[1]]);
}

/// Loop (t, e)'s pulse bus at y -1 in its bus lane (2 in from the track) along its north, east
/// and south sides from the station and up the west side's lane to its brake, with repeaters along
/// the line (a dust loses 1 a block); each stop's repeater under its bed's foot (y -2) reads the
/// bus's support beside it and powers the brake's mud.
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
    path.push([t + 2, y, e - 3]);
    let stops: Vec<[i32; 3]> = sides(t, e)
        .iter()
        .map(|s| {
            let p = s.at(BRAKE, 2);
            [p[0], y, p[1]]
        })
        .collect();
    for p in &stops {
        debug_assert!(path.contains(p), "the bus passes every stop's support");
    }
    // in-line repeaters: the first 6 after the entry (index 1), then every 13, never at a turn or
    // over a stop's support
    let turn = |i: usize| i == 0 || i + 1 >= path.len() || {
        let (a, c) = (path[i - 1], path[i + 1]);
        a[0] != c[0] && a[2] != c[2]
    };
    let mut next = 7usize;
    for i in 0..path.len() {
        let p = path[i];
        if i == next && !turn(i) && i + 2 < path.len() && !stops.contains(&p) {
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
    // the stops' repeaters (`facing` names the input side: the bus lane's), each on a block
    for s in sides(t, e) {
        let r = s.at(BRAKE, 1);
        b.set(r[0], -2, r[1], &format!("repeater[delay=1,facing={},locked=false,powered=false]", name(s.n)));
        b.set(r[0], -3, r[1], STONE);
    }
}

/// The night gate: the golem's pulse leaves the tripwire hook's block (3, -8, 5) through a
/// repeater (2, -8, 5) that a second repeater locks from its side while a redstone wall torch
/// burns (lit at night: a non-inverted daylight sensor's dust powers its block by day; the sensor
/// sees the sky through the pad's glass wall). A golem summoned at night (the villagers dropped
/// by the station stops still see the zombie) then advances nothing: an empty cart leaving its
/// brake would strand mid-side. The night's own pulses enter after the gate, at the stair's top.
fn night_gate(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>) {
    b.set(2, -8, 5, "repeater[delay=1,facing=east,locked=false,powered=false]");
    b.set(2, -9, 5, STONE);
    b.set(2, -8, 6, "repeater[delay=1,facing=south,locked=false,powered=false]");
    b.set(2, -9, 6, STONE);
    b.set(2, -8, 7, "redstone_wall_torch[facing=north,lit=true]");
    b.set(2, -8, 8, STONE);
    dust.insert([2, -7, 8]);
    b.set(2, -7, 7, "daylight_detector[inverted=false,power=0]");
}

/// The night's two pulses into the feed, each the rising edge of a daylight sensor's threshold (a
/// subtracting comparator reading a strongly powered block from behind, the same signal 4 gt later
/// in its side: one 4 gt pulse when the signal comes on, none when it goes off), both through a
/// dust inside loop 0 (x 4, south to north) down to a repeater into the stair's top (2, -3, 10),
/// after the night gate:
/// - at dusk, when a non-inverted sensor's output falls to 0 (day time about 13700; a redstone
///   wall torch the sensor's dust holds off by day lights): the one advance of the night, with
///   every activator on (the stops' torches light with it), which drops every villager;
/// - at dawn, when another's output reaches 8 (8 dusts, then a repeater; day time about 300, the
///   villagers up since 0 and standing in their pockets): the carts roll off their brakes and pick
///   them up. (An observer on a bed would also fire when its sleeper lies down, at night.)
fn night_pulse(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>) {
    let z0 = 24;
    // the line: from (4, -1, 19) north along x 4 (a repeater at z 18) down to the stair's top
    for z in (13..=z0 - 5).rev() {
        if z == z0 - 6 {
            b.set(4, -1, z, "repeater[delay=1,facing=south,locked=false,powered=false]");
            b.set(4, -2, z, STONE);
            continue;
        }
        dust.insert([4, -1, z]);
    }
    for p in [[4, -2, 12], [4, -3, 11], [4, -3, 10]] {
        dust.insert(p);
    }
    b.set(3, -3, 10, "repeater[delay=1,facing=east,locked=false,powered=false]");
    b.set(3, -4, 10, STONE);
    // the dawn kick (z 17..19, x 9..21): the sensor, 8 dusts west, a repeater into the block the
    // comparator reads, two repeaters around into its side; its dust joins the dusk drop's
    b.set(21, -1, z0 - 5, "daylight_detector[inverted=false,power=0]");
    for x in 13..=20 {
        dust.insert([x, -1, z0 - 5]);
    }
    b.set(12, -1, z0 - 5, "repeater[delay=1,facing=east,locked=false,powered=false]");
    b.set(12, -2, z0 - 5, STONE);
    b.set(11, -1, z0 - 5, STONE);
    b.set(10, -1, z0 - 5, "comparator[facing=east,mode=subtract,powered=false]");
    b.set(10, -2, z0 - 5, STONE);
    b.set(11, -1, z0 - 6, "repeater[delay=1,facing=south,locked=false,powered=false]");
    b.set(11, -2, z0 - 6, STONE);
    dust.insert([11, -1, z0 - 7]);
    dust.insert([10, -1, z0 - 7]);
    b.set(10, -1, z0 - 6, "repeater[delay=1,facing=north,locked=false,powered=false]");
    b.set(10, -2, z0 - 6, STONE);
    dust.insert([9, -1, z0 - 5]);
    // the dusk drop: the night signal's rising edge (z0 - 2 = 22 for side 28): a non-inverted
    // daylight sensor's dust powers a block holding a redstone wall torch by day; the torch, lit
    // at night, strongly powers the block over it, which a subtracting comparator reads from
    // behind and two repeaters feed into its side 4 gt later: one 4 gt pulse when the torch
    // lights, none when it goes out; the comparator's dust joins the line at z0 - 5
    let z = z0 - 2;
    b.set(11, -2, z, "daylight_detector[inverted=false,power=0]");
    dust.insert([10, -2, z]);
    b.set(9, -2, z, STONE);
    b.set(8, -2, z, "redstone_wall_torch[facing=west,lit=true]");
    b.set(8, -1, z, STONE);
    b.set(8, -1, z - 1, "comparator[facing=south,mode=subtract,powered=false]");
    b.set(8, -2, z - 1, STONE);
    b.set(7, -1, z, "repeater[delay=1,facing=east,locked=false,powered=false]");
    b.set(7, -2, z, STONE);
    dust.insert([6, -1, z]);
    dust.insert([6, -1, z - 1]);
    b.set(7, -1, z - 1, "repeater[delay=1,facing=west,locked=false,powered=false]");
    b.set(7, -2, z - 1, STONE);
    for p in [[8, -1, z - 2], [8, -1, z - 3], [7, -1, z - 3], [6, -1, z - 3], [5, -1, z - 3]] {
        dust.insert(p);
    }
}

/// Places the 12 carts with their villagers: one on each stop's brake, the stop's bed beside it
/// (the judge's arrival wants a bed within 2.5 of every villager; each claims its own).
pub fn carts(b: &mut Builder, side: i32) {
    for k in 0..3 {
        let (t, e) = (-3 * k, side + 3 * k);
        for s in sides(t, e) {
            let p = s.at(BRAKE, 0);
            // the cart on the slope's middle: half a block down
            b.entity("minecart", [p[0] as f64 + 0.5, -1.0 + 0.5 + 0.0625, p[1] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}"));
        }
    }
}

/// The station inside the inner loop's north-west corner: the pad (x 3..8, z 3..8, floor y 0)
/// under water from its 4 corners, walled in glass; its 2x2 hole (x 5..6, z 5..6) capped by wall
/// signs (dry: they stop the water, a golem falls through them); the shaft, a tripwire across it
/// at y -8 (its hooks at x 4 and 7, z 5: the hook's block (3, -8, 5) feeds the buses); the kill at
/// y -22 (the station's villagers ride at about y -0.5: 21 below, out of their follow box), a 2x2
/// of hoppers under open gates and lava; the chest. The zombie in a boat at (1.5, 3, -4.5), within
/// 8 of the 3 station carts and in their sight, 3 above them, roofed with leaves.
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
    // reach (a zombie's attack box grows only sideways), within 8 of the station's three (6.7, 5.7,
    // 7.5), in their sight over the north stops' cages (west of loop 0's station cart, along its
    // track's row; inside the outer loops' corners); leaves over it
    b.set(-2, 2, 0, GLASS);
    b.set(-2, 6, 0, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
    b.entity("oak_boat", [-1.5, 3.0, 0.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    // the night curtain: two oak trapdoors beside the zombie's eyes (east and north of it, where
    // every station villager's sight line passes), closed by day (flat at the bottom of their
    // cells: the lines pass over them) and open from evening to morning (upright: they hide the
    // zombie), so no villager panics at night (a panicking one never gives up a HOME it cannot
    // reach, never sleeps, and summons golems the parked empty carts would strand on): powered
    // by a redstone wall torch whose block a non-inverted daylight sensor's 8th dust powers while
    // the output is 8 or more (day time about 300 to 11,500)
    b.set(-1, 4, 0, "oak_trapdoor[facing=east,half=top,open=false,powered=false,waterlogged=false]");
    b.set(-2, 4, -1, "oak_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]");
    b.set(-1, 3, 0, "oak_trapdoor[facing=east,half=bottom,open=false,powered=false,waterlogged=false]");
    b.set(-2, 3, -1, "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]");
    b.set(-1, 3, -1, "redstone_wall_torch[facing=west,lit=true]");
    b.set(-1, 4, -1, STONE);
    b.set(0, 3, -1, STONE);
    // glass over the conductor and the top trapdoors (no golem spawns there, beside the zombie)
    for (x, z) in [(-1, -1), (-1, 0), (-2, -1)] {
        b.set(x, 5, z, GLASS);
    }
    b.set(0, 4, -1, GLASS);
    // the approach screen: glass north of the west sides' last rails (z 2, x -4..-1, y 2..4), so a
    // group still riding up a west side does not see the zombie (it would summon a golem whose
    // fall releases the carts before the others have braked)
    for x in -4..=-1 {
        for y in 2..=4 {
            b.set(x, y, 2, GLASS);
        }
    }
    b.set(9, 3, -1, "daylight_detector[inverted=false,power=0]");
}

/// The night curtain's dust: from its sensor (9, 3, -1) west, the 8th pointing into the torch's
/// block (0, 3, -1) (from the side: a dust on it would meet the block over the torch, which the
/// lit torch powers, and the torch would burn out).
fn curtain_dust(dust: &mut BTreeSet<[i32; 3]>) {
    for x in 1..=8 {
        dust.insert([x, 3, -1]);
    }
}

/// A dust's state from its neighbours (`RedStoneWireBlock.getConnectionState`): a side to dust
/// beside it (or one down past a non-solid cell, or one up under a non-solid cell), to a
/// repeater in line, to a signal source; one side runs through to the opposite one; none: a cross.
fn dust_state(blocks: &BTreeMap<[i32; 3], String>, dust: &BTreeSet<[i32; 3]>, p: [i32; 3]) -> String {
    // `isRedstoneConductor`: the full opaque blocks used here (not glass, leaves or a redstone block)
    let solid = |q: [i32; 3]| blocks.get(&q).is_some_and(|s| {
        let s = s.trim_start_matches("minecraft:");
        s.starts_with("stone") || s.starts_with("cobblestone")
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
        } else if s.starts_with("redstone_block") || s.starts_with("tripwire_hook") || s.starts_with("redstone_torch") || s.starts_with("daylight_detector") {
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
