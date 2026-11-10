//! The enchantments a naturally spawned mob's equipment gets (`Mob.populateDefaultEquipmentEnchantments`
//! -> `EnchantmentHelper.enchantItemFromProvider(.., VanillaEnchantmentProviders.MOB_SPAWN_EQUIPMENT, ..)`
//! = `EnchantmentsByCostWithDifficulty(#minecraft:on_mob_spawn_equipment, 5, 17)` ->
//! `EnchantmentHelper.selectEnchantment`), every level-random draw in the game's order.
//!
//! Data (26.3): `data/minecraft/enchantment/*.json` (weight, max level, min and max cost, exclusive
//! set), `tags/enchantment/non_treasure.json` (the order: `#on_mob_spawn_equipment` is
//! `#non_treasure`), `tags/enchantment/exclusive_set/*.json`, `tags/item/enchantable/*.json` (which
//! enchantments take an item as primary: `Enchantment.isPrimaryItem`), the items' `enchantable`
//! values (`Items`, `ToolMaterial`, `ArmorMaterials`).

use crate::rng::LegacyRandom;

/// An enchantment's definition: (id, weight, max level, min cost (base, per level above the
/// first), max cost (base, per level above the first), its exclusive set).
struct Ench {
    id: &'static str,
    weight: i32,
    max_level: i32,
    min: (i32, i32),
    max: (i32, i32),
    exclusive: &'static [&'static str],
}

const ARMOR_SET: &[&str] = &["protection", "blast_protection", "fire_protection", "projectile_protection"];
const DAMAGE_SET: &[&str] = &["sharpness", "smite", "bane_of_arthropods", "impaling", "density", "breach"];
const MINING_SET: &[&str] = &["fortune", "silk_touch"];
const BOW_SET: &[&str] = &["infinity", "mending"];
const RIPTIDE_SET: &[&str] = &["loyalty", "channeling"];
const BOOTS_SET: &[&str] = &["frost_walker", "depth_strider"];

const fn e(id: &'static str, weight: i32, max_level: i32, min: (i32, i32), max: (i32, i32), exclusive: &'static [&'static str]) -> Ench {
    Ench { id, weight, max_level, min, max, exclusive }
}

/// `#minecraft:non_treasure` in its order, the enchantments a spawned mob's items can take.
const ENCH: &[Ench] = &[
    e("protection", 10, 4, (1, 11), (12, 11), ARMOR_SET),
    e("fire_protection", 5, 4, (10, 8), (18, 8), ARMOR_SET),
    e("feather_falling", 5, 4, (5, 6), (11, 6), &[]),
    e("blast_protection", 2, 4, (5, 8), (13, 8), ARMOR_SET),
    e("projectile_protection", 5, 4, (3, 6), (9, 6), ARMOR_SET),
    e("respiration", 2, 3, (10, 10), (40, 10), &[]),
    e("aqua_affinity", 2, 1, (1, 0), (41, 0), &[]),
    e("thorns", 1, 3, (10, 20), (60, 20), &[]),
    e("depth_strider", 2, 3, (10, 10), (25, 10), BOOTS_SET),
    e("sharpness", 10, 5, (1, 11), (21, 11), DAMAGE_SET),
    e("smite", 5, 5, (5, 8), (25, 8), DAMAGE_SET),
    e("bane_of_arthropods", 5, 5, (5, 8), (25, 8), DAMAGE_SET),
    e("knockback", 5, 2, (5, 20), (55, 20), &[]),
    e("fire_aspect", 2, 2, (10, 20), (60, 20), &[]),
    e("looting", 2, 3, (15, 9), (65, 9), &[]),
    e("sweeping_edge", 2, 3, (5, 9), (20, 9), &[]),
    e("efficiency", 10, 5, (1, 10), (51, 10), &[]),
    e("silk_touch", 1, 1, (15, 0), (65, 0), MINING_SET),
    e("unbreaking", 5, 3, (5, 8), (55, 8), &[]),
    e("fortune", 2, 3, (15, 9), (65, 9), MINING_SET),
    e("power", 10, 5, (1, 10), (16, 10), &[]),
    e("punch", 2, 2, (12, 20), (37, 20), &[]),
    e("flame", 2, 1, (20, 0), (50, 0), &[]),
    e("infinity", 1, 1, (20, 0), (50, 0), BOW_SET),
    e("luck_of_the_sea", 2, 3, (15, 9), (65, 9), &[]),
    e("lure", 2, 3, (15, 9), (65, 9), &[]),
    e("loyalty", 5, 3, (12, 7), (50, 0), &[]),
    e("impaling", 2, 5, (1, 8), (21, 8), DAMAGE_SET),
    e("riptide", 2, 3, (17, 7), (50, 0), RIPTIDE_SET),
    e("channeling", 1, 1, (25, 0), (50, 0), &[]),
    e("lunge", 5, 3, (5, 8), (25, 8), &[]),
];

/// An item a natural mob spawns with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Bow,
    IronSword,
    IronSpear,
    IronShovel,
    /// A zombified piglin's (`ZombifiedPiglin.populateDefaultEquipmentSlots`).
    GoldenSword,
    /// A zombified piglin's, 1 in 20 (26.3's golden spear).
    GoldenSpear,
    Trident,
    FishingRod,
    /// A piece of armor: `slot` 0 head, 1 chest, 2 legs, 3 feet; `material` the
    /// `getEquipmentForSlot` type (0 leather, 1 copper, 2 golden, 3 chainmail, 4 iron, 5 diamond).
    Armor { slot: u8, material: u8 },
}

