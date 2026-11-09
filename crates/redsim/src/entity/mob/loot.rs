//! Entity loot tables (`data/minecraft/loot_table/entities/*.json` of 26.3) for the natural mobs,
//! drawn like `LootTable.getRandomItems`: each pool's conditions first (`killed_by_player` fails
//! without a player kill and draws nothing; `random_chance` draws a float), its rolls (a uniform
//! count draws), and per roll the valid entries (their own conditions draw nothing): one entry is
//! taken without a draw, several by `nextInt(total weight)`; then the entry's `set_count` (a
//! uniform count draws `Mth.nextInt(lo, hi)`), `furnace_smelt` when the mob is on fire, and
//! looting (no killer holds it: nothing). A table not listed here is the type's `TypeDef::loot`
//! (one pool per entry, its uniform count). Babies drop nothing (`shouldDropLoot`).

use crate::rng::LegacyRandom;

/// The loot context's random: a table's random sequence (`XoroshiroRandomSource`) in an exact
/// world, the seeded stand-in otherwise.
pub trait LootRandom {
    fn next_int(&mut self, bound: i32) -> i32;
    fn next_float(&mut self) -> f32;
}

impl LootRandom for LegacyRandom {
    fn next_int(&mut self, bound: i32) -> i32 {
        LegacyRandom::next_int(self, bound)
    }
    fn next_float(&mut self) -> f32 {
        LegacyRandom::next_float(self)
    }
}

impl LootRandom for crate::stems::LootSequence {
    fn next_int(&mut self, bound: i32) -> i32 {
        crate::stems::LootSequence::next_int(self, bound)
    }
    fn next_float(&mut self) -> f32 {
        crate::stems::LootSequence::next_float(self)
    }
}

/// A pool's condition.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PoolCond {
    None,
    /// `killed_by_player` (alone or first in `all_of`): never in these runs.
    KilledByPlayer,
    /// `random_chance`: `nextFloat() < chance`.
    RandomChance(f32),
    /// `entity_properties` this `type_specific/cube_mob size 1` (a slime's smallest).
    CubeSize1,
    /// `entity_properties` attacker `#skeletons` (a creeper's music disc).
    KilledBySkeleton,
    /// `damage_source_properties` `#is_lightning` (a turtle's bowl).
    Lightning,
}

/// An entry's own condition (checked in `expand`, no draws).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EntryCond {
    None,
    /// `entity_properties` this `vehicle: zombie_horse` (a zombie's red mushroom).
    RidesZombieHorse,
    /// A sheep's wool: the `alternatives` of `loot_table` entries by its colour, unsheared.
    SheepWool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Entry {
    pub item: &'static str,
    pub weight: i32,
    /// `set_count`: `None` keeps 1; `Some((lo, hi))` a uniform count (`lo == hi`: a constant).
    pub count: Option<(i32, i32)>,
    /// `furnace_smelt` when the mob is on fire.
    pub smelt: Option<&'static str>,
    pub cond: EntryCond,
}

const fn e(item: &'static str, lo: i32, hi: i32) -> Entry {
    Entry { item, weight: 1, count: Some((lo, hi)), smelt: None, cond: EntryCond::None }
}

