//! A farm built into a terrain sample (brief `designs/briefs/farm-iron.md` revision 8: day 1 is
//! judged in terrain; the terrain track's samples, `crate::terrain`). The site holds the sample's
//! world before the farm, the design's offset into it (the spot `terrain::place::best_spot` picks
//! for its footprint and depth) and what the build costs there: the natural blocks it digs and
//! the blocks it places. Two ways to build:
//!
//! - a design made for the void (`clear_box`): the whole box of its blocks is cleared, every
//!   column up to the sky (`place::clear`, what the terrain track's paste does), then its blocks
//!   go in: walls and floors included;
//! - a design made for terrain: only its listed cells are written, `air` included (a dug cell);
//!   every unlisted cell keeps its natural block, so natural rock is its walls.
//!
//! The farm's own box (`FarmSpec::lo..hi`, translated) stays the box a golem must not leave; the
//! world is the sample's whole box, with natural spawning around the declared AFK spot.

use super::farm::{FarmSpec, NetherSpec};
use crate::entity::Vec3;
use crate::pos::BlockPos;
use crate::terrain::{place, samples::Sample};
use crate::world::World;

/// A farm's place in a terrain sample.
pub struct Site {
    pub sample: &'static str,
    /// The sample's world before the farm (natural blocks, light, heightmaps, biomes); behind a
    /// lock so the judge's threads share one site (`World` is not `Sync`), and shared by every
    /// site in the same sample ([`sample_world_shared`]: the judge's 12 spots hold one copy).
    pub world: std::sync::Arc<std::sync::RwLock<World>>,
    /// Design coordinates to world coordinates.
    pub offset: [i32; 3],
    /// A void design: its blocks' box cleared first.
    pub clear_box: bool,
    /// Natural blocks the build removes.
    pub dug: u32,
    /// Blocks the build places (its listed cells that are not air).
    pub placed: u32,
    /// The AFK player's feet (world coordinates): natural spawning counts from it.
    pub afk: Vec3,
    /// The cells the build covers with glass (LEGACY_IRON_GOLEM excludes glass) so no golem spawns outside the farm's own
    /// floors (C7): every cell the panicking villagers' golem search finds whose floor is not one
    /// of the farm's blocks, covered until none is left.
    pub fills: Vec<BlockPos>,
    /// The natural cells the build seals with cobblestone ([`seals_at`]), and whether each held a
    /// block to dig first.
    pub seals: Vec<(BlockPos, bool)>,
    /// The fire clearance ([`clears_at`]): natural blocks within a lava's fire reach, dug (air) or
    /// replaced with cobblestone.
    pub clears: Vec<(BlockPos, crate::block::Block)>,
    /// How many spots the build was priced at, and their build times' range (seconds).
    pub spots: usize,
    pub build_range: (f64, f64),
    /// Every spot priced, cheapest first (the chosen one is `priced[k]` of [`Site::at_priced`]).
    pub priced: Vec<Priced>,
}

/// The spots priced (brief revision 8: the spot is a player's choice, the cheapest of the nearest).
pub const SPOTS: usize = 12;

/// Day 1's rank (brief `designs/briefs/farm-iron.md` revision 8, Stages: Day 1): what a build
/// takes a player, in the units the rank prices (2 s a block placed, gathering included; 1 s a
/// natural block dug; 120 s a lava source, one bucket trip each; 180 s a villager moved from its
/// village bed). One definition for the judge's card, the search (farmopt) and the pages.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effort {
    /// Blocks placed, the glass covers included.
    pub placed: u32,
    pub dug: u32,
    pub lava: u32,
    pub villagers: u32,
}

impl Effort {
    pub const PLACED_S: f64 = 2.0;
    pub const DUG_S: f64 = 1.0;
    pub const LAVA_S: f64 = 120.0;
    pub const VILLAGER_S: f64 = 180.0;

    /// The build time, seconds.
    pub fn seconds(&self) -> f64 {
        Effort::PLACED_S * self.placed as f64 + Effort::DUG_S * self.dug as f64 + Effort::LAVA_S * self.lava as f64 + Effort::VILLAGER_S * self.villagers as f64
    }
}

/// The iron of a full iron kit (the armour: 5 + 8 + 7 + 4).
pub const KIT_IRON: f64 = 24.0;

/// The way down to a dug-in farm's chest (a design made for terrain): a staircase dug from the
/// ground to the chest's level, one step down for each layer (3 cells dug a step: the step's
/// feet, head and the head room over the step above), a torch every 8 steps (no spawning on
/// it) and a fence gate on its top (no mob walks in; the player opens it): (blocks dug, blocks
/// placed) for `depth` layers. It leads only to the chest, with rock between it and the farm.
pub fn access_stair(depth: i32) -> (u32, u32) {
    if depth <= 0 {
        return (0, 0);
    }
    let d = depth as u32;
    (3 * d, d.div_ceil(8) + 1)
}

/// Day 1's rank: the minutes from the first block to owning a full iron kit: the build, then the
/// build's own iron (`bill_iron`, the tools counted), the kit and the iron expected lost meanwhile
/// (`losses_iron`: lightning, weather, hostiles during that time) at the farm's `rate` (iron an
/// hour). Infinite for a farm that makes no iron.
pub fn day1_minutes(effort: &Effort, bill_iron: f64, rate: f64, losses_iron: f64) -> f64 {
    if rate <= 0.0 {
        return f64::INFINITY;
    }
    effort.seconds() / 60.0 + (bill_iron + KIT_IRON + losses_iron) / rate * 60.0
}

/// A build priced at one spot: the cheap part (per-cell lookups in the sample's world).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotPrice {
    /// Design coordinates to world coordinates.
    pub offset: [i32; 3],
    /// Blocks the build places (its listed cells that are not air), the glass covers apart.
    pub placed: u32,
    /// Natural blocks it removes.
    pub dug: u32,
}

impl SpotPrice {
    /// Its blocks' share of the build time, the covers apart (2 s a block placed, 1 s dug).
    pub fn block_seconds(&self) -> f64 {
        Effort::PLACED_S * self.placed as f64 + Effort::DUG_S * self.dug as f64
    }
}

/// A spot priced whole: the cheap part, the seals ([`seals_at`]) and the glass covers
/// ([`covers_at`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Priced {
    pub price: SpotPrice,
    pub covers: Vec<BlockPos>,
    /// The natural cells sealed with cobblestone, and whether each held a block to dig first.
    pub seals: Vec<(BlockPos, bool)>,
    /// The fire clearance ([`clears_at`]): natural blocks within a lava's fire reach, dug (air)
    /// or replaced with cobblestone.
    pub clears: Vec<(BlockPos, crate::block::Block)>,
}

impl Priced {
    /// A spot's price with its seals, fire clearance and caps (the seals and the clearance
    /// computed once and handed on: tickexact 2026-11-08, they were recomputed by each of the
    /// three, the seals four times a spot).
    pub fn at(world: &World, spec: &FarmSpec, price: SpotPrice, clear_box: bool) -> Priced {
        let seals = seals_at(world, spec, price.offset, clear_box);
        let clears = clears_with(world, spec, price.offset, clear_box, &seals);
        let covers = covers_with(world, spec, price.offset, clear_box, true, &seals, &clears);
        Priced { covers, seals, clears, price }
    }

