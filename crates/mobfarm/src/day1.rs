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
