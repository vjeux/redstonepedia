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
