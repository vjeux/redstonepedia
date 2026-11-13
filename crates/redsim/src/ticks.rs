//! Scheduled block ticks: `LevelTicks` + `LevelChunkTicks` + `ScheduledTick`.
//!
//! * One container per chunk, a queue ordered by (trigger tick, priority, sub-tick order) and a
//!   set of (position, block) that refuses a second pending tick for the same pair.
//! * Each tick, every due tick is collected first (containers merged by (priority, sub-tick
//!   order) of their next tick: `INTRA_TICK_DRAIN_ORDER`), then run in that order; ticks
//!   scheduled meanwhile wait for a later tick even with delay 0.
//! * `willTickThisTick` = collected for this tick and not run yet; `hasScheduledTick` = still in
//!   its container (a collected tick no longer is).
//! * The containers due together are taken in the game's own order (tickexact): its
//!   `nextTickForContainer` (fastutil's `Long2LongOpenHashMap`, walked by its fast iterator) fills
//!   `containersToTick` (a `java.util.PriorityQueue`), so two containers whose heads tie on
//!   (priority, sub-tick order) run in the game's order. Sub-tick orders are unique until a chunk
//!   reload ([`LevelTicks::reload`]: every reloaded container numbers its ticks -N..-1), so the
//!   ties, and with them this order, only matter after one.

use crate::block::Kind;
use crate::pos::BlockPos;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::Hash;

/// What a tick list is keyed by besides the position: the block (`Kind`) for block ticks, the
/// fluid (`fluid::Fluid`) for fluid ticks.
pub trait TickKey: Copy + Eq + Hash + Ord + std::fmt::Debug {}
impl<T: Copy + Eq + Hash + Ord + std::fmt::Debug> TickKey for T {}

/// `TickPriority` (lower runs first).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Priority {
    ExtremelyHigh = -3,
    VeryHigh = -2,
    High = -1,
    Normal = 0,
    Low = 1,
    VeryLow = 2,
    ExtremelyLow = 3,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Tick<K: TickKey = Kind> {
    pub kind: K,
    pub pos: BlockPos,
    pub trigger: i64,
    pub priority: Priority,
    pub sub: i64,
}

#[derive(Clone)]
struct Container<K: TickKey> {
    /// (trigger, priority, sub) -> tick: `DRAIN_ORDER`.
    queue: BTreeSet<(i64, Priority, i64, BlockPos, K)>,
    pending: HashSet<(BlockPos, K)>,
}

impl<K: TickKey> Default for Container<K> {
    fn default() -> Self {
        Container { queue: BTreeSet::new(), pending: HashSet::new() }
    }
}

/// `ChunkPos.pack(x, z)`: the long the game keys a chunk's container by.
pub fn chunk_key(c: (i32, i32)) -> i64 {
    (c.0 as i64 & 0xFFFF_FFFF) | ((c.1 as i64 & 0xFFFF_FFFF) << 32)
}

fn chunk_of_key(k: i64) -> (i32, i32) {
    (k as i32, (k >> 32) as i32)
}

/// fastutil's `HashCommon.mix(long)`.
fn mix(x: i64) -> i64 {
    let h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15_u64 as i64);
    let h = h ^ ((h as u64) >> 32) as i64;
    h ^ ((h as u64) >> 16) as i64
}

/// fastutil 8.5.18's `Long2LongOpenHashMap` as `LevelTicks.nextTickForContainer` uses it (16
/// expected, load factor 0.75; `put`, `remove`, the fast iterator with its `remove` and
/// `setValue`): the table layout, so its walk order is the game's. Key 0 (chunk 0, 0) is the
/// map's null key, kept in slot `n` and walked first.
#[derive(Clone, Debug)]
struct LongMap {
    key: Vec<i64>,
    value: Vec<i64>,
    n: usize,
    mask: usize,
    min_n: usize,
    max_fill: usize,
    size: usize,
    contains_null: bool,
}

/// What a walk does with the entry it is at.
enum Walk {
    Keep,
    Set(i64),
    Remove,
}

fn array_size(expected: usize, f: f32) -> usize {
    let s = ((expected as f64) / (f as f64)).ceil() as u64;
    s.max(2).next_power_of_two() as usize
}

fn max_fill(n: usize, f: f32) -> usize {
    (((n as f64) * (f as f64)).ceil() as usize).min(n - 1)
}

