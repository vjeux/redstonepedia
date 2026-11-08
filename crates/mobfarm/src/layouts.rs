//! Layout generators (`mctas mobfarm gen <layout>`): each writes a design text with its caption.
//! Coordinates: the villagers' bed heads at y = 0, z = 0, north is -z.

use mctas::design::{Design, EntitySpec};
use std::collections::BTreeMap;

/// A design under construction.
pub struct Builder {
    pub d: Design,
}

impl Builder {
    pub fn new(name: &str) -> Builder {
        Builder { d: Design { name: name.to_string(), meta: BTreeMap::new(), ports: BTreeMap::new(), blocks: BTreeMap::new(), entities: Vec::new(), block_entities: BTreeMap::new() } }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, s: &str) {
        let s = if s.starts_with("minecraft:") { s.to_string() } else { format!("minecraft:{s}") };
        self.d.blocks.insert([x, y, z], s);
    }

    pub fn entity(&mut self, kind: &str, pos: [f64; 3], nbt: Option<&str>) {
        self.d.entities.push(EntitySpec { kind: format!("minecraft:{kind}"), pos, nbt: nbt.map(|s| s.to_string()) });
    }

    /// A villager on the head of its bed (facing north) in a closed 1-cell pod at (x, 0, 0): the
    /// foot capped so it cannot stand there, glass walls, an open trapdoor across the pod's
    /// north face at y+1 (its eyes see out over and under it, it cannot pass), a roof of leaves.
    pub fn pod(&mut self, x: i32) {
        self.set(x, 0, 0, "red_bed[facing=north,occupied=false,part=head]");
        self.set(x, 0, 1, "red_bed[facing=north,occupied=false,part=foot]");
        for z in 0..2 {
            self.set(x, -1, z, "glass");
        }
        self.set(x, 1, 1, "glass");
        self.set(x, 2, 1, "glass");
        for w in [x - 1, x + 1] {
            self.set(w, 1, 0, "glass");
            self.set(w, 2, 0, "glass");
        }
        self.set(x, 1, 0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
        for dx in -1..=1 {
            for z in 0..2 {
                self.set(x + dx, 3, z, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
            }
        }
        self.entity("villager", [x as f64 + 0.5, 0.5625, 0.5], None);
    }

    pub fn text(&self, caption: &[&str]) -> String {
        let mut s = String::new();
        for l in caption {
            s.push_str("# ");
            s.push_str(l);
            s.push('\n');
        }
        s.push_str(&self.d.to_text());
        s
    }
}

const STONE: &str = "stone";
const GLASS: &str = "glass";
const LAVA: &str = "lava[level=0]";
const SHUT: &str = "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]";

/// The zombie of every layout: a boat on a slab 7 north of the pods, a roof of leaves over its
/// eyes, and the trapdoor in front of it that hides it when powered.
fn zombie(b: &mut Builder) {
    b.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(2, 1, -6, SHUT);
}

fn wall_sign(b: &mut Builder, x: i32, y: i32, z: i32, facing: &str) {
    b.set(x, y, z, &format!("oak_wall_sign[facing={facing},waterlogged=false]"));
}

/// The dusk pulse: the zombie's trapdoor opens for about 46 gt when night falls, then shuts for
/// the rest of the night, so the villagers fall asleep once (`LAST_SLEPT`) and wake up scared
/// again. Two inverted daylight detectors switch on together at nightfall: D2 (through a dust
/// into B1) turns torch T1 off at once, D reaches K0 through 6 repeaters (46 gt); torch T0 under
/// the trapdoor is lit while K0 is unpowered: between the two. At dawn T1 relights before the
/// repeaters go dark: no pulse. No iron; 6 quartz.
fn dusk_pulse(b: &mut Builder) {
    b.set(2, -1, -6, STONE); // K0
    b.set(2, 0, -6, "redstone_torch[lit=false]"); // T0, under the trapdoor
    b.set(2, -3, -6, STONE); // B1
    b.set(2, -2, -6, "redstone_torch[lit=true]"); // T1, under K0
    b.set(2, -3, -8, "daylight_detector[inverted=true,power=0]"); // D2
    b.set(2, -4, -7, STONE);
    b.set(2, -3, -7, "redstone_wire[east=none,north=side,power=0,south=side,west=none]");
    for x in 3..=8 {
        b.set(x, -2, -6, STONE);
        b.set(x, -1, -6, "repeater[delay=4,facing=east,locked=false,powered=false]");
    }
    b.set(9, -1, -6, "daylight_detector[inverted=true,power=0]"); // D
}

/// L2, the middle ground: L1's pods and zombie, the dusk pulse instead of a hide all night, a 4x4
/// pad around a 2x2 hole (water from its corners), a shaft that drops each golem out of every
/// villager's follow range (feet at -22), and L1's kill chamber at its bottom with 2 hoppers.
pub fn l2(n: i32, fy: i32) -> String {
    let mut b = Builder::new(&format!("iron-middle-{n}"));
    for x in [0, 2, 4] {
        b.pod(x);
    }
    zombie(&mut b);
    dusk_pulse(&mut b);
    let k = -22;
    let h = n / 2;
    let (x0, x1, z0, z1) = (3 - h, 2 + h, 5 - h, 4 + h);
    let hole = |x: i32, z: i32| (2..=3).contains(&x) && (4..=5).contains(&z);
    for x in x0..=x1 {
        for z in z0..=z1 {
            if !hole(x, z) {
                b.set(x, fy, z, STONE);
            }
        }
    }
    for (x, z) in [(x0, z0), (x0, z1), (x1, z0), (x1, z1)] {
        b.set(x, fy + 1, z, "water[level=0]");
    }
    for y in (fy + 1)..=(fy + 2) {
        for x in (x0 - 1)..=(x1 + 1) {
            for z in [z0 - 1, z1 + 1] {
                b.set(x, y, z, GLASS);
            }
        }
        for z in z0..=z1 {
            for x in [x0 - 1, x1 + 1] {
                b.set(x, y, z, GLASS);
            }
        }
    }
    for z in 4..=5 {
        wall_sign(&mut b, 2, fy, z, "east");
        wall_sign(&mut b, 3, fy, z, "west");
    }
    // glass caps the shaft walls the pad does not cover (no spawn spot on their tops)
    for x in 1..=4 {
        for z in 1..=6 {
            if !(x0..=x1).contains(&x) || !(z0..=z1).contains(&z) {
                b.set(x, fy, z, GLASS);
            }
        }
    }
    // the shaft (x 2-3, z 4-5) down to feet level k, the chamber north of it (z 2-3)
    for y in (k - 1)..fy {
        for x in 1..=4 {
            for z in 1..=6 {
                let shaft = (2..=3).contains(&x) && (4..=5).contains(&z) && y >= k;
                let chamber = (2..=3).contains(&x) && (2..=3).contains(&z) && (k..=k + 2).contains(&y);
                let inner = (2..=3).contains(&x) && (2..=5).contains(&z);
                let wall = !inner && (x == 1 || x == 4 || z == 1 || z == 6);
                if !shaft && !chamber && (inner || wall) {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    for z in 2..=3 {
        wall_sign(&mut b, 2, k + 1, z, "east");
        wall_sign(&mut b, 3, k + 1, z, "west");
        b.set(2, k + 2, z, LAVA);
        b.set(3, k + 2, z, LAVA);
    }
    wall_sign(&mut b, 2, k + 2, 4, "east");
    wall_sign(&mut b, 3, k + 2, 4, "west");
    b.set(2, k, 5, "water[level=0]");
    b.set(3, k, 5, "water[level=0]");
    b.set(2, k - 1, 2, "hopper[enabled=true,facing=east]");
    b.set(3, k - 1, 2, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [3, k - 2, 2]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[
        "L2, the middle iron farm: three villagers on their beds in closed 1-cell pods under leaves, a",
        "zombie in a boat 7 blocks north of them in view. At nightfall two inverted daylight detectors open",
        "the trapdoor in front of the zombie for 46 game ticks: the villagers fall asleep, then wake up",
        "scared for the rest of the night (and the farm restarts by itself after a missed night). Golems",
        "spawn on a 4x4 pad around a 2x2 hole (water from its corners, wall signs in the hole stop it) and",
        "fall 18 blocks, out of every villager's sight, to water that pushes them under a lava blade held",
        "by signs, a ceiling right above it, over two hoppers.",
    ])
}

/// L1, the obvious design (the bar): three villagers in 1-cell pods, the zombie in a boat, an
/// inverted daylight detector on the trapdoor in front of it (it hides the zombie all night, so
/// the villagers sleep through every night); an 8 x 6 pad 4 below the villagers with water from
/// its corners to a 2x2 hole at its middle (wall signs in the hole stop the water, golems fall
/// through them); a shaft to a lava blade held by signs 6 below the pad; 4 hoppers under the
/// blade into one chest.
pub fn l1() -> String {
    let mut b = Builder::new("iron-obvious");
    for x in [0, 2, 4] {
        b.pod(x);
    }
    zombie(&mut b);
    b.set(2, 2, -6, "daylight_detector[inverted=true,power=0]");
    let (x0, x1, z0, z1) = (-1, 6, 3, 8);
    let hole = |x: i32, z: i32| (2..=3).contains(&x) && (5..=6).contains(&z);
    for x in x0..=x1 {
        for z in z0..=z1 {
            if !hole(x, z) {
                b.set(x, -4, z, STONE);
            }
        }
    }
    // one source in each corner: no flowing cell ever touches two sources (which would turn it
    // into a source and still the whole pad), and every cell's current runs toward the hole
    for (x, z) in [(x0, z0), (x0, z1), (x1, z0), (x1, z1)] {
        b.set(x, -3, z, "water[level=0]");
    }
    // the pad's rim holds the water and the golems (2 high: they step 1 and jump about 1.25)
    for y in -3..=-2 {
        for x in (x0 - 1)..=(x1 + 1) {
            for z in [z0 - 1, z1 + 1] {
                b.set(x, y, z, GLASS);
            }
        }
        for z in z0..=z1 {
            for x in [x0 - 1, x1 + 1] {
                b.set(x, y, z, GLASS);
            }
        }
    }
    // the hole: wall signs on the pad's blocks around it (water stops, golems fall through)
    wall_sign(&mut b, 2, -4, 5, "east");
    wall_sign(&mut b, 3, -4, 5, "west");
    wall_sign(&mut b, 2, -4, 6, "east");
    wall_sign(&mut b, 3, -4, 6, "west");
    // the shaft (x 2-3, z 5-6) down to feet level -9, its floor in water flowing east into the
    // kill chamber (x 4-5): lava at head height (-7) held by signs at -8 and at the chamber's
    // mouth, a ceiling right above it (-6) so a golem that jumps in the lava cannot float up out
    // of reach of its drops' hoppers
    for y in -10..=-5 {
        for x in 1..=6 {
            for z in 4..=7 {
                let shaft = (2..=3).contains(&x) && (5..=6).contains(&z) && y >= -9;
                let chamber = (4..=5).contains(&x) && (5..=6).contains(&z) && (-9..=-7).contains(&y);
                let mouth = x == 3 && (5..=6).contains(&z) && (-9..=-7).contains(&y);
                if !shaft && !chamber && !mouth {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    for x in 4..=5 {
        wall_sign(&mut b, x, -8, 5, "south");
        wall_sign(&mut b, x, -8, 6, "north");
        b.set(x, -7, 5, LAVA);
        b.set(x, -7, 6, LAVA);
    }
    wall_sign(&mut b, 3, -7, 5, "south");
    wall_sign(&mut b, 3, -7, 6, "north");
    b.set(2, -9, 5, "water[level=0]");
    b.set(2, -9, 6, "water[level=0]");
    b.set(4, -10, 5, "hopper[enabled=true,facing=east]");
    b.set(5, -10, 5, "hopper[enabled=true,facing=down]");
    b.set(4, -10, 6, "hopper[enabled=true,facing=north]");
    b.set(5, -10, 6, "hopper[enabled=true,facing=north]");
    b.d.ports.insert("output".into(), [5, -11, 5]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[
        "L1, the obvious iron farm (the bar): three villagers on their beds in closed 1-cell pods under",
        "leaves, a zombie in a boat 7 blocks north of them in view. An inverted daylight detector opens the",
        "trapdoor in front of the zombie at nightfall, so the villagers sleep through every night (and the",
        "farm restarts by itself after a missed night); it stops in rain (clear weather is not judged).",
        "Golems spawn on an 8x6 pad 4 below the villagers; water from its corners carries them to a 2x2 hole",
        "(wall signs in the hole stop the water); they fall to water that pushes them under a lava blade",
        "held by signs, with a ceiling right above it, over four hoppers, in view of the villagers.",
    ])
}

/// L3, the day-1 farm: three villagers in 1-cell pods, a zombie in a boat 7 blocks north, a row of
/// `pits` 1x1 spawn floors west of the zombie (x = -pits..-1, z = -6) where golems spawn with a
/// lava source beside their head on both sides and burn where they stand. One tripwire string
/// runs through every pit at golem knee height: a golem in any pit powers the hook under the
/// sight-blocking trapdoor in front of the zombie, so the villagers calm down (and sleep at
/// night) while it burns. A water channel under the string carries the drops east to one hopper.
pub fn l3(pits: i32) -> String {
    let mut b = Builder::new(&format!("iron-day1-{pits}"));
    for x in [0, 2, 4] {
        b.pod(x);
    }
    // the zombie: a boat on a slab, a roof of leaves over its eyes
    b.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    // the sight blocker, opened by the hook under it; a closed trapdoor west of it stops golems
    b.set(2, 1, -6, SHUT);
    b.set(2, 0, -6, "tripwire_hook[attached=true,facing=west,powered=false]");
    b.set(3, 0, -6, STONE);
    b.set(1, 1, -6, SHUT);
    // the string from the east hook to the west hook
    let west = -pits - 1;
    for x in (west + 1)..=1 {
        b.set(x, 0, -6, "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
    }
    b.set(west, 0, -6, "tripwire_hook[attached=true,facing=east,powered=false]");
    b.set(west - 1, 0, -6, STONE);
    b.set(west - 1, 1, -6, GLASS);
    b.set(west, 1, -6, GLASS);
    b.set(west, -1, -6, STONE);
    // the pits
    for px in (west + 1)..=-1 {
        b.set(px, -2, -6, STONE);
        b.set(px, 1, -6, SHUT);
        b.set(px, 2, -6, GLASS);
        for z in [-7, -5] {
            b.set(px, -1, z, STONE);
            b.set(px, 0, z, STONE);
            b.set(px, 1, z, LAVA);
            b.set(px, 2, z, GLASS);
        }
        b.set(px, 1, -8, GLASS);
        b.set(px, 1, -4, GLASS);
    }
    for x in [west, 0] {
        for z in [-7, -5] {
            b.set(x, 1, z, GLASS);
            b.set(x, 0, z, STONE);
            b.set(x, -1, z, STONE);
        }
    }
    // the channel: a source at the west pit, dry at x = 0 over the hopper
    b.set(west + 1, -1, -6, "water[level=0]");
    b.set(0, -2, -6, "hopper[enabled=true,facing=down]");
    b.set(0, 1, -6, GLASS);
    b.set(1, -1, -6, STONE);
    b.d.ports.insert("output".into(), [0, -3, -6]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.text(&[
        &format!("L3, the day-1 iron farm ({pits} spawn pits): three villagers on their beds in closed 1-cell pods under"),
        "leaves, a zombie in a boat 7 blocks north of them in view of their standing and sleeping eyes.",
        "Golems spawn in a row of 1x1 pits west of the zombie, a lava source beside their head on both",
        "sides, and burn where they stand. One tripwire runs through every pit at knee height: a golem in",
        "any pit powers the hook under the trapdoor in front of the zombie, the trapdoor opens and hides it,",
        "and the villagers calm down (at night they fall asleep) while the golem burns. A water channel",
        "under the string carries the iron to one hopper. Hide the zombie by hand on the first night: the",
        "villagers need a recent sleep to summon the first golem.",
    ])
}
