//! The exposure checks for natural mobs' unmodeled behaviour (the terrain lane; brief
//! `designs/briefs/terrain-and-spawning.md`, "The checks", channels C1-C16). Natural mobs' walking
//! and targeting is not modeled yet, so these checks report, for each channel, which mob could do
//! what to the farm and from which cells (vjeux: "it's all about tradeoffs": exposures are card
//! lines, never refusals, until natural mobs' AI turns them into measured losses):
//!
//! * a **flood** per body class (the types' boxes and step heights) over the terrain plus the farm,
//!   started at every standable position outside the farm a mob of the class can spawn at (a
//!   monster only at block light 0, whatever the sky; natural spawns, sieges, the village's own
//!   mobs, the box's edges all start somewhere in it); closed trapdoors and fence gates are walls
//!   (zombies break only doors); spiders climb, endermen teleport (every standable position),
//!   flyers take any air;
//! * the channels read the flood: entry into the farm's interior, a villager-hostile's sight of a
//!   farm villager, melee reach, the village golem's and villagers' presence, the golem search
//!   outside the farm's floors, creepers near the AFK player, endermen, fire, lightning, bed
//!   claims, natural spawns inside.
//!
//! Every channel is a superset of what the game can do (a position counts if the mob's box fits
//! anywhere in the cell), so a farm with no exposure cannot be reached by any natural mob.

use crate::block::Block;
use crate::entity::{Aabb, Vec3};
use crate::pos::BlockPos;
use crate::terrain::blocks;
use crate::world::World;
use std::collections::{HashSet, VecDeque};

/// A body class: a box and how it moves. A flood shared by several types (adults and babies, a
/// cube's sizes) moves with the smallest width and the smallest height among them (`width`,
/// `height`: its reachable set is a superset of each type's) and tests reach and proximity with
/// the largest box among them (`reach_width`, `reach_height`) at every position it reaches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub name: &'static str,
    pub width: f64,
    pub height: f64,
    /// Step height (the step-up of a walk) and the reach of a jump (1.25 blocks for every walker).
    pub step: f64,
    pub climbs: bool,
    pub flies: bool,
    /// Opens wooden doors (villagers) or breaks them (zombies on Hard).
    pub doors: bool,
    pub reach_width: f64,
    pub reach_height: f64,
}

const fn body(name: &'static str, width: f64, height: f64, step: f64, climbs: bool, flies: bool, doors: bool) -> Body {
    Body { name, width, height, step, climbs, flies, doors, reach_width: width, reach_height: height }
}

const fn shared(b: Body, reach_width: f64, reach_height: f64) -> Body {
    Body { reach_width, reach_height, ..b }
}

/// The body classes of the natural mobs, the village's villagers and golem (`EntityTypes` sizes;
/// a baby is half its adult's size, `AgeableMob.getAgeScale`).
/// zombie, drowned, husk, zombie villager, witch: all 0.6 x 1.95 (their babies: `BABY_ZOMBIE`)
pub const ZOMBIE: Body = body("zombie", 0.6, 1.95, 0.6, false, false, true);
pub const BABY_ZOMBIE: Body = body("baby zombie", 0.3, 0.975, 0.6, false, false, true);
/// skeleton, stray, bogged: 0.6 x 1.99
pub const SKELETON: Body = body("skeleton", 0.6, 1.99, 0.6, false, false, false);
pub const PILLAGER: Body = body("pillager", 0.6, 1.95, 0.6, false, false, false);
pub const CREEPER: Body = body("creeper", 0.6, 1.7, 0.6, false, false, false);
pub const SPIDER: Body = body("spider", 1.4, 0.9, 0.6, true, false, false);
pub const CAVE_SPIDER: Body = body("cave spider", 0.7, 0.5, 0.6, true, false, false);
pub const ENDERMAN: Body = body("enderman", 0.6, 2.9, 1.0, false, false, false);
/// cubes: moves as a baby size-1 sulfur cube (0.49 x 0.5 = 0.245; slimes are never babies:
/// `Slime.finalizeSpawn`), reaches as a size-4 slime (0.52 x 4 = 2.08)
pub const SLIME: Body = shared(body("slime", 0.245, 0.245, 0.6, false, false, false), 2.08, 2.08);
/// the village's villagers: moves as a baby (0.3 x 0.975), reaches as an adult (0.6 x 1.95)
pub const VILLAGER: Body = shared(body("villager", 0.3, 0.975, 0.6, false, false, true), 0.6, 1.95);
/// foxes: moves as a baby (0.3 x 0.35), reaches as an adult (0.6 x 0.7)
pub const FOX: Body = shared(body("fox", 0.3, 0.35, 0.6, false, false, false), 0.6, 0.7);
pub const GOLEM: Body = body("iron golem", 1.4, 2.7, 1.0, false, false, false);
pub const BAT: Body = body("bat", 0.5, 0.9, 0.0, false, true, false);

/// What the checks need to know about the farm.
#[derive(Clone, Debug, Default)]
pub struct Farm {
    /// The cells its own entities, items and fluids occupy in the runs, and the air connected to
    /// them (the recorded envelope).
    pub interior: Vec<BlockPos>,
    /// Its own blocks (the design's cells).
    pub blocks: Vec<BlockPos>,
    /// Its villagers' boxes over the runs (standing and sleeping).
    pub villagers: Vec<Aabb>,
    /// Its villagers' eye positions over the runs.
    pub villager_eyes: Vec<Vec3>,
    /// The farm's zombie (and its vehicle) boxes.
    pub zombie: Vec<Aabb>,
    /// The cells its golems may spawn at (its own spawn floors' cells above).
    pub golem_cells: Vec<BlockPos>,
    /// Its bed heads.
    pub beds: Vec<BlockPos>,
    /// The AFK player's feet.
    pub afk: Vec3,
    /// The villagers that can summon golems (a villager panics, and so searches for a golem spot,
    /// only when it sees a farm zombie: `VillagerPanicTrigger` -> `spawnGolemIfNeeded`), by their
    /// block position; empty: every villager box counts.
    pub summoners: Vec<BlockPos>,
}

/// One exposure: a channel, the mob, what it can do there, and every cell it can do it from (the
/// mob's cell; for C11 and C16 the farm's own cell), sorted.
#[derive(Clone, Debug, PartialEq)]
pub struct Exposure {
    pub channel: &'static str,
    pub mob: &'static str,
    pub what: &'static str,
    pub cells: Vec<BlockPos>,
}

impl Exposure {
    /// The card line: `C3 sight: a zombie can see a farm villager from 37 cells (e.g. (-333,69,248))`.
    pub fn line(&self) -> String {
        let n = self.cells.len();
        let e = self.cells.first().map(|c| format!(" (e.g. ({},{},{}))", c.x, c.y, c.z)).unwrap_or_default();
        format!("{}: {} {} from {n} cell{}{e}", self.channel, self.mob, self.what, if n == 1 { "" } else { "s" })
    }
}

/// The exposures found so far, by (channel, mob, what).
#[derive(Default)]
struct Exposures(std::collections::BTreeMap<(&'static str, &'static str, &'static str), std::collections::BTreeSet<(i32, i32, i32)>>);

impl Exposures {
    fn add(&mut self, channel: &'static str, mob: &'static str, what: &'static str, c: BlockPos) {
        self.0.entry((channel, mob, what)).or_default().insert((c.x, c.y, c.z));
    }

    fn done(self) -> Vec<Exposure> {
        let mut v: Vec<Exposure> = self
            .0
            .into_iter()
            .map(|((channel, mob, what), cells)| Exposure { channel, mob, what, cells: cells.into_iter().map(|(x, y, z)| BlockPos::new(x, y, z)).collect() })
            .collect();
        // by channel number (C1, C2, ... C16), then as found
        let num = |c: &str| c[1..].split(' ').next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(99);
        v.sort_by_key(|e| num(e.channel));
        v
    }
}

// ------------------------------------------------------------------------------- the boxes

fn is_door(b: &Block) -> bool {
    match b {
        Block::Door { .. } => true,
        Block::Terrain(id) => {
            let p = blocks::props(*id);
            p.classes.iter().any(|c| c == "DoorBlock") && !p.block_id().contains("iron")
        }
        _ => false,
    }
}

/// The collision boxes meeting `bb` (a body that passes doors ignores wooden doors).
fn collides(w: &World, bb: Aabb, doors: bool) -> bool {
    let (x0, x1) = (bb.min_x.floor() as i32 - 1, bb.max_x.floor() as i32 + 1);
    let (y0, y1) = (bb.min_y.floor() as i32 - 1, bb.max_y.floor() as i32 + 1);
    let (z0, z1) = (bb.min_z.floor() as i32 - 1, bb.max_z.floor() as i32 + 1);
    for y in y0..=y1 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                let p = BlockPos::new(x, y, z);
                let b = w.get(p);
                if b.is_air() || (doors && is_door(&b)) {
                    continue;
                }
                if crate::entity::collide::boxes_at(w, &b, p).iter().any(|s| s.intersects(bb)) {
                    return true;
                }
            }
        }
    }
    false
}

