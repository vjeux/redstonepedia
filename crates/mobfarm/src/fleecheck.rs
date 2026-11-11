//! (tickexact 2026-11-11, ironspec's HOME rule after 58f770cc's finding) The walk-target static
//! check: from every place a rider can be (each rail cell of the design, the rider seated in a cart
//! there), every cell its brain can take as a walk target, the chance each event picks it, and
//! whether the rider has a complete path to it (redsim's villager pathfinder: the villager's own
//! navigation, `setRequiredPathLength(48)`: the path's walked length under 48 and its search
//! stopping after 767 nodes, ironspec Nov 11 11:48; `mctas mobai flee-control` holds the
//! reachability controls). A target with no complete path starts the rider's CANT_REACH count, which
//! nothing clears
//! until a path reaches; at its first REST tick by the bed a count over 1,200 gt releases HOME (the
//! bench's z -5 stone: scene flee-stone bridges the flee's part against the game).
//!
//! The sources, by activity:
//! - **PANIC** (a hostile within 8 seen): SetWalkTargetAwayFrom(NEAREST_HOSTILE) while it is within
//!   6 (`LandRandomPos.getPosAway(mob, 16, 7)`, ten calls of ten draws), and VillageBoundRandomStroll
//!   (2/2).
//! - **IDLE** (calm): VillageBoundRandomStroll (10/7: a land draw in a village, else
//!   DefaultRandomPos toward the section closest to the village), and the walks to another villager
//!   within 8 (InteractWith; SetWalkTargetFromLookTarget): every other rider place within 8.
//! - **Job site / bell walks within 48** (SetWalkTargetFromBlockMemory): the job-site blocks and
//!   bells of the design, listed (a farm's riders hold none of them).
//! - **REST**: the HOME walk (the bed itself), for the brief's 1,000-gt line.
//!
//! A cell only the draws' boundary reaches (a share of 0: an offset of exactly 16) is listed as an
//! edge cell and kept out of the verdict. The hostiles are taken whether the rider sees them in the
//! design as placed or not (a hatch, a gate or a cart moves; the sight now is printed). The check
//! reads the design's own world (built and settled as the judge places it); a void bench offers only
//! the design's tops as candidates (ironspec: label its HOME rows "void bench").

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// One source's result at a place: its cells, and those with no complete path with the chance one
/// event picks each (a share of 0: an edge cell).
#[derive(Default)]
pub struct Source {
    pub cells: usize,
    pub unreached: Vec<(BlockPos, f64)>,
    /// The chance one event picks a cell the rider's path reaches (what clears a stamp).
    pub reached_share: f64,
}

impl Source {
    /// The chance one event picks an unreached cell.
    pub fn risk(&self) -> f64 {
        self.unreached.iter().map(|(_, s)| *s).sum()
    }

    /// Unreached cells a draw can really pick (a share over 0).
    pub fn live(&self) -> usize {
        self.unreached.iter().filter(|(_, s)| *s > 1e-9).count()
    }
}

/// One rider place's result.
pub struct Place {
    pub cell: BlockPos,
    /// A villager standing there (a dropped napper), not a rider seated in a cart.
    pub on_foot: bool,
    /// The hostiles within 6 (the flee's) and within 8 (PANIC), and how many it sees as placed.
    pub flee_from: usize,
    pub panic_from: usize,
    pub seen: usize,
    pub village: bool,
    pub flee: Source,
    pub panic_stroll: Source,
    pub stroll: Source,
    pub others: Source,
    pub home_reached: Option<bool>,
    /// The nearest hostile's distance: the least and the greatest over the place's points, and
    /// the hostile nearest at the least (None: no hostile in the design).
    pub near: Option<(f64, f64, Vec3)>,
    /// The closest rider-zombie pair at this place: the distance, the rider's position, the
    /// zombie's.
    pub worst: Option<(f64, Vec3, Vec3)>,
    /// In the design's `panic:` boxes (the part relies on panic there).
    pub panic_declared: bool,
    /// Every point sees a zombie within PANIC_BAR (the eyes' line clear).
    pub panic_ok: bool,
}

/// The hostiles of the design's entity lines (a zombie, or a zombie riding a boat or a cart): their
/// feet.
fn hostiles(spec: &FarmSpec) -> Vec<Vec3> {
    spec.entities
        .iter()
        .filter(|(ty, _, nbt)| ty.ends_with("zombie") || nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie\"")))
        .map(|(_, p, _)| Vec3::new(p[0], p[1], p[2]))
        .collect()
}

/// A cart's path through rail cell `c` (shape from `state`) as a segment, 0.0625 over the rail's
/// surface: straight and sloped rails from edge to edge (a slope rising toward its side), a curve
/// between its two exits (the cart runs the chord between them). None for an unknown shape.
pub fn rail_segment(c: BlockPos, state: &str) -> Option<(Vec3, Vec3)> {
    let (x, y, z) = (c.x as f64, c.y as f64 + 0.0625, c.z as f64);
    let s = state.split("shape=").nth(1)?.split([',', ']']).next()?;
    Some(match s {
        "north_south" => (Vec3::new(x + 0.5, y, z), Vec3::new(x + 0.5, y, z + 1.0)),
        "east_west" => (Vec3::new(x, y, z + 0.5), Vec3::new(x + 1.0, y, z + 0.5)),
        "ascending_east" => (Vec3::new(x, y, z + 0.5), Vec3::new(x + 1.0, y + 1.0, z + 0.5)),
        "ascending_west" => (Vec3::new(x, y + 1.0, z + 0.5), Vec3::new(x + 1.0, y, z + 0.5)),
        "ascending_south" => (Vec3::new(x + 0.5, y, z), Vec3::new(x + 0.5, y + 1.0, z + 1.0)),
        "ascending_north" => (Vec3::new(x + 0.5, y + 1.0, z), Vec3::new(x + 0.5, y, z + 1.0)),
        "south_east" => (Vec3::new(x + 0.5, y, z + 1.0), Vec3::new(x + 1.0, y, z + 0.5)),
        "south_west" => (Vec3::new(x + 0.5, y, z + 1.0), Vec3::new(x, y, z + 0.5)),
        "north_west" => (Vec3::new(x + 0.5, y, z), Vec3::new(x, y, z + 0.5)),
        "north_east" => (Vec3::new(x + 0.5, y, z), Vec3::new(x + 1.0, y, z + 0.5)),
        _ => return None,
    })
}

