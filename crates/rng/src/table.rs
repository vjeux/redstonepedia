//! The enchanting table, line for line from `EnchantmentMenu.slotsChanged`,
//! `EnchantmentMenu.getEnchantmentList` and `EnchantmentHelper` in 26.3.

use crate::data::{Data, EnchIdx, Item};
use crate::java_random::{java_round, JavaRandom};

pub type Ench = (EnchIdx, u8);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotOffer {
    /// The level requirement shown on the button (0 = slot disabled).
    pub cost: i32,
    /// The hover hint: one random entry of `enchants`.
    pub clue: Option<Ench>,
    /// Everything the item actually receives if this slot is clicked.
    pub enchants: Vec<Ench>,
}

/// The three level requirements. `rng` must have just been seeded with the XP seed.
#[inline]
pub fn slot_costs(rng: &mut JavaRandom, shelves: i32) -> [i32; 3] {
    let b = shelves.min(15);
    let mut costs = [0; 3];
    for (slot, out) in costs.iter_mut().enumerate() {
        let selected = rng.next_int_bound(8) + 1 + (b >> 1) + rng.next_int_bound(b + 1);
        let cost = match slot {
            0 => (selected / 3).max(1),
            1 => selected * 2 / 3 + 1,
            _ => selected.max(b * 2),
        };
        *out = if cost < slot as i32 + 1 { 0 } else { cost };
    }
    costs
}

#[inline]
fn weighted_pick(rng: &mut JavaRandom, data: &Data, list: &[Ench]) -> Option<usize> {
    let total: i32 = list.iter().map(|&(e, _)| data.enchants[e as usize].weight).sum();
    if total <= 0 {
        return None;
    }
    let mut index = rng.next_int_bound(total);
    for (i, &(e, _)) in list.iter().enumerate() {
        index -= data.enchants[e as usize].weight;
        if index < 0 {
            return Some(i);
        }
    }
    None
}

/// `EnchantmentHelper.selectEnchantment` over the item's table pool.
pub fn select_enchantment(rng: &mut JavaRandom, data: &Data, item: &Item, cost: i32) -> Vec<Ench> {
    let mut results = Vec::new();
    let enchantability = item.enchantability;
    let mut cost = cost + 1 + rng.next_int_bound(enchantability / 4 + 1) + rng.next_int_bound(enchantability / 4 + 1);
    let a = rng.next_float();
    let b = rng.next_float();
    let span = (a + b - 1.0f32) * 0.15f32;
    let costf = cost as f32;
    cost = java_round(costf + costf * span).max(1);
    let mut available: Vec<Ench> = item
        .table_pool
        .iter()
        .filter_map(|&e| {
            let level = data.level_for_cost(e, cost);
            (level > 0).then_some((e, level))
        })
        .collect();
    if available.is_empty() {
        return results;
    }
    if let Some(i) = weighted_pick(rng, data, &available) {
        results.push(available[i]);
    }
    while rng.next_int_bound(50) <= cost {
        if let Some(&(last, _)) = results.last() {
            available.retain(|&(e, _)| data.compatible(last, e));
        }
        if available.is_empty() {
            break;
        }
        if let Some(i) = weighted_pick(rng, data, &available) {
            results.push(available[i]);
        }
        cost /= 2;
    }
    results
}

/// `EnchantmentMenu.getEnchantmentList`: re-seeds with `xpSeed + slot` (int addition).
pub fn enchantment_list(rng: &mut JavaRandom, data: &Data, item: &Item, xp_seed: i32, slot: usize, cost: i32) -> Vec<Ench> {
    rng.set_seed(xp_seed.wrapping_add(slot as i32) as i64);
    let mut list = select_enchantment(rng, data, item, cost);
    if item.is_book && list.len() > 1 {
        let i = rng.next_int_bound(list.len() as i32) as usize;
        list.remove(i);
    }
    list
}

/// Everything the table shows (and would give) for this XP seed, item and bookshelf count.
pub fn offers(data: &Data, xp_seed: i32, item: &Item, shelves: i32) -> [SlotOffer; 3] {
    let mut rng = JavaRandom::new(xp_seed as i64);
    let costs = slot_costs(&mut rng, shelves);
    let mut out: [SlotOffer; 3] = Default::default();
    for slot in 0..3 {
        out[slot].cost = costs[slot];
        if costs[slot] > 0 {
            let list = enchantment_list(&mut rng, data, item, xp_seed, slot, costs[slot]);
            if !list.is_empty() {
                let i = rng.next_int_bound(list.len() as i32) as usize;
                out[slot].clue = Some(list[i]);
            }
            out[slot].enchants = list;
        }
    }
    out
}

pub fn format_ench(data: &Data, e: Ench) -> String {
    let ench = &data.enchants[e.0 as usize];
    if ench.max_level == 1 {
        ench.name.clone()
    } else {
        format!("{} {}", ench.name, roman(e.1 as i32))
    }
}

pub fn roman(n: i32) -> &'static str {
    match n {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        8 => "VIII",
        9 => "IX",
        10 => "X",
        _ => "?",
    }
}