/// The feet height a body has in cell `p`: the highest collision top inside the cell's footprint
/// below its middle (a slab, farmland), or the cell's bottom when the cell below holds it up; `None`
/// when nothing holds it (it falls) and the cell is not fluid.
pub fn floor_in(w: &World, p: BlockPos) -> Option<f64> {
    let here = w.get(p);
    let tops: Vec<f64> = crate::entity::collide::boxes_at(w, &here, p).iter().map(|b| b.max_y - p.y as f64).filter(|t| *t <= 0.5).collect();
    if let Some(t) = tops.iter().cloned().reduce(f64::max) {
        return Some(p.y as f64 + t);
    }
    let below = w.get(p.below());
    let held = crate::entity::collide::boxes_at(w, &below, p.below()).iter().any(|b| b.max_y >= p.y as f64 - 1.0e-7);
    if held || fluid(w, p) {
        return Some(p.y as f64);
    }
    None
}

fn fluid(w: &World, p: BlockPos) -> bool {
    let b = w.get(p);
    !crate::fluid::fluid_state(w, p).is_empty() || matches!(b, Block::Lava { .. }) || matches!(b, Block::Terrain(id) if blocks::props(id).fluid != blocks::FluidKind::Empty)
}

/// Whether a body fits in cell `p` with its feet at `feet` (anywhere in the cell: the centre and
/// the four corners).
pub fn fits(w: &World, b: &Body, p: BlockPos, feet: f64) -> bool {
    let hw = b.width / 2.0;
    let slack = (0.5 - hw).max(0.0);
    let (cx, cz) = (p.x as f64 + 0.5, p.z as f64 + 0.5);
    for (dx, dz) in [(0.0, 0.0), (-slack, -slack), (slack, -slack), (-slack, slack), (slack, slack)] {
        let bb = Aabb::new(cx + dx - hw, feet + 1.0e-4, cz + dz - hw, cx + dx + hw, feet + b.height, cz + dz + hw);
        if !collides(w, bb, b.doors) {
            return true;
        }
    }
    false
}

/// A body's box at a position (centred).
pub fn box_at(b: &Body, p: BlockPos, feet: f64) -> Aabb {
    let hw = b.width / 2.0;
    Aabb::new(p.x as f64 + 0.5 - hw, feet, p.z as f64 + 0.5 - hw, p.x as f64 + 0.5 + hw, feet + b.height, p.z as f64 + 0.5 + hw)
}

/// The box covering every placement of a body in a cell, at its largest size (for reach tests:
/// the superset).
pub fn reach_box(b: &Body, p: BlockPos, feet: f64) -> Aabb {
    let hw = b.reach_width / 2.0;
    let slack = (0.5 - hw).max(0.0);
    Aabb::new(p.x as f64 + 0.5 - hw - slack, feet, p.z as f64 + 0.5 - hw - slack, p.x as f64 + 0.5 + hw + slack, feet + b.reach_height, p.z as f64 + 0.5 + hw + slack)
}

// ------------------------------------------------------------------------------- the flood

/// The positions a body can take (cell, feet height).
pub struct Reach {
    pub body: Body,
    pub cells: Vec<(BlockPos, f64)>,
    seen: HashSet<BlockPos>,
}

impl Reach {
    pub fn contains(&self, p: BlockPos) -> bool {
        self.seen.contains(&p)
    }
}

/// How a flood picks its starts. `Worst`: the bounds' outer ring, every position a natural type of
/// the body can spawn at whatever the light (block light 0 for monsters), every position for
/// endermen outside `exclude`, then their teleport reach (into `exclude` too: see [`flood`]). `SpawnRules` (ironspec and G1 2026-11-11, vjeux's normal-operation
/// scope: a source is a cell where the game can spawn the mob): the ring as before (anything from
/// beyond walks in across it); every natural start with the type's spawn box free at the cell's
/// bottom; monsters at block light 0 on their spawn floor; endermen the same, then every spot
/// their teleport reaches (32 sideways, -32..+31 vertically, onto a motion-blocking floor with the
/// box free); a trap skeleton on every strike cell at any light (today's tops and the caller's
/// lasting ones), unless a lightning rod topping its column within 128 (a sphere) takes the
/// strike; patrol pillagers only on the surface (`PatrolSpawner`: the MOTION_BLOCKING_NO_LEAVES
/// top, block light 8 or less); slimes in slime chunks below y 40 (`Slime.checkSlimeSpawnRules`;
/// the world seed's chunks) and in swamps at y 51 to 69 at night light 7 or less (where the biome
/// isn't known, there too); bats under the world surface at night light 6 or less
/// (`Bat.checkBatSpawnRules`: Halloween's limit, counted whatever the date); foxes at light 9 or
/// more (`Animal`'s rule on their spawn floor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seeding {
    Worst,
    SpawnRules { seed: Option<i64> },
}

/// Flood a body over the world from every standable position outside `exclude` (the farm's
/// interior): walks (step or a 1.25 jump up, a step down past an open cell beside it, any fall
/// down an open column, stopping at a block; the side cell's head, a block up, isn't checked for a
/// step down or a fall: an over-reach the worst case keeps), swims, climbs (spiders), flies (any
/// air), and for endermen teleports (32 sideways, -32..+31 vertically, onto a motion-blocking floor
/// with the box free). Positions inside `exclude` are reached too (that is the entry channel).
pub fn flood(w: &World, b: Body, exclude: &HashSet<BlockPos>, bounds: (BlockPos, BlockPos)) -> Reach {
    flood_with(w, b, exclude, bounds, Seeding::Worst)
}

/// [`flood`] with its starts picked by `seeding` ([`Seeding`]).
pub fn flood_with(w: &World, b: Body, exclude: &HashSet<BlockPos>, bounds: (BlockPos, BlockPos), seeding: Seeding) -> Reach {
    flood_from(w, b, exclude, bounds, seeding, &[])
}