/// The least distance from point `p` to segment `a`-`b`, and the segment's nearest point.
pub fn seg_dist(a: Vec3, b: Vec3, p: Vec3) -> (f64, Vec3) {
    let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    let l2 = dx * dx + dy * dy + dz * dz;
    let t = if l2 > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy + (p.z - a.z) * dz) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let q = Vec3::new(a.x + t * dx, a.y + t * dy, a.z + t * dz);
    (((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt(), q)
}

/// The 6-8 rule's bars (ironspec Nov 11 10:54, scene flee-distance bridged): a zombie inside 6.05
/// of a rider position fails; a declared panic place needs one seen within 7.95.
pub const FLEE_BAR: f64 = 6.05;
pub const PANIC_BAR: f64 = 7.95;

/// One rider position whose eye sits inside a block with a collision shape.
pub struct EyeHit {
    /// The rail cell (the place).
    pub cell: BlockPos,
    /// The seated rider's eye.
    pub eye: Vec3,
    /// The block its eye box meets, and its state.
    pub block: BlockPos,
    pub state: String,
    /// The block suffocates (`isSuffocating`: `Entity.isInWall` hurts the rider every tick).
    pub suffocating: bool,
}

/// The blindness controls (ironspec Nov 11 14:08: sight clips from the eye point, so a rider is
/// blind only with the point inside a collision shape): a rider on a lone east-west rail at
/// (0,0,0), first with a top slab in its eye cell (0,1,0), the eye point inside it: blind; then
/// with the top slab one cell along at (1,1,0), where the rider's eye box at 0.9 along the rail
/// grazes it with the point outside: sighted. Each (what, blind found, as expected).
pub fn blind_control() -> Result<Vec<(&'static str, bool, bool)>, String> {
    let stone = redsim::block::Block::parse("minecraft:stone")?;
    let rail = redsim::block::Block::parse("minecraft:rail[shape=east_west]")?;
    let slab = redsim::block::Block::parse("minecraft:smooth_stone_slab[type=top]")?;
    let mut out = Vec::new();
    for (what, at, want) in [("the eye point inside a top slab at the eye cell (0,1,0)", BlockPos::new(0, 1, 0), true), ("the eye box grazing a top slab at (1,1,0), the point outside", BlockPos::new(1, 1, 0), false)] {
        let mut blocks = vec![(BlockPos::new(0, -1, 0), stone.clone()), (BlockPos::new(0, 0, 0), rail.clone()), (at, slab.clone())];
        if at.x == 1 {
            blocks.push((BlockPos::new(1, -1, 0), stone.clone()));
        }
        let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(20, 1, 0), lo: BlockPos::new(-4, -2, -4), hi: BlockPos::new(24, 8, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let (_, hits) = eye_check(&spec, false)?;
        let blind = hits.iter().any(|h| !h.suffocating && h.cell == BlockPos::new(0, 0, 0));
        out.push((what, blind, blind == want));
    }
    Ok(out)
}

/// The reachability controls (ironspec Nov 11 11:48: flee-check's paths are the villager's own
/// navigation, required length 48, the search stopping after 767 nodes): a villager at (1,1,1) in
/// a serpentine corridor one block wide (an open floor beside a wall measured unreachable at a
/// walk of about 30: the search's walked distances, overwritten from every neighbour as the game's
/// are, inflate there); target A (1,1,4) a 31-long walk (reachable at 48, unreachable at a 14-17.8
/// follow range); target B (1,1,10) a 63-long walk (unreachable); target C (3,1,1) plain. Each
/// target's reach (a complete path, reach 0) and the path's node count.
pub fn reach_control() -> Result<Vec<(&'static str, bool, usize, bool)>, String> {
    let stone = redsim::block::Block::parse("minecraft:stone")?;
    // (a serpentine corridor one block wide, walled everywhere else so no open floor can inflate
    // the search's walked distances: east along z 1 (x 1..15), north at x 15 (z 2..3), west along
    // z 4 (to x 1), north at x 1 (z 5..6), east along z 7, north at x 15 (z 8..9), west along z 10)
    let mut open: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    for x in 1..=15 {
        open.insert((x, 1));
        open.insert((x, 4));
        open.insert((x, 7));
        open.insert((x, 10));
    }
    for z in [2, 3, 8, 9] {
        open.insert((15, z));
    }
    for z in [5, 6] {
        open.insert((1, z));
    }
    let mut blocks = Vec::new();
    for x in -1..=17 {
        for z in -1..=12 {
            blocks.push((BlockPos::new(x, 0, z), stone.clone()));
            if !open.contains(&(x, z)) {
                for y in 1..=3 {
                    blocks.push((BlockPos::new(x, y, z), stone.clone()));
                }
            }
        }
    }
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(60, 1, 12), lo: BlockPos::new(-4, -2, -4), hi: BlockPos::new(64, 12, 16), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let vid = redsim::entity::spawn(w, "minecraft:villager", [1.5, 1.0, 1.5], Some("{PersistenceRequired:1b}"))?;
    let i = redsim::entity::index_of(w, vid).ok_or("the control's villager")?;
    // (standing on the floor: on the ground, as `canUpdatePath` needs)
    w.entities.list[i].on_ground = true;
    let mut out = Vec::new();
    for (name, t, want) in [("C plain, 2 along the corridor (3,1,1)", BlockPos::new(3, 1, 1), true), ("A a 31-long corridor walk (1,1,4)", BlockPos::new(1, 1, 4), true), ("B a 63-long corridor walk (1,1,10)", BlockPos::new(1, 1, 10), false)] {
        let (reached, nodes) = redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
            let mut copy = nav.clone();
            copy.create_path_pos(w, nm, t, 0, arena).map_or((false, 0), |p| (p.reached, p.nodes.len()))
        });
        out.push((name, reached, nodes, reached == want));
    }
    Ok(out)
}