const LOAD: f32 = 0.75;

impl Default for LongMap {
    fn default() -> Self {
        let n = array_size(16, LOAD);
        LongMap { key: vec![0; n + 1], value: vec![0; n + 1], n, mask: n - 1, min_n: n, max_fill: max_fill(n, LOAD), size: 0, contains_null: false }
    }
}

impl LongMap {
    fn slot(&self, k: i64) -> usize {
        ((mix(k) as i32) as usize) & self.mask
    }

    fn real_size(&self) -> usize {
        if self.contains_null { self.size - 1 } else { self.size }
    }

    fn get(&self, k: i64) -> Option<i64> {
        if k == 0 {
            return self.contains_null.then(|| self.value[self.n]);
        }
        let mut pos = self.slot(k);
        loop {
            let curr = self.key[pos];
            if curr == 0 {
                return None;
            }
            if curr == k {
                return Some(self.value[pos]);
            }
            pos = (pos + 1) & self.mask;
        }
    }

    fn put(&mut self, k: i64, v: i64) {
        let pos = if k == 0 {
            if self.contains_null {
                self.value[self.n] = v;
                return;
            }
            self.contains_null = true;
            self.n
        } else {
            let mut pos = self.slot(k);
            loop {
                let curr = self.key[pos];
                if curr == 0 {
                    break pos;
                }
                if curr == k {
                    self.value[pos] = v;
                    return;
                }
                pos = (pos + 1) & self.mask;
            }
        };
        self.key[pos] = k;
        self.value[pos] = v;
        let was = self.size;
        self.size += 1;
        if was >= self.max_fill {
            self.rehash(array_size(self.size + 1, LOAD));
        }
    }

    fn remove(&mut self, k: i64) {
        if k == 0 {
            if self.contains_null {
                self.contains_null = false;
                self.size -= 1;
                self.maybe_shrink();
            }
            return;
        }
        let mut pos = self.slot(k);
        loop {
            let curr = self.key[pos];
            if curr == 0 {
                return;
            }
            if curr == k {
                self.size -= 1;
                self.shift_keys(pos, None);
                self.maybe_shrink();
                return;
            }
            pos = (pos + 1) & self.mask;
        }
    }

    fn maybe_shrink(&mut self) {
        if self.n > self.min_n && self.size < self.max_fill / 4 && self.n > 16 {
            self.rehash(self.n / 2);
        }
    }

    /// `shiftKeys(pos)`; the iterator's version also notes an entry moved from below the walk's
    /// position to above it (`wrapped`).
    fn shift_keys(&mut self, mut pos: usize, mut wrapped: Option<&mut Vec<i64>>) {
        loop {
            let last = pos;
            pos = (last + 1) & self.mask;
            let curr = loop {
                let curr = self.key[pos];
                if curr == 0 {
                    self.key[last] = 0;
                    return;
                }
                let slot = self.slot(curr);
                if if last <= pos { last >= slot || slot > pos } else { last >= slot && slot > pos } {
                    break curr;
                }
                pos = (pos + 1) & self.mask;
            };
            if pos < last
                && let Some(w) = wrapped.as_deref_mut()
            {
                w.push(self.key[pos]);
            }
            self.key[last] = curr;
            self.value[last] = self.value[pos];
        }
    }

    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut new_key = vec![0i64; new_n + 1];
        let mut new_value = vec![0i64; new_n + 1];
        let mut i = self.n;
        for _ in 0..self.real_size() {
            loop {
                i -= 1;
                if self.key[i] != 0 {
                    break;
                }
            }
            let mut pos = ((mix(self.key[i]) as i32) as usize) & mask;
            while new_key[pos] != 0 {
                pos = (pos + 1) & mask;
            }
            new_key[pos] = self.key[i];
            new_value[pos] = self.value[i];
        }
        new_value[new_n] = self.value[self.n];
        self.n = new_n;
        self.mask = mask;
        self.max_fill = max_fill(new_n, LOAD);
        self.key = new_key;
        self.value = new_value;
    }

    /// The fast iterator's walk (`Long2LongMaps.fastIterator`): the null key first, then the slots
    /// from the top down, then the entries a removal wrapped; `f` keeps, sets or removes each.
    fn walk(&mut self, mut f: impl FnMut(i64, i64) -> Walk) {
        let mut pos = self.n as i64;
        let mut c = self.size;
        let mut must_null = self.contains_null;
        let mut wrapped: Vec<i64> = Vec::new();
        while c != 0 {
            c -= 1;
            // nextEntry
            let (idx, from_wrapped) = if must_null {
                must_null = false;
                (self.n, false)
            } else {
                loop {
                    pos -= 1;
                    if pos < 0 {
                        let k = wrapped[(-pos - 1) as usize];
                        let mut p = self.slot(k);
                        while self.key[p] != k {
                            p = (p + 1) & self.mask;
                        }
                        break (p, true);
                    }
                    if self.key[pos as usize] != 0 {
                        break (pos as usize, false);
                    }
                }
            };
            let k = if idx == self.n { 0 } else { self.key[idx] };
            match f(k, self.value[idx]) {
                Walk::Keep => {}
                Walk::Set(v) => self.value[idx] = v,
                Walk::Remove => {
                    if idx == self.n && !from_wrapped {
                        self.contains_null = false;
                        self.size -= 1;
                    } else if !from_wrapped {
                        self.shift_keys(idx, Some(&mut wrapped));
                        self.size -= 1;
                    } else {
                        // a wrapped entry: the map's own remove (its size counted there)
                        self.remove(k);
                    }
                }
            }
        }
    }
}

