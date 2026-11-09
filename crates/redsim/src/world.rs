//! The level: block storage, `Level.setBlock` with the game's update flags, the
//! `CollectingNeighborUpdater` (every neighbor and shape update goes through it, depth first in
//! layers), the signal queries (`SignalGetter`), scheduled ticks and block events.

use crate::behavior as bh;
use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::ticks::{LevelTicks, Priority};
use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};

/// `Block.UPDATE_*` flags.
pub mod flags {
    pub const NEIGHBORS: u32 = 1;
    pub const CLIENTS: u32 = 2;
    pub const INVISIBLE: u32 = 4;
    pub const IMMEDIATE: u32 = 8;
    pub const KNOWN_SHAPE: u32 = 16;
    pub const SUPPRESS_DROPS: u32 = 32;
    pub const MOVE_BY_PISTON: u32 = 64;
    pub const SKIP_SHAPE_UPDATE_ON_WIRE: u32 = 128;
    pub const SKIP_BLOCK_ENTITY_SIDEEFFECTS: u32 = 256;
    pub const SKIP_ON_PLACE: u32 = 512;
    pub const ALL: u32 = 3;
    /// The bridge's "raw" placement (`Sandbox.RAW_FLAGS`).
    pub const RAW: u32 = CLIENTS | KNOWN_SHAPE | SUPPRESS_DROPS | SKIP_BLOCK_ENTITY_SIDEEFFECTS | SKIP_ON_PLACE;
}

/// `Block.UPDATE_LIMIT`.
pub const UPDATE_LIMIT: i32 = 512;
/// Game rule `maxChainedNeighborUpdates` default.
pub const MAX_CHAINED_NEIGHBOR_UPDATES: u32 = 1_000_000;

/// One pending entry of the neighbor updater (`CollectingNeighborUpdater.NeighborUpdates`).
#[derive(Clone, Copy, Debug)]
enum Nu {
    Full { state: Block, pos: BlockPos, source: Kind, moved: bool },
    Multi { pos: BlockPos, source: Kind, skip: Option<Dir>, idx: u8 },
    Shape { dir: Dir, neighbor_state: Block, pos: BlockPos, neighbor_pos: BlockPos, flags: u32, limit: i32 },
    Simple { pos: BlockPos, source: Kind },
}

/// `BlockEventData`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockEvent {
    pub pos: BlockPos,
    pub kind: Kind,
    pub a: i32,
    pub b: i32,
}

/// The block-event queue: an insertion-ordered set (`ObjectLinkedOpenHashSet`), identical
/// pending events are dropped.
#[derive(Default, Clone)]
pub struct BlockEvents {
    queue: VecDeque<BlockEvent>,
    set: HashSet<BlockEvent>,
}
impl BlockEvents {
    pub fn add(&mut self, e: BlockEvent) {
        if self.set.insert(e) {
            self.queue.push_back(e);
        }
    }
    pub fn pop(&mut self) -> Option<BlockEvent> {
        let e = self.queue.pop_front()?;
        self.set.remove(&e);
        Some(e)
    }
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &BlockEvent> {
        self.queue.iter()
    }
}

