//! The game's light engines, incremental (`LightEngine`, `BlockLightEngine`, `SkyLightEngine` of
//! 26.3; tickexact 2026-11-09): what the game runs between ticks for the cells whose light
//! properties changed (`LevelChunk.setBlockState` → `LevelLightEngine.checkBlock`), instead of
//! [`World::relight`]'s fixed point over a region.
//!
//! Each engine keeps the cells to check in a fastutil `LongOpenHashSet` (512 expected, load 0.5,
//! trimmed back after every run) and checks them in its walk order (`checkNode`), then runs its
//! decrease queue and its increase queue to the end (FIFO queues of a cell and a packed entry,
//! `LightEngine$QueueEntry`). The game's results depend on that order where light goes stale (a
//! decrease that does not reach a cell leaves its old light), so the order is the game's.
//!
//! The world's box is the stored light (`storingLightForSection`): a cell outside it is not
//! stored, so light neither leaves the box nor comes back in (the game's margins around a terrain
//! sample are stored there; a farm far from the box's sides never reaches them). Every section of
//! the box holds light data, so the sky engine's empty-section paths
//! (`countEmptySectionsBelowIfAtBorder`, `propagateFromEmptySections`) never run.

use super::land;
use super::light::{self, LightProps};
use crate::block::Block;
use crate::pos::BlockPos;
use crate::world::World;
use std::collections::VecDeque;

// LightEngine$QueueEntry: the level in the low 4 bits, one bit per direction to propagate in
// (4 + its ordinal: down, up, north, south, west, east), and two flags.
const LEVEL_MASK: u64 = 15;
const DIRECTIONS_MASK: u64 = 1008;
const FLAG_FROM_EMPTY_SHAPE: u64 = 1024;
const FLAG_INCREASE_FROM_EMISSION: u64 = 2048;
const UP: usize = 1;

const fn dir_bit(d: usize) -> u64 {
    1 << (d + 4)
}

const fn with_level(e: u64, level: u8) -> u64 {
    (e & !LEVEL_MASK) | (level as u64 & LEVEL_MASK)
}

const fn empty_flag(empty: bool) -> u64 {
    if empty { FLAG_FROM_EMPTY_SHAPE } else { 0 }
}

const fn decrease_all_directions(level: u8) -> u64 {
    with_level(DIRECTIONS_MASK, level)
}

const fn decrease_skip_one_direction(level: u8, d: usize) -> u64 {
    with_level(DIRECTIONS_MASK & !dir_bit(d), level)
}

const fn increase_light_from_emission(level: u8, empty: bool) -> u64 {
    with_level(DIRECTIONS_MASK | FLAG_INCREASE_FROM_EMISSION | empty_flag(empty), level)
}

const fn increase_skip_one_direction(level: u8, empty: bool, d: usize) -> u64 {
    with_level((DIRECTIONS_MASK & !dir_bit(d)) | empty_flag(empty), level)
}

const fn increase_only_one_direction(level: u8, empty: bool, d: usize) -> u64 {
    with_level(dir_bit(d) | empty_flag(empty), level)
}

fn from_level(e: u64) -> u8 {
    (e & LEVEL_MASK) as u8
}

fn propagates(e: u64, d: usize) -> bool {
    e & dir_bit(d) != 0
}

/// `LightEngine.PULL_LIGHT_IN_ENTRY`.
const PULL_LIGHT_IN: u64 = decrease_all_directions(1);
/// `SkyLightEngine`'s source entries.
const REMOVE_TOP_SKY_SOURCE: u64 = decrease_all_directions(15);
const REMOVE_SKY_SOURCE: u64 = decrease_skip_one_direction(15, UP);
const ADD_SKY_SOURCE: u64 = increase_skip_one_direction(15, false, UP);

/// `Direction.getOpposite` by ordinal.
fn opposite(d: usize) -> usize {
    d ^ 1
}

/// fastutil's `HashCommon.mix(long)`.
fn mix(x: i64) -> i64 {
    let h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15_u64 as i64);
    let h = h ^ ((h as u64) >> 32) as i64;
    h ^ ((h as u64) >> 16) as i64
}