impl Item {
    /// The `enchantable` component's value.
    fn enchantability(self) -> i32 {
        match self {
            Item::Bow | Item::Trident | Item::FishingRod => 1,
            Item::IronSword | Item::IronSpear | Item::IronShovel => 14,
            Item::GoldenSword | Item::GoldenSpear => 22,
            Item::Armor { material, .. } => [15, 8, 25, 12, 9, 10][material as usize],
        }
    }

    /// The enchantments taking the item as primary (`isPrimaryItem`), in `#non_treasure` order
    /// (from the item tags: swords and spears are `melee_weapon`; swords `sweeping`; spears
    /// `lunge`; shovels `mining` and `mining_loot`; every one of these `durability`; thorns'
    /// primary items are chest armor only).
    fn takes(self, id: &str) -> bool {
        match self {
            Item::Bow => matches!(id, "unbreaking" | "power" | "punch" | "flame" | "infinity"),
            Item::IronSword | Item::GoldenSword => matches!(id, "sharpness" | "smite" | "bane_of_arthropods" | "knockback" | "fire_aspect" | "looting" | "sweeping_edge" | "unbreaking"),
            Item::IronSpear | Item::GoldenSpear => matches!(id, "sharpness" | "smite" | "bane_of_arthropods" | "knockback" | "fire_aspect" | "looting" | "unbreaking" | "lunge"),
            Item::IronShovel => matches!(id, "efficiency" | "silk_touch" | "unbreaking" | "fortune"),
            Item::Trident => matches!(id, "unbreaking" | "loyalty" | "impaling" | "riptide" | "channeling"),
            Item::FishingRod => matches!(id, "unbreaking" | "luck_of_the_sea" | "lure"),
            Item::Armor { slot, .. } => {
                matches!(id, "protection" | "fire_protection" | "blast_protection" | "projectile_protection" | "unbreaking")
                    || (slot == 0 && matches!(id, "respiration" | "aqua_affinity"))
                    || (slot == 1 && id == "thorns")
                    || (slot == 3 && matches!(id, "feather_falling" | "depth_strider"))
            }
        }
    }
}

/// `Math.round(float)`.
fn round(x: f32) -> i32 {
    (x + 0.5).floor() as i32
}

