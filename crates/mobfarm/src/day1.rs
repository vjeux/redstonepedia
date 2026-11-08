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

const STONE: &str = "cobblestone";
const GLASS: &str = "glass";
/// A day-1 cap (vjeux 2026-11-08 13:09: day-1 materials): a bottom cobblestone slab on a top no
/// golem may spawn on (`LEGACY_IRON_GOLEM`'s scan stops on it and its half top fails the spawn).
const CAP: &str = "cobblestone_slab[type=bottom,waterlogged=false]";

/// A day-1 build (its name `iron-day1-...`) uses what a fresh player has: cobblestone walls where
/// the other stages use glass walls, and [`CAP`] where they cap a top with glass.
fn day1_build(b: &Builder) -> bool {
    b.d.name.starts_with("iron-day1")
}

/// The wall block of a pod, the plate pod and the pad's rim: cobblestone on day 1, else glass.
fn wall_of(b: &Builder) -> &'static str {
    if day1_build(b) { STONE } else { GLASS }
}

/// The cap on a roof: [`CAP`] on day 1, else glass.
fn cap_of(b: &Builder) -> &'static str {
    if day1_build(b) { CAP } else { GLASS }
}

/// A day-1 void build's cobblestone tops: glass walls never held a golem, cobblestone ones do, so
/// every cobblestone block above the pad's floor (y -7, the farm's only spawn floor) with nothing on
/// top gets a [`CAP`].
fn cap_tops(b: &mut Builder) {
    let stone = format!("minecraft:{STONE}");
    let tops: Vec<[i32; 3]> = b.d.blocks.iter().filter(|(p, s)| **s == stone && p[1] > -7 && !b.d.blocks.contains_key(&[p[0], p[1] + 1, p[2]])).map(|(p, _)| *p).collect();
    for p in tops {
        b.set(p[0], p[1] + 1, p[2], CAP);
    }
}

/// A villager on the head of its bed (facing north) in a closed 1-cell glass pod at (x, 0, 0), a
/// glass roof; `sees`: its north face at y + 1 is an open trapdoor (it sees over it, it cannot
/// pass), else glass up to its eyes (it never sees north).
pub fn pod(b: &mut Builder, x: i32, sees: bool) {
    pod_at(b, x, 0, sees);
}