/// `java.util.PriorityQueue` with a comparator (its binary heap's exact sift order): the game's
/// `containersToTick`, of chunk keys.
#[derive(Default)]
struct JavaHeap {
    q: Vec<i64>,
}

impl JavaHeap {
    fn offer(&mut self, x: i64, cmp: &impl Fn(i64, i64) -> std::cmp::Ordering) {
        let mut k = self.q.len();
        self.q.push(x);
        while k > 0 {
            let parent = (k - 1) >> 1;
            let e = self.q[parent];
            if cmp(x, e) != std::cmp::Ordering::Less {
                break;
            }
            self.q[k] = e;
            k = parent;
        }
        self.q[k] = x;
    }

    fn peek(&self) -> Option<i64> {
        self.q.first().copied()
    }

    fn poll(&mut self, cmp: &impl Fn(i64, i64) -> std::cmp::Ordering) -> Option<i64> {
        if self.q.is_empty() {
            return None;
        }
        let result = self.q[0];
        let x = self.q.pop().unwrap();
        let n = self.q.len();
        if n > 0 {
            let mut k = 0;
            let half = n >> 1;
            while k < half {
                let mut child = 2 * k + 1;
                let mut c = self.q[child];
                let right = child + 1;
                if right < n && cmp(c, self.q[right]) == std::cmp::Ordering::Greater {
                    child = right;
                    c = self.q[child];
                }
                if cmp(x, c) != std::cmp::Ordering::Greater {
                    break;
                }
                self.q[k] = c;
                k = child;
            }
            self.q[k] = x;
        }
        Some(result)
    }
}

#[derive(Clone)]
pub struct LevelTicks<K: TickKey = Kind> {
    containers: HashMap<(i32, i32), Container<K>>,
    /// `nextTickForContainer`: each container's head's trigger tick, by chunk key.
    next: LongMap,
    to_run: HashSet<(BlockPos, K)>,
}

/// (tickexact 2026-11-10) Chunks' ticks as their chunks saved them ([`LevelTicks::unload`]): by
/// chunk, in sub-tick order, (trigger, priority, cell, kind).
pub type SavedTicks<K> = HashMap<(i32, i32), Vec<(i64, Priority, BlockPos, K)>>;

impl<K: TickKey> Default for LevelTicks<K> {
    fn default() -> Self {
        LevelTicks { containers: HashMap::new(), next: LongMap::default(), to_run: HashSet::new() }
    }
}

impl<K: TickKey> LevelTicks<K> {
    pub fn schedule(&mut self, kind: K, pos: BlockPos, trigger: i64, priority: Priority, sub: i64) {
        let chunk = pos.chunk();
        let c = self.containers.entry(chunk).or_default();
        if c.pending.insert((pos, kind)) {
            let t = (trigger, priority, sub, pos, kind);
            c.queue.insert(t);
            // onTickAdded: the new tick is the container's head -> updateContainerScheduling
            if c.queue.first() == Some(&t) {
                self.next.put(chunk_key(chunk), trigger);
            }
        }
    }

