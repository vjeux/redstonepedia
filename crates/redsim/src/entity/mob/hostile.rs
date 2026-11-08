//! The worst-case checks for natural mobs' unmodeled behaviour (the terrain lane; brief
//! `designs/briefs/terrain-and-spawning.md`, "The checks", channels C1-C16). Natural mobs' walking
//! and targeting is unmodeled AI, so a farm in terrain is refused whenever any mob could, by any
//! path, reach, see, attack or enter it:
//!
//! * a **flood** per body class (the types' boxes and step heights) over the terrain plus the farm,
//!   started at every standable position outside the farm whatever its light (natural spawns,
//!   sieges, the village's own mobs, the box's edges all start somewhere in it); spiders climb,
//!   endermen teleport (every standable position), flyers take any air;
//! * the channels read the flood: entry into the farm's interior, a villager-hostile's sight of a
//!   farm villager, melee and ranged reach, the village golem's and villagers' presence, the golem
//!   search outside the farm's floors, creepers near the AFK player, endermen, fire, lightning, bed
//!   claims, natural spawns inside.
//!
//! Every channel is a superset of what the game can do (a position counts if the mob's box fits
//! anywhere in the cell), so a farm that passes cannot be reached by any natural mob.

use crate::block::Block;
use crate::entity::{Aabb, Vec3};
use crate::pos::BlockPos;
use crate::terrain::blocks;
use crate::world::World;
use std::collections::{HashSet, VecDeque};

/// A body class: a box and how it moves.
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
}

const fn body(name: &'static str, width: f64, height: f64, step: f64, climbs: bool, flies: bool, doors: bool) -> Body {
    Body { name, width, height, step, climbs, flies, doors }
}

/// The body classes of the natural mobs, the village's villagers and golem (`EntityTypes` sizes).
pub const ZOMBIE: Body = body("zombie", 0.6, 1.95, 0.6, false, false, true);
pub const BABY_ZOMBIE: Body = body("baby zombie", 0.3, 0.975, 0.6, false, false, true);
pub const SKELETON: Body = body("skeleton", 0.6, 1.99, 0.6, false, false, false);
pub const CREEPER: Body = body("creeper", 0.6, 1.7, 0.6, false, false, false);
pub const SPIDER: Body = body("spider", 1.4, 0.9, 0.6, true, false, false);
pub const CAVE_SPIDER: Body = body("cave spider", 0.7, 0.5, 0.6, true, false, false);
pub const ENDERMAN: Body = body("enderman", 0.6, 2.9, 1.0, false, false, false);
pub const SLIME: Body = body("slime", 0.52, 0.52, 0.6, false, false, false);
pub const VILLAGER: Body = body("villager", 0.6, 1.95, 0.6, false, false, true);
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
}

/// One refusal: the channel and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub channel: &'static str,
    pub why: String,
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

