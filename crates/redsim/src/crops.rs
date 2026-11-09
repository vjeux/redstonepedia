//! Crop farms (webrender, 2026-10-21: the 3D view of w86's harvesters, study 18): sugar cane
//! (`SugarCaneBlock`, 26.3) and the random ticks that grow it.
//!
//! The random ticks, exact (clk-misc 2026-10-21; `redsim-fuzz trace-compare` checks the level
//! random and `Level.randValue` every frame): `ServerChunkCache.tickChunks` runs
//! `ServerLevel.tickChunk(chunk, randomTickSpeed)` for every entity-ticking chunk in the order of
//! the simulation tracker's hash map ([`tick_order`]). A chunk tick first draws `randomTickSpeed`
//! `nextInt(48)` from the level random (precipitation: each 0 picks a cell with
//! `getBlockRandomPos`, which steps `randValue`; the pick itself does nothing in a sandbox, no
//! rain and a temperate biome), then gives every randomly ticking section (lowest first, checked
//! when its turn comes) `randomTickSpeed` cells from `getBlockRandomPos`, each randomly ticking
//! block there getting its `randomTick`. The game seeds `randValue` from an unseeded random; the
//! bridge seeds it from the sandbox seed ([`bridge_rand_value`]), so a sandbox's growth repeats and
//! redsim's mirror starts where the game does.

use crate::block::{Block, Kind, Simple};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

/// The random ticks of a world (`gamerule random_tick_speed`, vanilla 3) and its position
/// generator (`Level.randValue`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RandomTicks {
    pub speed: u32,
    pub value: i32,
}

/// `Level.randValue` of a bridge sandbox seeded `seed` (`Sandbox.randValueOf`, at create and reset).
pub fn bridge_rand_value(seed: i64) -> i32 {
    (seed ^ 0x72616E64) as i32
}

impl RandomTicks {
    /// Random ticks at `speed`, the generator where a bridge sandbox seeded `seed` starts it.
    pub fn new(speed: u32, seed: i64) -> RandomTicks {
        RandomTicks { speed, value: bridge_rand_value(seed) }
    }

    /// `Level.getBlockRandomPos(xo, yo, zo, 15)`.
    pub fn position(&mut self, xo: i32, yo: i32, zo: i32) -> BlockPos {
        self.value = self.value.wrapping_mul(3).wrapping_add(1013904223);
        let val = self.value >> 2;
        BlockPos::new(xo + (val & 15), yo + ((val >> 16) & 15), zo + ((val >> 8) & 15))
    }
}

/// `BlockState.isRandomlyTicking` among redsim's blocks: sugar cane (always), a growing bamboo
/// stalk (stage 0), a bamboo shoot, kelp's top under age 25, and the ones whose random tick is
/// not modeled (a section holding one still takes its share of positions): grass (spreading,
/// decaying), leaves far from a log (decaying). Lava, the one randomly ticking fluid (its block
/// is too: `LiquidBlock.isRandomlyTicking`), lights fires (`weather::fire::lava_random_tick`).
pub fn randomly_ticking(b: &Block) -> bool {
    match *b {
        Block::SugarCane { .. } => true,
        Block::Bamboo { stage, .. } => stage == 0,
        Block::BambooSapling => true,
        Block::Kelp { age } => age < 25,
        Block::Simple(Simple::GrassBlock) => true,
        Block::Leaves { distance, persistent } => distance == 7 && !persistent,
        // ---- stem farms (clk-misc): stems and farmland tick at random (attached stems do not) ----
        Block::Stem { .. } | Block::Farmland { .. } => true,
        // ---- cactus farms (clk-misc) ----
        Block::Cactus { .. } => true,
        // ---- mob farms (mobinfra): lava, the randomly ticking fluid (its fire, `weather::fire`) ----
        Block::Lava { .. } => true,
        // ---- weathering copper (mobinfra): an unwaxed bulb or rod with a stage left ----
        Block::Bulb { .. } | Block::LightningRod { .. } => crate::copper::randomly_ticking(b),
        _ => false,
    }
}

/// The lowest and highest chunk sections of the game's sandboxes (y -64..319).
const MIN_SECTION: i32 = -4;
const MAX_SECTION: i32 = 19;