    pub fn has_scheduled_tick(&self, pos: BlockPos, kind: K) -> bool {
        self.containers.get(&pos.chunk()).is_some_and(|c| c.pending.contains(&(pos, kind)))
    }

    pub fn will_tick_this_tick(&self, pos: BlockPos, kind: K) -> bool {
        self.to_run.contains(&(pos, kind))
    }

    /// Pending ticks (all containers).
    pub fn count(&self) -> usize {
        self.containers.values().map(|c| c.queue.len()).sum()
    }

    /// Every pending tick moved `delta` ticks later, in the same order: a chunk saved and loaded
    /// again (`LevelChunkTicks.save` keeps each tick's delay, `SavedTick`; the load schedules it
    /// at the new game time + that delay).
    pub fn shift(&mut self, delta: i64) {
        for c in self.containers.values_mut() {
            c.queue = std::mem::take(&mut c.queue).into_iter().map(|(t, p, s, pos, k)| (t + delta, p, s, pos, k)).collect();
        }
        let keys: Vec<(i64, i64)> = self.containers.iter().filter_map(|(ch, c)| c.queue.first().map(|h| (chunk_key(*ch), h.0))).collect();
        for (k, t) in keys {
            if self.next.get(k).is_some() {
                self.next.put(k, t);
            }
        }
    }

    /// A chunk reload with the game time `delta` ticks later at the load: each chunk of `unload`
    /// saves its ticks (`LevelChunkTicks.pack`: in sub-tick order, each with its delay and
    /// priority) and leaves the level (`removeContainer`); then each chunk of `load`, in that
    /// order, comes back and starts ticking (`unpack`): its saved ticks scheduled again in their
    /// saved order with sub-tick orders -N..-1, its head put in `nextTickForContainer`. A chunk
    /// left out of `load` keeps its ticks saved (`pending` until it loads).
    pub fn reload(&mut self, unload: &[(i32, i32)], load: &[(i32, i32)], delta: i64) {
        let mut saved = self.unload(unload);
        self.load(load, &mut saved, delta);
        // chunks not loaded again: their saved ticks wait (not modeled: every reload loads back
        // the chunks it unloads)
        debug_assert!(saved.is_empty(), "a reload left chunks unloaded");
    }

    /// (tickexact 2026-11-10) The first half of a reload: `chunks`' ticks taken out as their chunks
    /// save them (`LevelChunkTicks.pack`: in sub-tick order, with their trigger times; [`Self::load`]
    /// gives them back shifted by the time away).
    pub fn unload(&mut self, chunks: &[(i32, i32)]) -> SavedTicks<K> {
        let mut saved: SavedTicks<K> = HashMap::new();
        for ch in chunks {
            self.next.remove(chunk_key(*ch));
            if let Some(c) = self.containers.remove(ch) {
                let mut ticks: Vec<_> = c.queue.into_iter().collect();
                ticks.sort_by_key(|t| t.2);
                saved.insert(*ch, ticks.into_iter().map(|(t, p, _, pos, k)| (t, p, pos, k)).collect());
            }
        }
        saved
    }

    /// (tickexact 2026-11-10) The second half: each of `chunks` (in load order) gets its saved
    /// ticks back, each delay kept (`unpack`: the trigger moved by `delta`, the time away),
    /// numbered -N..-1 in its saved order.
    pub fn load(&mut self, chunks: &[(i32, i32)], saved: &mut SavedTicks<K>, delta: i64) {
        for ch in chunks {
            let Some(ticks) = saved.remove(ch) else { continue };
            let n = ticks.len() as i64;
            for (i, (trigger, priority, pos, kind)) in ticks.into_iter().enumerate() {
                self.schedule(kind, pos, trigger + delta, priority, i as i64 - n);
            }
        }
    }

    /// Every pending tick, for debugging and comparisons.
    pub fn all(&self) -> Vec<Tick<K>> {
        let mut v: Vec<Tick<K>> = self
            .containers
            .values()
            .flat_map(|c| c.queue.iter().map(|&(trigger, priority, sub, pos, kind)| Tick { kind, pos, trigger, priority, sub }))
            .collect();
        v.sort_by_key(|t| (t.trigger, t.priority, t.sub));
        v
    }