/// The simulated level: a box of cells (outside: air, writes dropped and reported).
#[derive(Clone)]
pub struct World {
    pub min: BlockPos,
    pub size: [i32; 3],
    pub(crate) cells: Vec<Block>,
    /// Game time (`getGameTime`), incremented at the start of each tick.
    pub time: i64,
    sub_tick: i64,
    pub ticks: LevelTicks,
    /// Scheduled fluid ticks (`ServerLevel.fluidTicks`), keyed by the fluid type; run after the
    /// block ticks (`fluid/`).
    pub fluid_ticks: LevelTicks<crate::fluid::Fluid>,
    pub events: BlockEvents,
    nu_stack: Vec<Nu>,
    nu_added: Vec<Nu>,
    nu_count: u32,
    /// `ServerLevel.handlingTick`: true from the tick's start through the block-event phase.
    pub handling_tick: bool,
    /// Comparator block entities: their output signal.
    pub comparator_out: HashMap<BlockPos, u8>,
    /// Container block entities (droppers, dispensers, hoppers): contents, hopper cooldowns.
    pub containers: crate::redstone::container::Containers,
    /// `RedstoneTorchBlock.RECENT_TOGGLES` of this level.
    pub torch_toggles: Vec<(BlockPos, i64)>,
    /// `RedstoneWireBlock.shouldSignal`.
    pub wire_signal: Cell<bool>,
    /// Piston block entities (moving blocks) and their tickers (agent pistmodel's side).
    pub piston: crate::piston::State,
    /// Falling-block entities (agent pistmodel's side).
    pub gravity: crate::gravity::State,
    /// Boats, minecarts and item entities, in insertion order (`entity/`; empty for designs
    /// without entities, so nothing changes for them).
    pub entities: crate::entity::Entities,
    /// ironfarm: the mob layer's level state (`entity::mob::MobWorld`: its seeded random, day time,
    /// game rules, record).
    pub mob: crate::entity::mob::MobWorld,
    /// Item frames (`entity::frame`): comparator inputs, empty for designs without them.
    pub frames: Vec<crate::entity::frame::Frame>,
    /// Blocks destroyed with drops (an item entity in the game): (where, what).
    pub drops: Vec<(BlockPos, Block)>,
    /// Game events (vibrations), in order: (name, where). Sculk sensors hear them (`sculk.rs`).
    pub game_events: Vec<(&'static str, BlockPos)>,
    /// Sculk sensor block entities and their tickers (agent sculk's side).
    pub sculk: crate::sculk::State,
    /// Daylight detectors exist (`daylight.rs`): block changes are logged in `light_changes`
    /// (tick, where) for the light engine's timing, and the state key holds the time mod 20.
    pub watch_light: bool,
    pub light_changes: Vec<(i64, BlockPos)>,
    /// With `log_writes`: every cell whose state a write changed, in order, until the caller takes
    /// them (`writes`). The viewer's big TNT-ground worlds look again only at what changed after a
    /// tick, not at the whole box. Off by default; no effect on the simulation or the state key.
    pub log_writes: bool,
    pub writes: Vec<BlockPos>,
    /// Problems the model met (a write outside the box, an unmodeled case): a run with any is
    /// not trustworthy.
    pub errors: Vec<String>,
    /// TNT primed (`TntBlock.prime`: a `PrimedTnt` in the game, not simulated: its motion draws
    /// from `level.random` and the judge removes primed TNT before it explodes): (game time, the
    /// TNT's cell; the entity spawns at its bottom center).
    pub primed: Vec<(i64, BlockPos)>,
    /// The caller removes every primed TNT before it explodes (the TNT judge): no explosion
    /// error (`tick`).
    pub remove_primed: bool,
    /// Primed TNT become entities (`entity::primed_tnt`) with this value in place of the game's
    /// random `nextDouble()` for their kick; `None`: primes are records only (the judge's screen).
    pub tnt_kick: Option<f64>,
    // ---- shulker study (clk-rt) ----
    /// Item components (`Stack::payload` - 1 indexes it): a shulker box's 27 slots, an item's
    /// custom name.
    pub payloads: Vec<crate::redstone::container::Payload>,
    /// The sandbox seed the bridge draws piston-broken drops from (`rng::Drop`): with one, a
    /// destroyed shulker box drops as an item entity (`drop_count` numbers them); without, its
    /// drop is only recorded in `drops`.
    pub drop_seed: Option<i64>,
    pub drop_count: u64,
    /// Items the sandbox's dispensers shot into the world so far (`Sandbox.dispenseIndex`).
    pub dispense_count: u64,
    /// Riders moved off a removed vehicle since the last entity phase (`entity::stop_riding`).
    pub dismounted: Vec<u32>,
    /// The salt of the bridge sandbox's loot random sequences (`stems::loot_salt` of its seed): a
    /// melon's slices draw from its table's sequence (`stems::melon_slices`); without, a broken
    /// melon is a model error.
    pub loot_salt: Option<i32>,
    /// The loot random sequences drawn so far, by table id (`RandomSequences`).
    pub loot_sequences: std::collections::BTreeMap<String, crate::stems::LootSequence>,
    // ---- the level random (clk-misc) ----
    /// `Level.random`, mirrored (`mirror_level_random`): the sandbox's LegacyRandomSource, seeded
    /// like the bridge's and drawn at every game spot redsim models (chunk ticks, piston sounds,
    /// note blocks, door sounds, dispenser slots, entity containers, drops...). `None` (the
    /// default): nothing is drawn and anything that would choose by it stays a model error.
    /// Never part of the state key.
    pub level_random: Option<crate::rng::LegacyRandom>,
    /// (tickexact) The bridge sandbox's seed when the world mirrors one (set with the level
    /// random): every entity's stream comes from it ([`World::construct`],
    /// [`World::entering_random`]).
    pub exact_seed: Option<i64>,
    /// (tickexact) Entities constructed so far (`EntityType.create`: spawn attempts that were then
    /// rejected included): the next [`World::construct`]'s k.
    pub constructions: u64,
    /// Random ticks of blocks (`crops::RandomTicks`: `gamerule random_tick_speed` and
    /// `Level.randValue`, the position generator, seeded like the bridge's sandboxes:
    /// [`crate::crops::bridge_rand_value`]). `None` (the default): no block random ticks (the
    /// chunk ticks still draw the level random's precipitation numbers at `tick_speed`). Never
    /// part of the state key.
    pub random_ticks: Option<crate::crops::RandomTicks>,
    /// The sandbox's `randomTickSpeed` when `random_ticks` is off (3; `RandomTicks::speed` rules
    /// when on): each chunk tick draws that many `nextInt(48)` from the level random.
    pub tick_speed: u32,
    /// The chunks `ServerChunkCache.tickChunks` hands to `ServerLevel.tickChunk`, in the game's
    /// order ([`crate::crops::tick_order`] of the sandbox's forced box; set with the level random).
    pub chunk_order: Vec<[i32; 2]>,
    /// The sandbox's forced chunks [x0, z0, x1, z1] (set with the level random): entities only
    /// tick inside them ([`World::entity_ticks_at`]). None: every entity ticks.
    pub entity_chunks: Option<[i32; 4]>,
    /// Entities that stopped ticking outside `entity_chunks` (game time, entity id), once each: a
    /// sandbox artifact (in a real world the player keeps those chunks loaded), which a judge
    /// fails or avoids by forcing more chunks.
    pub frozen: Vec<(i64, u32)>,
    // ---- explosions and TNT outcomes (clk-misc, `explosion.rs`) ----
    /// How an explosion draws its rays' power (`explosion::Rolls`: the level random by default).
    pub explosion_rolls: crate::explosion::Rolls,
    /// Injected kicks: the `nextDouble()` of the k-th TNT entity spawned (block primes and
    /// explosion primes together, in spawn order), in place of the level random's (still drawn
    /// when mirrored) or `tnt_kick`. For robustness sweeps over every kick angle.
    pub tnt_kicks: Vec<f64>,
    /// TNT entities spawned so far (the index into `tnt_kicks`).
    pub tnt_spawned: u32,
    /// Injected fuses (10..=29) of the explosion-primed TNT (`TntBlock.wasExploded`), in order, in
    /// place of the level random's `nextInt(20) + 10`.
    pub chain_fuses: Vec<i32>,
    pub chain_spawned: u32,
    /// TNT blocks an explosion primed: (game time, cell), like `primed` for `TntBlock.prime`.
    pub chain_primed: Vec<(i64, BlockPos)>,
    /// Every explosion so far (`explosion::explode`): when, where, what it destroyed.
    pub explosions: Vec<crate::explosion::Blast>,
    /// The terrain lane: a world built from a terrain sample (`terrain::build_world`) carries its
    /// light, heightmaps, biomes and world spawn; `None` for the judges' void worlds.
    pub land: Option<Box<crate::terrain::Land>>,
    // ---- weather (mobinfra, `weather/`) ----
    /// The level's weather: `WeatherData`, the rain and thunder levels, lightning (its bolts).
    /// Off by default (no weather: nothing drawn, nothing changes).
    pub weather: crate::weather::Weather,
    /// The AFK player's position (a farm judge's declared spot): fire spreads and lava lights fire
    /// only within 128 blocks of a player (`ServerLevel.canSpreadFireAround`). `None` (the
    /// default, as in the bridge's sandboxes): no player.
    pub afk: Option<crate::entity::Vec3>,
    /// (tickexact) Blocks of the world outside redsim's box that the natural spawner reads: the
    /// void gate's stone 3 above its AFK player (the bridge places it; a cell `getRandomPosWithin`
    /// can pick, a redstone conductor that stops a spawn group there).
    pub outside: Vec<(BlockPos, Block)>,
    /// mobinfra: each chunk section's randomly ticking cells (`LevelChunkSection.tickingBlockCount`
    /// + `tickingFluidCount`, recalculated on load and kept by every write: [`World::put`]); built
    /// on the first chunk tick (`crops::tick_chunks`), `None` until then and after `load_cells`.
    pub ticking_sections: Option<crate::crops::TickingSections>,
}

impl World {
    /// An empty world covering `min..min+size` (exclusive).
    pub fn new(min: BlockPos, size: [i32; 3]) -> World {
        let n = (size[0] * size[1] * size[2]) as usize;
        World {
            min,
            size,
            cells: vec![Block::Air; n],
            time: 0,
            sub_tick: 0,
            ticks: LevelTicks::default(),
            fluid_ticks: LevelTicks::default(),
            events: BlockEvents::default(),
            nu_stack: Vec::new(),
            nu_added: Vec::new(),
            nu_count: 0,
            handling_tick: false,
            comparator_out: HashMap::new(),
            containers: Default::default(),
            torch_toggles: Vec::new(),
            wire_signal: Cell::new(true),
            piston: Default::default(),
            gravity: Default::default(),
            entities: Default::default(),
            mob: Default::default(),
            frames: Vec::new(),
            drops: Vec::new(),
            game_events: Vec::new(),
            sculk: Default::default(),
            watch_light: false,
            light_changes: Vec::new(),
            log_writes: false,
            writes: Vec::new(),
            errors: Vec::new(),
            primed: Vec::new(),
            remove_primed: false,
            tnt_kick: None,
            payloads: Vec::new(),
            drop_seed: None,
            drop_count: 0,
            dispense_count: 0,
            dismounted: Vec::new(),
            loot_salt: None,
            loot_sequences: std::collections::BTreeMap::new(),
            level_random: None,
            exact_seed: None,
            constructions: 0,
            random_ticks: None,
            tick_speed: 3,
            chunk_order: Vec::new(),
            entity_chunks: None,
            frozen: Vec::new(),
            explosion_rolls: Default::default(),
            tnt_kicks: Vec::new(),
            tnt_spawned: 0,
            chain_fuses: Vec::new(),
            chain_spawned: 0,
            chain_primed: Vec::new(),
            explosions: Vec::new(),
            land: None,
            weather: Default::default(),
            afk: None,
            outside: Vec::new(),
            ticking_sections: None,
        }
    }

