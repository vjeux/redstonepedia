//! The game's data, dumped from the real 26.3 server (`mctas rng game datagen`): every
//! enchantment (weight, levels, cost ranges, what it cannot sit with), every enchantable item
//! (its enchantability, its table pool), the enchantment tags, the trade sets and the villager
//! trades as the server jar ships them.

use serde::Deserialize;
use std::collections::HashMap;

pub type EnchIdx = u8;

#[derive(Deserialize)]
struct RawEnchant {
    id: String,
    rid: u32,
    weight: i32,
    min_level: i32,
    max_level: i32,
    min_cost: Vec<i32>,
    max_cost: Vec<i32>,
    incompatible: Vec<String>,
}

#[derive(Deserialize)]
struct RawItem {
    id: String,
    enchantability: i32,
    is_enchantable: bool,
    primary: Vec<String>,
    supported: Vec<String>,
}

#[derive(Deserialize)]
struct RawTradeSet {
    id: String,
    allow_duplicates: bool,
    random_sequence: Option<String>,
    trades: Vec<String>,
}

#[derive(Deserialize)]
struct RawData {
    version: String,
    enchantments: Vec<RawEnchant>,
    items: Vec<RawItem>,
    enchantment_tags: HashMap<String, Vec<String>>,
    trade_sets: Vec<RawTradeSet>,
    /// Raw `data/minecraft/trade_set/**.json` files from the server jar, keyed by id.
    trade_set_files: HashMap<String, serde_json::Value>,
    /// Raw `data/minecraft/villager_trade/**.json` files from the server jar, keyed by id.
    villager_trade_files: HashMap<String, serde_json::Value>,
    /// Max stack size of every item that appears in a trade.
    max_stack: HashMap<String, i32>,
}

#[derive(Clone, Debug)]
pub struct Enchant {
    pub id: String,
    /// Short name without the namespace (`sharpness`).
    pub name: String,
    /// Registry id: what the enchanting table sends to the client as the clue.
    pub rid: u32,
    pub weight: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub min_cost: Vec<i32>,
    pub max_cost: Vec<i32>,
    /// Bit i set when enchantment i cannot sit next to this one (itself included).
    pub incompatible: u128,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub enchantability: i32,
    pub is_book: bool,
    pub is_enchantable: bool,
    /// The enchanting-table pool for this item, in `#in_enchanting_table` order.
    pub table_pool: Vec<EnchIdx>,
    pub supported: u128,
}

#[derive(Clone, Debug)]
pub enum Modifier {
    /// `minecraft:enchant_randomly`
    EnchantRandomly { options: Vec<EnchIdx>, only_compatible: bool, include_additional_cost: bool },
    /// `minecraft:filtered` with `on_fail: discard` that requires non-empty stored enchantments.
    RequireStoredEnchantments,
}

#[derive(Clone, Debug)]
pub struct ItemCount {
    pub id: String,
    pub count: i32,
}

#[derive(Clone, Debug)]
pub struct TradeDef {
    pub id: String,
    pub wants: ItemCount,
    pub additional_wants: Option<ItemCount>,
    pub gives: ItemCount,
    pub modifiers: Vec<Modifier>,
    pub double_price: u128,
    pub max_uses: i32,
    pub xp: i32,
}

#[derive(Clone, Debug)]
pub struct TradeSetDef {
    pub id: String,
    pub amount: i32,
    pub allow_duplicates: bool,
    pub random_sequence: Option<String>,
    pub trades: Vec<String>,
}

pub struct Data {
    pub version: String,
    pub enchants: Vec<Enchant>,
    pub items: Vec<Item>,
    pub table_order: Vec<EnchIdx>,
    pub tags: HashMap<String, Vec<EnchIdx>>,
    pub trade_sets: HashMap<String, TradeSetDef>,
    /// Trades we can simulate exactly; others are listed in `unsupported_trades` with the reason.
    pub trades: HashMap<String, TradeDef>,
    pub unsupported_trades: HashMap<String, String>,
    pub max_stack: HashMap<String, i32>,
    enchant_by_name: HashMap<String, EnchIdx>,
    item_by_name: HashMap<String, usize>,
    /// level_lut[e][cost]: highest level of `e` whose [min_cost, max_cost] contains `cost`, else 0.
    level_lut: Vec<[u8; 256]>,
}

fn short(id: &str) -> String {
    id.strip_prefix("minecraft:").unwrap_or(id).to_string()
}

fn full(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("minecraft:{name}")
    }
}

pub const DATA_26_3: &str = include_str!("../data/mc-26.3.json");