/// The eye check (ironspec Nov 11 11:53, after 58f770cc's R plate support): at every rail cell,
/// the rider seated at three points along it, its eye box (`Entity.isInWall`'s: width x 0.8 around
/// the eye, 1e-6 tall) against every suffocating block, and its eye point against every other
/// collision shape. A suffocating one hurts the rider every tick (the census: a cause of a rider's
/// death the farm must exclude); any other (glass, a closed gate, a slab) with the eye point
/// inside it blinds it, since its sight ray starts inside the shape (a shape the eye box only
/// grazes leaves the point outside, and the ray then misses or crosses it by direction). With
/// `control`, a stone set at the eye cell of the first rail cell whose eye cell is air first (the
/// positive control: it must fail). Returns the places checked and the hits.
pub fn eye_check(spec: &FarmSpec, control: bool) -> Result<(usize, Vec<EyeHit>), String> {
    let mut s = spec.clone();
    s.entities.clear();
    let mut rails: Vec<(BlockPos, String)> = s.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    rails.sort_by_key(|(p, _)| (p.x, p.y, p.z));
    if control {
        let taken: std::collections::HashSet<BlockPos> = s.blocks.iter().map(|(p, _)| *p).collect();
        if let Some((c, _)) = rails.iter().find(|(c, _)| !taken.contains(&BlockPos::new(c.x, c.y + 1, c.z))) {
            s.blocks.push((BlockPos::new(c.x, c.y + 1, c.z), redsim::block::Block::parse("minecraft:stone")?));
        }
    }
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&s, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let mut hits = Vec::new();
    for (c, state) in &rails {
        let p0 = along_rail(*c, state, 0.5);
        let cart = redsim::entity::spawn(w, "minecraft:minecart", [p0.x, p0.y, p0.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
        let Some(ci) = redsim::entity::index_of(w, cart) else { continue };
        let Some(&vid) = w.entities.list[ci].passengers.first() else { continue };
        let Some(vi) = redsim::entity::index_of(w, vid) else { continue };
        let (width, eye_h) = match &w.entities.list[vi].kind {
            EntityKind::Mob(m) => (m.dims().0 as f64, m.eye() as f64),
            _ => (0.6, 1.62),
        };
        for t in [0.1, 0.5, 0.9] {
            w.entities.list[ci].pos = along_rail(*c, state, t);
            redsim::entity::position_rider(w, ci, vi);
            let me = w.entities.list[vi].pos;
            let eye = Vec3::new(me.x, me.y + eye_h, me.z);
            let half = (width as f32 * 0.8f32) as f64 / 2.0;
            let bb = redsim::entity::Aabb::new(eye.x - half, eye.y - 5.0e-7, eye.z - half, eye.x + half, eye.y + 5.0e-7, eye.z + half);
            for y in bb.min_y.floor() as i32 - 1..=bb.max_y.floor() as i32 {
                for z in bb.min_z.floor() as i32..=bb.max_z.floor() as i32 {
                    for x in bb.min_x.floor() as i32..=bb.max_x.floor() as i32 {
                        let p = BlockPos::new(x, y, z);
                        let b = w.get(p);
                        if b.is_air() {
                            continue;
                        }
                        // (a suffocating block hurts through `isInWall`'s eye box; any collider
                        // blinds only when the eye point itself is inside it: the sight ray's
                        // clip starts there (`LivingEntity.hasLineOfSight`, ClipContext COLLIDER))
                        let suffocating = redsim::entity::mob::blocks::is_suffocating(&b);
                        let boxes = redsim::entity::collide::boxes_at(w, &b, p);
                        let hit = if suffocating { boxes.iter().any(|q| q.intersects(bb)) } else { boxes.iter().any(|q| q.min_x < eye.x && eye.x < q.max_x && q.min_y < eye.y && eye.y < q.max_y && q.min_z < eye.z && eye.z < q.max_z) };
                        if !hit {
                            continue;
                        }
                        if !hits.iter().any(|h: &EyeHit| h.cell == *c && h.block == p) {
                            hits.push(EyeHit { cell: *c, eye, block: p, state: b.to_string(), suffocating });
                        }
                    }
                }
            }
        }
        let _ = redsim::entity::kill(w, vid);
        let _ = redsim::entity::kill(w, cart);
        w.entities.list.retain(|e| !e.removed);
    }
    Ok((rails.len(), hits))
}

/// The eye check's lines: the suffocation gate (a failing line) and the blind places (information).
pub fn eye_lines(places: usize, hits: &[EyeHit], show: usize) -> (Vec<String>, bool) {
    let suff: Vec<&EyeHit> = hits.iter().filter(|h| h.suffocating).collect();
    let blind: Vec<&EyeHit> = hits.iter().filter(|h| !h.suffocating).collect();
    let mut lines = vec![format!(
        "rider eyes (every rail cell, the rider seated at three points; the census: no suffocating block at a rider's eye): {} of {places} places with a suffocating block at the eye{}; {} blind places (the eye inside a collision shape that doesn't suffocate: the sight ray starts inside it)",
        suff.iter().map(|h| h.cell).collect::<std::collections::HashSet<_>>().len(),
        if suff.is_empty() { String::new() } else { format!(" ({})", suff.iter().take(8).map(|h| format!("{},{},{} at {},{},{} {}", h.cell.x, h.cell.y, h.cell.z, h.block.x, h.block.y, h.block.z, h.state.trim_start_matches("minecraft:").split('[').next().unwrap_or(""))).collect::<Vec<_>>().join("; ")) },
        blind.iter().map(|h| h.cell).collect::<std::collections::HashSet<_>>().len()
    )];
    for h in blind.iter().take(show.max(1) * 10) {
        lines.push(format!("  BLIND {},{},{}: the eye at {:.2},{:.2},{:.2} in {} at {},{},{}", h.cell.x, h.cell.y, h.cell.z, h.eye.x, h.eye.y, h.eye.z, h.state, h.block.x, h.block.y, h.block.z));
    }
    (lines, !suff.is_empty())
}

/// The zombies where they sit (ironspec Nov 11 10:54: "every position the zombie can take in its
/// post, the boat's drift and the zombie's seat height"): each zombie of the settled world at its
/// own position (a post's zombie seated in its boat, the seat's height included; a boat on its
/// glass has nothing to move it, so the seat is the post's one position); the design's entity lines
/// only when the world holds none.
pub fn zombies_seated(w: &redsim::World, spec: &FarmSpec) -> Vec<Vec3> {
    let v: Vec<Vec3> = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .filter_map(|e| {
            let EntityKind::Mob(m) = &e.kind else { return None };
            if m.ty != redsim::entity::mob::types::MobType::Zombie {
                return None;
            }
            // (seated in a boat: the boat's y plus its passenger attachment, then the zombie's
            // ridingOffset, as `Entity.positionRider` orders them; redsim's `position_rider`
            // seats only cart passengers, so the seat is taken here)
            let vehicle = e.vehicle.and_then(|id| w.entities.list.iter().find(|x| x.id == id && !x.removed));
            Some(match vehicle.and_then(|b| redsim::entity::mob::rider_seat(&b.kind, m).map(|s| (b.pos, s))) {
                Some((bp, (seat, off))) => Vec3::new(bp.x, (bp.y + seat) + off, bp.z),
                None => e.pos,
            })
        })
        .collect();
    if !v.is_empty() {
        return v;
    }
    // (the check's world holds no entities of the design (they arrive later): each zombie line at
    // its own position, and a zombie riding a boat or cart seated: the vehicle's passenger
    // attachment (a boat's 0.5625 / 3, a cart's 0.1875 for a non-villager) plus the zombie's
    // ridingOffset (-0.7), the boat's y first)
    let off = redsim::entity::mob::types::MobType::Zombie.def().riding_offset as f64;
    spec.entities
        .iter()
        .filter_map(|(ty, p, nbt)| {
            if ty.ends_with("zombie") {
                return Some(Vec3::new(p[0], p[1], p[2]));
            }
            let rides = nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie\""));
            if rides && ty.ends_with("boat") {
                return Some(Vec3::new(p[0], (p[1] + (0.5625f32 / 3.0f32) as f64) + off, p[2]));
            }
            if rides && ty.ends_with("minecart") {
                return Some(Vec3::new(p[0], (p[1] + 0.1875f32 as f64) + off, p[2]));
            }
            rides.then(|| Vec3::new(p[0], p[1], p[2]))
        })
        .collect()
}