/// Whether a cell counts toward its section's random ticking (`state.isRandomlyTicking()` or its
/// fluid state's): redsim's randomly ticking blocks, and the natural states the terrain lane's table
/// marks (grass, leaves, plants...: their section ticks and draws its positions even where redsim
/// models no effect of their tick).
pub fn ticks_in_section(b: &Block) -> bool {
    match b {
        Block::Terrain(id) => crate::terrain::blocks::props(*id).random_ticking,
        _ => randomly_ticking(b),
    }
}

/// Every chunk section's randomly ticking cells, over redsim's box (`LevelChunkSection`'s
/// counts: recalculated when built, adjusted by every write), and which cells they are (a bit per
/// cell: tickexact 2026-11-08, a random tick's position is tested in these 512 bytes a section
/// instead of the box's cells, most of them stone or air, which missed the cache: a sixth of a
/// terrain run).
#[derive(Clone, Debug)]
pub struct TickingSections {
    lo: [i32; 3],
    dims: [i32; 3],
    counts: Vec<u32>,
    /// 64 words a section: bit `(y * 16 + z) * 16 + x` of the section's cell (x, y, z).
    bits: Vec<u64>,
    /// Per chunk column (`z * dims[0] + x`): bit `sy - lo[1]` set when that section has a
    /// randomly ticking cell (a chunk's ticking sections without testing all 24: tickexact
    /// 2026-11-08).
    cols: Vec<u32>,
}

impl TickingSections {
    /// The counts of a world's cells (`LevelChunkSection.recalcBlockCounts`).
    pub fn build(w: &World) -> TickingSections {
        let (min, size) = (w.min, w.size);
        let lo = [min.x.div_euclid(16), min.y.div_euclid(16), min.z.div_euclid(16)];
        let hi = [(min.x + size[0] - 1).div_euclid(16), (min.y + size[1] - 1).div_euclid(16), (min.z + size[2] - 1).div_euclid(16)];
        let dims = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        let n = (dims[0] * dims[1] * dims[2]) as usize;
        let mut ts = TickingSections { lo, dims, counts: vec![0; n], bits: vec![0; n * 64], cols: vec![0; (dims[0] * dims[2]) as usize] };
        // the box's cells in their storage order (x fastest, then z, then y), a terrain cell's
        // answer from one flag per state id (tickexact 2026-11-09: the viewer's still spent 9% of
        // its time here, a bounds-checked lookup and a 100-byte table row read per cell)
        let cells: &[Block] = &w.cells;
        let rt = crate::terrain::blocks::random_ticking_ids();
        let mut i = 0usize;
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let b = &cells[i];
                    i += 1;
                    let ticks = match b {
                        Block::Air => false,
                        Block::Terrain(id) => rt.get(*id as usize).copied().unwrap_or(false),
                        other => randomly_ticking(other),
                    };
                    if ticks {
                        ts.adjust(BlockPos::new(min.x + x, min.y + y, min.z + z), true);
                    }
                }
            }
        }
        ts
    }

    /// The lowest section at or above `sy` of chunk column (`cx`, `cz`) with a randomly ticking
    /// cell (as testing [`TickingSections::ticking`] upward from `sy` finds it).
    pub fn next_ticking(&self, cx: i32, cz: i32, sy: i32) -> Option<i32> {
        let (x, z) = (cx - self.lo[0], cz - self.lo[2]);
        if x < 0 || z < 0 || x >= self.dims[0] || z >= self.dims[2] || self.dims[1] > 32 {
            return None;
        }
        let from = (sy - self.lo[1]).max(0);
        if from >= self.dims[1] {
            return None;
        }
        let m = self.cols[(z * self.dims[0] + x) as usize] >> from;
        if m == 0 { None } else { Some(self.lo[1] + from + m.trailing_zeros() as i32) }
    }

    /// The word and bit of cell `p` (its section's index `i`).
    fn bit(i: usize, p: BlockPos) -> (usize, u64) {
        let local = ((p.y.rem_euclid(16) * 16 + p.z.rem_euclid(16)) * 16 + p.x.rem_euclid(16)) as usize;
        (i * 64 + (local >> 6), 1u64 << (local & 63))
    }

    /// Whether cell `p` ticks randomly (`ticks_in_section` of its block), from the bits.
    pub fn cell(&self, p: BlockPos) -> bool {
        match self.index(p.x.div_euclid(16), p.y.div_euclid(16), p.z.div_euclid(16)) {
            Some(i) => {
                let (word, mask) = TickingSections::bit(i, p);
                self.bits[word] & mask != 0
            }
            None => false,
        }
    }

    fn index(&self, cx: i32, sy: i32, cz: i32) -> Option<usize> {
        let (x, y, z) = (cx - self.lo[0], sy - self.lo[1], cz - self.lo[2]);
        if x < 0 || y < 0 || z < 0 || x >= self.dims[0] || y >= self.dims[1] || z >= self.dims[2] {
            return None;
        }
        Some(((y * self.dims[2] + z) * self.dims[0] + x) as usize)
    }

    /// `LevelChunkSection.isRandomlyTicking`.
    pub fn ticking(&self, cx: i32, sy: i32, cz: i32) -> bool {
        self.index(cx, sy, cz).is_some_and(|i| self.counts[i] > 0)
    }

    /// One cell of block `p` starts (`up`) or stops ticking.
    pub fn adjust(&mut self, p: BlockPos, up: bool) {
        if let Some(i) = self.index(p.x.div_euclid(16), p.y.div_euclid(16), p.z.div_euclid(16)) {
            let (word, mask) = TickingSections::bit(i, p);
            if up {
                self.counts[i] += 1;
                self.bits[word] |= mask;
            } else {
                self.counts[i] = self.counts[i].saturating_sub(1);
                self.bits[word] &= !mask;
            }
            // the column's mask follows the section's count
            let (x, sy, z) = (p.x.div_euclid(16) - self.lo[0], p.y.div_euclid(16) - self.lo[1], p.z.div_euclid(16) - self.lo[2]);
            if sy < 32 {
                let c = &mut self.cols[(z * self.dims[0] + x) as usize];
                if self.counts[i] > 0 {
                    *c |= 1 << sy;
                } else {
                    *c &= !(1 << sy);
                }
            }
        }
    }
}