/// `BlockPos.asLong` (26 / 12 / 26 bits).
fn as_long(p: BlockPos) -> i64 {
    ((p.x as i64 & 0x3FF_FFFF) << 38) | ((p.z as i64 & 0x3FF_FFFF) << 12) | (p.y as i64 & 0xFFF)
}

/// `BlockPos.of(long)`.
fn of_long(k: i64) -> BlockPos {
    BlockPos::new((k >> 38) as i32, ((k << 52) >> 52) as i32, ((k << 26) >> 38) as i32)
}

/// A fastutil `LongOpenHashSet(512, 0.5f)` as `LightEngine.blockNodesToCheck` uses it (adds, then
/// one walk, then `clear` and `trim(512)`: every run starts from 1024 slots): its walk order.
struct NodeSet {
    key: Vec<i64>,
    n: usize,
    max_fill: usize,
    size: usize,
    contains_null: bool,
}

fn array_size(expected: usize) -> usize {
    // HashCommon.arraySize(expected, 0.5f)
    (expected * 2).max(2).next_power_of_two()
}

impl NodeSet {
    fn new() -> NodeSet {
        let n = array_size(512);
        NodeSet { key: vec![0; n + 1], n, max_fill: n / 2, size: 0, contains_null: false }
    }

    fn slot(k: i64, mask: usize) -> usize {
        ((mix(k) as i32) as usize) & mask
    }

    fn add(&mut self, k: i64) {
        if k == 0 {
            if self.contains_null {
                return;
            }
            self.contains_null = true;
        } else {
            let mask = self.n - 1;
            let mut pos = Self::slot(k, mask);
            while self.key[pos] != 0 {
                if self.key[pos] == k {
                    return;
                }
                pos = (pos + 1) & mask;
            }
            self.key[pos] = k;
        }
        let old = self.size;
        self.size += 1;
        if old >= self.max_fill {
            self.rehash(array_size(self.size + 1));
        }
    }

    /// `rehash(newN)`: the keys from the top slot down, each into the new table.
    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut key = vec![0i64; new_n + 1];
        let real = if self.contains_null { self.size - 1 } else { self.size };
        let mut i = self.n;
        for _ in 0..real {
            i -= 1;
            while self.key[i] == 0 {
                i -= 1;
            }
            let mut pos = Self::slot(self.key[i], mask);
            while key[pos] != 0 {
                pos = (pos + 1) & mask;
            }
            key[pos] = self.key[i];
        }
        self.n = new_n;
        self.max_fill = new_n / 2;
        self.key = key;
    }

    /// The iterator's order: the null key first, then the slots from the top down.
    fn walk(&self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.size);
        if self.contains_null {
            out.push(0);
        }
        for i in (0..self.n).rev() {
            if self.key[i] != 0 {
                out.push(self.key[i]);
            }
        }
        out
    }
}

/// Which stored layer an engine works on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Block,
    Sky,
}

/// One engine's run over the world's box.
struct Engine {
    min: BlockPos,
    size: [i32; 3],
    /// Each direction's step in the box's arrays.
    stride: [isize; 6],
    decrease: VecDeque<(usize, u64)>,
    increase: VecDeque<(usize, u64)>,
    /// The light behaviour of the states met (natural states have their own table).
    cache: crate::fxhash::FxHashMap<Block, LightProps>,
}

impl Engine {
    fn new(w: &World) -> Engine {
        let size = w.size;
        let stride = light::DIRS_PUB.map(|(_, [dx, dy, dz])| dx as isize + (dz * size[0]) as isize + (dy * size[0] * size[2]) as isize);
        Engine { min: w.min, size, stride, decrease: VecDeque::new(), increase: VecDeque::new(), cache: Default::default() }
    }

    fn index(&self, p: BlockPos) -> Option<usize> {
        let (x, y, z) = (p.x - self.min.x, p.y - self.min.y, p.z - self.min.z);
        (x >= 0 && y >= 0 && z >= 0 && x < self.size[0] && y < self.size[1] && z < self.size[2]).then(|| ((y * self.size[2] + z) * self.size[0] + x) as usize)
    }

    fn pos_of(&self, i: usize) -> BlockPos {
        let i = i as i32;
        let x = i % self.size[0];
        let z = (i / self.size[0]) % self.size[2];
        let y = i / (self.size[0] * self.size[2]);
        BlockPos::new(self.min.x + x, self.min.y + y, self.min.z + z)
    }