/// A cart's position at fraction `t` along rail `c` (shape from `state`): along the rail's axis,
/// on a slope at its height there (the cart rides 0.0625 over the rail's surface); a curve's centre.
pub fn along_rail(c: BlockPos, state: &str, t: f64) -> Vec3 {
    let (x, y, z) = (c.x as f64, c.y as f64 + 0.0625, c.z as f64);
    if state.contains("ascending_east") {
        Vec3::new(x + t, y + t, z + 0.5)
    } else if state.contains("ascending_west") {
        Vec3::new(x + t, y + 1.0 - t, z + 0.5)
    } else if state.contains("ascending_south") {
        Vec3::new(x + 0.5, y + t, z + t)
    } else if state.contains("ascending_north") {
        Vec3::new(x + 0.5, y + 1.0 - t, z + t)
    } else if state.contains("east_west") {
        Vec3::new(x + t, y, z + 0.5)
    } else if state.contains("north_south") {
        Vec3::new(x + 0.5, y, z + t)
    } else {
        Vec3::new(x + 0.5, y, z + 0.5)
    }
}

/// The job-site blocks and bells of `spec` (each a SetWalkTargetFromBlockMemory target within 48 for
/// a villager holding it).
pub fn job_and_bell_blocks(spec: &FarmSpec) -> Vec<(BlockPos, String)> {
    const JOBS: [&str; 14] = ["bell", "composter", "lectern", "barrel", "blast_furnace", "brewing_stand", "cartography_table", "cauldron", "fletching_table", "grindstone", "loom", "smithing_table", "smoker", "stonecutter"];
    spec.blocks
        .iter()
        .filter_map(|(p, b)| {
            let s = b.to_string();
            let name = s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
            JOBS.contains(&name.as_str()).then_some((*p, name))
        })
        .collect()
}