/// `ServerChunkCache.tickChunks`' chunk part, called by `World::tick` after the block events'
/// predecessors (raids): `ServerLevel.tickChunk` of every chunk in `World::chunk_order` (without a
/// mirrored sandbox, the chunks of redsim's box, by x then z, and no precipitation draws).
pub fn tick_chunks(w: &mut World) {
    let speed = w.random_ticks.as_ref().map_or(w.tick_speed, |r| r.speed);
    let order: Vec<[i32; 2]> = if !w.chunk_order.is_empty() {
        w.chunk_order.clone()
    } else if w.random_ticks.is_some() {
        let (lo, hi) = (w.min, w.min.offset(w.size[0] - 1, w.size[1] - 1, w.size[2] - 1));
        (lo.x.div_euclid(16)..=hi.x.div_euclid(16)).flat_map(|x| (lo.z.div_euclid(16)..=hi.z.div_euclid(16)).map(move |z| [x, z])).collect()
    } else {
        return;
    };
    let sections = (w.min.y.div_euclid(16).max(MIN_SECTION), (w.min.y + w.size[1] - 1).div_euclid(16).min(MAX_SECTION));
    if w.random_ticks.is_some() && w.ticking_sections.is_none() {
        w.ticking_sections = Some(TickingSections::build(w));
    }
    for [cx, cz] in order {
        // iceandsnow: randomTickSpeed x nextInt(48); a 0 picks a cell for tickPrecipitation
        // (getBlockRandomPos: randValue steps), which changes nothing in a sandbox
        if w.level_random.is_some() {
            for _ in 0..speed {
                let hit = w.level_random.as_mut().is_some_and(|r| r.next_int(48) == 0);
                if hit && let Some(rt) = w.random_ticks.as_mut() {
                    rt.position(cx * 16, 0, cz * 16);
                }
            }
        }
        // tickBlocks
        if speed == 0 || w.random_ticks.is_none() {
            continue;
        }
        // (the column's ticking sections from its mask, read afresh after each section: a random
        // tick may start or stop a section above it ticking, as testing each one in turn sees)
        let mut next = sections.0;
        while next <= sections.1 {
            let Some(sy) = w.ticking_sections.as_ref().and_then(|ts| ts.next_ticking(cx, cz, next)) else { break };
            if sy > sections.1 {
                break;
            }
            next = sy + 1;
            for _ in 0..speed {
                let Some(rt) = w.random_ticks.as_mut() else { return };
                let p = rt.position(cx * 16, sy * 16, cz * 16);
                // isRandomlyTicking (its block's or its fluid's): natural states by the table; the
                // section's bits say it without reading the box's cell (the same answer:
                // `ticks_in_section` of the cell, kept by every write)
                if w.ticking_sections.as_ref().is_some_and(|ts| ts.cell(p)) {
                    let s = w.get(p);
                    random_tick(w, s, p);
                }
            }
        }
    }
}

