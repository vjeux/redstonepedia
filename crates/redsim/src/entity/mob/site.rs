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

use super::farm::FarmSpec;
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
    pub world: std::sync::Arc<std::sync::Mutex<World>>,
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
}

impl Priced {
    /// Its blocks' share of the build time, the seals and the covers placed.
    pub fn seconds(&self) -> f64 {
        self.price.block_seconds() + Effort::PLACED_S * (self.covers.len() + self.seals.len()) as f64 + Effort::DUG_S * self.seals.iter().filter(|(_, dig)| *dig).count() as f64
    }
}

/// The seals of a design made for terrain at `offset`: every natural cell beside one of the farm's
/// open cells (its listed cells that are not full blocks: the dug air, beds, trapdoors, signs,
/// water, lava, the pad's gates) that is not a full block itself (a cave's air, a plant, lichen)
/// is filled with cobblestone, so no natural mob walks in from a cave, no water or lava flows
/// out and no drop leaves; each with whether its natural cell held a block to dig first. A void
/// design (its box cleared to the sky) has none.
pub fn seals_at(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool) -> Vec<(BlockPos, bool)> {
    if clear_box {
        return Vec::new();
    }
    let at = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
    let listed: std::collections::HashSet<BlockPos> = spec.blocks.iter().map(|(p, _)| at(*p)).collect();
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
            if !crate::behavior::full_cube(&n) {
                out.push((q, !place::air_like(&n)));
            }
        }
    }
    out.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    out
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

/// A standard sample's world, read and built once per process (the judge, the search, the
/// exposures and the runs share it); each call returns its own copy (`World` is not `Sync`).
pub fn sample_world(sample: &Sample) -> Result<World, String> {
    let shared = sample_world_shared(sample)?;
    let w = shared.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Ok(w)
}

/// The one shared copy of a standard sample's world (built on first use): what every [`Site`]
/// made by [`Site::new`] holds.
pub fn sample_world_shared(sample: &Sample) -> Result<std::sync::Arc<std::sync::Mutex<World>>, String> {
    type Shared = std::sync::Arc<std::sync::Mutex<World>>;
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Shared>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut m = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(w) = m.get(sample.file) {
        return Ok(w.clone());
    }
    let bytes = std::fs::read(sample.file).map_err(|e| format!("terrain sample {}: {e}", sample.file))?;
    let t = crate::terrain::format::Terrain::decode(&bytes)?;
    let w: Shared = std::sync::Arc::new(std::sync::Mutex::new(crate::terrain::land::build_world(&t)?));
    m.insert(sample.file.to_string(), w.clone());
    Ok(w)
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

/// The glass covers of a build at `offset` (LEGACY_IRON_GOLEM excludes glass: one block a spot,
/// nothing new above it): every natural cell the panicking villagers' golem search could spawn a
/// golem on outside the farm's own floors, covered until none is left. Reads a crop of the world
/// around the farm (the golem searches reach 8 columns and 7 layers from their villager), so it
/// costs a few milliseconds, not a copy of the sample.
pub fn covers_at(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool) -> Vec<BlockPos> {
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
    // covered like any natural spot
    let seals: Vec<(BlockPos, crate::block::Block)> = seals_at(world, spec, offset, clear_box).into_iter().map(|(p, _)| (p, SEAL)).collect();
    w.place_raw(&seals);
    let moved: Vec<(BlockPos, crate::block::Block)> = spec.blocks.iter().map(|(p, b)| (at(*p), *b)).collect();
    w.place_raw(&moved);
    let own: std::collections::HashSet<BlockPos> = moved.iter().filter(|(_, b)| !b.is_air()).map(|(p, _)| *p).collect();
    // the villagers that panic: each sees its nearest zombie as built, or with every trapdoor
    // open, or every trapdoor shut (a hide shows or hides the zombie either way), and their
    // searches' starts
    for (k, p, n) in &spec.entities {
        let _ = crate::entity::spawn(&mut w, k, [p[0] + offset[0] as f64, p[1] + offset[1] as f64, p[2] + offset[2] as f64], n.as_deref());
    }
    let starts = panicking(&w, &moved);
    let mut fills = Vec::new();
    for _ in 0..64 {
        let cells: Vec<BlockPos> = spawn_cells(&w, &starts).into_iter().filter(|c| !own.contains(&c.below())).collect();
        if cells.is_empty() {
            break;
        }
        for c in cells {
            w.set_block(c, crate::block::Block::Simple(crate::block::Simple::Glass), crate::world::flags::RAW);
            fills.push(c);
        }
    }
    fills
}

/// The cells of `w` in `lo..=hi` (clamped to its box) as a world of their own: blocks only (no
/// land, light or entities), for block reads that stay inside it.
pub fn crop(w: &World, lo: BlockPos, hi: BlockPos) -> World {
    let wmax = w.min.offset(w.size[0] - 1, w.size[1] - 1, w.size[2] - 1);
    let lo = BlockPos::new(lo.x.max(w.min.x), lo.y.max(w.min.y), lo.z.max(w.min.z));
    let hi = BlockPos::new(hi.x.min(wmax.x), hi.y.min(wmax.y), hi.z.min(wmax.z));
    let mut c = World::new(lo, [hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1]);
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = BlockPos::new(x, y, z);
                let b = w.get(p);
                if !b.is_air() {
                    c.set_block(p, b, crate::world::flags::RAW);
                }
            }
        }
    }
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
    let mut v: Vec<Priced> = price_spots(w, spec, fp, ground, clear_box, spots).into_iter().map(|price| Priced { covers: covers_at(w, spec, price.offset, clear_box), seals: seals_at(w, spec, price.offset, clear_box), price }).collect();
    v.sort_by(|a, b| a.seconds().total_cmp(&b.seconds()));
    v
}

