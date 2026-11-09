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
//! strands mid-side; no empty cart travels here. From dusk to dawn an activator rail just before
//! each brake drops the arriving cart's villager onto the stop's bed and the cart stays parked on
//! the brake, empty, all night: the night's one advance is the dusk pulse that drops (the night
//! signal's rising edge, [`night_pulse`]), and the home phase's counter ([`phase_hold`]) holds the
//! evening's golems so that advance brings every villager to the bed it claimed. The villager
//! sleeps; at dawn it stands up in a pocket just past the brake, and the pulse of its bed's waking
//! (an observer on one bed) releases the carts, each picking its villager up as it rolls off its
//! brake. It holds 3 days on 2 seeds of 10: a cart braked too near its villager has no speed at
//! the contact, one braked too far steps over the pickup's 0.2 and stops against it (minecarts
//! collide with pushable mobs). (`carousel-free` runs the night instead: every brake powered from
//! the night threshold to its end, the carts round all night with a booster every 6 rails, the
//! activators dropping each villager at the first stop its cart reaches; without those boosters
//! every empty cart stranded on the first night and the carousel stood still while the judge
//! passed it, so the judge now counts the riders at each midday.)
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

/// (experiment, `MCTAS_BIDE`: the cart parks in the stand-up cell: the activator beside the bed's
/// foot (rail 3), the brake beside its head (rail 4, no pocket), so a waking villager stands up
/// inside its parked cart, which the release starts under it; not at the north stops, the
/// station, whose carts' sight of the zombie the moved torch would cut)
fn bide() -> bool {
    std::env::var_os("MCTAS_BIDE").is_some()
}

/// Whether stop side `s` is laid out for `MCTAS_BIDE` (every side but the north).
fn bide_at(s: &Side) -> bool {
    // (`MCTAS_BIDE_INNER`: the inner two loops only; the outer loop (t -6; e 34 at pitch 28),
    // unboosted, parks its carts too far from a villager standing past the brake to pick it up;
    // `MCTAS_BIDE_ALL`: the north stops too: the station carts' sight line runs west along loop
    // 0's track row, clear of an outside torch's stone)
    let outer = s.c[0] == -6 || s.c[1] == -6 || s.c == [34, 34];
    let north_ok = s.d != [1, 0] || std::env::var_os("MCTAS_BIDE_ALL").is_some();
    if std::env::var_os("MCTAS_BIDE_NORTH_ONLY").is_some() {
        return bide() && s.d == [1, 0];
    }
    bide() && north_ok && !(outer && std::env::var_os("MCTAS_BIDE_INNER").is_some())
}

/// The activator's rail (2; 3 under `MCTAS_BIDE`).
fn act_of(s: &Side) -> i32 {
    if bide_at(s) { 3 } else { ACT }
}

/// The brake's rail (3; 4 under `MCTAS_BIDE`).
fn brake_of(s: &Side) -> i32 {
    if bide_at(s) { 4 } else { BRAKE }
}
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
    carousel_with(side, true, false)
}

/// [`carousel`], its curtain latched (`latch`: shut from dusk to the dawn kick) or not (shut from
/// the evening threshold, output under 8, to the morning's), and its night run free (`free_run`:
/// every brake powered from the night threshold to its end, the carts running round all night,
/// each villager dropped at the first powered activator its cart reaches, the empty carts braking
/// one to a stop before the villagers wake) or stopped (one advance at the night threshold's edge,
/// the home phase's counter holding the evening's golems so it brings every villager home).
pub fn carousel_with(side: i32, latch: bool, free_run: bool) -> anyhow::Result<String> {
    carousel_impl(side, latch, free_run, false)
}

/// The brief's quick stop (vjeux 2026-11-09 06:35: "Fix the sleeping station"; work in progress,
/// [`quick_flash`]): every stop gets a dorm zombie behind a hatch that its bed's sleeper sees from
/// its sleeping eye, opened while the bed is OCCUPIED, so each villager lies down (LAST_SLEPT) and
/// is woken within a hostile scan instead of sleeping until dawn.
pub fn carousel_quick(side: i32) -> anyhow::Result<String> {
    carousel_impl(side, true, false, true)
}