/// [`flood_with`] with `extra` starts besides the seeding's own (site-check's lasting strike cells
/// for the skeleton body, the trap's: dropped when a rod captures their strike).
pub fn flood_from(w: &World, b: Body, exclude: &HashSet<BlockPos>, bounds: (BlockPos, BlockPos), seeding: Seeding, extra: &[BlockPos]) -> Reach {
    let mut seen: HashSet<BlockPos> = HashSet::new();
    let mut cells = Vec::new();
    let mut queue = VecDeque::new();
    let (lo, hi) = bounds;
    let lo = BlockPos::new(lo.x.max(w.min.x), lo.y.max(w.min.y + 1), lo.z.max(w.min.z));
    let hi = BlockPos::new(hi.x.min(w.min.x + w.size[0] - 1), hi.y.min(w.min.y + w.size[1] - 1), hi.z.min(w.min.z + w.size[2] - 1));
    let inside = |p: BlockPos| p.x >= lo.x && p.y >= lo.y && p.z >= lo.z && p.x <= hi.x && p.y <= hi.y && p.z <= hi.z;
    let pos_of = |p: BlockPos| -> Option<f64> {
        if b.flies {
            if fits(w, &b, p, p.y as f64) { Some(p.y as f64) } else { None }
        } else {
            let f = floor_in(w, p)?;
            if fits(w, &b, p, f) { Some(f) } else { None }
        }
    };
    // the starts: where a mob of the body can appear inside the bounds — the bounds' outer ring
    // (anything from beyond walks in across it), every position a natural type of the body can
    // spawn at whatever the light (its spawn floor below: `isValidSpawn` for that type), the
    // sample's own villagers and golems for those bodies, every position for endermen (they
    // teleport) and flyers' air below the surface
    let ring = |p: BlockPos| p.x == lo.x || p.x == hi.x || p.z == lo.z || p.z == hi.z || p.y == lo.y || p.y == hi.y;
    let types: &[&str] = match b.name {
        "zombie" | "baby zombie" => &["minecraft:zombie", "minecraft:zombie_villager", "minecraft:drowned", "minecraft:husk", "minecraft:witch"],
        "skeleton" => &["minecraft:skeleton", "minecraft:stray", "minecraft:bogged"],
        "creeper" => &["minecraft:creeper"],
        "spider" | "cave spider" => &["minecraft:spider", "minecraft:cave_spider"],
        "slime" => &["minecraft:slime", "minecraft:sulfur_cube"],
        "bat" => &["minecraft:bat"],
        "pillager" => &["minecraft:pillager"],
        "fox" => &["minecraft:fox"],
        "enderman" => &["minecraft:enderman"],
        _ => &[],
    };
    let residents: Vec<BlockPos> = w
        .entities
        .list
        .iter()
        .filter(|e| match &e.kind {
            crate::entity::EntityKind::Mob(m) => match b.name {
                "villager" => matches!(m.ty, super::MobType::Villager),
                "iron golem" => matches!(m.ty, super::MobType::IronGolem),
                _ => false,
            },
            _ => false,
        })
        .map(|e| e.block_position())
        .collect();
    let rules = matches!(seeding, Seeding::SpawnRules { .. });
    let world_seed = match seeding {
        Seeding::SpawnRules { seed } => seed,
        Seeding::Worst => None,
    };
    // (spawn rules, the trap's starts: a lightning rod topping its column takes every strike
    // within 128, `ServerLevel.findLightningRod`, and a strike on a rod spawns no trap horse,
    // `tickChunk`; ironspec and G1 2026-11-11)
    let rods: Vec<BlockPos> = if rules && b.name == "skeleton" {
        let mut v = Vec::new();
        for z in w.min.z..w.min.z + w.size[2] {
            for x in w.min.x..w.min.x + w.size[0] {
                let top = BlockPos::new(x, w.height(crate::terrain::Heightmap::WorldSurface, x, z) - 1, z);
                if w.contains(top) && w.get(top).to_string().contains("lightning_rod") {
                    v.push(top);
                }
            }
        }
        v
    } else {
        Vec::new()
    };
    // (the rod's reach is a sphere, `PoiManager.findClosest`'s `distSqr(pos) <= 128 * 128` from the
    // strike's cell: G1 2026-11-11)
    let captured = |s: BlockPos| {
        rods.iter().any(|r| {
            let (dx, dy, dz) = ((r.x - s.x) as i64, (r.y - s.y) as i64, (r.z - s.z) as i64);
            dx * dx + dy * dy + dz * dz <= 128 * 128
        })
    };
    // (the light a spawn roll reads at night: `getMaxLocalRawBrightness`, the sky darkened by 11)
    let night = |p: BlockPos| w.light_or(w.block_light(p), 0).max(w.light_or(w.sky_light(p), 15) - 11);
    let may_start = |p: BlockPos| -> bool {
        if ring(p) || (b.name == "enderman" && !rules) {
            return true;
        }
        if !residents.is_empty() || matches!(b.name, "villager" | "iron golem") {
            return residents.contains(&p);
        }
        if b.flies && p.y >= w.height(crate::terrain::Heightmap::WorldSurface, p.x, p.z) {
            return false;
        }
        // (spawn rules: a trap's skeleton horse appears where the strike lands, on any floor at
        // any light, unless a rod takes the strike: G1 and ironspec 2026-11-11)
        if rules && b.name == "skeleton" && p.y == w.height(crate::terrain::Heightmap::MotionBlocking, p.x, p.z) && !captured(p) {
            return true;
        }
        // (a monster spawns in the overworld only at block light 0: `Monster.isDarkEnoughToSpawn`
        // against the dimension's monster_spawn_block_light_limit, 0; a torch-lit cell is no
        // spawn start, ironspec's fix for a lit built-in access. Slimes, pillagers, bats and foxes
        // keep their own rules: any light here)
        let monster = matches!(b.name, "zombie" | "baby zombie" | "skeleton" | "creeper" | "spider" | "cave spider") || (rules && b.name == "enderman");
        if monster && w.light_or(w.block_light(p), 0) > 0 {
            return false;
        }
        if rules {
            // (the spawn box stands at the cell's bottom, `NaturalSpawner`'s `noCollision` test on
            // the type's spawn box: no spawn in a cell a slab or a carpet holds)
            if !fits(w, &b, p, p.y as f64) {
                return false;
            }
            let raw = || w.light_or(w.block_light(p), 0).max(w.light_or(w.sky_light(p), 15));
            let ok = match b.name {
                // a patrol's pillager: on the surface only, block light 8 or less
                "pillager" => p.y == w.height(crate::terrain::Heightmap::MotionBlockingNoLeaves, p.x, p.z) && w.light_or(w.block_light(p), 0) <= 8,
                // a slime: in a slime chunk below y 40 at any light; or in a swamp at y 51 to 69 at
                // night light 7 or less (`#allows_surface_slime_spawns`, the moon's roll passing;
                // where the biome isn't known, there too)
                "slime" => (p.y < 40 && world_seed.is_some_and(|s| super::natural::slime_chunk(p.x >> 4, p.z >> 4, s))) || ((51..=69).contains(&p.y) && night(p) <= 7 && w.biome_at(p).is_none_or(|n| n.contains("swamp"))),
                // a bat: under the world surface (above: refused before), at night light 6 or
                // less (`Bat.checkBatSpawnRules`, 26.3: not sea level; its limit 3, and 6 from
                // Oct 20 to Nov 3, `isHalloween`, counted whatever the date: G1 2026-11-11)
                "bat" => night(p) <= 6,
                // a fox: at light 9 or more
                "fox" => raw() > 8,
                _ => true,
            };
            if !ok {
                return false;
            }
        }
        types.iter().any(|t| valid_spawn_floor(w, p.below(), t))
    };
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = BlockPos::new(x, y, z);
                if exclude.contains(&p) || !may_start(p) {
                    continue;
                }
                if let Some(f) = pos_of(p) {
                    seen.insert(p);
                    cells.push((p, f));
                    queue.push_back((p, f));
                }
            }
        }
    }
    // the extra starts (site-check's lasting strike cells), a rod's capture dropping them
    for &p in extra {
        if !inside(p) || exclude.contains(&p) || seen.contains(&p) || (rules && captured(p)) {
            continue;
        }
        if let Some(f) = pos_of(p) {
            seen.insert(p);
            cells.push((p, f));
            queue.push_back((p, f));
        }
    }
    let dirs: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
    loop {
    while let Some((p, f)) = queue.pop_front() {
        let mut next: Vec<BlockPos> = Vec::new();
        if b.flies {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        if (dx, dy, dz) != (0, 0, 0) {
                            next.push(p.offset(dx, dy, dz));
                        }
                    }
                }
            }
        } else {
            for (dx, dz) in dirs {
                // a diagonal needs both sides open
                if dx != 0 && dz != 0 && !(pos_of(p.offset(dx, 0, 0)).is_some() || pos_of(p.offset(0, 0, dz)).is_some()) {
                    continue;
                }
                // (a step down or a fall passes the cell beside the mob, which must be open, and a
                // fall stops at a block: the fall went straight down the neighbour column from one
                // below, through any block with air under it, so a mob on a one-block roof reached
                // the closed room under it; the parent and ironspec 2026-11-11: fixed in both modes)
                let side = p.offset(dx, 0, dz);
                let side_open = !crate::behavior::collides(&w.get(side));
                for dy in -1..=1 {
                    if dy == -1 && !side_open {
                        continue;
                    }
                    next.push(p.offset(dx, dy, dz));
                }
                // a fall: off the edge into the open column beside, straight down through open
                // cells to the first floor
                if side_open && floor_in(w, side).is_none() {
                    let mut q = side.below();
                    let mut n = 0;
                    while q.y > w.min.y && floor_in(w, q).is_none() && !crate::behavior::collides(&w.get(q)) && n < 384 {
                        q = q.below();
                        n += 1;
                    }
                    next.push(q);
                }
            }
            // swimming and climbing: straight up and down
            if fluid(w, p) || (b.climbs && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dz)| crate::behavior::collides(&w.get(p.offset(*dx, 0, *dz))))) {
                next.push(p.above());
            }
            next.push(p.below());
        }
        for q in next {
            if seen.contains(&q) || !w.contains(q) || !inside(q) {
                continue;
            }
            let Some(g) = pos_of(q) else { continue };
            // up by at most a jump (1.25) unless swimming or climbing
            if !b.flies && g - f > 1.25 + 1.0e-6 && !fluid(w, p) && !b.climbs {
                continue;
            }
            seen.insert(q);
            cells.push((q, g));
            queue.push_back((q, g));
        }
    }
    // (an enderman teleports, `EnderMan.teleport` / `LivingEntity.randomTeleport`, to a random
    // spot within 32 on each horizontal axis and -32..+31 vertically from any spot it reaches,
    // dropping to the first motion-blocking non-fluid floor below and landing only where its box is
    // free of blocks and fluid, light and floor type aside; its landings walk on, until no landing
    // is new: G1 and ironspec 2026-11-11, the brief's teleport reach; in both seedings, the worst
    // case seeding no spot inside `exclude` and so reaching the farm's space by teleport alone:
    // G1 and ironspec 2026-11-12)
    if b.name != "enderman" {
        break;
    }
    // the reached spots' y range per column, then its min and max over the 65-wide window of
    // columns round each (x first, then z)
    let (nx, nz) = ((hi.x - lo.x + 1) as usize, (hi.z - lo.z + 1) as usize);
    let mut mn = vec![i32::MAX; nx * nz];
    let mut mx = vec![i32::MIN; nx * nz];
    for (p, _) in &cells {
        let i = (p.x - lo.x) as usize * nz + (p.z - lo.z) as usize;
        mn[i] = mn[i].min(p.y);
        mx[i] = mx[i].max(p.y);
    }
    let window = |a: &[i32], low: bool| -> Vec<i32> {
        let pick = |v: i32, u: i32| if low { v.min(u) } else { v.max(u) };
        let none = if low { i32::MAX } else { i32::MIN };
        let mut along = vec![none; nx * nz];
        for xi in 0..nx {
            for zi in 0..nz {
                along[xi * nz + zi] = (xi.saturating_sub(32)..=(xi + 32).min(nx - 1)).fold(none, |v, xj| pick(v, a[xj * nz + zi]));
            }
        }
        let mut out = vec![none; nx * nz];
        for xi in 0..nx {
            for zi in 0..nz {
                out[xi * nz + zi] = (zi.saturating_sub(32)..=(zi + 32).min(nz - 1)).fold(none, |v, zj| pick(v, along[xi * nz + zj]));
            }
        }
        out
    };
    let (wmn, wmx) = (window(&mn, true), window(&mx, false));
    let mut landed = Vec::new();
    for z in lo.z..=hi.z {
        for x in lo.x..=hi.x {
            let i = (x - lo.x) as usize * nz + (z - lo.z) as usize;
            if wmn[i] == i32::MAX {
                continue;
            }
            let (ymin, ymax) = (wmn[i] - 32, wmx[i] + 31);
            // every floor in the band, then the first floor under it
            let mut y = ymax.min(hi.y);
            while y >= lo.y {
                let q = BlockPos::new(x, y, z);
                let floor = crate::behavior::collides(&w.get(q.below())) && !fluid(w, q.below());
                if floor {
                    let dry = !fluid(w, q) && !fluid(w, q.above()) && !fluid(w, q.offset(0, 2, 0));
                    if dry
                        && !seen.contains(&q)
                        && let Some(g) = pos_of(q)
                    {
                        landed.push((q, g));
                    }
                    if y <= ymin {
                        break;
                    }
                }
                y -= 1;
            }
        }
    }
    if landed.is_empty() {
        break;
    }
    for (q, g) in landed {
        if seen.insert(q) {
            cells.push((q, g));
            queue.push_back((q, g));
        }
    }
    }
    Reach { body: b, cells, seen }
}