    /// Its blocks' share of the build time, the seals, the fire clearance and the covers placed.
    pub fn seconds(&self) -> f64 {
        let cleared_placed = self.clears.iter().filter(|(_, b)| !b.is_air()).count();
        self.price.block_seconds() + Effort::PLACED_S * (self.covers.len() + self.seals.len() + cleared_placed) as f64 + Effort::DUG_S * (self.seals.iter().filter(|(_, dig)| *dig).count() + self.clears.len()) as f64
    }
}

/// The fire clearance of a build at `offset` (role `fire_clearance`, brief farm-iron.md revision 9
/// iteration 8): every natural block a lava's random tick can set on fire, from the farm's own lava
/// and the terrain's lava within 4 of the farm, as `LavaFluid.randomTick` walks it
/// ([`crate::weather::fire::lava_random_tick`]): up to 2 steps up, each +1 in y and -1..1 in x and
/// z, through any cell outside `#blocks_lava_fire_spread` (`weather::blocks_motion`), a fire lit in
/// an air cell of the walk beside an `ignitedByLava` block; and with no step, an `ignitedByLava`
/// block of the 3 x 3 at the lava's level with air above it. Every path is walked (the walk is
/// computed, not sampled: a rare path burns too). Each natural block so reached is dug when it has
/// no collision (a plant, lichen: air after) or replaced with cobblestone (a log, leaves, planks),
/// and the walk runs again on the cleared cells until nothing is left. A flammable block of the
/// design is the design's own (the judge's fire line); seals are cobblestone already.
pub fn clears_at(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool) -> Vec<(BlockPos, crate::block::Block)> {
    clears_with(world, spec, offset, clear_box, &seals_at(world, spec, offset, clear_box))
}

/// [`clears_at`] with the spot's seals ([`seals_at`]) already found.
fn clears_with(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool, seals: &[(BlockPos, bool)]) -> Vec<(BlockPos, crate::block::Block)> {
    let at = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
    let (lo, hi) = blocks_box(spec);
    // lava within 4 of the farm, the walk's 2 up and 2 sideways, and a neighbour beyond
    let mut w = crop(world, at(lo).offset(-7, -7, -7), at(hi).offset(7, 7, 7));
    if clear_box {
        place::clear(&mut w, at(lo), at(hi));
    }
    let seals: Vec<(BlockPos, crate::block::Block)> = seals.iter().map(|(p, _)| (*p, SEAL)).collect();
    w.place_raw(&seals);
    let moved: Vec<(BlockPos, crate::block::Block)> = spec.blocks.iter().map(|(p, b)| (at(*p), *b)).collect();
    w.place_raw(&moved);
    let ours: crate::fxhash::FxHashSet<BlockPos> = moved.iter().map(|(p, _)| *p).chain(seals.iter().map(|(p, _)| *p)).collect();
    let is_lava = |w: &World, p: BlockPos| crate::fluid::fluid_state(w, p).fluid.is_lava();
    // the lava cells within 4 (each axis) of a farm block: the farm's box grown by 4 scanned
    // once, y then z then x (already the sorted order), each lava cell kept when a farm block is
    // within 4 of it (tickexact 2026-11-08: the 9 x 9 x 9 cube around every farm block was read,
    // a tenth of the static level)
    let mut lava: Vec<BlockPos> = Vec::new();
    if let (Some(gl), Some(gh)) = (
        moved.iter().map(|(p, _)| *p).reduce(|a, b| BlockPos::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z))),
        moved.iter().map(|(p, _)| *p).reduce(|a, b| BlockPos::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z))),
    ) {
        for y in gl.y - 4..=gh.y + 4 {
            for z in gl.z - 4..=gh.z + 4 {
                for x in gl.x - 4..=gh.x + 4 {
                    let q = BlockPos::new(x, y, z);
                    if w.contains(q) && is_lava(&w, q) && moved.iter().any(|(p, _)| (p.x - x).abs() <= 4 && (p.y - y).abs() <= 4 && (p.z - z).abs() <= 4) {
                        lava.push(q);
                    }
                }
            }
        }
    }
    lava.sort_by_key(|p| (p.y, p.z, p.x));
    lava.dedup();
    let natural_flammable = |w: &World, p: BlockPos| w.contains(p) && !ours.contains(&p) && crate::weather::fire::ignited_by_lava(&w.get(p));
    let mut out: Vec<(BlockPos, crate::block::Block)> = Vec::new();
    for _ in 0..16 {
        let mut found: Vec<BlockPos> = Vec::new();
        for l in &lava {
            let mut layer = vec![*l];
            for _ in 0..2 {
                let mut next = Vec::new();
                for p in &layer {
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            let t = p.offset(dx, 1, dz);
                            if !w.contains(t) {
                                continue;
                            }
                            let tb = w.get(t);
                            if tb.is_air() {
                                let burning: Vec<BlockPos> = crate::pos::Dir::ALL.iter().map(|d| t.rel(*d)).filter(|n| w.contains(*n) && crate::weather::fire::ignited_by_lava(&w.get(*n))).collect();
                                if burning.is_empty() {
                                    next.push(t);
                                }
                                found.extend(burning.into_iter().filter(|n| natural_flammable(&w, *n)));
                            } else if !crate::weather::blocks_motion(&tb) {
                                next.push(t);
                            }
                        }
                    }
                }
                layer = next;
            }
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let t = l.offset(dx, 0, dz);
                    if w.contains(t.above()) && w.get(t.above()).is_air() && natural_flammable(&w, t) {
                        found.push(t);
                    }
                }
            }
        }
        found.sort_by_key(|p| (p.y, p.z, p.x));
        found.dedup();
        if found.is_empty() {
            break;
        }
        for p in found {
            let b = if crate::behavior::collides(&w.get(p)) { SEAL } else { crate::block::Block::Air };
            w.set_block(p, b, crate::world::flags::RAW);
            out.push((p, b));
        }
    }
    out
}

