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
    // the zombie (D1's) and the hide facing west (D4's)
    b.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
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
    b.set(lx, k - 1, lz, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [lx, k - 2, lz]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.d.meta.insert("terrain".into(), "cells".into());
    b.d.meta.insert("ground".into(), "8".into());
    b.d.meta.insert("afk".into(), "2.5 9 4.5".into());
    b.text(&["Day 1 (candidate D1U): D4 dug into the ground, natural rock for every wall: the bed plate hides the zombie", "from two scared villagers 6 below the surface; golems fall from the pad into a pit and burn under one lava source."])
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
    b.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
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
    b.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
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
fn m2_front_timer(n: i32, t: &crate::parts::Timer) -> Option<(Builder, Vec<[i32; 3]>)> {
    let mut best: Option<(Builder, Vec<[i32; 3]>)> = None;
    // the bed observer east of U1's bed head (its pulse goes east) or north of it (north)
    for (obs, facing, out) in [([7, 0, 0], "west", [1, 0, 0]), ([6, 0, -1], "south", [0, 0, -1])] {
        let mut base = Builder::new(&format!("iron-middle-{}-{n}", t.row.name));
        for x in [0, 2, 4] {
            pod(&mut base, x, true);
        }
        pod(&mut base, 6, false);
        base.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
        base.set(2, 2, -7, STONE);
        base.set(2, 3, -7, GLASS);
        base.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
        base.set(2, 1, -6, "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]");
        base.set(obs[0], obs[1], obs[2], &format!("observer[facing={facing},powered=false]"));
        let step = |p: [i32; 3], k: i32| [p[0] + k * out[0], p[1], p[2] + k * out[2]];
        let mut source = vec![obs];
        let input = match t.stretch {
            Some(d) => {
                let r = step(obs, 1);
                base.set(r[0], r[1] - 1, r[2], STONE);
                // `facing` names the repeater's input side: the observer's
                base.set(r[0], r[1], r[2], &format!("repeater[delay={d},facing={facing_back},locked=false,powered=false]", facing_back = if out[0] == 1 { "west" } else { "south" }));
                source.push(r);
                step(obs, 2)
            }
            None => step(obs, 1),
        };
        let beside = |a: [i32; 3], c: [i32; 3]| (a[0] - c[0]).abs() + (a[1] - c[1]).abs() + (a[2] - c[2]).abs() == 1;
        for turns in 0..4u8 {
            let mut b = Builder { d: base.d.clone() };
            let Ok(placed) = crate::parts::place(&mut b, t.row, turns, input) else { continue };
            // the part in free cells, east of the pods or low between them and the zombie (under
            // the villagers' lines of sight), clear of the hide's cell and the zombie's row, and
            // touching the source only at its input cell
            let region = |c: &[i32; 3]| c[0] >= 8 && (-5..=8).contains(&c[2]) || c[1] <= 0 && (-1..=7).contains(&c[0]) && (-5..=-1).contains(&c[2]);
            let fits = placed.cells.iter().all(|c| (*c == input || !base.d.blocks.contains_key(c)) && region(c) && (*c == input || source.iter().all(|s| !beside(*s, *c))));
            if !fits {
                continue;
            }
            let Some(start) = crate::parts::read_output(&mut b, &placed) else { continue };
            let out_rep = placed.output;
            let forbid = |p: [i32; 3]| p[0] < -1 || p[0] > 20 || p[2] < -6 || p[2] > 10 || (p[2] >= 0 && p[0] < 8) || (p[2] == -6 && p[0] < 4);
            let Some(path) = crate::parts::route(&mut b, start, [4, 0, -6], &forbid, &placed.cells, &[out_rep, [3, 0, -6]], 1) else { continue };
            // the climb onto the block east of the hide, the dust there pointing into it
            b.set(4, 0, -6, &b.d.blocks[&[4, 0, -6]].replace("west=none", "west=side"));
            b.set(3, 0, -6, STONE);
            b.set(3, 1, -6, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
            if best.as_ref().is_none_or(|(bb, _)| b.d.blocks.len() < bb.d.blocks.len()) {
                best = Some((b, path));
            }
        }
    }
    best
}

/// M2-open with its hide timed by the smallest catalog part that fits ([`m2_front_timer`]):
/// every part whose pulse from the bed observer lasts `l_min` gt or more with `stage`'s
/// materials ([`crate::parts::timers`]), fewest blocks first, the first that fits.
pub fn m2_open_timer(n: i32, l_min: u32, stage: crate::parts::Stage) -> anyhow::Result<String> {
    let rows = crate::parts::catalog()?;
    let timers = crate::parts::timers(&rows, l_min, 400, 2, stage);
    for t in &timers {
        let Some((mut b, path)) = m2_front_timer(n, t) else { continue };
        let (wl, k) = (-6, -22);
        crate::layouts::dry_hole_walls(&mut b, n, wl, k, -19);
        chamber_safe(&mut b, k);
        b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
        b.d.meta.insert("restart".into(), "yes".into());
        // the pad's spots stay (feet at wl + 1 and below); a part's top or a support gets glass
        let caps = crate::parts::cap_spawns(&mut b, &|p| p[1] <= wl + 1)?;
        let part = format!("hide {}{}", t.row.name, if t.stretch.is_some() { " + a repeater" } else { "" });
        b.d.meta.insert("parts".into(), part.clone());
        return Ok(b.text(&[
            &format!("Middle (M2-open, its hide from the catalog): the bed observer's pulse, stretched to {} gt by {} ({} blocks with", t.length, t.row.name, t.blocks),
            &format!("its wiring of {} dust and {caps} glass caps), hides the zombie while the villagers lie down; the dry hole, the open drop and the fire-safe chamber.", path.len()),
        ]));
    }
    anyhow::bail!("no catalog part of {} gives a pulse of {l_min} gt or more from the bed observer that fits ({} tried)", match stage {
        crate::parts::Stage::Day1 { .. } => "day 1",
        crate::parts::Stage::Nether => "the Nether",
        crate::parts::Stage::End => "the End",
    }, timers.len())
}

/// M2's front: three scared villagers and blind U1 in pods, the zombie, the hide, the bed
/// observer's circuit.
fn m2_front(n: i32) -> Builder {
    m2_front_hide(n, 48)
}

/// [`m2_front`] with its delay line's 12 repeaters set to `hide` gt in all (12..=48): the hide
/// stays open about that long after U1's bed changes.
fn m2_front_hide(n: i32, hide: i32) -> Builder {
    let hide = hide.clamp(12, 48);
    let delay = |k: i32| (hide / 12 + i32::from(k < hide % 12)).clamp(1, 4);
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

/// [`m2_open`] with a hide of `hide` gt (the delay line's repeaters, 12..=48).
pub fn m2_open_hide(n: i32, hide: i32) -> String {
    let mut b = m2_module_walls_hide(n, -19, hide);
    b.d.name = if hide == 48 { format!("iron-middle-open-{n}") } else { format!("iron-middle-open-{n}-hide{hide}") };
    b.text(&["Middle (M2-open): the bed observer hides the zombie for 48 gt when U1 lies down; L2d's dry hole over an", "open 16-block drop (walled in its bottom 4 layers) and a chamber whose lava cannot light anything."])
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
fn drain_chamber(b: &mut Builder, k: i32) {
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
fn collect_bottom(b: &mut Builder, y_land: i32) {
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
fn collect_bottom_at1(b: &mut Builder, y_land: i32) {
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
    // the iron slice drops into a chest over the crafter's hopper; the poppy slice into a composter,
    // its bone meal into a chest
    b.set(1, yl - 3, -1, "chest[facing=north,type=single,waterlogged=false]");
    b.set(2, yl - 2, -1, "composter[level=0]");
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
    b.set(0, 0, 0, "stone_slab[type=bottom,waterlogged=false]");
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