    /// Mirror the bridge sandbox's level random: seeded `seed` (`level.getRandom().setSeed`, at
    /// create and reset), its chunk box `[minX, minZ, maxX, maxZ]` (chunk coordinates, forced:
    /// each chunk tick draws `randomTickSpeed` (3) `nextInt(48)`, chunk after chunk in the game's
    /// order, `chunk_order`).
    pub fn mirror_level_random(&mut self, seed: i64, chunks: [i32; 4]) {
        self.level_random = Some(crate::rng::LegacyRandom::new(seed));
        self.exact_seed = Some(seed);
        self.constructions = 0;
        self.chunk_order = crate::crops::tick_order(chunks);
        self.entity_chunks = Some(chunks);
        // the bridge reseeds the loot random sequences with the level random (Sandbox.lootSaltOf)
        self.loot_salt = Some(crate::stems::loot_salt(seed));
        self.loot_sequences.clear();
    }

    /// (tickexact) A new entity's own random right after its constructor (`EntityType.create`;
    /// the bridge's `Sandbox.onConstructed`): the sandbox's next construction seed
    /// ([`crate::rng::construct_seed`]), k counting every construction, spawn attempts that are
    /// then rejected included. What the entity draws from its own random before it enters
    /// (`finalizeSpawn`: a zombie's `handleAttributes`, a salmon's variant) comes from it. Without
    /// an exact world: a stand-in drawn from the mob layer's random.
    pub fn construct(&mut self) -> crate::rng::LegacyRandom {
        match self.exact_seed {
            Some(s) => {
                let k = self.constructions;
                self.constructions += 1;
                crate::rng::LegacyRandom::new(crate::rng::construct_seed(s, k))
            }
            None => crate::rng::LegacyRandom::new(self.mob.rng_mut().next_long()),
        }
    }

    /// (tickexact) Entity `n`'s own random when it enters the level (the bridge's
    /// `Exact.entitySeed`: n = its id, the n-th entity to enter), before its constructor's draws
    /// are replayed (the caller replays them: [`crate::rng::replay_uuid`], a living entity's yaw,
    /// its sensors, ...). Without an exact world: a stand-in drawn from the mob layer's random.
    pub fn entering_random(&mut self, n: u32) -> crate::rng::LegacyRandom {
        match self.exact_seed {
            Some(s) => crate::rng::LegacyRandom::new(crate::rng::entity_seed(s, n)),
            None => crate::rng::LegacyRandom::new(self.mob.rng_mut().next_long()),
        }
    }

    /// Does an entity at (x, z) tick? Only in the sandbox's forced chunks when they are known
    /// (`entity_chunks`): `ServerLevel.tick` skips an entity outside the entity-ticking range (a
    /// forced chunk has level 31, its neighbours 32+), so a TNT or an item flying out of them
    /// freezes there.
    pub fn entity_ticks_at(&self, x: f64, z: f64) -> bool {
        match self.entity_chunks {
            None => true,
            Some([x0, z0, x1, z1]) => {
                let (cx, cz) = ((x.floor() as i64).div_euclid(16) as i32, (z.floor() as i64).div_euclid(16) as i32);
                cx >= x0 && cx <= x1 && cz >= z0 && cz <= z1
            }
        }
    }

    /// Record entity `id` as frozen (outside `entity_chunks`), once.
    pub fn note_frozen(&mut self, id: u32) {
        if !self.frozen.iter().any(|(_, i)| *i == id) {
            self.frozen.push((self.time, id));
        }
    }

    /// The mirrored level random, if any (draw from it at the game's spot).
    pub fn rng(&mut self) -> Option<&mut crate::rng::LegacyRandom> {
        self.level_random.as_mut()
    }

    /// One `level.getRandom().nextFloat()` (a sound's random pitch: pistons, doors, trapdoors,
    /// fence gates, sculk clicks, dripleaf tilts...), when the level random is mirrored.
    pub fn draw_sound_pitch(&mut self) {
        if let Some(r) = self.level_random.as_mut() {
            r.next_float();
        }
    }

    /// `Block.popResource`'s three `Mth.nextDouble(level.random, -0.25, 0.25)` (one item stack a
    /// broken block drops; the bridge then reseeds the item's own random: `rng::Drop`), when the
    /// level random is mirrored.
    pub fn draw_pop_resource(&mut self) {
        if let Some(r) = self.level_random.as_mut() {
            for _ in 0..3 {
                r.next_double_between(-0.25, 0.25);
            }
        }
    }