impl Data {
    pub fn load() -> &'static Data {
        use std::sync::OnceLock;
        static DATA: OnceLock<Data> = OnceLock::new();
        DATA.get_or_init(|| Data::parse(DATA_26_3).expect("embedded data parses"))
    }

    pub fn parse(json: &str) -> Result<Data, String> {
        let raw: RawData = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut raw_enchants = raw.enchantments;
        raw_enchants.sort_by_key(|e| e.rid);
        if raw_enchants.len() > 128 {
            return Err("more than 128 enchantments".into());
        }
        let enchant_by_name: HashMap<String, EnchIdx> =
            raw_enchants.iter().enumerate().map(|(i, e)| (e.id.clone(), i as EnchIdx)).collect();
        let idx = |id: &str| -> Result<EnchIdx, String> {
            enchant_by_name.get(id).copied().ok_or_else(|| format!("unknown enchantment {id}"))
        };
        let mask = |ids: &[String]| -> Result<u128, String> {
            let mut m = 0u128;
            for id in ids {
                m |= 1u128 << idx(id)?;
            }
            Ok(m)
        };
        let mut enchants = Vec::new();
        for e in &raw_enchants {
            enchants.push(Enchant {
                id: e.id.clone(),
                name: short(&e.id),
                rid: e.rid,
                weight: e.weight,
                min_level: e.min_level,
                max_level: e.max_level,
                min_cost: e.min_cost.clone(),
                max_cost: e.max_cost.clone(),
                incompatible: mask(&e.incompatible)?,
            });
        }
        let mut tags = HashMap::new();
        for (tag, ids) in &raw.enchantment_tags {
            let list: Result<Vec<EnchIdx>, String> = ids.iter().map(|id| idx(id)).collect();
            tags.insert(tag.clone(), list?);
        }
        let table_order = tags
            .get("minecraft:in_enchanting_table")
            .cloned()
            .ok_or("missing #minecraft:in_enchanting_table")?;
        let mut items = Vec::new();
        for it in &raw.items {
            let is_book = it.id == "minecraft:book";
            let primary = mask(&it.primary)?;
            let table_pool = table_order.iter().copied().filter(|&e| is_book || primary >> e & 1 == 1).collect();
            items.push(Item {
                id: it.id.clone(),
                name: short(&it.id),
                enchantability: it.enchantability,
                is_book,
                is_enchantable: it.is_enchantable,
                table_pool,
                supported: mask(&it.supported)?,
            });
        }
        let item_by_name = items.iter().enumerate().map(|(i, it)| (it.id.clone(), i)).collect();
        let mut level_lut = Vec::new();
        for e in &enchants {
            let mut lut = [0u8; 256];
            for (cost, slot) in lut.iter_mut().enumerate() {
                for level in (e.min_level..=e.max_level).rev() {
                    let l = (level - 1) as usize;
                    if cost as i32 >= e.min_cost[l] && cost as i32 <= e.max_cost[l] {
                        *slot = level as u8;
                        break;
                    }
                }
            }
            level_lut.push(lut);
        }
        let mut trade_sets = HashMap::new();
        for t in &raw.trade_sets {
            let file = raw.trade_set_files.get(&t.id);
            let amount = file.and_then(|f| f.get("amount")).and_then(|a| a.as_i64()).map(|a| a as i32).unwrap_or(-1);
            trade_sets.insert(
                t.id.clone(),
                TradeSetDef {
                    id: t.id.clone(),
                    amount,
                    allow_duplicates: t.allow_duplicates,
                    random_sequence: t.random_sequence.clone(),
                    trades: t.trades.clone(),
                },
            );
        }
        let mut trades = HashMap::new();
        let mut unsupported_trades = HashMap::new();
        for (id, json) in &raw.villager_trade_files {
            match parse_trade(id, json, &tags, &enchant_by_name) {
                Ok(t) => {
                    trades.insert(id.clone(), t);
                }
                Err(reason) => {
                    unsupported_trades.insert(id.clone(), reason);
                }
            }
        }
        Ok(Data {
            version: raw.version,
            enchants,
            items,
            table_order,
            tags,
            trade_sets,
            trades,
            unsupported_trades,
            max_stack: raw.max_stack,
            enchant_by_name,
            item_by_name,
            level_lut,
        })
    }

    pub fn enchant(&self, name: &str) -> Option<EnchIdx> {
        self.enchant_by_name.get(&full(name)).copied()
    }

    pub fn item(&self, name: &str) -> Option<usize> {
        self.item_by_name.get(&full(name)).copied()
    }

    #[inline]
    pub fn level_for_cost(&self, e: EnchIdx, cost: i32) -> u8 {
        if cost < 0 {
            0
        } else {
            self.level_lut[e as usize][cost.min(255) as usize]
        }
    }

    #[inline]
    pub fn compatible(&self, a: EnchIdx, b: EnchIdx) -> bool {
        self.enchants[a as usize].incompatible >> b & 1 == 0
    }
}