/// `ChunkPos.pack`.
fn pack(x: i32, z: i32) -> i64 {
    (x as u32 as i64) | ((z as u32 as i64) << 32)
}

/// fastutil's `HashCommon.mix`.
fn mix(x: i64) -> i64 {
    let h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15_u64 as i64);
    let h = h ^ ((h as u64) >> 32) as i64;
    h ^ ((h as u64) >> 16) as i64
}

/// The keys of a fastutil `Long2ByteOpenHashMap` built with the default size and load factor (16,
/// 0.75): linear probing from `(int) mix(k) & mask`, growth by `rehash(arraySize(size + 1))`
/// (the old slots re-inserted from the top), the null key (0) aside; iterated as its fast entry
/// iterator does (the null key first, then the slots from the top down).
struct OpenMap {
    key: Vec<i64>,
    size: usize,
    max_fill: usize,
    null: bool,
}

impl OpenMap {
    fn new() -> OpenMap {
        OpenMap { key: vec![0; 32], size: 0, max_fill: 24, null: false }
    }

    fn slot(k: i64, mask: usize) -> usize {
        ((mix(k) as i32) & (mask as i32)) as usize
    }

    fn place(key: &mut [i64], k: i64) {
        let mask = key.len() - 1;
        let mut pos = OpenMap::slot(k, mask);
        while key[pos] != 0 {
            pos = (pos + 1) & mask;
        }
        key[pos] = k;
    }

    fn put(&mut self, k: i64) {
        if k == 0 {
            if self.null {
                return;
            }
            self.null = true;
        } else {
            let mask = self.key.len() - 1;
            let mut pos = OpenMap::slot(k, mask);
            while self.key[pos] != 0 {
                if self.key[pos] == k {
                    return;
                }
                pos = (pos + 1) & mask;
            }
            self.key[pos] = k;
        }
        let before = self.size;
        self.size += 1;
        if before >= self.max_fill {
            // arraySize(size + 1, 0.75): the next power of two of ceil((size + 1) / 0.75f)
            let n = (((self.size + 1) as f32 / 0.75f32).ceil() as usize).next_power_of_two().max(2);
            let mut fresh = vec![0i64; n];
            for &k in self.key.iter().rev().filter(|k| **k != 0) {
                OpenMap::place(&mut fresh, k);
            }
            self.key = fresh;
            self.max_fill = ((n as f32 * 0.75f32).ceil() as usize).min(n - 1);
        }
    }

    fn iter(&self) -> Vec<i64> {
        let mut out = Vec::new();
        if self.null {
            out.push(0);
        }
        out.extend(self.key.iter().rev().filter(|k| **k != 0));
        out
    }
}

/// The order `ServerChunkCache.tickChunks` ticks a sandbox's chunks in (`forEachBlockTickingChunk`:
/// the entity-ticking chunks, iterating `SimulationChunkTracker.chunks`, a `Long2ByteOpenHashMap`
/// of every chunk below level 33). The bridge queues every forced ticket before the tracker runs
/// them (`Sandbox.create`), so the map takes the forced chunks (level 31) in ticket order (x
/// outer, z inner), then the chunks around them (level 32) in the order those forced chunks'
/// neighbour checks first queued them (x offset outer, z inner); the forced ones, in the map's
/// iteration order, are the ticking chunks.
pub fn tick_order(chunks: [i32; 4]) -> Vec<[i32; 2]> {
    let [x0, z0, x1, z1] = chunks;
    let forced: Vec<(i32, i32)> = (x0..=x1).flat_map(|x| (z0..=z1).map(move |z| (x, z))).collect();
    let mut inserted: Vec<(i32, i32)> = forced.clone();
    for &(x, z) in &forced {
        for dx in -1..=1 {
            for dz in -1..=1 {
                let n = (x + dx, z + dz);
                if !inserted.contains(&n) {
                    inserted.push(n);
                }
            }
        }
    }
    let mut map = OpenMap::new();
    for &(x, z) in &inserted {
        map.put(pack(x, z));
    }
    map.iter().into_iter().map(|k| (k as i32, (k >> 32) as i32)).filter(|c| forced.contains(c)).map(|(x, z)| [x, z]).collect()
}