fn carousel_impl(side: i32, latch: bool, free_run: bool, quick: bool) -> anyhow::Result<String> {
    anyhow::ensure!(side >= 28, "the carousel's inner loop needs a side of 28 or more (its waiting stops out of the golems' follow box)");
    let mut b = Builder::new(&format!("iron-carousel-3x4-{side}"));
    let mut dust: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut redstone_blocks: BTreeSet<[i32; 3]> = BTreeSet::new();
    for k in 0..3 {
        let (t, e) = (-3 * k, side + 3 * k);
        // (`MCTAS_BIDE_BOOST`: the outer loop boosted too, so its carts arrive and park like the
        // inner loops' at BIDE stops)
        let rails = loop_rails(t, e, k < 2 || std::env::var_os("MCTAS_BIDE_BOOST").is_some());
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
            if quick {
                quick_flash(&mut b, &mut dust, &s)?;
            }
            if std::env::var_os("MCTAS_WAKE_RELEASE").is_some() {
                wake_release(&mut b, &mut dust, &s)?;
            }
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
    if !free_run {
        phase_hold(&mut b, &mut dust);
    }
    night_pulse(&mut b, &mut dust, free_run);
    station(&mut b);
    curtain_latch(&mut b, &mut dust, latch);
    carts(&mut b, side);
    if free_run {
        night_boosters(&mut b, &dust, side);
    }
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
///
/// `boost`: the booster after each stop's pocket; the outer loop has none: its carts leave the
/// brake with the brake's own 8 gt of boost, which carries a ridden cart round its 40 rails, and
/// arrive slower, braking as far from the next pocket as the inner loops' boosted ones do
/// (boosted, they braked 0.84 from their villagers and met them with no speed: 2 seeds of 20 held
/// 3 days, 7 without).
fn loop_rails(t: i32, e: i32, boost: bool) -> Vec<([i32; 3], String, &'static str)> {
    let n = e - t;
    let corners = ["south_east", "south_west", "north_west", "north_east"];
    let mut out = Vec::new();
    for (s, corner) in sides(t, e).into_iter().zip(corners) {
        let (act, brake) = (act_of(&s), brake_of(&s));
        let axis = if s.d[1] == 0 { "east_west" } else { "north_south" };
        let back = name([-s.d[0], -s.d[1]]);
        let fwd = name(s.d);
        let c = s.at(0, 0);
        out.push(([c[0], 0, c[1]], format!("rail[shape={corner},waterlogged=false]"), STONE));
        for i in 1..n {
            let p = s.at(i, 0);
            let (y, rail, support) = match i {
                _ if i < act => (0, format!("rail[shape={axis},waterlogged=false]"), STONE),
                _ if i == act => (0, format!("activator_rail[powered=false,shape={axis},waterlogged=false]"), STONE),
                _ if i == brake => (-1, format!("powered_rail[powered=false,shape=ascending_{back},waterlogged=false]"), "mud"),
                // (under `MCTAS_BIDE` one rail further: beside the brake it would power it through
                // the powered rails' own line)
                _ if boost && i == if bide_at(&s) { BOOST + 1 } else { BOOST } => (-1, format!("powered_rail[powered=true,shape={axis},waterlogged=false]"), "redstone_block"),
                _ if i == n - 1 => (-1, format!("rail[shape=ascending_{fwd},waterlogged=false]"), STONE),
                _ => (-1, format!("rail[shape={axis},waterlogged=false]"), STONE),
            };
            out.push(([p[0], y, p[1]], rail, support));
        }
    }
    out
}

/// The quick stop's flash at one stop: a dorm zombie over the cage's lane-1 barrier glass at the
/// booster's column (lane 1, y 1..2, BOOST; ironfarm's sight line: the sleeping eye, the head's
/// centre at y -0.11, sees its eye at y 2.74 through the air over the head), walled (stone at y 1,
/// glass at y 2 on the track and bus sides: no golem spawns on glass; the latch tower on the far
/// side), roofed (stone), stone over the hatch cell (the zombie has no headroom into the pocket's
/// column); the hatch an iron trapdoor (lane 1, y 1, POCKET), closed a bottom plate across the
/// line. The latch: an observer under the bed's head (y -2, facing up) pulses when the sleeper lies
/// down and when it wakes; a copper bulb under it toggles (lit exactly while the bed is OCCUPIED);
/// a comparator beside the bulb (BOOST, y -3) feeds a torch tower one rail further (stone -3,
/// torch -2, stone -1, torch 0, stone 1, torch 2, stone 3, torch 4: the bulb's signal 4 inversions
/// up), the top torch powers a dust on the zombie's roof and down onto the stone over the hatch,
/// which opens it. The bus beside the tower's stone at y -1 gets a repeater there (a powered block
/// beside a repeater does not feed it). Returns an error where the tower meets another block.
fn quick_flash(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>, s: &Side) -> anyhow::Result<()> {
    let z = s.at(BOOST, 1);
    if std::env::var_os("MCTAS_QUICK_OPEN").is_some() {
        // (experiment: no hatch; the zombie seen only from the sleeping eye) the dorm zombie over
        // the barrier glass at the booster's column, held to the cell's far half by an open oak
        // trapdoor at its head's height on the pocket's face (its centre 0.99-1.2 rails past the
        // head's: the sleeping eye's line crosses into its column at y 1.08-1.33); a top slab over
        // the hatch cell (lane 1, y 1, POCKET: y 1.5-2): the line passes under it, but a villager
        // cannot stand on the bed's head (its top at 1.51) and no line from the zombie reaches the
        // foot, the pocket or the head standing; glass on every top a golem could spawn on
        for lane in [0, 2] {
            let p = s.at(BOOST, lane);
            b.set(p[0], 1, p[1], STONE);
            b.set(p[0], 2, p[1], GLASS);
        }
        let f = s.at(BOOST + 1, 1);
        b.set(f[0], 1, f[1], STONE);
        b.set(f[0], 2, f[1], GLASS);
        b.set(z[0], 2, z[1], &format!("oak_trapdoor[facing={},half=bottom,open=true,powered=false,waterlogged=false]", name(s.d)));
        b.set(z[0], 3, z[1], STONE);
        b.set(z[0], 4, z[1], GLASS);
        let q = s.at(POCKET, 1);
        b.set(q[0], 1, q[1], "cobblestone_slab[type=top,waterlogged=false]");
        b.set(q[0], 2, q[1], STONE);
        b.set(q[0], 3, q[1], GLASS);
        b.entity("zombie", [z[0] as f64 + 0.5 + 0.15 * s.d[0] as f64, 1.0, z[1] as f64 + 0.5 + 0.15 * s.d[1] as f64], Some("{PersistenceRequired:1b}"));
        let _ = dust;
        return Ok(());
    }
    for lane in [0, 2] {
        let p = s.at(BOOST, lane);
        b.set(p[0], 1, p[1], STONE);
        b.set(p[0], 2, p[1], GLASS);
    }
    b.set(z[0], 3, z[1], STONE);
    let q = s.at(POCKET, 1);
    b.set(q[0], 2, q[1], STONE);
    b.set(q[0], 1, q[1], &format!("iron_trapdoor[facing={},half=bottom,open=false,powered=false,waterlogged=false]", name(s.d)));
    b.entity("zombie", [z[0] as f64 + 0.5, 1.0, z[1] as f64 + 0.5], Some("{PersistenceRequired:1b}"));
    // the latch
    let o = s.at(POCKET, 1);
    b.set(o[0], -2, o[1], "observer[facing=up,powered=false]");
    b.set(o[0], -3, o[1], "copper_bulb[lit=false,powered=false]");
    b.set(o[0], -4, o[1], STONE);
    // (`facing` names the input side: the bulb's)
    b.set(z[0], -3, z[1], &format!("comparator[facing={},mode=compare,powered=false]", name([-s.d[0], -s.d[1]])));
    b.set(z[0], -4, z[1], STONE);
    let t = s.at(BOOST + 1, 1);
    for (y, block) in [(-3, STONE), (-2, "redstone_torch[lit=true]"), (-1, STONE), (0, "redstone_torch[lit=false]"), (1, STONE), (2, "redstone_torch[lit=true]"), (3, STONE), (4, "redstone_torch[lit=false]")] {
        anyhow::ensure!(!b.d.blocks.contains_key(&[t[0], y, t[1]]) && !dust.contains(&[t[0], y, t[1]]), "the quick stop's latch tower at {:?} meets {:?}", [t[0], y, t[1]], b.d.blocks.get(&[t[0], y, t[1]]));
        b.set(t[0], y, t[1], block);
    }
    dust.insert([z[0], 4, z[1]]);
    dust.insert([q[0], 3, q[1]]);
    // the bus beside the tower's stone at y -1 (powered while the bulb is out): a repeater there
    let bp = s.at(BOOST + 1, 2);
    if dust.remove(&[bp[0], -1, bp[1]]) {
        b.set(bp[0], -1, bp[1], &format!("repeater[delay=1,facing={},locked=false,powered=false]", name([-s.d[0], -s.d[1]])));
        if !b.d.blocks.contains_key(&[bp[0], -2, bp[1]]) {
            b.set(bp[0], -2, bp[1], STONE);
        }
    }
    Ok(())
}

/// The per-stop wake release (experiment, `MCTAS_WAKE_RELEASE`; ironfarm's next step 1: "a
/// per-stop wake release" against the dawn stroll-push): the stop's own cart leaves its brake when
/// its own sleeper wakes, a few ticks after it stands up in the pocket, before it strolls back
/// into the parked cart. An observer under the bed's head (lane 1, y -2, facing up) pulses into a
/// stone under it (y -3) at each lie-down and wake; a copper bulb under that (y -4), placed lit,
/// toggles on each pulse: lit exactly while the bed is free. A comparator beside the bulb (BRAKE,
/// y -4) reads it into a block (ACT, y -4); a subtracting comparator in front of that block (rail
/// 1, y -4) reads it from behind and the same signal 4 gt later in its side (two repeaters around
/// through lanes 2 and 3): one 4 gt pulse when the bed frees (the wake), none when it fills. The
/// pulse's block (rail 0, lane 1, y -4) carries a dust up onto the track column (y -2, rails 0 and
/// 1) into a repeater (ACT, y -2) into the brake's mud.
fn wake_release(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>, s: &Side) -> anyhow::Result<()> {
    let at = |i: i32, off: i32, y: i32| {
        let p = s.at(i, off);
        [p[0], y, p[1]]
    };
    let from_plus_d = name(s.d);
    let from_minus_d = name([-s.d[0], -s.d[1]]);
    let from_plus_n = name(s.n);
    let from_minus_n = name([-s.n[0], -s.n[1]]);
    let release_delay = std::env::var("MCTAS_WAKE_DELAY").ok().and_then(|v| v.parse::<u8>().ok()).unwrap_or(4).clamp(1, 4);
    if std::env::var_os("MCTAS_WAKE_FAST").is_some() {
        // (experiment: 2 gt sooner, the bulb read straight by the edge comparator: the
        // subtracting comparator beside the bulb (BRAKE, y -4) reads it from behind, the same
        // signal 4 gt later in its side (a comparator under the pocket (track, y -4) reading the
        // bulb, a dust outside and a repeater under the brake back into the side); its block
        // (ACT, y -4) carries a dust along lane 1 (y -3) to the corner and up into the repeater)
        let blocks: Vec<([i32; 3], String)> = vec![
            (at(POCKET, 1, -2), "observer[facing=up,powered=false]".into()),
            (at(POCKET, 1, -3), STONE.into()),
            (at(POCKET, 1, -4), "copper_bulb[lit=true,powered=false]".into()),
            (at(POCKET, 1, -5), STONE.into()),
            (at(BRAKE, 1, -4), format!("comparator[facing={from_plus_d},mode=subtract,powered=false]")),
            (at(BRAKE, 1, -5), STONE.into()),
            (at(ACT, 1, -4), STONE.into()),
            (at(1, 1, -4), STONE.into()),
            (at(0, 1, -4), STONE.into()),
            (at(POCKET, 0, -4), format!("comparator[facing={from_plus_n},mode=compare,powered=true]")),
            (at(POCKET, 0, -5), STONE.into()),
            (at(POCKET, -1, -5), STONE.into()),
            (at(BRAKE, -1, -5), STONE.into()),
            (at(BRAKE, 0, -4), format!("repeater[delay=1,facing={from_minus_n},locked=false,powered=true]")),
            (at(BRAKE, 0, -5), STONE.into()),
            (at(0, 0, -4), STONE.into()),
            (at(-1, 0, -3), STONE.into()),
            (at(-1, 0, -2), GLASS.into()),
            (at(1, 0, -3), STONE.into()),
            (at(ACT, 0, -2), format!("repeater[delay={release_delay},facing={from_minus_d},locked=false,powered=false]")),
            (at(ACT, 0, -3), STONE.into()),
        ];
        let dusts = [at(POCKET, -1, -4), at(BRAKE, -1, -4), at(ACT, 1, -3), at(1, 1, -3), at(0, 1, -3), at(0, 0, -3), at(1, 0, -2)];
        for (p, _) in &blocks {
            anyhow::ensure!(!b.d.blocks.contains_key(p) && !dust.contains(p), "the wake release at {p:?} meets {:?}", b.d.blocks.get(p));
        }
        for p in &dusts {
            anyhow::ensure!(!b.d.blocks.contains_key(p) && !dust.contains(p), "the wake release's dust at {p:?} meets {:?}", b.d.blocks.get(p));
        }
        for (p, block) in blocks {
            b.set(p[0], p[1], p[2], &block);
        }
        for p in dusts {
            dust.insert(p);
        }
        return Ok(());
    }
    let blocks: Vec<([i32; 3], String)> = vec![
        (at(POCKET, 1, -2), "observer[facing=up,powered=false]".into()),
        (at(POCKET, 1, -3), STONE.into()),
        (at(POCKET, 1, -4), "copper_bulb[lit=true,powered=false]".into()),
        (at(POCKET, 1, -5), STONE.into()),
        (at(BRAKE, 1, -4), format!("comparator[facing={from_plus_d},mode=compare,powered=true]")),
        (at(BRAKE, 1, -5), STONE.into()),
        (at(ACT, 1, -4), STONE.into()),
        (at(1, 1, -4), format!("comparator[facing={from_plus_d},mode=subtract,powered=false]")),
        (at(1, 1, -5), STONE.into()),
        (at(0, 1, -4), STONE.into()),
        (at(ACT, 2, -4), format!("repeater[delay=1,facing={from_minus_n},locked=false,powered=true]")),
        (at(ACT, 2, -5), STONE.into()),
        (at(ACT, 3, -5), STONE.into()),
        (at(1, 3, -5), STONE.into()),
        (at(1, 2, -4), format!("repeater[delay=1,facing={from_plus_n},locked=false,powered=true]")),
        (at(1, 2, -5), STONE.into()),
        (at(0, 0, -4), STONE.into()),
        // (beside the corner's dust, beyond it: the next loop's repeater path (its lane 3, y -4)
        // runs under that cell and the corner's dust would step down into it)
        (at(-1, 0, -3), STONE.into()),
        // (and glass over it: its top would be a golem spawn spot in the station's search)
        (at(-1, 0, -2), GLASS.into()),
        (at(1, 0, -3), STONE.into()),
        (at(ACT, 0, -2), format!("repeater[delay={},facing={from_minus_d},locked=false,powered=false]", std::env::var("MCTAS_WAKE_DELAY").ok().and_then(|v| v.parse::<u8>().ok()).unwrap_or(4).clamp(1, 4))),
        (at(ACT, 0, -3), STONE.into()),
    ];
    // (the pulse's dust: on its block (lane 1, rail 0, y -3), into the track column at the
    // corner (y -3: at y -2 the previous side's climb rail's support stands over lane 1's rail 0),
    // up onto rail 1 (y -2) into the repeater)
    let dusts = [at(ACT, 3, -4), at(1, 3, -4), at(0, 1, -3), at(0, 0, -3), at(1, 0, -2)];
    for (p, _) in &blocks {
        anyhow::ensure!(!b.d.blocks.contains_key(p) && !dust.contains(p), "the wake release at {p:?} meets {:?}", b.d.blocks.get(p));
    }
    for p in &dusts {
        anyhow::ensure!(!b.d.blocks.contains_key(p) && !dust.contains(p), "the wake release's dust at {p:?} meets {:?}", b.d.blocks.get(p));
    }
    for (p, block) in blocks {
        b.set(p[0], p[1], p[2], &block);
    }
    for p in dusts {
        dust.insert(p);
    }
    Ok(())
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
    // (experiment, `MCTAS_SHIFTBED`: the rails as they are, the bed one rail back: its foot
    // beside the activator, its head beside the brake, so the head's side cell is the parked
    // cart's and the waking villager stands up in the pocket diagonal past the head; not at the
    // north stops (the station's sight line), the activator's torch outside as under BIDE)
    let shift = std::env::var_os("MCTAS_SHIFTBED").is_some() && s.d != [1, 0];
    let mut set = |p: [i32; 2], y: i32, block: &str| b.set(p[0], y, p[1], block);
    let facing = name(s.d);
    let (foot, head) = if shift { (ACT, BRAKE) } else { (BRAKE, POCKET) };
    set(s.at(foot, 1), -1, &format!("red_bed[facing={facing},occupied=false,part=foot]"));
    set(s.at(head, 1), -1, &format!("red_bed[facing={facing},occupied=false,part=head]"));
    for i in ACT..=BOOST {
        set(s.at(i, 2), 0, GLASS);
    }
    for i in [foot - 1, head + 1] {
        for y in -1..=0 {
            set(s.at(i, 1), y, GLASS);
        }
    }
    set(s.at(1, 0), 1, GLASS);
    if shift {
        set(s.at(BOOST, 0), 0, GLASS);
        for i in 1..=BOOST {
            for y in (if outer { -1 } else { 0 })..=1 {
                set(s.at(i, -1), y, GLASS);
            }
        }
        let o = s.at(ACT, -1);
        set(o, 1, STONE);
        set(s.at(ACT, 0), 1, &format!("redstone_wall_torch[facing={},lit=true]", name(s.n)));
        set(s.at(1, -1), 2, "daylight_detector[inverted=false,power=0]");
        dust.insert([o[0], 2, o[1]]);
        return;
    }
    // (under `MCTAS_BIDE` one rail further: the brake's slope ends at rail 5, and the cart
    // leaving it still stands into y 0)
    set(s.at(if bide_at(&s) { BOOST + 1 } else { BOOST }, 0), 0, GLASS);
    if bide_at(&s) && std::env::var_os("MCTAS_BIDE_SLAB").is_some() {
        // (experiment: a top slab over rail 5 (y 0.5-1: the cart leaving the slope passes under,
        // riders' heads do not suffocate in a slab) so the waking villager cannot stand there:
        // it stands further on, giving the released cart a longer run to it)
        set(s.at(BOOST, 0), 0, "smooth_stone_slab[type=top,waterlogged=false]");
    }
    for i in 1..=BOOST {
        for y in (if outer { -1 } else { 0 })..=1 {
            set(s.at(i, -1), y, GLASS);
        }
    }
    let a = s.at(ACT, 1);
    if bide_at(&s) {
        // (glass over the lane-1 barrier before the foot (y 1): the base's torch stone stood
        // there; without it a villager on the track at y 0 climbs that barrier, the glass over
        // rail 1 and the walls out of its cage: the falls, the cross-loop riders)
        set(s.at(ACT, 1), 1, GLASS);
        // the activator beside the foot: its torch over it hangs on a stone in the outside lane
        // (y 1; over the foot it would cut the dropped villager's headroom, over the rail before
        // it the riders' heads: they pass at y 0 with their eyes over 1), that stone's dust and
        // sensor outside too
        if std::env::var_os("MCTAS_BIDE_NO_TORCH").is_some() {
            // (diagnostic: no activator power at all)
            return;
        }
        let o = s.at(3, -1);
        if s.d == [1, 0] {
            // (the north stops: the station's curtain latch runs over the outside lane at y 2-3
            // (its repeaters' stones at y 2, its dust at y 3), so the stone is fed from the side
            // at y 1: a dust on the outside glass beside it, the sensor beyond)
            set(o, 1, STONE);
            // (glass over it: its top would be a golem spawn spot in the station's search; the
            // curtain's dust over the outside lane rests there at the first north stop (a slab
            // instead kills the station: 2 golems on seed 9's day 1); with the glass, some
            // villagers walk the curtain's line at y 3 and fall: untraced)
            set(o, 2, GLASS);
            set(s.at(3, 0), 1, &format!("redstone_wall_torch[facing={},lit=true]", name(s.n)));
            set(s.at(5, -1), 1, "daylight_detector[inverted=false,power=0]");
            let w = s.at(4, -1);
            dust.insert([w[0], 1, w[1]]);
            return;
        }
        set(o, 1, STONE);
        set(s.at(3, 0), 1, &format!("redstone_wall_torch[facing={},lit=true]", name(s.n)));
        set(s.at(2, -1), 2, "daylight_detector[inverted=false,power=0]");
        dust.insert([o[0], 2, o[1]]);
        return;
    }
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
    if bide_at(&sides(t, e)[3]) {
        // (the west stop's brake one rail on)
        path.push([t + 2, y, e - 4]);
    }
    let stops: Vec<[i32; 3]> = sides(t, e)
        .iter()
        .map(|s| {
            let p = s.at(brake_of(s), 2);
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
        let r = s.at(brake_of(&s), 1);
        b.set(r[0], -2, r[1], &format!("repeater[delay=1,facing={},locked=false,powered=false]", name(s.n)));
        b.set(r[0], -3, r[1], STONE);
    }
}

/// The night gate: the golem's pulse leaves the tripwire hook's block (3, -8, 5) through a
/// repeater (2, -8, 5) that a second repeater locks from its side while a redstone wall torch
/// burns (lit from the evening's output under 2, day time about 13,450, to the morning: a
/// non-inverted daylight sensor's 2nd dust powers its block; the sensor sees the sky through the
/// pad's glass wall). The last golem advance of the day thus ends before the dusk drop's (output
/// 0) pulse, and a golem summoned at night advances nothing (an empty cart leaving its brake
/// would strand mid-side). The night's own pulses enter after the gate, at the stair's top.
fn night_gate(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>) {
    b.set(2, -8, 5, "repeater[delay=1,facing=east,locked=false,powered=false]");
    b.set(2, -9, 5, STONE);
    b.set(2, -8, 6, "repeater[delay=1,facing=south,locked=false,powered=false]");
    b.set(2, -9, 6, STONE);
    b.set(2, -8, 7, "redstone_wall_torch[facing=north,lit=true]");
    b.set(2, -8, 8, STONE);
    dust.insert([2, -7, 8]);
    dust.insert([2, -6, 9]);
    b.set(3, -6, 9, "daylight_detector[inverted=false,power=0]");
}

/// The home phase (ironspec 2026-11-09): every villager claims the bed of the stop its cart
/// starts at, and an advance moves every cart one stop, so the dusk drop lands every villager on
/// its own bed when the advances so far are 3 short of a multiple of 4 before it. A 2-bit counter
/// of copper bulbs counts every release (A under the stair's first dust (1, -8, 5), which every
/// pulse reaches: a golem's through the gate, the night's down from the stair's top; B toggled by
/// A going out, through a comparator and a torch); from the evening threshold (a daylight sensor's
/// output under 8) a torch burns while A and B are lit (the count at 3), pressing a repeater on the
/// gate's other side: the golems' advances stop there until the dusk drop, which brings every cart
/// home. (Its villagers then never hold a HOME at another stop, never walk off their beds, and
/// the wake kick's bed is always slept in.)
fn phase_hold(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>) {
    b.set(1, -9, 5, "copper_bulb[lit=false,powered=false]");
    b.set(0, -9, 5, "comparator[facing=east,mode=compare,powered=false]");
    b.set(0, -10, 5, STONE);
    b.set(-1, -9, 5, STONE);
    b.set(-2, -9, 5, "redstone_wall_torch[facing=west,lit=true]");
    b.set(-2, -8, 5, "copper_bulb[lit=false,powered=true]");
    b.set(-3, -8, 5, "comparator[facing=east,mode=compare,powered=false]");
    b.set(-3, -9, 5, STONE);
    b.set(-4, -8, 5, STONE);
    b.set(-5, -8, 5, "redstone_wall_torch[facing=west,lit=true]");
    // glass over B and its comparator's block (y -8: their tops are in the station's golem search)
    b.set(-2, -7, 5, GLASS);
    b.set(-4, -7, 5, GLASS);
    // the NOR block (-5, -9, 4): not-B's dust on it, not-A's line and D8's 8th dust into it
    b.set(-5, -9, 4, STONE);
    dust.insert([-5, -8, 4]);
    for x in [-2, -3, -4] {
        dust.insert([x, -9, 4]);
    }
    b.set(-7, -9, 11, "daylight_detector[inverted=false,power=0]");
    for z in 4..=10 {
        dust.insert([-7, -9, z]);
    }
    dust.insert([-6, -9, 4]);
    // its torch, lit at the count of 3 in the evening, and its line to the gate's lock: 16 gt of
    // repeaters on the way (a repeater locked while on stays on: the count reaches 3 on a golem's
    // own pulse, which must have left the gate before the lock presses it)
    b.set(-5, -9, 3, "redstone_wall_torch[facing=north,lit=true]");
    for p in [[-5, -9, 2], [-4, -9, 2], [-3, -9, 2], [-2, -9, 2], [0, -9, 2], [1, -9, 2], [1, -9, 3], [2, -8, 3]] {
        dust.insert(p);
    }
    b.set(-1, -9, 2, "repeater[delay=4,facing=west,locked=false,powered=false]");
    b.set(-1, -10, 2, STONE);
    b.set(2, -8, 4, "repeater[delay=4,facing=north,locked=false,powered=false]");
    b.set(2, -9, 4, STONE);
}

/// The running night's boosters: an empty cart slows by 0.96 a tick, so it coasts about 10 rails
/// from a booster's 0.4 and strands; on the running night every cart is empty from its drop to
/// dawn, so each side gets a booster every 6 rails after its first (the brake powered at night is
/// the next stop's), the last 4 rails before the climb. A booster whose redstone block would touch
/// anything but inert blocks (a dust, a repeater: the feed, the basement, the night's circuits) is
/// left out.
fn night_boosters(b: &mut Builder, dust: &BTreeSet<[i32; 3]>, side: i32) {
    const INERT: [&str; 4] = [STONE, GLASS, "mud", "redstone_block"];
    for k in 0..3 {
        let (t, e) = (-3 * k, side + 3 * k);
        let n = e - t;
        for s in sides(t, e) {
            let axis = if s.d[1] == 0 { "east_west" } else { "north_south" };
            let mut i = BOOST + 6;
            while i <= n - 2 {
                let p = s.at(i, 0);
                let q = [p[0], -2, p[1]];
                let inert = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]].iter().all(|d| {
                    let r = [q[0] + d[0], q[1] + d[1], q[2] + d[2]];
                    !dust.contains(&r) && b.d.blocks.get(&r).is_none_or(|name| INERT.iter().any(|x| name.trim_start_matches("minecraft:") == *x))
                });
                if inert {
                    b.set(p[0], -1, p[1], &format!("powered_rail[powered=true,shape={axis},waterlogged=false]"));
                    b.set(q[0], q[1], q[2], "redstone_block");
                }
                i += 6;
            }
        }
    }
}

/// The night's signals into the feed, through a dust inside loop 0 (x 4, south to north) down to a
/// repeater into the stair's top (2, -3, 10), after the night gate:
/// - the night: a non-inverted sensor's dust holds a redstone wall torch off by day; lit from its
///   output's fall to 0 (day time about 13700, with every stop's activator torch) to its rise
///   (about 22700, before the villagers wake). `free_run`: its level, every brake powered all night
///   (the carts run round, the activators drop every villager); otherwise its rising edge (a
///   subtracting comparator reading the torch's block from behind, the same signal 4 gt later in
///   its side: one 4 gt pulse when it lights, none when it goes out), the night's one advance;
/// - at dawn, the wake kick (below): the carts roll off their brakes and pick their villagers up.
fn night_pulse(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>, free_run: bool) {
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
    // the dawn kick's pulse maker (z 17..19, x 9..12): the block the comparator reads, two
    // repeaters around into its side; its dust joins the dusk drop's. (No daylight edge drives it:
    // a sensor placed at the judge's arrival rises from 0 and kicked the carts at the start, and
    // the villagers claimed HOMEs at two stops; the wake kick below is its only input.)
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
    // the wake kick: an observer beside loop 0's west stop's bed head (in the bus lane's air at
    // (2, -1, 24)) fires when its sleeper wakes (day time 0, before the villagers stroll off their
    // pockets), east along z 24 to a repeater K (8, -1, 24) that the night locks (a repeater from
    // the dusk drop's block over its torch, lit at night: the observer also fires when the
    // sleeper lies down), then around to a repeater into the dawn kick's block (11, -1, 19): the
    // comparator there makes it a pulse
    if bide() {
        // (BIDE: the west stop's bus reaches its brake beside the head, where the observer
        // stood; an observer over the head would obstruct the bed (no sleeping under a solid
        // block). The observer takes the lane-1 barrier past the head (rail 5, z0 - 1), facing
        // the head, its pulse north into a dust (rail 6), east along z0 - 2 and south onto the
        // line's first dust)
        b.set(1, -1, z0 - 1, "observer[facing=south,powered=false]");
        for p in [[1, -1, z0 - 2], [2, -1, z0 - 2], [3, -1, z0 - 2], [3, -1, z0 - 1]] {
            dust.insert(p);
            b.set(p[0], p[1] - 1, p[2], STONE);
        }
        for x in 3..=7 {
            dust.insert([x, -1, z0]);
        }
    } else if std::env::var_os("MCTAS_SHIFTBED").is_some() {
        // (the west stop's head where the observer stood: the observer watches the head from
        // over it (facing down), its stone over it feeds a dust on the bus lane's glass, down
        // onto the line's first block)
        let hz = z0 + 1;
        b.set(1, 0, hz, "observer[facing=down,powered=false]");
        b.set(1, 1, hz, STONE);
        // (stone, not the stop's glass, under the dust: a non-conductor there would join the bus's
        // end under it to the dust stepping down past it)
        b.set(2, 0, hz, STONE);
        dust.insert([2, 1, hz]);
        dust.insert([3, 0, hz]);
        b.set(3, -1, hz, STONE);
        for x in 3..=7 {
            dust.insert([x, -1, z0]);
        }
    } else {
        b.set(2, -1, z0, if std::env::var_os("MCTAS_NO_KICK").is_some() { GLASS } else { "observer[facing=west,powered=false]" });
        for x in 3..=7 {
            dust.insert([x, -1, z0]);
        }
    }
    b.set(8, -1, z0, "repeater[delay=4,facing=west,locked=false,powered=false]");
    b.set(8, -2, z0, STONE);
    b.set(8, -1, z0 - 1, "repeater[delay=1,facing=north,locked=false,powered=false]");
    b.set(8, -2, z0 - 1, STONE);
    for p in [[9, -1, z0], [10, -1, z0], [11, -1, z0], [12, -1, z0], [12, -1, z0 - 1], [12, -1, z0 - 2], [12, -1, z0 - 3], [11, -1, z0 - 3]] {
        dust.insert(p);
    }
    b.set(11, -1, z0 - 4, "repeater[delay=1,facing=south,locked=false,powered=false]");
    b.set(11, -2, z0 - 4, STONE);
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
    if free_run {
        // the night's level, not its edge: a repeater from the torch's block into the line
        b.set(8, -1, z - 1, "repeater[delay=1,facing=south,locked=false,powered=true]");
        b.set(8, -2, z - 1, STONE);
    } else {
        b.set(8, -1, z - 1, "comparator[facing=south,mode=subtract,powered=false]");
        b.set(8, -2, z - 1, STONE);
        // (both repeaters placed powered, as the torch's lit start makes them: unpowered, the
        // comparator's side lagged its back by 4 gt through the settle, and its pulse counted a
        // release on the home phase's counter before the first cart moved)
        b.set(7, -1, z, "repeater[delay=1,facing=east,locked=false,powered=true]");
        b.set(7, -2, z, STONE);
        dust.insert([6, -1, z]);
        dust.insert([6, -1, z - 1]);
        b.set(7, -1, z - 1, "repeater[delay=1,facing=west,locked=false,powered=true]");
        b.set(7, -2, z - 1, STONE);
    }
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
            let p = s.at(brake_of(&s), 0);
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
    // by a redstone wall torch whose block the curtain's latch powers by day and through the
    // evening ([`curtain_latch`]: shut from dusk to the dawn kick)
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
}

/// The curtain's latch (ironspec 2026-11-09: shut only from dusk to the dawn kick): the curtain
/// torch's block (0, 3, -1) is powered (the curtain open) while a memory repeater M (1, 3, -1)
/// is on. M's input is D8 (a non-inverted daylight sensor's output 8 or more: its 8th dust at
/// (3, 3, -1), then a 4-tick repeater); a lock repeater L presses on M's side while a torch on
/// the NOR block (4, 3, -3) burns, that is while neither D8 (a branch dust from the line's 7th)
/// nor the night (a torch under the NOR block, lit while another sensor's output is 0) powers
/// it. So M keeps D8's last value from the evening (on: the curtain stays open, the station
/// works until dusk) and the night's (off: it stays shut until the dawn kick's D8), weather
/// included (a storm by day locks M on). The lock comes 4 gt after D8 falls, the 4-tick
/// repeater's 8 gt before M's input does.
fn curtain_latch(b: &mut Builder, dust: &mut BTreeSet<[i32; 3]>, latch: bool) {
    b.set(11, 3, -1, "daylight_detector[inverted=false,power=0]");
    for x in 3..=10 {
        dust.insert([x, 3, -1]);
    }
    b.set(2, 3, -1, "repeater[delay=4,facing=east,locked=false,powered=false]");
    b.set(2, 2, -1, STONE);
    b.set(1, 3, -1, "repeater[delay=1,facing=east,locked=false,powered=false]");
    b.set(1, 2, -1, STONE);
    if !latch {
        return;
    }
    // the NOR block: D8 through a repeater from the line's 8th dust (one way: the night torch
    // strongly powers the block, which would feed a dust back into the line), the night torch
    // under it, its torch to L
    b.set(3, 3, -2, "repeater[delay=1,facing=south,locked=false,powered=false]");
    b.set(3, 2, -2, STONE);
    b.set(3, 3, -3, STONE);
    b.set(3, 4, -3, GLASS);
    b.set(3, 3, -4, "redstone_wall_torch[facing=north,lit=true]");
    for p in [[2, 3, -4], [1, 3, -4], [1, 3, -3]] {
        dust.insert(p);
    }
    b.set(1, 3, -2, "repeater[delay=1,facing=north,locked=false,powered=false]");
    b.set(1, 2, -2, STONE);
    // the night: a torch under the NOR block on a block a sensor's dust holds off by day (glass
    // under that dust: it hangs over loop 1's track, where a rider's eyes pass)
    b.set(3, 2, -3, "redstone_torch[lit=true]");
    b.set(3, 1, -3, STONE);
    dust.insert([4, 1, -3]);
    b.set(4, 0, -3, GLASS);
    b.set(5, 1, -3, "daylight_detector[inverted=false,power=0]");
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