    /// `DefaultDispenseItemBehavior.spawnItem(level, stack, 6, facing, position)` (clk-misc
    /// 2026-10-21: a dropper shooting into the world, a bubble-column elevator's intake): one
    /// item at the dispenser's center + 0.7 facing (y minus 0.125 vertical, 0.15625 horizontal),
    /// its motion from the level random (`pow = nextDouble() * 0.1 + 0.2`, then three
    /// `triangle(step * pow | 0.2, 0.0172275 * 6)`), no pickup delay; its own random as the bridge
    /// seeds a dispensed item (`Sandbox.dispenseSeed`: `seed ^ (0x64697370 + n)`, then the
    /// constructor's yaw and drift draws). Without the mirrored level random: a model error.
    pub fn spawn_dispensed(&mut self, pos: BlockPos, facing: crate::pos::Dir, item: &str, count: u32, payload: u16) {
        let (Some(seed), Some(r)) = (self.drop_seed, self.level_random.as_mut()) else {
            self.errors.push(format!("dropper at {pos} spits an item into the world: its motion draws from the level random and its own random from the sandbox seed (World::mirror_level_random, drop_seed)"));
            return;
        };
        let [sx, sy, sz] = facing.step();
        let mut p = [pos.x as f64 + 0.5 + 0.7 * sx as f64, pos.y as f64 + 0.5 + 0.7 * sy as f64, pos.z as f64 + 0.5 + 0.7 * sz as f64];
        p[1] -= if sy != 0 { 0.125 } else { 0.15625 };
        let pow = r.next_double() * 0.1 + 0.2;
        let acc = 0.0172275 * 6.0;
        let m = [r.triangle(sx as f64 * pow, acc), r.triangle(0.2, acc), r.triangle(sz as f64 * pow, acc)];
        let mut own = crate::rng::LegacyRandom::new(seed ^ 0x6469_7370_i64.wrapping_add(self.dispense_count as i64));
        self.dispense_count += 1;
        let y_rot = own.next_float() * 360.0;
        own.next_double();
        own.next_double();
        let state = crate::entity::item::ItemState { item: item.to_string(), count: count.min(255) as u8, pickup_delay: 0, age: 0, payload, health: 5 };
        let id = self.entities.add(crate::entity::EntityKind::Item(state), crate::entity::Vec3::new(p[0], p[1], p[2]), y_rot);
        if let Some(e) = self.entities.list.iter_mut().rev().find(|e| e.id == id) {
            e.delta_movement = crate::entity::Vec3::new(m[0], m[1], m[2]);
            e.random = Some(own);
        }
    }

    /// `Block.popResource(level, pos, stack)`: the three level-random draws (when mirrored), and
    /// with a drop seed the item entity the bridge spawns (`rng::Drop`, the sandbox's n-th drop;
    /// `payload`: a shulker box item's contents).
    pub fn pop_resource(&mut self, pos: BlockPos, item: &str, count: u32, payload: u16) {
        self.draw_pop_resource();
        let Some(seed) = self.drop_seed else { return };
        let d = crate::rng::Drop::seeded(seed, self.drop_count, pos);
        self.drop_count += 1;
        let state = crate::entity::item::ItemState { item: item.to_string(), count: count.min(255) as u8, pickup_delay: 10, age: 0, payload, health: 5 };
        let id = self.entities.add(crate::entity::EntityKind::Item(state), crate::entity::Vec3::new(d.pos[0], d.pos[1], d.pos[2]), d.y_rot);
        if let Some(e) = self.entities.list.iter_mut().rev().find(|e| e.id == id) {
            e.delta_movement = crate::entity::Vec3::new(d.motion[0], d.motion[1], d.motion[2]);
            e.random = Some(d.random.clone());
        }
    }

    /// `Block.dropResources(state, level, pos)` of the block `s` just broken at `pos` (callers
    /// record it in `drops`): with the level random mirrored or a drop seed, every stack of its
    /// loot table pops (`pop_resource`); a table redsim cannot evaluate is a model error. Without
    /// either, nothing (no draw, no entity).
    pub fn drop_resources(&mut self, pos: BlockPos, s: Block) {
        if self.level_random.is_none() && self.drop_seed.is_none() {
            return;
        }
        let payload = crate::redstone::shulker::box_payload(self, pos, &s).unwrap_or(0);
        // a melon's slices draw from its loot table's random sequence (stems::melon_slices)
        if s == Block::Simple(crate::block::Simple::Melon) {
            match crate::stems::melon_slices(self) {
                Ok(n) => self.pop_resource(pos, "minecraft:melon_slice", n, payload),
                Err(e) => self.errors.push(e),
            }
            return;
        }
        match crate::loot::drops(&s) {
            Ok(v) => {
                for (item, n) in v {
                    self.pop_resource(pos, &item, n, payload);
                }
            }
            Err(e) => self.errors.push(e),
        }
    }

    pub(crate) fn index(&self, p: BlockPos) -> Option<usize> {
        let x = p.x - self.min.x;
        let y = p.y - self.min.y;
        let z = p.z - self.min.z;
        if x < 0 || y < 0 || z < 0 || x >= self.size[0] || y >= self.size[1] || z >= self.size[2] {
            return None;
        }
        Some(((y * self.size[2] + z) * self.size[0] + x) as usize)
    }

    pub fn contains(&self, p: BlockPos) -> bool {
        self.index(p).is_some()
    }

    pub fn get(&self, p: BlockPos) -> Block {
        match self.index(p) {
            Some(i) => self.cells[i],
            None => Block::Air,
        }
    }

    /// Write a state with no side effect at all (the chunk section write).
    pub fn put(&mut self, p: BlockPos, s: Block) {
        match self.index(p) {
            Some(i) => {
                if self.watch_light && self.cells[i] != s && crate::daylight::light_differs(&self.cells[i], &s) {
                    // (daylight detectors) the light engine's timing: recent changes only
                    let now = self.time;
                    self.light_changes.retain(|&(t, _)| t >= now - 1);
                    self.light_changes.push((now, p));
                }
                if self.log_writes && self.cells[i] != s {
                    self.writes.push(p);
                }
                let old = self.cells[i];
                self.cells[i] = s;
                // the section's ticking count (`LevelChunkSection.setBlockState`)
                if old != s
                    && let Some(ts) = self.ticking_sections.as_mut()
                {
                    let (was, is) = (crate::crops::ticks_in_section(&old), crate::crops::ticks_in_section(&s));
                    if was != is {
                        ts.adjust(p, is);
                    }
                }
                if self.land.is_some() && old != s {
                    // terrain worlds: heightmaps and sky sources follow the change (`terrain::land`)
                    self.land_changed(p, old, s);
                }
            }
            None => {
                if !s.is_air() {
                    self.errors.push(format!("write outside the box at {p}: {s}"));
                }
            }
        }
    }

