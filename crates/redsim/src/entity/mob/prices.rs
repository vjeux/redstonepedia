//! Build minutes at the stage's prices (ironspec iteration 3, designs/briefs/farm-iron.md rev 9):
//! 1 s per block placed, the stage's seconds per natural block dug, and the seconds to GET every
//! item at the stage's price (`designs/farms/prices.tsv`, compiled in: the judge, the optimizer and
//! the wasm viewer price the same way), lava sources, villagers moved and the zombie included; each
//! dug block is one free filler block. An id the table does not price is an error (a build is never
//! cheaper because a price is missing).

use std::collections::{BTreeMap, HashMap};

/// The table, compiled in.
pub const TABLE: &str = include_str!("../../../../../designs/farms/prices.tsv");

/// The prices table: item -> seconds per stage (day-1, middle, late; None: not available).
pub struct Prices {
    rows: HashMap<String, [Option<f64>; 3]>,
}

/// The stage's column: day-1 0, middle 1, late 2.
pub fn stage_ix(stage: &str) -> usize {
    match stage {
        "day-1" | "day-1-dust" => 0,
        "late" => 2,
        _ => 1,
    }
}

/// Whether a block id needs redstone (day 1: the redstone fork's iron pickaxe).
fn needs_redstone(id: &str) -> bool {
    ["redstone", "repeater", "piston", "observer", "comparator", "dropper", "dispenser", "note_block", "powered_rail", "detector_rail", "activator_rail", "crafter", "daylight_detector", "target", "calibrated_sculk_sensor", "lamp"].iter().any(|k| id.contains(k))
}

impl Prices {
    pub fn parse(text: &str) -> Prices {
        let mut rows = HashMap::new();
        for line in text.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 4 {
                continue;
            }
            let v = |s: &str| s.trim().parse::<f64>().ok();
            rows.insert(f[0].trim().to_string(), [v(f[1]), v(f[2]), v(f[3])]);
        }
        Prices { rows }
    }

    /// The compiled-in table (parsed once).
    pub fn get() -> Result<&'static Prices, String> {
        static P: std::sync::OnceLock<Prices> = std::sync::OnceLock::new();
        Ok(P.get_or_init(|| Prices::parse(TABLE)))
    }

    pub fn price(&self, item: &str, stage: usize) -> Result<f64, String> {
        match self.rows.get(item) {
            Some(r) => r[stage].ok_or_else(|| format!("{item} is not available at stage {}", ["day-1", "middle", "late"][stage])),
            None => Err(format!("no price for {item} (designs/farms/prices.tsv)")),
        }
    }
}

/// The item a placed block costs (None: nothing to get: air, or counted elsewhere: lava sources).
pub fn item_of(state: &str) -> Option<String> {
    let id = state.trim_start_matches("minecraft:");
    let id = id.split('[').next().unwrap_or(id);
    let props = state.split('[').nth(1).unwrap_or("");
    Some(match id {
        "air" | "cave_air" | "void_air" | "lava" | "fire" => return None,
        // (iteration 5, vjeux 13:09: mining stone without silk touch gives cobblestone, so a placed
        // stone or deepslate block is smelted: the `stone` row; cobblestone and dirt are filler)
        "stone" | "deepslate" => "stone".into(),
        "cobblestone" | "dirt" | "cobbled_deepslate" | "grass_block" | "andesite" | "granite" | "diorite" | "tuff" | "gravel" => "filler".into(),
        "stone_slab" => "stone_slab".into(),
        "cobblestone_slab" => "cobblestone_slab".into(),
        "wall_torch" | "torch" => "torch".into(),
        "redstone_wall_torch" | "redstone_torch" => "redstone_torch".into(),
        "stone_button" => "stone_button".into(),
        "waxed_copper_bulb" | "copper_bulb" => "copper_bulb".into(),
        _ if id.ends_with("_bed") => {
            // one bed per villager: the foot half comes with the head
            if props.contains("part=foot") {
                return None;
            }
            "red_bed".into()
        }
        _ if id.ends_with("_trapdoor") && id != "iron_trapdoor" => "oak_trapdoor".into(),
        _ if id.ends_with("_wall_sign") || id.ends_with("_sign") => "oak_wall_sign".into(),
        _ if id.ends_with("_button") => "oak_button".into(),
        _ if id.ends_with("_pressure_plate") && !id.contains("stone") => "oak_pressure_plate".into(),
        _ if id.ends_with("_fence_gate") => "oak_fence_gate".into(),
        _ if id.ends_with("_fence") => "oak_fence".into(),
        _ if id.ends_with("_slab") && !id.contains("stone") => "oak_slab".into(),
        _ if id.ends_with("_carpet") => "white_carpet".into(),
        _ if id.ends_with("_leaves") => "oak_leaves".into(),
        _ if id.ends_with("_log") => "log".into(),
        _ if id.ends_with("_planks") => "planks".into(),
        _ => id.to_string(),
    })
}