    /// The neighbour of cell `i` (at `p`) in direction `d`, if it is stored (in the box).
    fn neighbour(&self, i: usize, p: BlockPos, d: usize) -> Option<usize> {
        let [dx, dy, dz] = light::DIRS_PUB[d].1;
        let (x, y, z) = (p.x - self.min.x + dx, p.y - self.min.y + dy, p.z - self.min.z + dz);
        (x >= 0 && y >= 0 && z >= 0 && x < self.size[0] && y < self.size[1] && z < self.size[2]).then(|| (i as isize + self.stride[d]) as usize)
    }

    fn props(&mut self, w: &World, i: usize) -> LightProps {
        match w.cells[i] {
            Block::Terrain(id) => land::terrain_lprops(id),
            Block::Air => LightProps::AIR,
            b => *self.cache.entry(b).or_insert_with(|| land::lprops(&b)),
        }
    }

    fn level(w: &World, layer: Layer, i: usize) -> u8 {
        let l = w.land.as_ref().unwrap();
        if layer == Layer::Sky { l.sky[i] } else { l.block[i] }
    }

    fn set_level(w: &mut World, layer: Layer, i: usize, v: u8) {
        let l = w.land.as_mut().unwrap();
        if layer == Layer::Sky {
            l.sky[i] = v;
        } else {
            l.block[i] = v;
        }
    }

    /// `LightEngine.isEmptyShape`: not occluding by shape (`!canOcclude || !useShapeForLightOcclusion`).
    fn empty_shape(p: &LightProps) -> bool {
        p.faces.is_none()
    }

    /// `BlockLightEngine.checkNode`.
    fn check_block_node(&mut self, w: &mut World, i: usize) {
        let state = self.props(w, i);
        let emission = state.emission.min(15);
        let level = Self::level(w, Layer::Block, i);
        if emission < level {
            Self::set_level(w, Layer::Block, i, 0);
            self.decrease.push_back((i, decrease_all_directions(level)));
        } else {
            self.decrease.push_back((i, PULL_LIGHT_IN));
        }
        if emission > 0 {
            self.increase.push_back((i, increase_light_from_emission(emission, Self::empty_shape(&state))));
        }
    }

    /// The lowest sky source of a column (`ChunkSkyLightSources.getLowestSourceY`), `default`
    /// outside the box.
    fn lowest_source(w: &World, x: i32, z: i32, default: i32) -> i32 {
        match land::Land::col(w.min, w.size, x, z) {
            Some(c) => w.land.as_ref().unwrap().sources[c],
            None => default,
        }
    }

    fn bottom_section(&self) -> i32 {
        self.min.y >> 4
    }

    fn top_section(&self) -> i32 {
        (self.min.y + self.size[1] - 1) >> 4
    }

    /// `SkyLightEngine.checkNode`.
    fn check_sky_node(&mut self, w: &mut World, i: usize) {
        let p = self.pos_of(i);
        let lowest = Self::lowest_source(w, p.x, p.z, i32::MAX);
        if lowest != i32::MAX {
            // updateSourcesInColumn
            let bottom = self.bottom_section() << 4;
            self.remove_sources_below(w, p.x, p.z, lowest, bottom);
            self.add_sources_above(w, p.x, p.z, lowest, bottom);
        }
        if p.y >= lowest {
            self.decrease.push_back((i, REMOVE_SKY_SOURCE));
            self.increase.push_back((i, ADD_SKY_SOURCE));
        } else {
            let level = Self::level(w, Layer::Sky, i);
            if level > 0 {
                Self::set_level(w, Layer::Sky, i, 0);
                self.decrease.push_back((i, decrease_all_directions(level)));
            } else {
                self.decrease.push_back((i, PULL_LIGHT_IN));
            }
        }
    }

