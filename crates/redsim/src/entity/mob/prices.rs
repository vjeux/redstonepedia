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
        "stone" | "cobblestone" | "dirt" | "deepslate" | "cobbled_deepslate" | "grass_block" | "andesite" | "granite" | "diorite" | "tuff" | "gravel" => "filler".into(),
        "stone_slab" | "cobblestone_slab" => "stone_slab".into(),
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
        if item == "lightning_rod" {
            // 3 copper ingots (not a row of its own)
            *counts.entry("copper_ingot".into()).or_default() += 3;
            continue;
        }
        *counts.entry(item).or_default() += 1;
    }
    add(&mut b, "place", placed, p.price("place", stage)?);
    add(&mut b, "dig", dug, p.price("dig", stage)?);
    let paid = filler.saturating_sub(dug);
    add(&mut b, "filler", paid, p.price("filler", stage)?);
    add(&mut b, "lava_source", lava, p.price("lava_source", stage)?);
    for (item, n) in &counts {
        add(&mut b, item, *n, p.price(item, stage)?);
    }
    for (kind, nbt) in entities {
        let kind = kind.trim_start_matches("minecraft:");
        match kind {
            "villager" => add(&mut b, "villager_moved", 1, p.price("villager_moved", stage)?),
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
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table prices the M2 build's items; glass costs 7 s on day 1 and 2 s in the middle; a dug
    /// block pays for one filler block.
    #[test]
    fn a_build_is_priced_from_the_table() {
        let blocks = ["minecraft:glass", "minecraft:stone", "minecraft:stone", "minecraft:red_bed[facing=north,occupied=false,part=head]", "minecraft:red_bed[facing=north,occupied=false,part=foot]", "minecraft:lava[level=0]"];
        let d1 = bill(&blocks, &[("villager", ""), ("oak_boat", "{Passengers:[{id:\"minecraft:zombie\"}]}")], 1, &[], 0).unwrap();
        // place 6 (1 s each) + dig 1 + 1 filler paid + lava 120 + glass 7 + bed 5 + villager 180 +
        // boat 2.5 + zombie 180 + bucket 90
        assert!((d1.seconds - (6.0 + 1.0 + 1.0 + 120.0 + 7.0 + 5.0 + 180.0 + 2.5 + 180.0 + 90.0)).abs() < 1e-9, "{:?}", d1.items);
        let mid = bill(&["minecraft:glass"], &[], 0, &[], 1).unwrap();
        assert!((mid.seconds - 3.0).abs() < 1e-9);
        assert!(bill(&["minecraft:unobtainium"], &[], 0, &[], 0).is_err());
    }
}
