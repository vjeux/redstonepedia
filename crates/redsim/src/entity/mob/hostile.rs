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
fn floor_in(w: &World, p: BlockPos) -> Option<f64> {
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
fn fits(w: &World, b: &Body, p: BlockPos, feet: f64) -> bool {
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

/// Flood a body over the world from every standable position outside `exclude` (the farm's
/// interior): walks (step or a 1.25 jump up, any fall down), swims, climbs (spiders), flies (any
/// air). Positions inside `exclude` are reached too (that is the entry channel).
pub fn flood(w: &World, b: Body, exclude: &HashSet<BlockPos>, bounds: (BlockPos, BlockPos)) -> Reach {
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
    let may_start = |p: BlockPos| -> bool {
        if ring(p) || b.name == "enderman" {
            return true;
        }
        if !residents.is_empty() || matches!(b.name, "villager" | "iron golem") {
            return residents.contains(&p);
        }
        if b.flies && p.y >= w.height(crate::terrain::Heightmap::WorldSurface, p.x, p.z) {
            return false;
        }
        // (a monster spawns in the overworld only at block light 0: `Monster.isDarkEnoughToSpawn`
        // against the dimension's monster_spawn_block_light_limit, 0; a torch-lit cell is no
        // spawn start, ironspec's fix for a lit built-in access. Slimes, pillagers, bats and foxes
        // keep their own rules: any light here)
        if matches!(b.name, "zombie" | "baby zombie" | "skeleton" | "creeper" | "spider" | "cave spider") && w.block_light(p).unwrap_or(0) > 0 {
            return false;
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
    let dirs: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
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
                for dy in -1..=1 {
                    next.push(p.offset(dx, dy, dz));
                }
                // a fall: straight down the neighbour column
                let mut q = p.offset(dx, -1, dz);
                let mut n = 0;
                while q.y > w.min.y && floor_in(w, q).is_none() && n < 384 {
                    q = q.below();
                    n += 1;
                }
                next.push(q);
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
pub fn check(w: &World, farm: &Farm, village_golem: bool) -> Vec<Exposure> {
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
    if afk_cell.y >= 63 && w.can_see_sky(afk_cell).unwrap_or(false) {
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
        if flammable && w.can_see_sky(b.above()).unwrap_or(false) {
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
                        if empty_ok(w, p) && w.block_light(p).unwrap_or(0) <= 8 && valid_spawn_floor(w, p.below(), "minecraft:pillager") {
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
        if w.can_see_sky(c).unwrap_or(false) {
            ex.add("C16 lightning", "lightning", "can strike a farm villager whose eye sees the sky (a witch)", c);
        }
    }
    ex.done()
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
        super::natural::Placement::OnGround => valid_spawn_floor(w, p.below(), k.id) && empty_ok(w, p) && empty_ok(w, p.above()),
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

fn empty_ok(w: &World, p: BlockPos) -> bool {
    let b = w.get(p);
    !crate::behavior::full_cube(&b) && !crate::behavior::is_signal_source(w, &b) && !fluid(w, p)
}

fn holdable(w: &World, p: BlockPos) -> bool {
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