    /// Every non-air cell.
    pub fn blocks(&self) -> Vec<(BlockPos, Block)> {
        let mut v = Vec::new();
        for y in 0..self.size[1] {
            for z in 0..self.size[2] {
                for x in 0..self.size[0] {
                    let p = self.min.offset(x, y, z);
                    let s = self.get(p);
                    if !s.is_air() {
                        v.push((p, s));
                    }
                }
            }
        }
        v
    }

    /// Fill the whole box at once, x fastest then z then y (a terrain sample: no update, no
    /// change record; `terrain::build_world` then installs the land).
    pub fn load_cells(&mut self, cells: Vec<Block>) {
        assert_eq!(cells.len(), self.cells.len(), "load_cells: the box holds {} cells", self.cells.len());
        self.cells = cells;
        // the sections' ticking counts are recalculated on the next chunk tick (a section's load)
        self.ticking_sections = None;
    }

    /// A world over the box holding these cells (x fastest, then z, then y), without filling it
    /// with air first (a terrain box: 5.5M cells).
    pub fn with_cells(min: BlockPos, size: [i32; 3], cells: Vec<Block>) -> World {
        assert_eq!(cells.len(), (size[0] * size[1] * size[2]) as usize, "with_cells: the box needs {} cells", size[0] * size[1] * size[2]);
        let mut w = World::new(min, [0, 0, 0]);
        w.size = size;
        w.cells = cells;
        w
    }

    // ------------------------------------------------------------------ setBlock

    pub fn set_block(&mut self, pos: BlockPos, state: Block, fl: u32) -> bool {
        self.set_block_limit(pos, state, fl, UPDATE_LIMIT)
    }

    /// `Level.setBlock` (+ `LevelChunk.setBlockState`).
    pub fn set_block_limit(&mut self, pos: BlockPos, state: Block, fl: u32, limit: i32) -> bool {
        if !self.contains(pos) {
            if !state.is_air() {
                self.errors.push(format!("setBlock outside the box at {pos}: {state}"));
            }
            return false;
        }
        let old = self.get(pos);
        if old == state {
            return false;
        }
        self.put(pos, state);
        let new_kind = state.kind();
        let block_changed = old.kind() != new_kind;
        let moved = fl & flags::MOVE_BY_PISTON != 0;
        let side_effects = fl & flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS == 0;
        if block_changed && bh::has_block_entity(&old) {
            if side_effects {
                crate::piston::pre_remove_side_effects(self, pos, old);
            }
            self.remove_block_entity(pos, old);
        }
        // LevelChunk.setBlockState: `blockChanged || newBlock instanceof BaseRailBlock` (every
        // rail kind: powered, activator, plain, detector)
        if (block_changed || crate::rails::is_rail(&state)) && (fl & flags::NEIGHBORS != 0 || moved) {
            bh::affect_neighbors_after_removal(self, old, pos, moved);
        }
        if self.get(pos).kind() != new_kind {
            return false;
        }
        if fl & flags::SKIP_ON_PLACE == 0 {
            bh::on_place(self, state, pos, old, moved);
        }
        if bh::has_block_entity(&state) && self.get(pos).kind() == new_kind {
            self.ensure_block_entity(pos, state);
        }
        // Level.setBlock after the chunk write
        if self.get(pos) == state {
            // ---- mob AI (the mob-AI lane): the POI index; `sendBlockUpdated` (flag 2): paths
            // near it recompute ----
            if self.mob.ai.on {
                crate::entity::mob::ai::poi::on_block_changed(self, pos, &old, &state);
                if fl & flags::CLIENTS != 0 {
                    crate::entity::mob::ai::send_block_updated(self, pos, old, state);
                }
            }
            if fl & flags::NEIGHBORS != 0 {
                self.update_neighbors_at(pos, old.kind());
                if bh::has_analog_output(&state) {
                    self.update_neighbour_for_output_signal(pos, new_kind);
                }
            }
            if fl & flags::KNOWN_SHAPE == 0 && limit > 0 {
                // `updateFlags & -34`: shape updates never update neighbors (bit 1) nor
                // suppress drops (bit 32)
                let nf = fl & !(flags::NEIGHBORS | flags::SUPPRESS_DROPS);
                bh::update_indirect_neighbour_shapes(self, old, pos, nf, limit - 1);
                self.update_neighbour_shapes(state, pos, nf, limit - 1);
                bh::update_indirect_neighbour_shapes(self, state, pos, nf, limit - 1);
            }
        }
        true
    }

    fn remove_block_entity(&mut self, pos: BlockPos, old: Block) {
        match old.kind() {
            Kind::Comparator => {
                self.comparator_out.remove(&pos);
            }
            Kind::MovingPiston => crate::piston::remove_block_entity(self, pos),
            Kind::Dispenser(_) | Kind::Hopper | Kind::ChiseledBookshelf | Kind::Crafter | Kind::Jukebox | Kind::Furnace | Kind::BrewingStand => crate::redstone::container::remove(self, pos),
            // ---- shulker study (clk-rt) ----
            Kind::ShulkerBox(_) | Kind::Chest => crate::redstone::container::remove(self, pos),
            Kind::DaylightDetector => crate::redstone::container::remove_ticker(self, pos),
            Kind::Sculk(_) => crate::sculk::remove(self, pos),
            _ => {}
        }
    }

    fn ensure_block_entity(&mut self, pos: BlockPos, state: Block) {
        match state.kind() {
            Kind::Comparator => {
                self.comparator_out.entry(pos).or_insert(0);
            }
            Kind::Dispenser(_) | Kind::Hopper | Kind::ChiseledBookshelf | Kind::Crafter | Kind::Furnace | Kind::BrewingStand => crate::redstone::container::create(self, pos, &state),
            // ---- shulker study (clk-rt) ----
            Kind::ShulkerBox(_) | Kind::Chest => crate::redstone::container::create(self, pos, &state),
            Kind::DaylightDetector => crate::daylight::create(self, pos),
            Kind::Jukebox => {
                crate::redstone::container::create(self, pos, &state);
                crate::redstone::container::jukebox_update_ticker(self, pos, &state);
            }
            Kind::Sculk(_) => crate::sculk::create(self, pos),
            _ => {}
        }
    }

    /// `Level.setBlockAndUpdate` (flags 3).
    pub fn set_block_and_update(&mut self, pos: BlockPos, state: Block) -> bool {
        self.set_block(pos, state, flags::ALL)
    }

