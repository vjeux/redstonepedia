//! Day-1 iron farm candidates (revision 5 of the brief: no quartz, at most 10 iron with the
//! tools, at most 1 lava source, 3 villagers and their beds from a village, a night calm that does
//! not come from golems). Coordinates as in `layouts`: bed heads at y = 0, z = 0, north is -z.
//!
//! The calm here is the bed plate: of the three villagers only S sees the zombie (layout L6); U1
//! and U2 never see it, so they go to bed at every REST on their own. U1 stands by day on a wooden
//! pressure plate next to its bed; the plate powers the stone under it, and that powers the
//! trapdoor in S's line of sight, which then stands open. At nightfall U1 lies down in its bed,
//! the plate lets go, the trapdoor shuts across S's line of sight, S calms down and sleeps; at
//! dawn U1 stands up on the plate and the zombie shows again.

use crate::layouts::Builder;

const STONE: &str = "stone";
const GLASS: &str = "glass";

/// A villager on the head of its bed (facing north) in a closed 1-cell glass pod at (x, 0, 0), a
/// glass roof; `sees`: its north face at y + 1 is an open trapdoor (it sees over it, it cannot
/// pass), else glass up to its eyes (it never sees north).
fn pod(b: &mut Builder, x: i32, sees: bool) {
    b.set(x, 0, 0, "red_bed[facing=north,occupied=false,part=head]");
    b.set(x, 0, 1, "red_bed[facing=north,occupied=false,part=foot]");
    for z in 0..2 {
        b.set(x, -1, z, STONE);
    }
    b.set(x, 1, 1, GLASS);
    b.set(x, 2, 1, GLASS);
    for w in [x - 1, x + 1] {
        b.set(w, 1, 0, GLASS);
        b.set(w, 2, 0, GLASS);
    }
    if sees {
        b.set(x, 1, 0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
    } else {
        b.set(x, 1, -1, GLASS);
        b.set(x, 2, -1, GLASS);
    }
    for dx in -1..=1 {
        for z in 0..2 {
            // (an opaque roof, its sky light under 15, capped with glass: no golem spawns on it)
            b.set(x + dx, 3, z, STONE);
            b.set(x + dx, 4, z, GLASS);
        }
    }
    b.entity("villager", [x as f64 + 0.5, 0.5625, 0.5], None);
}

/// The zombie in a boat on a slab 7 north of S (x = 2) under a stone roof (its eyes never see the
/// sky), and the hide: a closed top-half trapdoor facing east at (2, 1, -6). S's line of sight
/// crosses that cell 1.78-1.84 up, through the shut trapdoor's slab; opened (powered), the
/// trapdoor stands against the cell's west side and the line passes.
fn zombie(b: &mut Builder) {
    b.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, STONE);
    // (the stone roof capped with glass: no golem spawns on it)
    b.set(2, 3, -7, GLASS);
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(2, 1, -6, "oak_trapdoor[facing=east,half=top,open=false,powered=false,waterlogged=false]");
}

/// U1's pod east of the hide: a wooden pressure plate at (3, 2, -6) on the stone (3, 1, -6) next to
/// the trapdoor, U1's bed head east of the plate (facing west), glass walls, a glass roof at y 4
/// (U1 cannot stand on its bed: the plate cell is its only standing cell).
fn plate_pod(b: &mut Builder) {
    b.set(3, 1, -6, STONE);
    b.set(3, 2, -6, "oak_pressure_plate[powered=false]");
    b.set(4, 2, -6, "red_bed[facing=west,occupied=false,part=head]");
    b.set(5, 2, -6, "red_bed[facing=west,occupied=false,part=foot]");
    for x in 4..=5 {
        b.set(x, 1, -6, STONE);
    }
    for y in 2..=3 {
        for x in 3..=5 {
            b.set(x, y, -7, GLASS);
            b.set(x, y, -5, GLASS);
        }
        b.set(2, y, -6, GLASS);
        b.set(6, y, -6, GLASS);
    }
    for x in 3..=5 {
        b.set(x, 4, -6, STONE);
        b.set(x, 5, -6, STONE);
        b.set(x, 6, -6, GLASS);
    }
    b.entity("villager", [3.5, 2.0, -5.5], None);
}

