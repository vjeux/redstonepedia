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
        Builder { d: Design { name: name.to_string(), meta: BTreeMap::new(), ports: BTreeMap::new(), blocks: BTreeMap::new(), entities: Vec::new(), block_entities: BTreeMap::new(), ..Default::default() } }
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

    /// [`Builder::pod`] in day-1 materials (vjeux 13:09: no glass, leaves or stone on day 1): a
    /// cobblestone floor and walls (the villager sees out over its north trapdoor only), and a
    /// roof of bottom cobblestone slabs (a golem's spawn fails on a half top, as on leaves).
    pub fn pod_day1(&mut self, x: i32) {
        self.set(x, 0, 0, "red_bed[facing=north,occupied=false,part=head]");
        self.set(x, 0, 1, "red_bed[facing=north,occupied=false,part=foot]");
        for z in 0..2 {
            self.set(x, -1, z, STONE);
        }
        self.set(x, 1, 1, STONE);
        self.set(x, 2, 1, STONE);
        for w in [x - 1, x + 1] {
            self.set(w, 1, 0, STONE);
            self.set(w, 2, 0, STONE);
        }
        self.set(x, 1, 0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
        for dx in -1..=1 {
            for z in 0..2 {
                self.set(x + dx, 3, z, CAP);
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

// (roles, designs/farms/roles.tsv: a structural, floor or fluid-holding block is cobblestone, the
// cheapest at every stage's prices; mining stone without silk touch gives cobblestone)
const STONE: &str = "cobblestone";
const GLASS: &str = "glass";
/// A day-1 cap: a bottom cobblestone slab (no golem spawns on its half top; 3 cobblestone make 6).
const CAP: &str = "cobblestone_slab[type=bottom,waterlogged=false]";
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

/// The farm's single chest (at its `port output`: the judge reads it, the download holds it).
pub const CHEST: &str = "minecraft:chest[facing=north,type=single,waterlogged=false]";

/// A design text edited, its leading comment block (the caption) kept: `chest` puts the farm's
/// single chest at its `port output`, `how` sets its `how:` line.
pub fn edit(text: &str, chest: bool, how: Option<&str>) -> anyhow::Result<String> {
    edit_meta(text, chest, how, &[])
}

/// [`edit`], and each `key=value` of `meta` set as a meta line (`sample=village-edge`: judge a
/// void design in that terrain sample; an empty value removes the line).
pub fn edit_meta(text: &str, chest: bool, how: Option<&str>, meta: &[String]) -> anyhow::Result<String> {
    let caption: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    let mut d = Design::parse(text)?;
    for kv in meta {
        let (k, v) = kv.split_once('=').ok_or_else(|| anyhow::anyhow!("--meta {kv:?}: not key=value"))?;
        if v.trim().is_empty() {
            d.meta.remove(k.trim());
        } else {
            d.meta.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    if chest {
        let out = *d.ports.get("output").ok_or_else(|| anyhow::anyhow!("no `port output` for the chest"))?;
        match d.blocks.get(&out) {
            None => {
                d.blocks.insert(out, CHEST.to_string());
            }
            Some(s) if s.trim_start_matches("minecraft:").starts_with("chest[") => {}
            Some(s) => anyhow::bail!("port output {out:?} holds {s}"),
        }
    }
    if let Some(h) = how {
        d.meta.insert("how".to_string(), h.to_string());
    }
    let mut s = String::new();
    for l in caption {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str(&d.to_text());
    Ok(s)
}

/// A design text with its tier on its `circuit:` line (`circuit: mob-farm iron_ingot middle`):
/// the published kind (one site page per tier).
pub fn set_tier(text: &str, tier: &str) -> anyhow::Result<String> {
    let tiers = circuit::mobfarm::TIERS;
    if !tiers.iter().any(|(t, _)| *t == tier) {
        anyhow::bail!("no tier {tier:?} ({})", tiers.iter().map(|(t, _)| *t).collect::<Vec<_>>().join(", "));
    }
    let mut found = false;
    let out: Vec<String> = text
        .lines()
        .map(|l| match l.strip_prefix("circuit: mob-farm ") {
            Some(rest) if !found => {
                found = true;
                let item = rest.split_whitespace().next().unwrap_or("iron_ingot");
                format!("circuit: mob-farm {item} {tier}")
            }
            _ => l.to_string(),
        })
        .collect();
    if !found {
        anyhow::bail!("no `circuit: mob-farm` line to give the tier to");
    }
    Ok(out.join("\n") + "\n")
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
pub fn l2(n: i32, fy: i32, villagers: i32) -> String {
    let mut b = Builder::new(&format!("iron-middle-{n}"));
    if villagers == 4 {
        front4(&mut b);
    } else {
        for x in [0, 2, 4] {
            b.pod(x);
        }
        zombie(&mut b);
        dusk_pulse(&mut b);
    }
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
/// the villagers sleep through every night); a 6 x 5 pad 4 below the villagers whose water flows
/// one way (sources along its far edge) into a 2-deep trench along its near edge (wall signs
/// hold the water over the trench to its wall: the current never stops over a floor); the trench
/// down to water that carries the golems to a lava blade held by signs, a ceiling right above it,
/// 2 hoppers into one chest.
pub fn l1() -> String {
    let mut b = Builder::new("iron-obvious");
    for x in [0, 2, 4] {
        b.pod(x);
    }
    zombie(&mut b);
    b.set(2, 2, -6, "daylight_detector[inverted=true,power=0]");
    let (x0, x1) = (-1, 4);
    // the pad (x 0..3, z 4..8) is one block narrower than the trench on each side: a golem spawned
    // 0.2 into the inner rim meets no wall on its way into the trench, and the outer rim stops it
    // walking out through the inner one (a box ignores the shapes it already overlaps)
    for x in (x0 + 1)..=(x1 - 1) {
        for z in 4..=8 {
            b.set(x, -4, z, STONE);
        }
        b.set(x, -3, 8, "water[level=0]");
    }
    for x in x0..=x1 {
        wall_sign(&mut b, x, -4, 3, "north");
        wall_sign(&mut b, x, -4, 2, "south");
    }
    // the rims: open fence gates beside and behind the pad hold its water (no fluid enters a fence
    // gate) with no collision box, so a golem spawned 0.2 into them meets nothing; glass 2 high
    // (golems step 1 and jump about 1.25) one block further out stops them; the north row holds
    // the trench's signs
    for x in x0..=x1 {
        b.set(x, -4, 9, STONE);
        b.set(x, -3, 9, GATE_N);
    }
    for z in 4..=8 {
        for x in [x0, x1] {
            b.set(x, -4, z, STONE);
            b.set(x, -3, z, GATE_E);
        }
    }
    for y in -4..=-2 {
        for x in (x0 - 1)..=(x1 + 1) {
            b.set(x, y, 1, GLASS);
            if y > -4 {
                b.set(x, y, 10, GLASS);
            }
        }
        if y > -4 {
            for z in 2..=10 {
                b.set(x0 - 1, y, z, GLASS);
                b.set(x1 + 1, y, z, GLASS);
            }
        }
    }
    // the trench (x -1..4, z 2..3) down to feet level -9; at its bottom water flows east into the
    // kill chamber (x 5..6): lava at head height (-7) held by signs at -8 and at its mouth (x 4), a
    // ceiling right above it (-6) so a golem that jumps in the lava cannot float up with its drops
    for y in -10..=-5 {
        for x in (x0 - 1)..=7 {
            for z in 1..=4 {
                let trench = (x0..=x1).contains(&x) && (2..=3).contains(&z) && y >= -9;
                let chamber = (5..=6).contains(&x) && (2..=3).contains(&z) && (-9..=-7).contains(&y);
                let inner = (2..=3).contains(&z) && (x0..=6).contains(&x);
                if !trench && !chamber && (inner || z == 1 || z == 4 || x == x0 - 1 || x == 7) {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    // glass caps the walls the pad does not cover (no spawn spot on their tops)
    for z in 1..=4 {
        b.set(x0 - 1, -4, z, GLASS);
        for x in (x1 + 1)..=7 {
            b.set(x, -4, z, GLASS);
        }
    }
    for x in 5..=6 {
        wall_sign(&mut b, x, -8, 2, "south");
        wall_sign(&mut b, x, -8, 3, "north");
        b.set(x, -7, 2, LAVA);
        b.set(x, -7, 3, LAVA);
    }
    wall_sign(&mut b, 4, -7, 2, "south");
    wall_sign(&mut b, 4, -7, 3, "north");
    b.set(x0, -9, 2, "water[level=0]");
    b.set(x0, -9, 3, "water[level=0]");
    b.set(6, -10, 3, "hopper[enabled=true,facing=north]");
    b.set(6, -10, 2, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [6, -11, 2]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[
        "L1, the obvious iron farm (the bar): three villagers on their beds in closed 1-cell pods under",
        "leaves, a zombie in a boat 7 blocks north of them in view. An inverted daylight detector opens the",
        "trapdoor in front of the zombie at nightfall, so the villagers sleep through every night (and the",
        "farm restarts by itself after a missed night); it stops in rain (clear weather is not judged).",
        "Golems spawn on a 6x5 pad 4 below the villagers; water from its far edge carries them into a trench",
        "along its near edge (wall signs hold the water over it); they fall to water that pushes them under",
        "a lava blade held by signs, with a ceiling right above it, over two hoppers, in view of the villagers.",
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
        b.pod_day1(x);
    }
    // the zombie: a boat on a slab, a slab roof over its eyes (no sky: it never burns)
    b.set(2, 0, -7, CAP);
    b.set(2, 2, -7, CAP);
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
    b.set(west - 1, 1, -6, STONE);
    b.set(west, 1, -6, STONE);
    b.set(west, -1, -6, STONE);
    // the pits (day-1 materials, vjeux 13:09: cobblestone for glass; a bottom cobblestone slab
    // holds a lava source and caps it: a golem's spawn fails on its half top)
    for px in (west + 1)..=-1 {
        b.set(px, -2, -6, STONE);
        b.set(px, 1, -6, SHUT);
        // over the pit's trapdoor ceiling, a cobblestone with a torch on it (glass before): no
        // golem spawns on the trapdoor or the cobblestone (a block is above each), and the
        // golem search's scan passes the torch and goes on down to the pit
        b.set(px, 2, -6, STONE);
        b.set(px, 3, -6, "torch");
        for z in [-7, -5] {
            b.set(px, -1, z, STONE);
            b.set(px, 0, z, STONE);
            b.set(px, 1, z, LAVA);
            b.set(px, 2, z, CAP);
        }
        b.set(px, 1, -8, CAP);
        b.set(px, 1, -4, CAP);
    }
    for x in [west, 0] {
        for z in [-7, -5] {
            b.set(x, 1, z, CAP);
            b.set(x, 0, z, STONE);
            b.set(x, -1, z, STONE);
        }
    }
    // the channel: a source at the west pit, dry at x = 0 over the hopper
    b.set(west + 1, -1, -6, "water[level=0]");
    b.set(0, -2, -6, "hopper[enabled=true,facing=down]");
    b.set(0, 1, -6, STONE);
    b.set(1, -1, -6, STONE);
    b.d.ports.insert("output".into(), [0, -3, -6]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    // every other top a golem could spawn on gets its cap by its role (`parts::cap_spawns`: a torch
    // over a pit's column, so the search scans on down to the pit, else a slab)
    let _ = crate::parts::cap_spawns(&mut b, &|p| p[2] == -6 && p[0] > west && p[0] < 0 && p[1] == -1);
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

/// The 4-villager front of L4: pods at x = 0, 2, 4, 6; the zombie in a boat at x 3.5, 7 north;
/// three trapdoors at z = -5 (1.5 from its eyes: every ray crosses them) opened by the dust on
/// the blocks above them; the dusk pulse drives that dust (D2 into B1 turns T1 off at once, D
/// reaches K0 through 6 repeaters: T0 on K0 is lit for about 46 gt at nightfall).
fn front4(b: &mut Builder) {
    for x in [0, 2, 4, 6] {
        b.pod(x);
    }
    b.set(3, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
    b.set(3, 2, -7, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
    b.entity("oak_boat", [3.5, 0.5, -6.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    for x in 2..=4 {
        b.set(x, 1, -5, "oak_trapdoor[facing=south,half=bottom,open=false,powered=false,waterlogged=false]");
        b.set(x, 2, -5, STONE);
        let (e, w) = (if x < 4 { "side" } else { "side" }, if x > 2 { "side" } else { "none" });
        b.set(x, 3, -5, &format!("redstone_wire[east={e},north=none,power=0,south=none,west={w}]"));
    }
    b.set(5, 2, -5, STONE); // K0
    b.set(5, 3, -5, "redstone_torch[lit=false]"); // T0 feeds the dust
    b.set(5, 0, -5, STONE); // B1
    b.set(5, 1, -5, "redstone_torch[lit=true]"); // T1 under K0
    b.set(5, -1, -4, STONE);
    b.set(5, 0, -4, "redstone_wire[east=none,north=side,power=0,south=side,west=none]");
    b.set(5, 0, -3, "daylight_detector[inverted=true,power=0]"); // D2
    for x in 6..=11 {
        b.set(x, 1, -5, STONE);
        b.set(x, 2, -5, "repeater[delay=4,facing=east,locked=false,powered=false]");
    }
    b.set(12, 2, -5, "daylight_detector[inverted=true,power=0]"); // D
}

const GATE_N: &str = "oak_fence_gate[facing=north,in_wall=false,open=true,powered=false]";
const GATE_E: &str = "oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]";

/// The platforms of L4 at water level wl, units at x 0, 3, 6 (each 2 wide), in two banks facing
/// the drop column (z 4-7). North bank: source row z 1 (water on bottom slabs: no spawn there),
/// spawn rows 2 and 3, edge row 4 (water over open fence gates: it holds the water, not a golem),
/// stop row 5 (open fence gates: no fluid enters one, a golem walks through). South bank: source
/// row 9, spawn row 8, edge row 7, stop row 6 (spawn rows reach z 8, the villagers' square). A
/// golem leaves the floor still in the edge row's water, so it never stands on land at the drop
/// (where a walk against the current would step it back up), and falls dry past the stop row.
/// Glass walls between the units hold the water; at the edge rows, fence gates do (a golem
/// spawned 0.2 into a wall's column meets no wall ahead).
fn banks4(b: &mut Builder, wl: i32) {
    let fl = wl - 1;
    for sx in [0, 3, 6] {
        for x in sx..=sx + 1 {
            for (src, rows, edge, stop, back) in [(1, &[2, 3][..], 4, 5, 0), (9, &[8][..], 7, 6, 10)] {
                b.set(x, fl, src, "stone_slab[type=bottom,waterlogged=false]");
                b.set(x, wl, src, "water[level=0]");
                for &z in rows {
                    b.set(x, fl, z, STONE);
                }
                b.set(x, fl, edge, GATE_N);
                b.set(x, wl, stop, GATE_N);
                b.set(x, wl, back, GLASS);
                b.set(x, wl + 1, back, GLASS);
            }
        }
    }
    for x in [-1, 2, 5, 8] {
        for z in (0..=3).chain(8..=10) {
            b.set(x, fl, z, STONE);
            b.set(x, wl, z, GLASS);
            b.set(x, wl + 1, z, GLASS);
        }
        b.set(x, wl, 4, GATE_E);
        b.set(x, wl, 7, GATE_E);
    }
}

/// The drop column of L4 (x -1..8, z 4-7) from the platforms down to feet level k, and its kill:
/// water from a row of sources in a niche of the south wall (z 8) pushes every golem north into
/// the chamber (x -1..8, z 2-3): lava at head height held by open fence gates below it and at its
/// mouth, a ceiling right above it, hoppers under its north row.
fn column4(b: &mut Builder, wl: i32, k: i32) {
    let fl = wl - 1;
    for y in (k - 1)..=(wl + 1) {
        for x in -2..=9 {
            if y < fl {
                for z in [3, 8] {
                    let chamber = z == 3 && (-1..=8).contains(&x) && (k..=k + 2).contains(&y);
                    let niche = z == 8 && (-1..=8).contains(&x) && y == k;
                    if !chamber && !niche {
                        b.set(x, y, z, STONE);
                    }
                }
            }
            if x == -2 || x == 9 || y == k - 1 {
                // glass where it rises above the platforms' floor (no spawn on its top)
                let m = if y >= fl { GLASS } else { STONE };
                for z in 4..=7 {
                    b.set(x, y, z, m);
                }
            }
        }
    }
    for x in -1..=8 {
        b.set(x, k, 8, "water[level=0]");
        b.set(x, k, 9, STONE);
        for y in k..=k + 3 {
            b.set(x, y, 1, STONE);
        }
        b.set(x, k + 3, 2, STONE);
        b.set(x, k + 3, 3, STONE);
        for z in 2..=3 {
            b.set(x, k + 1, z, GATE_E);
            b.set(x, k + 2, z, LAVA);
        }
        b.set(x, k + 2, 4, GATE_N);
        b.set(x, k - 1, 3, STONE);
        b.set(x, k - 1, 2, if x < 8 { "hopper[enabled=true,facing=east]" } else { "hopper[enabled=true,facing=down]" });
    }
    for z in 2..=3 {
        for y in (k - 1)..=(k + 3) {
            b.set(-2, y, z, STONE);
            b.set(9, y, z, STONE);
        }
    }
    b.d.ports.insert("output".into(), [8, k - 2, 2]);
}

/// L4, the fast module: 4 villagers (only 3 must agree, so one sighting does not stretch a
/// cycle), the dusk pulse, two banks of short platforms whose open fence gates drop each golem
/// dry within about 2 blocks of its spawn, out of every villager's range, and one kill chamber.
pub fn l4() -> String {
    let mut b = Builder::new("iron-fast");
    front4(&mut b);
    let (wl, k) = (-6, -22);
    banks4(&mut b, wl);
    column4(&mut b, wl, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[
        "L4, the fast iron farm module: four villagers on their beds in closed 1-cell pods under leaves",
        "(only three must agree, so a golem one of them sees does not stretch the cycle) and a zombie in a",
        "boat 7 blocks north. At nightfall two inverted daylight detectors open the three trapdoors in front",
        "of the zombie for 46 game ticks: the villagers fall asleep, then wake up scared for the rest of the",
        "night. Golems spawn on 2-wide platforms in two banks facing a drop: water pushes them at most 2.2",
        "blocks through open fence gates (no water enters a fence gate) and they fall dry, out of every",
        "villager's sight, to water that pushes them under a lava blade with a ceiling, over two hoppers.",
    ])
}

/// The dry hole of L2d: a 6x6 pad (x 0..5, z 2..7, water level wl) whose 4x4 middle is the hole:
/// its outer ring holds water over open fence gates (the water stays, a golem sinks through), its
/// 2x2 center has open fence gates at the water level (dry). Water from the pad's corners pushes
/// each golem off the stone while it is still in water (no step back onto land) and on into the
/// dry center, where it falls freely down a 4x4 shaft.
pub(crate) fn dry_hole(b: &mut Builder, n: i32, wl: i32, k: i32) {
    dry_hole_walls(b, n, wl, k, wl - 1);
}

/// [`dry_hole`] with the shaft walled only below `top` (M2-open: in the void a golem leaving the
/// dry center falls straight, so only the drop's bottom layers need walls).
pub fn dry_hole_walls(b: &mut Builder, n: i32, wl: i32, k: i32, top: i32) {
    let fl = wl - 1;
    let ring = |x: i32, z: i32| (1..=4).contains(&x) && (3..=6).contains(&z);
    let center = |x: i32, z: i32| (2..=3).contains(&x) && (4..=5).contains(&z);
    let h = n / 2;
    let (x0, x1, z0, z1) = (3 - h, 2 + h, 5 - h, 4 + h);
    for x in x0..=x1 {
        for z in z0..=z1 {
            if center(x, z) {
                b.set(x, wl, z, GATE_N);
            } else if ring(x, z) {
                b.set(x, fl, z, GATE_N);
            } else {
                b.set(x, fl, z, STONE);
            }
        }
    }
    for (x, z) in [(x0, z0), (x0, z1), (x1, z0), (x1, z1)] {
        b.set(x, wl, z, "water[level=0]");
    }
    // the pad's walls: they hold its water and keep golems on it (fluid holder and walk blocker);
    // a wall top a golem could spawn on gets its cap from the judge's spot check (parts::cap_spawns)
    for y in wl..=wl + 1 {
        for x in (x0 - 1)..=(x1 + 1) {
            b.set(x, y, z0 - 1, STONE);
            b.set(x, y, z1 + 1, STONE);
        }
        for z in z0..=z1 {
            b.set(x0 - 1, y, z, STONE);
            b.set(x1 + 1, y, z, STONE);
        }
    }
    // the shaft (x 1..4, z 3..6) down to feet level k
    for y in (k - 1)..fl.min(top) {
        for x in 0..=5 {
            for z in 2..=7 {
                let inside = ring(x, z) && y >= k;
                let mouth = (1..=4).contains(&x) && z == 2 && (k..=k + 2).contains(&y);
                if !inside && !mouth {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    // the bottom: sources in its two south corners only; the levels then fall toward the north
    // row's middle, so every golem is pushed north and toward x 2..3, into the chamber
    for (x, z) in [(1, 6), (4, 6)] {
        b.set(x, k, z, "water[level=0]");
    }
}

/// The kill chamber of L2d north of its shaft's bottom, as wide as the shaft (x 1..4, z 1..2,
/// feet k), so no golem meets a corner: lava at head height held by open fence gates below it and
/// at its mouth (z 3), a ceiling right above it. The water enters from the shaft's south corners,
/// so its levels fall toward the middle columns: drops drift to the 2 hoppers under x 2..3.
pub(crate) fn chamber2(b: &mut Builder, k: i32) {
    for x in 1..=4 {
        for y in k..=k + 3 {
            b.set(x, y, 0, STONE);
        }
        b.set(x, k + 3, 1, STONE);
        b.set(x, k + 3, 2, STONE);
        for z in 1..=2 {
            b.set(x, k + 1, z, GATE_E);
            b.set(x, k + 2, z, LAVA);
        }
        b.set(x, k + 2, 3, GATE_N);
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

/// L2d, the middle ground with a dry drop: L2's front (3 villagers, or L4's 4), the dusk pulse,
/// the dry hole (golems fall out of every villager's range without sinking through water) and a
/// 2-hopper chamber: 13 iron.
fn l2d_module(villagers: i32, n: i32) -> Builder {
    let mut b = Builder::new(&format!("iron-middle-dry-{villagers}-{n}"));
    if villagers == 4 {
        front4(&mut b);
    } else {
        for x in [0, 2, 4] {
            b.pod(x);
        }
        zombie(&mut b);
        dusk_pulse(&mut b);
    }
    let (wl, k) = (-6, -22);
    dry_hole(&mut b, n, wl, k);
    chamber2(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b
}

fn l2d_caption(villagers: i32, n: i32) -> Vec<String> {
    let front = if villagers == 4 {
        "four villagers on their beds in closed 1-cell pods under leaves (only three must agree, so a golem one of\nthem sees does not stretch the cycle) and a zombie in a boat 7 blocks north of them in view. At nightfall\ntwo inverted daylight detectors open the three trapdoors in front of the zombie for 46 game ticks"
    } else {
        "three villagers on their beds in closed 1-cell pods under leaves and a zombie in a boat 7 blocks north of\nthem in view. At nightfall two inverted daylight detectors open the trapdoor in front of the zombie for 46\ngame ticks"
    };
    format!(
        "{front}: the villagers fall asleep, then wake up scared for the rest of the night (the farm restarts by\nitself after a missed night). Golems spawn on a {n}x{n} pad; water from its corners pushes them onto the hole's\nring (water over open fence gates: it holds the water, not a golem) and into its dry 2x2 center (open fence\ngates stop the water), and they fall 16 blocks out of every villager's sight to water that pushes them under\na lava blade held by fence gates, a ceiling right above it, over two hoppers."
    )
    .lines()
    .map(|l| l.to_string())
    .collect()
}

/// L2d, the dry-drop module: L2's front (3 villagers, or L4's 4), the dusk pulse, the dry hole in
/// an n x n pad (golems fall out of every villager's range without sinking through water) and a
/// 2-hopper chamber: 13 iron.
pub fn l2d(villagers: i32, n: i32) -> String {
    let b = l2d_module(villagers, n);
    let mut cap = vec![format!("L2d, the dry-drop iron farm module:")];
    cap.extend(l2d_caption(villagers, n));
    let cap: Vec<&str> = cap.iter().map(|s| s.as_str()).collect();
    b.text(&cap)
}

/// L5, the scaled farm: g x g L2d modules at a 27-block pitch (no golem of one module is ever in
/// another module's villagers' follow range, 26.9 at most); each module keeps its own villagers,
/// zombie, dusk pulse and kill. The first module's chest is the output; the others' are blocks.
pub fn l5(villagers: i32, n: i32, g: i32) -> String {
    let m = l2d_module(villagers, n).d;
    let mut b = Builder::new(&format!("iron-scaled-{g}x{g}"));
    b.d.meta = m.meta.clone();
    let out = m.ports["output"];
    for i in 0..g {
        for j in 0..g {
            let (dx, dz) = (27 * i, 27 * j);
            for (p, s) in &m.blocks {
                b.d.blocks.insert([p[0] + dx, p[1], p[2] + dz], s.clone());
            }
            for e in &m.entities {
                b.d.entities.push(EntitySpec { kind: e.kind.clone(), pos: [e.pos[0] + dx as f64, e.pos[1], e.pos[2] + dz as f64], nbt: e.nbt.clone() });
            }
            if i == 0 && j == 0 {
                b.d.ports.insert("output".into(), out);
            } else {
                b.set(out[0] + dx, out[1], out[2] + dz, "chest[facing=north,type=single,waterlogged=false]");
            }
        }
    }
    let mut cap = vec![format!("L5, the scaled iron farm: {g} x {g} L2d modules at a 27-block pitch, each one:")];
    cap.extend(l2d_caption(villagers, n));
    let cap: Vec<&str> = cap.iter().map(|s| s.as_str()).collect();
    b.text(&cap)
}