/// The seals of a design made for terrain at `offset`: every natural cell beside one of the farm's
/// open cells (its listed cells that are not full blocks: the dug air, beds, trapdoors, signs,
/// water, lava, the pad's gates) that is not a full block itself (a cave's air, a plant, lichen)
/// is filled with cobblestone, so no natural mob walks in from a cave, no water or lava flows
/// out and no drop leaves; and every natural gravity block (sand, red sand, gravel) the build's
/// writes set falling, found by simulating the dig with the gravity model, cascades included
/// ([`falling_seals`]: gravel walls over a cave fall away when a pit is dug and drain it), is
/// replaced with cobblestone; each with whether its natural cell held a block to dig first. A void
/// design (its box cleared to the sky) has none.
pub fn seals_at(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool) -> Vec<(BlockPos, bool)> {
    if clear_box {
        return Vec::new();
    }
    let at = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
    let listed: crate::fxhash::FxHashSet<BlockPos> = spec.blocks.iter().map(|(p, _)| at(*p)).collect();
    let mut out: Vec<(BlockPos, bool)> = Vec::new();
    for (p, b) in &spec.blocks {
        if crate::behavior::full_cube(b) {
            continue;
        }
        // the six neighbours (a mob walks in, a fluid flows out), and the four diagonal ones at
        // the same height (a mob's pickup box reaches 1 block sideways through any corner:
        // `Mob.aiStep`'s inflate(1, 0, 1))
        let around = crate::pos::Dir::ALL.iter().map(|d| at(*p).rel(*d)).chain([(1, 1), (1, -1), (-1, 1), (-1, -1)].into_iter().map(|(dx, dz)| at(*p).offset(dx, 0, dz)));
        for q in around {
            if listed.contains(&q) || !world.contains(q) || out.iter().any(|(s, _)| *s == q) {
                continue;
            }
            let n = world.get(q);
            // (the surface's own air is the world's: under the open sky nothing walks in from a
            // cave; sealing it covered every built-in access's hatch with cobblestone, the cells
            // over its gates and around its top)
            let open_sky = (1..=96).map(|k| q.offset(0, k, 0)).take_while(|u| world.contains(*u)).all(|u| !crate::behavior::full_cube(&world.get(u)));
            if !crate::behavior::full_cube(&n) && !(place::air_like(&n) && open_sky) {
                out.push((q, !place::air_like(&n)));
            }
        }
        // (a falling block over an open cell: sand or gravel would drop into the farm when the
        // cell is dug; a farm dug into rock seals it first. Natural sand, red sand and gravel are
        // redsim's `Block::Falling` since 32dfe400da and are sealed by the dig's simulation below;
        // this keeps the inert natural falling blocks, an anvil's kind, that stay `Terrain`)
        let up = at(*p).above();
        if !listed.contains(&up)
            && world.contains(up)
            && !out.iter().any(|(s, _)| *s == up)
            && let crate::block::Block::Terrain(id) = world.get(up)
            && crate::terrain::blocks::props(id).classes.iter().any(|c| c.contains("FallingBlock"))
        {
            out.push((up, true));
        }
    }
    // (farmopt 2026-11-10, ironspec's ruling) every natural gravity block the dig sets falling,
    // cascades included: bed-plate-lava-out-of-sight's pit at village-edge build 0 had natural
    // gravel walls over cave air; digging the pit dropped them one after another, the pit's water
    // drained into the cave and the hopper's corner stayed dry (the bridge's compare: the same)
    let placed: crate::fxhash::FxHashMap<BlockPos, crate::block::Block> = spec.blocks.iter().map(|(p, b)| (at(*p), *b)).collect();
    for p in falling_seals(world, &placed, &out) {
        out.push((p, true));
    }
    out.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    out
}

/// The natural gravity blocks (`Block::Falling`: sand, red sand, gravel) a build sets falling,
/// each to seal (cobblestone, the block dug first): the dig simulated ([`first_movers`]), its
/// first movers sealed, and again until nothing falls. Sealing the first movers keeps the blocks a
/// falling one would have set moving where they are (a cascade starts at a first mover).
fn falling_seals(world: &World, placed: &crate::fxhash::FxHashMap<BlockPos, crate::block::Block>, seals: &[(BlockPos, bool)]) -> Vec<BlockPos> {
    let mut sealed: crate::fxhash::FxHashSet<BlockPos> = seals.iter().map(|(p, _)| *p).collect();
    let mut out: Vec<BlockPos> = Vec::new();
    loop {
        let movers = first_movers(world, placed, &sealed);
        if movers.is_empty() {
            return out;
        }
        for p in movers {
            if sealed.insert(p) {
                out.push(p);
            }
        }
    }
}

/// One simulation of a build's dig with the gravity model (`crate::gravity`): the cells it writes
/// (the farm's listed cells and the seals) over the natural world, each one's six neighbours
/// updated (`FallingBlock.updateShape` schedules the tick); a gravity block whose cell below is
/// free (`gravity::is_free`: air, fire, a liquid, a replaceable block) falls to the cell its
/// entity lands in (where the cell below is not free; it becomes the block there if that cell is
/// replaceable, else it breaks), its departure and its landing updating their own neighbours,
/// until nothing moves. Returns the gravity blocks the build's own writes set falling (the first
/// movers), sorted; a cell outside the world counts as solid.
fn first_movers(world: &World, placed: &crate::fxhash::FxHashMap<BlockPos, crate::block::Block>, sealed: &crate::fxhash::FxHashSet<BlockPos>) -> Vec<BlockPos> {
    use crate::block::Block;
    use crate::gravity;
    let mut moved: crate::fxhash::FxHashMap<BlockPos, Block> = Default::default();
    let get = |moved: &crate::fxhash::FxHashMap<BlockPos, Block>, p: BlockPos| -> Block {
        if let Some(b) = moved.get(&p).or_else(|| placed.get(&p)) {
            *b
        } else if sealed.contains(&p) || !world.contains(p) {
            SEAL
        } else {
            world.get(p)
        }
    };
    // (cell, whether the build's own write updated it)
    let mut queue: std::collections::VecDeque<(BlockPos, bool)> = placed.keys().chain(sealed.iter()).flat_map(|p| crate::pos::Dir::ALL.into_iter().map(move |d| (p.rel(d), true))).collect();
    let mut first: Vec<BlockPos> = Vec::new();
    while let Some((p, by_build)) = queue.pop_front() {
        if placed.contains_key(&p) || sealed.contains(&p) {
            continue;
        }
        let b = get(&moved, p);
        if !matches!(b, Block::Falling { .. }) || p.y < gravity::MIN_Y || !gravity::is_free(get(&moved, p.below())) {
            continue;
        }
        if by_build {
            first.push(p);
        }
        let mut land = p.below();
        while land.y > gravity::MIN_Y && gravity::is_free(get(&moved, land.below())) {
            land = land.below();
        }
        moved.insert(p, Block::Air);
        let there = get(&moved, land);
        if there.is_air() || crate::behavior::can_be_replaced(&there) {
            moved.insert(land, b);
        }
        for d in crate::pos::Dir::ALL {
            queue.push_back((p.rel(d), false));
            queue.push_back((land.rel(d), false));
        }
    }
    first.sort_by_key(|p| (p.y, p.z, p.x));
    first.dedup();
    first
}

/// The block a seal is.
pub const SEAL: crate::block::Block = crate::block::Block::Simple(crate::block::Simple::Cobblestone);

/// What the spot search keys on: a design's footprint (its blocks' columns), how deep its box
/// reaches under its ground layer, and its blocks' min corner (design coordinates). Valid spots
/// depend only on `size` and `below` (and the sample): a search can cache them per pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Footprint {
    pub size: [i32; 2],
    pub below: i32,
    pub lo: BlockPos,
}

/// A spec's footprint for a ground layer at design y `ground`.
pub fn footprint(spec: &FarmSpec, ground: i32) -> Result<Footprint, String> {
    if spec.blocks.is_empty() {
        return Err("an empty design".into());
    }
    let (lo, hi) = blocks_box(spec);
    Ok(Footprint { size: [hi.x - lo.x + 1, hi.z - lo.z + 1], below: ground - lo.y, lo })
}

/// The [`SPOTS`] nearest valid spots of a footprint in a sample's world: (min column, ground layer).
pub fn spots_for(w: &World, sample: &Sample, fp: Footprint) -> Vec<([i32; 2], i32)> {
    valid_spots(w, fp.size, fp.below, sample.near, sample.radius, SPOTS)
}