    /// `SkyLightEngine.removeSourcesBelow`: the cells under the column's new lowest source that
    /// still hold a source's 15, from the top down, to 0 (each a decrease).
    fn remove_sources_below(&mut self, w: &mut World, x: i32, z: i32, lowest: i32, bottom: i32) {
        if lowest <= bottom {
            return;
        }
        let y0 = lowest - 1;
        let mut sy = y0 >> 4;
        while sy >= self.bottom_section() {
            if sy <= self.top_section() {
                let sb = sy << 4;
                let mut y = (sb + 15).min(y0);
                while y >= sb {
                    if let Some(i) = self.index(BlockPos::new(x, y, z)) {
                        if Self::level(w, Layer::Sky, i) != 15 {
                            return;
                        }
                        Self::set_level(w, Layer::Sky, i, 0);
                        self.decrease.push_back((i, if y == lowest - 1 { REMOVE_TOP_SKY_SOURCE } else { REMOVE_SKY_SOURCE }));
                    }
                    y -= 1;
                }
            }
            sy -= 1;
        }
    }

    /// `SkyLightEngine.addSourcesAbove`: the cells from the column's lowest source up that do not
    /// hold a source's 15 yet, to 15; an increase from those below every neighbour column's
    /// lowest source, and from the lowest.
    fn add_sources_above(&mut self, w: &mut World, x: i32, z: i32, lowest: i32, bottom: i32) {
        let low = |w: &World, x: i32, z: i32| Self::lowest_source(w, x, z, i32::MIN);
        let neighbours = low(w, x - 1, z).max(low(w, x + 1, z)).max(low(w, x, z - 1).max(low(w, x, z + 1)));
        let start = lowest.max(bottom);
        let mut sy = start >> 4;
        while sy <= self.top_section() {
            if sy >= self.bottom_section() {
                let sb = sy << 4;
                for y in sb.max(start)..=sb + 15 {
                    let Some(i) = self.index(BlockPos::new(x, y, z)) else { continue };
                    if Self::level(w, Layer::Sky, i) == 15 {
                        return;
                    }
                    Self::set_level(w, Layer::Sky, i, 15);
                    if y < neighbours || y == lowest {
                        self.increase.push_back((i, ADD_SKY_SOURCE));
                    }
                }
            }
            sy += 1;
        }
    }

    /// `LightEngine.propagateDecreases`: the decrease queue to its end.
    fn propagate_decreases(&mut self, w: &mut World, layer: Layer) {
        while let Some((i, e)) = self.decrease.pop_front() {
            self.propagate_decrease(w, layer, i, e);
        }
    }

    /// `BlockLightEngine.propagateDecrease` / `SkyLightEngine.propagateDecrease`.
    fn propagate_decrease(&mut self, w: &mut World, layer: Layer, i: usize, e: u64) {
        let p = self.pos_of(i);
        let from = from_level(e) as i32;
        for d in 0..6 {
            if !propagates(e, d) {
                continue;
            }
            let Some(j) = self.neighbour(i, p, d) else { continue };
            let to_level = Self::level(w, layer, j);
            if to_level == 0 {
                continue;
            }
            if to_level as i32 <= from - 1 {
                if layer == Layer::Block {
                    let to_state = self.props(w, j);
                    let emission = to_state.emission.min(15);
                    Self::set_level(w, layer, j, 0);
                    if emission < to_level {
                        self.decrease.push_back((j, decrease_skip_one_direction(to_level, opposite(d))));
                    }
                    if emission > 0 {
                        self.increase.push_back((j, increase_light_from_emission(emission, Self::empty_shape(&to_state))));
                    }
                } else {
                    Self::set_level(w, layer, j, 0);
                    self.decrease.push_back((j, decrease_skip_one_direction(to_level, opposite(d))));
                }
            } else {
                self.increase.push_back((j, increase_only_one_direction(to_level, false, opposite(d))));
            }
        }
    }

    /// `LightEngine.propagateIncreases`: the increase queue to its end; an emission's entry raises
    /// its cell first, and an entry whose level its cell no longer holds is dropped.
    fn propagate_increases(&mut self, w: &mut World, layer: Layer) {
        while let Some((i, e)) = self.increase.pop_front() {
            let mut level = Self::level(w, layer, i);
            let from = from_level(e);
            if e & FLAG_INCREASE_FROM_EMISSION != 0 && level < from {
                Self::set_level(w, layer, i, from);
                level = from;
            }
            if level == from {
                self.propagate_increase(w, layer, i, e, level);
            }
        }
    }

