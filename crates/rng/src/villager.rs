//! Villager trades in 26.3: `AbstractVillager.addOffersFromTradeSet` with the trade set's world
//! random sequence. Every librarian level-1 trade generation in the world draws from the same
//! stream (`minecraft:trade_set/librarian/level_1`), seeded only by the world seed, so the
//! n-th generation since the world started is fixed. Rerolling the lectern just moves to n+1.

use crate::data::{Data, EnchIdx, ItemCount, Modifier, TradeDef, TradeSetDef};
use crate::table::{format_ench, Ench};
use crate::xoroshiro::{random_sequence, SequenceSettings, Xoroshiro};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub trade: String,
    pub cost_a: (String, i32),
    pub cost_b: Option<(String, i32)>,
    pub result: (String, i32),
    /// Stored enchantments on an enchanted book result.
    pub book: Option<Ench>,
    pub max_uses: i32,
    pub xp: i32,
}

impl Offer {
    pub fn describe(&self, data: &Data) -> String {
        let s = |(id, n): &(String, i32)| format!("{} {}", n, id.strip_prefix("minecraft:").unwrap_or(id));
        let mut out = s(&self.cost_a);
        if let Some(b) = &self.cost_b {
            out += &format!(" + {}", s(b));
        }
        out += " -> ";
        match self.book {
            Some(e) => out += &format!("enchanted book ({})", format_ench(data, e)),
            None => out += &s(&self.result),
        }
        out
    }
}

fn get_offer(data: &Data, t: &TradeDef, rng: &mut Xoroshiro) -> Option<Offer> {
    let mut result = t.gives.clone();
    let mut book: Option<Ench> = None;
    let mut additional_cost = 0;
    let mut discarded = false;
    for m in &t.modifiers {
        match m {
            Modifier::EnchantRandomly { options, only_compatible, include_additional_cost } => {
                let target_is_book = result.id == "minecraft:book";
                let check = !target_is_book && *only_compatible;
                let candidates: Vec<EnchIdx> = if check {
                    let item = data.item(&result.id);
                    options
                        .iter()
                        .copied()
                        .filter(|&e| item.map(|i| data.items[i].supported >> e & 1 == 1).unwrap_or(false))
                        .collect()
                } else {
                    options.clone()
                };
                if candidates.is_empty() {
                    continue;
                }
                let e = candidates[rng.next_int_bound(candidates.len() as i32) as usize];
                let ench = &data.enchants[e as usize];
                let level = rng.next_int_between(ench.min_level, ench.max_level);
                if target_is_book {
                    result = ItemCount { id: "minecraft:enchanted_book".into(), count: 1 };
                }
                book = Some((e, level as u8));
                if *include_additional_cost {
                    additional_cost = 2 + rng.next_int_bound(5 + level * 10) + 3 * level;
                }
            }
            Modifier::RequireStoredEnchantments => {
                if !(result.id == "minecraft:enchanted_book" && book.is_some()) {
                    discarded = true;
                }
            }
        }
        if discarded {
            return None;
        }
    }
    if let Some((e, _)) = book
        && t.double_price >> e & 1 == 1
    {
        additional_cost *= 2;
    }
    let max = |id: &str| data.max_stack.get(id).copied().unwrap_or(64);
    let count_a = (t.wants.count + additional_cost).clamp(0, max(&t.wants.id));
    if count_a < 1 {
        return None;
    }
    let cost_b = t.additional_wants.as_ref().map(|w| (w.id.clone(), w.count.clamp(0, max(&w.id))));
    if let Some((_, n)) = &cost_b
        && *n < 1
    {
        return None;
    }
    Some(Offer {
        trade: t.id.clone(),
        cost_a: (t.wants.id.clone(), count_a),
        cost_b,
        result: (result.id, result.count),
        book,
        max_uses: t.max_uses.max(1),
        xp: t.xp.max(0),
    })
}

/// One trade generation (what a villager gets when its trades for this level are created).
pub fn generate(data: &Data, set: &TradeSetDef, rng: &mut Xoroshiro) -> Result<Vec<Offer>, String> {
    let mut pool: Vec<&TradeDef> = Vec::new();
    for id in &set.trades {
        match data.trades.get(id) {
            Some(t) => pool.push(t),
            None => {
                let why = data.unsupported_trades.get(id).cloned().unwrap_or_else(|| "missing".into());
                return Err(format!("trade {id} cannot be simulated: {why}"));
            }
        }
    }
    if set.amount < 0 {
        return Err(format!("trade set {} has a non-constant amount", set.id));
    }
    let mut offers = Vec::new();
    let mut found = 0;
    if set.allow_duplicates {
        while found < set.amount && !pool.is_empty() {
            let roll = rng.next_int_bound(pool.len() as i32) as usize;
            match get_offer(data, pool[roll], rng) {
                None => {
                    pool.remove(roll);
                }
                Some(o) => {
                    offers.push(o);
                    found += 1;
                }
            }
        }
    } else {
        while found < set.amount && !pool.is_empty() {
            let t = pool.remove(rng.next_int_bound(pool.len() as i32) as usize);
            if let Some(o) = get_offer(data, t, rng) {
                offers.push(o);
                found += 1;
            }
        }
    }
    Ok(offers)
}