    /// `Level.removeBlock` (no fluids: air).
    pub fn remove_block(&mut self, pos: BlockPos, moved: bool) -> bool {
        // Level.removeBlock: the cell keeps its fluid (a waterlogged block leaves water)
        let keep = crate::fluid::fluid_state(self, pos).create_legacy_block();
        self.set_block(pos, keep, flags::ALL | if moved { flags::MOVE_BY_PISTON } else { 0 })
    }

    /// `Level.destroyBlock`: the drop (an item entity) is recorded in `drops`.
    pub fn destroy_block(&mut self, pos: BlockPos, drop: bool, limit: i32) -> bool {
        let s = self.get(pos);
        if s.is_air() {
            return false;
        }
        if drop && bh::drops_item(&s) {
            // dropResources: its loot's stacks pop (level-random draws, seeded items)
            self.drop_resources(pos, s);
            self.drops.push((pos, s));
        }
        // Level.destroyBlock: the cell keeps its fluid (a waterlogged block leaves water)
        let keep = crate::fluid::fluid_state_of(&s).create_legacy_block();
        let destroyed = self.set_block_limit(pos, keep, flags::ALL, limit);
        if destroyed {
            self.game_event_state("block_destroy", pos, s);
        }
        destroyed
    }

    /// `BlockStateBase.updateNeighbourShapes`.
    pub fn update_neighbour_shapes(&mut self, state: Block, pos: BlockPos, fl: u32, limit: i32) {
        for d in Dir::SHAPE_ORDER {
            self.neighbor_shape_changed(d.opposite(), pos.rel(d), pos, state, fl, limit);
        }
    }

    /// `Level.updateNeighbourForOutputSignal`: comparators next to the block, or behind a
    /// conductor next to it.
    pub fn update_neighbour_for_output_signal(&mut self, pos: BlockPos, source: Kind) {
        for d in Dir::HORIZONTAL {
            let mut p = pos.rel(d);
            let mut s = self.get(p);
            if s.kind() == Kind::Comparator {
                self.neighbor_changed_full(s, p, source, false);
            } else if bh::is_conductor(self, &s, p) {
                p = p.rel(d);
                s = self.get(p);
                if s.kind() == Kind::Comparator {
                    self.neighbor_changed_full(s, p, source, false);
                }
            }
        }
    }

    // ------------------------------------------------------------------ neighbor updater

    pub fn update_neighbors_at(&mut self, pos: BlockPos, source: Kind) {
        self.add_and_run(Nu::Multi { pos, source, skip: None, idx: if Dir::UPDATE_ORDER[0] == Dir::West { 0 } else { 0 } });
    }

    pub fn update_neighbors_except(&mut self, pos: BlockPos, source: Kind, skip: Dir) {
        let idx = if Dir::UPDATE_ORDER[0] == skip { 1 } else { 0 };
        self.add_and_run(Nu::Multi { pos, source, skip: Some(skip), idx });
    }

    pub fn neighbor_changed(&mut self, pos: BlockPos, source: Kind) {
        self.add_and_run(Nu::Simple { pos, source });
    }

    pub fn neighbor_changed_full(&mut self, state: Block, pos: BlockPos, source: Kind, moved: bool) {
        self.add_and_run(Nu::Full { state, pos, source, moved });
    }

    pub fn neighbor_shape_changed(&mut self, dir: Dir, pos: BlockPos, neighbor_pos: BlockPos, neighbor_state: Block, fl: u32, limit: i32) {
        self.add_and_run(Nu::Shape { dir, neighbor_state, pos, neighbor_pos, flags: fl, limit });
    }

    fn add_and_run(&mut self, u: Nu) {
        let running = self.nu_count > 0;
        let too_many = self.nu_count >= MAX_CHAINED_NEIGHBOR_UPDATES;
        self.nu_count += 1;
        if !too_many {
            if running {
                self.nu_added.push(u);
            } else {
                self.nu_stack.push(u);
            }
        } else if self.nu_count - 1 == MAX_CHAINED_NEIGHBOR_UPDATES {
            self.errors.push("too many chained neighbor updates".into());
        }
        if !running {
            self.run_updates();
        }
    }

    fn run_updates(&mut self) {
        while !self.nu_stack.is_empty() || !self.nu_added.is_empty() {
            while let Some(u) = self.nu_added.pop() {
                // pushed in reverse order: the first added ends on top
                self.nu_stack.push(u);
            }
            // (nu_added was drained from its end, i.e. last added pushed first: the first added
            // is now on top, as the game's reverse loop does)
            let top = self.nu_stack.len() - 1;
            while self.nu_added.is_empty() {
                let mut u = self.nu_stack[top];
                let more = self.run_next(&mut u);
                self.nu_stack[top] = u;
                if !more {
                    self.nu_stack.pop();
                    break;
                }
            }
        }
        self.nu_stack.clear();
        self.nu_added.clear();
        self.nu_count = 0;
    }

    /// One step of an entry; false when it is finished.
    fn run_next(&mut self, u: &mut Nu) -> bool {
        match u {
            Nu::Full { state, pos, source, moved } => {
                bh::neighbor_changed(self, *state, *pos, *source, *moved);
                false
            }
            Nu::Simple { pos, source } => {
                let s = self.get(*pos);
                bh::neighbor_changed(self, s, *pos, *source, false);
                false
            }
            Nu::Shape { dir, neighbor_state, pos, neighbor_pos, flags: fl, limit } => {
                let current = self.get(*pos);
                if *fl & flags::SKIP_SHAPE_UPDATE_ON_WIRE == 0 || current.kind() != Kind::Wire {
                    let new = bh::update_shape(self, current, *pos, *dir, *neighbor_pos, *neighbor_state);
                    self.update_or_destroy(current, new, *pos, *fl, *limit);
                }
                false
            }
            Nu::Multi { pos, source, skip, idx } => {
                let d = Dir::UPDATE_ORDER[*idx as usize];
                *idx += 1;
                let np = pos.rel(d);
                let s = self.get(np);
                bh::neighbor_changed(self, s, np, *source, false);
                if (*idx as usize) < 6 && Some(Dir::UPDATE_ORDER[*idx as usize]) == *skip {
                    *idx += 1;
                }
                (*idx as usize) < 6
            }
        }
    }

    /// `Block.updateOrDestroy`.
    pub fn update_or_destroy(&mut self, old: Block, new: Block, pos: BlockPos, fl: u32, limit: i32) {
        if new != old {
            if new.is_air() {
                self.destroy_block(pos, fl & flags::SUPPRESS_DROPS == 0, limit);
            } else {
                self.set_block_limit(pos, new, fl & !flags::SUPPRESS_DROPS, limit);
            }
        }
    }

    // ------------------------------------------------------------------ signals (SignalGetter)