/// The bed-plate front: S (x 2, sees the zombie), U2 (x 0, blind), U1 on the plate by the hide.
fn front(b: &mut Builder) {
    pod(b, 0, false);
    pod(b, 2, true);
    zombie(b);
    plate_pod(b);
}

/// Prototype A: the bed-plate front over L2d's dry hole and 2-hopper chamber (the calm only: its
/// kill is not a day-1 one).
pub fn l6p_dry(n: i32) -> String {
    let mut b = Builder::new(&format!("iron-day1-bedplate-dry-{n}"));
    front(&mut b);
    let (wl, k) = (-6, -22);
    crate::layouts::dry_hole(&mut b, n, wl, k);
    crate::layouts::chamber2(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&["The bed-plate calm (prototype): S sees the zombie by day; U1 lies down at nightfall and lets go of", "the plate, the trapdoor shuts across S's line of sight, S sleeps; L2d's dry hole and chamber below."])
}

const GATE_N: &str = "oak_fence_gate[facing=north,in_wall=false,open=true,powered=false]";

/// The day-1 kill: L2d's dry-hole pad (n x n, floor y -7, x 0..n-1, z 2..n+1, n even; water in
/// its 4 corners; the 4 x 4 hole at its center: its ring is water over open fence gates, its
/// center open gates at the water's level: they hold the water, not a golem) over a 4 x 4 stone
/// pit (feet y -12, 5 blocks down: the lava's fire never reaches the gates). One water source
/// in the pit's south-east corner pushes every golem into the north-west corner, where one lava
/// source at head height burns it, held by 3 wall signs (signs do not burn); the drops float to
/// the same corner's hopper, over the chest: 1 lava source, 1 hopper.
fn pit_kill(b: &mut Builder, n: i32) {
    let (x0, x1, z0, z1) = (0, n - 1, 2, n + 1);
    // the hole: x hx..hx+3, z hz..hz+3; its center 2 x 2 at x hx+1..hx+2, z hz+1..hz+2
    let (hx, hz) = (n / 2 - 2, z0 + n / 2 - 2);
    let hole = |x: i32, z: i32| (hx..=hx + 3).contains(&x) && (hz..=hz + 3).contains(&z);
    let center = |x: i32, z: i32| (hx + 1..=hx + 2).contains(&x) && (hz + 1..=hz + 2).contains(&z);
    for x in x0..=x1 {
        for z in z0..=z1 {
            if center(x, z) {
                b.set(x, -6, z, GATE_N);
            } else if hole(x, z) {
                b.set(x, -7, z, GATE_N);
            } else {
                b.set(x, -7, z, STONE);
            }
        }
    }
    for y in -6..=-5 {
        for x in (x0 - 1)..=(x1 + 1) {
            b.set(x, y, z0 - 1, GLASS);
            b.set(x, y, z1 + 1, GLASS);
        }
        for z in z0..=z1 {
            b.set(x0 - 1, y, z, GLASS);
            b.set(x1 + 1, y, z, GLASS);
        }
    }
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        b.set(x, -6, z, "water[level=0]");
    }
    // the pit: a stone shell around the hole's columns from the pad floor down to its floor
    let k = -12;
    for y in (k - 1)..=-8 {
        for x in (hx - 1)..=(hx + 4) {
            for z in (hz - 1)..=(hz + 4) {
                if !hole(x, z) || y == k - 1 {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    let (lx, lz) = (hx, hz);
    b.set(hx + 3, k, hz + 3, "water[level=0]");
    b.set(lx, k + 2, lz, "lava[level=0]");
    b.set(lx, k + 1, lz, "oak_wall_sign[facing=south,waterlogged=false]");
    b.set(lx + 1, k + 2, lz, "oak_wall_sign[facing=south,waterlogged=false]");
    b.set(lx, k + 2, lz + 1, "oak_wall_sign[facing=east,waterlogged=false]");
    b.set(lx, k - 1, lz, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [lx, k - 2, lz]);
}

/// Day 1, candidate D1: the bed-plate front over the pit kill (8 iron: the bucket and one hopper;
/// one lava source; no quartz).
pub fn l6p_pad(n: i32) -> String {
    let mut b = Builder::new(&format!("iron-day1-bedplate-pit-{n}"));
    front(&mut b);
    pit_kill(&mut b, n);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&["Day 1 (candidate): the bed-plate calm; golems spawn on a pad 6 below the villagers, fall through its", "center into a 4 x 4 pit and burn in its corner under one lava source, over one hopper."])
}

/// L2d's chamber without its fire: lava only in the north row (x 1..4, head height k + 2, z 1),
/// open fence gates under it and south of it (z 2) right under the ceiling, no mouth gates. Lava's
/// random tick (`LavaFluid.randomTick`) then never lights anything: every first step up is the
/// ceiling or the north wall, and the gates beside it at its own level have the ceiling above them
/// (the archived chamber's mouth gates at z 3 had the open shaft above). A golem in the 2-deep
/// chamber always meets the north row.
fn chamber_safe(b: &mut Builder, k: i32) {
    for x in 1..=4 {
        for y in k..=k + 3 {
            b.set(x, y, 0, STONE);
        }
        b.set(x, k + 3, 1, STONE);
        b.set(x, k + 3, 2, STONE);
        b.set(x, k + 1, 1, GATE_E);
        b.set(x, k + 2, 1, "lava[level=0]");
        b.set(x, k + 2, 2, GATE_E);
        b.set(x, k - 1, 2, STONE);
        if x == 1 || x == 4 {
            b.set(x, k - 1, 1, STONE);
        }
    }
    for z in 0..=2 {
        for y in (k - 1)..=(k + 3) {
            b.set(0, y, z, STONE);
            b.set(5, y, z, STONE);
        }
    }
    b.set(2, k - 1, 1, "hopper[enabled=true,facing=east]");
    b.set(3, k - 1, 1, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [3, k - 2, 1]);
}

const GATE_E: &str = "oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]";

/// Middle, candidate M2: the bed observer. Three scared villagers (pods at x 0, 2, 4) see the
/// zombie through the cell (2, 1, -6), where a closed bottom-half trapdoor lets their lines pass
/// (L2d's hide: opened, it stands across them); U1 (x 6, blind) sleeps at every REST. An observer
/// watching U1's bed head fires when the bed goes OCCUPIED (or free) and toggles a copper bulb;
/// a comparator reading the bulb opens the hide and feeds a 12-repeater line (48 gt) back into the
/// bulb, which toggles it off: the zombie hides for about 48 gt at nightfall (and at dawn), the
/// scared villagers fall asleep, then wake scared for the rest of the night. Nothing reads the sky.
pub fn m2_observer(n: i32) -> String {
    let mut b = Builder::new(&format!("iron-middle-observer-{n}"));
    for x in [0, 2, 4] {
        pod(&mut b, x, true);
    }
    pod(&mut b, 6, false);
    b.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, STONE);
    b.set(2, 3, -7, GLASS);
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(2, 1, -6, "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]");
    // the observer east of U1's bed head, the bulb behind it, the comparator north of the bulb
    // (reading it, its output north into the junction dust at (8, 0, -2))
    let support = |b: &mut Builder, x: i32, z: i32| b.set(x, -1, z, STONE);
    b.set(7, 0, 0, "observer[facing=west,powered=false]");
    b.set(8, 0, 0, "waxed_copper_bulb[lit=false,powered=false]");
    b.set(8, 1, 0, GLASS);
    support(&mut b, 8, -1);
    b.set(8, 0, -1, "comparator[facing=south,mode=compare,powered=false]");
    support(&mut b, 8, -2);
    b.set(8, 0, -2, "redstone_wire[east=side,north=side,power=0,south=side,west=none]");
    // the delay, inside the chunk column: 5 repeaters east along z -2 (x 9..13), the turn at x 14,
    // 7 repeaters south along x 14 (z -1..5): 12 at delay 4, 48 gt; then dust west along z 6 and
    // north along x 9 to a repeater at (8, 0, 1) that powers the bulb from the south
    for x in 9..=13 {
        support(&mut b, x, -2);
        b.set(x, 0, -2, "repeater[delay=4,facing=west,locked=false,powered=false]");
    }
    support(&mut b, 14, -2);
    b.set(14, 0, -2, "redstone_wire[east=none,north=none,power=0,south=side,west=side]");
    for z in -1..=5 {
        support(&mut b, 14, z);
        b.set(14, 0, z, "repeater[delay=4,facing=north,locked=false,powered=false]");
    }
    for x in 9..=14 {
        support(&mut b, x, 6);
        let s = match x {
            14 => "redstone_wire[east=none,north=side,power=0,south=none,west=side]",
            9 => "redstone_wire[east=side,north=side,power=0,south=none,west=none]",
            _ => "redstone_wire[east=side,north=none,power=0,south=none,west=side]",
        };
        b.set(x, 0, 6, s);
    }
    for z in 2..=5 {
        support(&mut b, 9, z);
        b.set(9, 0, z, if z == 2 { "redstone_wire[east=none,north=none,power=0,south=side,west=side]" } else { "redstone_wire[east=none,north=side,power=0,south=side,west=none]" });
    }
    support(&mut b, 8, 2);
    b.set(8, 0, 2, "redstone_wire[east=side,north=side,power=0,south=none,west=none]");
    support(&mut b, 8, 1);
    b.set(8, 0, 1, "repeater[delay=1,facing=south,locked=false,powered=false]");
    // the hide's line: north from the junction to z -6, then west to a block east of the
    // trapdoor, the dust on top of it pointing into the trapdoor
    for z in -6..=-3 {
        support(&mut b, 8, z);
        b.set(8, 0, z, if z == -6 { "redstone_wire[east=none,north=none,power=0,south=side,west=side]" } else { "redstone_wire[east=none,north=side,power=0,south=side,west=none]" });
    }
    for x in 4..=7 {
        support(&mut b, x, -6);
        b.set(x, 0, -6, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
    }
    b.set(3, 0, -6, STONE);
    b.set(3, 1, -6, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
    let (wl, k) = (-6, -22);
    crate::layouts::dry_hole(&mut b, n, wl, k);
    chamber_safe(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&["Middle (candidate M2): the bed observer hides the zombie for 48 gt when U1 lies down; L2d's dry hole, a", "16-block drop and a chamber whose lava cannot light anything."])
}

/// Middle, candidate M3: the bed plate for three scared villagers (pods at x 0, 2, 4) and U1 on its
/// plate, over L2d's 6 x 6 dry hole, the 16-block drop and the fire-safe chamber.
pub fn m3_bedplate(n: i32) -> String {
    let mut b = Builder::new(&format!("iron-middle-bedplate-{n}"));
    for x in [0, 2, 4] {
        pod(&mut b, x, true);
    }
    zombie(&mut b);
    plate_pod(&mut b);
    let (wl, k) = (-6, -22);
    crate::layouts::dry_hole(&mut b, n, wl, k);
    chamber_safe(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&["Middle (candidate M3): the bed plate for three scared villagers; L2d's dry hole, a 16-block drop and a", "chamber whose lava cannot light anything."])
}