/// The box covering every placement of a body in a cell (for reach tests: the superset).
pub fn reach_box(b: &Body, p: BlockPos, feet: f64) -> Aabb {
    let hw = b.width / 2.0;
    let slack = (0.5 - hw).max(0.0);
    Aabb::new(p.x as f64 + 0.5 - hw - slack, feet, p.z as f64 + 0.5 - hw - slack, p.x as f64 + 0.5 + hw + slack, feet + b.height, p.z as f64 + 0.5 + hw + slack)
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
    // the starts: every standable position outside the farm (inside the bounds: their edge is a
    // start too, so nothing beyond them reaches further than the flood shows)
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = BlockPos::new(x, y, z);
                if exclude.contains(&p) {
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

/// Run every channel; returns the refusals (empty: the farm passes).
pub fn check(w: &World, farm: &Farm, village_golem: bool) -> Vec<Refusal> {
    let mut out = Vec::new();
    let interior: HashSet<BlockPos> = farm.interior.iter().copied().collect();
    let refuse = |out: &mut Vec<Refusal>, channel: &'static str, why: String| {
        if !out.iter().any(|r: &Refusal| r.channel == channel) {
            out.push(Refusal { channel, why });
        }
    };
    // C1: a natural spawn inside: any type listed at the cell (structure overrides included) whose
    // placement passes and whose rule can pass at the worst light: monsters need block light 0
    // (the night's darkening and the gates' best draws do the rest), sulfur cubes and slimes no
    // light, animals brightness above 8, bats a raw brightness of 3 or less at the darkest
    use super::natural::{Category, Rule, mobs_at};
    for &p in &farm.interior {
        let block_light = w.block_light(p).unwrap_or(0);
        let sky = w.sky_light(p).unwrap_or(15);
        for cat in [Category::Monster, Category::Creature, Category::Ambient] {
            for s in mobs_at(w, cat, p) {
                let Some(k) = super::natural::kind_of(&s.ty) else { continue };
                let light_ok = match k.rule {
                    Rule::Always | Rule::AnyLight | Rule::Slime => true,
                    Rule::Monster | Rule::Drowned => block_light == 0,
                    Rule::Animal | Rule::AnimalTag(_) | Rule::Turtle => block_light.max(sky) > 8,
                    Rule::Bat => block_light.max(sky - 11) <= 3 && p.y < w.height(crate::terrain::Heightmap::WorldSurface, p.x, p.z),
                    Rule::GlowSquid | Rule::SurfaceWater => true,
                };
                if !light_ok {
                    continue;
                }
                let floor_ok = match k.placement {
                    super::natural::Placement::OnGround => valid_spawn_floor(w, p.below(), k.id) && empty_ok(w, p) && empty_ok(w, p.above()),
                    super::natural::Placement::InWater => fluid(w, p),
                    super::natural::Placement::NoRestrictions => true,
                };
                if floor_ok {
                    let d = k.ty.def();
                    let hw = d.width as f64 / 2.0;
                    let bb = Aabb::new(p.x as f64 + 0.5 - hw, p.y as f64, p.z as f64 + 0.5 - hw, p.x as f64 + 0.5 + hw, p.y as f64 + d.height as f64, p.z as f64 + 0.5 + hw);
                    if !collides(w, bb, false) {
                        refuse(&mut out, "C1 natural spawn inside", format!("a {} can spawn at {p} inside the farm", s.ty));
                    }
                }
            }
        }
    }
    // the floods, over the farm's surroundings (24 columns around its blocks and villagers, the
    // whole height)
    let mut lo = BlockPos::new(i32::MAX, w.min.y, i32::MAX);
    let mut hi = BlockPos::new(i32::MIN, w.min.y + w.size[1] - 1, i32::MIN);
    for p in farm.blocks.iter().copied().chain(farm.villagers.iter().map(|v| BlockPos::new(v.min_x.floor() as i32, v.min_y.floor() as i32, v.min_z.floor() as i32))) {
        lo = BlockPos::new(lo.x.min(p.x - 24), lo.y, lo.z.min(p.z - 24));
        hi = BlockPos::new(hi.x.max(p.x + 24), hi.y, hi.z.max(p.z + 24));
    }
    let bounds = (lo, hi);
    let walkers = [ZOMBIE, BABY_ZOMBIE, SKELETON, CREEPER, SPIDER, CAVE_SPIDER, SLIME];
    let mut reaches: Vec<Reach> = walkers.iter().map(|b| flood(w, *b, &interior, bounds)).collect();
    reaches.push(flood(w, BAT, &interior, bounds));
    // C2: entry into the interior (and contact with the farm's villagers)
    for r in &reaches {
        if let Some((p, _)) = r.cells.iter().find(|(p, _)| interior.contains(p)) {
            refuse(&mut out, "C2 entry", format!("a {} gets into the farm at {p}", r.body.name));
        }
    }
    // C3: a villager hostile's sight (zombie family: 8 blocks between feet, `VillagerHostilesSensor`;
    // a clear COLLIDER ray between eyes, `hasLineOfSight`); the mob anywhere in its cell
    let villager_feet: Vec<Vec3> = farm.villagers.iter().map(|v| Vec3::new(v.center().x, v.min_y, v.center().z)).collect();
    for r in reaches.iter().filter(|r| matches!(r.body.name, "zombie" | "baby zombie")) {
        for (p, f) in &r.cells {
            let feet = Vec3::new(p.x as f64 + 0.5, *f, p.z as f64 + 0.5);
            let eye = Vec3::new(feet.x, feet.y + r.body.height * 0.85, feet.z);
            for (vf, ve) in villager_feet.iter().zip(farm.villager_eyes.iter()) {
                if dist2(feet, *vf).sqrt() <= 8.0 + 0.71 && super::los::has_line_of_sight(w, eye, *ve) {
                    refuse(&mut out, "C3 sight", format!("a {} at {p} sees the farm villager at {:.1},{:.1},{:.1}", r.body.name, ve.x, ve.y, ve.z));
                }
            }
        }
    }
    // C5: melee reach (the attack box: the mob's box inflated by sqrt(2.04) - 0.6 horizontally)
    let reach = 2.04f64.sqrt() - 0.6;
    for r in reaches.iter().filter(|r| matches!(r.body.name, "zombie" | "baby zombie" | "spider" | "cave spider")) {
        for (p, f) in &r.cells {
            let ab = reach_box(&r.body, *p, *f).inflate(reach, 0.0, reach);
            if let Some(v) = farm.villagers.iter().find(|v| v.intersects(ab)) {
                refuse(&mut out, "C5 reach", format!("a {} at {p} reaches the farm villager at {:.1},{:.1},{:.1}", r.body.name, v.min_x, v.min_y, v.min_z));
            }
        }
    }
    // C4 + C5: the village golem near the farm villagers (their golem sensor) or reaching the zombie
    if village_golem {
        let g = flood(w, GOLEM, &interior, bounds);
        for (p, f) in &g.cells {
            let gb = reach_box(&GOLEM, *p, *f);
            if farm.villagers.iter().any(|v| (gb.center().x - v.center().x).abs() < 17.84 + 0.3 + 0.7 && (gb.center().z - v.center().z).abs() < 17.84 + 0.3 + 0.7 && (gb.min_y - v.max_y) < 8.0 && (v.min_y - gb.max_y) < 8.0) {
                refuse(&mut out, "C4 presence", format!("the village golem can stand at {p}, within the farm villagers' golem sensor"));
                break;
            }
            let ab = gb.inflate(reach, 0.0, reach);
            if farm.zombie.iter().any(|z| z.intersects(ab)) {
                refuse(&mut out, "C5 reach", format!("the village golem at {p} reaches the farm's zombie"));
                break;
            }
        }
    }
    // C7: the golem search from each farm villager finds a spot outside the farm's floors
    let floors: HashSet<BlockPos> = farm.golem_cells.iter().copied().collect();
    let mut starts: Vec<BlockPos> = farm.villagers.iter().map(|v| BlockPos::new(v.center().x.floor() as i32, v.min_y.floor() as i32, v.center().z.floor() as i32)).collect();
    starts.sort_by_key(|p| (p.x, p.y, p.z));
    starts.dedup();
    'c7: for s in &starts {
        for dz in -8..=8 {
            for dx in -8..=8 {
                if let Some(c) = super::spawn::possible_spawn_position(w, s.offset(dx, 6, dz), 6)
                    && !floors.contains(&c)
                {
                    let pos = Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5);
                    if super::spawn::check_spawn_obstruction(w, c, pos) {
                        refuse(&mut out, "C7 golem search", format!("a farm villager at {s} can spawn a golem at {c}, outside the farm's spawn floors"));
                        break 'c7;
                    }
                }
            }
        }
    }
    // C8: creepers near the AFK player whose explosion reaches the farm
    if let Some(cr) = reaches.iter().find(|r| r.body.name == "creeper") {
        for (p, f) in &cr.cells {
            let c = Vec3::new(p.x as f64 + 0.5, *f, p.z as f64 + 0.5);
            if dist2(c, farm.afk) > 49.0 {
                continue;
            }
            if let Some(b) = farm.blocks.iter().find(|b| dist2(Vec3::new(b.x as f64 + 0.5, b.y as f64 + 0.5, b.z as f64 + 0.5), c) <= 10.4 * 10.4) {
                refuse(&mut out, "C8 creepers", format!("a creeper at {p} near the AFK player can blow up the farm block at {b}"));
                break;
            }
        }
    }
    // C9: endermen take a holdable farm block (every standable position: they teleport)
    let holdable: Vec<BlockPos> = farm.blocks.iter().copied().filter(|b| holdable(w, *b)).collect();
    if !holdable.is_empty() {
        let e = flood(w, ENDERMAN, &interior, bounds);
        'c9: for (p, _) in &e.cells {
            for b in &holdable {
                if (b.x - p.x).abs() <= 2 && (b.z - p.z).abs() <= 2 && b.y - p.y >= 0 && b.y - p.y <= 3 {
                    refuse(&mut out, "C9 endermen", format!("an enderman at {p} can take the farm block {} at {b}", w.get(*b)));
                    break 'c9;
                }
            }
        }
    }
    // C10: flyers inside (bats)
    // (the flood above includes the bat: C2 reports it)
    // C11: fire: a flammable farm block that lightning can reach (its top sees the sky)
    for b in &farm.blocks {
        let st = w.get(*b);
        let flammable = match st {
            Block::Terrain(id) => blocks::props(id).burn_odds > 0,
            _ => blocks::id_of(&st.to_string()).is_some_and(|id| blocks::props(id).burn_odds > 0),
        };
        if flammable && w.can_see_sky(b.above()).unwrap_or(false) {
            refuse(&mut out, "C11 fire", format!("the flammable farm block {st} at {b} is open to the sky (lightning fire)"));
            break;
        }
    }
    // C13: village villagers reach a farm bed
    if !farm.beds.is_empty() {
        let v = flood(w, VILLAGER, &interior, bounds);
        for bed in &farm.beds {
            if let Some((p, _)) = v.cells.iter().find(|(p, _)| (p.x - bed.x).abs() <= 1 && (p.z - bed.z).abs() <= 1 && (p.y - bed.y).abs() <= 1 && !interior.contains(p)) {
                refuse(&mut out, "C13 bed claims", format!("a village villager can walk to {p}, next to the farm bed at {bed}"));
                break;
            }
        }
    }
    // C16: a farm villager's eye sees the sky (lightning turns it into a witch)
    for e in &farm.villager_eyes {
        let c = BlockPos::new(e.x.floor() as i32, e.y.floor() as i32, e.z.floor() as i32);
        if w.can_see_sky(c).unwrap_or(false) {
            refuse(&mut out, "C16 lightning", format!("the farm villager's eye at {c} sees the sky"));
            break;
        }
    }
    out
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