/// The valid min columns for a footprint near `near` (level ground within 2, no refused natural
/// block: `place::refusal`), the `k` nearest, each with its ground layer.
pub fn valid_spots(w: &World, size: [i32; 2], below: i32, near: [i32; 2], radius: i32, k: usize) -> Vec<([i32; 2], i32)> {
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
            if ground - low > 2 {
                continue;
            }
            out.push(((dx as i64).pow(2) + (dz as i64).pow(2), lo_xz, ground));
        }
    }
    out.sort();
    let mut keep = Vec::new();
    for (_, lo_xz, ground) in out {
        if keep.len() >= k {
            break;
        }
        let lo = BlockPos::new(lo_xz[0], ground - below, lo_xz[1]);
        let hi = BlockPos::new(lo_xz[0] + size[0] - 1, ground, lo_xz[1] + size[1] - 1);
        if place::refusal(w, lo, hi).is_none() {
            keep.push((lo_xz, ground));
        }
    }
    keep
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
    let mut out: Vec<BlockPos> = Vec::new();
    for s in starts {
        for (c, ok) in super::spawn::golem_spawn_cells_from(w, *s) {
            if ok.is_ok() && !out.contains(&c) {
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
    Ok((ground, afk, terrain.map(str::trim) != Some("cells")))
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
        Site::with_shared(std::sync::Arc::new(std::sync::Mutex::new(world)), sample, spec, ground, afk, clear_box)
    }

    /// [`Site::with_world`] on a shared world ([`sample_world_shared`]).
    pub fn with_shared(world: std::sync::Arc<std::sync::Mutex<World>>, sample: &'static Sample, spec: &FarmSpec, ground: i32, afk: [f64; 3], clear_box: bool) -> Result<Site, String> {
        let fp = footprint(spec, ground)?;
        let priced = {
            let w = world.lock().unwrap_or_else(|e| e.into_inner());
            let spots = spots_for(&w, sample, fp);
            if spots.is_empty() {
                return Err(format!("no spot for a {}x{} footprint {} deep in {}", fp.size[0], fp.size[1], fp.below, sample.name));
            }
            price_all(&w, spec, fp, ground, clear_box, &spots)
        };
        Ok(Site::at_priced(world, sample, priced, 0, afk, clear_box))
    }

    /// The site at the `k`-th cheapest of its priced spots (the judge's other builds).
    pub fn at_priced(world: std::sync::Arc<std::sync::Mutex<World>>, sample: &'static Sample, priced: Vec<Priced>, k: usize, afk: [f64; 3], clear_box: bool) -> Site {
        let range = (priced.iter().map(Priced::seconds).fold(f64::INFINITY, f64::min), priced.iter().map(Priced::seconds).fold(0.0, f64::max));
        let chosen = priced[k.min(priced.len() - 1)].clone();
        let offset = chosen.price.offset;
        let afk = Vec3::new(afk[0] + offset[0] as f64, afk[1] + offset[1] as f64, afk[2] + offset[2] as f64);
        Site { sample: sample.name, world, offset, clear_box, dug: chosen.price.dug, placed: chosen.price.placed, afk, fills: chosen.covers, seals: chosen.seals, spots: priced.len(), build_range: range, priced }
    }

    /// The build effort at this site for day 1's rank ([`Effort`]): its blocks placed with the glass
    /// covers, the natural blocks dug, and the design's lava sources and villagers.
    pub fn effort(&self, lava: u32, villagers: u32) -> Effort {
        let dug_seals = self.seals.iter().filter(|(_, dig)| *dig).count() as u32;
        Effort { placed: self.placed + self.fills.len() as u32 + self.seals.len() as u32, dug: self.dug + dug_seals, lava, villagers }
    }

    /// `spec` moved into the site's world.
    pub fn spec(&self, spec: &FarmSpec) -> FarmSpec {
        let [ox, oy, oz] = self.offset;
        let at = |p: BlockPos| BlockPos::new(p.x + ox, p.y + oy, p.z + oz);
        FarmSpec {
            blocks: spec.blocks.iter().map(|(p, b)| (at(*p), *b)).collect(),
            entities: spec.entities.iter().map(|(k, p, n)| (k.clone(), [p[0] + ox as f64, p[1] + oy as f64, p[2] + oz as f64], n.clone())).collect(),
            output: at(spec.output),
            lo: at(spec.lo),
            hi: at(spec.hi),
            block_entities: spec.block_entities.iter().map(|(p, s)| (at(*p), s.clone())).collect(),
        }
    }

    /// The world a run of the moved spec starts from: the sample's, its box cleared for a void
    /// design (the moved spec's blocks then go in like the judge places a design).
    pub fn world_for(&self, moved: &FarmSpec) -> World {
        let mut w = self.world.lock().map(|w| w.clone()).unwrap_or_else(|e| e.into_inner().clone());
        if self.clear_box {
            let (lo, hi) = blocks_box(moved);
            place::clear(&mut w, lo, hi);
        }
        let mut stone: Vec<(BlockPos, crate::block::Block)> = self.seals.iter().map(|(p, _)| (*p, SEAL)).collect();
        stone.extend(self.fills.iter().map(|p| (*p, crate::block::Block::Simple(crate::block::Simple::Glass))));
        w.place_raw(&stone);
        w
    }
}