// ------------------------------------------------------------------------------ the channels

fn dist2(a: Vec3, b: Vec3) -> f64 {
    (a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y) + (a.z - b.z) * (a.z - b.z)
}

/// Run every channel; returns the exposures (empty: no natural mob can do anything to the farm).
/// Nothing here refuses a design: each exposure is a card line (which mob can do what, from which
/// cells) until natural mobs' AI is modeled and the losses can be measured.
///
/// C1's test at one cell, [`can_spawn_at`]: `mctas mobfarm spawn-rules` checks it against the
/// game's own `SpawnPlacements` at every interior cell of a farm built in a terrain sample.
///
/// An error when a light read the channels need failed (cc767b78, ironspec 12:54: no light read is
/// silently defaulted; `World::light_or` records them, taken here).
pub fn check(w: &World, farm: &Farm, village_golem: bool) -> Result<Vec<Exposure>, String> {
    let mut ex = Exposures::default();
    let interior: HashSet<BlockPos> = farm.interior.iter().copied().collect();
    // C1: a natural spawn inside: any type listed at the cell (structure overrides included) whose
    // placement passes and whose spawn rule can pass at all (`natural::rules_can_pass`: every
    // static part exact — floor tags, biome, heights, the slime chunk — the draws at their best and
    // the light at its worst for the rule)
    use super::natural::{Category, mobs_at};
    for &p in &farm.interior {
        for cat in [Category::Monster, Category::Creature, Category::Ambient] {
            for s in mobs_at(w, cat, p) {
                let Some(k) = super::natural::kind_of(&s.ty) else { continue };
                if can_spawn_at(w, &k, p) {
                    ex.add("C1 natural spawn inside", k.id.trim_start_matches("minecraft:"), "can spawn inside the farm", p);
                }
            }
        }
    }
    // the floods, over the farm's surroundings (24 columns around its blocks and villagers, the
    // whole height)
    let mut lo = BlockPos::new(i32::MAX, i32::MAX, i32::MAX);
    let mut hi = BlockPos::new(i32::MIN, i32::MIN, i32::MIN);
    for p in farm.blocks.iter().copied().chain(farm.villagers.iter().map(|v| BlockPos::new(v.min_x.floor() as i32, v.min_y.floor() as i32, v.min_z.floor() as i32))) {
        lo = BlockPos::new(lo.x.min(p.x - 24), lo.y.min(p.y - 24), lo.z.min(p.z - 24));
        hi = BlockPos::new(hi.x.max(p.x + 24), hi.y.max(p.y + 24), hi.z.max(p.z + 24));
    }
    let bounds = (lo, hi);
    let walkers = [ZOMBIE, BABY_ZOMBIE, SKELETON, PILLAGER, CREEPER, SPIDER, CAVE_SPIDER, SLIME, FOX];
    let mut reaches: Vec<Reach> = walkers.iter().map(|b| flood(w, *b, &interior, bounds)).collect();
    reaches.push(flood(w, BAT, &interior, bounds));
    // C2: entry into the interior (and contact with the farm's villagers)
    for r in &reaches {
        for (p, _) in r.cells.iter().filter(|(p, _)| interior.contains(p)) {
            ex.add("C2 entry", r.body.name, "can get into the farm", *p);
        }
    }
    // C3: a villager hostile's sight (`VillagerHostilesSensor`: the zombie family within 8 blocks
    // between feet, a pillager (patrols, outposts) within 15, its own body; a clear COLLIDER ray
    // between eyes, `hasLineOfSight`); the mob anywhere in its cell
    let villager_feet: Vec<Vec3> = farm.villagers.iter().map(|v| Vec3::new(v.center().x, v.min_y, v.center().z)).collect();
    for r in &reaches {
        let range = match r.body.name {
            "zombie" | "baby zombie" => 8.0,
            "pillager" => 15.0,
            _ => continue,
        };
        let who = r.body.name;
        for (p, f) in &r.cells {
            let feet = Vec3::new(p.x as f64 + 0.5, *f, p.z as f64 + 0.5);
            let eye = Vec3::new(feet.x, feet.y + r.body.height * 0.85, feet.z);
            if villager_feet.iter().zip(farm.villager_eyes.iter()).any(|(vf, ve)| dist2(feet, *vf).sqrt() <= range + 0.71 && super::los::has_line_of_sight(w, eye, *ve)) {
                ex.add("C3 sight", who, "can see a farm villager (it panics, its sleep breaks)", *p);
            }
        }
    }
    // C5: melee reach (the attack box: the mob's box inflated by sqrt(2.04) - 0.6 horizontally).
    // Villagers: the zombie family only (`Zombie` targets `AbstractVillager`; `Spider` targets
    // players and iron golems only). The farm's golems (a box at each golem cell): any hostile but a
    // creeper (`IronGolem` targets every `Enemy` but creepers; zombies and spiders target golems)
    let reach = 2.04f64.sqrt() - 0.6;
    let golem_boxes: Vec<Aabb> = farm.golem_cells.iter().map(|c| reach_box(&GOLEM, *c, c.y as f64)).collect();
    for r in &reaches {
        let villagers = matches!(r.body.name, "zombie" | "baby zombie");
        let golems = !matches!(r.body.name, "creeper" | "bat" | "fox");
        if !villagers && !golems {
            continue;
        }
        for (p, f) in &r.cells {
            let mb = reach_box(&r.body, *p, *f);
            let ab = mb.inflate(reach, 0.0, reach);
            if villagers && farm.villagers.iter().any(|v| v.intersects(ab)) {
                ex.add("C5 reach", r.body.name, "can hit a farm villager", *p);
            }
            if golems && golem_boxes.iter().any(|g| g.intersects(ab) || g.inflate(reach, 0.0, reach).intersects(mb)) {
                ex.add("C5 reach", r.body.name, "and a farm golem can hit each other", *p);
            }
        }
    }
    // C4 + C5: the village golem near the farm villagers (their golem sensor) or reaching the zombie
    if village_golem {
        let g = flood(w, GOLEM, &interior, bounds);
        for (p, f) in &g.cells {
            let gb = reach_box(&GOLEM, *p, *f);
            // `GolemSensor` reads `NearestLivingEntitySensor`: the villager's box inflated by its
            // follow range (16 plus `finalizeSpawn`'s bonus: up to 17.84) on all three axes, no sight
            let r = 17.84;
            if farm.villagers.iter().any(|v| gb.intersects(v.inflate(r, r, r))) {
                ex.add("C4 presence", "the village golem", "can stand within the farm villagers' golem sensor (they stop spawning golems)", *p);
            }
            let ab = gb.inflate(reach, 0.0, reach);
            if farm.zombie.iter().any(|z| z.intersects(ab)) {
                ex.add("C5 reach", "the village golem", "can hit the farm's zombie", *p);
            }
        }
    }
    // C7: the golem search from each summoning farm villager finds a spot outside the farm's floors
    let floors: HashSet<BlockPos> = farm.golem_cells.iter().copied().collect();
    let mut starts: Vec<BlockPos> = if farm.summoners.is_empty() {
        farm.villagers.iter().map(|v| BlockPos::new(v.center().x.floor() as i32, v.min_y.floor() as i32, v.center().z.floor() as i32)).collect()
    } else {
        farm.summoners.clone()
    };
    starts.sort_by_key(|p| (p.x, p.y, p.z));
    starts.dedup();
    for s in &starts {
        for dz in -8..=8 {
            for dx in -8..=8 {
                if let Some(c) = super::spawn::possible_spawn_position(w, s.offset(dx, 6, dz), 6)
                    && !floors.contains(&c)
                {
                    let pos = Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5);
                    if super::spawn::check_spawn_obstruction(w, c, pos) {
                        ex.add("C7 golem search", "a farm golem", "can spawn outside the farm's spawn floors", c);
                    }
                }
            }
        }
    }
    // C8: creepers near the AFK player (they explode at a player: a position within 7 of the AFK
    // spot, no ray test) whose explosion reaches the farm: a farm block within 10.4 (the blocks a
    // power-6 blast can break at the worst factor 1.3) or a farm entity within 12 (its entity
    // radius: damage and knockback, exposure taken as 1)
    let farm_entities: Vec<Vec3> = farm
        .villagers
        .iter()
        .chain(farm.zombie.iter())
        .map(|b| b.center())
        .chain(farm.golem_cells.iter().map(|c| Vec3::new(c.x as f64 + 0.5, c.y as f64 + 1.35, c.z as f64 + 0.5)))
        .collect();
    if let Some(cr) = reaches.iter().find(|r| r.body.name == "creeper") {
        for (p, f) in &cr.cells {
            let c = Vec3::new(p.x as f64 + 0.5, *f, p.z as f64 + 0.5);
            if dist2(c, farm.afk) > 49.0 {
                continue;
            }
            if farm.blocks.iter().any(|b| dist2(Vec3::new(b.x as f64 + 0.5, b.y as f64 + 0.5, b.z as f64 + 0.5), c) <= 10.4 * 10.4) {
                ex.add("C8 creepers", "a creeper", "near the AFK player can blow up farm blocks", *p);
            }
            if farm_entities.iter().any(|e| dist2(*e, c) <= 144.0) {
                ex.add("C8 creepers", "a creeper", "near the AFK player can hurt a farm villager, zombie or golem (within 12)", *p);
            }
        }
    }
    // C9: endermen (every standable position: they teleport) take a holdable farm block (x and z
    // within 2, 0..3 above: `EndermanTakeBlockGoal`'s box) or place a carried block into a farm
    // air cell (x and z within 1, 0..2 above: `EndermanLeaveBlockGoal`'s box); supersets: no
    // OUTLINE ray, no test of the block under the placed one
    let holdable: Vec<BlockPos> = farm.blocks.iter().copied().filter(|b| holdable(w, *b)).collect();
    let farm_air: Vec<BlockPos> = farm.interior.iter().copied().filter(|c| w.get(*c).is_air()).collect();
    if !holdable.is_empty() || !farm_air.is_empty() {
        let e = flood(w, ENDERMAN, &interior, bounds);
        for (p, _) in &e.cells {
            if holdable.iter().any(|b| (b.x - p.x).abs() <= 2 && (b.z - p.z).abs() <= 2 && b.y - p.y >= 0 && b.y - p.y <= 3) {
                ex.add("C9 endermen", "an enderman", "can take a farm block", *p);
            }
            if farm_air.iter().any(|a| (a.x - p.x).abs() <= 1 && (a.z - p.z).abs() <= 1 && a.y - p.y >= 0 && a.y - p.y <= 2) {
                ex.add("C9 endermen", "an enderman", "can place a block into a farm air cell", *p);
            }
        }
    }
    // C6 pickup: a looting mob whose pickup box (its box inflated by 1, 0, 1:
    // `Mob.aiStep`'s `getItemPickUpReach`) meets a farm interior cell, where the farm's items can
    // be — the zombie family, skeletons, pillagers, the village's villagers and foxes (each its own
    // flood; the shared ones move as their smallest member and reach as their largest); a superset
    // until the farm spec carries its items' cells
    let interior_boxes: Vec<Aabb> = farm.interior.iter().map(|c| Aabb::of_block(*c)).collect();
    let mut looters: Vec<(&Reach, &'static str)> = reaches.iter().filter_map(|r| match r.body.name {
        "zombie" => Some((r, "zombie")),
        "baby zombie" => Some((r, "baby zombie")),
        "skeleton" => Some((r, "skeleton")),
        "pillager" => Some((r, "pillager")),
        "fox" => Some((r, "fox")),
        _ => None,
    }).collect();
    let villager_flood = if !farm.beds.is_empty() || !farm.villagers.is_empty() { Some(flood(w, VILLAGER, &interior, bounds)) } else { None };
    if let Some(v) = &villager_flood {
        looters.push((v, "village villager"));
    }
    for (r, who) in looters {
        for (p, f) in &r.cells {
            if interior.contains(p) {
                continue;
            }
            let pb = reach_box(&r.body, *p, *f).inflate(1.0, 0.0, 1.0);
            if interior_boxes.iter().any(|c| c.intersects(pb)) {
                ex.add("C6 pickup", who, "can pick up an item inside the farm", *p);
            }
        }
    }
    // C10: flyers inside (bats: the bat flood above, reported by C2); phantoms: they spawn over a
    // player who has not slept for 3 days (`PhantomSpawner`) when the player's cell sees the sky,
    // and the AFK player never sleeps
    let afk_cell = BlockPos::new(farm.afk.x.floor() as i32, farm.afk.y.floor() as i32, farm.afk.z.floor() as i32);
    if afk_cell.y >= 63 && w.sees_or(w.can_see_sky(afk_cell), false) {
        ex.add("C10 flyers", "a phantom", "spawns over the AFK player (no sleep for 3 days; the AFK cell sees the sky)", afk_cell);
    }
    // C5 ranged: a skeleton shoots an iron golem (`AbstractSkeleton` targets players, iron golems
    // and turtles; range 15), a pillager shoots a villager (its crossbow, 8), a drowned throws a
    // trident at a villager (10; drowned share the zombie's body): a clear COLLIDER ray from its eye
    let golem_eyes: Vec<Vec3> = farm.golem_cells.iter().map(|c| Vec3::new(c.x as f64 + 0.5, c.y as f64 + 2.295, c.z as f64 + 0.5)).collect();
    for r in &reaches {
        let (mob, targets, range, what): (&'static str, &Vec<Vec3>, f64, &'static str) = match r.body.name {
            "skeleton" => ("skeleton", &golem_eyes, 15.0, "can shoot a farm golem"),
            "pillager" => ("pillager", &farm.villager_eyes, 8.0, "can shoot a farm villager"),
            "zombie" => ("drowned", &farm.villager_eyes, 10.0, "can throw a trident at a farm villager (from water)"),
            _ => continue,
        };
        for (p, f) in &r.cells {
            let eye = Vec3::new(p.x as f64 + 0.5, *f + r.body.height * 0.85, p.z as f64 + 0.5);
            if targets.iter().any(|t| dist2(eye, *t) <= range * range && super::los::has_line_of_sight(w, eye, *t)) {
                ex.add("C5 ranged", mob, what, *p);
            }
        }
    }
    // C12 sculk: a sculk sensor of the design within 8 of a mob position (steps are vibrations)
    let sensors: Vec<BlockPos> = farm.blocks.iter().copied().filter(|b| w.get(*b).to_string().contains("sculk_sensor")).collect();
    if !sensors.is_empty() {
        for r in &reaches {
            for (p, _) in &r.cells {
                if sensors.iter().any(|s| (s.x - p.x).pow(2) + (s.y - p.y).pow(2) + (s.z - p.z).pow(2) <= 64) {
                    ex.add("C12 sculk", r.body.name, "can make a farm sculk sensor fire (its steps)", *p);
                }
            }
        }
    }
    // C4 presence (villagers): a village villager within 10 of a farm villager's box joins their
    // golem count (`Villager.spawnGolemIfNeeded` counts the villagers near the one that spawns)
    if let Some(v) = &villager_flood {
        for (p, f) in v.cells.iter().filter(|(p, _)| !interior.contains(p)) {
            let vb = reach_box(&VILLAGER, *p, *f);
            if farm.villagers.iter().any(|fv| vb.intersects(fv.inflate(10.0, 10.0, 10.0))) {
                ex.add("C4 presence", "a village villager", "can come within 10 of a farm villager (it joins their golem count)", *p);
            }
        }
    }
    // C11: fire: a flammable farm block that lightning can reach (its top sees the sky)
    for b in &farm.blocks {
        let st = w.get(*b);
        let flammable = match st {
            Block::Terrain(id) => blocks::props(id).burn_odds > 0,
            _ => blocks::id_of(&st.to_string()).is_some_and(|id| blocks::props(id).burn_odds > 0),
        };
        if flammable && w.sees_or(w.can_see_sky(b.above()), false) {
            ex.add("C11 fire", "lightning", "can set fire to a flammable farm block open to the sky", *b);
        }
    }
    // C11: lava's fire (`LavaFluid.randomTick`, `doFireTick` on): up to 3 steps up (each +1 y and
    // -1..1 in x and z) through cells that do not block motion, the first air cell with an
    // `ignitedByLava` neighbour catching fire; or, with no step, a cell of the 3 x 3 at the lava's
    // level that is `ignitedByLava` with air above it, the fire above. Every lava cell within 4 of
    // the farm counts (the farm's own blades and the terrain's pools)
    let lava_flammable = |p: BlockPos| -> bool {
        let b = w.get(p);
        match b {
            Block::Terrain(id) => blocks::props(id).ignited_by_lava,
            Block::Air => false,
            _ => blocks::id_of(&b.to_string()).is_some_and(|id| blocks::props(id).ignited_by_lava),
        }
    };
    let near_farm = |p: BlockPos| farm.blocks.iter().any(|b| (b.x - p.x).abs() <= 4 && (b.y - p.y).abs() <= 4 && (b.z - p.z).abs() <= 4);
    let is_lava = |p: BlockPos| match w.get(p) {
        Block::Lava { .. } => true,
        Block::Terrain(id) => blocks::props(id).fluid == blocks::FluidKind::Lava,
        _ => false,
    };
    let farm_set: HashSet<BlockPos> = farm.blocks.iter().copied().collect();
    let mut lava_cells: Vec<BlockPos> = Vec::new();
    for b in &farm.blocks {
        for dy in -4..=4 {
            for dz in -4..=4 {
                for dx in -4..=4 {
                    let p = b.offset(dx, dy, dz);
                    if w.contains(p) && is_lava(p) {
                        lava_cells.push(p);
                    }
                }
            }
        }
    }
    lava_cells.sort_by_key(|p| (p.x, p.y, p.z));
    lava_cells.dedup();
    for l in lava_cells.iter().copied().filter(|l| near_farm(*l)) {
        // the walk up: every path of up to 3 steps
        let mut layer = vec![l];
        for _ in 0..3 {
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
                            if let Some(n) = crate::pos::Dir::ALL.iter().map(|d| t.rel(*d)).find(|n| lava_flammable(*n) && (farm_set.contains(n) || interior.contains(&t))) {
                                let _ = n;
                                ex.add("C11 fire", "lava", "can light a fire beside a flammable block of the farm", t);
                            }
                        } else if !crate::behavior::collides(&tb) {
                            next.push(t);
                        }
                    }
                }
            }
            layer = next;
        }
        // no step: the 3 x 3 at the lava's level
        for dz in -1..=1 {
            for dx in -1..=1 {
                let t = l.offset(dx, 0, dz);
                if w.contains(t.above()) && w.get(t.above()).is_air() && lava_flammable(t) && (farm_set.contains(&t) || interior.contains(&t.above())) {
                    ex.add("C11 fire", "lava", "can light a fire on a flammable block of the farm", t.above());
                }
            }
        }
    }
    // C14 patrols (`PatrolSpawner`, after day 5, during the day): none when the AFK player's section
    // is within 2 sections of a village center (`isCloseToVillage(pos, 2)`); else a patrol leader
    // can stand 24-47 blocks out in x and z on the MOTION_BLOCKING_NO_LEAVES surface where a
    // pillager passes `isValidEmptySpawnBlock`, block light <= 8 and its floor's `isValidSpawn`
    let afk_block = BlockPos::new(farm.afk.x.floor() as i32, farm.afk.y.floor() as i32, farm.afk.z.floor() as i32);
    if sections_to_village(w, afk_block, &farm.beds) > 2 {
        for sx in [-1, 1] {
            for sz in [-1, 1] {
                for ax in 24..48 {
                    for az in 24..48 {
                        let (x, z) = (afk_block.x + sx * ax, afk_block.z + sz * az);
                        if x - 10 < w.min.x || z - 10 < w.min.z || x + 10 >= w.min.x + w.size[0] || z + 10 >= w.min.z + w.size[2] {
                            continue;
                        }
                        let y = w.height(crate::terrain::Heightmap::MotionBlockingNoLeaves, x, z);
                        let p = BlockPos::new(x, y, z);
                        if empty_ok(w, p, "minecraft:pillager") && w.light_or(w.block_light(p), 0) <= 8 && valid_spawn_floor(w, p.below(), "minecraft:pillager") {
                            ex.add("C14 patrols", "a pillager patrol", "can spawn 24-47 blocks from the AFK player (it is more than 2 sections from a village; from day 5)", p);
                        }
                    }
                }
            }
        }
    }
    // C13: village villagers reach a farm bed
    if let Some(v) = villager_flood.as_ref().filter(|_| !farm.beds.is_empty()) {
        for (p, _) in v.cells.iter().filter(|(p, _)| !interior.contains(p)) {
            if farm.beds.iter().any(|bed| (p.x - bed.x).abs() <= 1 && (p.z - bed.z).abs() <= 1 && (p.y - bed.y).abs() <= 1) {
                ex.add("C13 bed claims", "a village villager", "can walk next to a farm bed (and claim it)", *p);
            }
        }
    }
    // C16: a farm villager's eye sees the sky (lightning turns it into a witch)
    for e in &farm.villager_eyes {
        let c = BlockPos::new(e.x.floor() as i32, e.y.floor() as i32, e.z.floor() as i32);
        if w.sees_or(w.can_see_sky(c), false) {
            ex.add("C16 lightning", "lightning", "can strike a farm villager whose eye sees the sky (a witch)", c);
        }
    }
    let errors = w.take_light_errors();
    if !errors.is_empty() {
        return Err(format!("the hostile checks: {} light read(s) failed (pending since the world's last tick or take: some may predate this check): {}", errors.len(), errors.join("; ")));
    }
    Ok(ex.done())
}