/// The check over every rail cell of `spec` (its own world built and settled; a rider seated in a
/// cart there, at three points along the rail's axis), and on foot at every rail cell within 1 of the
/// bed `home` (a dropped napper stands there) and at each `foot` cell; the bed `home` for REST's walk.
pub fn check(spec: &FarmSpec, home: Option<BlockPos>, foot: &[BlockPos], panic_boxes: &[([i32; 3], [i32; 3])]) -> Result<Vec<Place>, String> {
    check_with(spec, home, foot, panic_boxes, true)
}

/// [`check`]; `continuous`: each rider place's least zombie distance also over the cart's whole
/// path through the cell (the default), else only at the sampled points.
pub fn check_with(spec: &FarmSpec, home: Option<BlockPos>, foot: &[BlockPos], panic_boxes: &[([i32; 3], [i32; 3])], continuous: bool) -> Result<Vec<Place>, String> {
    let continuous_off = !continuous;
    use redsim::entity::mob::ai::goal::stroll;
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let foes = zombies_seated(w, spec);
    let rails: Vec<(BlockPos, String)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    let rider_at = |c: BlockPos| Vec3::new(c.x as f64 + 0.5, c.y as f64 + 0.0625, c.z as f64 + 0.5);
    // the places: (cell, on foot, the points tried)
    let mut places: Vec<(BlockPos, bool, Vec<Vec3>)> = Vec::new();
    for (c, state) in &rails {
        let along = |t: f64| along_rail(*c, state, t);
        places.push((*c, false, vec![along(0.1), along(0.5), along(0.9)]));
    }
    for (c, _) in &rails {
        if home.is_some_and(|h| (c.x - h.x).abs().max((c.z - h.z).abs()) <= 1 && (c.y - h.y).abs() <= 1) {
            places.push((*c, true, vec![Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5)]));
        }
    }
    for c in foot {
        places.push((*c, true, vec![Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5)]));
    }
    let mut out = Vec::new();
    for (cell, on_foot, points) in places {
        let p0 = points[points.len() / 2];
        let (vid, cart) = if on_foot {
            (redsim::entity::spawn(w, "minecraft:villager", [p0.x, p0.y, p0.z], Some("{PersistenceRequired:1b}"))?, None)
        } else {
            let cart = redsim::entity::spawn(w, "minecraft:minecart", [p0.x, p0.y, p0.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
            let Some(ci) = redsim::entity::index_of(w, cart) else { continue };
            let Some(&vid) = w.entities.list[ci].passengers.first() else { continue };
            (vid, Some(cart))
        };
        let Some(i) = redsim::entity::index_of(w, vid) else { continue };
        if let Some(h) = home
            && let EntityKind::Mob(m) = &mut w.entities.list[i].kind
            && let Some(b) = m.brain.as_deref_mut()
        {
            b.home = Some(h);
        }
        // (put the rider at point `p`: the cart there and the rider seated, or the villager standing)
        let put = |w: &mut redsim::World, p: Vec3| {
            match cart.and_then(|c| redsim::entity::index_of(w, c)) {
                Some(ci) => {
                    w.entities.list[ci].pos = p;
                    redsim::entity::position_rider(w, ci, i);
                }
                None => {
                    // (a villager standing on the floor is on the ground: `canUpdatePath` needs it,
                    // else no path is made and every target reads unreachable)
                    w.entities.list[i].pos = p;
                    w.entities.list[i].on_ground = true;
                }
            }
            w.entities.list[i].pos
        };
        let reaches = |w: &mut redsim::World, t: BlockPos| -> bool {
            redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
                let mut copy = nav.clone();
                copy.create_path_pos(w, nm, t, 0, arena).is_some_and(|p| p.reached)
            })
        };
        let merge = |into: &mut Vec<(BlockPos, f64)>, from: Vec<(BlockPos, f64)>| {
            for (c, s) in from {
                match into.iter_mut().find(|(x, _)| *x == c) {
                    Some((_, t)) => *t = t.max(s),
                    None => into.push((c, s)),
                }
            }
        };
        let (mut flee_s, mut panic_s, mut idle_s) = (Vec::new(), Vec::new(), Vec::new());
        let (mut flee_from, mut panic_from, mut seen, mut village) = (0, 0, 0, false);
        // (the 6-8 rule, ironspec Nov 11 10:18: the nearest hostile's distance at each point, the
        // least and the greatest over the place's points, and that hostile)
        let mut near: Option<(f64, f64, Vec3)> = None;
        // (the closest pair: the distance, the rider's position, the zombie's; and whether every
        // point sees a zombie within PANIC_BAR)
        let mut worst: Option<(f64, Vec3, Vec3)> = None;
        let mut panic_ok = true;
        for &p in &points {
            let me = put(w, p);
            let eye = Vec3::new(me.x, me.y + 1.62, me.z);
            let dist = |f: &Vec3| ((me.x - f.x).powi(2) + (me.y - f.y).powi(2) + (me.z - f.z).powi(2)).sqrt();
            if let Some(f) = foes.iter().min_by(|a, b| dist(a).total_cmp(&dist(b))) {
                let d = dist(f);
                near = Some(match near {
                    None => (d, d, *f),
                    Some((lo, hi, h)) => (lo.min(d), hi.max(d), if d < lo { *f } else { h }),
                });
                if worst.is_none_or(|(wd, _, _)| d < wd) {
                    worst = Some((d, me, *f));
                }
            }
            panic_ok &= foes.iter().any(|f| dist(f) <= PANIC_BAR && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z)));
            let near6: Vec<Vec3> = foes.iter().copied().filter(|f| dist(f) < 6.0).collect();
            let near8: Vec<Vec3> = foes.iter().copied().filter(|f| dist(f) <= 8.0).collect();
            flee_from = flee_from.max(near6.len());
            panic_from = panic_from.max(near8.len());
            seen = seen.max(near8.iter().filter(|f| redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z))).count());
            // PANIC's flee: every hostile within 6 (the largest share over them and the points; the
            // exact enumeration's cells a share of 0 misses kept at 0: an offset of exactly 16)
            for f in &near6 {
                merge(&mut flee_s, stroll::flee_pick_shares(w, i, 16.0, 7, *f));
                let edge: Vec<(BlockPos, f64)> = stroll::flee_candidates(w, i, 16.0, 7, *f).into_iter().map(|c| (c, 0.0)).collect();
                merge(&mut flee_s, edge);
            }
            // VillageBoundRandomStroll (PANIC's 2/2, IDLE's 10/7): in a village a land draw around;
            // else DefaultRandomPos toward the section closest to the village within 2, or a land
            // draw when that section is its own
            let me_b = w.entities.list[i].block_position();
            let v = redsim::entity::mob::ai::poi::is_village(w, me_b);
            village |= v;
            let s = (me_b.x.div_euclid(16), me_b.y.div_euclid(16), me_b.z.div_euclid(16));
            let best = if v { s } else { redsim::entity::mob::ai::poi::section_closest_to_village(w, s, 2) };
            let village_stroll = |w: &redsim::World, h: f64, vv: i32| {
                if best != s {
                    let c = ((best.0 * 16 + 8) as f64 + 0.5, (best.2 * 16 + 8) as f64 + 0.5);
                    stroll::pick_shares(w, i, h, vv, Some((c.0 - me.x, c.1 - me.z)), false, 10)
                } else {
                    stroll::pick_shares(w, i, h, vv, None, true, 10)
                }
            };
            if !near8.is_empty() {
                merge(&mut panic_s, village_stroll(w, 2.0, 2));
            }
            merge(&mut idle_s, village_stroll(w, 10.0, 7));
        }
        let me = put(w, p0);
        let mut judge = |w: &mut redsim::World, shares: Vec<(BlockPos, f64)>| -> Source {
            let cells = shares.len();
            let total: f64 = shares.iter().map(|(_, s)| *s).sum();
            let mut unreached: Vec<(BlockPos, f64)> = shares.into_iter().filter(|(c, _)| !reaches(w, *c)).collect();
            unreached.sort_by(|a, b| b.1.total_cmp(&a.1));
            let reached_share = (total - unreached.iter().map(|(_, s)| *s).sum::<f64>()).max(0.0);
            Source { cells, unreached, reached_share }
        };
        let flee = judge(w, flee_s);
        let panic_stroll = judge(w, panic_s);
        let stroll_src = judge(w, idle_s);
        // IDLE's walks to another villager within 8 (InteractWith; SetWalkTargetFromLookTarget): every
        // other rider place within 8, its rider's cell
        let mut others: Vec<(BlockPos, f64)> = Vec::new();
        for (o, _) in &rails {
            let q = rider_at(*o);
            if *o != cell && ((me.x - q.x).powi(2) + (me.y - q.y).powi(2) + (me.z - q.z).powi(2)).sqrt() <= 8.0 {
                others.push((*o, 1.0));
            }
        }
        let others = judge(w, others);
        let home_reached = home.map(|h| reaches(w, h));
        // (the least distance over the continuous track path, ironspec Nov 11 12:23: the cart's
        // path through the cell as a segment (a curve's between its two exits, a slope's rising),
        // the rider's seat offset added, the exact least distance to each zombie's seated position;
        // the sampled points' least kept when it is lower, as for an on-foot place)
        if !on_foot && !continuous_off
            && let Some((_, state)) = rails.iter().find(|(c, _)| *c == cell)
            && let Some((a, b)) = rail_segment(cell, state)
        {
            let cart_mid = along_rail(cell, state, 0.5);
            let rider_mid = put(w, cart_mid);
            let off = Vec3::new(rider_mid.x - cart_mid.x, rider_mid.y - cart_mid.y, rider_mid.z - cart_mid.z);
            let (a, b) = (Vec3::new(a.x + off.x, a.y + off.y, a.z + off.z), Vec3::new(b.x + off.x, b.y + off.y, b.z + off.z));
            for f in &foes {
                let (d, q) = seg_dist(a, b, *f);
                if near.is_none_or(|n| d < n.0) {
                    near = Some(match near {
                        None => (d, d, *f),
                        Some((_, hi, _)) => (d, hi.max(d), *f),
                    });
                }
                if worst.is_none_or(|(wd, _, _)| d < wd) {
                    worst = Some((d, q, *f));
                }
            }
        }
        let panic_declared = panic_boxes.iter().any(|(a, b)| (a[0]..=b[0]).contains(&cell.x) && (a[1]..=b[1]).contains(&cell.y) && (a[2]..=b[2]).contains(&cell.z));
        out.push(Place { cell, on_foot, flee_from, panic_from, seen, village, flee, panic_stroll, stroll: stroll_src, others, home_reached, near, worst, panic_declared, panic_ok });
        let _ = redsim::entity::kill(w, vid);
        if let Some(c) = cart {
            let _ = redsim::entity::kill(w, c);
        }
        w.entities.list.retain(|e| !e.removed);
    }
    Ok(out)
}