/// [`spots_for`] for a design as its `terrain:` meta places it: by its box; a farm dug wholly
/// into rock (`terrain: rock`, `FarmSpec::rock`: nothing on the surface) by its own cells under
/// the natural surface (`valid_spots_of`, the parent's ruling 20:06: the surface stays natural, so
/// its shape does not matter).
pub fn spots_for_spec(w: &World, sample: &Sample, spec: &FarmSpec, fp: Footprint, clear_box: bool) -> Vec<([i32; 2], i32)> {
    if clear_box || !spec.rock {
        return spots_for(w, sample, fp);
    }
    let ground = fp.lo.y + fp.below;
    let cells: Vec<BlockPos> = spec.blocks.iter().map(|(p, _)| BlockPos::new(p.x - fp.lo.x, p.y - ground, p.z - fp.lo.z)).collect();
    valid_spots_of(w, fp.size, fp.below, sample.near, sample.radius, SPOTS, Some(&cells))
}

/// A standard sample's world, read and built once per process (the judge, the search, the
/// exposures and the runs share it); each call returns its own copy (`World` is not `Sync`).
pub fn sample_world(sample: &Sample) -> Result<World, String> {
    let shared = sample_world_shared(sample)?;
    let w = shared.read().unwrap_or_else(|e| e.into_inner()).clone();
    Ok(w)
}

/// The one shared copy of a standard sample's world (built on first use): what every [`Site`]
/// made by [`Site::new`] holds. Read-only once built, behind a read-write lock so every thread
/// prices and copies it at once (tickexact 2026-11-08: a mutex serialized the optimizer's
/// candidates on it); its sections' ticking cells are counted once here, and every copy keeps them.
pub fn sample_world_shared(sample: &Sample) -> Result<std::sync::Arc<std::sync::RwLock<World>>, String> {
    type Shared = std::sync::Arc<std::sync::RwLock<World>>;
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Shared>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut m = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(w) = m.get(sample.file) {
        return Ok(w.clone());
    }
    let bytes = std::fs::read(sample.file).map_err(|e| format!("terrain sample {}: {e}", sample.file))?;
    let t = crate::terrain::format::Terrain::decode(&bytes)?;
    let mut world = crate::terrain::land::build_world(&t)?;
    world.ticking_sections = Some(crate::crops::TickingSections::build(&world));
    let w: Shared = std::sync::Arc::new(std::sync::RwLock::new(world));
    m.insert(sample.file.to_string(), w.clone());
    Ok(w)
}

/// The villagers a sample's world holds (its village's: village-edge has 8), from its saved
/// entities.
pub fn sample_villagers(sample: &Sample) -> Result<usize, String> {
    Ok(sample_entities(sample)?.iter().filter(|(id, _)| id == "villager").count())
}

/// A sample's saved entities (the village's villagers, golem, cats...): (type without the
/// `minecraft:` prefix, position).
pub fn sample_entities(sample: &Sample) -> Result<Vec<(String, [f64; 3])>, String> {
    let bytes = std::fs::read(sample.file).map_err(|e| format!("terrain sample {}: {e}", sample.file))?;
    let t = crate::terrain::format::Terrain::decode(&bytes)?;
    Ok(t.entities.iter().map(|e| (e.id.trim_start_matches("minecraft:").to_string(), e.pos)).collect())
}

/// The cheap part of a build's price at each of `spots` (from [`spots_for`]): blocks placed and
/// natural blocks dug, read cell by cell from the sample's world (no copy of it). `ground`: the
/// design y lying at the ground layer; `clear_box`: a void design (its box's columns cleared up
/// to the sky first), else a design made for terrain (only its listed cells written).
pub fn price_spots(w: &World, spec: &FarmSpec, fp: Footprint, ground: i32, clear_box: bool, spots: &[([i32; 2], i32)]) -> Vec<SpotPrice> {
    let (lo, hi) = blocks_box(spec);
    let placed = spec.blocks.iter().filter(|(_, b)| !b.is_air()).count() as u32;
    let top = w.min.y + w.size[1] - 1;
    spots
        .iter()
        .map(|(lo_xz, g)| {
            let offset = [lo_xz[0] - fp.lo.x, g - ground, lo_xz[1] - fp.lo.z];
            let at = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
            let dug = if clear_box {
                let (a, b) = (at(lo), at(hi));
                let mut n = 0u32;
                for y in a.y..=top {
                    for z in a.z..=b.z {
                        for x in a.x..=b.x {
                            n += !place::air_like(&w.get(BlockPos::new(x, y, z))) as u32;
                        }
                    }
                }
                n
            } else {
                spec.blocks.iter().filter(|(p, _)| !place::air_like(&w.get(at(*p)))).count() as u32
            };
            SpotPrice { offset, placed, dug }
        })
        .collect()
}

/// The caps of a build at `offset`: every natural cell a farm villager's golem search could spawn
/// a golem on outside the farm's own floors, capped until none is left ([`cover_block`]). Every
/// villager's search, not only those that panic at the farm's zombie: a villager panics too when
/// it is hurt or sees any hostile (mobinfra's game gate of D1U, run 5: a golem the blind third
/// villager summoned onto the surface escaped and stalled the farm). Reads a crop of the world
/// around the farm (the golem searches reach 8 columns and 7 layers from their villager), so it
/// costs a few milliseconds, not a copy of the sample.
pub fn covers_at(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool) -> Vec<BlockPos> {
    covers_from(world, spec, offset, clear_box, true)
}

/// [`covers_at`] for every farm villager's search (`all`) or only the panicking ones' (the
/// searches that run at the farm's own zombie: `panicking`).
pub fn covers_from(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool, all: bool) -> Vec<BlockPos> {
    let seals = seals_at(world, spec, offset, clear_box);
    let clears = clears_with(world, spec, offset, clear_box, &seals);
    covers_with(world, spec, offset, clear_box, all, &seals, &clears)
}

/// [`covers_from`] with the spot's seals and fire clearance already found.
fn covers_with(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool, all: bool, seals: &[(BlockPos, bool)], clears: &[(BlockPos, crate::block::Block)]) -> Vec<BlockPos> {
    let at = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
    let (lo, hi) = blocks_box(spec);
    let (mut clo, mut chi) = (at(lo).offset(-1, -1, -1), at(hi).offset(1, 1, 1));
    for (_, p, _) in &spec.entities {
        let c = BlockPos::new(p[0].floor() as i32 + offset[0], p[1].floor() as i32 + offset[1], p[2].floor() as i32 + offset[2]);
        clo = BlockPos::new(clo.x.min(c.x - 10), clo.y.min(c.y - 10), clo.z.min(c.z - 10));
        chi = BlockPos::new(chi.x.max(c.x + 10), chi.y.max(c.y + 10), chi.z.max(c.z + 10));
    }
    let mut w = crop(world, clo, chi);
    if clear_box {
        place::clear(&mut w, at(lo), at(hi));
    }
    // the seals go in first (`seals_at`); they are not the farm's floors: a cave cell over one is
    // covered like any natural spot; then the fire clearance (`clears_at`)
    let seals: Vec<(BlockPos, crate::block::Block)> = seals.iter().map(|(p, _)| (*p, SEAL)).collect();
    w.place_raw(&seals);
    w.place_raw(clears);
    let moved: Vec<(BlockPos, crate::block::Block)> = spec.blocks.iter().map(|(p, b)| (at(*p), *b)).collect();
    w.place_raw(&moved);
    let own: crate::fxhash::FxHashSet<BlockPos> = moved.iter().filter(|(_, b)| !b.is_air()).map(|(p, _)| *p).collect();
    // the villagers that panic: each sees its nearest zombie as built, or with every trapdoor
    // open, or every trapdoor shut (a hide shows or hides the zombie either way), and their
    // searches' starts
    for (k, p, n) in &spec.entities {
        let n = super::shift_home_nbt(n, offset);
        let _ = crate::entity::spawn(&mut w, k, [p[0] + offset[0] as f64, p[1] + offset[1] as f64, p[2] + offset[2] as f64], n.as_deref());
    }
    let starts = if all { summoner_starts(&w) } else { panicking(&w, &moved) };
    let mut fills = Vec::new();
    for _ in 0..64 {
        let cells: Vec<BlockPos> = spawn_cells(&w, &starts).into_iter().filter(|c| !own.contains(&c.below())).collect();
        if cells.is_empty() {
            break;
        }
        for c in cells {
            let b = cover_block(&w, c);
            w.set_block(c, b, crate::world::flags::RAW);
            fills.push(c);
        }
    }
    fills
}