/// `randomTick` (the blocks [`randomly_ticking`] names).
pub fn random_tick(w: &mut World, s: Block, p: BlockPos) {
    match s {
        Block::Bamboo { .. } | Block::BambooSapling => return crate::bamboo::random_tick(w, s, p),
        Block::Kelp { .. } => return crate::kelp::random_tick(w, s, p),
        Block::Stem { .. } | Block::Farmland { .. } => return crate::stems::random_tick(w, s, p),
        Block::Cactus { .. } => return crate::cactus::random_tick(w, s, p),
        // lava: `LiquidBlock.randomTick` (its fluid's), then the fluid state's own: twice
        Block::Lava { .. } => {
            crate::weather::fire::lava_random_tick(w, p);
            crate::weather::fire::lava_random_tick(w, p);
            return;
        }
        // ---- terrain lane: the natural states' own random ticks (grass, crops, amethyst...) ----
        Block::Terrain(id) => return crate::terrain::ticks::random_tick(w, id, p),
        Block::Simple(crate::block::Simple::GrassBlock) => return crate::terrain::ticks::grass_tick(w, p),
        // weathering copper: `ChangeOverTimeBlock.changeOverTime`
        Block::Bulb { .. } | Block::LightningRod { .. } => return crate::copper::change_over_time(w, s, p),
        _ => {}
    }
    if !matches!(s, Block::SugarCane { .. }) {
        let why = format!("the random tick of {} is not modeled (random ticks on: a grass block spreads or dies, far leaves decay)", s);
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
        return;
    }
    if let Block::SugarCane { age } = s {
        // SugarCaneBlock.randomTick: under air, a cane under 3 blocks tall ages; at 15 it grows the
        // block above (setBlockAndUpdate) and starts again at 0 (flags 260: no neighbour updates,
        // its shape updates still reach an observer)
        if !w.get(p.above()).is_air() {
            return;
        }
        let mut height = 1;
        while matches!(w.get(p.offset(0, -height, 0)), Block::SugarCane { .. }) {
            height += 1;
        }
        if height < 3 {
            if age == 15 {
                w.set_block(p.above(), Block::SugarCane { age: 0 }, flags::ALL);
                w.set_block(p, Block::SugarCane { age: 0 }, flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
            } else {
                w.set_block(p, Block::SugarCane { age: age + 1 }, flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
            }
        }
    }
}

/// `SugarCaneBlock.canSurvive`: on sugar cane, or on a block of #supports_sugar_cane beside a block
/// whose fluid is water (#supports_sugar_cane_adjacently: water, flowing water, a waterlogged
/// block; frosted ice is not modeled).
pub fn can_survive(w: &World, p: BlockPos) -> bool {
    let below = w.get(p.below());
    if matches!(below, Block::SugarCane { .. }) {
        return true;
    }
    if !supports_cane(&below) {
        return false;
    }
    let b = p.below();
    Dir::HORIZONTAL.iter().any(|d| crate::fluid::fluid_state(w, b.rel(*d)).fluid.is_water())
}

/// #supports_sugar_cane (#substrate_overworld: #dirt, #mud, #moss_blocks, #grass_blocks; #sand):
/// the ones redsim builds.
fn supports_cane(b: &Block) -> bool {
    match b {
        Block::Simple(s) => matches!(s, Simple::Dirt | Simple::GrassBlock | Simple::Mud),
        Block::Falling { id } => matches!(crate::block::FALLING[*id as usize], "sand" | "red_sand"),
        _ => false,
    }
}

/// `SugarCaneBlock.updateShape`: a cane that can no longer stand breaks a tick later ([`tick`]).
pub fn update_shape(w: &mut World, s: Block, p: BlockPos) -> Block {
    if !can_survive(w, p) {
        w.schedule_tick(p, Kind::SugarCane, 1, Priority::Normal);
    }
    s
}

/// `SugarCaneBlock.tick`: `destroyBlock(pos, true)` when it can no longer stand (the item drops).
pub fn tick(w: &mut World, p: BlockPos) {
    if !can_survive(w, p) {
        w.destroy_block(p, true, crate::world::UPDATE_LIMIT);
    }
}