    /// `getSignal(pos, direction)`: the block at `pos` asked by a neighbor `direction` away
    /// from it (the neighbor is at `pos.relative(direction.opposite())`).
    pub fn signal(&self, pos: BlockPos, dir: Dir) -> i32 {
        let s = self.get(pos);
        let sig = bh::get_signal(self, &s, pos, dir);
        if bh::is_conductor(self, &s, pos) { sig.max(self.direct_signal_to(pos)) } else { sig }
    }

    pub fn direct_signal(&self, pos: BlockPos, dir: Dir) -> i32 {
        let s = self.get(pos);
        bh::get_direct_signal(self, &s, pos, dir)
    }

    /// `getDirectSignalTo`: the strong power a block receives.
    pub fn direct_signal_to(&self, pos: BlockPos) -> i32 {
        let mut r = 0;
        for d in [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East] {
            r = r.max(self.direct_signal(pos.rel(d), d));
            if r >= 15 {
                return r;
            }
        }
        r
    }

    pub fn has_signal(&self, pos: BlockPos, dir: Dir) -> bool {
        self.signal(pos, dir) > 0
    }

    pub fn has_neighbor_signal(&self, pos: BlockPos) -> bool {
        [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East].into_iter().any(|d| self.signal(pos.rel(d), d) > 0)
    }

    pub fn best_neighbor_signal(&self, pos: BlockPos) -> i32 {
        let mut best = 0;
        for d in Dir::ALL {
            let s = self.signal(pos.rel(d), d);
            if s >= 15 {
                return 15;
            }
            best = best.max(s);
        }
        best
    }