const fn one(item: &'static str) -> Entry {
    Entry { item, weight: 1, count: None, smelt: None, cond: EntryCond::None }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pool {
    pub cond: PoolCond,
    /// `rolls`: (lo, hi), a constant when equal.
    pub rolls: (i32, i32),
    pub entries: &'static [Entry],
}

const fn p(entries: &'static [Entry]) -> Pool {
    Pool { cond: PoolCond::None, rolls: (1, 1), entries }
}

const fn pc(cond: PoolCond, entries: &'static [Entry]) -> Pool {
    Pool { cond, rolls: (1, 1), entries }
}

const ZOMBIE_RARE: &[Entry] = &[one("minecraft:iron_ingot"), one("minecraft:carrot"), Entry { smelt: Some("minecraft:baked_potato"), ..one("minecraft:potato") }];
const ZOMBIE: &[Pool] = &[
    p(&[e("minecraft:rotten_flesh", 0, 2)]),
    p(&[Entry { cond: EntryCond::RidesZombieHorse, ..e("minecraft:red_mushroom", 0, 1) }]),
    pc(PoolCond::KilledByPlayer, ZOMBIE_RARE),
    pc(PoolCond::KilledByPlayer, &[one("minecraft:music_disc_lava_chicken")]),
];
const ZOMBIE_VILLAGER: &[Pool] = &[p(&[e("minecraft:rotten_flesh", 0, 2)]), pc(PoolCond::KilledByPlayer, ZOMBIE_RARE)];
const DROWNED: &[Pool] = &[p(&[e("minecraft:rotten_flesh", 0, 2)]), pc(PoolCond::KilledByPlayer, &[one("minecraft:copper_ingot")])];
const ZOMBIE_HORSE: &[Pool] = &[p(&[e("minecraft:rotten_flesh", 2, 3)])];
const SKELETON: &[Pool] = &[p(&[e("minecraft:arrow", 0, 2)]), p(&[e("minecraft:bone", 0, 2)])];
const CREEPER: &[Pool] = &[p(&[e("minecraft:gunpowder", 0, 2)]), pc(PoolCond::KilledBySkeleton, &[])];
const SPIDER: &[Pool] = &[p(&[e("minecraft:string", 0, 2)]), pc(PoolCond::KilledByPlayer, &[e("minecraft:spider_eye", -1, 1)])];
const WITCH: &[Pool] = &[
    Pool {
        cond: PoolCond::None,
        rolls: (1, 3),
        entries: &[
            e("minecraft:glowstone_dust", 0, 2),
            e("minecraft:sugar", 0, 2),
            e("minecraft:spider_eye", 0, 2),
            e("minecraft:glass_bottle", 0, 2),
            e("minecraft:gunpowder", 0, 2),
            Entry { weight: 2, ..e("minecraft:stick", 0, 2) },
        ],
    },
    p(&[e("minecraft:redstone", 4, 8)]),
];
const ENDERMAN: &[Pool] = &[p(&[e("minecraft:ender_pearl", 0, 1)])];
const SLIME: &[Pool] = &[pc(PoolCond::CubeSize1, &[e("minecraft:slime_ball", 0, 2)])];
const NOTHING: &[Pool] = &[];
const SQUID: &[Pool] = &[p(&[e("minecraft:ink_sac", 1, 3)])];
const GLOW_SQUID: &[Pool] = &[p(&[e("minecraft:glow_ink_sac", 1, 3)])];
const CHICKEN: &[Pool] = &[p(&[e("minecraft:feather", 0, 2)]), p(&[Entry { smelt: Some("minecraft:cooked_chicken"), ..one("minecraft:chicken") }])];
const COW: &[Pool] = &[p(&[e("minecraft:leather", 0, 2)]), p(&[Entry { smelt: Some("minecraft:cooked_beef"), ..e("minecraft:beef", 1, 3) }])];
const PIG: &[Pool] = &[p(&[Entry { smelt: Some("minecraft:cooked_porkchop"), ..e("minecraft:porkchop", 1, 3) }])];
const SHEEP: &[Pool] = &[
    p(&[Entry { smelt: Some("minecraft:cooked_mutton"), ..e("minecraft:mutton", 1, 2) }]),
    p(&[Entry { cond: EntryCond::SheepWool, ..one("minecraft:white_wool") }]),
];
const LEATHER: &[Pool] = &[p(&[e("minecraft:leather", 0, 2)])];
const RABBIT: &[Pool] = &[
    p(&[e("minecraft:rabbit_hide", 0, 1)]),
    p(&[Entry { smelt: Some("minecraft:cooked_rabbit"), ..e("minecraft:rabbit", 1, 1) }]),
    pc(PoolCond::KilledByPlayer, &[one("minecraft:rabbit_foot")]),
];
const TURTLE: &[Pool] = &[p(&[Entry { weight: 3, ..e("minecraft:seagrass", 0, 2) }]), pc(PoolCond::Lightning, &[one("minecraft:bowl")])];
const CAT: &[Pool] = &[p(&[e("minecraft:string", 0, 2)])];
const SALMON: &[Pool] = &[p(&[Entry { smelt: Some("minecraft:cooked_salmon"), ..one("minecraft:salmon") }]), pc(PoolCond::RandomChance(0.05), &[one("minecraft:bone_meal")])];
const COD: &[Pool] = &[p(&[Entry { smelt: Some("minecraft:cooked_cod"), ..one("minecraft:cod") }]), pc(PoolCond::RandomChance(0.05), &[one("minecraft:bone_meal")])];

/// The natural mobs' tables by entity type id (`None`: the type's `TypeDef::loot`).
pub fn table(game_id: &str) -> Option<&'static [Pool]> {
    Some(match game_id {
        "minecraft:zombie" => ZOMBIE,
        "minecraft:zombie_villager" | "minecraft:husk" => ZOMBIE_VILLAGER,
        "minecraft:drowned" => DROWNED,
        "minecraft:zombie_horse" => ZOMBIE_HORSE,
        "minecraft:skeleton" => SKELETON,
        "minecraft:creeper" => CREEPER,
        "minecraft:spider" | "minecraft:cave_spider" => SPIDER,
        "minecraft:witch" => WITCH,
        "minecraft:enderman" => ENDERMAN,
        "minecraft:slime" => SLIME,
        "minecraft:sulfur_cube" | "minecraft:fox" | "minecraft:wolf" | "minecraft:bee" | "minecraft:bat" => NOTHING,
        "minecraft:squid" => SQUID,
        "minecraft:glow_squid" => GLOW_SQUID,
        "minecraft:chicken" => CHICKEN,
        "minecraft:cow" => COW,
        "minecraft:pig" => PIG,
        "minecraft:sheep" => SHEEP,
        "minecraft:horse" | "minecraft:donkey" | "minecraft:mule" | "minecraft:llama" | "minecraft:trader_llama" => LEATHER,
        "minecraft:rabbit" => RABBIT,
        "minecraft:turtle" => TURTLE,
        "minecraft:cat" => CAT,
        "minecraft:salmon" => SALMON,
        "minecraft:cod" => COD,
        _ => return None,
    })
}