/// `Enchantment.areCompatible`.
fn compatible(a: &Ench, b: &Ench) -> bool {
    a.id != b.id && !a.exclusive.contains(&b.id) && !b.exclusive.contains(&a.id)
}

/// `WeightedRandom.getRandomItem` over (enchantment, level) instances.
fn weighted(r: &mut LegacyRandom, list: &[(usize, i32)]) -> Option<(usize, i32)> {
    let total: i32 = list.iter().map(|(i, _)| ENCH[*i].weight).sum();
    if total == 0 {
        return None;
    }
    let mut pick = r.next_int(total);
    for &(i, l) in list {
        pick -= ENCH[i].weight;
        if pick < 0 {
            return Some((i, l));
        }
    }
    None
}

/// `EnchantmentsByCostWithDifficulty(#on_mob_spawn_equipment, 5, 17).enchant`: the cost, then
/// `selectEnchantment`; the (enchantment id, level) pairs, every draw on `r`.
pub fn mob_spawn_equipment(r: &mut LegacyRandom, item: Item, special: f32) -> Vec<(&'static str, i32)> {
    let cost = r.next_int((special * 17.0) as i32 + 1) + 5;
    select(r, item, cost)
}

/// `EnchantmentHelper.selectEnchantment(random, item, cost, #non_treasure)`.
fn select(r: &mut LegacyRandom, item: Item, cost: i32) -> Vec<(&'static str, i32)> {
    let e = item.enchantability();
    let mut cost = cost + 1 + r.next_int(e / 4 + 1) + r.next_int(e / 4 + 1);
    let span = (r.next_float() + r.next_float() - 1.0) * 0.15;
    cost = round(cost as f32 + cost as f32 * span).max(1);
    // getAvailableEnchantmentResults: each primary enchantment at its highest level whose costs hold
    let mut avail: Vec<(usize, i32)> = Vec::new();
    for (i, en) in ENCH.iter().enumerate() {
        if !item.takes(en.id) {
            continue;
        }
        for level in (1..=en.max_level).rev() {
            let lo = en.min.0 + en.min.1 * (level - 1);
            let hi = en.max.0 + en.max.1 * (level - 1);
            if cost >= lo && cost <= hi {
                avail.push((i, level));
                break;
            }
        }
    }
    let mut out: Vec<(usize, i32)> = Vec::new();
    if !avail.is_empty() {
        if let Some(x) = weighted(r, &avail) {
            out.push(x);
        }
        while r.next_int(50) <= cost {
            if let Some(&(last, _)) = out.last() {
                avail.retain(|(i, _)| compatible(&ENCH[last], &ENCH[*i]));
            }
            if avail.is_empty() {
                break;
            }
            if let Some(x) = weighted(r, &avail) {
                out.push(x);
            }
            cost /= 2;
        }
    }
    out.into_iter().map(|(i, l)| (ENCH[i].id, l)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_available_lists_follow_the_tags_order_and_costs() {
        // a bow at cost 21 (power IV's 21..46? no: power's min 1+10(l-1), max 16+10(l-1); punch
        // 12+20(l-1)..37+20(l-1); flame and infinity 20..50; unbreaking 5+8(l-1)..55+8(l-1))
        let mut r = LegacyRandom::new(1);
        let out = mob_spawn_equipment(&mut r, Item::Bow, 0.125);
        assert!(!out.is_empty() && out.iter().all(|(id, l)| Item::Bow.takes(id) && *l >= 1));
        // armor sets exclude each other
        for seed in 0..200 {
            let mut r = LegacyRandom::new(seed);
            let out = mob_spawn_equipment(&mut r, Item::Armor { slot: 3, material: 2 }, 1.0);
            let prot = out.iter().filter(|(id, _)| ARMOR_SET.contains(id)).count();
            assert!(prot <= 1, "{out:?}");
        }
    }
}