/// C1 at one cell: can a natural spawn of type `k` happen at `p` at all — its placement
/// (`SpawnPlacements.isSpawnPositionOk`: the floor, two empty cells, or water), its spawn rule
/// ([`super::natural::rules_can_pass`] on Hard: the static parts exact, the draws at their best,
/// the light at its worst for the rule) and no block in its box (`noCollision`).
pub fn can_spawn_at(w: &World, k: &super::natural::Kind, p: BlockPos) -> bool {
    let (rules, placement, free) = spawn_parts(w, k, p);
    rules && placement && free
}

/// [`can_spawn_at`]'s three parts at `p`: the spawn rule, the placement, no collision.
pub fn spawn_parts(w: &World, k: &super::natural::Kind, p: BlockPos) -> (bool, bool, bool) {
    let rules = super::natural::rules_can_pass(w, k, p, super::natural::Difficulty::Hard);
    let placement = match k.placement {
        super::natural::Placement::OnGround => valid_spawn_floor(w, p.below(), k.id) && empty_ok(w, p, k.id) && empty_ok(w, p.above(), k.id),
        super::natural::Placement::InWater => fluid(w, p),
        super::natural::Placement::NoRestrictions => true,
    };
    let d = k.ty.def();
    let hw = d.width as f64 / 2.0;
    let bb = Aabb::new(p.x as f64 + 0.5 - hw, p.y as f64, p.z as f64 + 0.5 - hw, p.x as f64 + 0.5 + hw, p.y as f64 + d.height as f64, p.z as f64 + 0.5 + hw);
    (rules, placement, !collides(w, bb, false))
}