    /// Collect the ticks due at `now`, in run order (`collectTicks`: `sortContainersToTick`,
    /// `drainContainers`).
    pub fn collect(&mut self, now: i64) -> Vec<Tick<K>> {
        self.collect_if(now, |_| true)
    }

    /// (tickexact 2026-11-10, a partial unload) [`Self::collect`] where only the chunks `ticking`
    /// accepts tick (`LevelTicks`' tick check, `isPositionTickingWithEntitiesLoaded`): the others'
    /// due ticks stay where they are (their next tick unchanged) and come due when they tick
    /// again.
    pub fn collect_if(&mut self, now: i64, ticking: impl Fn((i32, i32)) -> bool) -> Vec<Tick<K>> {
        let mut out = Vec::new();
        let containers = &mut self.containers;
        // sortContainersToTick: the map's walk, each due container into the heap
        let mut heap = JavaHeap::default();
        let head = |cs: &HashMap<(i32, i32), Container<K>>, k: i64| cs.get(&chunk_of_key(k)).and_then(|c| c.queue.first().map(|h| (h.1, h.2)));
        {
            let cs: &HashMap<(i32, i32), Container<K>> = containers;
            let cmp = |a: i64, b: i64| head(cs, a).cmp(&head(cs, b));
            let mut due: Vec<i64> = Vec::new();
            self.next.walk(|k, next| {
                if next > now {
                    return Walk::Keep;
                }
                match cs.get(&chunk_of_key(k)).and_then(|c| c.queue.first()) {
                    None => Walk::Remove,
                    Some(h) if h.0 > now => Walk::Set(h.0),
                    Some(_) if !ticking(chunk_of_key(k)) => Walk::Keep,
                    Some(_) => {
                        due.push(k);
                        Walk::Remove
                    }
                }
            });
            for k in due {
                heap.offer(k, &cmp);
            }
        }
        // drainContainers
        loop {
            let top = {
                let cs: &HashMap<(i32, i32), Container<K>> = containers;
                let cmp = |a: i64, b: i64| head(cs, a).cmp(&head(cs, b));
                match heap.poll(&cmp) {
                    Some(t) => t,
                    None => break,
                }
            };
            let next_best = heap.peek().and_then(|k| head(containers, k));
            let c = containers.get_mut(&chunk_of_key(top)).unwrap();
            let first = c.queue.pop_first().unwrap();
            c.pending.remove(&(first.3, first.4));
            out.push(Tick { kind: first.4, pos: first.3, trigger: first.0, priority: first.1, sub: first.2 });
            // drainFromCurrentContainer
            while let Some(&h) = c.queue.first() {
                if h.0 > now || next_best.is_some_and(|nb| (h.1, h.2) > nb) {
                    break;
                }
                c.queue.pop_first();
                c.pending.remove(&(h.3, h.4));
                out.push(Tick { kind: h.4, pos: h.3, trigger: h.0, priority: h.1, sub: h.2 });
            }
            if let Some(&h) = c.queue.first() {
                if h.0 <= now {
                    let cs: &HashMap<(i32, i32), Container<K>> = containers;
                    let cmp = |a: i64, b: i64| head(cs, a).cmp(&head(cs, b));
                    heap.offer(top, &cmp);
                } else {
                    self.next.put(top, h.0);
                }
            }
        }
        self.to_run = out.iter().map(|t| (t.pos, t.kind)).collect();
        out
    }

    /// A collected tick is about to run (`runCollectedTicks`).
    pub fn start_running(&mut self, t: &Tick<K>) {
        self.to_run.remove(&(t.pos, t.kind));
    }

