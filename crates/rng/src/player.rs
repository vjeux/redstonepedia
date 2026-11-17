//! The player's own RNG: recovering it from two consecutive XP seeds, and steering the next
//! XP seed by dropping items (each Q press = 4 `nextFloat` calls in
//! `LivingEntity.createItemStackToDrop`; each enchant = 1 `nextInt` in `Player.onEnchantmentPerformed`).

use crate::data::{Data, EnchIdx, Item};
use crate::java_random::JavaRandom;
use crate::table::{enchantment_list, slot_costs, Ench};

/// Player RNG calls per item drop (`createItemStackToDrop`, not randomly: four `nextFloat`).
pub const CALLS_PER_DROP: u64 = 4;

/// Given XP seed `xp1` and the XP seed `xp2` drawn by the next enchant, return the player's RNG
/// state right after `xp2` was drawn. `max_gap` allows that many unrelated player RNG calls
/// between the two enchants (0 = nothing else touched the player's RNG).
pub fn crack_player(xp1: i32, xp2: i32, max_gap: u32) -> Vec<(u64, u32)> {
    let high = (xp1 as u32 as u64) << 16;
    let mut out = Vec::new();
    for low in 0..65536u64 {
        let mut state = high | low;
        for gap in 0..=max_gap {
            state = JavaRandom::step(state);
            if (state >> 16) as u32 == xp2 as u32 {
                out.push((state, gap));
            }
        }
    }
    out
}

/// All XP seeds the next enchant can produce when `xp1` is the current one (65,536 of them),
/// assuming nothing else used the player's RNG in between. Used to crack the second seed fast.
pub fn next_seed_candidates(xp1: i32) -> Vec<i32> {
    let high = (xp1 as u32 as u64) << 16;
    (0..65536u64).map(|low| (JavaRandom::step(high | low) >> 16) as i32).collect()
}

#[derive(Clone, Debug)]
pub struct Goal {
    pub item: usize,
    pub wanted: Vec<(EnchIdx, i32)>,
    pub unwanted: Vec<EnchIdx>,
    pub max_shelves: i32,
    pub max_drops: u32,
    pub player_level: i32,
}

#[derive(Clone, Debug)]
pub struct Plan {
    /// `None`: enchant right away with the current seed. `Some(n)`: drop n items, then do one
    /// throwaway enchant, then enchant the target.
    pub drops: Option<u32>,
    pub shelves: i32,
    pub slot: usize,
    pub cost: i32,
    pub xp_seed: i32,
    pub enchants: Vec<Ench>,
    /// The player's RNG state after the target enchant (for chaining the next plan).
    pub state_after: u64,
}

fn satisfies(list: &[Ench], goal: &Goal) -> bool {
    goal.wanted.iter().all(|&(e, lvl)| list.iter().any(|&(f, l)| f == e && l as i32 >= lvl))
        && goal.unwanted.iter().all(|&e| list.iter().all(|&(f, _)| f != e))
}

/// The search of Earthcomputer's EnchantmentCracker: fewest drops, then fewest shelves, then the
/// lowest slot. `state` is the player's RNG right after the current XP seed was drawn.
pub fn plan(data: &Data, state: u64, goal: &Goal) -> Option<Plan> {
    let item: &Item = &data.items[goal.item];
    let mut rng_state = state;
    for i in -1i64..=goal.max_drops as i64 {
        let xp_seed = if i < 0 { (state >> 16) as i32 } else { (JavaRandom::step(rng_state) >> 16) as i32 };
        for shelves in 0..=goal.max_shelves {
            let mut rng = JavaRandom::new(xp_seed as i64);
            let costs = slot_costs(&mut rng, shelves);
            for (slot, &cost) in costs.iter().enumerate() {
                if cost == 0 {
                    continue;
                }
                let needed = if i < 0 { cost } else { cost + 1 };
                if goal.player_level < needed {
                    continue;
                }
                let list = enchantment_list(&mut rng, data, item, xp_seed, slot, cost);
                if list.is_empty() || !satisfies(&list, goal) {
                    continue;
                }
                let drops = if i < 0 { None } else { Some(i as u32) };
                let mut after = JavaRandom::from_state(state);
                if let Some(n) = drops {
                    after.advance(CALLS_PER_DROP * n as u64 + 1);
                }
                after.advance(1);
                return Some(Plan { drops, shelves, slot, cost, xp_seed, enchants: list, state_after: after.state });
            }
        }
        if i >= 0 {
            let mut r = JavaRandom::from_state(rng_state);
            r.advance(CALLS_PER_DROP);
            rng_state = r.state;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_videos_numbers() {
        // the short this calculator reproduces (chrzz, "the secret to choose any enchant you want
        // in Minecraft"): enchanting seeds 5F94CCE0 then BE8A2899 give the player seed
        // BE8A2899B11..., and Sharpness IV, Fire Aspect II and Looting III on a diamond sword are
        // "drop 195 (3 stacks and 3), 15 bookshelves, the bottom slot"
        let data = Data::load();
        let found = crack_player(0x5F94CCE0u32 as i32, 0xBE8A2899u32 as i32, 0);
        assert_eq!(found.len(), 1);
        assert!(format!("{:012X}", found[0].0).starts_with("BE8A2899B11"));
        let e = |n: &str| data.enchant(n).unwrap();
        let goal = Goal {
            item: data.item("diamond_sword").unwrap(),
            wanted: vec![(e("sharpness"), 4), (e("fire_aspect"), 2), (e("looting"), 3)],
            unwanted: vec![],
            max_shelves: 15,
            max_drops: 2048,
            player_level: 999,
        };
        let p = plan(data, found[0].0, &goal).expect("a plan");
        assert_eq!((p.drops, p.shelves, p.slot), (Some(195), 15, 2));
    }
}