fn valid_spawn_floor(w: &World, below: BlockPos, type_id: &str) -> bool {
    let b = w.get(below);
    let id = match b {
        Block::Terrain(id) => Some(id),
        _ => blocks::id_of(&b.to_string()),
    };
    match (id.map(blocks::props), blocks::type_index(type_id)) {
        (Some(p), Some(c)) => p.valid_spawn.get(c).copied().unwrap_or(false),
        // a modeled state the table does not list: a full collision cube counts as a valid floor
        (None, _) => crate::behavior::full_cube(&b),
        _ => false,
    }
}

/// `NaturalSpawner.isValidEmptySpawnBlock(level, pos, state, fluid, type)` (cc767b78, G1 12:07, ironspec 12:54: this
/// check lacked its last two parts, `#prevent_mob_spawning_inside` and `EntityType.isBlockDangerous`): the natural
/// spawner's own reading of it ([`super::natural::valid_empty`]: not a full collision cube, not a signal source, no
/// fluid, not in the tag (the table's, and redsim's own rails), not dangerous to the type by the table's column).
fn empty_ok(w: &World, p: BlockPos, type_id: &str) -> bool {
    super::natural::valid_empty(w, p, type_id)
}

/// Whether the block at `p` is in the `enderman_holdable` tag (an enderman can take it).
pub fn holdable(w: &World, p: BlockPos) -> bool {
    let b = w.get(p);
    let id = match b {
        Block::Terrain(id) => Some(id),
        _ => blocks::id_of(&b.to_string()),
    };
    id.is_some_and(|id| blocks::props(id).has_tag("minecraft:enderman_holdable"))
}