/// `mctas mobfarm flee-check <design>`: the report, `show` cells a place; the verdicts last.
pub fn report(places: &[Place], jobs: &[(BlockPos, String)], show: usize) -> Vec<String> {
    report_with(places, jobs, show, false)
}

/// [`report`], with `clearing`: every place's clearing line (what can clear a stamp there).
pub fn report_with(places: &[Place], jobs: &[(BlockPos, String)], show: usize, clearing: bool) -> Vec<String> {
    let mut lines = clearing_lines(places, clearing);
    lines.extend(report_body(places, jobs, show));
    lines
}

/// What can clear a rider's stamp at each place (ironspec Nov 11 11:07; design numbers, not a
/// bar): panicking (a zombie seen within 8), the panic stroll's cells within 2 and 2 its path
/// reaches; calm, the calm stroll's chance an event picks a reachable cell.
fn clearing_lines(places: &[Place], each: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let panicking = |p: &Place| p.seen > 0;
    let pan_ok = places.iter().filter(|p| panicking(p) && p.panic_stroll.cells > p.panic_stroll.unreached.len()).count();
    let pan_none = places.iter().filter(|p| panicking(p) && p.panic_stroll.cells <= p.panic_stroll.unreached.len()).count();
    let calm_ok = places.iter().filter(|p| !panicking(p) && p.stroll.reached_share > 1e-9).count();
    let calm_none = places.iter().filter(|p| !panicking(p) && p.stroll.reached_share <= 1e-9).count();
    lines.push(format!(
        "what can clear a stamp, per place (design numbers, not a bar; the oldest-count rows decide): {} panicking places (a zombie seen within 8) with a reachable panic-stroll cell, {} with none; {} calm places whose calm stroll can pick a reachable cell, {} with none",
        pan_ok, pan_none, calm_ok, calm_none
    ));
    for p in places.iter().filter(|p| each || (panicking(p) && p.panic_stroll.cells <= p.panic_stroll.unreached.len()) || (!panicking(p) && p.stroll.reached_share <= 1e-9)) {
        let tag = if each { "CLEARING" } else { "NO-CLEARING" };
        if panicking(p) {
            lines.push(format!("  {tag} {},{},{}{}: panicking ({} seen within 8); the panic stroll reaches {} of its {} cells (an event picks one {:.0}%)", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, p.seen, p.panic_stroll.cells - p.panic_stroll.unreached.len(), p.panic_stroll.cells, 100.0 * p.panic_stroll.reached_share));
        } else {
            lines.push(format!("  {tag} {},{},{}{}: calm (no zombie seen within 8); the calm stroll reaches {} of its {} cells (an event picks one {:.1}%)", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, p.stroll.cells - p.stroll.unreached.len(), p.stroll.cells, 100.0 * p.stroll.reached_share));
        }
    }
    lines
}