/// [`pod`] with its bed head at (x, 0, z) (the foot south of it).
pub fn pod_at(b: &mut Builder, x: i32, z0: i32, sees: bool) {
    let (wall, cap) = (wall_of(b), cap_of(b));
    b.set(x, 0, z0, "red_bed[facing=north,occupied=false,part=head]");
    b.set(x, 0, z0 + 1, "red_bed[facing=north,occupied=false,part=foot]");
    for z in z0..z0 + 2 {
        b.set(x, -1, z, STONE);
    }
    b.set(x, 1, z0 + 1, wall);
    b.set(x, 2, z0 + 1, wall);
    for w in [x - 1, x + 1] {
        b.set(w, 1, z0, wall);
        b.set(w, 2, z0, wall);
    }
    if sees {
        b.set(x, 1, z0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
    } else {
        b.set(x, 1, z0 - 1, wall);
        b.set(x, 2, z0 - 1, wall);
    }
    for dx in -1..=1 {
        for z in z0..z0 + 2 {
            // (an opaque roof, its sky light under 15, capped: no golem spawns on it)
            b.set(x + dx, 3, z, STONE);
            b.set(x + dx, 4, z, cap);
        }
    }
    b.entity("villager", [x as f64 + 0.5, 0.5625, z0 as f64 + 0.5], None);
}

/// [`pod_at`] by roles (designs/farms/roles.tsv): its walls cobblestone (walk blockers; a blind
/// pod's front a sight blocker), its roof cobblestone (a sky cap), and no caps on the roof: the
/// module's spot check caps every top a golem could spawn on by its role ([`crate::parts::cap_spawns`]).
pub fn pod_bare(b: &mut Builder, x: i32, z0: i32, sees: bool) {
    b.set(x, 0, z0, "red_bed[facing=north,occupied=false,part=head]");
    b.set(x, 0, z0 + 1, "red_bed[facing=north,occupied=false,part=foot]");
    for z in z0..z0 + 2 {
        b.set(x, -1, z, STONE);
    }
    b.set(x, 1, z0 + 1, STONE);
    b.set(x, 2, z0 + 1, STONE);
    for w in [x - 1, x + 1] {
        b.set(w, 1, z0, STONE);
        b.set(w, 2, z0, STONE);
    }
    if sees {
        b.set(x, 1, z0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
    } else {
        b.set(x, 1, z0 - 1, STONE);
        b.set(x, 2, z0 - 1, STONE);
    }
    for dx in -1..=1 {
        for z in z0..z0 + 2 {
            b.set(x + dx, 3, z, STONE);
        }
    }
    b.entity("villager", [x as f64 + 0.5, 0.5625, z0 as f64 + 0.5], None);
}

/// The zombie in a boat on a slab 7 north of S (x = 2) under a stone roof (its eyes never see the
/// sky), and the hide: a closed top-half trapdoor facing east at (2, 1, -6). S's line of sight
/// crosses that cell 1.78-1.84 up, through the shut trapdoor's slab; opened (powered), the
/// trapdoor stands against the cell's west side and the line passes.
fn zombie(b: &mut Builder) {
    let cap = cap_of(b);
    b.set(2, 0, -7, "cobblestone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, STONE);
    // (the stone roof capped: no golem spawns on it)
    b.set(2, 3, -7, cap);
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(2, 1, -6, "oak_trapdoor[facing=east,half=top,open=false,powered=false,waterlogged=false]");
}

/// U1's pod east of the hide: a wooden pressure plate at (3, 2, -6) on the stone (3, 1, -6) next to
/// the trapdoor, U1's bed head east of the plate (facing west), walls, a capped roof at y 4..6
/// (U1 cannot stand on its bed: the plate cell is its only standing cell).
fn plate_pod(b: &mut Builder) {
    let (wall, cap) = (wall_of(b), cap_of(b));
    b.set(3, 1, -6, STONE);
    b.set(3, 2, -6, "oak_pressure_plate[powered=false]");
    b.set(4, 2, -6, "red_bed[facing=west,occupied=false,part=head]");
    b.set(5, 2, -6, "red_bed[facing=west,occupied=false,part=foot]");
    for x in 4..=5 {
        b.set(x, 1, -6, STONE);
    }
    for y in 2..=3 {
        for x in 3..=5 {
            b.set(x, y, -7, wall);
            b.set(x, y, -5, wall);
        }
        b.set(2, y, -6, wall);
        b.set(6, y, -6, wall);
    }
    for x in 3..=5 {
        b.set(x, 4, -6, STONE);
        b.set(x, 5, -6, STONE);
        b.set(x, 6, -6, cap);
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
/// pit (feet y -13, 6 blocks down: `LavaFluid.randomTick` climbs 2 steps from the lava at y -11,
/// to y -9, short of the air under the ring gates at y -8, so its fire never reaches them). One water source
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
    let (wall, day1) = (wall_of(b), day1_build(b));
    for y in -6..=-5 {
        for x in (x0 - 1)..=(x1 + 1) {
            b.set(x, y, z0 - 1, wall);
            b.set(x, y, z1 + 1, wall);
        }
        for z in z0..=z1 {
            b.set(x0 - 1, y, z, wall);
            b.set(x1 + 1, y, z, wall);
        }
    }
    // (a day-1 rim is cobblestone, a golem's floor: each rim cell gets a cap on top)
    if day1 {
        for x in (x0 - 1)..=(x1 + 1) {
            b.set(x, -4, z0 - 1, CAP);
            b.set(x, -4, z1 + 1, CAP);
        }
        for z in z0..=z1 {
            b.set(x0 - 1, -4, z, CAP);
            b.set(x1 + 1, -4, z, CAP);
        }
    }
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        b.set(x, -6, z, "water[level=0]");
    }
    // the pit: a stone shell around the hole's columns from the pad floor down to its floor
    let k = -13;
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
    cap_tops(&mut b);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&["Day 1 (candidate): the bed-plate calm; golems spawn on a pad 6 below the villagers, fall through its", "center into a 4 x 4 pit and burn in its corner under one lava source, over one hopper."])
}

/// Day 1, candidate D4: D1 with its second pod open too (S2 at x 0 sees the zombie). S2's line
/// to the zombie crosses the same hide cell (2, 1, -6) as S's, 1.78-1.84 up (from z -5 to -6 it
/// runs x 2.01-2.28), and its sleeping eye passes under the slab like S's: one plate and one
/// trapdoor hide the zombie from both, and two panicking villagers make 20 tries a mark.
pub fn d4_two_scared(n: i32) -> String {
    let mut b = Builder::new(&format!("iron-day1-two-scared-{n}"));
    pod(&mut b, 0, true);
    pod(&mut b, 2, true);
    zombie(&mut b);
    // the hide facing west: opened, its slab stands against the cell's east side, clear of both
    // lines (S2's runs x 2.01-2.28 through the cell, S's x 2.5); shut, its top slab crosses both
    b.set(2, 1, -6, "oak_trapdoor[facing=west,half=top,open=false,powered=false,waterlogged=false]");
    plate_pod(&mut b);
    pit_kill(&mut b, n);
    cap_tops(&mut b);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&["Day 1 (candidate D4): the bed plate hides the zombie from two scared villagers through one trapdoor;", "golems fall through the pad's center into a 4 x 4 pit and burn in its corner under one lava source."])
}

/// Day 1, candidate D1U: D4's mechanism dug into the ground (brief revision 8: day 1 in terrain).
/// The villagers stand 8 below the ground layer (design y 8), so the golem search of a scared
/// villager (y + 5 down to y - 7) never meets a natural surface; natural rock is every wall,
/// floor and roof, so the design lists only the cells it digs (`air`) and the blocks it places
/// (beds, trapdoors, the plate, the zombie's slab, the pad's floor and gates, water, lava, signs,
/// the hopper; no torch yet: redsim models none, so hostiles spawning in its dark cells are an
/// exposure on its card). A void judge cannot run it
/// (nothing holds its water): it is judged in a terrain sample (`terrain: cells`).
pub fn d1u(n: i32) -> String {
    let mut b = Builder::new(&format!("iron-day1-underground-{n}"));
    let air = "air";
    // the pods: S2 (x 0) and S (x 2) on their beds, their in-cell trapdoors, the cell above
    for x in [0, 2] {
        b.set(x, 0, 0, "red_bed[facing=north,occupied=false,part=head]");
        b.set(x, 0, 1, "red_bed[facing=north,occupied=false,part=foot]");
        b.set(x, 1, 0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
        b.set(x, 2, 0, air);
        b.entity("villager", [x as f64 + 0.5, 0.5625, 0.5], None);
    }
    // the sight tunnel north to the hide (x 0..2, y 1..2, z -1..-5)
    for z in -5..=-1 {
        for x in 0..=2 {
            for y in 1..=2 {
                b.set(x, y, z, air);
            }
        }
    }
    // the sleeping eyes' rays to the zombie's eye (a sleeping villager settles on its bed's top:
    // its eye at bed + 0.7625) rise through y 1 only 1.8 blocks north of the beds, so the floor
    // cells in front of the two pods are dug two deep (else the natural floor hides the zombie from
    // the sleeping villagers and no golem spawns at night), each under a top slab: the slabs are
    // above the sleeping ray (1.10 at most there) and below the standing one (2.02 at least), and
    // leave no standing room in the dug cells (no stand-up spot for the villager,
    // `BedBlock.findStandUpPosition`) and no golem room over themselves
    for x in [0, 2] {
        for z in [-1, -2] {
            b.set(x, 0, z, air);
            b.set(x, 1, z, "cobblestone_slab[type=top,waterlogged=false]");
        }
    }
    // a torch on the tunnel's west wall lights the tunnel, the pods and the cells under the slabs
    // (no monster spawns at block light 1 or more); it has no collision, so both rays pass
    b.set(0, 2, -3, "wall_torch[facing=east]");
    // the zombie (D1's) and the hide facing west (D4's)
    b.set(2, 0, -7, "cobblestone_slab[type=bottom,waterlogged=false]");
    b.set(2, 1, -7, air);
    // (the zombie's eye box reaches z -7.04: the cell north of it dug too, or it suffocates)
    b.set(2, 1, -8, air);
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(2, 1, -6, "oak_trapdoor[facing=west,half=top,open=false,powered=false,waterlogged=false]");
    // U1: the plate on stone by the hide, its bed east of it, the cells it stands in
    b.set(3, 1, -6, STONE);
    b.set(3, 2, -6, "oak_pressure_plate[powered=false]");
    b.set(3, 3, -6, air);
    b.set(4, 2, -6, "red_bed[facing=west,occupied=false,part=head]");
    b.set(5, 2, -6, "red_bed[facing=west,occupied=false,part=foot]");
    b.set(4, 3, -6, air);
    b.set(5, 3, -6, air);
    b.entity("villager", [3.5, 2.0, -5.5], None);
    // the pad (pit_kill's, n x n at x 0..n-1, z 2..n+1): its floor and gates, water, the 2 cells of
    // golem headroom over the water
    let (x1, z0, z1) = (n - 1, 2, n + 1);
    let (hx, hz) = (n / 2 - 2, z0 + n / 2 - 2);
    let hole = |x: i32, z: i32| (hx..=hx + 3).contains(&x) && (hz..=hz + 3).contains(&z);
    let center = |x: i32, z: i32| (hx + 1..=hx + 2).contains(&x) && (hz + 1..=hz + 2).contains(&z);
    for x in 0..=x1 {
        for z in z0..=z1 {
            let corner = (x == 0 || x == x1) && (z == z0 || z == z1);
            if center(x, z) {
                b.set(x, -7, z, air);
                b.set(x, -6, z, GATE_N);
            } else if hole(x, z) {
                b.set(x, -7, z, GATE_N);
                b.set(x, -6, z, air);
            } else {
                b.set(x, -7, z, STONE);
                b.set(x, -6, z, if corner { "water[level=0]" } else { air });
            }
            b.set(x, -5, z, air);
            b.set(x, -4, z, air);
        }
    }
    // the pit under the hole (feet k = -13, fire-safe like pit_kill's): dug, the water source pushing golems into the corner
    // under the lava, 3 wall signs holding the lava, the hopper over the output
    let k = -13;
    for x in hx..=hx + 3 {
        for z in hz..=hz + 3 {
            for y in k..=-8 {
                b.set(x, y, z, air);
            }
        }
    }
    let (lx, lz) = (hx, hz);
    b.set(hx + 3, k, hz + 3, "water[level=0]");
    b.set(lx, k + 2, lz, "lava[level=0]");
    b.set(lx, k + 1, lz, "oak_wall_sign[facing=south,waterlogged=false]");
    b.set(lx + 1, k + 2, lz, "oak_wall_sign[facing=south,waterlogged=false]");
    b.set(lx, k + 2, lz + 1, "oak_wall_sign[facing=east,waterlogged=false]");
    // the fire shield: the cells over the two signs beside the lava hold oak buttons (on the pit's
    // natural walls: no collision, not ignited by lava, made of planks on day 1), so lava's random
    // tick (`LavaFluid.randomTick`: fire goes only into an air cell beside a block lava ignites, or
    // an empty cell above one) never lights a fire beside a sign; a golem falls through them
    b.set(lx, k + 3, lz + 1, "oak_button[face=wall,facing=east,powered=false]");
    b.set(lx + 1, k + 3, lz, "oak_button[face=wall,facing=south,powered=false]");
    b.set(lx, k - 1, lz, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [lx, k - 2, lz]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.d.meta.insert("terrain".into(), "cells".into());
    // the ground layer 11 above the pods' floor: the third villager (its bed's cell at y 2) panics
    // standing or hopping a block higher, so its golem search's first floor is y 8 (start 3, + 5);
    // with the ground layer at 8 or 9 the surface was in reach and golems it summoned there
    // stalled the farm (mobinfra's traced gates: a golem at (-2, 7, -12) on ground 8; golems at
    // y 72 over the farm on ground 9); at 11 the surface lies 2 above that reach, and the
    // cheapest spot has no cave a mob walks in from (10's had one); its AFK spot on the ground
    // over the pad
    b.d.meta.insert("ground".into(), "11".into());
    b.d.meta.insert("afk".into(), "2.5 12 4.5".into());
    b.text(&[
        "Day-1 iron farm dug into the ground beside a village: two villagers on their beds watch a zombie parked in a boat",
        "and keep summoning iron golems. At nightfall a third villager lies down in its bed, its pressure plate lets go and a",
        "trapdoor hides the zombie: the two fall asleep, then see it again from their beds and wake scared, so golems come",
        "day and night. Golems appear on a water pad 7 blocks down, fall through its middle into a pit and burn in its corner",
        "under one lava source; the iron drops into a hopper and a chest at the bottom, reached by a staircase.",
    ])
}

/// L2d's chamber without its fire: lava only in the north row (x 1..4, head height k + 2, z 1),
/// open fence gates under it and south of it (z 2) right under the ceiling, no mouth gates. Lava's
/// random tick (`LavaFluid.randomTick`) then never lights anything: every first step up is the
/// ceiling or the north wall, and the gates beside it at its own level have the ceiling above them
/// (the archived chamber's mouth gates at z 3 had the open shaft above). A golem in the 2-deep
/// chamber always meets the north row.
pub fn chamber_safe(b: &mut Builder, k: i32) {
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
    let mut b = m2_module(n);
    b.text(&["Middle (candidate M2): the bed observer hides the zombie for 48 gt when U1 lies down; L2d's dry hole, a", "16-block drop and a chamber whose lava cannot light anything."])
}

/// The bar's packing (brief, Stages: Late): `modules` copies of M2, module i moved by (dx, dy, dz)
/// x i, each with its own villagers, zombie, bed observer, drop and chamber; the first module's
/// chest is the output, the others' are chests. Stacked (dy 43, dx dz 0) no module's golem ever
/// enters the follow box of another's villagers: the chamber above starts 43 over the villagers
/// below (their box ends 2.51 + 16 x 1.11485 above their feet). Side by side (dx 16..27) it
/// measures the coupling through the follow box.
pub fn m2_stack(modules: i32, step: [i32; 3], n: i32) -> String {
    let m = m2_module(n).d;
    let [sx, sy, sz] = step;
    let mut b = Builder::new(&format!("iron-late-stack-{modules}x{sx}-{sy}-{sz}"));
    b.d.meta = m.meta.clone();
    let out = m.ports["output"];
    for i in 0..modules {
        let (dx, dy, dz) = (sx * i, sy * i, sz * i);
        for (p, s) in &m.blocks {
            b.d.blocks.insert([p[0] + dx, p[1] + dy, p[2] + dz], s.clone());
        }
        for e in &m.entities {
            b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [e.pos[0] + dx as f64, e.pos[1] + dy as f64, e.pos[2] + dz as f64], nbt: e.nbt.clone() });
        }
        if i == 0 {
            b.d.ports.insert("output".into(), out);
        } else {
            b.set(out[0] + dx, out[1] + dy, out[2] + dz, "chest[facing=north,type=single,waterlogged=false]");
        }
    }
    b.text(&[&format!("Late (the bar's packing): {modules} M2 modules {sx} {sy} {sz} apart, each with its own villagers, zombie,"), "bed observer, drop and fire-safe chamber."])
}

/// M2's front with the smallest hide circuit (the catalog's pulse-extender-repeater-L{2d}): U1's
/// bed observer pulses (2 gt) into one repeater (delay `d`, 1..=4: the pulse stretched to 2d gt),
/// whose dust runs to the hide. `d` = 0: the observer's own 2 gt pulse, no repeater.
fn m2_front_rep(n: i32, d: i32) -> Builder {
    let mut b = Builder::new(&format!("iron-middle-rep{d}-{n}"));
    for x in [0, 2, 4] {
        pod(&mut b, x, true);
    }
    pod(&mut b, 6, false);
    b.set(2, 0, -7, "cobblestone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, STONE);
    b.set(2, 3, -7, GLASS);
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(2, 1, -6, "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]");
    let support = |b: &mut Builder, x: i32, z: i32| b.set(x, -1, z, STONE);
    b.set(7, 0, 0, "observer[facing=west,powered=false]");
    support(&mut b, 8, 0);
    if d > 0 {
        b.set(8, 0, 0, &format!("repeater[delay={},facing=west,locked=false,powered=false]", d.clamp(1, 4)));
    } else {
        b.set(8, 0, 0, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
    }
    // the dust: east of the repeater, north along x 9 to z -6, west along z -6, up onto the block
    // east of the trapdoor
    support(&mut b, 9, 0);
    b.set(9, 0, 0, "redstone_wire[east=none,north=side,power=0,south=none,west=side]");
    for z in -5..=-1 {
        support(&mut b, 9, z);
        b.set(9, 0, z, "redstone_wire[east=none,north=side,power=0,south=side,west=none]");
    }
    support(&mut b, 9, -6);
    b.set(9, 0, -6, "redstone_wire[east=none,north=none,power=0,south=side,west=side]");
    for x in 4..=8 {
        support(&mut b, x, -6);
        b.set(x, 0, -6, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
    }
    b.set(3, 0, -6, STONE);
    b.set(3, 1, -6, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
    b
}

/// M2-open with [`m2_front_rep`]'s hide (one repeater, `d` 0..=4).
pub fn m2_open_rep(n: i32, d: i32) -> String {
    let mut b = m2_front_rep(n, d);
    let (wl, k) = (-6, -22);
    crate::layouts::dry_hole_walls(&mut b, n, wl, k, -19);
    chamber_safe(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[&format!("Middle (M2-open, the smallest hide): the bed observer hides the zombie for {} gt through one repeater;", 2 * d.max(1)), "L2d's dry hole over an open 16-block drop and a chamber whose lava cannot light anything."])
}

/// M2's front with a compact bulb loop (the hide measured, not a pick): U1's bed observer toggles
/// the bulb; its comparator opens the hide and feeds two repeaters (delays `d1`, `d2`) back into
/// the bulb, which toggles it off: the hide lasts about 2 (d1 + d2) gt plus the comparator's 2.
fn m2_front_loop(n: i32, d1: i32, d2: i32) -> Builder {
    let mut b = Builder::new(&format!("iron-middle-loop{d1}{d2}-{n}"));
    for x in [0, 2, 4] {
        pod(&mut b, x, true);
    }
    pod(&mut b, 6, false);
    b.set(2, 0, -7, "cobblestone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, STONE);
    b.set(2, 3, -7, GLASS);
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(2, 1, -6, "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]");
    let support = |b: &mut Builder, x: i32, z: i32| b.set(x, -1, z, STONE);
    b.set(7, 0, 0, "observer[facing=west,powered=false]");
    b.set(8, 0, 0, "waxed_copper_bulb[lit=false,powered=false]");
    // (a glass cap: no golem spawns on the bulb)
    b.set(8, 1, 0, GLASS);
    support(&mut b, 8, -1);
    b.set(8, 0, -1, "comparator[facing=south,mode=compare,powered=false]");
    support(&mut b, 8, -2);
    b.set(8, 0, -2, "redstone_wire[east=side,north=side,power=0,south=side,west=none]");
    // the loop: a repeater east of the junction, dust south along x 9 to z 2, west to (8, 0, 2),
    // a repeater north into the bulb's south face
    support(&mut b, 9, -2);
    b.set(9, 0, -2, &format!("repeater[delay={},facing=west,locked=false,powered=false]", d1.clamp(1, 4)));
    support(&mut b, 10, -2);
    b.set(10, 0, -2, "redstone_wire[east=none,north=none,power=0,south=side,west=side]");
    for z in -1..=1 {
        support(&mut b, 10, z);
        b.set(10, 0, z, "redstone_wire[east=none,north=side,power=0,south=side,west=none]");
    }
    support(&mut b, 10, 2);
    b.set(10, 0, 2, "redstone_wire[east=none,north=side,power=0,south=none,west=side]");
    support(&mut b, 9, 2);
    b.set(9, 0, 2, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
    support(&mut b, 8, 2);
    b.set(8, 0, 2, "redstone_wire[east=side,north=side,power=0,south=none,west=none]");
    support(&mut b, 8, 1);
    b.set(8, 0, 1, &format!("repeater[delay={},facing=south,locked=false,powered=false]", d2.clamp(1, 4)));
    // the hide's line, as M2's
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
    b
}

/// M2-open with [`m2_front_loop`]'s hide.
pub fn m2_open_loop(n: i32, d1: i32, d2: i32) -> String {
    let mut b = m2_front_loop(n, d1, d2);
    let (wl, k) = (-6, -22);
    crate::layouts::dry_hole_walls(&mut b, n, wl, k, -19);
    chamber_safe(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[&format!("Middle (M2-open, a compact bulb loop): the hide lasts about {} gt;", 2 * (d1 + d2) + 2), "L2d's dry hole over an open 16-block drop and a chamber whose lava cannot light anything."])
}

/// M2's front with its hide timed by a part from the catalog (vjeux 10:48: "We built an entire
/// clock system to find the smallest one for each tick"): U1's bed observer (its 2 gt pulse),
/// the part's stretching repeater when it needs a longer input, the part turned to fit east of
/// the pods, a repeater reading its output, and dust to the hide. Tries the part's 4 turns; None
/// when none fits.
pub fn m2_front_timer(n: i32, t: &crate::parts::Timer) -> Option<(Builder, Vec<[i32; 3]>)> {
    let mut best: Option<(Builder, Vec<[i32; 3]>)> = None;
    // the cage's coupling (the parent, 11:44): blind U1's bed head anywhere within the cluster
    // (its bed observer is the calm's input), so the trigger, the timer and the hide's power cell
    // can sit close together; the hide's power cell is the block east of the trapdoor
    let u1s: [[i32; 3]; 7] = [[6, 0, 0], [5, 0, -4], [6, 0, -5], [-1, 0, -5], [-1, 0, -3], [6, 0, -2], [-2, 0, 0]];
    let reserved = [[3, 0, -6], [2, 0, -6], [3, 1, -6]];
    for u1 in u1s {
        let mut cage = Builder::new(&format!("iron-middle-{}-{n}", t.row.name));
        for x in [0, 2, 4] {
            pod_bare(&mut cage, x, 0, true);
        }
        if (0..=4).contains(&u1[0]) && u1[2] >= -1 {
            continue;
        }
        let before = cage.d.blocks.len();
        pod_bare(&mut cage, u1[0], u1[2], false);
        // (a pod overlapping another's cells: refused)
        if cage.d.blocks.len() < before + 12 {
            continue;
        }
        cage.set(2, 0, -7, "cobblestone_slab[type=bottom,waterlogged=false]");
        cage.set(2, 2, -7, STONE);
        cage.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
        cage.set(2, 1, -6, "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]");
        // the bed observer on any side of U1's bed head but its foot's: (cell, facing, its pulse's
        // direction, the stretching repeater's input side)
        let sides = [([1, 0, 0], "west", "west"), ([-1, 0, 0], "east", "east"), ([0, 0, -1], "south", "south")];
        for (d, facing, back) in sides {
            let obs = [u1[0] + d[0], 0, u1[2] + d[2]];
            if cage.d.blocks.contains_key(&obs) || cage.d.blocks.contains_key(&[obs[0], -1, obs[2]]) && false {
                continue;
            }
            let mut base = Builder { d: cage.d.clone() };
            base.set(obs[0], obs[1], obs[2], &format!("observer[facing={facing},powered=false]"));
            let step = |p: [i32; 3], k: i32| [p[0] + k * d[0], p[1], p[2] + k * d[2]];
            let mut source = vec![obs];
            let input = match t.stretch {
                Some(dl) => {
                    let r = step(obs, 1);
                    if base.d.blocks.contains_key(&r) || base.d.blocks.contains_key(&[r[0], -1, r[2]]) {
                        continue;
                    }
                    base.set(r[0], r[1] - 1, r[2], STONE);
                    base.set(r[0], r[1], r[2], &format!("repeater[delay={dl},facing={back},locked=false,powered=false]"));
                    source.push(r);
                    step(obs, 2)
                }
                None => step(obs, 1),
            };
            if base.d.blocks.contains_key(&input) || reserved.contains(&input) {
                continue;
            }
            let beside = |a: [i32; 3], c: [i32; 3]| (a[0] - c[0]).abs() + (a[1] - c[1]).abs() + (a[2] - c[2]).abs() == 1;
            for turns in 0..4u8 {
                let mut b = Builder { d: base.d.clone() };
                let Ok(placed) = crate::parts::place(&mut b, t.row, turns, input) else { continue };
                // the part in free cells, under the scared villagers' lines of sight (no cell at
                // y 1 or more in front of them), clear of the zombie and the hide's cells, and
                // touching the source only at its input cell
                let region = |c: &[i32; 3]| (-4..=14).contains(&c[0]) && (-6..=8).contains(&c[2]) && (-3..=3).contains(&c[1]) && !reserved.contains(c) && !(c[1] >= 1 && (0..=4).contains(&c[0]) && (-6..=0).contains(&c[2]));
                let fits = placed.cells.iter().all(|c| (*c == input || !base.d.blocks.contains_key(c)) && region(c) && (*c == input || source.iter().all(|s| !beside(*s, *c))));
                if !fits {
                    continue;
                }
                let Some(start) = crate::parts::read_output(&mut b, &placed) else { continue };
                let out_rep = placed.output;
                // the climb onto the block east of the hide first (its dust points into the
                // trapdoor), so the route keeps off its diagonals
                b.set(3, 0, -6, STONE);
                b.set(3, 1, -6, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
                let forbid = |p: [i32; 3]| !(-4..=14).contains(&p[0]) || !(-6..=8).contains(&p[2]) || reserved.contains(&p) || (p[2] >= 0 && (-1..=5).contains(&p[0]));
                let Some(path) = crate::parts::route(&mut b, start, [4, 0, -6], &forbid, &placed.cells, &[out_rep, [3, 0, -6], [3, 1, -6]], 1) else { continue };
                b.set(4, 0, -6, &b.d.blocks[&[4, 0, -6]].replace("west=none", "west=side"));
                if best.as_ref().is_none_or(|(bb, _)| b.d.blocks.len() < bb.d.blocks.len()) {
                    best = Some((b, path));
                }
            }
        }
    }
    best
}

/// M2-open with its hide timed by the smallest catalog part that fits ([`m2_front_timer`]):
/// every part whose pulse from the bed observer lasts `l_min` gt or more with `stage`'s
/// materials ([`crate::parts::timers`]), fewest blocks first, the first that fits.
pub fn m2_open_timer(n: i32, l_min: u32, stage: crate::parts::Stage) -> anyhow::Result<String> {
    let (b, t, path, caps) = m2_timer_module(n, l_min, stage, &chamber_safe)?;
    Ok(b.text(&[
        &format!("Middle (M2-open, its hide from the catalog): the bed observer's pulse, stretched to {} gt by {} ({} blocks with", t.0, t.1, t.2),
        &format!("its wiring of {path} dust and {caps} glass caps), hides the zombie while the villagers lie down; the dry hole, the open drop and the fire-safe chamber."),
    ]))
}

/// One M2-open module with its hide from the catalog ([`m2_front_timer`]: the smallest timer
/// that fits, the cage's trigger next to it), the dry hole and open drop, and `chamber` at k -22;
/// stray golem spawn spots capped with glass. Returns the module, its timer (the pulse's gt, the
/// part's name, its blocks), the wiring's dust and the glass caps.
pub fn m2_timer_module(n: i32, l_min: u32, stage: crate::parts::Stage, chamber: &dyn Fn(&mut Builder, i32)) -> anyhow::Result<(Builder, (u32, String, u32), usize, u32)> {
    let rows = crate::parts::catalog()?;
    let timers = crate::parts::timers(&rows, l_min, 400, 2, stage);
    for t in &timers {
        let Some((mut b, path)) = m2_front_timer(n, t) else { continue };
        let (wl, k) = (-6, -22);
        crate::layouts::dry_hole_walls(&mut b, n, wl, k, -19);
        chamber(&mut b, k);
        b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
        b.d.meta.insert("restart".into(), "yes".into());
        // the pad's spots stay (feet at wl + 1 and below); a part's top or a support gets glass
        let caps = crate::parts::cap_spawns(&mut b, &|p| p[1] <= wl + 1)?;
        let part = format!("hide {}{}", t.row.name, if t.stretch.is_some() { " + a repeater" } else { "" });
        b.d.meta.insert("parts".into(), part.clone());
        return Ok((b, (t.length, t.row.name.clone(), t.blocks), path.len(), caps));
    }
    anyhow::bail!("no catalog part of {} gives a pulse of {l_min} gt or more from the bed observer that fits ({} tried)", match stage {
        crate::parts::Stage::Day1 { .. } => "day 1",
        crate::parts::Stage::Nether => "the Nether",
        crate::parts::Stage::End => "the End",
    }, timers.len())
}

/// The kill chamber of a row module ([`chamber_safe`]): its floor's north row (z 1) all hoppers
/// facing east, its side walls' cells in that row too, so the row's hopper line runs through
/// every chamber to the collection at the east end.
pub fn chamber_line(b: &mut Builder, k: i32) {
    chamber_safe(b, k);
    for x in 0..=5 {
        b.set(x, k - 1, 1, "hopper[enabled=true,facing=east]");
    }
    // (alone, the module's line ends in the judge's chest east of it)
    b.d.ports.insert("output".into(), [6, k - 1, 1]);
}

/// The kill chamber of a stacked module with its own 8 h of storage ([`chamber_safe`]): its
/// output hopper drops into a chest (the module's output), a hopper under it into a second chest,
/// and a third the same way (81 slots and 2 hoppers: about 11 h of one module's iron and poppies).
pub fn chamber_store(b: &mut Builder, k: i32) {
    chamber_safe(b, k);
    b.set(3, k - 2, 1, "chest[facing=north,type=single,waterlogged=false]");
    b.set(3, k - 3, 1, "hopper[enabled=true,facing=down]");
    b.set(3, k - 4, 1, "chest[facing=north,type=single,waterlogged=false]");
    b.set(3, k - 5, 1, "hopper[enabled=true,facing=down]");
    b.set(3, k - 6, 1, "chest[facing=north,type=single,waterlogged=false]");
    b.d.ports.insert("output".into(), [3, k - 2, 1]);
}

/// The stack (farmopt's late search): `modules` M2-open-timer modules `pitch` apart vertically
/// upward (43: no module's golem ever enters another's follow box; the world's floor is y -64), each
/// with its own cage, calm, drop,
/// chamber and 8 h of storage ([`chamber_store`]); the first module's first chest is the output.
pub fn stack(modules: i32, pitch: i32, n: i32, l_min: u32, stage: crate::parts::Stage) -> anyhow::Result<String> {
    let (m, _, _, _) = m2_timer_module(n, l_min, stage, &chamber_store)?;
    let mut b = Builder::new(&format!("iron-late-stack-{modules}x{pitch}"));
    b.d.meta = m.d.meta.clone();
    let out = m.d.ports["output"];
    for i in 0..modules {
        let dy = pitch * i;
        for (p, s) in &m.d.blocks {
            b.d.blocks.insert([p[0], p[1] + dy, p[2]], s.clone());
        }
        for (p, s) in &m.d.block_entities {
            b.d.block_entities.insert([p[0], p[1] + dy, p[2]], s.clone());
        }
        for e in &m.d.entities {
            b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [e.pos[0], e.pos[1] + dy as f64, e.pos[2]], nbt: e.nbt.clone() });
        }
    }
    b.d.ports.insert("output".into(), out);
    Ok(b.text(&[
        &format!("Late: {modules} farms stacked {pitch} blocks apart (far enough that no farm's golems are seen by another's villagers), each with"),
        "its own villagers, zombie, comparator-ring timer, drop, lava chamber and three chests (8 hours of iron each).",
    ]))
}
/// M2-open-timer modules side by side `pitch` apart on x, each with its own cage, calm, drop and
/// chamber ([`chamber_line`]); one hopper line under every chamber's north row carries every
/// drop east to the shared collection ([`collect_bottom_at1`]: the sorter, the iron-block crafter
/// into a double chest, the composter).
pub fn row(modules: i32, pitch: i32, n: i32, l_min: u32, stage: crate::parts::Stage) -> anyhow::Result<String> {
    let (m, t, _, caps) = m2_timer_module(n, l_min, stage, &chamber_line)?;
    let k = -22;
    let mut b = Builder::new(&format!("iron-late-row-{modules}x{pitch}"));
    b.d.meta = m.d.meta.clone();
    for i in 0..modules {
        let dx = pitch * i;
        for (p, s) in &m.d.blocks {
            b.d.blocks.insert([p[0] + dx, p[1], p[2]], s.clone());
        }
        for (p, s) in &m.d.block_entities {
            b.d.block_entities.insert([p[0] + dx, p[1], p[2]], s.clone());
        }
        for e in &m.d.entities {
            b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [e.pos[0] + dx as f64, e.pos[1], e.pos[2]], nbt: e.nbt.clone() });
        }
    }
    // the hopper line from the first chamber's west wall to the collection
    let x_end = pitch * (modules - 1) + 6;
    for x in 0..=x_end {
        b.set(x, k - 1, 1, "hopper[enabled=true,facing=east]");
    }
    // the collection with its line hopper L at (x_end + 1, k - 1, 1)
    let mut c = Builder::new("collect");
    collect_bottom_at1(&mut c, k);
    let (dx, dz) = (x_end + 1 - 1, 1 - (-1));
    for (p, s) in &c.d.blocks {
        b.d.blocks.insert([p[0] + dx, p[1], p[2] + dz], s.clone());
    }
    for (p, s) in &c.d.block_entities {
        b.d.block_entities.insert([p[0] + dx, p[1], p[2] + dz], s.clone());
    }
    // the iron blocks' chest (the output, a single chest) drains into a second through a hopper
    // (8 h of 8 modules: 3,520 iron an hour, 3,129 blocks)
    let out = c.d.ports["output"];
    let out = [out[0] + dx, out[1], out[2] + dz];
    b.set(out[0], out[1] - 1, out[2], "hopper[enabled=true,facing=down]");
    b.set(out[0], out[1] - 2, out[2], "chest[facing=north,type=single,waterlogged=false]");
    b.d.ports.insert("output".into(), out);
    let _ = (t, caps);
    Ok(b.text(&[
        &format!("{modules} farms side by side, {pitch} blocks apart, each with its own villagers, zombie, comparator-ring timer, drop and lava"),
        "chamber; one line of hoppers under every chamber carries every drop to one sorter, one iron-block crafter and one composter.",
    ]))
}

/// The shared kill (farmopt's generator; vjeux 14:51: "why does each module have their own kill
/// chamber?"; ironspec's ruling (b): a golem dies where it lands, the worst walk included): the
/// modules' drops, each given by the bottom cell (west, north corner) of its 4 x 4 drop, all at
/// one y and at least 8 apart along x, go straight down to one 4-wide water channel (z of the drops
/// + 0..3) whose feet stay at or below `villagers_y` - 24, out of every villager's follow cube; at
/// the bottom of each drop a blade (open gates holding lava over the landing 4 x 4) kills its
/// golems where they land, and the channel carries only their drops east (a row of sources at its
/// west end and at each drop, a step down 1 wherever the flow would reach level 7) to one
/// collection: hoppers in its last column's floor into one output chest, and a hopper and a chest
/// under it for every 2 modules (the judge's run of N). Returns the output chest's cell.
pub fn shared_kill(b: &mut Builder, drops: &[[i32; 3]], villagers_y: i32) -> anyhow::Result<[i32; 3]> {
    let (Some(first), Some(last)) = (drops.iter().min_by_key(|d| d[0]), drops.iter().max_by_key(|d| d[0])) else {
        anyhow::bail!("shared kill: no drops");
    };
    let (y_drop, z0) = (first[1], first[2]);
    if drops.iter().any(|d| d[1] != y_drop || d[2] != z0) {
        anyhow::bail!("shared kill: every drop's bottom cell must be at one y and one z (a row along x)");
    }
    let mut starts: Vec<i32> = drops.iter().map(|d| d[0]).collect();
    starts.sort();
    if starts.windows(2).any(|w| w[1] - w[0] < 8) {
        anyhow::bail!("shared kill: drops at least 8 apart on x (each blade has its own stretch of the channel)");
    }
    let (xa, xe) = (starts[0], starts[starts.len() - 1] + 3 + 2);
    let f0 = (villagers_y - 24).min(y_drop - 1);
    // the channel's profile: a row of sources at each drop's first column, one level down from the
    // channel before it (no water flows back), and a step down 1 wherever the flow would reach
    // level 7 (it falls there and spreads 7 more)
    let mut profile = Vec::new();
    let (mut level, mut since) = (f0, 0);
    let mut sources = vec![xa];
    for x in xa..=xe {
        if x > xa && starts.contains(&x) {
            level -= 1;
            since = 0;
            sources.push(x);
        } else if x > xa {
            since += 1;
            if since == 7 {
                level -= 1;
                since = 0;
            }
        }
        profile.push(level);
    }
    let feet = |x: i32| profile[(x.clamp(xa, xe) - xa) as usize];
    let over = |x: i32| drops.iter().any(|d| (d[0]..=d[0] + 3).contains(&x));
    for x in xa - 1..=xe + 1 {
        let f = feet(x);
        // the highest level this column's channel or step face reaches
        let top = feet(x - 1).max(f) + 3;
        for z in z0 - 1..=z0 + 4 {
            let wall = x == xa - 1 || x == xe + 1 || z == z0 - 1 || z == z0 + 4;
            // the floor under the channel and its walls
            b.set(x, f - 1, z, STONE);
            for y in f..=top {
                if wall {
                    b.set(x, y, z, STONE);
                } else {
                    b.d.blocks.remove(&[x, y, z]);
                }
            }
        }
    }
    // the drops: each shaft straight down from its bottom cell to the channel, walled
    drop_shafts(b, drops, &feet, &over, y_drop, z0);
    // the flow: a row of sources at the channel's west end and at each drop's first column
    for x in &sources {
        for z in z0..=z0 + 3 {
            b.set(*x, feet(*x), z, "water[level=0]");
        }
    }
    // each drop's blade where its golems land (vjeux's rule of the worst walk: a golem dies where
    // it lands): open gates at feet + 1 hold lava at feet + 2 over the drop's 4 x 4, cobblestone at
    // feet + 2 beside it holds the lava in; the dead golem's drops fall into the channel's water
    for d in drops {
        let f = feet(d[0]);
        for x in d[0]..=d[0] + 3 {
            for z in z0..=z0 + 3 {
                b.set(x, f + 1, z, GATE_E);
                b.set(x, f + 2, z, "lava[level=0]");
            }
        }
        for z in z0..=z0 + 3 {
            b.set(d[0] - 1, f + 2, z, STONE);
            b.set(d[0] + 4, f + 2, z, STONE);
        }
    }
    // the collection at the channel's east end: hoppers in its last column's floor, north into one
    // chest in the wall, and a hopper and a chest under it for every 2 modules
    let fe = feet(xe);
    for z in z0..=z0 + 3 {
        b.set(xe, fe - 1, z, "hopper[enabled=true,facing=north]");
    }
    let out = [xe, fe - 1, z0 - 1];
    b.set(out[0], out[1], out[2], "chest[facing=south,type=single,waterlogged=false]");
    let extra = (drops.len() as i32 + 1) / 2;
    for i in 0..extra {
        b.set(out[0], out[1] - 1 - 2 * i, out[2], "hopper[enabled=true,facing=down]");
        b.set(out[0], out[1] - 2 - 2 * i, out[2], "chest[facing=north,type=single,waterlogged=false]");
    }
    b.d.ports.insert("output".into(), out);
    Ok(out)
}

/// Where a shared kill kills ([`shared_kill_form`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KillForm {
    /// A blade at each drop's bottom; the channel carries only drops (ironspec's (b)).
    Blades,
    /// The channel carries the golems to one blade chamber at its end (ironspec's iteration 13:
    /// counts once the judge's drift check refuses only escapes and the still policy).
    Channel,
}

/// [`shared_kill`] in either form: `Channel` sends the golems themselves down the channel (one
/// row of sources at its west end, a step down 1 every 7 cells) into one [`chamber_safe`] north of
/// its last 4 cells (sources in their south corners push them in), its output chest and a hopper
/// and a chest under it for every 2 modules.
pub fn shared_kill_form(b: &mut Builder, drops: &[[i32; 3]], villagers_y: i32, form: KillForm) -> anyhow::Result<[i32; 3]> {
    if form == KillForm::Blades {
        return shared_kill(b, drops, villagers_y);
    }
    let mut starts: Vec<i32> = drops.iter().map(|d| d[0]).collect();
    starts.sort();
    let (Some(first), Some(&last)) = (drops.iter().min_by_key(|d| d[0]), starts.last()) else {
        anyhow::bail!("shared kill: no drops");
    };
    let (y_drop, z0) = (first[1], first[2]);
    if drops.iter().any(|d| d[1] != y_drop || d[2] != z0) {
        anyhow::bail!("shared kill: every drop's bottom cell must be at one y and one z (a row along x)");
    }
    let (xa, xe) = (starts[0], last + 3 + 4);
    let f0 = (villagers_y - 24).min(y_drop - 1);
    let step = |x: i32| f0 - (x - xa).div_euclid(7);
    let f_end = step(xe - 3);
    let feet = |x: i32| if x >= xe - 3 { f_end } else { step(x.clamp(xa, xe)) };
    let over = |x: i32| drops.iter().any(|d| (d[0]..=d[0] + 3).contains(&x));
    for x in xa - 1..=xe + 1 {
        let f = feet(x);
        let top = feet(x - 1).max(f) + 3;
        for z in z0 - 1..=z0 + 4 {
            let wall = x == xa - 1 || x == xe + 1 || z == z0 - 1 || z == z0 + 4;
            b.set(x, f - 1, z, STONE);
            // (the chamber's mouth on the end area's north side stays open)
            if wall && z == z0 - 1 && (xe - 3..=xe).contains(&x) {
                continue;
            }
            for y in f..=top {
                if wall {
                    b.set(x, y, z, STONE);
                } else {
                    b.d.blocks.remove(&[x, y, z]);
                }
            }
        }
    }
    drop_shafts(b, drops, &feet, &over, y_drop, z0);
    for z in z0..=z0 + 3 {
        b.set(xa, f0, z, "water[level=0]");
    }
    b.set(xe - 3, f_end, z0 + 3, "water[level=0]");
    b.set(xe, f_end, z0 + 3, "water[level=0]");
    let mut c = Builder::new("chamber");
    chamber_safe(&mut c, f_end);
    let (dx, dz) = (xe - 3 - 1, z0 - 3);
    for (p, s) in &c.d.blocks {
        b.set(p[0] + dx, p[1], p[2] + dz, s);
    }
    // (the channel's flow reaches the chamber's corners too: its whole lava row's floor is hoppers)
    b.set(dx + 1, f_end - 1, dz + 1, "hopper[enabled=true,facing=east]");
    b.set(dx + 4, f_end - 1, dz + 1, "hopper[enabled=true,facing=west]");
    let o = c.d.ports["output"];
    let out = [o[0] + dx, o[1], o[2] + dz];
    for i in 0..(drops.len() as i32 + 1) / 2 {
        b.set(out[0], out[1] - 1 - 2 * i, out[2], "hopper[enabled=true,facing=down]");
        b.set(out[0], out[1] - 2 - 2 * i, out[2], "chest[facing=north,type=single,waterlogged=false]");
    }
    b.d.ports.insert("output".into(), out);
    Ok(out)
}

/// Each drop's shaft straight down from its bottom cell to the channel (feet `feet(x)`), walled.
fn drop_shafts(b: &mut Builder, drops: &[[i32; 3]], feet: &dyn Fn(i32) -> i32, over: &dyn Fn(i32) -> bool, y_drop: i32, z0: i32) {
    for d in drops {
        for x in d[0] - 1..=d[0] + 4 {
            let f = feet(x);
            for y in f + 4..y_drop {
                for z in z0 - 1..=z0 + 4 {
                    let inside = (d[0]..=d[0] + 3).contains(&x) && (z0..=z0 + 3).contains(&z);
                    if inside {
                        b.d.blocks.remove(&[x, y, z]);
                    } else if !(over(x) && (z0..=z0 + 3).contains(&z)) {
                        b.set(x, y, z, STONE);
                    }
                }
            }
        }
    }
}

/// A row of M2-open-ring modules `pitch` apart on x sharing one kill ([`shared_kill`]): each
/// module without its own chamber, its shaft opened at the bottom into the channel.
pub fn row_shared(modules: i32, pitch: i32, n: i32, l_min: u32, stage: crate::parts::Stage) -> anyhow::Result<String> {
    row_shared_form(modules, pitch, n, l_min, stage, KillForm::Blades)
}

/// [`row_shared`] with the kill in either form ([`shared_kill_form`]).
pub fn row_shared_form(modules: i32, pitch: i32, n: i32, l_min: u32, stage: crate::parts::Stage, form: KillForm) -> anyhow::Result<String> {
    let k = -22;
    // (the module's own chamber: none; the judge's port where it was, for the module's spot check)
    let none = |b: &mut Builder, k: i32| {
        b.d.ports.insert("output".into(), [3, k - 2, 1]);
    };
    let (m, _, _, _) = m2_timer_module(n, l_min, stage, &none)?;
    let mut b = Builder::new(&format!("iron-late-row-{}-{modules}x{pitch}", if form == KillForm::Channel { "channel" } else { "shared" }));
    b.d.meta = m.d.meta.clone();
    let mut drops = Vec::new();
    for i in 0..modules {
        let dx = pitch * i;
        for (p, s) in &m.d.blocks {
            // the shaft's bottom (its floor, its sources) and its mouth to a chamber: gone
            let ring = (1..=4).contains(&p[0]) && (3..=6).contains(&p[2]) && p[1] <= k;
            let mouth = (1..=4).contains(&p[0]) && p[2] == 2 && (k..=k + 2).contains(&p[1]);
            if ring {
                continue;
            }
            b.d.blocks.insert([p[0] + dx, p[1], p[2]], if mouth { STONE.to_string() } else { s.clone() });
        }
        for x in 1..=4 {
            for y in k..=k + 2 {
                b.d.blocks.insert([x + dx, y, 2], STONE.to_string());
            }
        }
        for (p, s) in &m.d.block_entities {
            b.d.block_entities.insert([p[0] + dx, p[1], p[2]], s.clone());
        }
        for e in &m.d.entities {
            b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [e.pos[0] + dx as f64, e.pos[1], e.pos[2]], nbt: e.nbt.clone() });
        }
        drops.push([1 + dx, k, 3]);
    }
    shared_kill_form(&mut b, &drops, 0, form)?;
    Ok(b.text(&[
        &format!("{modules} farms side by side, {pitch} blocks apart, each with its own villagers, zombie and comparator-ring timer; their"),
        "golems fall to one water channel deep below every farm's sight, which carries them to one lava chamber and one chest.",
    ]))
}

/// M2's front: three scared villagers and blind U1 in pods, the zombie, the hide, the bed
/// observer's circuit.
fn m2_front(n: i32) -> Builder {
    m2_front_hide(n, 48)
}

/// [`m2_front`] with its delay line's 12 repeaters set to `hide` gt in all (12..=48): the hide
/// stays open about that long after U1's bed changes.
fn m2_front_hide(n: i32, hide: i32) -> Builder {
    m2_front_hide_r(n, hide, false)
}

/// [`m2_front_hide`], by roles when `roles` (pods of [`pod_bare`], the zombie's roof uncapped:
/// the module's spot check caps every top by its role).
fn m2_front_hide_r(n: i32, hide: i32, roles: bool) -> Builder {
    let hide = hide.clamp(12, 48);
    let delay = |k: i32| (hide / 12 + i32::from(k < hide % 12)).clamp(1, 4);
    let mut b = Builder::new(&format!("iron-middle-observer-{n}"));
    for x in [0, 2, 4] {
        if roles {
            pod_bare(&mut b, x, 0, true);
        } else {
            pod(&mut b, x, true);
        }
    }
    if roles {
        pod_bare(&mut b, 6, 0, false);
    } else {
        pod(&mut b, 6, false);
    }
    b.set(2, 0, -7, "cobblestone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, STONE);
    if !roles {
        b.set(2, 3, -7, GLASS);
    }
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
        b.set(x, 0, -2, &format!("repeater[delay={},facing=west,locked=false,powered=false]", delay(x - 9)));
    }
    support(&mut b, 14, -2);
    b.set(14, 0, -2, "redstone_wire[east=none,north=none,power=0,south=side,west=side]");
    for z in -1..=5 {
        support(&mut b, 14, z);
        b.set(14, 0, z, &format!("repeater[delay={},facing=north,locked=false,powered=false]", delay(z + 6)));
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
    b
}

/// M2: the bed-observer front over L2d's dry hole, the 16-block drop and the fire-safe chamber.
fn m2_module(n: i32) -> Builder {
    m2_module_walls(n, -7)
}

/// M2 with the drop's walls only below `top` (`dry_hole_walls`).
fn m2_module_walls(n: i32, top: i32) -> Builder {
    m2_module_walls_hide(n, top, 48)
}

/// [`m2_module_walls`] with a hide of `hide` gt ([`m2_front_hide`]).
fn m2_module_walls_hide(n: i32, top: i32, hide: i32) -> Builder {
    let mut b = m2_front_hide(n, hide);
    let (wl, k) = (-6, -22);
    crate::layouts::dry_hole_walls(&mut b, n, wl, k, top);
    chamber_safe(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b
}

/// Middle, M2-open (the pick, brief revision 8): M2 with its drop walled only in its bottom 4
/// layers (below y -19): in the void a golem leaving the dry center falls straight down the
/// open shaft, so 234 of M2's blocks were walls nothing touches.
pub fn m2_open(n: i32) -> String {
    m2_open_hide(n, 48)
}

/// [`m2_open`] with a hide of `hide` gt (the delay line's repeaters, 12..=48), by roles: cobblestone
/// walls, floors and roofs, every top a golem could spawn on capped by its role.
pub fn m2_open_hide(n: i32, hide: i32) -> String {
    let mut b = m2_front_hide_r(n, hide, true);
    let (wl, k) = (-6, -22);
    crate::layouts::dry_hole_walls(&mut b, n, wl, k, -19);
    chamber_safe(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    if let Err(e) = crate::parts::cap_spawns(&mut b, &|p| p[1] <= wl + 1) {
        eprintln!("m2-open: the golem spot check failed ({e}): its tops stay uncapped");
    }
    b.d.name = if hide == 48 { format!("iron-middle-open-{n}") } else { format!("iron-middle-open-{n}-hide{hide}") };
    b.text(&["Middle: the bed observer's pulse, stretched by a line of repeaters, hides the zombie while the villagers lie down; the", "golems spawn on a dry hole over an open drop (walled in its bottom 4 layers) into a chamber whose lava cannot light anything."])
}

/// LP's upper cluster (brief, Stages: Late): M2's front, its golems sent up. The pad sits at the
/// spawn search's top, golem feet 6 above the beds (stone at 5, water at 6 from the corner
/// sources), pushing every golem to the 2 x 2 center: a soul-sand bubble column (feet 6 to 20)
/// behind open fence gates at 7 and 8 (they hold the column's water and let a golem in) and walls
/// above. At 21, outside the cluster's own follow cube (it ends 2.51 + 17.84 above the beds), the
/// water from the two south corners pushes the risen golem off the column north into the
/// fire-safe chamber (feet 21).
fn up_module(_n: i32) -> Builder {
    let mut b = m2_front(_n);
    let (fl, wl, k) = (5, 6, 21);
    let center = |x: i32, z: i32| (2..=3).contains(&x) && (4..=5).contains(&z);
    let ring = |x: i32, z: i32| (1..=4).contains(&x) && (3..=6).contains(&z);
    // the pad: the 12 cells around the column (every golem spawned on one overlaps the column and
    // rises at once), its water flowing from the column, walls 3 high around it
    for x in 1..=4 {
        for z in 3..=6 {
            b.set(x, fl, z, if center(x, z) { "soul_sand" } else { STONE });
        }
    }
    for y in wl..=wl + 2 {
        for x in 0..=5 {
            b.set(x, y, 2, GLASS);
            b.set(x, y, 7, GLASS);
        }
        for z in 3..=6 {
            b.set(0, y, z, GLASS);
            b.set(5, y, z, GLASS);
        }
    }
    // the shaft's walls around the column and its gate ring, up to the chamber
    for y in (wl + 3)..k {
        for x in 0..=5 {
            b.set(x, y, 2, STONE);
            b.set(x, y, 7, STONE);
        }
        for z in 3..=6 {
            b.set(0, y, z, STONE);
            b.set(5, y, z, STONE);
        }
    }
    for y in wl..k {
        for x in 1..=4 {
            for z in 3..=6 {
                if center(x, z) {
                    b.set(x, y, z, "bubble_column[drag=false]");
                } else if ring(x, z) && y > wl {
                    // open gates around the column all the way up: dry and open at the pad's
                    // level (the pad's water flows toward them, like L2d's dry center), holding
                    // the column's water, and letting a golem rise with any part of it in the
                    // column (a wall would catch a golem not centered on it)
                    b.set(x, y, z, GATE_E);
                }
            }
        }
    }
    // the chamber's floor around the column top and its walls (x 0..5, z 2..7), the chamber; over
    // the column and its gate ring the floor stays open (a golem rises through it off-center: the
    // chamber's water, from the two south corners, carries it north onto chamber_safe's floor)
    for x in 0..=5 {
        for z in 2..=7 {
            if !center(x, z) && !ring(x, z) {
                b.set(x, k - 1, z, STONE);
            }
            for y in k..=k + 3 {
                let inside = ring(x, z) || ((1..=4).contains(&x) && z == 2 && y <= k + 2);
                if !inside {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    for x in 1..=4 {
        for z in 3..=6 {
            b.set(x, k + 3, z, STONE);
        }
    }
    for (x, z) in [(1, 6), (4, 6)] {
        b.set(x, k, z, "water[level=0]");
    }
    chamber_safe(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b
}

/// LP (brief, Stages: Late): `pairs` pairs, each a lower M2 cluster (golems down) and an upper one
/// (`up_module`, golems up) `gap` blocks over it, the pairs `period` apart; each module keeps its
/// own chamber and chest (the first's is the output).
/// LT, the twin (brief, Stages: Late): two M2 fronts back to back, the second turned 180 degrees
/// about (3, 5.5) so its villagers stand 11 from the first's (out of each other's 10-box) and its
/// zombie lies south; one pad between them (x 0..5, z 2..9, golem feet 6 below the beds) that both
/// clusters' spawn searches reach, one dry hole, one drop and one chamber. A golem on the shared
/// pad sits in the other cluster's follow cubes until it falls: that coupling is the price of
/// the shared parts, paid by `extra` blind villagers per cluster (3 of up to 5 must want a golem).
/// `chamber_safe` draining into the stack's chute instead of its hoppers: the floor closed under
/// the north row, an opening in the north wall at (1, k, 0) over a solid block, and the chute's
/// top (1, k, -1) walled on three sides: the chamber's water runs north into the opening and
/// falls down the chute (a column free through every module), carrying the drops; a golem does not
/// fit the opening.
pub fn drain_chamber(b: &mut Builder, k: i32) {
    chamber_safe(b, k);
    b.d.ports.remove("output");
    b.set(2, k - 1, 1, STONE);
    b.set(3, k - 1, 1, STONE);
    // the opening in the north wall and a 1-wide channel north of it (its walls take the drops'
    // sideways speed), ending at a wall sign: flowing water cannot enter a sign's cell
    // (`FlowingFluid.canHoldFluid`), a drop has no collision with it, so the drops the stream
    // carries pass on and fall down the air column (x, y < k, CHUTE_Z) under gravity (no water
    // column: falling water pushes down only beside a solid face, `FlowingFluid.getFlow`)
    b.d.blocks.remove(&[DRAIN_X, k, 0]);
    b.set(DRAIN_X, k - 1, 0, STONE);
    b.set(DRAIN_X, k - 1, -1, STONE);
    for x in [DRAIN_X - 1, DRAIN_X + 1] {
        b.set(x, k, -1, STONE);
        b.set(x, k + 1, -1, STONE);
    }
    b.set(DRAIN_X, k + 1, -1, STONE);
    b.set(DRAIN_X, k, CHUTE_Z, "oak_wall_sign[facing=south,waterlogged=false]");
    // the column's top walled for 4 layers (the drops leave the sign moving north)
    for y in (k - 4)..=(k + 1) {
        b.set(DRAIN_X, y, CHUTE_Z - 1, STONE);
        if y < k {
            b.set(DRAIN_X - 1, y, CHUTE_Z, STONE);
            b.set(DRAIN_X + 1, y, CHUTE_Z, STONE);
        }
    }
    for x in [DRAIN_X - 1, DRAIN_X + 1] {
        b.set(x, k, CHUTE_Z, STONE);
        b.set(x, k + 1, CHUTE_Z, STONE);
    }
    b.set(DRAIN_X, k + 1, CHUTE_Z, STONE);
}

/// The chute's column (x, y, CHUTE_Z): the chamber's two flows meet at x 2..3 of its north row.
const DRAIN_X: i32 = 2;
const CHUTE_Z: i32 = -2;

/// The stack's shared collection under its lowest module (brief, Stages: Late: 8 h AFK, low lag):
/// the chute's water lands on the line hopper L at (1, y_land - 1, -1) (walls hold it); the repo's
/// archived sorter slice (sorter-classic) sends the iron down to the archived iron-block crafter
/// (ironblock-v24, mirrored in z), whose blocks drop into a double chest (the output); every
/// other drop (the poppies) passes east into a hopper over a composter, whose bone meal a hopper
/// moves into a chest. 5 hoppers for the whole stack.
pub fn collect_bottom(b: &mut Builder, y_land: i32) {
    let mut t = Builder::new("collect");
    collect_bottom_at1(&mut t, y_land);
    let (dx, dz) = (DRAIN_X - 1, CHUTE_Z + 1);
    for (p, s) in &t.d.blocks {
        b.d.blocks.insert([p[0] + dx, p[1], p[2] + dz], s.clone());
    }
    for (p, s) in &t.d.block_entities {
        b.d.block_entities.insert([p[0] + dx, p[1], p[2] + dz], s.clone());
    }
    let out = t.d.ports["output"];
    b.d.ports.insert("output".into(), [out[0] + dx, out[1], out[2] + dz]);
}

/// `collect_bottom` with the chute at x 1, z -1 (its drops land on the line hopper L).
pub fn collect_bottom_at1(b: &mut Builder, y_land: i32) {
    let yl = y_land - 1;
    let dust = "redstone_wire[east=side,north=side,power=0,south=side,west=side]";
    let named = |item: &str, name: &str, n: i32| format!("{{Items:[{{Slot:0b,id:\"minecraft:{item}\",count:{n}}},{}]}}", (1..=4).map(|k| format!("{{Slot:{k}b,id:\"minecraft:paper\",count:1,components:{{\"minecraft:custom_name\":\"{name}\"}}}}")).collect::<Vec<_>>().join(","));
    // a sorter slice (sorter-classic) with its filter hopper A at `a`, its redstone running south
    // (dz 1) or, mirrored, north (dz -1): (sx, sy, sz) -> a + (sx, sy - 2, dz sz)
    let slice = |b: &mut Builder, a: [i32; 3], dz: i32, item: &str, tag: &str| {
        let (n, s_) = if dz > 0 { ("north", "south") } else { ("south", "north") };
        let s = |b: &mut Builder, sx: i32, sy: i32, sz: i32, st: &str| b.set(a[0] + sx, a[1] - 2 + sy, a[2] + dz * sz, st);
        s(b, 0, 2, 0, "hopper[enabled=true,facing=down]");
        s(b, 0, 1, 0, "hopper[enabled=false,facing=down]");
        s(b, 0, 2, 1, &format!("comparator[facing={n},mode=compare,powered=false]"));
        for (sx, sy, sz) in [(0, 1, 1), (0, 1, 2), (0, 1, 3), (0, 0, 2), (0, 0, 4), (0, -1, 3)] {
            s(b, sx, sy, sz, STONE);
        }
        s(b, 0, 0, 1, &format!("redstone_wall_torch[facing={n},lit=true]"));
        s(b, 0, 0, 3, &format!("repeater[delay=1,facing={s_},locked=false,powered=false]"));
        for (sx, sy, sz) in [(0, 2, 2), (0, 2, 3), (0, 1, 4)] {
            s(b, sx, sy, sz, dust);
        }
        b.d.block_entities.insert(a, named(item, &format!("{tag} filter"), 41));
        b.d.block_entities.insert([a[0], a[1] - 1, a[2]], named(item, &format!("{tag} lock"), 64));
    };
    // the line hopper L under the chute pushes east into the poppy slice's filter hopper (which
    // takes nothing but poppies) and the iron slice's filter under L pulls the iron: no drop can
    // pass the wrong way; the two slices' redstone runs on opposite sides (no crosstalk)
    b.set(1, yl, -1, "hopper[enabled=true,facing=east]");
    slice(b, [1, yl - 1, -1], 1, "iron_ingot", "iron");
    slice(b, [2, yl, -1], -1, "poppy", "poppy");
    // the iron slice drops into a chest over the crafter's hopper; the poppy slice into a chest
    // that a hopper empties into a second (no composter: a farmer's job site a farm villager could
    // take; two chests hold 15 h of two modules' poppies)
    b.set(1, yl - 3, -1, "chest[facing=north,type=single,waterlogged=false]");
    b.set(2, yl - 2, -1, "chest[facing=north,type=single,waterlogged=false]");
    b.set(2, yl - 3, -1, "hopper[enabled=true,facing=east]");
    b.set(3, yl - 3, -1, "chest[facing=north,type=single,waterlogged=false]");
    // the crafter (ironblock-v24) mirrored in z under the iron chest: (cx, cy, cz) -> (cx + 2, cy + yl - 4, -cz - 1)
    let c = |b: &mut Builder, cx: i32, cy: i32, cz: i32, st: &str| b.set(cx + 2, cy + yl - 4, -cz - 1, st);
    c(b, -1, 0, 0, "hopper[enabled=true,facing=east]");
    c(b, 0, 0, 0, "crafter[crafting=false,orientation=down_south,triggered=false]");
    c(b, 1, 0, 0, "comparator[facing=west,mode=compare,powered=false]");
    c(b, 2, 0, 0, dust);
    c(b, 0, 0, -1, STONE);
    c(b, 1, 0, -1, "repeater[delay=1,facing=east,locked=false,powered=false]");
    c(b, 2, 0, -1, dust);
    c(b, -1, 0, 1, "crafter[crafting=false,orientation=south_up,triggered=false]");
    c(b, 0, 0, 1, "comparator[facing=west,mode=compare,powered=false]");
    c(b, 1, 0, 1, dust);
    for (cx, cz) in [(1, -1), (2, -1), (1, 0), (2, 0), (0, 1), (1, 1)] {
        c(b, cx, -1, cz, STONE);
    }
    b.d.block_entities.insert([1, yl - 4, -2], "{disabled_slots:[I;0,1,2,3,4,5,6,7,8]}".into());
    // the iron blocks' chest under the crafter (27 stacks: 15,552 iron)
    b.set(2, yl - 5, -1, "chest[facing=north,type=single,waterlogged=false]");
    b.d.ports.insert("output".into(), [2, yl - 5, -1]);
}

/// LS-open (brief, Stages: Late): M2-open modules (the drop walled only in its bottom 4 layers)
/// `pitch` apart vertically, every chamber draining into one chute and one shared collection.
pub fn stack_collect(modules: i32, pitch: i32, n: i32) -> String {
    let mut m = m2_front(n);
    let (wl, k) = (-6, -22);
    crate::layouts::dry_hole_walls(&mut m, n, wl, k, -19);
    drain_chamber(&mut m, k);
    let m = m.d;
    let mut b = Builder::new(&format!("iron-late-shared-{modules}x{pitch}"));
    for i in 0..modules {
        let dy = pitch * i;
        for (p, s) in &m.blocks {
            b.d.blocks.insert([p[0], p[1] + dy, p[2]], s.clone());
        }
        for e in &m.entities {
            b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [e.pos[0], e.pos[1] + dy as f64, e.pos[2]], nbt: e.nbt.clone() });
        }
    }
    collect_bottom(&mut b, k - 4);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[&format!("Late (candidate LS-open): {modules} M2-open modules {pitch} apart sharing one chute, one sorter, one"), "iron-block crafter and one composter."])
}
/// cell at `at` (turn 1: facing east, turn 3: facing west).
fn pod_turned(b: &mut Builder, at: [i32; 3], turns: i32, sees: bool) {
    let mut p = Builder::new("pod");
    pod(&mut p, 0, sees);
    let rot = |x: i32, z: i32| (0..turns.rem_euclid(4)).fold((x, z), |(x, z), _| (-z, x));
    let rotf = |x: f64, z: f64| (0..turns.rem_euclid(4)).fold((x, z), |(x, z), _| (1.0 - z, x));
    for (q, s) in &p.d.blocks {
        let (x, z) = rot(q[0], q[2]);
        let st = mctas::blockstate::BlockState::parse(s).map(|st| st.rotate_y(turns).to_string()).unwrap_or_else(|_| s.clone());
        b.d.blocks.insert([at[0] + x, at[1] + q[1], at[2] + z], st);
    }
    for e in &p.d.entities {
        let (x, z) = rotf(e.pos[0], e.pos[2]);
        b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [at[0] as f64 + x, at[1] as f64 + e.pos[1], at[2] as f64 + z], nbt: e.nbt.clone() });
    }
}

/// A strip pad on the far side of a TZ cluster (x0..x0+3, z -5..6, golem feet 6 below the beds):
/// L2d's dry hole squeezed to 4 wide (the center x0+1..x0+2, z 0..1; the ring the whole width,
/// z -1..2), the borders north and south the spawn cells, sources in the 4 corners, glass walls;
/// under it M2's open drop and fire-safe chamber (north of the drop), moved there.
fn strip_pad(b: &mut Builder, x0: i32, output: bool) {
    let (wl, fl, k) = (-6, -7, -22);
    for u in 0..4 {
        let x = x0 + u;
        for z in -5..=6 {
            let center = (1..=2).contains(&u) && (0..=1).contains(&z);
            let ring = (-1..=2).contains(&z);
            if center {
                b.set(x, wl, z, GATE_N);
            } else if ring {
                b.set(x, fl, z, GATE_N);
            } else {
                b.set(x, fl, z, STONE);
            }
        }
    }
    for (x, z) in [(x0, -5), (x0 + 3, -5), (x0, 6), (x0 + 3, 6)] {
        b.set(x, wl, z, "water[level=0]");
    }
    for y in wl..=wl + 1 {
        for x in (x0 - 1)..=(x0 + 4) {
            b.set(x, y, -6, GLASS);
            b.set(x, y, 7, GLASS);
        }
        for z in -5..=6 {
            b.set(x0 - 1, y, z, GLASS);
            b.set(x0 + 4, y, z, GLASS);
        }
    }
    let mut m = Builder::new("drop");
    crate::layouts::dry_hole_walls(&mut m, 6, wl, k, -8);
    chamber_safe(&mut m, k);
    let (dx, dz) = (x0 - 1, -4);
    for (p, s) in &m.d.blocks {
        if p[1] < fl {
            b.d.blocks.insert([p[0] + dx, p[1], p[2] + dz], s.clone());
        }
    }
    let out = m.d.ports["output"];
    let out = [out[0] + dx, out[1], out[2] + dz];
    if output {
        b.d.ports.insert("output".into(), out);
    } else {
        b.set(out[0], out[1], out[2], "chest[facing=north,type=single,waterlogged=false]");
    }
}

/// TZ (brief, Stages: Late): two clusters share one zombie, one hide and one bed observer, with no
/// coupling. The zombie lies in its boat at (0, 0, 0) with a closed bottom-half trapdoor on each
/// side (x -1 and 1, the hide: opened, each stands across its side's lines of sight). The west
/// cluster (3 scared villagers in pods at x -7, z -2, 0, 2, facing east, and blind U1 at z 4) and
/// the east one (mirrored, a blind 4th at z 4) stand 14 apart, out of each other's 10-box, each
/// within 8 of the zombie. U1's bed observer runs M2's bulb and 48 gt delay, and the hide line
/// powers both trapdoors' blocks. Each cluster's pad lies on its far side (x -15..-12 and 12..15),
/// so its golems spawn, fall and die 18.84 or more (along x) from the other cluster's villagers:
/// never in their follow cube.
pub fn tz(n: i32) -> String {
    let _ = n;
    let mut b = Builder::new("iron-late-tz");
    // the zombie and the hide
    b.set(0, 0, 0, "cobblestone_slab[type=bottom,waterlogged=false]");
    b.set(0, 2, 0, STONE);
    b.set(0, 3, 0, GLASS);
    b.entity("oak_boat", [0.5, 0.5, 0.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(-1, 1, 0, "oak_trapdoor[facing=east,half=bottom,open=false,powered=false,waterlogged=false]");
    b.set(1, 1, 0, "oak_trapdoor[facing=west,half=bottom,open=false,powered=false,waterlogged=false]");
    b.set(-1, 0, 0, STONE);
    b.set(1, 0, 0, STONE);
    // the clusters
    for z in [-2, 0, 2] {
        pod_turned(&mut b, [-7, 0, z], 1, true);
        pod_turned(&mut b, [7, 0, z], 3, true);
    }
    pod_turned(&mut b, [-7, 0, 4], 1, false);
    pod_turned(&mut b, [7, 0, 4], 3, false);
    // the bed observer on U1's bed head, the bulb, the comparator, the junction
    let support = |b: &mut Builder, x: i32, z: i32| b.set(x, -1, z, STONE);
    b.set(-7, 0, 5, "observer[facing=north,powered=false]");
    b.set(-7, 0, 6, "waxed_copper_bulb[lit=false,powered=false]");
    b.set(-7, 1, 6, GLASS);
    support(&mut b, -6, 6);
    b.set(-6, 0, 6, "comparator[facing=west,mode=compare,powered=false]");
    support(&mut b, -5, 6);
    b.set(-5, 0, 6, "redstone_wire[east=side,north=side,power=0,south=none,west=side]");
    // the delay: 12 repeaters east along z 6, back west along z 8 into the bulb's south face
    for x in -4..=7 {
        support(&mut b, x, 6);
        b.set(x, 0, 6, "repeater[delay=4,facing=west,locked=false,powered=false]");
    }
    for (x, z, s) in [(8, 6, "redstone_wire[east=none,north=none,power=0,south=side,west=side]"), (8, 7, "redstone_wire[east=none,north=side,power=0,south=side,west=none]"), (8, 8, "redstone_wire[east=none,north=side,power=0,south=none,west=side]")] {
        support(&mut b, x, z);
        b.set(x, 0, z, s);
    }
    for x in -7..=7 {
        support(&mut b, x, 8);
        let s = match x {
            0 => "repeater[delay=1,facing=east,locked=false,powered=false]",
            -7 => "redstone_wire[east=side,north=side,power=0,south=none,west=none]",
            _ => "redstone_wire[east=side,north=none,power=0,south=none,west=side]",
        };
        b.set(x, 0, 8, s);
    }
    support(&mut b, -7, 7);
    b.set(-7, 0, 7, "repeater[delay=1,facing=south,locked=false,powered=false]");
    // the hide line: north from the junction, east along z 2, a repeater into each trapdoor's block
    for z in 2..=5 {
        support(&mut b, -5, z);
        b.set(-5, 0, z, if z == 2 { "redstone_wire[east=side,north=none,power=0,south=side,west=none]" } else { "redstone_wire[east=none,north=side,power=0,south=side,west=none]" });
    }
    for x in -4..=1 {
        support(&mut b, x, 2);
        let s = match x {
            -1 => "redstone_wire[east=side,north=side,power=0,south=none,west=side]",
            1 => "redstone_wire[east=none,north=side,power=0,south=none,west=side]",
            _ => "redstone_wire[east=side,north=none,power=0,south=none,west=side]",
        };
        b.set(x, 0, 2, s);
    }
    for x in [-1, 1] {
        support(&mut b, x, 1);
        b.set(x, 0, 1, "repeater[delay=1,facing=south,locked=false,powered=false]");
    }
    strip_pad(&mut b, -15, true);
    strip_pad(&mut b, 12, false);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&["Late (candidate TZ): two clusters share one zombie, one hide and one bed observer; each one's pad, drop and", "chamber lie on its far side, out of the other's follow cube."])
}

pub fn twin(n: i32, extra: i32) -> String {
    let mut front = m2_front(n);
    for k in 0..extra {
        // blind redundancy pods west of the scared ones (x -2, -4, ...)
        pod(&mut front, -2 - 2 * k, false);
    }
    let front = front.d;
    let mut b = Builder::new(&format!("iron-late-twin-{extra}"));
    for (p, s) in &front.blocks {
        b.d.blocks.insert(*p, s.clone());
        let r = mctas::blockstate::BlockState::parse(s).map(|st| st.rotate_y(2).to_string()).unwrap_or_else(|_| s.clone());
        b.d.blocks.insert([6 - p[0], p[1], 11 - p[2]], r);
    }
    for e in &front.entities {
        b.d.entities.push(e.clone());
        b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [7.0 - e.pos[0], e.pos[1], 12.0 - e.pos[2]], nbt: e.nbt.clone() });
    }
    // the shared pad, L2d's dry hole stretched to 6 x 8: the center x 2..3, z 5..6 (gates at the
    // water's level), the ring x 1..4, z 4..7 (gates under the water), stone elsewhere, sources
    // in the 4 corners, glass walls
    let (wl, fl, k) = (-6, -7, -22);
    let center = |x: i32, z: i32| (2..=3).contains(&x) && (5..=6).contains(&z);
    let ring = |x: i32, z: i32| (1..=4).contains(&x) && (4..=7).contains(&z);
    for x in 0..=5 {
        for z in 2..=9 {
            if center(x, z) {
                b.set(x, wl, z, GATE_N);
            } else if ring(x, z) {
                b.set(x, fl, z, GATE_N);
            } else {
                b.set(x, fl, z, STONE);
            }
        }
    }
    for (x, z) in [(0, 2), (5, 2), (0, 9), (5, 9)] {
        b.set(x, wl, z, "water[level=0]");
    }
    for y in wl..=wl + 1 {
        for x in -1..=6 {
            b.set(x, y, 1, GLASS);
            b.set(x, y, 10, GLASS);
        }
        for z in 2..=9 {
            b.set(-1, y, z, GLASS);
            b.set(6, y, z, GLASS);
        }
    }
    // the drop and the chamber: M2's, one block further south (the hollow under the ring)
    let mut m = Builder::new("twin-drop");
    crate::layouts::dry_hole_walls(&mut m, 6, wl, k, -19);
    chamber_safe(&mut m, k);
    for (p, s) in &m.d.blocks {
        if p[1] < fl {
            b.d.blocks.insert([p[0], p[1], p[2] + 1], s.clone());
        }
    }
    let out = m.d.ports["output"];
    b.d.ports.insert("output".into(), [out[0], out[1], out[2] + 1]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[&format!("Late (candidate LT): two M2 clusters back to back ({} villagers each) share one pad, one drop and", 4 + extra), "one chamber."])
}

pub fn lp(pairs: i32, gap: i32, period: i32, n: i32) -> String {
    let lower = m2_module(n).d;
    let upper = up_module(n).d;
    let mut b = Builder::new(&format!("iron-late-lp-{pairs}x{gap}-{period}"));
    b.d.meta = lower.meta.clone();
    let mut first = true;
    for p in 0..pairs {
        for (m, dy) in [(&lower, p * period), (&upper, p * period + gap)] {
            for (q, s) in &m.blocks {
                b.d.blocks.insert([q[0], q[1] + dy, q[2]], s.clone());
            }
            for e in &m.entities {
                b.d.entities.push(mctas::design::EntitySpec { kind: e.kind.clone(), pos: [e.pos[0], e.pos[1] + dy as f64, e.pos[2]], nbt: e.nbt.clone() });
            }
            let out = m.ports["output"];
            if first {
                b.d.ports.insert("output".into(), [out[0], out[1] + dy, out[2]]);
                first = false;
            } else {
                b.set(out[0], out[1] + dy, out[2], "chest[facing=north,type=single,waterlogged=false]");
            }
        }
    }
    b.text(&[&format!("Late (candidate LP): {pairs} pairs of clusters {gap} apart, the lower one's golems falling, the upper one's"), "rising in a bubble column, so no golem is ever in the other cluster's follow cube."])
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