    /// `getControlInputSignal` (diode side inputs).
    pub fn control_input_signal(&self, pos: BlockPos, dir: Dir, only_diodes: bool) -> i32 {
        let s = self.get(pos);
        if only_diodes {
            if bh::is_diode(&s) { self.direct_signal(pos, dir) } else { 0 }
        } else {
            match s {
                Block::RedstoneBlock => 15,
                Block::Wire { power, .. } => power as i32,
                _ => {
                    if bh::is_signal_source(self, &s) {
                        self.direct_signal(pos, dir)
                    } else {
                        0
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------ scheduled ticks, events

    /// `scheduleTick(pos, block, delay, priority)` (ignored when one is pending for the same
    /// position and block).
    pub fn schedule_tick(&mut self, pos: BlockPos, kind: Kind, delay: i32, priority: Priority) {
        let sub = self.sub_tick;
        self.sub_tick += 1;
        self.ticks.schedule(kind, pos, self.time + delay as i64, priority, sub);
    }

    pub fn has_scheduled_tick(&self, pos: BlockPos, kind: Kind) -> bool {
        self.ticks.has_scheduled_tick(pos, kind)
    }

    /// `scheduleTick(pos, fluid, delay)` (the fluid tick list; priority normal; the sub-tick
    /// counter is the level's, shared with block ticks).
    pub fn schedule_fluid_tick(&mut self, pos: BlockPos, fluid: crate::fluid::Fluid, delay: i32) {
        let sub = self.sub_tick;
        self.sub_tick += 1;
        self.fluid_ticks.schedule(fluid, pos, self.time + delay as i64, Priority::Normal, sub);
    }

    pub fn will_tick_this_tick(&self, pos: BlockPos, kind: Kind) -> bool {
        self.ticks.will_tick_this_tick(pos, kind)
    }

    /// `Level.blockEvent`.
    pub fn block_event(&mut self, pos: BlockPos, kind: Kind, a: i32, b: i32) {
        self.events.add(BlockEvent { pos, kind, a, b });
    }

    // ------------------------------------------------------------------ the tick

    /// `ServerLevel.tick` for a world with no entities, fluids or random ticks.
    pub fn tick(&mut self) {
        self.handling_tick = true;
        self.time += 1;
        // terrain worlds: the light engine settles last tick's changes between ticks
        if self.land.is_some() {
            self.settle_light();
        }
        // mobinfra: the weather (advanceWeatherCycle, first in ServerLevel.tick); off: nothing
        if self.weather.on {
            crate::weather::tick(self);
        }
        // a primed TNT explodes after its 80 gt fuse (it ticks from the tick it spawns in): not
        // modeled, unless the caller removes primed TNT before that (the TNT judge does); TNT
        // entities (`tnt_kick`) count their own fuse
        if !self.remove_primed && self.tnt_kick.is_none() {
            if let Some(&(t, p)) = self.primed.iter().find(|(t, _)| self.time >= *t + 79) {
                if !self.errors.iter().any(|e| e.starts_with("explosion")) {
                    self.errors.push(format!("explosion not modeled: the TNT primed at gt {t} ({p}) explodes"));
                }
            }
        }
        let due = self.ticks.collect(self.time);
        for t in due {
            self.ticks.start_running(&t);
            let s = self.get(t.pos);
            if s.kind() == t.kind {
                bh::tick(self, s, t.pos);
            }
        }
        self.ticks.end_tick();
        // ServerLevel.tick: fluidTicks right after blockTicks (`tickFluid` checks the type)
        if self.fluid_ticks.count() > 0 {
            let due = self.fluid_ticks.collect(self.time);
            for t in due {
                self.fluid_ticks.start_running(&t);
                crate::fluid::tick(self, t.pos, t.kind);
            }
            self.fluid_ticks.end_tick();
        }
        // ServerChunkCache.tickChunks (after raids, before block events): ServerLevel.tickChunk of
        // every forced chunk in the game's order: the precipitation draws, then the random ticks
        // of its randomly ticking sections (crops::tick_chunks)
        // terrain mode: NaturalSpawner's cycle first (`tickChunks` spawns before ticking chunks)
        if self.mob.natural.is_some() {
            crate::entity::mob::natural::tick_chunks(self);
        }
        // mobinfra: before the random ticks, tickChunk's thunder rolls
        // (`weather::lightning::tick_thunder`)
        if self.weather.on {
            crate::weather::lightning::tick_thunder(self);
        }
        crate::crops::tick_chunks(self);
        // tickexact: ServerChunkCache.tickChunks ends with the custom spawners (doMobSpawning)
        if self.mob.custom.is_some() {
            crate::entity::mob::custom::tick(self);
        }
        while let Some(e) = self.events.pop() {
            let s = self.get(e.pos);
            if s.kind() == e.kind {
                bh::trigger_event(self, s, e.pos, e.a, e.b);
            }
        }
        self.handling_tick = false;
        // a rider moved off a removed vehicle and still alive (its free-position search is not
        // modeled: `entity::stop_riding`)
        let gone: Vec<u32> = std::mem::take(&mut self.dismounted);
        if let Some(id) = gone.into_iter().find(|id| self.entities.list.iter().any(|e| e.id == *id && !e.removed)) {
            self.errors.push(format!("entity {id} dismounted from a removed vehicle and lives on: LivingEntity.dismountVehicle's findFreePosition is not modeled"));
        }
        crate::entity::tick_phase(self);
        // block entities: a design's hoppers exist before any moving block is created, so they
        // come first in `blockEntityTickers`
        crate::redstone::container::tick_hoppers(self);
        // sculk sensors of a design exist before any moving block too (hoppers placed with them:
        // see `sculk.rs`, an empty hopper's tick changes nothing)
        crate::sculk::tick_all(self);
        crate::piston::tick_block_entities(self);
        if !self.frames.is_empty() {
            crate::entity::frame::check_static(self);
        }
        // tickexact: the players tick after the levels (MinecraftServer.tickConnection ->
        // ServerPlayer.doTick): the AFK player's TIME_SINCE_REST
        if self.mob.custom.is_some() {
            crate::entity::mob::custom::player_tick(self);
        }
    }

    /// Nothing pending: no scheduled ticks, no block events, no moving blocks, no falling
    /// blocks (the bridge's `Sandbox.idle` for a design without entities).
    pub fn idle(&self) -> bool {
        self.ticks.count() == 0 && self.fluid_ticks.count() == 0 && self.events.is_empty() && crate::piston::is_idle(self) && crate::gravity::is_idle(self)
    }

    /// `Level.gameEvent` with no affected state (`GameEvent.Context.of(entity=null)`): recorded,
    /// and heard by the sculk sensors in range (`sculk::dispatch`).
    pub fn game_event(&mut self, name: &'static str, pos: BlockPos) {
        self.game_events.push((name, pos));
        if !self.sculk.is_empty() {
            crate::sculk::dispatch(self, name, pos, None);
        }
    }

    /// `Level.gameEvent` with an affected state (`GameEvent.Context.of(state)`: wool dampens).
    pub fn game_event_state(&mut self, name: &'static str, pos: BlockPos, state: Block) {
        self.game_events.push((name, pos));
        if !self.sculk.is_empty() {
            crate::sculk::dispatch(self, name, pos, Some(state));
        }
    }

    /// `Level.gameEvent` at an exact position (an entity's: `HIT_GROUND`, `ENTITY_DAMAGE`), with
    /// its affected state and whether its source entity dampens vibrations (`sculk::dispatch_at`).
    pub fn game_event_at(&mut self, name: &'static str, at: [f64; 3], affected: Option<Block>, source_dampens: bool) {
        let pos = BlockPos::new(at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32);
        self.game_events.push((name, pos));
        if !self.sculk.is_empty() {
            crate::sculk::dispatch_at(self, name, at, affected, source_dampens);
        }
    }

    /// Step until idle (at most `max` ticks); returns the ticks run.
    pub fn run_until_idle(&mut self, max: u32) -> u32 {
        let mut n = 0;
        while n < max && !self.idle() {
            self.tick();
            n += 1;
        }
        n
    }

    // ------------------------------------------------------------------ inputs

    /// Right-click a block (the bridge's `use`): levers and buttons directly (`LeverBlock.pull`,
    /// `ButtonBlock.press`), other blocks their `useWithoutItem` for a building player (note
    /// blocks tune, repeaters cycle the delay, comparators the mode, dust toggles cross/dot, oak
    /// trapdoors and doors open). Returns false when nothing is modeled for that block.
    pub fn use_block(&mut self, pos: BlockPos) -> bool {
        let s = self.get(pos);
        match s {
            Block::Lever { .. } => crate::redstone::lever::pull(self, s, pos),
            Block::Button { .. } => crate::redstone::button::press(self, s, pos),
            Block::Note { .. } => crate::redstone::blocks::note::use_block(self, s, pos),
            Block::Repeater { facing, delay, locked, powered } => {
                self.set_block_and_update(pos, Block::Repeater { facing, delay: delay % 4 + 1, locked, powered });
            }
            Block::Comparator { .. } => crate::redstone::diode::use_comparator(self, s, pos),
            Block::Wire { .. } => crate::redstone::wire::use_block(self, s, pos),
            Block::Trapdoor { iron: false, facing, top, open, powered } => {
                self.set_block(pos, Block::Trapdoor { iron: false, facing, top, open: !open, powered }, flags::CLIENTS);
                // TrapDoorBlock.toggle -> playSound (a level-random nextFloat pitch)
                self.draw_sound_pitch();
            }
            Block::Door { iron: false, facing, upper, hinge_right, open, powered } => {
                self.set_block(pos, Block::Door { iron: false, facing, upper, hinge_right, open: !open, powered }, flags::CLIENTS | flags::IMMEDIATE);
                // DoorBlock.useWithoutItem -> playSound (a level-random nextFloat pitch)
                self.draw_sound_pitch();
                self.game_event(if open { "block_close" } else { "block_open" }, pos);
            }
            Block::Trapdoor { .. } | Block::Door { .. } | Block::Air | Block::Water { .. } | Block::BubbleColumn { .. } => {}
            _ => {
                self.errors.push(format!("use on {s} at {pos} is not modeled"));
                return false;
            }
        }
        true
    }

    /// The judge's placement (`piston-door/src/video.rs::place`): raw writes in this order, a
    /// shape update of every wire, a `self` update of every block (in this order); then the
    /// caller settles (`run_until_idle`).
    pub fn place_like_judge(&mut self, blocks: &[(BlockPos, Block)]) {
        self.place_raw(blocks);
        self.place_updates(blocks);
    }

    /// The judge's placement, first half: raw writes in this order.
    pub fn place_raw(&mut self, blocks: &[(BlockPos, Block)]) {
        for (p, s) in blocks {
            self.set_block(*p, *s, flags::RAW);
        }
    }

    /// The judge's placement, second half: a shape update of every wire, then a `self` update of
    /// every block.
    pub fn place_updates(&mut self, blocks: &[(BlockPos, Block)]) {
        for (p, s) in blocks {
            if matches!(s, Block::Wire { .. }) {
                let cur = self.get(*p);
                let mut st = cur;
                for d in Dir::SHAPE_ORDER {
                    let np = p.rel(d);
                    let ns = self.get(np);
                    st = bh::update_shape(self, st, *p, d, np, ns);
                }
                if st != cur {
                    self.set_block(*p, st, flags::RAW);
                }
            }
        }
        for (p, _) in blocks {
            let k = self.get(*p).kind();
            self.neighbor_changed(*p, k);
        }
    }
}