/// The trade set a profession uses at a level, e.g. ("librarian", 1).
pub fn trade_set<'a>(data: &'a Data, profession: &str, level: i32) -> Result<&'a TradeSetDef, String> {
    let id = format!("minecraft:{profession}/level_{level}");
    data.trade_sets.get(&id).ok_or_else(|| format!("no trade set {id}"))
}

/// Iterates the generations of one trade set's world random sequence from its creation.
pub struct Generations<'a> {
    data: &'a Data,
    set: &'a TradeSetDef,
    rng: Xoroshiro,
    pub index: u64,
}

impl<'a> Generations<'a> {
    pub fn new(data: &'a Data, set: &'a TradeSetDef, world_seed: i64, settings: SequenceSettings) -> Result<Self, String> {
        let key = set.random_sequence.as_ref().ok_or_else(|| format!("{} has no random sequence", set.id))?;
        Ok(Self { data, set, rng: random_sequence(world_seed, key, settings), index: 0 })
    }

    /// From an explicit generator state (e.g. read from `random_sequences.dat`).
    pub fn from_state(data: &'a Data, set: &'a TradeSetDef, rng: Xoroshiro, index: u64) -> Self {
        Self { data, set, rng, index }
    }

    pub fn state(&self) -> Xoroshiro {
        self.rng
    }

    pub fn next_generation(&mut self) -> Result<Vec<Offer>, String> {
        let offers = generate(self.data, self.set, &mut self.rng)?;
        self.index += 1;
        Ok(offers)
    }
}

/// What the player wrote down from one fresh trade screen. Only the parts given are compared.
#[derive(Clone, Debug, Default)]
pub struct ObservedTrades {
    /// The book on offer: enchantment, level and (optionally) the emerald price.
    pub book: Option<(EnchIdx, u8, Option<i32>)>,
    /// True when the screen had no enchanted book at all.
    pub no_book: bool,
    /// Other result items seen (e.g. "minecraft:bookshelf").
    pub items: Vec<String>,
}

pub fn observation_matches(offers: &[Offer], obs: &ObservedTrades) -> bool {
    let book = offers.iter().find_map(|o| o.book.map(|b| (b, o.cost_a.1)));
    if obs.no_book && book.is_some() {
        return false;
    }
    if let Some((e, l, price)) = obs.book {
        match book {
            Some(((be, bl), p)) => {
                if be != e || bl != l || price.is_some_and(|x| x != p) {
                    return false;
                }
            }
            None => return false,
        }
    }
    obs.items.iter().all(|it| offers.iter().any(|o| &o.result.0 == it))
}

/// Generation indices n (0 = first ever) where generations n, n+1, ... match the observations.
pub fn sync(
    data: &Data,
    set: &TradeSetDef,
    world_seed: i64,
    settings: SequenceSettings,
    observations: &[ObservedTrades],
    max_generations: u64,
) -> Result<Vec<u64>, String> {
    let mut gens = Generations::new(data, set, world_seed, settings)?;
    let mut window: std::collections::VecDeque<Vec<Offer>> = std::collections::VecDeque::new();
    let mut hits = Vec::new();
    let k = observations.len().max(1);
    while gens.index < max_generations + k as u64 {
        window.push_back(gens.next_generation()?);
        if window.len() > k {
            window.pop_front();
        }
        if window.len() == k && window.iter().zip(observations).all(|(o, obs)| observation_matches(o, obs)) {
            hits.push(gens.index - k as u64);
        }
    }
    Ok(hits)
}

#[derive(Clone, Debug)]
pub struct Want {
    pub enchant: EnchIdx,
    pub min_level: u8,
    pub max_price: Option<i32>,
}

/// How many rerolls, and every generation on the way.
pub type RerollPlan = (u64, Vec<Vec<Offer>>);

/// The first generation after `last_seen` whose book satisfies `want`, with every generation on
/// the way (so the player can check they stay in sync while rerolling).
pub fn plan_rerolls(
    data: &Data,
    set: &TradeSetDef,
    world_seed: i64,
    settings: SequenceSettings,
    last_seen: u64,
    want: &Want,
    max_rerolls: u64,
) -> Result<Option<RerollPlan>, String> {
    let mut gens = Generations::new(data, set, world_seed, settings)?;
    while gens.index <= last_seen {
        gens.next_generation()?;
    }
    let mut path = Vec::new();
    for reroll in 1..=max_rerolls {
        let offers = gens.next_generation()?;
        let ok = offers.iter().any(|o| match o.book {
            Some((e, l)) => e == want.enchant && l >= want.min_level && want.max_price.is_none_or(|p| o.cost_a.1 <= p),
            None => false,
        });
        path.push(offers);
        if ok {
            return Ok(Some((reroll, path)));
        }
    }
    Ok(None)
}