/// `PoiManager.sectionsToVillage`: the distance in sections (26 neighbours, 1 per step: the 3D
/// Chebyshev distance) from `p`'s section to the nearest village center (a section holding an
/// OCCUPIED `#village` POI: a bed head, a bell, an acquirable job site), as far as 2 (what
/// `isCloseToVillage(pos, 2)` asks; anything farther reads 7). Counted as occupied: the bed heads
/// and bells of the world (the samples' villagers sleep in their beds and meet at their bells) and
/// the farm's own beds (`extra`, its villagers sleep there); job sites are not counted (their
/// claims are not in the sample), the side that never hides a patrol.
pub fn sections_to_village(w: &World, p: BlockPos, extra: &[BlockPos]) -> i32 {
    let sec = |q: BlockPos| (q.x.div_euclid(16), q.y.div_euclid(16), q.z.div_euclid(16));
    let (px, py, pz) = sec(p);
    let mut best = 7;
    let mut consider = |q: BlockPos| {
        let (x, y, z) = sec(q);
        best = best.min((x - px).abs().max((y - py).abs()).max((z - pz).abs()));
    };
    for b in extra {
        consider(*b);
    }
    // the sections within 2 (what `isCloseToVillage(pos, 2)` asks); farther POIs read as 7
    let (x0, y0, z0) = ((px - 2) * 16, (py - 2) * 16, (pz - 2) * 16);
    for y in y0.max(w.min.y)..(y0 + 80).min(w.min.y + w.size[1]) {
        for z in z0.max(w.min.z)..(z0 + 80).min(w.min.z + w.size[2]) {
            for x in x0.max(w.min.x)..(x0 + 80).min(w.min.x + w.size[0]) {
                let q = BlockPos::new(x, y, z);
                let Block::Terrain(id) = w.get(q) else { continue };
                let name = blocks::props(id).name.as_str();
                if name.starts_with("minecraft:bell[") || (name.contains("_bed[") && name.contains("part=head")) {
                    consider(q);
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (cc767b78, G1 12:07, ironspec 12:54) `isValidEmptySpawnBlock`'s last two checks, as the natural spawner reads
    /// them: each rail kind refuses a spawn inside it (`#prevent_mob_spawning_inside`); `isBlockDangerous` refuses a
    /// zombie in a cactus, powder snow, fire, a magma block or a lit campfire, not in an unlit campfire or air; a
    /// stray is immune to powder snow, a skeleton isn't. (Wither roses and sweet berry bushes are neither redsim
    /// blocks nor block-table rows, so no redsim world holds one.)
    #[test]
    fn empty_spawn_cells_refuse_rails_and_dangerous_blocks() {
        let mut w = World::new(BlockPos::new(-2, -2, -2), [20, 4, 4]);
        let at = |x: i32| BlockPos::new(x, 0, 0);
        let cells: &[(i32, &str)] = &[
            (0, "minecraft:rail[shape=north_south,waterlogged=false]"),
            (1, "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]"),
            (2, "minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]"),
            (3, "minecraft:activator_rail[powered=false,shape=north_south,waterlogged=false]"),
            (4, "minecraft:cactus[age=0]"),
            (5, "minecraft:powder_snow"),
            (6, "minecraft:fire[age=0,east=false,north=false,south=false,up=false,west=false]"),
            (7, "minecraft:magma_block"),
            (8, "minecraft:campfire[facing=north,lit=true,signal_fire=false,waterlogged=false]"),
            (9, "minecraft:campfire[facing=north,lit=false,signal_fire=false,waterlogged=false]"),
        ];
        for &(x, s) in cells {
            // (a block redsim doesn't model as its own: the terrain cell a world holds it as, its table row)
            let b = Block::parse(s).ok().or_else(|| blocks::id_of(s).map(Block::Terrain)).unwrap_or_else(|| panic!("{s}: neither modeled nor in the block table"));
            w.place_raw(&[(at(x), b)]);
        }
        let ok = |x: i32, ty: &str| empty_ok(&w, at(x), ty);
        for x in 0..=3 {
            assert!(!ok(x, "minecraft:zombie"), "{} refuses a spawn inside it", w.get(at(x)));
        }
        for x in 4..=8 {
            assert!(!ok(x, "minecraft:zombie"), "{} is dangerous to a zombie", w.get(at(x)));
        }
        assert!(ok(9, "minecraft:zombie"), "an unlit campfire burns nothing");
        assert!(ok(15, "minecraft:zombie"), "air passes");
        assert!(ok(5, "minecraft:stray") && !ok(5, "minecraft:skeleton"), "a stray is immune to powder snow, a skeleton isn't");
    }

    /// (cc767b78, ironspec 12:54) The hostile checks refuse when a light read failed (recorded by
    /// `World::light_or`), naming it; with none, the same farm's check is a verdict.
    #[test]
    fn the_hostile_checks_refuse_on_a_failed_light_read() {
        let w = World::new(BlockPos::new(0, 0, 0), [8, 8, 8]);
        let farm = Farm::default();
        assert!(check(&w, &farm, true).is_ok(), "no failed read: a verdict");
        let _ = w.light_or(Err("the block light at (1,2,3): planted".to_string()), 0);
        let r = check(&w, &farm, true);
        assert!(r.as_ref().is_err_and(|e| e.contains("planted")), "{r:?}");
        assert!(check(&w, &farm, true).is_ok(), "taken: the next check is a verdict again");
    }

    /// (G1 14:55 H4) The refusals through real reads: a land-less world with a torch beside a brewing
    /// stand (a block whose light behaviour isn't modeled): the block light near them is an error;
    /// `spawn_parts` there records it (its spawn rule's light read); and `check` refuses on a farm
    /// villager's eye under a brewing stand (C16's sky read through it).
    #[test]
    fn the_hostile_checks_refuse_through_a_real_failed_read() {
        let mut w = World::new(BlockPos::new(-8, -2, -8), [17, 10, 17]);
        let stone = Block::parse("stone").unwrap();
        for x in -6..=6 {
            for z in -6..=6 {
                w.set_block(BlockPos::new(x, 0, z), stone, crate::world::flags::ALL);
            }
        }
        let stand = Block::parse("minecraft:brewing_stand[has_bottle_0=false,has_bottle_1=false,has_bottle_2=false]").unwrap();
        w.place_raw(&[(BlockPos::new(2, 1, 0), stand), (BlockPos::new(1, 1, 0), Block::parse("torch").unwrap())]);
        let r = w.block_light(BlockPos::new(0, 1, 0));
        assert!(r.as_ref().is_err_and(|e| e.contains("brewing_stand")), "{r:?}");
        // spawn_parts: its spawn rule reads the light there
        let zombie = super::super::natural::kind_of("minecraft:zombie").expect("the zombie's kind");
        let _ = spawn_parts(&w, &zombie, BlockPos::new(0, 1, 0));
        let errs = w.take_light_errors();
        assert!(errs.iter().any(|e| e.contains("brewing_stand")), "spawn_parts recorded it: {errs:?}");
        // check: a farm villager's eye under a brewing stand
        w.place_raw(&[(BlockPos::new(-3, 4, -3), stand)]);
        let farm = Farm { villager_eyes: vec![Vec3::new(-2.5, 2.6, -2.5)], ..Farm::default() };
        let r = check(&w, &farm, true);
        assert!(r.as_ref().is_err_and(|e| e.contains("brewing_stand")), "{r:?}");
    }

    /// A closed room: a shell of `wall` round the interior x and z `lo..=lo+4`, y `y0..=y0+2`, its
    /// floor `floor` and its roof `roof`.
    fn room(cells: &mut Vec<(BlockPos, Block)>, lo: i32, y0: i32, floor: Block, wall: Block, roof: Block) {
        for y in y0 - 1..=y0 + 3 {
            for z in lo - 1..=lo + 5 {
                for x in lo - 1..=lo + 5 {
                    let inner = (lo..=lo + 4).contains(&x) && (lo..=lo + 4).contains(&z);
                    let b = if y == y0 - 1 {
                        floor
                    } else if y == y0 + 3 {
                        roof
                    } else if inner {
                        continue;
                    } else {
                        wall
                    };
                    cells.push((BlockPos::new(x, y, z), b));
                }
            }
        }
    }

    /// ironspec and G1 2026-11-11 (vjeux's normal-operation scope, the brief's 26.3 reads): under
    /// `Seeding::SpawnRules`, a closed dark room over y 69 starts a monster and a bat (under its
    /// own roof, night light 0) but no patrol pillager (the surface only) and no slime (no slime
    /// chunk without a seed, over the swamp band); a closed room of glass walls and roof on a stone floor starts a bat too (night light
    /// 4, under Halloween's 6, counted whatever the date); a dark room at y 60 starts a swamp slime where the biome isn't known; the worst case
    /// starts a pillager, a slime and a bat in the dark room; an enderman teleports into a closed
    /// glass-floored room within 31 above the ground it spawns on, and not into one 40 above it.
    #[test]
    fn spawn_rules_seed_only_where_the_game_spawns() {
        let mut w = World::new(BlockPos::new(0, 50, 0), [60, 70, 60]);
        let stone = Block::parse("minecraft:stone").expect("stone");
        let glass = Block::parse("minecraft:glass").expect("glass");
        let slab = Block::parse("minecraft:cobblestone_slab[type=bottom,waterlogged=false]").expect("slab");
        let mut cells = Vec::new();
        for z in 0..60 {
            for x in 0..60 {
                cells.push((BlockPos::new(x, 70, z), stone));
            }
        }
        room(&mut cells, 10, 71, stone, stone, stone); // A: dark, on the ground
        room(&mut cells, 20, 71, stone, glass, glass); // B: glass walls and roof, on the ground
        room(&mut cells, 30, 60, stone, stone, stone); // C: dark, under the ground, y 60
        room(&mut cells, 40, 76, glass, stone, slab); // D: glass floor, 5 over the ground
        room(&mut cells, 40, 111, glass, stone, slab); // E: glass floor, 40 over the ground
        w.place_raw(&cells);
        let none = HashSet::new();
        let bounds = (BlockPos::new(0, 51, 0), BlockPos::new(59, 119, 59));
        let rules = Seeding::SpawnRules { seed: None };
        let reaches = |b: Body, s: Seeding, at: BlockPos| flood_with(&w, b, &none, bounds, s).contains(at);
        let (a, bb, c, d, e) = (BlockPos::new(12, 71, 12), BlockPos::new(22, 71, 22), BlockPos::new(32, 60, 32), BlockPos::new(42, 76, 42), BlockPos::new(42, 111, 42));
        assert!(reaches(ZOMBIE, rules, a), "a monster spawns in the dark room");
        assert!(reaches(BAT, rules, a), "a bat spawns in the dark room under its own roof");
        assert!(reaches(BAT, rules, bb), "a bat in the glass room too (night light 4, Halloween's 6 counted)");
        for b in [PILLAGER, SLIME] {
            assert!(!reaches(b, rules, a), "the spawn rules start no {} in the dark room", b.name);
        }
        for b in [PILLAGER, SLIME, BAT] {
            assert!(reaches(b, Seeding::Worst, a), "the worst case starts a {} in the dark room", b.name);
        }
        assert!(reaches(SLIME, rules, c), "a swamp slime in the dark room at y 60, the biome unknown");
        assert!(reaches(PILLAGER, rules, BlockPos::new(5, 71, 5)), "the surface starts a patrol's pillager");
        assert!(reaches(ENDERMAN, rules, d), "an enderman teleports into the glass-floored room 5 over the ground");
        assert!(!reaches(ENDERMAN, rules, e), "no enderman teleports into the room 40 over the ground");
        // the worst case too (G1 and ironspec 2026-11-12): with the room's inside excluded from its
        // starts, as the judge excludes a farm's, an enderman still teleports in
        let mut d_inside = HashSet::new();
        for y in 76..=78 {
            for z in 40..=44 {
                for x in 40..=44 {
                    d_inside.insert(BlockPos::new(x, y, z));
                }
            }
        }
        assert!(flood_with(&w, ENDERMAN, &d_inside, bounds, Seeding::Worst).contains(d), "the worst case: an enderman teleports into the excluded room 5 over the ground");
    }

    /// G1 2026-11-11: a lightning rod takes the strikes within 128 of it as a sphere
    /// (`PoiManager.findClosest`'s `distSqr`), so a strike cell 100 off on both horizontal axes
    /// (141 away) keeps its trap start, which the square |dx|, |dz| <= 128 dropped; one 60 off on
    /// both (85 away) loses it. (The strike cell stands on glass, no skeleton's spawn floor: the
    /// trap is its only start.)
    #[test]
    fn a_rod_takes_strikes_within_128_as_a_sphere() {
        let strike = BlockPos::new(0, 62, 0);
        let with_rod = |at: i32| -> World {
            let mut w = World::new(BlockPos::new(-5, 55, -5), [110, 15, 110]);
            let glass = Block::parse("minecraft:glass").expect("glass");
            let rod = Block::parse("minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]").expect("a lightning rod");
            w.place_raw(&[(strike.below(), glass), (BlockPos::new(at, 61, at), rod)]);
            w
        };
        let none = HashSet::new();
        let bounds = (BlockPos::new(-2, 56, -2), BlockPos::new(2, 68, 2));
        let rules = Seeding::SpawnRules { seed: None };
        let trap = |w: &World| flood_with(w, SKELETON, &none, bounds, rules).contains(strike);
        assert!(trap(&with_rod(100)), "a rod 141 away takes no strike: the trap starts");
        assert!(!trap(&with_rod(60)), "a rod 85 away takes the strike: no trap");
    }

    /// The parent and ironspec 2026-11-11: a flood's fall stops at a block, in both modes. A pit
    /// with a glass floor (no spawn floor) and stone walls 3 high: a zombie on the wall's top falls
    /// in when the pit is open (the positive control); with a one-block stone roof over the pit, a
    /// zombie on the roof no longer falls through it into the closed pit (the negative one; the old
    /// fall went straight down the neighbour column from one below, through any block with air
    /// under it, and reached the pit).
    #[test]
    fn a_fall_stops_at_a_block() {
        let pit = |roofed: bool| -> World {
            let mut w = World::new(BlockPos::new(0, 60, 0), [40, 20, 40]);
            let stone = Block::parse("minecraft:stone").expect("stone");
            let glass = Block::parse("minecraft:glass").expect("glass");
            let mut cells = Vec::new();
            for z in 0..40 {
                for x in 0..40 {
                    let inside = (18..=22).contains(&x) && (18..=22).contains(&z);
                    cells.push((BlockPos::new(x, 64, z), if inside { glass } else { stone }));
                }
            }
            for y in 65..=67 {
                for z in 17..=23 {
                    for x in 17..=23 {
                        if !((18..=22).contains(&x) && (18..=22).contains(&z)) {
                            cells.push((BlockPos::new(x, y, z), stone));
                        }
                    }
                }
            }
            if roofed {
                for z in 17..=23 {
                    for x in 17..=23 {
                        cells.push((BlockPos::new(x, 68, z), stone));
                    }
                }
            }
            w.place_raw(&cells);
            w
        };
        let none = HashSet::new();
        let bounds = (BlockPos::new(0, 61, 0), BlockPos::new(39, 79, 39));
        let floor = BlockPos::new(20, 65, 20);
        for s in [Seeding::Worst, Seeding::SpawnRules { seed: None }] {
            let open = pit(false);
            assert!(flood_with(&open, ZOMBIE, &none, bounds, s).contains(floor), "{s:?}: a zombie on the open pit's wall falls in");
            let roofed = pit(true);
            let reach = flood_with(&roofed, ZOMBIE, &none, bounds, s);
            assert!(reach.contains(BlockPos::new(20, 69, 20)), "{s:?}: a zombie stands on the roof");
            assert!(!reach.contains(floor), "{s:?}: no zombie falls through the one-block roof into the closed pit");
        }
    }
}