    pub fn end_tick(&mut self) {
        self.to_run.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_map_walks_like_fastutil() {
        // the null key first, then the slots from the top down
        let mut m = LongMap::default();
        for k in [chunk_key((0, 0)), chunk_key((-1, 0)), chunk_key((0, -1)), chunk_key((1, 0)), chunk_key((0, 1))] {
            m.put(k, 7);
        }
        let mut seen = Vec::new();
        m.walk(|k, _| {
            seen.push(k);
            Walk::Keep
        });
        assert_eq!(seen[0], 0);
        let mut slots: Vec<usize> = seen[1..].iter().map(|k| m.slot(*k)).collect();
        let sorted = {
            let mut s = slots.clone();
            s.sort_by(|a, b| b.cmp(a));
            s
        };
        assert_eq!(slots, sorted);
        // removing every entry in a walk empties it
        m.walk(|_, _| Walk::Remove);
        assert_eq!(m.size, 0);
        slots.clear();
    }

    #[test]
    fn a_reload_numbers_each_chunk_from_minus_n() {
        let mut t: LevelTicks<u8> = LevelTicks::default();
        // two ticks due together in two chunks, the east one scheduled first
        t.schedule(1, BlockPos::new(16, 0, 0), 10, Priority::Normal, 5);
        t.schedule(2, BlockPos::new(15, 0, 0), 10, Priority::Normal, 6);
        let before: Vec<u8> = t.clone().collect(10).iter().map(|x| x.kind).collect();
        assert_eq!(before, vec![1, 2]);
        t.reload(&[(0, 0), (1, 0)], &[(0, 0), (1, 0)], 0);
        // both now sub -1: chunk (0, 0) (the map's null key, walked first) wins the tie
        let after: Vec<u8> = t.collect(10).iter().map(|x| x.kind).collect();
        assert_eq!(after, vec![2, 1]);
    }

    /// (cc767b78; tick-exact 15:25 from 26.3's `LevelChunkTicks.unpack` 109–119, `int subTickBase =
    /// -this.pendingTicks.size(); … pendingTick.unpack(currentTick, subTickBase++)`, and
    /// `SavedTick.unpack` 51–53; ironspec 14:53) A load's overdue ticks: two chunks' saved ticks, due
    /// at save + 1 and save + 2 in each, numbered -2, -1 per chunk; the world then idles past both
    /// (a world open's idle: no block tick runs). At the first block tick every one is due: in each
    /// chunk in due order; across the chunks the heads compare on (priority, sub), the first tie broken
    /// by the containers' heap order (A-2, then B-2); and `drainFromCurrentContainer` keeps draining the
    /// current chunk while its next tick is not STRICTLY after the other chunk's head (26.3
    /// `LevelTicks.drainFromCurrentContainer` line 164, `|| nextFromNextContainer != null &&
    /// ScheduledTick.INTRA_TICK_DRAIN_ORDER.compare(nextFromCurrentContainer, nextFromNextContainer) > 0)`
    /// then `break;`: a tie does not break; tick-exact 15:38), so B's -1 runs before A's -1: A-2, B-2,
    /// B-1, A-1. A tick scheduled after the load (a positive sub from the level's counter) runs after
    /// all of them at equal priority, even with an earlier trigger.
    #[test]
    fn a_loads_overdue_ticks_interleave_across_chunks() {
        let mut t: LevelTicks<u8> = LevelTicks::default();
        let (a, b, c) = (BlockPos::new(1, 0, 1), BlockPos::new(17, 0, 1), BlockPos::new(33, 0, 1));
        // saved at game time 10: chunk A's ticks due at 11 and 12, chunk B's the same
        t.schedule(1, a, 11, Priority::Normal, 1);
        t.schedule(2, a.offset(1, 0, 0), 12, Priority::Normal, 2);
        t.schedule(3, b, 11, Priority::Normal, 3);
        t.schedule(4, b.offset(1, 0, 0), 12, Priority::Normal, 4);
        t.reload(&[(0, 0), (1, 0)], &[(0, 0), (1, 0)], 0);
        let subs: Vec<(u8, i64)> = t.all().iter().map(|x| (x.kind, x.sub)).collect();
        for (k, s) in [(1u8, -2i64), (2, -1), (3, -2), (4, -1)] {
            assert!(subs.contains(&(k, s)), "tick {k} numbered {s} after the load: {subs:?}");
        }
        // after the load, a fresh tick in a third chunk, due earliest of all (the level's counter: sub 7)
        t.schedule(5, c, 10, Priority::Normal, 7);
        // the idle: the first block tick is at 20, everything overdue
        let order: Vec<u8> = t.collect(20).iter().map(|x| x.kind).collect();
        assert_eq!(order, vec![1, 3, 4, 2, 5], "A-2, B-2, B-1 (the tie keeps draining B), A-1, then the fresh one");
    }
}