/// The block that caps a stray golem spawn cell (rev 9 iterations 3-5: the cheapest at day-1
/// prices that stops `LEGACY_IRON_GOLEM`'s scan there): a bottom cobblestone slab (solid with air
/// above, so the scan stops on it, and its half top fails the golem's spawn: 3 cobblestone make 6),
/// or glass where the cell holds water or lava. [`covers_at`] scans again after each round of caps,
/// so a cap that let the scan reach a lower top would get one there too.
pub fn cover_block(w: &World, cell: BlockPos) -> crate::block::Block {
    if crate::fluid::fluid_state(w, cell).is_empty() {
        crate::block::Block::parse("minecraft:cobblestone_slab[type=bottom,waterlogged=false]").unwrap_or(crate::block::Block::Simple(crate::block::Simple::Glass))
    } else {
        crate::block::Block::Simple(crate::block::Simple::Glass)
    }
}

/// The cells of `w` in `lo..=hi` (clamped to its box) as a world of their own: blocks only (no
/// land, light or entities), for block reads that stay inside it.
pub fn crop(w: &World, lo: BlockPos, hi: BlockPos) -> World {
    let wmax = w.min.offset(w.size[0] - 1, w.size[1] - 1, w.size[2] - 1);
    let lo = BlockPos::new(lo.x.max(w.min.x), lo.y.max(w.min.y), lo.z.max(w.min.z));
    let hi = BlockPos::new(hi.x.min(wmax.x), hi.y.min(wmax.y), hi.z.min(wmax.z));
    let mut c = World::new(lo, [hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1]);
    if hi.x < lo.x || hi.y < lo.y || hi.z < lo.z {
        return c;
    }
    // each row copied whole (a fresh world's raw write is the cell's write alone: no neighbour,
    // shape or place update, no land, no AI), then a block entity's block written again through
    // setBlock, its entity made (tickexact 2026-11-09: a lookup and a test per cell made the crop
    // 6% of the viewer's still; 2026-11-08: the full setBlock per cell, a fifth of the static
    // level)
    let len = (hi.x - lo.x + 1) as usize;
    let mut entities = Vec::new();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            let (Some(si), Some(di)) = (w.index(BlockPos::new(lo.x, y, z)), c.index(BlockPos::new(lo.x, y, z))) else { continue };
            let row = &w.cells[si..si + len];
            c.cells[di..di + len].copy_from_slice(row);
            for (k, b) in row.iter().enumerate() {
                if !b.is_air() && crate::behavior::has_block_entity(b) {
                    entities.push((BlockPos::new(lo.x + k as i32, y, z), *b));
                }
            }
        }
    }
    for (p, b) in entities {
        if let Some(i) = c.index(p) {
            c.cells[i] = crate::block::Block::Air;
        }
        c.set_block(p, b, crate::world::flags::RAW);
    }
    c.light_version += 1;
    c
}

/// The terrain sample a design is judged and shown in (brief `farm-iron.md` revision 8: day 1 in
/// real terrain), from its meta: `sample: <name>` names a standard sample; a design made for
/// terrain (`terrain: cells`) without one is in village-edge, the day-1 standard; neither: the void.
/// One rule for the judge, the 3D view and the game gate.
pub fn sample_of(terrain: Option<&str>, sample: Option<&str>) -> Result<Option<&'static Sample>, String> {
    match (sample.map(str::trim), terrain.map(str::trim)) {
        (Some(name), _) => crate::terrain::samples::sample(name).map(Some).ok_or_else(|| format!("no terrain sample {name:?} (village-edge, plains, forest)")),
        (None, Some(_)) => Ok(crate::terrain::samples::sample("village-edge")),
        (None, None) => Ok(None),
    }
}

/// Every one of `spots` priced whole (the cheap part, then the covers), cheapest first (ties: the
/// nearer spot, `spots`' order).
pub fn price_all(w: &World, spec: &FarmSpec, fp: Footprint, ground: i32, clear_box: bool, spots: &[([i32; 2], i32)]) -> Vec<Priced> {
    let mut v: Vec<Priced> = price_spots(w, spec, fp, ground, clear_box, spots).into_iter().map(|price| Priced::at(w, spec, price, clear_box)).collect();
    v.sort_by(|a, b| a.seconds().total_cmp(&b.seconds()));
    v
}

/// The valid min columns for a footprint near `near` (level ground within 2, no refused natural
/// block: `place::refusal`), the `k` nearest, each with its ground layer.
pub fn valid_spots(w: &World, size: [i32; 2], below: i32, near: [i32; 2], radius: i32, k: usize) -> Vec<([i32; 2], i32)> {
    valid_spots_of(w, size, below, near, radius, k, None)
}

/// [`valid_spots`]; with `cells` (a `terrain: cells` design's own cells, footprint-relative:
/// x and z from the footprint's min column, y from the ground layer, negative below it), the
/// parent's ruling 20:06 for a farm dug into rock: the surface stays natural, so its shape does not
/// matter. The ground layer goes at the lowest surface over the footprint (the whole design under
/// the natural surface), and only the design's cells (and their neighbours) must be refusal-free.
pub fn valid_spots_of(w: &World, size: [i32; 2], below: i32, near: [i32; 2], radius: i32, k: usize, cells: Option<&[BlockPos]>) -> Vec<([i32; 2], i32)> {
    let mut out: Vec<(i64, [i32; 2], i32)> = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let lo_xz = [near[0] + dx, near[1] + dz];
            let hi_xz = [lo_xz[0] + size[0] - 1, lo_xz[1] + size[1] - 1];
            if lo_xz[0] < w.min.x + 8 || lo_xz[1] < w.min.z + 8 || hi_xz[0] > w.min.x + w.size[0] - 9 || hi_xz[1] > w.min.z + w.size[2] - 9 {
                continue;
            }
            let ground = place::ground_y(w, lo_xz, hi_xz);
            let mut low = i32::MAX;
            for z in lo_xz[1]..=hi_xz[1] {
                for x in lo_xz[0]..=hi_xz[0] {
                    low = low.min(place::surface_y(w, x, z));
                }
            }
            match cells {
                Some(_) => out.push(((dx as i64).pow(2) + (dz as i64).pow(2), lo_xz, low)),
                None => {
                    if ground - low > 2 {
                        continue;
                    }
                    out.push(((dx as i64).pow(2) + (dz as i64).pow(2), lo_xz, ground));
                }
            }
        }
    }
    out.sort();
    let mut keep = Vec::new();
    for (_, lo_xz, ground) in out {
        if keep.len() >= k {
            break;
        }
        let why = match cells {
            Some(cells) => place::cells_refusal(w, cells.iter().map(|c| BlockPos::new(lo_xz[0] + c.x, ground + c.y, lo_xz[1] + c.z))),
            None => {
                let lo = BlockPos::new(lo_xz[0], ground - below, lo_xz[1]);
                let hi = BlockPos::new(lo_xz[0] + size[0] - 1, ground, lo_xz[1] + size[1] - 1);
                place::refusal(w, lo, hi)
            }
        };
        // (`MCTAS_SPOT_WHY=1`: why each candidate was refused)
        if let Some(why) = &why
            && std::env::var("MCTAS_SPOT_WHY").is_ok()
        {
            eprintln!("spot {lo_xz:?} ground {ground}: {why}");
        }
        if why.is_none() {
            keep.push((lo_xz, ground));
        }
    }
    keep
}