fn report_body(places: &[Place], jobs: &[(BlockPos, String)], show: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let fmt = |v: &[(BlockPos, f64)], pct: bool| {
        v.iter()
            .take(show)
            .map(|(c, s)| if pct { format!("{},{},{} ({:.3}%)", c.x, c.y, c.z, 100.0 * s) } else { format!("{},{},{}", c.x, c.y, c.z) })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut section = |name: &str, what: &str, pick: &dyn Fn(&Place) -> Option<&Source>, pct: bool| -> usize {
        let runs: Vec<&Place> = places.iter().filter(|p| pick(p).is_some()).collect();
        let bad: Vec<&Place> = runs.iter().copied().filter(|p| pick(p).is_some_and(|s| s.live() > 0)).collect();
        let edge = runs.iter().filter(|p| pick(p).is_some_and(|s| s.live() == 0 && !s.unreached.is_empty())).count();
        lines.push(format!("{name}: {what}: {} rider places where it runs, {} with cells it picks that no path reaches{}", runs.len(), bad.len(), if edge > 0 { format!(" ({edge} more with edge cells only)") } else { String::new() }));
        for p in &bad {
            let s = pick(p).expect("a source");
            lines.push(format!(
                "  {name} {},{},{}{} ({} hostile(s) within 6, {} within 8, {} in sight as placed): {} cells, {} unreached{}: {}",
                p.cell.x,
                p.cell.y,
                p.cell.z,
                if p.on_foot { " on foot" } else { "" },
                p.flee_from,
                p.panic_from,
                p.seen,
                s.cells,
                s.live(),
                if pct { format!(" ({:.3}% of events)", 100.0 * s.risk()) } else { String::new() },
                fmt(&s.unreached, pct)
            ));
        }
        bad.len()
    };
    let flee_bad = section("FLEE", "PANIC's SetWalkTargetAwayFrom (getPosAway 16/7, a hostile within 6)", &|p| (p.flee_from > 0).then_some(&p.flee), true);
    let panic_bad = section("PANIC-STROLL", "PANIC's VillageBoundRandomStroll (2/2)", &|p| (p.panic_from > 0).then_some(&p.panic_stroll), true);
    let stroll_bad = section("STROLL", "IDLE's VillageBoundRandomStroll (10/7; a land draw in a village, else toward the village's section)", &|p| Some(&p.stroll), true);
    let others_bad = section("VILLAGER", "IDLE's walks to another villager within 8 (InteractWith, SetWalkTargetFromLookTarget): each other rider place within 8", &|p| Some(&p.others), false);
    lines.push(format!(
        "job site / bell walks within 48: {}",
        if jobs.is_empty() { "no job-site block or bell in the design".to_string() } else { jobs.iter().map(|(p, n)| format!("{n} {},{},{}", p.x, p.y, p.z)).collect::<Vec<_>>().join(", ") }
    ));
    let homes: Vec<&Place> = places.iter().filter(|p| p.home_reached.is_some()).collect();
    if !homes.is_empty() {
        let reached: Vec<String> = homes.iter().filter(|p| p.home_reached == Some(true)).map(|p| format!("{},{},{}", p.cell.x, p.cell.y, p.cell.z)).collect();
        lines.push(format!(
            "REST's HOME walk: the bed reached from {} of {} rider places (from each other one a resting rider's CANT_REACH count runs: the brief's 1,000-gt line){}",
            reached.len(),
            homes.len(),
            if reached.is_empty() { String::new() } else { format!(": {}", reached.iter().take(12).cloned().collect::<Vec<_>>().join(" ")) }
        ));
    }
    let verdict = |n: usize| if n == 0 { "PASS".to_string() } else { format!("FAIL at {n} places") };
    // (the 6-8 rule, ironspec Nov 11 10:54 (scene flee-distance bridged): every zombie at least
    // FLEE_BAR (6.05) from every rider position, measured from where it sits in its post; a place
    // declared in `panic:` sees one within PANIC_BAR (7.95) from every point. Each place's nearest
    // zombie, least and greatest over its points, and the closest pair overall)
    let inside: Vec<&Place> = places.iter().filter(|p| p.near.is_some_and(|n| n.0 < FLEE_BAR)).collect();
    let beyond = places.iter().filter(|p| p.near.is_none_or(|n| n.1 > PANIC_BAR)).count();
    let declared: Vec<&Place> = places.iter().filter(|p| p.panic_declared).collect();
    let declared_bad: Vec<&Place> = declared.iter().copied().filter(|p| !p.panic_ok).collect();
    lines.push(format!(
        "the 6-8 rule (every zombie at least {FLEE_BAR} from every rider position, where it sits in its post; a declared panic place sees one within {PANIC_BAR}): {} rider places inside {FLEE_BAR}, {} whose nearest zombie is beyond {PANIC_BAR} somewhere, {} at {FLEE_BAR}..{PANIC_BAR} throughout; {} places declared in panic:, {} of them without a seen zombie within {PANIC_BAR} at some point",
        inside.len(),
        beyond,
        places.iter().filter(|p| p.near.is_some_and(|n| n.0 >= FLEE_BAR && n.1 <= PANIC_BAR)).count(),
        declared.len(),
        declared_bad.len()
    ));
    if let Some((p, (d, me, f))) = places.iter().filter_map(|p| p.worst.map(|w| (p, w))).min_by(|a, b| a.1.0.total_cmp(&b.1.0)) {
        lines.push(format!("the closest rider-zombie pair: {d:.3}, the rider at {:.3},{:.3},{:.3} (place {},{},{}{}), the zombie at {:.3},{:.3},{:.3}", me.x, me.y, me.z, p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, f.x, f.y, f.z));
    }
    for p in inside.iter().take(show.max(1) * 20) {
        let (lo, hi, h) = p.near.expect("a place inside the bar has a zombie");
        lines.push(format!("  INSIDE {},{},{}{}: the nearest zombie {lo:.2}..{hi:.2} (at {:.2},{:.2},{:.2})", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, h.x, h.y, h.z));
    }
    for p in declared_bad.iter().take(show.max(1) * 20) {
        lines.push(format!("  PANIC-UNCOVERED {},{},{}: declared in panic:, no seen zombie within {PANIC_BAR} at some point (nearest {})", p.cell.x, p.cell.y, p.cell.z, p.near.map_or("none".to_string(), |n| format!("{:.2}..{:.2}", n.0, n.1))));
    }
    for p in places.iter().filter(|p| p.near.is_none_or(|n| n.1 > PANIC_BAR)).take(show.max(1) * 20) {
        match p.near {
            Some((lo, hi, h)) => lines.push(format!("  BEYOND {},{},{}{}: the nearest zombie {lo:.2}..{hi:.2} (at {:.2},{:.2},{:.2}), {} seen within 8", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, h.x, h.y, h.z, p.seen)),
            None => lines.push(format!("  BEYOND {},{},{}: no zombie in the design", p.cell.x, p.cell.y, p.cell.z)),
        }
    }
    lines.push(format!("the 6-8 gate (a place inside {FLEE_BAR}, or a declared panic place beyond {PANIC_BAR} or out of sight): {}", verdict(inside.len() + declared_bad.len())));
    lines.push(format!(
        "verdicts (every cell a source can pick reachable wherever it runs): flee {}, panic stroll {}, idle stroll {}, walks to villagers {}",
        verdict(flee_bad),
        verdict(panic_bad),
        verdict(stroll_bad),
        verdict(others_bad)
    ));
    lines
}