    /// `BlockLightEngine.propagateIncrease` / `SkyLightEngine.propagateIncrease`.
    fn propagate_increase(&mut self, w: &mut World, layer: Layer, i: usize, e: u64, level: u8) {
        let p = self.pos_of(i);
        let mut from_state: Option<LightProps> = None;
        for d in 0..6 {
            if !propagates(e, d) {
                continue;
            }
            let Some(j) = self.neighbour(i, p, d) else { continue };
            let to_level = Self::level(w, layer, j);
            if level as i32 - 1 <= to_level as i32 {
                continue;
            }
            let to_state = self.props(w, j);
            let target = level as i32 - to_state.dampening.max(1) as i32;
            if target <= to_level as i32 {
                continue;
            }
            let fs = match from_state {
                Some(s) => s,
                None => {
                    let s = if e & FLAG_FROM_EMPTY_SHAPE != 0 { LightProps::AIR } else { self.props(w, i) };
                    from_state = Some(s);
                    s
                }
            };
            if light::shape_occludes_pub(&fs, &to_state, d) {
                continue;
            }
            Self::set_level(w, layer, j, target as u8);
            if target > 1 {
                self.increase.push_back((j, increase_skip_one_direction(target as u8, Self::empty_shape(&to_state), opposite(d))));
            }
        }
    }
}

/// Whether worlds settle their light with these engines (explore: `MCTAS_LIGHT_ENGINE=1`) instead
/// of [`World::relight`]'s fixed point.
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("MCTAS_LIGHT_ENGINE").is_ok_and(|v| v == "1"))
}

/// `LevelLightEngine.runLightUpdates` over the cells changed since the last run (`Land::dirty`):
/// the block engine, then the sky engine, each checking the changed cells in its set's walk order.
pub fn run(w: &mut World) {
    let Some(l) = w.land.as_ref() else { return };
    if l.dirty.is_empty() {
        return;
    }
    let mut set = NodeSet::new();
    for &(p, _, _) in &l.dirty {
        set.add(as_long(p));
    }
    let mut e = Engine::new(w);
    let nodes: Vec<usize> = set.walk().into_iter().filter_map(|k| e.index(of_long(k))).collect();
    for &i in &nodes {
        e.check_block_node(w, i);
    }
    e.propagate_decreases(w, Layer::Block);
    e.propagate_increases(w, Layer::Block);
    for &i in &nodes {
        e.check_sky_node(w, i);
    }
    e.propagate_decreases(w, Layer::Sky);
    e.propagate_increases(w, Layer::Sky);
    if let Some(l) = w.land.as_mut() {
        l.dirty.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_long_round_trips() {
        for p in [BlockPos::new(0, 0, 0), BlockPos::new(-432, -64, 309), BlockPos::new(1, 319, -1), BlockPos::new(-30_000_000, -2048, 29_999_999)] {
            assert_eq!(of_long(as_long(p)), p);
        }
    }

    #[test]
    fn entries_match_the_game() {
        // QueueEntry.decreaseAllDirections(1) = 1008 | 1, decreaseSkipOneDirection(15, UP) clears
        // bit 5, increaseSkipOneDirection(15, false, UP) the same
        assert_eq!(PULL_LIGHT_IN, 1009);
        assert_eq!(REMOVE_SKY_SOURCE, 1008 & !32 | 15);
        assert_eq!(ADD_SKY_SOURCE, 1008 & !32 | 15);
        assert_eq!(increase_light_from_emission(14, true), 1008 | 2048 | 1024 | 14);
        assert_eq!(increase_only_one_direction(7, false, 4), 256 | 7);
    }

    #[test]
    fn the_set_walks_its_slots_from_the_top() {
        let mut s = NodeSet::new();
        let keys: Vec<i64> = (1..600).map(|k| as_long(BlockPos::new(k, 64, -k))).collect();
        for &k in &keys {
            s.add(k);
        }
        // 600 keys grew the table once (past 512)
        assert_eq!(s.n, 2048);
        let walked = s.walk();
        assert_eq!(walked.len(), keys.len());
        // each key's slot, walked from the top
        let slots: Vec<usize> = walked.iter().map(|&k| s.key.iter().position(|&x| x == k).unwrap()).collect();
        assert!(slots.windows(2).all(|w| w[0] > w[1]));
    }
}