/// Every villager's block position in `w` (the starts of their golem searches: any of them can
/// summon, [`covers_at`]).
pub fn villager_cells(w: &World) -> Vec<BlockPos> {
    (0..w.entities.list.len()).filter(|&i| matches!(&w.entities.list[i].kind, crate::entity::EntityKind::Mob(m) if m.ty == super::types::MobType::Villager)).map(|i| w.entities.list[i].block_position()).collect()
}

/// The starts of every villager's golem search (`spawnGolemIfNeeded`: `trySpawnMob` from its
/// `blockPosition()`): its cell as built and the cell above it. A villager woken in a 1-cell pod
/// (a panic wakes it: `VillagerPanicTrigger` → `stopSleeping`) finds no stand-up spot around its
/// bed (`BedBlock.findStandUpPosition`) and is placed at the fallback, the bed + 1.1, so its
/// block position is one up until it falls back onto the bed (mob-AI; never two: a jump from the
/// bed top peaks in the same block). mobinfra's traced gate of D1U, 2026-11-08: golems on the
/// surface at y 72, a floor at 71 that only a search from the plate villager woken one block above
/// its bed's cell at 65 reaches (the scan's first floor is its start + 5).
pub fn summoner_starts(w: &World) -> Vec<BlockPos> {
    let mut out: Vec<BlockPos> = Vec::new();
    for c in villager_cells(w) {
        for s in [c, c.above()] {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// The villagers that panic in a farm built in `w` (its entities in), by block position: each
/// sees its nearest zombie as built, or with every trapdoor of `blocks` open, or every one shut
/// (a hide shows or hides the zombie either way). Only they summon golems
/// (`VillagerPanicTrigger` -> `spawnGolemIfNeeded`).
pub fn panicking(w: &World, blocks: &[(BlockPos, crate::block::Block)]) -> Vec<BlockPos> {
    let mut starts = summoners(w);
    for open in [true, false] {
        let mut seeing = w.clone();
        for (p, b) in blocks {
            if let crate::block::Block::Trapdoor { iron, facing, top, powered, .. } = *b {
                seeing.set_block(*p, crate::block::Block::Trapdoor { iron, facing, top, open, powered }, crate::world::flags::RAW);
            }
        }
        for s in summoners(&seeing) {
            if !starts.contains(&s) {
                starts.push(s);
            }
        }
    }
    starts
}

/// The cells the golem searches of villagers that see a zombie start from.
fn summoners(w: &World) -> Vec<BlockPos> {
    use super::types::MobType;
    use crate::entity::EntityKind;
    let of = |t: MobType| (0..w.entities.list.len()).filter(|&i| matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.ty == t)).collect::<Vec<usize>>();
    let zombies = of(MobType::Zombie);
    let mut out = Vec::new();
    for i in of(MobType::Villager) {
        let pos = w.entities.list[i].pos;
        let Some(&z) = zombies.iter().min_by(|a, b| w.entities.list[**a].pos.distance_to_sqr(pos).total_cmp(&w.entities.list[**b].pos.distance_to_sqr(pos))) else { continue };
        if super::los::first_hit(w, super::villager::eye_pos(w, i), super::villager::eye_pos(w, z)).is_none() {
            out.push(w.entities.list[i].block_position());
        }
    }
    out
}

/// Every cell a golem can spawn at for searches started at `starts` (`trySpawnMob`'s 17 x 17
/// columns, `checkSpawnObstruction`).
fn spawn_cells(w: &World, starts: &[BlockPos]) -> Vec<BlockPos> {
    // (the cells in their first-found order; a set answers "seen" without searching the list:
    // tickexact 2026-11-09, the covers' rounds were a tenth of a day-1 page's spot pricing)
    let mut out: Vec<BlockPos> = Vec::new();
    let mut seen: crate::fxhash::FxHashSet<BlockPos> = Default::default();
    for s in starts {
        for (c, ok) in super::spawn::golem_spawn_cells_from(w, *s) {
            if ok.is_ok() && seen.insert(c) {
                out.push(c);
            }
        }
    }
    out
}

/// The box of a spec's blocks (inclusive).
fn blocks_box(spec: &FarmSpec) -> (BlockPos, BlockPos) {
    let (mut lo, mut hi) = (spec.blocks[0].0, spec.blocks[0].0);
    for (p, _) in &spec.blocks {
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    (lo, hi)
}

/// A design's site parameters from its meta values: `ground: <y>` (the design y lying at the
/// ground layer; default -1, a void design's floor under its villagers), `afk: x y z` (the AFK
/// player's feet; default on top of the build's middle), `terrain: cells` (written cell by cell,
/// natural rock for the rest; otherwise its box is cleared first): (ground, afk, clear_box).
pub fn params(spec: &FarmSpec, ground: Option<&str>, afk: Option<&str>, terrain: Option<&str>) -> Result<(i32, [f64; 3], bool), String> {
    if spec.blocks.is_empty() {
        return Err("an empty design".into());
    }
    let ground = ground.map(|v| v.trim().parse::<i32>()).transpose().map_err(|e| format!("ground: {e}"))?.unwrap_or(-1);
    let (lo, hi) = blocks_box(spec);
    let afk = match afk {
        Some(v) => {
            let p: Vec<f64> = v.split_whitespace().map(|t| t.parse::<f64>()).collect::<Result<_, _>>().map_err(|e| format!("afk: {e}"))?;
            if p.len() != 3 {
                return Err("afk: x y z".into());
            }
            [p[0], p[1], p[2]]
        }
        None => [(lo.x + hi.x) as f64 / 2.0 + 0.5, hi.y as f64 + 1.0, (lo.z + hi.z) as f64 / 2.0 + 0.5],
    };
    Ok((ground, afk, !matches!(terrain.map(str::trim), Some("cells") | Some("rock"))))
}

impl Site {
    /// The site of `spec` (design coordinates) in `sample`: its world from the sample's committed
    /// file (read relative to the working directory, the repository root, once per process:
    /// [`sample_world`]), the spot for the footprint of its blocks reaching `ground - lo.y` layers
    /// under the ground layer (`ground`: the design's y that lies at the ground layer), and the
    /// blocks dug and placed there. `afk`: the player's feet in design coordinates.
    pub fn new(sample: &'static Sample, spec: &FarmSpec, ground: i32, afk: [f64; 3], clear_box: bool) -> Result<Site, String> {
        Site::with_shared(sample_world_shared(sample)?, sample, spec, ground, afk, clear_box)
    }

    /// [`Site::new`] from the sample's decoded file (the 3D viewer, compiled to wasm, reads no file).
    pub fn from_terrain(t: &crate::terrain::format::Terrain, sample: &'static Sample, spec: &FarmSpec, ground: i32, afk: [f64; 3], clear_box: bool) -> Result<Site, String> {
        Site::with_world(crate::terrain::land::build_world(t)?, sample, spec, ground, afk, clear_box)
    }

    /// [`Site::new`] on a world already built from the sample ([`sample_world`]): the
    /// [`SPOTS`] nearest valid spots (level ground, no refused natural block) a player would take
    /// near the sample's start, each priced as built, the cheapest kept.
    pub fn with_world(world: World, sample: &'static Sample, spec: &FarmSpec, ground: i32, afk: [f64; 3], clear_box: bool) -> Result<Site, String> {
        Site::with_shared(std::sync::Arc::new(std::sync::RwLock::new(world)), sample, spec, ground, afk, clear_box)
    }

    /// [`Site::with_world`] on a shared world ([`sample_world_shared`]).
    pub fn with_shared(world: std::sync::Arc<std::sync::RwLock<World>>, sample: &'static Sample, spec: &FarmSpec, ground: i32, afk: [f64; 3], clear_box: bool) -> Result<Site, String> {
        let fp = footprint(spec, ground)?;
        let priced = {
            let w = world.read().unwrap_or_else(|e| e.into_inner());
            let spots = spots_for_spec(&w, sample, spec, fp, clear_box);
            if spots.is_empty() {
                return Err(format!("no spot for a {}x{} footprint {} deep in {}", fp.size[0], fp.size[1], fp.below, sample.name));
            }
            price_all(&w, spec, fp, ground, clear_box, &spots)
        };
        Ok(Site::at_priced(world, sample, priced, 0, afk, clear_box))
    }

    /// The site at the `k`-th cheapest of its priced spots (the judge's other builds).
    pub fn at_priced(world: std::sync::Arc<std::sync::RwLock<World>>, sample: &'static Sample, priced: Vec<Priced>, k: usize, afk: [f64; 3], clear_box: bool) -> Site {
        // the sections' ticking cells counted once here, so every copy of the site's world keeps
        // them (each run's first tick recounted 5.5M cells otherwise: tickexact 2026-11-09, 7% of
        // a cached farm's view session; the shared sample worlds have them already)
        if world.read().map(|w| w.ticking_sections.is_none()).unwrap_or(false)
            && let Ok(mut w) = world.write()
            && w.ticking_sections.is_none()
        {
            let ts = crate::crops::TickingSections::build(&w);
            w.ticking_sections = Some(ts);
        }
        let range = (priced.iter().map(Priced::seconds).fold(f64::INFINITY, f64::min), priced.iter().map(Priced::seconds).fold(0.0, f64::max));
        let chosen = priced[k.min(priced.len() - 1)].clone();
        let offset = chosen.price.offset;
        let afk = Vec3::new(afk[0] + offset[0] as f64, afk[1] + offset[1] as f64, afk[2] + offset[2] as f64);
        Site { sample: sample.name, world, offset, clear_box, dug: chosen.price.dug, placed: chosen.price.placed, afk, fills: chosen.covers, seals: chosen.seals, clears: chosen.clears, spots: priced.len(), build_range: range, priced }
    }

    /// The build effort at this site for day 1's rank ([`Effort`]): its blocks placed with the glass
    /// covers, the natural blocks dug, and the design's lava sources and villagers.
    pub fn effort(&self, lava: u32, villagers: u32) -> Effort {
        let dug_seals = self.seals.iter().filter(|(_, dig)| *dig).count() as u32;
        let cleared_placed = self.clears.iter().filter(|(_, b)| !b.is_air()).count() as u32;
        Effort { placed: self.placed + self.fills.len() as u32 + self.seals.len() as u32 + cleared_placed, dug: self.dug + dug_seals + self.clears.len() as u32, lava, villagers }
    }

    /// `spec` moved into the site's world.
    pub fn spec(&self, spec: &FarmSpec) -> FarmSpec {
        let [ox, oy, oz] = self.offset;
        let at = |p: BlockPos| BlockPos::new(p.x + ox, p.y + oy, p.z + oz);
        FarmSpec {
            blocks: spec.blocks.iter().map(|(p, b)| (at(*p), *b)).collect(),
            entities: spec.entities.iter().map(|(k, p, n)| (k.clone(), [p[0] + ox as f64, p[1] + oy as f64, p[2] + oz as f64], super::shift_home_nbt(n, [ox, oy, oz]))).collect(),
            output: at(spec.output),
            lo: at(spec.lo),
            hi: at(spec.hi),
            block_entities: spec.block_entities.iter().map(|(p, s)| (at(*p), s.clone())).collect(),
            rock: spec.rock,
            // the Nether part follows the anchor (its image moves by floor(anchor / 8))
            nether: spec.nether.as_ref().map(|n| NetherSpec { anchor: at(n.anchor), ..n.clone() }),
        }
    }

    /// The world a run of the moved spec starts from: the sample's, its box cleared for a void
    /// design (the moved spec's blocks then go in like the judge places a design).
    pub fn world_for(&self, moved: &FarmSpec) -> World {
        let mut w = self.world.read().map(|w| w.clone()).unwrap_or_else(|e| e.into_inner().clone());
        if self.clear_box {
            let (lo, hi) = blocks_box(moved);
            place::clear(&mut w, lo, hi);
        }
        let seals: Vec<(BlockPos, crate::block::Block)> = self.seals.iter().map(|(p, _)| (*p, SEAL)).collect();
        w.place_raw(&seals);
        w.place_raw(&self.clears);
        let covers: Vec<(BlockPos, crate::block::Block)> = self.fills.iter().map(|p| (*p, cover_block(&w, *p))).collect();
        w.place_raw(&covers);
        w
    }

    /// The blocks the site adds to its world before the farm, in [`Site::world_for`]'s order: its
    /// cobblestone seals, its fire clearance (air where it dug, cobblestone where it replaced),
    /// then the caps of its stray golem spawn cells (a cobblestone slab, or glass in a fluid). The
    /// game's side of a run in the site (the exact compare, the game gate) places the same, raw.
    pub fn site_blocks(&self) -> Vec<(BlockPos, crate::block::Block)> {
        let mut w = self.world.read().map(|w| w.clone()).unwrap_or_else(|e| e.into_inner().clone());
        let mut out: Vec<(BlockPos, crate::block::Block)> = self.seals.iter().map(|(p, _)| (*p, SEAL)).collect();
        out.extend(self.clears.iter().copied());
        w.place_raw(&out);
        out.extend(self.fills.iter().map(|p| (*p, cover_block(&w, *p))));
        out
    }

    /// The caps of its stray golem spawn cells, as [`Site::world_for`] places them: (cobblestone
    /// slabs, glass blocks).
    pub fn cover_counts(&self) -> (usize, usize) {
        let w = self.world.read().map(|w| w.clone()).unwrap_or_else(|e| e.into_inner().clone());
        let glass = self.fills.iter().filter(|p| !crate::fluid::fluid_state(&w, **p).is_empty()).count();
        (self.fills.len() - glass, glass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;

    fn block(s: &str) -> Block {
        Block::parse(s).ok().or_else(|| crate::terrain::blocks::id_of(s).map(Block::Terrain)).unwrap_or_else(|| panic!("{s}: no block"))
    }

    /// Water poured at the sample's edge goes on past it, into the bridge sandbox's void (the
    /// world's `outer` cells) with no error (tickexact 2026-11-08: D1U's live view stopped on a
    /// natural water's `setBlock outside the box`): it spreads out of the box and falls there.
    #[test]
    fn water_flows_past_the_sample_edge() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/terrain/village-edge.mcterrain");
        let t = crate::terrain::format::Terrain::decode(&std::fs::read(path).unwrap()).unwrap();
        let mut w = crate::terrain::land::build_world(&t).unwrap();
        let p = BlockPos::new(w.min.x, 100, w.min.z + 20);
        assert!(w.get(p).is_air() && w.get(p.offset(-1, 0, 0)).is_air(), "an open cell at the edge");
        w.set_block(p, Block::Water { level: 0 }, 3);
        for _ in 0..200 {
            w.tick();
        }
        assert!(w.errors.is_empty(), "errors: {:?}", w.errors);
        let out: Vec<BlockPos> = w.outer.iter().filter(|(_, b)| matches!(b, Block::Water { .. })).map(|(q, _)| *q).collect();
        assert!(out.iter().any(|q| q.x == w.min.x - 1 && q.y == 100), "the source spread west past the edge: {out:?}");
        assert!(out.iter().any(|q| q.y < 95), "and fell there: {out:?}");
    }

    /// The fire clearance's walk (`LavaFluid.randomTick`, every path): a short grass beside the
    /// farm's lava at its level is dug, a log over an air cell 2 up is replaced with cobblestone,
    /// a grass 3 columns away and a log 4 up are out of reach, and the farm's own flammable block
    /// is the design's.
    #[test]
    fn fire_clearance_walk() {
        let mut w = World::new(BlockPos::new(-12, -4, -12), [25, 16, 25]);
        let stone = block("minecraft:stone");
        for z in -12..=12 {
            for x in -12..=12 {
                w.set_block(BlockPos::new(x, -1, z), stone, crate::world::flags::RAW);
            }
        }
        let grass = block("minecraft:short_grass");
        let log = block("minecraft:oak_log[axis=y]");
        // (a grass on a stone step 2 columns over, its cell beside the walk's first air; a log in
        // the lava's 3 x 3 with air over it; a log over the walk's air 2 up; a grass beside the
        // lava, which its seal replaces; and three blocks out of reach)
        let step = (2, 0, 0);
        for (p, b) in [(step, stone), ((2, 1, 0), grass), ((1, 0, 1), log), ((-1, 0, 0), grass), ((0, 3, 0), log), ((3, 0, 3), grass), ((5, 4, 5), log), ((-4, 4, 0), log)] {
            w.set_block(BlockPos::new(p.0, p.1, p.2), b, crate::world::flags::RAW);
        }
        let planks = block("minecraft:oak_planks");
        let spec = FarmSpec {
            blocks: vec![(BlockPos::new(0, 0, 0), block("minecraft:lava[level=0]")), (BlockPos::new(-1, 1, 0), planks)],
            entities: Vec::new(),
            output: BlockPos::new(0, -2, 0),
            lo: BlockPos::new(-8, -3, -8),
            hi: BlockPos::new(8, 8, 8),
            block_entities: Vec::new(),
            rock: false,
            nether: None,
        };
        let clears = clears_at(&w, &spec, [0, 0, 0], false);
        let got: Vec<([i32; 3], bool)> = clears.iter().map(|(p, b)| ([p.x, p.y, p.z], b.is_air())).collect();
        assert!(got.contains(&([2, 1, 0], true)), "the grass beside the walk's first air is dug: {got:?}");
        assert!(got.contains(&([1, 0, 1], false)), "the log in the lava's 3 x 3 is replaced: {got:?}");
        assert!(got.contains(&([0, 3, 0], false)), "the log over the walk's air 2 up is replaced: {got:?}");
        assert!(!got.iter().any(|(p, _)| *p == [3, 0, 3] || *p == [5, 4, 5] || *p == [-4, 4, 0]), "out of reach: {got:?}");
        assert!(!got.iter().any(|(p, _)| *p == [-1, 1, 0] || *p == [-1, 0, 0]), "the design's planks are its own, the grass by the lava its seal's: {got:?}");
        assert_eq!(got.len(), 3, "{got:?}");
    }

    /// The dig's gravity (farmopt 2026-11-10, ironspec's ruling on bed-plate-lava-out-of-sight's
    /// pit): a gravel beside the farm's open cell over air falls when the cell is dug and is
    /// sealed; a gravel on a gravel over air stays (nothing updates the lower one once the first
    /// mover is sealed: the cascade it would have started); a gravel on stone stays; a gravel over
    /// the open cell (it would drop in) is sealed, as `Block::Falling` (the natural row's rule no
    /// longer sees it since natural gravel falls).
    #[test]
    fn gravel_the_dig_sets_falling_is_sealed() {
        let mut w = World::new(BlockPos::new(-12, -4, -12), [25, 16, 25]);
        let stone = block("minecraft:stone");
        for y in -4..=6 {
            for z in -6..=6 {
                for x in -6..=6 {
                    w.set_block(BlockPos::new(x, y, z), stone, crate::world::flags::RAW);
                }
            }
        }
        let gravel = block("minecraft:gravel");
        assert!(matches!(gravel, Block::Falling { .. }));
        // the first mover (0, 0, 1) over air; the cascade's column (1, 0, 1) on (1, -1, 1) over air,
        // beside the first mover's landing (0, -1, 1); a gravel on stone; a gravel over the open cell
        for (p, b) in [((0, 0, 1), gravel), ((0, -1, 1), Block::Air), ((1, 0, 1), gravel), ((1, -1, 1), gravel), ((1, -2, 1), Block::Air), ((-1, 0, 0), gravel), ((0, 1, 0), gravel)] {
            w.set_block(BlockPos::new(p.0, p.1, p.2), b, crate::world::flags::RAW);
        }
        let spec = FarmSpec {
            blocks: vec![(BlockPos::new(0, 0, 0), Block::Air), (BlockPos::new(1, 0, 0), Block::Air)],
            entities: Vec::new(),
            output: BlockPos::new(0, -3, 0),
            lo: BlockPos::new(-3, -3, -3),
            hi: BlockPos::new(3, 3, 3),
            block_entities: Vec::new(),
            rock: false,
            nether: None,
        };
        let got: Vec<([i32; 3], bool)> = seals_at(&w, &spec, [0, 0, 0], false).iter().map(|(p, dig)| ([p.x, p.y, p.z], *dig)).collect();
        assert_eq!(got, vec![([0, 0, 1], true), ([0, 1, 0], true)], "the first mover and the gravel over the open cell, dug first");
        // without the seal, the dig drops the first mover and the cascade follows
        let placed: crate::fxhash::FxHashMap<BlockPos, Block> = spec.blocks.iter().copied().collect();
        let movers = first_movers(&w, &placed, &Default::default());
        assert_eq!(movers, vec![BlockPos::new(0, 0, 1), BlockPos::new(0, 1, 0)], "the build's own updates set those two falling");
    }
}