/// The bill of one build at a stage.
#[derive(Clone, Debug, Default)]
pub struct Bill {
    pub seconds: f64,
    /// item -> (count, seconds), the tools and the free filler included (filler's count is what was
    /// paid for).
    pub items: BTreeMap<String, (u32, f64)>,
}

impl Bill {
    pub fn line(&self) -> String {
        let mut v: Vec<(&String, &(u32, f64))> = self.items.iter().filter(|(_, (_, s))| *s > 0.0).collect();
        v.sort_by(|a, b| b.1.1.partial_cmp(&a.1.1).unwrap_or(std::cmp::Ordering::Equal));
        v.iter().take(8).map(|(k, (n, s))| format!("{n} {k} {:.0} s", s)).collect::<Vec<_>>().join(", ")
    }
}

/// The build's seconds at `stage`: `blocks` the design places (states), `entities` (kind, nbt),
/// `dug` natural blocks removed, `covers` blocks the site adds (each priced as `cover`), the
/// redstone fork (day 1: the iron pickaxe once when the build holds redstone).
pub fn bill(blocks: &[&str], entities: &[(&str, &str)], dug: u32, covers: &[&str], stage: usize) -> Result<Bill, String> {
    let p = Prices::get()?;
    let mut b = Bill::default();
    let add = |b: &mut Bill, item: &str, n: u32, each: f64| {
        let e = b.items.entry(item.to_string()).or_insert((0, 0.0));
        e.0 += n;
        e.1 += each * n as f64;
        b.seconds += each * n as f64;
    };
    let (mut placed, mut filler, mut lava, mut water, mut leaves, mut redstone) = (0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for s in blocks.iter().chain(covers.iter()) {
        let id = s.trim_start_matches("minecraft:").split('[').next().unwrap_or("");
        if matches!(id, "air" | "cave_air" | "void_air") {
            continue;
        }
        placed += 1;
        if id == "lava" {
            if s.contains("level=0") || !s.contains("level=") {
                lava += 1;
            }
            continue;
        }
        if id == "water" {
            water += 1;
        }
        if id.ends_with("_leaves") {
            leaves += 1;
        }
        if needs_redstone(id) {
            redstone += 1;
        }
        let Some(item) = item_of(s) else { continue };
        if item == "filler" {
            filler += 1;
            continue;
        }
        if item == "lightning_rod" && p.rows.get("lightning_rod").is_none() {
            // 3 copper ingots (a table without its own row)
            *counts.entry("copper_ingot".into()).or_default() += 3;
            continue;
        }
        *counts.entry(item).or_default() += 1;
    }
    add(&mut b, "place", placed, p.price("place", stage)?);
    add(&mut b, "dig", dug, p.price("dig", stage)?);
    let paid = filler.saturating_sub(dug);
    add(&mut b, "filler", paid, p.price("filler", stage)?);
    // lava per realistic action (iteration 6): trips of `lava_buckets` buckets, a fill per source;
    // on day 1 extra buckets (3 iron, 90 s each) where they save a trip: the cheapest k
    if lava > 0 {
        let (trip, fill, carry) = (p.price("lava_trip", stage)?, p.price("lava_fill", stage)?, p.price("lava_buckets", stage)?.max(1.0) as u32);
        let cost = |k: u32| (lava.div_ceil(carry + k)) as f64 * trip + k as f64 * EXTRA_BUCKET_S;
        let k = if stage == 0 { (0..lava).min_by(|a, c| cost(*a).total_cmp(&cost(*c))).unwrap_or(0) } else { 0 };
        add(&mut b, "lava_trip", lava.div_ceil(carry + k), trip);
        add(&mut b, "lava_fill", lava, fill);
        if k > 0 {
            add(&mut b, "extra_bucket", k, EXTRA_BUCKET_S);
        }
    }
    // water: a bucket refill per source
    add(&mut b, "water", water, p.price("water", stage)?);
    for (item, n) in &counts {
        if item == "water" {
            continue;
        }
        add(&mut b, item, *n, p.price(item, stage)?);
    }
    let mut villagers = 0u32;
    for (kind, nbt) in entities {
        let kind = kind.trim_start_matches("minecraft:");
        match kind {
            "villager" => villagers += 1,
            "zombie" => add(&mut b, "zombie", 1, p.price("zombie", stage)?),
            k if k.ends_with("_boat") => {
                add(&mut b, "oak_boat", 1, p.price("oak_boat", stage)?);
                if nbt.contains("zombie") {
                    add(&mut b, "zombie", 1, p.price("zombie", stage)?);
                }
            }
            k => return Err(format!("no price for the entity {k}")),
        }
    }
    if lava + water > 0 {
        add(&mut b, "tool:bucket", 1, p.price("tool:bucket", stage)?);
    }
    if leaves > 0 {
        add(&mut b, "tool:shears", 1, p.price("tool:shears", stage)?);
    }
    if redstone > 0 && stage == 0 {
        add(&mut b, "tool:iron_pickaxe", 1, p.price("tool:iron_pickaxe", stage)?);
    }
    // villagers by the stage's cheapest route: moved each, or (where the stage has it) bred at the
    // farm: the first pair moved, 30 s of actions per further villager, and the breeding days only
    // where they outlast the rest of the build (the player builds meanwhile)
    if villagers > 0 {
        let moved = p.price("villager_moved", stage)?;
        let by_moving = villagers as f64 * moved;
        let by_breeding = match p.price("villager_bred", stage) {
            Ok(each) if villagers > 2 => {
                let days = breeding_days(villagers);
                let wait = (days as f64 * GAME_DAY_S - b.seconds).max(0.0);
                Some((2.0 * moved + (villagers - 2) as f64 * each, wait, each))
            }
            _ => None,
        };
        match by_breeding {
            Some((actions, wait, each)) if actions + wait < by_moving => {
                add(&mut b, "villager_moved", 2, moved);
                add(&mut b, "villager_bred", villagers - 2, each);
                if wait > 0.0 {
                    add(&mut b, "breeding_wait", 1, wait);
                }
            }
            _ => add(&mut b, "villager_moved", villagers, moved),
        }
    }
    Ok(b)
}

/// An extra bucket on day 1 (3 iron at 30 s an ingot).
const EXTRA_BUCKET_S: f64 = 90.0;
/// A game day in seconds of play.
const GAME_DAY_S: f64 = 1200.0;

/// The game days a pair moved to the farm takes to breed `n` villagers (iteration 6): each day
/// every adult pair gives up to 2 births (at most the villagers still needed); babies grow up in a
/// day (24000 gt), so they breed from the next day and the last ones need that day too.
pub fn breeding_days(n: u32) -> u32 {
    let (mut adults, mut have, mut days) = (2u32, 2u32, 0u32);
    while have < n {
        let births = (2 * (adults / 2)).min(n - have);
        have += births;
        days += 1;
        adults = have;
    }
    // the last babies grow up before their pods are sealed
    days + u32::from(n > 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table prices the M2 build's items; glass costs 7 s on day 1 and 2 s in the middle; a dug
    /// block pays for one filler block; a placed stone block is smelted (iteration 5).
    #[test]
    fn a_build_is_priced_from_the_table() {
        let blocks = ["minecraft:glass", "minecraft:cobblestone", "minecraft:cobblestone", "minecraft:red_bed[facing=north,occupied=false,part=head]", "minecraft:red_bed[facing=north,occupied=false,part=foot]", "minecraft:lava[level=0]"];
        let d1 = bill(&blocks, &[("villager", ""), ("oak_boat", "{Passengers:[{id:\"minecraft:zombie\"}]}")], 1, &[], 0).unwrap();
        // place 6 (1 s each) + dig 1 + 1 filler paid + lava (iteration 6: one trip 120 + one fill
        // 5) + glass 7 + bed 5 + villager 180 + boat 2.5 + zombie 180 + bucket 90
        assert!((d1.seconds - (6.0 + 1.0 + 1.0 + 125.0 + 7.0 + 5.0 + 180.0 + 2.5 + 180.0 + 90.0)).abs() < 1e-9, "{:?}", d1.items);
        let mid = bill(&["minecraft:glass"], &[], 0, &[], 1).unwrap();
        assert!((mid.seconds - 3.0).abs() < 1e-9);
        assert!(bill(&["minecraft:unobtainium"], &[], 0, &[], 0).is_err());
        // stone: place 1 + 6.5 smelted; a cobblestone slab 0.5, a stone slab 3.25
        let stone = bill(&["minecraft:stone", "minecraft:cobblestone_slab[type=bottom,waterlogged=false]", "minecraft:stone_slab[type=bottom,waterlogged=false]"], &[], 0, &[], 0).unwrap();
        assert!((stone.seconds - (3.0 + 6.5 + 0.5 + 3.25)).abs() < 1e-9, "{:?}", stone.items);
    }

    /// Lava per realistic action (iteration 6): 4 sources on day 1 cost the cheapest of every
    /// number of extra buckets (min over k of ceil(4 / (1 + k)) trips x 120 + k x 90 + 4 x 5: one
    /// extra bucket, two trips, 240 + 90 + 20 = 350 s; the brief's example of 3 extra buckets, 410 s,
    /// is not the minimum), in the middle one trip of 16 buckets (60 + 4 x 3 = 72 s); water a
    /// refill per source; the bucket once.
    #[test]
    fn lava_and_water_by_the_action() {
        let lava = ["minecraft:lava[level=0]"; 4];
        let d1 = bill(&lava, &[], 0, &[], 0).unwrap();
        assert!((d1.seconds - (4.0 + 350.0 + 90.0)).abs() < 1e-9, "{:?}", d1.items);
        let mid = bill(&lava, &[], 0, &[], 1).unwrap();
        assert!((mid.seconds - (4.0 + 72.0)).abs() < 1e-9, "{:?}", mid.items);
        let water = bill(&["minecraft:water[level=0]", "minecraft:water[level=0]"], &[], 0, &[], 1).unwrap();
        assert!((water.seconds - (2.0 + 2.0)).abs() < 1e-9, "{:?}", water.items);
    }

    /// Villagers by the cheapest route: moved each, or in the middle bred at the farm (the pair
    /// moved, 30 s per further villager) when its breeding days do not outlast the build by more
    /// than moving saves; 8 villagers take 2 days of births and a day to grow up.
    #[test]
    fn villagers_by_the_cheapest_route() {
        assert_eq!(breeding_days(8), 3);
        assert_eq!(breeding_days(2), 0);
        let eight: Vec<(&str, &str)> = vec![("villager", ""); 8];
        // a bare build: the 3 breeding days (3600 s) outlast it, so moving (8 x 120) is cheaper
        let small = bill(&[], &eight, 0, &[], 1).unwrap();
        assert!((small.seconds - 960.0).abs() < 1e-9, "{:?}", small.items);
        // a build of 4000 blocks placed (4000 s): breeding's days hide in it: 2 x 120 + 6 x 30
        let big: Vec<&str> = vec!["minecraft:cobblestone"; 4000];
        let large = bill(&big, &eight, 4000, &[], 1).unwrap();
        assert!(large.items.contains_key("villager_bred"), "{:?}", large.items);
        assert!((large.seconds - (4000.0 + 4000.0 * 0.5 + 240.0 + 180.0)).abs() < 1e-9, "{:?}", large.items);
    }

    /// Every item `bill` can name has a row in prices.tsv (a renamed row fails here, at the source).
    #[test]
    fn every_item_bill_names_has_a_row() {
        let p = Prices::get().unwrap();
        let states = [
            "stone", "cobblestone", "stone_slab[type=bottom]", "cobblestone_slab[type=bottom]", "glass", "torch", "wall_torch[facing=east]", "redstone_torch", "stone_button[face=wall]", "oak_button[face=wall]",
            "waxed_copper_bulb", "red_bed[part=head]", "oak_trapdoor[half=bottom]", "oak_wall_sign[facing=south]", "oak_pressure_plate", "oak_fence_gate", "oak_fence", "oak_slab[type=bottom]", "white_carpet",
            "oak_leaves", "oak_log", "oak_planks", "hopper", "chest", "observer", "repeater", "comparator", "redstone_wire", "lightning_rod", "piston", "water", "campfire", "composter", "crafter", "dropper",
        ];
        for s in states {
            let state = format!("minecraft:{s}");
            if let Some(item) = item_of(&state)
                && item != "filler"
            {
                assert!(p.rows.contains_key(&item), "{s} -> {item}: no row in designs/farms/prices.tsv");
            }
        }
        for row in ["place", "dig", "filler", "lava_trip", "lava_fill", "lava_buckets", "water", "villager_moved", "villager_bred", "zombie", "oak_boat", "tool:bucket", "tool:shears", "tool:iron_pickaxe", "copper_ingot"] {
            assert!(p.rows.contains_key(row), "{row}: no row in designs/farms/prices.tsv");
        }
    }
}