fn int_field(v: &serde_json::Value, key: &str, default: i32) -> Result<i32, String> {
    match v.get(key) {
        None => Ok(default),
        Some(x) => x.as_i64().map(|n| n as i32).ok_or_else(|| format!("{key} is not a constant: {x}")),
    }
}

fn item_count(v: &serde_json::Value) -> Result<ItemCount, String> {
    let id = v.get("id").and_then(|s| s.as_str()).ok_or("item without id")?.to_string();
    if v.get("components").is_some() {
        return Err("item components are not modelled".into());
    }
    Ok(ItemCount { id, count: int_field(v, "count", 1)? })
}

fn holder_set(v: &serde_json::Value, tags: &HashMap<String, Vec<EnchIdx>>, by_name: &HashMap<String, EnchIdx>) -> Result<Vec<EnchIdx>, String> {
    match v {
        serde_json::Value::String(s) if s.starts_with('#') => tags.get(&s[1..]).cloned().ok_or_else(|| format!("unknown tag {s}")),
        serde_json::Value::String(s) => by_name.get(s).map(|&e| vec![e]).ok_or_else(|| format!("unknown enchantment {s}")),
        serde_json::Value::Array(a) => a
            .iter()
            .map(|x| x.as_str().and_then(|s| by_name.get(s).copied()).ok_or_else(|| format!("bad entry {x}")))
            .collect(),
        _ => Err(format!("bad holder set {v}")),
    }
}

/// Parse one villager trade. Anything that could draw from the random sequence and that we do not
/// model makes the trade unsupported (it is never silently approximated).
fn parse_trade(
    id: &str,
    v: &serde_json::Value,
    tags: &HashMap<String, Vec<EnchIdx>>,
    by_name: &HashMap<String, EnchIdx>,
) -> Result<TradeDef, String> {
    let known = ["wants", "additional_wants", "gives", "max_uses", "xp", "reputation_discount", "given_item_modifier", "double_trade_price_enchantments"];
    if let Some(obj) = v.as_object() {
        for k in obj.keys() {
            if !known.contains(&k.as_str()) {
                return Err(format!("field {k} is not modelled"));
            }
        }
    }
    if let Some(r) = v.get("reputation_discount")
        && !r.is_number()
    {
        return Err("reputation_discount is not a constant".into());
    }
    let mut modifiers = Vec::new();
    if let Some(mods) = v.get("given_item_modifier") {
        let list: Vec<&serde_json::Value> = match mods {
            serde_json::Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for m in list {
            let ty = m.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match ty {
                "minecraft:enchant_randomly" => {
                    if m.get("conditions").is_some() {
                        return Err("enchant_randomly conditions are not modelled".into());
                    }
                    let options = match m.get("options") {
                        Some(o) => holder_set(o, tags, by_name)?,
                        None => return Err("enchant_randomly without options is not modelled".into()),
                    };
                    modifiers.push(Modifier::EnchantRandomly {
                        options,
                        only_compatible: m.get("only_compatible").and_then(|b| b.as_bool()).unwrap_or(true),
                        include_additional_cost: m.get("include_additional_cost_component").and_then(|b| b.as_bool()).unwrap_or(false),
                    });
                }
                "minecraft:filtered" => {
                    let filter = m.get("item_filter").ok_or("filtered without item_filter")?;
                    let on_fail = m.get("on_fail").and_then(|f| f.get("type")).and_then(|t| t.as_str());
                    let preds = filter.get("predicates");
                    let wants_stored = preds.and_then(|p| p.get("minecraft:stored_enchantments")).is_some();
                    let items_ok = filter.get("items").and_then(|i| i.as_str()) == Some("minecraft:enchanted_book");
                    if on_fail == Some("minecraft:discard") && wants_stored && items_ok && m.get("on_pass").is_none() {
                        modifiers.push(Modifier::RequireStoredEnchantments);
                    } else {
                        return Err(format!("filtered modifier not modelled: {m}"));
                    }
                }
                other => return Err(format!("modifier {other} is not modelled")),
            }
        }
    }
    let double_price = match v.get("double_trade_price_enchantments") {
        None => 0,
        Some(h) => holder_set(h, tags, by_name)?.iter().fold(0u128, |m, &e| m | 1u128 << e),
    };
    Ok(TradeDef {
        id: id.to_string(),
        wants: item_count(v.get("wants").ok_or("no wants")?)?,
        additional_wants: v.get("additional_wants").map(item_count).transpose()?,
        gives: item_count(v.get("gives").ok_or("no gives")?)?,
        modifiers,
        double_price,
        max_uses: int_field(v, "max_uses", 4)?,
        xp: int_field(v, "xp", 1)?,
    })
}