/// The dying mob's facts the tables read.
pub struct Victim {
    pub on_fire: bool,
    pub cube_size: Option<i32>,
    pub rides_zombie_horse: bool,
    /// The sheep's wool item (its colour; `None` once sheared).
    pub wool: Option<&'static str>,
    pub killed_by_skeleton: bool,
    pub lightning: bool,
}

/// `Mth.nextInt(random, lo, hi)`.
fn next_int(r: &mut dyn LootRandom, lo: i32, hi: i32) -> i32 {
    if lo >= hi { lo } else { r.next_int(hi - lo + 1) + lo }
}

/// `LootTable.getRandomItems` of `pools` for `v`: the stacks in drop order (a count under 1 drops
/// nothing). An unmodeled branch (a creeper's music disc) returns its reason.
pub fn roll(pools: &[Pool], v: &Victim, r: &mut dyn LootRandom) -> Result<Vec<(&'static str, i32)>, String> {
    let mut out = Vec::new();
    for pool in pools {
        let ok = match pool.cond {
            PoolCond::None => true,
            PoolCond::KilledByPlayer => false,
            PoolCond::RandomChance(c) => r.next_float() < c,
            PoolCond::CubeSize1 => v.cube_size == Some(1),
            PoolCond::KilledBySkeleton => {
                if v.killed_by_skeleton {
                    return Err("a creeper killed by a skeleton (its music disc) is not modeled yet".into());
                }
                false
            }
            PoolCond::Lightning => v.lightning,
        };
        if !ok {
            continue;
        }
        let rolls = next_int(r, pool.rolls.0, pool.rolls.1);
        for _ in 0..rolls {
            let valid: Vec<&Entry> = pool
                .entries
                .iter()
                .filter(|en| match en.cond {
                    EntryCond::None => true,
                    EntryCond::RidesZombieHorse => v.rides_zombie_horse,
                    EntryCond::SheepWool => v.wool.is_some(),
                })
                .collect();
            let total: i32 = valid.iter().map(|en| en.weight).sum();
            if total == 0 || valid.is_empty() {
                continue;
            }
            let en = if valid.len() == 1 {
                valid[0]
            } else {
                let mut k = r.next_int(total);
                let mut pick = valid[0];
                for c in &valid {
                    k -= c.weight;
                    if k < 0 {
                        pick = c;
                        break;
                    }
                }
                pick
            };
            let n = match en.count {
                None => 1,
                Some((lo, hi)) => next_int(r, lo, hi),
            };
            let item = match en.cond {
                EntryCond::SheepWool => v.wool.unwrap_or(en.item),
                _ if v.on_fire => en.smelt.unwrap_or(en.item),
                _ => en.item,
            };
            out.push((item, n));
        }
    }
    Ok(out)
}

/// A sheep's wool item by its `DyeColor` id.
pub fn wool_of(color: u8) -> &'static str {
    const W: [&str; 16] = [
        "minecraft:white_wool",
        "minecraft:orange_wool",
        "minecraft:magenta_wool",
        "minecraft:light_blue_wool",
        "minecraft:yellow_wool",
        "minecraft:lime_wool",
        "minecraft:pink_wool",
        "minecraft:gray_wool",
        "minecraft:light_gray_wool",
        "minecraft:cyan_wool",
        "minecraft:purple_wool",
        "minecraft:blue_wool",
        "minecraft:brown_wool",
        "minecraft:green_wool",
        "minecraft:red_wool",
        "minecraft:black_wool",
    ];
    W[(color & 15) as usize]
}
