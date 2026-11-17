//! Finding the player's enchanting seed (the `XpSeed` the table is seeded with) from what the
//! enchanting table shows: three level costs at a bookshelf count, and the hover hints if read.

use crate::data::{Data, EnchIdx};
use crate::java_random::JavaRandom;
use crate::table::{offers, slot_costs};

/// One look at the table: the bookshelf count, the three level requirements and, optionally, the
/// item and the hover hints (`None` = not read / don't care).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub shelves: i32,
    pub costs: [i32; 3],
    pub item: Option<usize>,
    pub clues: [Option<(EnchIdx, u8)>; 3],
}

impl Observation {
    pub fn costs_only(shelves: i32, costs: [i32; 3]) -> Self {
        Self { shelves, costs, item: None, clues: [None; 3] }
    }

    fn has_clues(&self) -> bool {
        self.item.is_some() && self.clues.iter().any(|c| c.is_some())
    }
}

/// Cheap first pass: do the three level requirements match? (`slot_costs` with an early exit
/// after each slot: two `nextInt` calls per slot.)
#[inline]
pub fn costs_match(seed: i32, obs: &Observation) -> bool {
    let mut rng = JavaRandom::new(seed as i64);
    let b = obs.shelves.min(15);
    for slot in 0..3 {
        let selected = rng.next_int_bound(8) + 1 + (b >> 1) + rng.next_int_bound(b + 1);
        let cost = match slot {
            0 => (selected / 3).max(1),
            1 => selected * 2 / 3 + 1,
            _ => selected.max(b * 2),
        };
        let cost = if cost < slot + 1 { 0 } else { cost };
        if cost != obs.costs[slot as usize] {
            return false;
        }
    }
    true
}

pub fn matches(data: &Data, seed: i32, obs: &Observation) -> bool {
    if !costs_match(seed, obs) {
        return false;
    }
    if !obs.has_clues() {
        return true;
    }
    let item = &data.items[obs.item.unwrap()];
    let o = offers(data, seed, item, obs.shelves);
    (0..3).all(|i| obs.clues[i].is_none() || obs.clues[i] == o[i].clue)
}

/// Keep the candidates that agree with every observation.
pub fn filter(data: &Data, candidates: &[i32], observations: &[Observation]) -> Vec<i32> {
    candidates.iter().copied().filter(|&s| observations.iter().all(|o| matches(data, s, o))).collect()
}

/// The seeds `start .. start + count` (as unsigned 32-bit values, wrapping) that agree with every
/// observation: one slice of the full search (the browser's workers each take slices).
pub fn crack_range(data: &Data, observations: &[Observation], start: u32, count: u32) -> Vec<i32> {
    let Some(first) = observations.first() else { return Vec::new() };
    let mut found = Vec::new();
    for i in 0..count {
        let seed = start.wrapping_add(i) as i32;
        if costs_match(seed, first) && observations.iter().all(|o| matches(data, seed, o)) {
            found.push(seed);
        }
    }
    found
}

/// Search all 2^32 seeds on `threads` threads (0: the machine's), sorted.
#[cfg(not(target_arch = "wasm32"))]
pub fn crack_xp_seed(data: &Data, observations: &[Observation], threads: usize) -> Vec<i32> {
    const CHUNK: u64 = 1 << 22;
    let chunks = (1u64 << 32) / CHUNK;
    let threads = if threads == 0 { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) } else { threads };
    let next = std::sync::atomic::AtomicU64::new(0);
    let mut found: Vec<i32> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let c = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if c >= chunks {
                            break;
                        }
                        mine.extend(crack_range(data, observations, (c * CHUNK) as u32, CHUNK as u32));
                    }
                    mine
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    found.sort_unstable();
    found
}

/// The level requirements alone for a given seed (handy for printing).
pub fn costs_for(seed: i32, shelves: i32) -> [i32; 3] {
    let mut rng = JavaRandom::new(seed as i64);
    slot_costs(&mut rng, shelves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Data;

    #[test]
    fn readings_pin_one_seed() {
        // the seed 905F2AFF read at nine bookshelf counts (costs only), then three with hints
        let data = Data::load();
        let secret = 0x905F2AFFu32 as i32;
        let sword = data.item("diamond_sword").unwrap();
        let obs: Vec<Observation> = [15, 8, 3, 1, 12, 6, 10, 0, 14].iter().map(|&b| Observation::costs_only(b, costs_for(secret, b))).collect();
        // (a slice around the secret: the full search is mctas rng crack's)
        let found = crack_range(data, &obs, (secret as u32).wrapping_sub(1 << 20), 1 << 21);
        assert_eq!(found, vec![secret]);
        let with_hints: Vec<Observation> = [15, 8, 3]
            .iter()
            .map(|&b| {
                let o = offers(data, secret, &data.items[sword], b);
                Observation { shelves: b, costs: [o[0].cost, o[1].cost, o[2].cost], item: Some(sword), clues: [o[0].clue, o[1].clue, o[2].clue] }
            })
            .collect();
        assert!(with_hints.iter().all(|o| matches(data, secret, o)));
    }
}
