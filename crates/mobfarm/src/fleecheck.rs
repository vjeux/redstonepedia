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
use std::collections::HashMap;

/// One source's result at a place: its cells, and those with no complete path with the chance one
/// event picks each (a share of 0: an edge cell).
#[derive(Default)]
pub struct Source {
    pub cells: usize,
    pub unreached: Vec<(BlockPos, f64)>,
    /// The chance one event picks a cell the rider's path reaches (what clears a stamp).
    pub reached_share: f64,
    /// The cells a complete path reaches, with the chance one event picks each.
    pub reached: Vec<(BlockPos, f64)>,
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

/// The pickup-reach check (ironspec Nov 11 14:32: the brief keeps zombies and boats out of empty
/// carts' pickup reach): every zombie post's boat box and seated zombie's box against an empty
/// cart's pickup box (its 0.98 x 0.7 box inflated 0.2 sideways, `OldMinecartBehavior.
/// pushAndPickupEntities`) at every point along every rail cell (along each shape's segment, 21
/// points); the least gap per post and a failing line on any overlap. With `control`, a boated
/// zombie set 0.1 beside the first rail cell's cart box (inside its pickup reach) must fail it.
pub fn pickup_check(spec: &FarmSpec, control: bool) -> (Vec<String>, bool) {
    type Bx = ([f64; 3], [f64; 3]);
    let off = redsim::entity::mob::types::MobType::Zombie.def().riding_offset as f64;
    let mut posts: Vec<([f64; 3], Vec<(&'static str, Bx)>)> = Vec::new();
    let mut add_post = |p: [f64; 3], boat: bool| {
        let mut b = Vec::new();
        let feet = if boat {
            b.push(("boat", ([p[0] - 0.6875, p[1], p[2] - 0.6875], [p[0] + 0.6875, p[1] + 0.5625, p[2] + 0.6875])));
            (p[1] + (0.5625f32 / 3.0f32) as f64) + off
        } else {
            p[1]
        };
        b.push(("zombie", ([p[0] - 0.3, feet, p[2] - 0.3], [p[0] + 0.3, feet + 1.95, p[2] + 0.3])));
        posts.push((p, b));
    };
    for (ty, p, nbt) in &spec.entities {
        let rides = nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie\""));
        if ty.ends_with("boat") && rides {
            add_post(*p, true);
        } else if ty.ends_with("zombie") {
            add_post(*p, false);
        }
    }
    let rails: Vec<(BlockPos, String)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    let samples = |c: BlockPos, st: &str| -> Vec<Vec3> {
        match rail_segment(c, st) {
            Some((a, b)) => (0..=20).map(|i| { let t = i as f64 / 20.0; Vec3::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t) }).collect(),
            None => (0..=20).map(|i| along_rail(c, st, i as f64 / 20.0)).collect(),
        }
    };
    if control && let Some((c, st)) = rails.first() {
        let m = along_rail(*c, st, 0.5);
        add_post([m.x + 0.49 + 0.1 + 0.6875, m.y, m.z], true);
    }
    let gap = |a: &Bx, b: &Bx| -> f64 {
        let d: Vec<f64> = (0..3).map(|k| (a.0[k] - b.1[k]).max(b.0[k] - a.1[k])).collect();
        if d.iter().all(|v| *v < 0.0) { d.iter().cloned().fold(f64::NEG_INFINITY, f64::max) } else { d.iter().map(|v| v.max(0.0).powi(2)).sum::<f64>().sqrt() }
    };
    let mut worst: Vec<(f64, [f64; 3], &'static str, BlockPos)> = Vec::new();
    for (p, boxes) in &posts {
        let mut best = (f64::INFINITY, "-", BlockPos::new(0, 0, 0));
        for (c, st) in &rails {
            for m in samples(*c, st) {
                let pick: Bx = ([m.x - 0.69, m.y, m.z - 0.69], [m.x + 0.69, m.y + 0.7, m.z + 0.69]);
                for (what, b) in boxes {
                    let g = gap(b, &pick);
                    if g < best.0 {
                        best = (g, what, *c);
                    }
                }
            }
        }
        worst.push((best.0, *p, best.1, best.2));
    }
    let bad: Vec<&(f64, [f64; 3], &str, BlockPos)> = worst.iter().filter(|w| w.0 < 0.0).collect();
    let least = worst.iter().cloned().min_by(|a, b| a.0.total_cmp(&b.0));
    let mut lines = vec![format!(
        "pickup reach (each post's boat and zombie boxes against an empty cart's pickup box, its box inflated 0.2 sideways, along every rail cell): {} posts{}{}{}",
        posts.len(),
        least.map_or(String::new(), |l| format!(", the least gap {:.3} ({} of the post at {:.2},{:.2},{:.2}, at rail {},{},{})", l.0, l.2, l.1[0], l.1[1], l.1[2], l.3.x, l.3.y, l.3.z)),
        if bad.is_empty() { String::new() } else { format!("; {} overlap -- the static gate FAILS", bad.len()) },
        if control { " [control: a boated zombie 0.1 beside the first rail cell's cart]" } else { "" }
    )];
    for w in &worst {
        lines.push(format!("  post {:.2},{:.2},{:.2}: least {:.3} ({} to rail {},{},{}){}", w.1[0], w.1[1], w.1[2], w.0, w.2, w.3.x, w.3.y, w.3.z, if w.0 < 0.0 { "  OVERLAP" } else { "" }));
    }
    (lines, !bad.is_empty())
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
    check_claims(spec, home, foot, panic_boxes, continuous, &[])
}

/// [`check_with`] with POI tickets taken first (9fc3155f, ironspec Nov 11 19:18's bed states): one
/// ticket on each `claims` POI (the shared bed taken, a sentinel's bed), the village centres noted,
/// so the village bias of the strolls is the one that bed state gives (`poi::take`,
/// `poi::note_village_centres`: world state, the brain model unchanged).
pub fn check_claims(spec: &FarmSpec, home: Option<BlockPos>, foot: &[BlockPos], panic_boxes: &[([i32; 3], [i32; 3])], continuous: bool, claims: &[BlockPos]) -> Result<Vec<Place>, String> {
    check_inner(spec, home, foot, panic_boxes, continuous, claims, &[], None, &[])
}

/// [`check_claims`] with declared pillagers (9fc3155f, ironspec Nov 11 22:53's S2 scarer; redsim
/// does not spawn a pillager, so each is a position, its feet, not an entity): a hostile within
/// PILLAGER_BAR (`VillagerHostilesSensor`'s 15) of a rider point panics it when seen (the rider's
/// eye to the pillager's, 1.62 up); within 6 it is fled from, as a zombie.
pub fn check_pillagers(spec: &FarmSpec, home: Option<BlockPos>, foot: &[BlockPos], panic_boxes: &[([i32; 3], [i32; 3])], continuous: bool, claims: &[BlockPos], pills: &[Vec3]) -> Result<Vec<Place>, String> {
    check_inner(spec, home, foot, panic_boxes, continuous, claims, pills, None, &[])
}

/// A pillager's panic distance (`VillagerHostilesSensor.ACCEPTABLE_DISTANCE_FROM_HOSTILES`).
pub const PILLAGER_BAR: f64 = 15.0;

/// A pillager's eye over its feet (G1's review: no `eyeHeight` on its builder, so 0.85 x 1.95).
pub const PILLAGER_EYE: f64 = 0.85 * 1.95;

/// A pillager seated in a boat: its feet over the boat's y (the boat's passenger attachment
/// 0.5625 / 3, then the pillager's `ridingOffset(-0.6)`; G1's review: searched standing, it sat
/// ~0.37 too high).
pub const PILLAGER_SEAT: f64 = (0.5625f32 / 3.0f32) as f64 - 0.6;

/// An iron golem's eye over its feet (EntityTypes.IRON_GOLEM 1.4 x 2.7, eye 2.7 x 0.85).
pub const GOLEM_EYE: f64 = 2.295;

/// The block cells a straight segment passes through, its two end cells left out (sampled at
/// 1/40 block: a visor's candidate cells on a golem's sight ray).
fn ray_cells(a: Vec3, b: Vec3) -> Vec<BlockPos> {
    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
    let n = ((len * 40.0).ceil() as usize).max(1);
    let cell = |p: Vec3| BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let (ca, cb) = (cell(a), cell(b));
    let mut v: Vec<BlockPos> = Vec::new();
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let c = cell(Vec3::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t));
        if c != ca && c != cb && !v.contains(&c) {
            v.push(c);
        }
    }
    v
}

/// An iron golem's follow range: it targets an Enemy it sees within it (IronGolem's
/// NearestAttackableTargetGoal) and keeps the target while within it.
pub const GOLEM_TARGET: f64 = 16.0;

/// [`check_claims`], only the rider place at `only` (the others skipped: the fix search's runs).
fn check_inner(spec: &FarmSpec, home: Option<BlockPos>, foot: &[BlockPos], panic_boxes: &[([i32; 3], [i32; 3])], continuous: bool, claims: &[BlockPos], pills: &[Vec3], only: Option<BlockPos>, at: &[Vec3]) -> Result<Vec<Place>, String> {
    let continuous_off = !continuous;
    use redsim::entity::mob::ai::goal::stroll;
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    for c in claims {
        redsim::entity::mob::ai::poi::take(w, *c);
    }
    if !claims.is_empty() {
        redsim::entity::mob::ai::poi::note_village_centres(w);
    }
    let foes = zombies_seated(w, spec);
    let rails: Vec<(BlockPos, String)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    // (a rider's feet are its cart's position: 26.3's LOWERED_PASSENGER_ATTACHMENT for a villager,
    // tick-exact's official print Nov 12 07:20; the place's own rider is seated by position_rider)
    let rider_at = |c: BlockPos| Vec3::new(c.x as f64 + 0.5, c.y as f64 + 0.0625, c.z as f64 + 0.5);
    // the places: (cell, on foot, the points tried)
    let mut places: Vec<(BlockPos, bool, Vec<Vec3>)> = Vec::new();
    // (ironspec Nov 12 07:08, ironfarm's real queue rest positions: one place at each explicit
    // cart position, its cell the one the cart stands in)
    for p in at {
        places.push((BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32), false, vec![*p]));
    }
    for (c, state) in rails.iter().filter(|_| at.is_empty()) {
        if only.is_some_and(|o| o != *c) {
            continue;
        }
        let along = |t: f64| along_rail(*c, state, t);
        places.push((*c, false, vec![along(0.1), along(0.5), along(0.9)]));
    }
    for (c, _) in rails.iter().filter(|_| at.is_empty()) {
        if home.is_some_and(|h| (c.x - h.x).abs().max((c.z - h.z).abs()) <= 1 && (c.y - h.y).abs() <= 1) {
            places.push((*c, true, vec![Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5)]));
        }
    }
    for c in foot.iter().filter(|_| at.is_empty()) {
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
                    // (the rider's fluid state where it now sits: inside a water column the path
                    // starts at the water's surface, ff1ca056a's official row)
                    redsim::entity::fluid_push::update_fluid_interaction(w, i);
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
            panic_ok &= foes.iter().any(|f| dist(f) <= PANIC_BAR && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z)))
                || pills.iter().any(|f| dist(f) <= PILLAGER_BAR - 0.05 && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + PILLAGER_EYE, f.z)));
            let near6: Vec<Vec3> = foes.iter().chain(pills.iter()).copied().filter(|f| dist(f) < 6.0).collect();
            let near8: Vec<Vec3> = foes.iter().copied().filter(|f| dist(f) <= 8.0).collect();
            let near15: Vec<Vec3> = pills.iter().copied().filter(|f| dist(f) <= PILLAGER_BAR).collect();
            flee_from = flee_from.max(near6.len());
            panic_from = panic_from.max(near8.len() + near15.len());
            seen = seen.max(
                near8.iter().filter(|f| redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z))).count()
                    + near15.iter().filter(|f| redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + PILLAGER_EYE, f.z))).count(),
            );
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
            // (explicit positions, `at`: the panic stroll judged there whatever the range, for
            // `clear_all`'s carried panic: NEAREST_HOSTILE held between the sensor's scans)
            if !near8.is_empty() || !near15.is_empty() || !at.is_empty() {
                merge(&mut panic_s, village_stroll(w, 2.0, 2));
            }
            // (IDLE's stroll, explicit positions included: ironspec Nov 12 10:24, every walk-target
            // source the brain can run at each position)
            merge(&mut idle_s, village_stroll(w, 10.0, 7));
        }
        let me = put(w, p0);
        let mut judge = |w: &mut redsim::World, shares: Vec<(BlockPos, f64)>| -> Source {
            let cells = shares.len();
            let total: f64 = shares.iter().map(|(_, s)| *s).sum();
            let (mut reached, mut unreached): (Vec<(BlockPos, f64)>, Vec<(BlockPos, f64)>) = shares.into_iter().partition(|(c, _)| reaches(w, *c));
            unreached.sort_by(|a, b| b.1.total_cmp(&a.1));
            reached.sort_by(|a, b| b.1.total_cmp(&a.1));
            let reached_share = (total - unreached.iter().map(|(_, s)| *s).sum::<f64>()).max(0.0);
            Source { cells, unreached, reached_share, reached }
        };
        let flee = judge(w, flee_s);
        let panic_stroll = judge(w, panic_s);
        let stroll_src = judge(w, idle_s);
        // IDLE's walks to another villager within 8 (InteractWith; SetWalkTargetFromLookTarget): every
        // other rider place within 8 that the rider sees (both take a visible villager: the living
        // entities sensor's visible list), its rider's cell
        let mut others: Vec<(BlockPos, f64)> = Vec::new();
        for (o, _) in rails.iter() {
            let q = rider_at(*o);
            if *o != cell
                && ((me.x - q.x).powi(2) + (me.y - q.y).powi(2) + (me.z - q.z).powi(2)).sqrt() <= 8.0
                && redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(me.x, me.y + 1.62, me.z), Vec3::new(q.x, q.y + 1.62, q.z))
            {
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

/// The stamp-arming class of each place (9fc3155f, ironspec Nov 11 19:18; a prediction for the
/// bills, not a bar): the sources its activity runs (panicking, a zombie seen within 8: PANIC's
/// flee while one is within 6 and its stroll 2/2; calm: IDLE's stroll 10/7 and its walks to riders
/// within 8), a live draw with no complete path ARMS the CANT_REACH stamp, a live draw with one
/// CLEARS it: "arms and clears", "arms, never clears" or "never arms". One line a place, then the
/// counts.
/// A place's stamp sources by its activity (panicking: PANIC's stroll 2/2 and its flee while a
/// hostile is within 6; calm: IDLE's stroll 10/7 and its walks to riders within 8).
fn stamp_sources(p: &Place) -> Vec<(&'static str, &Source)> {
    if p.seen > 0 {
        let mut v = vec![("panic stroll", &p.panic_stroll)];
        if p.flee_from > 0 {
            v.push(("flee", &p.flee));
        }
        v
    } else {
        vec![("calm stroll", &p.stroll), ("walks to riders", &p.others)]
    }
}

/// A place's stamp-arming class: a live draw with no complete path arms the CANT_REACH stamp, a
/// live draw with one clears it.
/// The path-integrated stamp age (ironspec Nov 11 20:39, after day-1's e6 trace): the stamp
/// (CANT_REACH_WALK_TARGET_SINCE) is set once at the first unreachable walk target, never refreshed,
/// erased only on a reachable one, so it ages along the rider's path. A panicking cell (a zombie
/// seen within 8) with draws (PANIC's flee while one is within 6, its stroll 2/2) none of which a
/// path reaches ARMS; a panicking cell with a reachable draw CLEARS; calm cells and cells with no
/// draw do neither (no new walk target, no erase). Per cell: (arms, clears, the arming source).
pub fn stamp_path_roles(places: &[Place]) -> std::collections::HashMap<BlockPos, (bool, bool, &'static str)> {
    let mut m = std::collections::HashMap::new();
    for p in places.iter().filter(|p| !p.on_foot) {
        if p.seen == 0 {
            m.insert(p.cell, (false, false, "calm"));
            continue;
        }
        let mut cells = p.panic_stroll.cells;
        let mut reached = p.panic_stroll.reached_share;
        let mut src = "panic stroll";
        if p.flee_from > 0 {
            cells += p.flee.cells;
            reached += p.flee.reached_share;
            src = if p.panic_stroll.cells > 0 { "flee + panic stroll" } else { "flee" };
        }
        m.insert(p.cell, (cells > 0 && reached <= 1e-9, reached > 1e-9, src));
    }
    m
}

/// A path position's rail cell: the one it stands in or the one below (a slope's cart rides up to a
/// block over its cell), with its role.
pub fn path_cell(roles: &std::collections::HashMap<BlockPos, (bool, bool, &'static str)>, p: Vec3) -> Option<(BlockPos, (bool, bool, &'static str))> {
    let c0 = BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let c1 = BlockPos::new(c0.x, c0.y - 1, c0.z);
    [c0, c1].into_iter().find_map(|c| roles.get(&c).map(|r| (c, *r)))
}

/// One stamp along a path: armed at tick `t0` in `cell0` (its source), cleared at `t1` in `cell1`
/// (None: still armed when the path ends).
pub struct StampRun {
    pub t0: i64,
    pub cell0: BlockPos,
    pub src: &'static str,
    pub cleared: Option<(i64, BlockPos)>,
}

/// The stamps along a rider's path (one cart position a tick), each position's rail cell the one it
/// stands in or the one below (a slope's cart rides up to a block over its cell).
pub fn stamp_path(roles: &std::collections::HashMap<BlockPos, (bool, bool, &'static str)>, path: &[(i64, Vec3, String)]) -> Vec<StampRun> {
    let mut out: Vec<StampRun> = Vec::new();
    let mut armed: Option<usize> = None;
    for (t, p, _) in path {
        let Some((cell, role)) = path_cell(roles, *p) else { continue };
        match armed {
            None if role.0 => {
                out.push(StampRun { t0: *t, cell0: cell, src: role.2, cleared: None });
                armed = Some(out.len() - 1);
            }
            Some(i) if role.1 => {
                out[i].cleared = Some((*t, cell));
                armed = None;
            }
            _ => {}
        }
    }
    out
}

/// A `queue-loop-run --watch K` file read as a path: each `c<slot> +<dt>: x,y,z ...` line one tick
/// (its `c<slot> +<dt>` label kept).
pub fn read_watch_path(text: &str) -> Vec<(i64, Vec3, String)> {
    let mut out = Vec::new();
    for l in text.lines() {
        if !l.starts_with('c') {
            continue;
        }
        let Some((head, rest)) = l.split_once(": ") else { continue };
        if !head.contains(" +") {
            continue;
        }
        let xyz: Vec<f64> = rest.split_whitespace().next().unwrap_or("").split(',').filter_map(|v| v.parse().ok()).collect();
        if xyz.len() == 3 {
            out.push((out.len() as i64, Vec3::new(xyz[0], xyz[1], xyz[2]), head.to_string()));
        }
    }
    out
}

pub fn stamp_class(p: &Place) -> &'static str {
    let srcs = stamp_sources(p);
    let arm: f64 = srcs.iter().map(|(_, s)| s.risk()).sum();
    let clear: f64 = srcs.iter().map(|(_, s)| s.reached_share).sum();
    match (arm > 1e-9, clear > 1e-9) {
        (true, true) => "arms and clears",
        (true, false) => "arms, never clears",
        (false, _) => "never arms",
    }
}

/// The golem-proof clearing cells for a place (9fc3155f, ironspec Nov 11 19:18's fix price): each
/// standing cell within 2 sideways and 2 up or down of the rider's rail cell that is air with air
/// above and below, the cell below set to the floor block (refused unless golem-proof) and every
/// place re-checked in that world (G1's review: the floor can cut a zombie's sight or add a stroll
/// target elsewhere); the cells whose floor turns the place's class from "arms, never clears", each
/// with the other places whose class or `seen` it changes.
/// A floor cell in a cart's or rider's space (a rail cell or the two cells over one) is skipped.
/// The fix search's floor block, refused unless golem-proof (G1's review, ironspec Nov 12 00:09:
/// `SpawnUtil.Strategy.LEGACY_IRON_GOLEM`'s excluded list: glass, stained and tinted glass, panes,
/// glowstone, sea lantern, leaves, ice...; a block a golem can spawn on would add a golem spot).
fn golem_proof(block: &str) -> Result<redsim::block::Block, String> {
    let b = redsim::block::Block::parse(block)?;
    if !redsim::entity::mob::blocks::golem_excluded(&b) {
        return Err(format!("--fix-block {block}: a golem can spawn on it (not on LEGACY_IRON_GOLEM's excluded list: glass, glowstone, sea lantern, leaves, ice...): refused"));
    }
    Ok(b)
}

/// The full re-check of a fix (G1's review): every place but `skip` whose stamp class or `seen`
/// count changed between `base` (no fix) and `ps` (the fix placed), each as "x,y,z: class (seen n)
/// -> class (seen m)"; a place gone or new printed too.
fn class_changes(base: &[Place], ps: &[Place], skip: BlockPos) -> Vec<String> {
    let mut out = Vec::new();
    for p in ps {
        if p.cell == skip && !p.on_foot {
            continue;
        }
        match base.iter().find(|b| b.cell == p.cell && b.on_foot == p.on_foot) {
            Some(b) => {
                if stamp_class(b) != stamp_class(p) || b.seen != p.seen {
                    out.push(format!("{},{},{}{}: {} (seen {}) -> {} (seen {})", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, stamp_class(b), b.seen, stamp_class(p), p.seen));
                }
            }
            None => out.push(format!("{},{},{}: new, {} (seen {})", p.cell.x, p.cell.y, p.cell.z, stamp_class(p), p.seen)),
        }
    }
    for b in base {
        if !ps.iter().any(|p| p.cell == b.cell && p.on_foot == b.on_foot) {
            out.push(format!("{},{},{}: gone", b.cell.x, b.cell.y, b.cell.z));
        }
    }
    out
}

/// (9fc3155f, ironspec Nov 12 05:24, fix family F1a: reliable clears) A place's panic-stroll clear
/// rate: the share of PANIC's VillageBoundRandomStroll (2/2) draws whose cell a complete path from
/// the seat reaches (MoveToTargetSink's `canReach()`: what erases CANT_REACH_WALK_TARGET_SINCE), now
/// and with golem-proof floors added within 2 sideways and 2 up or down (each making a standable
/// cell), chosen greedily one at a time, each the floor that raises the share most; with each set,
/// the per-dwell miss probability (1 - share)^draws at 10, 20, 30 and 40 draws, and the full
/// re-check of every place with the set laid (places whose class or sight it changes).
pub fn clear_rate(spec: &FarmSpec, home: Option<BlockPos>, panic_boxes: &[([i32; 3], [i32; 3])], claims: &[BlockPos], pills: &[Vec3], place: BlockPos, block: &str, max_floors: usize, exclude: &[([i32; 3], [i32; 3])], at: Option<Vec3>) -> Result<Vec<String>, String> {
    let glass = golem_proof(block)?;
    let share_of = |s: &FarmSpec| -> Result<(f64, f64, usize, usize), String> {
        let ps = check_inner(s, home, &[], panic_boxes, true, claims, pills, Some(place), at.as_slice())?;
        let p = ps.iter().find(|p| p.cell == place && !p.on_foot).ok_or_else(|| format!("{},{},{}: not a rider place", place.x, place.y, place.z))?;
        Ok((p.panic_stroll.reached_share, p.panic_stroll.risk(), p.panic_stroll.cells, p.seen))
    };
    // (the base's cells, each with the chance one stroll event picks it and whether a complete path
    // reaches it: to reconcile with tick-exact's landed draws; and, ironspec Nov 12 16:39, the calm
    // villager's sources there, every candidate reached or not: IDLE's stroll (10/7) and its walks
    // to a rider it sees within 8)
    let (base_cells, idle_line) = {
        let ps = check_inner(spec, home, &[], panic_boxes, true, claims, pills, Some(place), at.as_slice())?;
        let list = |s: &Source| s.reached.iter().map(|(c, sh)| format!("{},{},{} {:.1}% reached", c.x, c.y, c.z, sh * 100.0)).chain(s.unreached.iter().map(|(c, sh)| format!("{},{},{} {:.1}% UNREACHED", c.x, c.y, c.z, sh * 100.0))).collect::<Vec<_>>().join(", ");
        ps.iter()
            .find(|p| p.cell == place && !p.on_foot)
            .map(|p| {
                (
                    list(&p.panic_stroll),
                    format!(
                        "  IDLE there (calm, ironspec Nov 12 16:39): the stroll (10/7): {} cells, {} unreached ({}); the walks to a rider it sees within 8: {} riders' cells, {} unreached ({})",
                        p.stroll.cells,
                        p.stroll.unreached.len(),
                        list(&p.stroll),
                        p.others.cells,
                        p.others.unreached.len(),
                        list(&p.others)
                    ),
                )
            })
            .unwrap_or_default()
    };
    let (s0, u0, n0, seen0) = share_of(spec)?;
    let mut lines = Vec::new();
    // (ironspec Nov 12 05:56, the deterministic rule: every candidate the panic stroll can land on
    // reachable by a complete path (none-or-all), so the first landed draw clears and a panic window
    // of T ticks misses only when every tick's draw is null: p_miss = P_null^T, P_null the chance
    // all ten tries of an event find no candidate)
    let pm = |pnull: f64| [10, 22, 43, 55].iter().map(|t| format!("{t} gt: {:.2e}", pnull.max(0.0).powi(*t))).collect::<Vec<_>>().join(", ");
    let pnull0 = (1.0 - s0 - u0).max(0.0);
    lines.push(format!(
        "clear rate at {}: {}; panic stroll now: {n0} cells ({base_cells}); none-or-all: {}; P_null {:.4} an event; p_miss by panic window ({})",
        match at {
            Some(p) => format!("{:.3},{:.3},{:.3} (a cart: its rider's feet)", p.x, p.y, p.z),
            None => format!("{},{},{}", place.x, place.y, place.z),
        },
        if seen0 > 0 { "panicking (a hostile seen within its range)" } else { "CALM (no hostile seen: the panic stroll does not run here)" },
        if u0 <= 1e-12 { "yes (every candidate reached)".to_string() } else { format!("NO (an event lands on an unreached candidate {:.1}%: remove it or make it reachable)", u0 * 100.0) },
        pnull0,
        pm(pnull0)
    ));
    if !idle_line.is_empty() {
        lines.push(idle_line);
    }
    let solid: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| !b.to_string().ends_with(":air")).map(|(p, _)| *p).collect();
    let rails: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, _)| *p).collect();
    let cart_space = |p: BlockPos| rails.contains(&p) || rails.contains(&BlockPos::new(p.x, p.y - 1, p.z)) || rails.contains(&BlockPos::new(p.x, p.y - 2, p.z));
    let mut cands: Vec<BlockPos> = Vec::new();
    // (the floor and the cell it makes standable kept out of every excluded box: the station's pad
    // and stand, other parts' working space)
    let inside = |q: &BlockPos| exclude.iter().any(|(a, b)| a[0] <= q.x && q.x <= b[0] && a[1] <= q.y && q.y <= b[1] && a[2] <= q.z && q.z <= b[2]);
    for dy in -2..=2 {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let c = BlockPos::new(place.x + dx, place.y + dy, place.z + dz);
                let up = BlockPos::new(c.x, c.y + 1, c.z);
                let f = BlockPos::new(c.x, c.y - 1, c.z);
                if solid.contains(&c) || solid.contains(&up) || solid.contains(&f) || cart_space(f) || cart_space(c) || inside(&f) || inside(&c) || inside(&up) {
                    continue;
                }
                cands.push(f);
            }
        }
    }
    let mut chosen: Vec<BlockPos> = Vec::new();
    let mut cur = s0 + u0;
    let mut s_cur = spec.clone();
    // (each floor kept only if every candidate stays reached and the place still panics: the
    // greedy lowers P_null under the none-or-all rule)
    while chosen.len() < max_floors && u0 <= 1e-12 {
        let mut best: Option<(BlockPos, f64)> = None;
        for f in cands.iter().filter(|f| !chosen.contains(f)) {
            let mut s2 = s_cur.clone();
            s2.blocks.push((*f, glass.clone()));
            let (s, u, _, seen) = share_of(&s2)?;
            if u <= 1e-12 && seen > 0 && s > cur + 1e-9 && best.is_none_or(|(_, b)| s > b) {
                best = Some((*f, s));
            }
        }
        let Some((f, s)) = best else { break };
        chosen.push(f);
        s_cur.blocks.push((f, glass.clone()));
        cur = s;
        lines.push(format!("  + {} {},{},{}: P_null {:.4}; p_miss by panic window ({})", block.trim_start_matches("minecraft:"), f.x, f.y, f.z, (1.0 - s).max(0.0), pm((1.0 - s).max(0.0))));
    }
    if chosen.is_empty() {
        lines.push(format!("  no {} floor within 2 raises it ({} candidates)", block.trim_start_matches("minecraft:"), cands.len()));
    } else {
        // (the full re-check with the set laid: a floor can cut a scarer's sight or add a target elsewhere)
        let base = check_inner(spec, home, &[], panic_boxes, true, claims, pills, None, &[])?;
        let ps = check_inner(&s_cur, home, &[], panic_boxes, true, claims, pills, None, &[])?;
        let ch = class_changes(&base, &ps, place);
        lines.push(format!("  the full re-check with the {} floors laid: {}", chosen.len(), if ch.is_empty() { "no other place's class or sight changes".to_string() } else { ch.join("; ") }));
    }
    Ok(lines)
}

pub fn clearing_fixes(spec: &FarmSpec, home: Option<BlockPos>, panic_boxes: &[([i32; 3], [i32; 3])], claims: &[BlockPos], place: BlockPos, block: &str) -> Result<Vec<(BlockPos, &'static str, Vec<String>)>, String> {
    let glass = golem_proof(block)?;
    let base = check_inner(spec, home, &[], panic_boxes, true, claims, &[], None, &[])?;
    let solid: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| !b.to_string().ends_with(":air")).map(|(p, _)| *p).collect();
    let rails: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, _)| *p).collect();
    let cart_space = |p: BlockPos| rails.contains(&p) || rails.contains(&BlockPos::new(p.x, p.y - 1, p.z)) || rails.contains(&BlockPos::new(p.x, p.y - 2, p.z));
    let mut out = Vec::new();
    for dy in -2..=2 {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let c = BlockPos::new(place.x + dx, place.y + dy, place.z + dz);
                let up = BlockPos::new(c.x, c.y + 1, c.z);
                let f = BlockPos::new(c.x, c.y - 1, c.z);
                if solid.contains(&c) || solid.contains(&up) || solid.contains(&f) || cart_space(f) || cart_space(c) {
                    continue;
                }
                let mut s2 = spec.clone();
                s2.blocks.push((f, glass.clone()));
                // (the full re-check, G1's review: every place's class and `seen` with the floor
                // placed, since a floor can cut a zombie's sight or add a stroll target elsewhere)
                let ps = check_inner(&s2, home, &[], panic_boxes, true, claims, &[], None, &[])?;
                if let Some(p) = ps.iter().find(|p| p.cell == place && !p.on_foot) {
                    let k = stamp_class(p);
                    if k != "arms, never clears" {
                        out.push((f, k, class_changes(&base, &ps, place)));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// [`clearing_fixes`] over pairs: when no single glass floor turns the place, every pair of the
/// candidate floors (a walkway's two cells), the pairs whose glass turns it.
pub fn clearing_fix_pairs(spec: &FarmSpec, home: Option<BlockPos>, panic_boxes: &[([i32; 3], [i32; 3])], claims: &[BlockPos], place: BlockPos, block: &str) -> Result<(usize, Vec<(BlockPos, BlockPos, &'static str, Vec<String>)>), String> {
    let glass = golem_proof(block)?;
    let base = check_inner(spec, home, &[], panic_boxes, true, claims, &[], None, &[])?;
    let solid: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| !b.to_string().ends_with(":air")).map(|(p, _)| *p).collect();
    let rails: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, _)| *p).collect();
    let cart_space = |p: BlockPos| rails.contains(&p) || rails.contains(&BlockPos::new(p.x, p.y - 1, p.z)) || rails.contains(&BlockPos::new(p.x, p.y - 2, p.z));
    let mut cands = Vec::new();
    for dy in -2..=2 {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let c = BlockPos::new(place.x + dx, place.y + dy, place.z + dz);
                let up = BlockPos::new(c.x, c.y + 1, c.z);
                let f = BlockPos::new(c.x, c.y - 1, c.z);
                if solid.contains(&c) || solid.contains(&up) || solid.contains(&f) || cart_space(f) || cart_space(c) {
                    continue;
                }
                cands.push(f);
            }
        }
    }
    let mut out = Vec::new();
    for i in 0..cands.len() {
        for j in i + 1..cands.len() {
            // (a pair is two floors: the second cell can't stand in the first's head room)
            let (a, b) = (cands[i], cands[j]);
            if (a.x, a.z) == (b.x, b.z) && (a.y - b.y).abs() <= 2 {
                continue;
            }
            let mut s2 = spec.clone();
            s2.blocks.push((a, glass.clone()));
            s2.blocks.push((b, glass.clone()));
            let ps = check_inner(&s2, home, &[], panic_boxes, true, claims, &[], None, &[])?;
            if let Some(p) = ps.iter().find(|p| p.cell == place && !p.on_foot) {
                let k = stamp_class(p);
                if k != "arms, never clears" {
                    out.push((a, b, k, class_changes(&base, &ps, place)));
                }
            }
        }
    }
    Ok((cands.len(), out))
}

/// A post's blocks: its stone floor (with `any_floor`), and with an open side its stone roof and
/// two-high walls on the other three sides.
fn post_blocks(c: BlockPos, open: Option<(i32, i32)>, any_floor: bool) -> Vec<BlockPos> {
    let mut v = Vec::new();
    if any_floor {
        v.push(BlockPos::new(c.x, c.y - 1, c.z));
    }
    if let Some(o) = open {
        v.push(BlockPos::new(c.x, c.y + 2, c.z));
        for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            if (dx, dz) != o {
                v.push(BlockPos::new(c.x + dx, c.y, c.z + dz));
                v.push(BlockPos::new(c.x + dx, c.y + 1, c.z + dz));
            }
        }
    }
    v
}

/// The rider points the scarer tools score, each a villager's feet: each rail place's three points
/// (`along_rail`: a seated villager's feet are its cart's position, 26.3's
/// AbstractMinecart.getPassengerAttachmentPoint (LOWERED_PASSENGER_ATTACHMENT, (0,0,0) for a
/// villager), confirmed officially by tick-exact Nov 12 07:20) and `extra` (feet: a rider's are its
/// cart's position, one on foot its own).
fn scarer_points(spec: &FarmSpec, extra: &[Vec3]) -> Vec<(String, Vec3)> {
    let mut pts: Vec<(String, Vec3)> = Vec::new();
    for (c, b) in spec.blocks.iter() {
        let s = b.to_string();
        if !s.contains("rail") {
            continue;
        }
        for t in [0.1, 0.5, 0.9] {
            pts.push((format!("{},{},{}", c.x, c.y, c.z), along_rail(*c, &s, t)));
        }
    }
    for e in extra {
        pts.push((format!("rider {:.2},{:.2},{:.2}", e.x, e.y, e.z), *e));
    }
    pts
}

/// A zombie seated in a boat: its feet over the boat's y (the boat's passenger attachment, then
/// the zombie's riding offset), as `zombies_seated` seats it.
fn zombie_seat() -> f64 {
    (0.5625f32 / 3.0f32) as f64 + redsim::entity::mob::types::MobType::Zombie.def().riding_offset as f64
}

/// The riders' paths sampled densely: every rail cell along its track (a straight or sloped cell
/// at 1/20 of its length, a curve from one exit's edge through its centre to the other's), and each
/// column (x, z, y0, y1: a drop or a lift, the cart's position every 0.1); (label, position, the
/// path length it stands for).
fn dense_rider_points(spec: &FarmSpec, columns: &[[f64; 4]]) -> Vec<(String, Vec3, f64)> {
    let mut v = Vec::new();
    for (c, b) in spec.blocks.iter() {
        let s = b.to_string();
        if !s.contains("rail") {
            continue;
        }
        let lab = format!("{},{},{}", c.x, c.y, c.z);
        let curve = ["north_east", "north_west", "south_east", "south_west"].iter().any(|k| s.contains(k));
        if curve {
            let centre = Vec3::new(c.x as f64 + 0.5, c.y as f64 + 0.0625, c.z as f64 + 0.5);
            let edges: Vec<Vec3> = [("north", (0.5, 0.0)), ("south", (0.5, 1.0)), ("east", (1.0, 0.5)), ("west", (0.0, 0.5))]
                .iter()
                .filter(|(k, _)| s.split("shape=").nth(1).is_some_and(|sh| sh.split([',', ']']).next().unwrap_or("").contains(k)))
                .map(|(_, (ex, ez))| Vec3::new(c.x as f64 + ex, centre.y, c.z as f64 + ez))
                .collect();
            for e in edges {
                for i in 0..10 {
                    let t = i as f64 / 10.0;
                    v.push((lab.clone(), Vec3::new(e.x + (centre.x - e.x) * t, centre.y, e.z + (centre.z - e.z) * t), 0.05));
                }
            }
            v.push((lab, centre, 0.0));
        } else {
            for i in 0..=20 {
                v.push((lab.clone(), along_rail(*c, &s, i as f64 / 20.0), 0.05));
            }
        }
    }
    for col in columns {
        let (x, z, y0, y1) = (col[0], col[2], col[1].min(col[3]), col[1].max(col[3]));
        let n = ((y1 - y0) * 10.0).round() as usize;
        for i in 0..=n {
            v.push((format!("column {x},{z}"), Vec3::new(x, y0 + i as f64 * 0.1, z), 0.1));
        }
    }
    v
}

/// The laid scarers' check (ironspec Nov 12 01:25): each given post (its boat's cell and kind; its
/// blocks already in the design), seated, against every rider point: refused when a pillager sees
/// a rider inside 8.5 or a zombie sees one inside 6.05 or one inside 6.05 is hidden from it only by
/// partial blocks (ironspec Nov 12 10:20: the flee bar sight-gated, robust hiding only: a hidden
/// position's eye ray blocked by a full-collider block, listed as "hidden within 6.05"); the points each covers
/// (a pillager 8.5..15 in sight, a zombie 6.05..7.95 in sight); the places no post covers.
/// (9fc3155f, ironspec Nov 12 06:44, dfb89499's caller measure) An occluder for a post: glass on
/// every sight ray from the post's mob's eye to the `hide` rider positions (each a rider's feet; its
/// eye 1.62 over), on no ray to a rider point the post covers now (scarer_points outside 0.75 of
/// every hide position, seen within its range), each cell air, off the rails and out of every
/// excluded box and out of the rider space over every rail (the 1..3 cells over a rail cell, where a
/// seated villager's 1.95-tall box passes, a slope's rise included); the fewest cells by a greedy
/// hitting set, then every hide position and every kept point re-checked with the glass laid.
pub fn occlude(spec: &FarmSpec, post: BlockPos, pill: bool, hide: &[Vec3], exclude: &[([i32; 3], [i32; 3])], extra: &[Vec3]) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let glass = redsim::block::Block::parse("minecraft:glass")?;
    let air_block = redsim::block::Block::parse("minecraft:air")?;
    let seat = zombie_seat();
    let feet = Vec3::new(post.x as f64 + 0.5, post.y as f64 + if pill { PILLAGER_SEAT } else { seat }, post.z as f64 + 0.5);
    let eye = Vec3::new(feet.x, feet.y + if pill { PILLAGER_EYE } else { 1.74 }, feet.z);
    let far = if pill { 15.0 } else { PANIC_BAR };
    let reye = |p: &Vec3| Vec3::new(p.x, p.y + 1.62, p.z);
    let d = |p: &Vec3| ((p.x - feet.x).powi(2) + (p.y - feet.y).powi(2) + (p.z - feet.z).powi(2)).sqrt();
    let sees = |w: &redsim::World, p: &Vec3| d(p) <= far && redsim::entity::mob::los::has_line_of_sight(w, reye(p), eye);
    let hidden_now: Vec<Vec3> = hide.iter().copied().filter(|p| sees(w, p)).collect();
    // (the kept points: every rail place's three points and the `extra` rider positions, e.g. a
    // seat's exact cart position, 9fc3155f Nov 12 09:57)
    let keep: Vec<(String, Vec3)> = scarer_points(spec, extra)
        .into_iter()
        .filter(|(_, p)| hide.iter().all(|h| ((p.x - h.x).powi(2) + (p.y - h.y).powi(2) + (p.z - h.z).powi(2)).sqrt() > 0.75))
        .filter(|(_, p)| sees(w, p))
        .collect();
    let keep_cells: std::collections::HashSet<BlockPos> = keep.iter().flat_map(|(_, p)| ray_cells(reye(p), eye)).collect();
    let rails: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, _)| *p).collect();
    let ok_cell = |w: &redsim::World, c: &BlockPos| {
        w.get(*c).is_air() && !keep_cells.contains(c) && !rails.contains(c) && !(1..=3).any(|k| rails.contains(&BlockPos::new(c.x, c.y - k, c.z))) && !exclude.iter().any(|(a, b)| a[0] <= c.x && c.x <= b[0] && a[1] <= c.y && c.y <= b[1] && a[2] <= c.z && c.z <= b[2])
    };
    let mut sets: Vec<(Vec3, Vec<BlockPos>)> = hidden_now.iter().map(|p| (*p, ray_cells(reye(p), eye).into_iter().filter(|c| ok_cell(w, c)).collect())).collect();
    let mut chosen: Vec<BlockPos> = Vec::new();
    let mut lines = vec![format!(
        "occluder for the {} at {},{},{}: {} of {} hide positions seen now; {} covered points kept",
        if pill { "pillager" } else { "zombie" },
        post.x,
        post.y,
        post.z,
        hidden_now.len(),
        hide.len(),
        keep.len()
    )];
    if let Some((p, _)) = sets.iter().find(|(_, s)| s.is_empty()) {
        lines.push(format!("  NONE: no free cell on the ray to {:.2},{:.2},{:.2} that spares every kept ray", p.x, p.y, p.z));
        return Ok(lines);
    }
    while !sets.is_empty() {
        let mut count: std::collections::HashMap<BlockPos, usize> = std::collections::HashMap::new();
        for (_, s) in &sets {
            for c in s {
                *count.entry(*c).or_default() += 1;
            }
        }
        let best = count.iter().max_by_key(|(c, n)| (**n, std::cmp::Reverse((c.y, c.z, c.x)))).map(|(c, _)| *c).expect("a cell");
        chosen.push(best);
        sets.retain(|(_, s)| !s.contains(&best));
    }
    for c in &chosen {
        w.set_block(*c, glass.clone(), redsim::world::flags::CLIENTS);
    }
    let still: Vec<&Vec3> = hidden_now.iter().filter(|p| sees(w, p)).collect();
    let lost: Vec<&String> = keep.iter().filter(|(_, p)| !sees(w, p)).map(|(l, _)| l).collect();
    for c in &chosen {
        w.set_block(*c, air_block.clone(), redsim::world::flags::CLIENTS);
    }
    lines.push(format!(
        "  glass {}: {} ({} hide positions still seen; {} kept points lost{})",
        chosen.len(),
        chosen.iter().map(|c| format!("{},{},{}", c.x, c.y, c.z)).collect::<Vec<_>>().join(" "),
        still.len(),
        lost.len(),
        if lost.is_empty() { String::new() } else { format!(": {}", lost.iter().take(6).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) }
    ));
    Ok(lines)
}

pub fn scarer_check(spec: &FarmSpec, extra: &[Vec3], posts: &[(BlockPos, bool)], transient: &[String], golem_pad: Option<([i32; 3], [i32; 3])>, columns: &[[f64; 4]]) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &run.world;
    let pts = scarer_points(spec, extra);
    let seat = zombie_seat();
    let mut covered = vec![false; pts.len()];
    let mut lines = Vec::new();
    let mut refused = 0usize;
    for (c, pill) in posts {
        let (near, far) = if *pill { (8.5, 15.0) } else { (FLEE_BAR, PANIC_BAR) };
        let feet = Vec3::new(c.x as f64 + 0.5, c.y as f64 + if *pill { PILLAGER_SEAT } else { seat }, c.z as f64 + 0.5);
        let eye = Vec3::new(feet.x, feet.y + if *pill { PILLAGER_EYE } else { 1.74 }, feet.z);
        let mut n = 0usize;
        let mut least = f64::MAX;
        // (the nearest rider sight or not, for a pillager too: G1 on 4784b03af)
        let mut least_any = f64::MAX;
        let mut bad: Vec<String> = Vec::new();
        // (a zombie's positions within 6.05 it cannot see, each behind a full-collider block)
        let mut hidden: Vec<(String, Vec3)> = Vec::new();
        // (the positions this zombie sees within its panic range: the seeds of the hidden
        // positions' travel times)
        let mut seen8: Vec<Vec3> = Vec::new();
        for (k, (l, p)) in pts.iter().enumerate() {
            let d = ((p.x - feet.x).powi(2) + (p.y - feet.y).powi(2) + (p.z - feet.z).powi(2)).sqrt();
            least_any = least_any.min(d);
            if d > far {
                continue;
            }
            let seen = redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + 1.62, p.z), eye);
            if !*pill && seen {
                seen8.push(*p);
            }
            if *pill && seen {
                least = least.min(d);
            }
            if !*pill {
                least = least.min(d);
            }
            if d < near && (seen || !*pill) {
                // (ironspec Nov 12 10:20: a zombie's 6.05 flee bar is sight-gated with robust hiding
                // only: a position it cannot see counts as hidden when its eye ray to the zombie's eye
                // is blocked by a full-collider block (glass or solid, not panes, fences or partials))
                if !*pill && !seen {
                    let blocker = ray_cells(Vec3::new(p.x, p.y + 1.62, p.z), eye).into_iter().find(|q| {
                        let b = w.get(*q);
                        !b.is_air() && redsim::behavior::collision_full_block_at(w, &b, *q)
                    });
                    match blocker {
                        Some(q) => hidden.push((format!("{l} at {d:.2} behind {} {},{},{}", w.get(q).to_string().trim_start_matches("minecraft:"), q.x, q.y, q.z), *p)),
                        None => bad.push(format!("{l} at {d:.2} (hidden only by a partial block)")),
                    }
                    continue;
                }
                bad.push(format!("{l} at {d:.2}{}", if seen { " (seen)" } else { "" }));
                continue;
            }
            if seen && d >= near {
                covered[k] = true;
                n += 1;
            }
        }
        // (G1's CHANGES 2 on fee712f72: each hidden position's travel time from the last position
        // this zombie sees within its panic range, along the track (Track: the rails at <= 0.4 b/gt,
        // the columns at 1); bar >= 40 gt: a stale NEAREST_HOSTILE can still drive the flee there)
        let hidden_t: Vec<(String, Option<f64>)> = if hidden.is_empty() {
            Vec::new()
        } else {
            let track = Track::new(spec, columns, &[]);
            let seeds: Vec<TailNode> = seen8.iter().filter_map(|p| track.node_at(*p)).collect();
            let times = track.walk(&seeds, 400.0);
            hidden.iter().map(|(s, p)| (s.clone(), track.node_at(*p).and_then(|nd| times.get(&nd).copied()))).collect()
        };
        for (s, t) in &hidden_t {
            if let Some(t) = t
                && *t < HIDDEN_BAR_GT
            {
                bad.push(format!("{s}, only {t:.1} gt after the last position it sees within {PANIC_BAR} (bar {HIDDEN_BAR_GT} gt, inherited: G1's CHANGES 2 on fee712f72, no measured failure)"));
            }
        }
        if !bad.is_empty() {
            refused += 1;
        }
        // (ironspec Nov 12 05:26: the official bridge shows a pillager charging at and shooting a
        // villager it sees within 8 (RangedCrossbowAttackGoal: feet within 8, eye-to-eye sight,
        // seeTime >= 5): the riders' paths sampled densely (every rail cell along its track at
        // 1/20 of a cell, a curve through its centre, and each --scarer-column every 0.1), the nearest
        // position it sees and the path length it sees within 8, by cell)
        if *pill {
            let mut dense_least = f64::MAX;
            let mut within: Vec<(String, f64)> = Vec::new();
            for (lab, p, step) in dense_rider_points(spec, columns) {
                let d = ((p.x - feet.x).powi(2) + (p.y - feet.y).powi(2) + (p.z - feet.z).powi(2)).sqrt();
                if d >= dense_least && d > 8.0 {
                    continue;
                }
                if redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + 1.62, p.z), eye) {
                    dense_least = dense_least.min(d);
                    if d <= 8.0 {
                        match within.iter_mut().find(|(l, _)| *l == lab) {
                            Some((_, len)) => *len += step,
                            None => within.push((lab, step)),
                        }
                    }
                }
            }
            lines.push(format!(
                "    pillager {},{},{} over the riders' paths (dense): the nearest position it sees {}; seen within its crossbow's 8: {}",
                c.x,
                c.y,
                c.z,
                if dense_least == f64::MAX { "none".to_string() } else { format!("{dense_least:.2}") },
                if within.is_empty() { "none (no tick of any rider within 8 in its sight)".to_string() } else { within.iter().map(|(l, len)| format!("{l} for {len:.2} blocks of path")).collect::<Vec<_>>().join(", ") }
            ));
        }
        // (the pad golems that see this post within their 16 follow range: 0 required, G1)
        let mut seers: Vec<String> = Vec::new();
        if let Some((glo, ghi)) = golem_pad {
            let eye = Vec3::new(feet.x, feet.y + if *pill { PILLAGER_EYE } else { 1.74 }, feet.z);
            for gx in glo[0]..=ghi[0] {
                for gy in glo[1]..=ghi[1] {
                    for gz in glo[2]..=ghi[2] {
                        let gf = Vec3::new(gx as f64 + 0.5, gy as f64, gz as f64 + 0.5);
                        let d = ((gf.x - feet.x).powi(2) + (gf.y - feet.y).powi(2) + (gf.z - feet.z).powi(2)).sqrt();
                        if d <= GOLEM_TARGET && redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(gf.x, gf.y + GOLEM_EYE, gf.z), eye) {
                            seers.push(format!("{gx},{gy},{gz} at {d:.2}"));
                        }
                    }
                }
            }
            if !seers.is_empty() {
                refused += 1;
            }
        }
        // (ironspec Nov 12 05:39, dfb89499's finding: a zombie under a glass roof burns by day;
        // canSeeSky is sky light 15 at its eye, so every zombie post's eye must read < 15)
        // (canSeeSky = sky light 15 = a sky light source: `daylight::sky_source`'s column scan; an
        // unclassified block in the column is reported, never read as 15)
        let sky: Option<Result<bool, String>> = if *pill { None } else { Some(redsim::daylight::sky_source(w, BlockPos::new(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32))) };
        if !matches!(sky, None | Some(Ok(false))) {
            refused += 1;
        }
        lines.push(format!(
            "  {} {},{},{}: covers {n} points; the nearest rider {} {}{}{}{}{}",
            if *pill { "pillager" } else { "zombie" },
            c.x,
            c.y,
            c.z,
            if *pill { "it sees" } else { "(sight or not)" },
            if least == f64::MAX { "none within range".to_string() } else { format!("{least:.2}") },
            if *pill && least_any < f64::MAX { format!(" (sight or not {least_any:.2})") } else { String::new() },
            if bad.is_empty() { String::new() } else { format!("; REFUSED, {} inside its bar: {}", bad.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join(", ")) },
            match golem_pad {
                None => String::new(),
                Some(_) if seers.is_empty() => "; no pad golem sees it within 16".to_string(),
                Some(_) => format!("; REFUSED, {} pad golem cells see it within 16: {}", seers.len(), seers.iter().take(4).cloned().collect::<Vec<_>>().join(", ")),
            },
            match sky {
                None => String::new(),
                Some(Ok(false)) => "; its eye under cover (sky light below 15: it does not burn)".to_string(),
                Some(Ok(true)) => "; REFUSED, sky light 15 at its eye: it sees the sky and burns by day (an opaque, golem-proof roof: glowstone)".to_string(),
                Some(Err(e)) => format!("; REFUSED, its eye's sky light unknown ({e})"),
            }
        ));
        // (ironspec Nov 12 10:20, the "hidden within 6.05" class: every position inside the flee
        // bar that the zombie cannot see, with the full-collider block that hides it)
        if !*pill && !hidden_t.is_empty() {
            let least_t = hidden_t.iter().filter_map(|(_, t)| *t).fold(f64::INFINITY, f64::min);
            lines.push(format!(
                "    hidden within {near}: {} positions (each eye ray to the zombie's eye blocked by a full-collider block; travel time from the last position it sees within {PANIC_BAR}, bar {HIDDEN_BAR_GT} gt (inherited: G1's CHANGES 2 on fee712f72): least {}): {}",
                hidden_t.len(),
                if least_t.is_finite() { format!("{least_t:.1} gt") } else { "none reached from a seen position (never in sight within the track's reach)".to_string() },
                hidden_t.iter().take(8).map(|(s, t)| format!("{s} ({})", t.map_or("no seen position upstream".to_string(), |t| format!("{t:.1} gt")))).collect::<Vec<_>>().join("; ")
            ));
        }
    }
    let mut labels: Vec<&str> = Vec::new();
    for (l, _) in &pts {
        if !labels.contains(&l.as_str()) {
            labels.push(l);
        }
    }
    let mut open = Vec::new();
    let mut declared = Vec::new();
    for l in &labels {
        let ks: Vec<usize> = pts.iter().enumerate().filter(|(_, (x, _))| x == l).map(|(k, _)| k).collect();
        let m = ks.iter().filter(|&&k| covered[k]).count();
        if m < ks.len() {
            if transient.iter().any(|t| t == l) {
                declared.push(format!("{l} ({m}/{})", ks.len()));
            } else {
                open.push(format!("{l} ({m}/{})", ks.len()));
            }
        }
    }
    lines.insert(
        0,
        format!(
            "scarer check ({} posts, each seated in its boat, the design's blocks in the sight test): {refused} refused; {} of {} rider points covered; places not fully covered: {}{}",
            posts.len(),
            covered.iter().filter(|&&c| c).count(),
            pts.len(),
            if open.is_empty() { "none".to_string() } else { open.join(", ") },
            if declared.is_empty() { String::new() } else { format!("; declared transient (flee-check's one exception): {}", declared.join(", ")) }
        ),
    );
    Ok(lines)
}

/// The scarer search (ironspec Nov 11 22:53, the station's night bars): each candidate post in
/// `lo..=hi` (an air cell with air above; its floor the design's, or with `any_floor` a stone floor
/// placed for the test) for `kind`, seated in a boat (G1's review): a zombie (its seat as
/// `zombies_seated` seats it, eye 1.74) covers a rider point it is FLEE_BAR..PANIC_BAR (6.05..7.95)
/// from (the rider's feet to its seat) and sees (the rider's eye to its eye, `has_line_of_sight`),
/// and is refused when any rider point is inside FLEE_BAR (the 6-8 gate's distance rule, sight or
/// not); a pillager (feet at boat + 0.1875 - 0.6, eye 1.6575) covers 8.5..15 in sight and is
/// refused when a rider point it sees is inside 8.5 (`near`). With `walls` each post's stone roof
/// and two-high walls on three sides are in the sight test (each open side tried). The rider
/// points: each rail place's three points (`along_rail`) and `extra` (feet: a rider's are its
/// cart's position). A greedy cover (the
/// post covering the most still-uncovered points first, at most `max_posts`), or with `exact` an
/// exact least cover over both kinds' posts (`exact_cover`). Not checked here: golem paths to the
/// posts (`golem_reach`); the laid result goes through the full flee-check (classes, the 6-8 gate).
#[allow(clippy::too_many_arguments)]
pub fn scarer_search(spec: &FarmSpec, extra: &[Vec3], lo: [i32; 3], hi: [i32; 3], kind: &str, max_posts: usize, any_floor: bool, exclude: &[([i32; 3], [i32; 3])], near: Option<f64>, walls: bool, target: &[String], exact: Option<usize>, transient: &[String], mix: bool, golem_pad: Option<([i32; 3], [i32; 3])>, visor: bool) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let stone = redsim::block::Block::parse("minecraft:stone")?;
    let air_block = redsim::block::Block::parse("minecraft:air")?;
    let pillager = kind == "pillager";
    let (mut near_bar, far_bar) = if pillager { (8.0, 15.0) } else { (FLEE_BAR, PANIC_BAR) };
    // (a margin over the bar, ironspec Nov 11 23:57: a boat bobs in its cell and the charge test runs
    // on the true distance, so a pillager keeps every rider it sees >= 8.5)
    if let Some(n) = near {
        near_bar = n;
    }
    let pts = scarer_points(spec, extra);
    let seat = zombie_seat();
    let air = |w: &redsim::World, x: i32, y: i32, z: i32| w.get(BlockPos::new(x, y, z)).is_air();
    let mut cands: Vec<(BlockPos, Vec<usize>, Option<(i32, i32)>)> = Vec::new();
    // (exact: both kinds, every valid open side its own candidate; (cell, covers, open side, a
    // pillager, the blocks it places))
    let mut all: Vec<(BlockPos, Vec<usize>, Option<(i32, i32)>, bool, usize)> = Vec::new();
    // (each candidate's visor cells, by its index in `all`)
    let mut visors: HashMap<usize, Vec<BlockPos>> = HashMap::new();
    let kinds: Vec<bool> = if exact.is_some() { vec![true, false] } else { vec![pillager] };
    let mut refused = 0usize;
    for &pill in &kinds {
    let (near_k, far_k) = match (exact.is_some(), pill) {
        (false, _) => (near_bar, far_bar),
        (true, true) => (near.unwrap_or(8.5).max(8.5), 15.0),
        (true, false) => (FLEE_BAR, PANIC_BAR),
    };
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                // (a post brings its own floor with `any_floor`: then any air cell with air above;
                // its floor block (stone) is placed for the sight test, G1's review)
                if !air(w, x, y, z) || !air(w, x, y + 1, z) || (!any_floor && air(w, x, y - 1, z)) {
                    continue;
                }
                // (a post, its floor and the zombie's head cell kept out of every excluded box:
                // other parts' reserved space, the pad's spawn columns, the riders' own cells)
                // (with `walls`, the post's whole footprint (x +-1, z +-1, its floor to its roof) kept
                // out of the excluded boxes too)
                let r = if walls { 1 } else { 0 };
                let (ytop, ybot) = if walls { (y + 2, y - 1) } else { (y + 1, y - 1) };
                if exclude.iter().any(|(a, b)| a[0] <= x + r && x - r <= b[0] && a[2] <= z + r && z - r <= b[2] && a[1] <= ytop && ybot <= b[1]) {
                    continue;
                }
                // (the post's own blocks in the sight test, G1's review / dfb89499: the floor
                // (stone) with `any_floor`, and with `walls` a stone roof and two-high walls on three
                // sides, each of the four open sides tried and the best kept)
                let sides: &[Option<(i32, i32)>] = if walls { &[Some((0, -1)), Some((1, 0)), Some((0, 1)), Some((-1, 0))] } else { &[None] };
                let feet = Vec3::new(x as f64 + 0.5, y as f64 + if pill { PILLAGER_SEAT } else { seat }, z as f64 + 0.5);
                let eye = Vec3::new(feet.x, feet.y + if pill { PILLAGER_EYE } else { 1.74 }, feet.z);
                let mut best_cfg: Option<(Vec<usize>, Option<(i32, i32)>)> = None;
                let mut any_bad = false;
                for open in sides {
                    let mut post: Vec<BlockPos> = Vec::new();
                    if any_floor {
                        post.push(BlockPos::new(x, y - 1, z));
                    }
                    if let Some(o) = open {
                        post.push(BlockPos::new(x, y + 2, z));
                        for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                            if (dx, dz) != *o {
                                post.push(BlockPos::new(x + dx, y, z + dz));
                                post.push(BlockPos::new(x + dx, y + 1, z + dz));
                            }
                        }
                    }
                    let placed: Vec<BlockPos> = post.into_iter().filter(|p| air(w, p.x, p.y, p.z)).collect();
                    for p in &placed {
                        w.set_block(*p, stone.clone(), redsim::world::flags::CLIENTS);
                    }
                    let mut covers = Vec::new();
                    let mut bad = false;
                    for (k, (_, p)) in pts.iter().enumerate() {
                        let d = ((p.x - feet.x).powi(2) + (p.y - feet.y).powi(2) + (p.z - feet.z).powi(2)).sqrt();
                        if !pill && d < near_k {
                            bad = true;
                            break;
                        }
                        if d > far_k {
                            continue;
                        }
                        let seen = redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + 1.62, p.z), eye);
                        if pill && d < near_k && seen {
                            bad = true;
                            break;
                        }
                        if seen && d >= near_k && (target.is_empty() || target.iter().any(|t| pts[k].0.contains(t.as_str()))) {
                            covers.push(k);
                        }
                    }
                    // (G1, Nov 12 03:29: an iron golem targets an Enemy it sees within its 16 follow
                    // range (NearestAttackableTargetGoal, its line of sight eye to eye) and keeps it
                    // while within 16: a post any pad golem sees within 16 parks golems at the pad's
                    // edge and keeps the summoners golem-detected; refused, or with --scarer-visor
                    // shielded by a visor (ironspec Nov 12 03:54): glass on the golems' rays and on
                    // no covered rider's ray, inside the box and out of every excluded box, the
                    // fewest cells by a greedy hitting set, then every golem cell and every covered
                    // point re-checked with it laid)
                    // (ironspec Nov 12 05:39: a zombie post's eye must read sky light < 15, its roof
                    // opaque: a zombie that sees the sky burns by day)
                    if !bad && !pill {
                        let ec = BlockPos::new(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32);
                        if !matches!(redsim::daylight::sky_source(w, ec), Ok(false)) {
                            bad = true;
                        }
                    }
                    let mut vis: Vec<BlockPos> = Vec::new();
                    if !bad {
                        if let Some((glo, ghi)) = golem_pad {
                            let golem_rays = |w: &redsim::World| -> Vec<Vec3> {
                                let mut v = Vec::new();
                                for gx in glo[0]..=ghi[0] {
                                    for gy in glo[1]..=ghi[1] {
                                        for gz in glo[2]..=ghi[2] {
                                            let gf = Vec3::new(gx as f64 + 0.5, gy as f64, gz as f64 + 0.5);
                                            let d = ((gf.x - feet.x).powi(2) + (gf.y - feet.y).powi(2) + (gf.z - feet.z).powi(2)).sqrt();
                                            let ge = Vec3::new(gf.x, gf.y + GOLEM_EYE, gf.z);
                                            if d <= GOLEM_TARGET && redsim::entity::mob::los::has_line_of_sight(w, ge, eye) {
                                                v.push(ge);
                                            }
                                        }
                                    }
                                }
                                v
                            };
                            let rays = golem_rays(w);
                            if !rays.is_empty() {
                                if !visor {
                                    bad = true;
                                } else {
                                    let pt_eye = |k: usize| Vec3::new(pts[k].1.x, pts[k].1.y + 1.62, pts[k].1.z);
                                    let keep: std::collections::HashSet<BlockPos> = covers.iter().flat_map(|&k| ray_cells(pt_eye(k), eye)).collect();
                                    let ok_cell = |c: &BlockPos| {
                                        air(w, c.x, c.y, c.z)
                                            && !keep.contains(c)
                                            && c.x >= lo[0] - 1
                                            && c.x <= hi[0] + 1
                                            && c.z >= lo[2] - 1
                                            && c.z <= hi[2] + 1
                                            && !exclude.iter().any(|(a, b)| a[0] <= c.x && c.x <= b[0] && a[1] <= c.y && c.y <= b[1] && a[2] <= c.z && c.z <= b[2])
                                    };
                                    let mut sets: Vec<Vec<BlockPos>> = rays.iter().map(|g| ray_cells(*g, eye).into_iter().filter(|c| ok_cell(c)).collect()).collect();
                                    while !sets.is_empty() {
                                        if sets.iter().any(|s| s.is_empty()) {
                                            break;
                                        }
                                        let mut count: HashMap<BlockPos, usize> = HashMap::new();
                                        for s in &sets {
                                            for c in s {
                                                *count.entry(*c).or_default() += 1;
                                            }
                                        }
                                        let best = count.iter().max_by_key(|(c, n)| (**n, std::cmp::Reverse((c.y, c.z, c.x)))).map(|(c, _)| *c).expect("a cell");
                                        vis.push(best);
                                        sets.retain(|s| !s.contains(&best));
                                    }
                                    if !sets.is_empty() {
                                        bad = true;
                                        vis.clear();
                                    } else {
                                        for c in &vis {
                                            w.set_block(*c, stone.clone(), redsim::world::flags::CLIENTS);
                                        }
                                        let still = golem_rays(w);
                                        let lost = covers.iter().any(|&k| !redsim::entity::mob::los::has_line_of_sight(w, pt_eye(k), eye));
                                        for c in &vis {
                                            w.set_block(*c, air_block.clone(), redsim::world::flags::CLIENTS);
                                        }
                                        if !still.is_empty() || lost {
                                            bad = true;
                                            vis.clear();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    for p in &placed {
                        w.set_block(*p, air_block.clone(), redsim::world::flags::CLIENTS);
                    }
                    if bad {
                        any_bad = true;
                        continue;
                    }
                    if exact.is_some() && !covers.is_empty() {
                        if !vis.is_empty() {
                            visors.insert(all.len(), vis.clone());
                        }
                        all.push((BlockPos::new(x, y, z), covers.clone(), *open, pill, placed.len() + vis.len()));
                    }
                    if best_cfg.as_ref().is_none_or(|(c, _)| covers.len() > c.len()) {
                        best_cfg = Some((covers, *open));
                    }
                }
                match best_cfg {
                    Some((covers, open)) if !covers.is_empty() => cands.push((BlockPos::new(x, y, z), covers, open)),
                    None if any_bad => refused += 1,
                    _ => {}
                }
            }
        }
    }
    }
    if let Some(maxk) = exact {
        // (the built re-check inside the search, ironspec Nov 12 01:25 / the parent 02:52: every
        // chosen post's blocks laid at once, each post re-scored against every rider point with
        // the others' walls in its sight: None on a refusal, else the points the set covers)
        let pill_near = near.unwrap_or(8.5).max(8.5);
        let mut verify = |sel: &[usize]| -> Option<Vec<usize>> {
            let mut placed: Vec<BlockPos> = Vec::new();
            for &i in sel {
                let (c, _, o, _, _) = &all[i];
                for p in post_blocks(*c, *o, any_floor).into_iter().chain(visors.get(&i).cloned().unwrap_or_default()) {
                    if air(w, p.x, p.y, p.z) && !placed.contains(&p) {
                        w.set_block(p, stone.clone(), redsim::world::flags::CLIENTS);
                        placed.push(p);
                    }
                }
            }
            let mut cov = vec![false; pts.len()];
            let mut ok = true;
            'post: for &i in sel {
                let (c, _, _, pill, _) = &all[i];
                let (nb, fb) = if *pill { (pill_near, 15.0) } else { (FLEE_BAR, PANIC_BAR) };
                let feet = Vec3::new(c.x as f64 + 0.5, c.y as f64 + if *pill { PILLAGER_SEAT } else { seat }, c.z as f64 + 0.5);
                let eye = Vec3::new(feet.x, feet.y + if *pill { PILLAGER_EYE } else { 1.74 }, feet.z);
                for (k, (_, p)) in pts.iter().enumerate() {
                    let d = ((p.x - feet.x).powi(2) + (p.y - feet.y).powi(2) + (p.z - feet.z).powi(2)).sqrt();
                    if !*pill && d < nb {
                        ok = false;
                        break 'post;
                    }
                    if d > fb {
                        continue;
                    }
                    let seen = redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + 1.62, p.z), eye);
                    if *pill && d < nb && seen {
                        ok = false;
                        break 'post;
                    }
                    if seen && d >= nb {
                        cov[k] = true;
                    }
                }
            }
            for p in &placed {
                w.set_block(*p, air_block.clone(), redsim::world::flags::CLIENTS);
            }
            if ok { Some(cov.iter().enumerate().filter(|(_, b)| **b).map(|(k, _)| k).collect()) } else { None }
        };
        return Ok(exact_cover(&pts, &all, maxk, refused, any_floor, transient, mix, &mut verify, &visors));
    }
    let mut covered = vec![false; pts.len()];
    let mut chosen: Vec<(BlockPos, usize, Option<(i32, i32)>)> = Vec::new();
    while chosen.len() < max_posts {
        let best = cands.iter().map(|(c, cv, o)| (*c, cv, *o, cv.iter().filter(|&&k| !covered[k]).count())).max_by_key(|(c, _, _, n)| (*n, std::cmp::Reverse((c.y, c.z, c.x))));
        let Some((c, cv, o, n)) = best else { break };
        if n == 0 {
            break;
        }
        for &k in cv.iter() {
            covered[k] = true;
        }
        chosen.push((c, n, o));
    }
    let side = |o: Option<(i32, i32)>| match o {
        Some((0, -1)) => " open north",
        Some((1, 0)) => " open east",
        Some((0, 1)) => " open south",
        Some((-1, 0)) => " open west",
        _ => "",
    };
    let mut lines = vec![format!(
        "scarer search ({kind} seated in a boat: covers {near_bar}..{far_bar} in sight; refused with a rider point inside {near_bar}{}; the post's {} in the sight test), candidates in x {}..{} y {}..{} z {}..{}: {} posts cover a point, {refused} refused; {} rider points; greedy, at most {max_posts}: {}",
        if pillager { " that it sees" } else { ", sight or not" },
        match (any_floor, walls) { (true, true) => "floor, roof and three walls", (true, false) => "floor", (false, true) => "roof and three walls (its floor the design's)", (false, false) => "cell (its floor the design's)" },
        lo[0], hi[0], lo[1], hi[1], lo[2], hi[2],
        cands.len(),
        pts.len(),
        if chosen.is_empty() { "none".to_string() } else { chosen.iter().map(|(c, n, o)| format!("{},{},{}{} (+{n})", c.x, c.y, c.z, side(*o))).collect::<Vec<_>>().join(", ") }
    )];
    // (each place's points: covered when every point is)
    let mut labels: Vec<&str> = Vec::new();
    for (l, _) in &pts {
        if !labels.contains(&l.as_str()) {
            labels.push(l);
        }
    }
    let mut open = Vec::new();
    for l in &labels {
        let ks: Vec<usize> = pts.iter().enumerate().filter(|(_, (x, _))| x == l).map(|(k, _)| k).collect();
        let n = ks.iter().filter(|&&k| covered[k]).count();
        if n < ks.len() {
            open.push(format!("{l} ({n}/{})", ks.len()));
        }
    }
    lines.push(format!("places and riders fully covered: {} of {}{}", labels.len() - open.len(), labels.len(), if open.is_empty() { String::new() } else { format!("; not: {}", open.join(", ")) }));
    for (c, _, o) in &chosen {
        let mob = if pillager { "pillager" } else { "zombie" };
        // (the boat at the cell's centre, x + 0.5 as a number: "{x}.5" read x - 0.5 for a negative x)
        lines.push(format!("entity: minecraft:oak_boat {} {} {} {{Passengers:[{{id:\"minecraft:{mob}\",PersistenceRequired:1b}}]}}", c.x as f64 + 0.5, c.y, c.z as f64 + 0.5));
        // (the post's blocks: stone floor, roof and the three walls, the open side's two cells air)
        let mut blocks = Vec::new();
        if any_floor {
            blocks.push(format!("{},{},{}", c.x, c.y - 1, c.z));
        }
        if let Some(op) = o {
            blocks.push(format!("{},{},{}", c.x, c.y + 2, c.z));
            for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                if (dx, dz) != *op {
                    blocks.push(format!("{},{},{}", c.x + dx, c.y, c.z + dz));
                    blocks.push(format!("{},{},{}", c.x + dx, c.y + 1, c.z + dz));
                }
            }
            lines.push(format!("  its post (stone): {}; the opening {},{},{} and {},{},{} air", blocks.join(" "), c.x + op.0, c.y, c.z + op.1, c.x + op.0, c.y + 1, c.z + op.1));
        } else if !blocks.is_empty() {
            lines.push(format!("  its floor (stone): {}", blocks.join(" ")));
        }
    }
    Ok(lines)
}

const XC_W: usize = 8;
const XC_MAX: usize = 64 * XC_W;
type XcBits = [u64; XC_W];

fn xc_and_not(a: &XcBits, b: &XcBits) -> XcBits {
    let mut r = [0u64; XC_W];
    for i in 0..XC_W {
        r[i] = a[i] & !b[i];
    }
    r
}

fn xc_and(a: &XcBits, b: &XcBits) -> XcBits {
    let mut r = [0u64; XC_W];
    for i in 0..XC_W {
        r[i] = a[i] & b[i];
    }
    r
}

fn xc_or(a: &XcBits, b: &XcBits) -> XcBits {
    let mut r = [0u64; XC_W];
    for i in 0..XC_W {
        r[i] = a[i] | b[i];
    }
    r
}

fn xc_count(b: &XcBits) -> usize {
    b.iter().map(|x| x.count_ones() as usize).sum()
}

fn xc_has(b: &XcBits, k: usize) -> bool {
    b[k / 64] >> (k % 64) & 1 == 1
}

struct XcCtx<'a> {
    cand: &'a [(XcBits, bool, BlockPos)],
    by_pt: &'a [Vec<usize>],
    uni: XcBits,
    maxgain: usize,
    nodes: u64,
    limit: u64,
}

/// One bounded depth-first try for a cover of at most `k` posts with at most `max_p` pillagers
/// (zombie posts only when `allow_z`); two posts whose footprints overlap never both.
fn xc_dfs(cx: &mut XcCtx, chosen: &mut Vec<usize>, cov: XcBits, k: usize, max_p: usize, allow_z: bool, pills: usize, verify: &mut dyn FnMut(&[usize]) -> bool) -> bool {
    cx.nodes += 1;
    if cx.nodes > cx.limit {
        return false;
    }
    let rem: XcBits = xc_and_not(&cx.uni, &cov);
    let r = xc_count(&rem);
    if r == 0 {
        // (a cover on paper: it counts only when it passes the built re-check, every post laid)
        return verify(chosen);
    }
    if chosen.len() >= k || chosen.len() + r.div_ceil(cx.maxgain) > k {
        return false;
    }
    let clash = |a: BlockPos, b: BlockPos| (a.x - b.x).abs() <= 2 && (a.z - b.z).abs() <= 2 && (a.y - b.y).abs() <= 3;
    let usable = |c: usize, chosen: &[usize]| {
        let (_, pill, pos) = cx.cand[c];
        (if pill { pills < max_p } else { allow_z }) && !chosen.iter().any(|&o| clash(pos, cx.cand[o].2))
    };
    let mut best: Option<(usize, usize)> = None;
    for p in 0..XC_MAX {
        if !xc_has(&rem, p) {
            continue;
        }
        let m = cx.by_pt[p].iter().filter(|&&c| usable(c, chosen)).count();
        if m == 0 {
            return false;
        }
        if best.is_none_or(|b| m < b.1) {
            best = Some((p, m));
        }
    }
    let Some((p, _)) = best else { return false };
    let mut opts: Vec<usize> = cx.by_pt[p].iter().copied().filter(|&c| usable(c, chosen)).collect();
    opts.sort_by_key(|&c| {
        let b = cx.cand[c].0;
        std::cmp::Reverse(xc_count(&xc_and(&b, &rem)))
    });
    for c in opts {
        chosen.push(c);
        let b = cx.cand[c].0;
        let ncov = xc_or(&cov, &b);
        if xc_dfs(cx, chosen, ncov, k, max_p, allow_z, pills + cx.cand[c].1 as usize, verify) {
            return true;
        }
        chosen.pop();
    }
    false
}

/// The exact least cover (ironspec Nov 12 01:25): over every candidate post of both kinds (each
/// valid open side its own candidate, every refusal rule applied), the least set covering every
/// rider point some candidate covers; two posts whose footprints (x +-1, z +-1, floor to roof)
/// overlap are never both taken. Three answers: the fewest posts; the fewest pillagers (then the
/// fewest posts); pillagers only. Each by iterative deepening (the uncovered point with the fewest
/// candidates branched first, a counting bound) after dominated candidates are dropped (a cover
/// inside another's whose post has no more pillagers and no more blocks; footprint clashes are not
/// weighed in that drop, so a found set is re-checked built). Points no candidate covers are listed.
fn exact_cover(pts: &[(String, Vec3)], all: &[(BlockPos, Vec<usize>, Option<(i32, i32)>, bool, usize)], maxk: usize, refused: usize, any_floor: bool, transient: &[String], mix: bool, verify: &mut dyn FnMut(&[usize]) -> Option<Vec<usize>>, visors: &HashMap<usize, Vec<BlockPos>>) -> Vec<String> {
    let n = pts.len();
    if n > XC_MAX {
        return vec![format!("exact cover: {n} rider points, over the {XC_MAX} the solver holds")];
    }
    let bits = |v: &[usize]| {
        let mut b = [0u64; XC_W];
        for &k in v {
            b[k / 64] |= 1 << (k % 64);
        }
        b
    };
    // (a declared transient's points are not required: ironspec Nov 12 01:58, a place no resting
    // rider holds, named in the output as flee-check's one exception)
    let mut tmask = [0u64; XC_W];
    for (k, (l, _)) in pts.iter().enumerate() {
        if transient.iter().any(|t| t == l) {
            tmask[k / 64] |= 1 << (k % 64);
        }
    }
    let cb: Vec<XcBits> = all.iter().map(|c| {
        let b = bits(&c.1);
        xc_and_not(&b, &tmask)
    }).collect();
    let mut uni = [0u64; XC_W];
    for b in &cb {
        for i in 0..XC_W {
            uni[i] |= b[i];
        }
    }
    // (the places some point of which no candidate covers)
    let mut lost: Vec<String> = Vec::new();
    for (k, (l, _)) in pts.iter().enumerate() {
        if !xc_has(&uni, k) && !xc_has(&tmask, k) && !lost.contains(l) {
            lost.push(l.clone());
        }
    }
    let sub = |a: &XcBits, b: &XcBits| (0..XC_W).all(|i| a[i] & !b[i] == 0);
    let mut order: Vec<usize> = (0..all.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(xc_count(&cb[i])), all[i].3, all[i].4));
    let mut kept: Vec<usize> = Vec::new();
    for &i in &order {
        // (only a post's own other open sides drop it: a cross-cell drop can lose every answer
        // when the dominating posts' footprints clash, so the search stays exact)
        if kept.iter().any(|&j| all[j].0 == all[i].0 && sub(&cb[i], &cb[j]) && (!all[j].3 || all[i].3) && all[j].4 <= all[i].4) {
            continue;
        }
        kept.push(i);
    }
    let cand: Vec<(XcBits, bool, BlockPos)> = kept.iter().map(|&i| (cb[i], all[i].3, all[i].0)).collect();
    let by_pt: Vec<Vec<usize>> = (0..XC_MAX).map(|k| if k < n { (0..cand.len()).filter(|&c| xc_has(&cand[c].0, k)).collect() } else { Vec::new() }).collect();
    let maxgain = cand.iter().map(|c| xc_count(&c.0)).max().unwrap_or(1).max(1);
    let np = all.iter().filter(|c| c.3).count();
    let mut lines = vec![format!(
        "exact cover (both kinds seated in boats, every valid open side: a pillager covers 8.5..15 in sight, none it sees inside 8.5 (the crossbow's 8, physical, + 0.5 inherited); a zombie covers 6.05..7.95 in sight, none inside 6.05 (6 and 8 physical, the 0.05 margins inherited), sight or not): {} candidates ({np} pillager, {} zombie), {} kept after dominance, {refused} cells refused outright; {n} rider points, {} coverable{}",
        all.len(),
        all.len() - np,
        cand.len(),
        xc_count(&uni),
        if lost.is_empty() { String::new() } else { format!("; NOT coverable by any post: {}", lost.join(", ")) } + &if transient.is_empty() { String::new() } else { format!("; declared transient (not required; flee-check's one exception): {}", transient.join(", ")) }
    )];
    let mut cx = XcCtx { cand: &cand, by_pt: &by_pt, uni, maxgain, nodes: 0, limit: 0 };
    let uni_c = uni;
    let mut run = |k: usize, max_p: usize, allow_z: bool| -> (Option<Vec<usize>>, bool) {
        cx.nodes = 0;
        cx.limit = 20_000_000;
        let mut chosen = Vec::new();
        // (the built re-check: every chosen post laid, each re-scored against every rider point;
        // a refusal or a lost point sends the search on)
        let mut v = |sel: &[usize]| -> bool {
            let idx: Vec<usize> = sel.iter().map(|&c| kept[c]).collect();
            match verify(&idx) {
                None => false,
                Some(cov) => {
                    let b = bits(&cov);
                    (0..XC_W).all(|i| uni_c[i] & !b[i] == 0)
                }
            }
        };
        let ok = xc_dfs(&mut cx, &mut chosen, [0; XC_W], k, max_p, allow_z, 0, &mut v);
        (if ok { Some(chosen) } else { None }, cx.nodes > cx.limit)
    };
    let mut answers: Vec<(String, Vec<usize>)> = Vec::new();
    let mut cut = false;
    // (1) the fewest posts
    for k in 1..=maxk {
        let (s, c) = run(k, usize::MAX, true);
        cut |= c;
        if let Some(s) = s {
            answers.push((format!("the fewest posts: {k}"), s));
            break;
        }
    }
    if mix {
        // (the mix (vjeux via the parent, Nov 12 02:52: scarer mobs are their own bill line): from
        // the fewest posts' pillagers down, the least passing cover with at most p pillagers)
        let p0 = answers.first().map(|(_, s)| s.iter().filter(|&&c| cand[c].1).count()).unwrap_or(maxk);
        for p in (0..p0).rev() {
            let mut found = false;
            for k in p.max(1)..=maxk {
                let (s, c) = run(k, p, true);
                cut |= c;
                if let Some(s) = s {
                    answers.push((format!("mix, at most {p} pillagers: {k} posts"), s));
                    found = true;
                    break;
                }
            }
            if !found {
                lines.push(format!("mix: no passing cover within {maxk} posts with at most {p} pillagers"));
                break;
            }
        }
    } else {
        // (2) the fewest pillagers, then the fewest posts
        'p: for p in 0..=maxk {
            for k in p.max(1)..=maxk {
                let (s, c) = run(k, p, true);
                cut |= c;
                if let Some(s) = s {
                    answers.push((format!("the fewest pillagers: {p} ({k} posts)"), s));
                    break 'p;
                }
            }
        }
        // (3) pillagers only
        for k in 1..=maxk {
            let (s, c) = run(k, usize::MAX, false);
            cut |= c;
            if let Some(s) = s {
                answers.push((format!("pillagers only: {k}"), s));
                break;
            }
        }
    }
    if cut {
        lines.push("(a search hit its 20M-node cap: an answer below may not be the least)".to_string());
    }
    // (the proof: the dominance drop above ignores footprint clashes, so the fewest posts found
    // is proven least by a search one post short over the candidates with only same-cell
    // dominance (a post's other open sides))
    if let Some((_, s)) = answers.first() {
        let k = s.len();
        let mut kc: Vec<usize> = Vec::new();
        for &i in &order {
            if kc.iter().any(|&j| all[j].0 == all[i].0 && sub(&cb[i], &cb[j]) && (!all[j].3 || all[i].3) && all[j].4 <= all[i].4) {
                continue;
            }
            kc.push(i);
        }
        let cand2: Vec<(XcBits, bool, BlockPos)> = kc.iter().map(|&i| (cb[i], all[i].3, all[i].0)).collect();
        let by_pt2: Vec<Vec<usize>> = (0..XC_MAX).map(|p| if p < n { (0..cand2.len()).filter(|&c| xc_has(&cand2[c].0, p)).collect() } else { Vec::new() }).collect();
        let mut cx2 = XcCtx { cand: &cand2, by_pt: &by_pt2, uni, maxgain, nodes: 0, limit: 200_000_000 };
        let mut chosen = Vec::new();
        let found = k > 1 && xc_dfs(&mut cx2, &mut chosen, [0; XC_W], k - 1, usize::MAX, true, 0, &mut |_: &[usize]| true);
        lines.push(if found {
            format!("proof: a {}-post cover exists among the {} candidates without the cross-cell drop (the answer below is not the least)", k - 1, cand2.len())
        } else if cx2.nodes > cx2.limit {
            format!("proof: not settled (the {}-post search over {} candidates hit its 200M-node cap)", k - 1, cand2.len())
        } else {
            format!("proof: no {}-post cover among the {} candidates without the cross-cell drop ({} nodes): {k} posts is the least", k - 1, cand2.len(), cx2.nodes)
        });
    }
    let side = |o: Option<(i32, i32)>| match o {
        Some((0, -1)) => "open north",
        Some((1, 0)) => "open east",
        Some((0, 1)) => "open south",
        Some((-1, 0)) => "open west",
        _ => "no walls",
    };
    for (what, set) in &answers {
        let posts: Vec<usize> = set.iter().map(|&c| kept[c]).collect();
        let pills = posts.iter().filter(|&&i| all[i].3).count();
        let blocks: usize = posts.iter().map(|&i| all[i].4).sum();
        lines.push(format!("{what}: {} pillager(s), {} zombie post(s), {blocks} placed blocks (+ a boat each)", pills, posts.len() - pills));
        for &i in &posts {
            let (c, cov, o, pill, nb) = &all[i];
            let mut b = Vec::new();
            if any_floor {
                b.push(format!("{},{},{}", c.x, c.y - 1, c.z));
            }
            if let Some(op) = o {
                b.push(format!("{},{},{}", c.x, c.y + 2, c.z));
                for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                    if (dx, dz) != *op {
                        b.push(format!("{},{},{}", c.x + dx, c.y, c.z + dz));
                        b.push(format!("{},{},{}", c.x + dx, c.y + 1, c.z + dz));
                    }
                }
            }
            lines.push(format!(
                "  {} {},{},{} {} (covers {} points; {nb} placed): stone {}{}",
                if *pill { "pillager" } else { "zombie" },
                c.x,
                c.y,
                c.z,
                side(*o),
                cov.len(),
                b.join(" "),
                match visors.get(&i) {
                    Some(v) => format!("; visor (glass, on the pad golems' rays only) {}", v.iter().map(|p| format!("{},{},{}", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ")),
                    None => String::new(),
                }
            ));
        }
    }
    if answers.is_empty() {
        lines.push(format!("no cover within {maxk} posts"));
    }
    lines
}

/// Whether an iron golem standing on any cell of the pad `lo..=hi` (feet cells) has a path to
/// within 2 of `target` (ironspec Nov 11 23:27: golems target an Enemy in follow range with no sight
/// needed and walk at it when they can reach it): one golem moved cell to cell, `create_path_pos`
/// with reach 2 from each (the world settled, not ticked between cells).
pub fn golem_reach(spec: &FarmSpec, lo: [i32; 3], hi: [i32; 3], target: BlockPos) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let id = redsim::entity::spawn(w, "minecraft:iron_golem", [lo[0] as f64 + 0.5, lo[1] as f64, lo[2] as f64 + 0.5], Some("{PersistenceRequired:1b}"))?;
    let Some(i) = redsim::entity::index_of(w, id) else { return Err("the golem did not spawn".into()) };
    let (mut tried, mut reach) = (0usize, Vec::new());
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let feet = BlockPos::new(x, y, z);
                if !w.get(feet).is_air() || w.get(feet.below()).is_air() {
                    continue;
                }
                tried += 1;
                w.entities.list[i].pos = Vec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5);
                w.entities.list[i].on_ground = true;
                let ok = redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
                    let mut copy = nav.clone();
                    copy.create_path_pos(w, nm, target, 2, arena).is_some_and(|p| p.reached)
                });
                if ok {
                    reach.push(format!("{x},{y},{z}"));
                }
            }
        }
    }
    Ok(vec![format!(
        "golem paths to within 2 of {},{},{} from the pad x {}..{} y {}..{} z {}..{}: {} of {} standable cells have one{}",
        target.x, target.y, target.z, lo[0], hi[0], lo[1], hi[1], lo[2], hi[2],
        reach.len(),
        tried,
        if reach.is_empty() { String::new() } else { format!(": {}", reach.iter().take(20).cloned().collect::<Vec<_>>().join(" ")) }
    )])
}

/// A post's drift (ironspec Nov 11 23:57: pin the boat and print its position spread): a boat with
/// a zombie (the stand-in seat: redsim does not spawn pillagers; the boat's motion does not read
/// its passenger's kind) placed at `cell` in the settled world, ticked `ticks` times; the boat's and
/// the seat's least and greatest coordinates.
pub fn post_drift(spec: &FarmSpec, cell: BlockPos, ticks: u32) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let boat = redsim::entity::spawn(&mut run.world, "minecraft:oak_boat", [cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    let mut seat_dy = (f64::MAX, f64::MIN);
    for _ in 0..ticks {
        run.tick();
        let w = &run.world;
        let Some(bi) = redsim::entity::index_of(w, boat) else { return Ok(vec![format!("post drift at {},{},{}: the boat is gone", cell.x, cell.y, cell.z)]) };
        let p = w.entities.list[bi].pos;
        for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            lo[k] = lo[k].min(v);
            hi[k] = hi[k].max(v);
        }
        if let Some(&zid) = w.entities.list[bi].passengers.first()
            && let Some(zi) = redsim::entity::index_of(w, zid)
        {
            let d = w.entities.list[zi].pos.y - p.y;
            seat_dy = (seat_dy.0.min(d), seat_dy.1.max(d));
        }
    }
    Ok(vec![format!(
        "post drift at {},{},{} over {ticks} ticks: the boat x {:.4}..{:.4} y {:.4}..{:.4} z {:.4}..{:.4} (spread {:.4} / {:.4} / {:.4}); its passenger {} the boat",
        cell.x, cell.y, cell.z, lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2],
        if seat_dy.0 == f64::MAX { "left".to_string() } else { format!("stayed in, seat {:+.4}..{:+.4} over", seat_dy.0, seat_dy.1) }
    )])
}

pub fn stamp_classes(places: &[Place], each: bool) -> Vec<String> {
    let mut rows = Vec::new();
    let (mut ac, mut an, mut na) = (0usize, 0usize, 0usize);
    for p in places {
        let panicking = p.seen > 0;
        let srcs = stamp_sources(p);
        let class = stamp_class(p);
        match class {
            "arms and clears" => ac += 1,
            "arms, never clears" => an += 1,
            _ => na += 1,
        }
        if each || class == "arms, never clears" {
            let detail: Vec<String> = srcs
                .iter()
                .map(|(n, s)| {
                    if *n == "walks to riders" {
                        // (each rider within 8 is its own target: counts, not a draw's chance)
                        format!("{n}: {} riders within 8, {} unreached", s.cells, s.live())
                    } else {
                        format!("{n}: {} cells, {} unreached (an event picks one {:.1}%), a reached one {:.1}%", s.cells, s.live(), 100.0 * s.risk(), 100.0 * s.reached_share)
                    }
                })
                .collect();
            rows.push(format!("  STAMP {},{},{}{}: {class}{}; {} ({}){}", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, if panicking { "" } else { " (day: classed on IDLE's sources; REST's HOME walks not modeled)" }, if panicking { "panicking" } else { "calm" }, detail.join("; "), if p.village { "; in a village" } else { "" }));
        }
    }
    let mut lines = vec![format!(
        "stamp-arming classes ({} places; a prediction for the bills, not a bar): arms and clears {ac}, arms, never clears {an}, never arms {na}",
        places.len()
    )];
    lines.extend(rows);
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
        "what can clear a stamp, per place (design numbers, not a bar; the oldest-count rows decide): {} panicking places (a hostile seen within its range: a zombie 8, a declared --pillager 15) with a reachable panic-stroll cell, {} with none; {} calm places whose calm stroll can pick a reachable cell, {} with none",
        pan_ok, pan_none, calm_ok, calm_none
    ));
    for p in places.iter().filter(|p| each || (panicking(p) && p.panic_stroll.cells <= p.panic_stroll.unreached.len()) || (!panicking(p) && p.stroll.reached_share <= 1e-9)) {
        let tag = if each { "CLEARING" } else { "NO-CLEARING" };
        if panicking(p) {
            lines.push(format!("  {tag} {},{},{}{}: panicking ({} seen within 8); the panic stroll reaches {} of its {} cells (an event picks one {:.0}%)", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, p.seen, p.panic_stroll.cells - p.panic_stroll.unreached.len(), p.panic_stroll.cells, 100.0 * p.panic_stroll.reached_share));
        } else {
            lines.push(format!("  {tag} {},{},{}{}: calm, day (no hostile seen within its range; IDLE's sources; REST's HOME walks not modeled); the calm stroll reaches {} of its {} cells (an event picks one {:.1}%)", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, p.stroll.cells - p.stroll.unreached.len(), p.stroll.cells, 100.0 * p.stroll.reached_share));
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
        "the 6-8 rule (every zombie at least {FLEE_BAR} from every rider position, where it sits in its post; a declared panic place sees one within {PANIC_BAR}; the bars: SetWalkTargetAwayFrom's 6 and the sensor's 8, physical, with the 0.05 margins inherited, ironspec Nov 11 10:54): {} rider places inside {FLEE_BAR}, {} whose nearest zombie is beyond {PANIC_BAR} somewhere, {} at {FLEE_BAR}..{PANIC_BAR} throughout; {} places declared in panic:, {} of them without a seen zombie within {PANIC_BAR} at some point",
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

/// (ironspec Nov 12 08:24, an occluder's hidden riders) Every rider position on the rails inside a
/// box: each rail cell's positions every 0.05 along it (`along_rail`; the cart's position, its
/// rider's feet).
pub fn rail_positions_in(spec: &FarmSpec, lo: [i32; 3], hi: [i32; 3]) -> Vec<Vec3> {
    let mut v = Vec::new();
    for (c, b) in spec.blocks.iter() {
        if c.x < lo[0] || c.x > hi[0] || c.y < lo[1] || c.y > hi[1] || c.z < lo[2] || c.z > hi[2] {
            continue;
        }
        let s = b.to_string();
        if !s.contains("rail") {
            continue;
        }
        for i in 0..=20 {
            v.push(along_rail(*c, &s, i as f64 / 20.0));
        }
    }
    v
}

/// Every rider position the all-positions statics try: each rail cell every 0.1 along it (t 0.05 ..
/// 0.95: a cart rolling through passes them all) and on an ascending rail at 0.97 too (the seat
/// over the cell above: the stroll's origin block changes), with its cell and t; in (y, z, x, t)
/// order.
fn rider_positions(spec: &FarmSpec) -> Vec<(BlockPos, f64, Vec3)> {
    let mut v = Vec::new();
    for (c, b) in spec.blocks.iter() {
        let s = b.to_string();
        if !s.contains("rail") {
            continue;
        }
        let mut ts: Vec<f64> = (0..10).map(|i| 0.05 + 0.1 * i as f64).collect();
        if s.contains("ascending") {
            ts.push(0.97);
        }
        for t in ts {
            v.push((*c, t, along_rail(*c, &s, t)));
        }
    }
    v.sort_by(|a, b| (a.0.y, a.0.z, a.0.x).cmp(&(b.0.y, b.0.z, b.0.x)).then(a.1.total_cmp(&b.1)));
    v
}

/// A rail cell's two exits: each a horizontal step, climbing one on an ascending rail's high side.
fn rail_exits(state: &str) -> Vec<(i32, i32, i32)> {
    let shape = state.split("shape=").nth(1).and_then(|s| s.split([',', ']']).next()).unwrap_or("");
    match shape {
        "north_south" => vec![(0, 0, -1), (0, 0, 1)],
        "east_west" => vec![(-1, 0, 0), (1, 0, 0)],
        "ascending_east" => vec![(-1, 0, 0), (1, 1, 0)],
        "ascending_west" => vec![(1, 0, 0), (-1, 1, 0)],
        "ascending_north" => vec![(0, 0, 1), (0, 1, -1)],
        "ascending_south" => vec![(0, 0, -1), (0, 1, 1)],
        "north_east" => vec![(0, 0, -1), (1, 0, 0)],
        "north_west" => vec![(0, 0, -1), (-1, 0, 0)],
        "south_east" => vec![(0, 0, 1), (1, 0, 0)],
        "south_west" => vec![(0, 0, 1), (-1, 0, 0)],
        _ => Vec::new(),
    }
}

/// (G1's CHANGES 1 on d3dc5aa46, ironspec Nov 12 15:03) A walk's track ends with time left: every
/// node it reached with `window - d` > 0 that has an open end (`Track::open_ends`), not a rest; the
/// remainder kept per slot (0 the tail, 1 the late-path walks, 2 the approach walks), the most over
/// the walks.
fn record_ends(track: &Track, reached: &HashMap<TailNode, f64>, window: f64, slot: usize, ends: &mut std::collections::BTreeMap<String, (Vec<BlockPos>, [f64; 3])>) {
    for (n, d) in reached {
        if let TailNode::Rail(x, y, z) = n
            && track.rests.contains(&BlockPos::new(*x, *y, *z))
        {
            continue;
        }
        let left = window - d;
        if left <= 0.0 {
            continue;
        }
        for (desc, cells) in track.open_ends(*n) {
            let e = ends.entry(desc).or_insert((cells, [0.0; 3]));
            e.1[slot] = e.1[slot].max(left);
        }
    }
}

/// (G1's CHANGES 1 on d3dc5aa46, ironspec Nov 12 15:02 / 15:03: a path computed in free fall starts
/// at the landing, getStart's airborne scan; a path computed just above a lift's surface starts at
/// its foot, ff1ca056a) The design's own columns for the walks, beside the declared ones: every
/// bubble column (a vertical run of bubble_column cells: up over soul sand, down over magma), an up
/// column with its exit cell above the surface when that cell is open (no collision shape, no
/// rail): the cart lifted out of the water before the flow pushes it onto the exit rail; and every
/// free fall: a rail exit whose cell and the cell below hold no rail and have no collision shape,
/// and from which no column is entered, down from the exit cell to the first rail (the landing) or
/// collision block (a floor without a rail: its end is a track end); a bubble run at an x,z a
/// declared column covers is skipped, a fall out of the design's box is left to the ends report.
/// Timed like a column (1 block a tick: a bound, the fall is slower). Returns the columns and a line
/// for each.
pub fn auto_columns(spec: &FarmSpec, declared: &[[f64; 4]]) -> (Vec<[f64; 4]>, Vec<String>) {
    let blk: HashMap<BlockPos, redsim::block::Block> = spec.blocks.iter().map(|(p, b)| (*p, *b)).collect();
    let name = |p: BlockPos| blk.get(&p).map(|b| b.to_string()).unwrap_or_else(|| "minecraft:air".to_string());
    let short = |p: BlockPos| name(p).trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
    let rail = |p: BlockPos| name(p).contains("rail");
    let open = |p: BlockPos| blk.get(&p).is_none_or(|b| redsim::behavior::collision_boxes(b).is_empty());
    let near = |a: BlockPos, b: BlockPos| (a.x - b.x).abs() <= 1 && (a.y - b.y).abs() <= 1 && (a.z - b.z).abs() <= 1;
    let first = |c: &[f64; 4]| BlockPos::new(c[0].floor() as i32, c[1].floor() as i32, c[2].floor() as i32);
    let last = |c: &[f64; 4]| BlockPos::new(c[0].floor() as i32, c[3].floor() as i32, c[2].floor() as i32);
    let mut out: Vec<[f64; 4]> = Vec::new();
    let mut lines = Vec::new();
    // (the bubble columns)
    let mut cells: std::collections::BTreeMap<(i32, i32), Vec<(i32, bool)>> = std::collections::BTreeMap::new();
    for (p, b) in &spec.blocks {
        let s = b.to_string();
        if s.contains("bubble_column") {
            cells.entry((p.x, p.z)).or_default().push((p.y, s.contains("drag=true")));
        }
    }
    for ((x, z), mut ys) in cells {
        if declared.iter().any(|c| c[0].floor() as i32 == x && c[2].floor() as i32 == z) {
            continue;
        }
        ys.sort();
        let mut start = 0;
        for k in 1..=ys.len() {
            if k == ys.len() || ys[k].0 != ys[k - 1].0 + 1 {
                let (lo, hi, down) = (ys[start].0, ys[k - 1].0, ys[start].1);
                if down {
                    out.push([x as f64 + 0.5, hi as f64, z as f64 + 0.5, lo as f64]);
                    lines.push(format!("auto column: x {x} z {z} y {lo}..{hi} (down, magma)"));
                } else {
                    let top = BlockPos::new(x, hi + 1, z);
                    let ext = open(top) && !rail(top);
                    out.push([x as f64 + 0.5, lo as f64, z as f64 + 0.5, if ext { (hi + 1) as f64 } else { hi as f64 }]);
                    lines.push(format!(
                        "auto column: x {x} z {z} y {lo}..{hi} (up, soul sand){}",
                        if ext { format!(", with its exit cell above the surface {x},{},{z} ({}): a path computed there starts at the column's foot", hi + 1, short(top)) } else { String::new() }
                    ));
                }
                start = k;
            }
        }
    }
    // (the free falls)
    let mut rails: Vec<(BlockPos, String)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    rails.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    for (c, s) in &rails {
        for (dx, dy, dz) in rail_exits(s) {
            if dx == 0 && dz == 0 {
                continue;
            }
            let m = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
            if rail(m) || rail(m.below()) || !open(m) || !open(m.below()) {
                continue;
            }
            if declared.iter().chain(out.iter()).any(|k| near(first(k), *c) || near(last(k), *c)) {
                continue;
            }
            let mut y = m.y - 1;
            let mut land: Option<(BlockPos, bool)> = None;
            while y >= spec.lo.y {
                let q = BlockPos::new(m.x, y, m.z);
                if rail(q) {
                    land = Some((q, true));
                    break;
                }
                if !open(q) {
                    land = Some((q, false));
                    break;
                }
                y -= 1;
            }
            let Some((l, on_rail)) = land else { continue };
            let beyond = BlockPos::new(m.x + dx, m.y, m.z + dz);
            let stop = if open(beyond) { format!("nothing stops it at {},{},{}: it may fly on, the fall taken straight down from the exit cell", beyond.x, beyond.y, beyond.z) } else { format!("{} {},{},{} stops it", short(beyond), beyond.x, beyond.y, beyond.z) };
            out.push([m.x as f64 + 0.5, m.y as f64, m.z as f64 + 0.5, (l.y + 1) as f64]);
            lines.push(format!(
                "auto column: a free fall off the rail end {},{},{} into {},{},{} ({stop}), down {} blocks to {} {},{},{}{}",
                c.x,
                c.y,
                c.z,
                m.x,
                m.y,
                m.z,
                m.y - l.y - 1,
                if on_rail { "the rail" } else { "the floor" },
                l.x,
                l.y,
                l.z,
                if on_rail { String::new() } else { format!(" ({}: no rail, a track end)", short(l)) }
            ));
        }
    }
    (out, lines)
}

/// (farmopt / ironspec Nov 12 11:13-11:14: the statics refuse an unsupported attachable) Every block
/// of the design that would not survive where it is placed, on the paste: redsim's
/// `behavior::can_survive` for the design's own block (dust or a diode without a sturdy top under
/// it, a tripwire hook without its anchor, a torch, lever or button without its face, a rail
/// without its support, ...) and `rails::would_pop` (an ascending rail without its high side's
/// block); a rail already gone after the paste counts too; and (ironspec's audit 11:05) every
/// cell outside the world's box, which `place_raw` would skip. Each listed with the block under it.
pub fn unsupported(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let run = Run::start(spec, &o)?;
    let w = &run.world;
    let mut v = Vec::new();
    for (p, b) in &spec.blocks {
        if b.is_air() {
            continue;
        }
        let label = b.to_string();
        // (ironspec's audit Nov 12 11:05: `World::place_raw` silently skips a cell outside the
        // world's box; every such cell is listed, never dropped)
        if !w.contains(*p) {
            v.push(format!("{} at {},{},{} outside the world's box, never placed", label.trim_start_matches("minecraft:"), p.x, p.y, p.z));
            continue;
        }
        let rail = label.contains("rail");
        let now = w.get(*p);
        let gone = rail && !now.to_string().contains("rail");
        if gone || !redsim::behavior::can_survive(w, b, *p) || (rail && redsim::rails::would_pop(w, *p)) {
            v.push(format!(
                "{} at {},{},{} (under it: {}){}",
                label.trim_start_matches("minecraft:"),
                p.x,
                p.y,
                p.z,
                w.get(p.below()).to_string().trim_start_matches("minecraft:"),
                if gone { ", already gone after the paste" } else { "" }
            ));
        }
    }
    Ok(v)
}

/// (ironspec Nov 12 12:13 / tick-exact's official `panic-lift` row 12:29) Whether a seated rider's
/// path reaches each target from each cart position: (cart position, target) pairs, the cart there
/// and the rider seated, a fresh path (redsim's start-node rule for a passenger: inside a water
/// column the path is the water's surface, one node).
fn reach_pairs(spec: &FarmSpec, home: Option<BlockPos>, claims: &[BlockPos], pairs: &[(Vec3, BlockPos)]) -> Result<Vec<bool>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    for c in claims {
        redsim::entity::mob::ai::poi::take(w, *c);
    }
    if !claims.is_empty() {
        redsim::entity::mob::ai::poi::note_village_centres(w);
    }
    let Some((p0, _)) = pairs.first() else { return Ok(Vec::new()) };
    let cart = redsim::entity::spawn(w, "minecraft:minecart", [p0.x, p0.y, p0.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
    let ci = redsim::entity::index_of(w, cart).ok_or("the cart")?;
    let vid = *w.entities.list[ci].passengers.first().ok_or("the rider")?;
    let i = redsim::entity::index_of(w, vid).ok_or("the rider")?;
    if let Some(h) = home
        && let EntityKind::Mob(m) = &mut w.entities.list[i].kind
        && let Some(b) = m.brain.as_deref_mut()
    {
        b.home = Some(h);
    }
    let mut out = Vec::with_capacity(pairs.len());
    for (q, t) in pairs {
        let ci = redsim::entity::index_of(w, cart).ok_or("the cart")?;
        w.entities.list[ci].pos = *q;
        redsim::entity::position_rider(w, ci, i);
        // (the rider's fluid state there: in a water column the path starts at the surface)
        redsim::entity::fluid_push::update_fluid_interaction(w, i);
        let ok = redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
            let mut copy = nav.clone();
            copy.create_path_pos(w, nm, *t, 0, arena).is_some_and(|p| p.reached)
        });
        out.push(ok);
    }
    Ok(out)
}

/// (ironspec Nov 12 12:36, the positive control against tick-exact's ff1ca056a `panic-lift` row) A
/// seated rider's path from a cart position to a target: its nodes (start first), whether it
/// reaches the target.
pub fn path_probe(spec: &FarmSpec, cart_at: Vec3, target: BlockPos) -> Result<String, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &mut run.world;
    let cart = redsim::entity::spawn(w, "minecraft:minecart", [cart_at.x, cart_at.y, cart_at.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
    let ci = redsim::entity::index_of(w, cart).ok_or("the cart")?;
    let vid = *w.entities.list[ci].passengers.first().ok_or("the rider")?;
    let i = redsim::entity::index_of(w, vid).ok_or("the rider")?;
    w.entities.list[ci].pos = cart_at;
    redsim::entity::position_rider(w, ci, i);
    // (the rider's fluid state there, as a tick's baseTick leaves it: in a water column the path
    // starts at the water's surface)
    redsim::entity::fluid_push::update_fluid_interaction(w, i);
    let feet = w.entities.list[i].pos;
    let below = |w: &redsim::World, p: Vec3| w.get(BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)).to_string();
    let here = below(w, feet);
    let path = redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
        let mut copy = nav.clone();
        copy.create_path_pos(w, nm, target, 0, arena)
    });
    Ok(match path {
        None => format!("path probe: the rider's feet {:.3},{:.3},{:.3} (in {}), target {},{},{}: no path", feet.x, feet.y, feet.z, here.trim_start_matches("minecraft:"), target.x, target.y, target.z),
        Some(p) => format!(
            "path probe: the rider's feet {:.3},{:.3},{:.3} (in {}), target {},{},{}: {} node(s), {}: {}",
            feet.x,
            feet.y,
            feet.z,
            here.trim_start_matches("minecraft:"),
            target.x,
            target.y,
            target.z,
            p.node_count(),
            if p.reached { "reached" } else { "partial" },
            p.nodes.iter().map(|n| format!("{},{},{}", n.x, n.y, n.z)).collect::<Vec<_>>().join(" ")
        ),
    })
}

/// (ironspec Nov 12 15:35: a permanent statics row) Every caller's scarers: each pocket rider at its
/// rest point (the stand at rest, as at every summon tick: the scarers sit still), every hostile
/// within the villager's sensing range (VillagerHostilesSensor: a zombie 8, a pillager 15), its
/// distance and whether the rider sees it eye to eye; a caller that sees none does not panic and
/// its summon does not run: FAIL. Lines and the count of blind callers.
pub fn caller_sight(spec: &FarmSpec, pills: &[Vec3], callers: &[(String, Vec3)]) -> Result<(Vec<String>, usize), String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    while run.settle_tick() {}
    let w = &run.world;
    let zs = hostiles(spec);
    let seat = zombie_seat();
    let dist = |a: Vec3, b: Vec3| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let mut lines = Vec::new();
    let mut blind = 0usize;
    for (name, p) in callers {
        let eye = Vec3::new(p.x, p.y + 1.62, p.z);
        let (mut seen, mut hidden): (Vec<(f64, String)>, Vec<(f64, String)>) = (Vec::new(), Vec::new());
        for z in &zs {
            let f = Vec3::new(z.x, z.y + seat, z.z);
            let d = dist(f, *p);
            if d > 8.0 {
                continue;
            }
            let s = redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z));
            (if s { &mut seen } else { &mut hidden }).push((d, format!("the zombie at {:.1},{:.1},{:.1} at {d:.2} ({}, ironspec Nov 12 16:02)", z.x, z.y, z.z, if d >= FLEE_BAR { format!("+{:.2} over the {FLEE_BAR} flee bar", d - FLEE_BAR) } else { format!("{:.2} INSIDE the {FLEE_BAR} flee bar: the flee runs", d - FLEE_BAR) })));
        }
        for f in pills {
            let d = dist(*f, *p);
            if d > PILLAGER_BAR {
                continue;
            }
            let s = redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + PILLAGER_EYE, f.z));
            (if s { &mut seen } else { &mut hidden }).push((d, format!("the pillager at {:.1},{:.1},{:.1} at {d:.2}", f.x, f.y, f.z)));
        }
        seen.sort_by(|a, b| a.0.total_cmp(&b.0));
        hidden.sort_by(|a, b| a.0.total_cmp(&b.0));
        if seen.is_empty() {
            blind += 1;
        }
        lines.push(format!(
            "  CALLER {name} rider {:.2},{:.2},{:.2}: {}{}",
            p.x,
            p.y,
            p.z,
            if seen.is_empty() { "sees NO scarer: FAIL (it does not panic; its summon does not run)".to_string() } else { format!("sees {}", seen.iter().map(|(_, s)| s.clone()).collect::<Vec<_>>().join(", ")) },
            if hidden.is_empty() { String::new() } else { format!("; in range but hidden: {}", hidden.iter().map(|(_, s)| s.clone()).collect::<Vec<_>>().join(", ")) }
        ));
    }
    let mut out = vec![format!(
        "caller sight (ironspec Nov 12 15:35, a permanent row; the bar, each caller seeing a scarer, an inherited policy: each pocket rider at its rest point, as at every summon tick, and each hostile within the villager's sensing range (a zombie 8, a pillager 15, physical) with its distance, eye to eye): {}",
        if callers.is_empty() {
            "no callers (the design's run-cells P0.. or --caller)".to_string()
        } else if blind == 0 {
            format!("every one of {} callers sees a scarer", callers.len())
        } else {
            format!("FAIL: {blind} of {} callers see none", callers.len())
        }
    )];
    out.extend(lines);
    Ok((out, blind))
}

/// (G1's CHANGES 2 on fee712f72) A zombie's hidden position within the flee bar needs this travel
/// time (gt) from the last position the zombie sees within its panic range.
const HIDDEN_BAR_GT: f64 = 40.0;

/// The tail's speeds (G1's CHANGES 2 on fee712f72): a ridden cart moves 0.75 x its motion clamped
/// to getMaxSpeed 0.4 a game tick on each axis, so a straight or sloped rail cell takes at least
/// 1 / 0.4 = 2.5 gt and a curve cell, crossed on its diagonal at up to 0.4 on each axis, half of
/// that (0.5 / 0.4 = 1.25 gt); in a lift's bubble column or a drop's free fall the bound is 1
/// block a tick.
const TAIL_RAIL_SPEED: f64 = 0.4;
const TAIL_COLUMN_SPEED: f64 = 1.0;
/// MoveToTargetSink's longest run for a villager (VillagerGoalPackages: 150..250 gt): a walk target
/// set with a partial path keeps that path at most this long.
const MOVE_SINK_MAX: f64 = 250.0;

/// A node of the tail's walk: a rail cell, or a column position (column, index).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum TailNode {
    Rail(i32, i32, i32),
    Col(usize, usize),
}

/// The track a rider travels, for the shortest-time walks (the tail, the hidden positions' times,
/// the stale-path hazard): the design's rail cells and the `columns` (x, y_from, z, y_to, every
/// 0.25, only from y_from toward y_to, entered at the start from a rail whose exit leads into its
/// first cell or that it drops out of, and left at the end onto the rail it feeds); a `rests` cell
/// (a hold) stops a walk. Along a rail either way, until `orient` keeps each link to the way carts
/// travel it (ironspec Nov 12 16:02, the default).
struct Track<'a> {
    rails: HashMap<BlockPos, String>,
    columns: &'a [[f64; 4]],
    col_len: Vec<usize>,
    rests: &'a [BlockPos],
    /// (`orient`) the moves the walks may take; `None`: every move, either way along a rail.
    allowed: Option<std::collections::HashSet<(TailNode, TailNode)>>,
    /// (G1's note 4 on a4cd9b8b0) The switched Ts: plain rails with a neighbouring rail on 3 or 4
    /// sides (level, one up or one down) and a signal source (a lever, torch, repeater, comparator,
    /// detector rail, redstone block or dust, button, observer, plate, ...) within Manhattan 2
    /// (ironspec's reach rule for placement-order Ts): a cart may take either shape, so their
    /// moves are the union of every arm, and their shapes orient nothing.
    switched_ts: std::collections::HashSet<BlockPos>,
    /// The cells with a collision shape (a rail exit into one is a wall the cart stops at, not a
    /// track end: the open ends are falls and roll-offs).
    solid: std::collections::HashSet<BlockPos>,
}

impl<'a> Track<'a> {
    fn new(spec: &FarmSpec, columns: &'a [[f64; 4]], rests: &'a [BlockPos]) -> Track<'a> {
        let rails: HashMap<BlockPos, String> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
        // (G1's note (c) on fa1267b75: the signal sources are redsim's own `isSignalSource`, not
        // another list; a one-cell world answers it for dust (the default-state answer RailBlock
        // reads))
        let w0 = redsim::World::new(BlockPos::new(0, 0, 0), [1, 1, 1]);
        let sources: Vec<BlockPos> = spec.blocks.iter().filter(|(_, b)| redsim::behavior::is_signal_source(&w0, b)).map(|(p, _)| *p).collect();
        let solid = spec.blocks.iter().filter(|(_, b)| !redsim::behavior::collision_boxes(b).is_empty()).map(|(p, _)| *p).collect();
        Track::from_parts(rails, columns, rests, sources, solid)
    }

    /// A track from its rail cells (game strings), columns, rests and signal sources.
    fn from_parts(rails: HashMap<BlockPos, String>, columns: &'a [[f64; 4]], rests: &'a [BlockPos], sources: Vec<BlockPos>, solid: std::collections::HashSet<BlockPos>) -> Track<'a> {
        let col_len = columns.iter().map(|c| ((c[3] - c[1]).abs() / 0.25).round() as usize + 1).collect();
        let mut switched_ts = std::collections::HashSet::new();
        for (c, s) in &rails {
            if !s.contains("minecraft:rail[") {
                continue;
            }
            let arms = Track::arm_rails(&rails, *c).iter().filter(|v| !v.is_empty()).count();
            if arms >= 3 && sources.iter().any(|q| (q.x - c.x).abs() + (q.y - c.y).abs() + (q.z - c.z).abs() <= 2) {
                switched_ts.insert(*c);
            }
        }
        Track { rails, columns, col_len, rests, allowed: None, switched_ts, solid }
    }

    /// The rails beside `c` in each horizontal direction (N, S, W, E), level, one up or one down
    /// (a rail's potential connections, `RailState.hasNeighborRail`).
    fn arm_rails(rails: &HashMap<BlockPos, String>, c: BlockPos) -> [Vec<BlockPos>; 4] {
        let mut out: [Vec<BlockPos>; 4] = Default::default();
        for (i, (dx, dz)) in [(0, -1), (0, 1), (-1, 0), (1, 0)].into_iter().enumerate() {
            for dy in [0, 1, -1] {
                let m = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                if rails.contains_key(&m) {
                    out[i].push(m);
                }
            }
        }
        out
    }

    /// A rail cell's moves to other rail cells: its shape's two exits (level, or one down onto a
    /// slope's foot), or for a switched T every arm (the union of its shapes).
    fn rail_moves(&self, c: BlockPos) -> Vec<BlockPos> {
        if self.switched_ts.contains(&c) {
            return Track::arm_rails(&self.rails, c).into_iter().flatten().collect();
        }
        let mut out = Vec::new();
        for (dx, dy, dz) in rail_exits(self.rails.get(&c).map(|s| s.as_str()).unwrap_or("")) {
            for m in [BlockPos::new(c.x + dx, c.y + dy, c.z + dz), BlockPos::new(c.x + dx, c.y + dy - 1, c.z + dz)] {
                if self.rails.contains_key(&m) {
                    out.push(m);
                }
            }
        }
        out
    }

    /// The cells a rail's exits lead into (each exit's cell and the cell below it; a switched T's
    /// every side), and the rail's own cell and the cell below it (a cart dropping out of it, a
    /// trapdoor opening under it): a column whose first cell is one of them is entered from it.
    fn entry_cells(&self, c: BlockPos) -> Vec<BlockPos> {
        let mut out = vec![c, c.below()];
        let dirs: Vec<(i32, i32, i32)> = if self.switched_ts.contains(&c) { vec![(0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)] } else { rail_exits(self.rails.get(&c).map(|s| s.as_str()).unwrap_or("")) };
        for (dx, dy, dz) in dirs {
            let m = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
            out.push(m);
            out.push(m.below());
        }
        out
    }

    /// The rails a column's last position leaves onto (G1's note 3 on a4cd9b8b0): a fall (y_to
    /// below y_from) onto the rail in its last cell or the one under it (the landing); a lift onto
    /// a rail in its last cell or level beside it (the exit the top's current feeds).
    fn exit_rails(&self, k: usize) -> Vec<BlockPos> {
        let c = self.columns[k];
        let cc = self.col_cell(k, self.col_len[k] - 1);
        let cand: Vec<BlockPos> = if c[3] < c[1] {
            vec![cc, cc.below()]
        } else {
            vec![cc, BlockPos::new(cc.x, cc.y, cc.z - 1), BlockPos::new(cc.x, cc.y, cc.z + 1), BlockPos::new(cc.x - 1, cc.y, cc.z), BlockPos::new(cc.x + 1, cc.y, cc.z)]
        };
        cand.into_iter().filter(|r| self.rails.contains_key(r)).collect()
    }

    /// (ironspec Nov 12 16:02: the direction-aware walk, the default; `--two-way` keeps the old
    /// walk) Keep each link of the track to the way carts travel it: a link is oriented by a
    /// column (its own steps, entered at its start, left at its end), by its rails' shapes when
    /// only one of the two points to the other (a merge through a curve's free side runs into the
    /// curve only), or by a declared `travel` path (A>B>C: each step one way, a run's evidence its
    /// reason); an oriented link orients its 2-arm chain (a cart leaves a cell by its other arm),
    /// and a junction whose arms are all known but one, every known one into it (out of it),
    /// has that one out of it (into it). A `reversal` cell (a run shows a cart turning back
    /// there) keeps its links both ways, carried along the chain the same way; a link nothing
    /// orients stays both ways (printed), and so does one oriented both ways (a conflict, printed).
    /// With `list`, every oriented link with its source (G1's note 2 on a4cd9b8b0): a column, a
    /// shape, a declaration, a chain, a junction completion (INFERRED: at an undeclared reversal
    /// it drops a real move, ironspec 17:34), a reversal.
    fn orient(&mut self, travel: &[(Vec<BlockPos>, String)], reversal: &[(BlockPos, String)], list: bool) -> Vec<String> {
        use std::collections::{BTreeSet, HashSet};
        self.allowed = None;
        let mut nodes: Vec<TailNode> = self.rails.keys().map(|c| TailNode::Rail(c.x, c.y, c.z)).collect();
        for k in 0..self.columns.len() {
            for i in 0..self.col_len[k] {
                nodes.push(TailNode::Col(k, i));
            }
        }
        let mut e: BTreeSet<(TailNode, TailNode)> = BTreeSet::new();
        for n in &nodes {
            for (m, _) in self.edges(*n) {
                e.insert((*n, m));
            }
        }
        let mut adj: std::collections::BTreeMap<TailNode, BTreeSet<TailNode>> = std::collections::BTreeMap::new();
        for (a, b) in &e {
            adj.entry(*a).or_default().insert(*b);
            adj.entry(*b).or_default().insert(*a);
        }
        let rail = |c: &BlockPos| TailNode::Rail(c.x, c.y, c.z);
        // (a switched T's stored shape tells nothing of the way carts take it: a link one way by
        // shapes there orients nothing; its moves are already every arm's)
        let switched: BTreeSet<TailNode> = self.switched_ts.iter().map(rail).collect();
        let mut f: BTreeSet<(TailNode, TailNode)> = BTreeSet::new();
        let mut src: std::collections::BTreeMap<(TailNode, TailNode), &'static str> = std::collections::BTreeMap::new();
        let mut queue: Vec<(TailNode, TailNode)> = Vec::new();
        let (mut by_col, mut by_shape, mut by_decl) = (0usize, 0usize, 0usize);
        let mut warn: Vec<String> = Vec::new();
        for (a, b) in &e {
            if matches!(a, TailNode::Col(..)) || matches!(b, TailNode::Col(..)) {
                if f.insert((*a, *b)) {
                    src.insert((*a, *b), "column");
                    queue.push((*a, *b));
                    by_col += 1;
                }
            } else if !e.contains(&(*b, *a)) && !switched.contains(a) && !switched.contains(b) && f.insert((*a, *b)) {
                src.insert((*a, *b), "shape");
                queue.push((*a, *b));
                by_shape += 1;
            }
        }
        for (path, _) in travel {
            for w in path.windows(2) {
                let (a, b) = (rail(&w[0]), rail(&w[1]));
                if e.contains(&(a, b)) {
                    if f.insert((a, b)) {
                        queue.push((a, b));
                    }
                    src.insert((a, b), "declared");
                    by_decl += 1;
                } else {
                    warn.push(format!("--travel {},{},{}>{},{},{}: no move of the track joins them that way", w[0].x, w[0].y, w[0].z, w[1].x, w[1].y, w[1].z));
                }
            }
        }
        // (a cart leaves a 2-arm cell by its other arm; a junction with every known arm one way
        // and one unknown has that one the other way)
        fn propagate(e: &BTreeSet<(TailNode, TailNode)>, adj: &std::collections::BTreeMap<TailNode, BTreeSet<TailNode>>, f: &mut BTreeSet<(TailNode, TailNode)>, src: &mut std::collections::BTreeMap<(TailNode, TailNode), &'static str>, mut queue: Vec<(TailNode, TailNode)>, chain_label: &'static str) {
            let is_chain = |n: &TailNode| matches!(n, TailNode::Rail(..)) && adj.get(n).is_some_and(|s| s.len() == 2);
            while let Some((a, b)) = queue.pop() {
                let mut next: Vec<(TailNode, TailNode, &'static str)> = Vec::new();
                if is_chain(&b) {
                    let other = *adj[&b].iter().find(|x| **x != a).expect("two arms");
                    next.push((b, other, chain_label));
                }
                if is_chain(&a) {
                    let other = *adj[&a].iter().find(|x| **x != b).expect("two arms");
                    next.push((other, a, chain_label));
                }
                for j in [a, b] {
                    let Some(arms) = adj.get(&j) else { continue };
                    if !matches!(j, TailNode::Rail(..)) || arms.len() < 3 {
                        continue;
                    }
                    let ins = arms.iter().filter(|x| f.contains(&(**x, j))).count();
                    let outs = arms.iter().filter(|x| f.contains(&(j, **x))).count();
                    let unknown: Vec<TailNode> = arms.iter().filter(|x| !f.contains(&(**x, j)) && !f.contains(&(j, **x))).copied().collect();
                    if unknown.len() == 1 {
                        if outs == 0 && ins > 0 {
                            next.push((j, unknown[0], "junction (INFERRED)"));
                        } else if ins == 0 && outs > 0 {
                            next.push((unknown[0], j, "junction (INFERRED)"));
                        }
                    }
                }
                for (x, y, label) in next {
                    if e.contains(&(x, y)) && f.insert((x, y)) {
                        src.insert((x, y), label);
                        queue.push((x, y));
                    }
                }
            }
        }
        propagate(&e, &adj, &mut f, &mut src, queue, "chain");
        // (the reversals on top: a link both ways from them is a reversal's, not a conflict)
        let f1 = f.clone();
        let rev_cells: BTreeSet<TailNode> = reversal.iter().map(|(c, _)| rail(c)).collect();
        let mut rq: Vec<(TailNode, TailNode)> = Vec::new();
        for c in &rev_cells {
            for m in adj.get(c).cloned().unwrap_or_default() {
                for (a, b) in [(*c, m), (m, *c)] {
                    if e.contains(&(a, b)) && f.insert((a, b)) {
                        src.insert((a, b), "reversal");
                        rq.push((a, b));
                    }
                }
            }
        }
        propagate(&e, &adj, &mut f, &mut src, rq, "reversal chain");
        let name = |n: &TailNode| match n {
            TailNode::Rail(x, y, z) => format!("{x},{y},{z}"),
            TailNode::Col(k, i) => {
                let p = self.col_pos(*k, *i);
                format!("column {:.2},{:.2},{:.2}", p.x, p.y, p.z)
            }
        };
        let mut allowed: HashSet<(TailNode, TailNode)> = HashSet::new();
        let (mut one_way, mut undecided, mut both, mut conflict): (usize, BTreeSet<String>, usize, BTreeSet<String>) = (0, BTreeSet::new(), 0, BTreeSet::new());
        for (a, b) in &e {
            let (fwd, back) = (f.contains(&(*a, *b)), f.contains(&(*b, *a)));
            if fwd || !back {
                allowed.insert((*a, *b));
            }
            let key = if a < b { format!("{}-{}", name(a), name(b)) } else { format!("{}-{}", name(b), name(a)) };
            if fwd && back {
                if f1.contains(&(*a, *b)) && f1.contains(&(*b, *a)) {
                    conflict.insert(key);
                } else {
                    both += 1;
                }
            } else if !fwd && !back {
                undecided.insert(key);
            } else if fwd {
                one_way += 1;
            }
        }
        let mut out = vec![format!(
            "travel (the direction-aware walk, ironspec Nov 12 16:02; --two-way for the old walk): {one_way} moves one way ({by_col} column moves, {by_shape} links one way by their rails' shapes away from switched Ts ({} Ts with a signal source within Manhattan 2: every arm walked, their shapes orient nothing), {by_decl} declared --travel steps, the rest carried along chains and through junctions), {both} reversal moves both ways (--reversal), {} links both ways where nothing orients them, {} links oriented both ways (a conflict: kept both ways)",
            switched.len(),
            undecided.len(),
            conflict.len()
        )];
        if !undecided.is_empty() {
            out.push(format!("  both ways, nothing orients them (declare --travel A>B with a run's evidence): {}", undecided.iter().cloned().collect::<Vec<_>>().join("; ")));
        }
        if !conflict.is_empty() {
            out.push(format!("  oriented both ways (a conflict: a --reversal if a run shows one, else a wrong --travel): {}", conflict.iter().cloned().collect::<Vec<_>>().join("; ")));
        }
        out.extend(warn.into_iter().map(|w| format!("  {w}")));
        // (G1's note 1: each declaration with its reason, the evidence it rests on)
        for (path, why) in travel {
            out.push(format!("  declared --travel {}: {}", path.iter().map(|c| format!("{},{},{}", c.x, c.y, c.z)).collect::<Vec<_>>().join(">"), if why.is_empty() { "(no reason given)" } else { why.as_str() }));
        }
        for (c, why) in reversal {
            out.push(format!("  declared --reversal {},{},{}: {}", c.x, c.y, c.z, if why.is_empty() { "(no reason given)" } else { why.as_str() }));
        }
        let inferred = src.values().filter(|s| s.starts_with("junction")).count();
        out.push(format!(
            "  oriented links by source: {}; {inferred} by junction completion (INFERRED: at an undeclared cart reversal it drops a real move; check each against the runs){}",
            ["column", "shape", "declared", "chain", "junction (INFERRED)", "reversal", "reversal chain"].iter().map(|k| format!("{} {k}", src.values().filter(|s| *s == k).count())).collect::<Vec<_>>().join(", "),
            if list { "" } else { " (--orient-list prints every one)" }
        ));
        if list {
            let mut all: Vec<String> = src
                .iter()
                .filter(|((a, b), _)| !matches!(a, TailNode::Col(..)) || !matches!(b, TailNode::Col(..)))
                .map(|((a, b), s)| format!("  ORIENTED {} > {} ({s})", name(a), name(b)))
                .collect();
            all.sort();
            out.extend(all);
        }
        self.allowed = Some(allowed);
        out
    }

    fn col_pos(&self, k: usize, i: usize) -> Vec3 {
        let c = self.columns[k];
        Vec3::new(c[0], c[1] + if c[3] >= c[1] { 1.0 } else { -1.0 } * i as f64 * 0.25, c[2])
    }

    fn col_cell(&self, k: usize, i: usize) -> BlockPos {
        let p = self.col_pos(k, i);
        BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
    }

    /// A rail cell's crossing time (G1: 2.5 gt straight or sloped, 1.25 gt a curve).
    fn cell_gt(&self, c: BlockPos) -> f64 {
        let s = self.rails.get(&c).map(|s| s.as_str()).unwrap_or("");
        let curve = ["north_east", "north_west", "south_east", "south_west"].iter().any(|k| s.contains(&format!("shape={k}")));
        if curve { 0.5 / TAIL_RAIL_SPEED } else { 1.0 / TAIL_RAIL_SPEED }
    }

    /// The node a rider position is at: a column position within 0.13 of it, else its rail cell.
    fn node_at(&self, p: Vec3) -> Option<TailNode> {
        for (k, c) in self.columns.iter().enumerate() {
            if (p.x - c[0]).abs() < 0.01 && (p.z - c[2]).abs() < 0.01 {
                let i = ((p.y - c[1]).abs() / 0.25).round() as usize;
                let lo = c[1].min(c[3]) - 0.13;
                let hi = c[1].max(c[3]) + 0.13;
                if p.y >= lo && p.y <= hi && i < self.col_len[k] {
                    return Some(TailNode::Col(k, i));
                }
            }
        }
        let c = BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        self.rails.contains_key(&c).then_some(TailNode::Rail(c.x, c.y, c.z))
    }

    /// The shortest travel time (gt) from any seed to every node within `max`.
    fn walk(&self, seeds: &[TailNode], max: f64) -> HashMap<TailNode, f64> {
        self.walk_within(seeds, max, None)
    }

    /// `walk`, through the `allowed` nodes only (when given).
    fn walk_within(&self, seeds: &[TailNode], max: f64, allowed: Option<&std::collections::HashSet<TailNode>>) -> HashMap<TailNode, f64> {
        let mut best: HashMap<TailNode, f64> = HashMap::new();
        let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, TailNode)>> = std::collections::BinaryHeap::new();
        for n in seeds {
            if best.insert(*n, 0.0).is_none() {
                heap.push(std::cmp::Reverse((0, *n)));
            }
        }
        while let Some(std::cmp::Reverse((_, n))) = heap.pop() {
            let d = best[&n];
            // (a rest cell, where a cart waits longer than the walk, stops it: the holds)
            let next = match n {
                TailNode::Rail(x, y, z) if self.rests.contains(&BlockPos::new(x, y, z)) && d > 0.0 => Vec::new(),
                _ => self.edges(n),
            };
            for (m, w) in next {
                let nd = d + w;
                if nd <= max && allowed.is_none_or(|a| a.contains(&m)) && best.get(&m).is_none_or(|b| nd < *b) {
                    best.insert(m, nd);
                    heap.push(std::cmp::Reverse(((nd * 1000.0) as u64, m)));
                }
            }
        }
        best
    }

    /// The track's moves out of a node, with their times: a rail cell to the cells its shape
    /// points to (level, or one down onto a slope's foot; a switched T to every arm), and into a
    /// column it enters (`entry_cells`); a column position one step on, and at its end onto the
    /// rails it leaves onto (`exit_rails`, G1's note 3 on a4cd9b8b0).
    fn edges(&self, n: TailNode) -> Vec<(TailNode, f64)> {
        let mut next: Vec<(TailNode, f64)> = Vec::new();
        match n {
            TailNode::Rail(x, y, z) => {
                let c = BlockPos::new(x, y, z);
                let own = self.cell_gt(c);
                for m in self.rail_moves(c) {
                    next.push((TailNode::Rail(m.x, m.y, m.z), 0.5 * own + 0.5 * self.cell_gt(m)));
                }
                let entry = self.entry_cells(c);
                for k in 0..self.columns.len() {
                    if entry.contains(&self.col_cell(k, 0)) {
                        next.push((TailNode::Col(k, 0), 0.5 * own));
                    }
                }
            }
            TailNode::Col(k, i) => {
                if i + 1 < self.col_len[k] {
                    next.push((TailNode::Col(k, i + 1), 0.25 / TAIL_COLUMN_SPEED));
                } else {
                    for r in self.exit_rails(k) {
                        next.push((TailNode::Rail(r.x, r.y, r.z), 0.5 * self.cell_gt(r)));
                    }
                }
            }
        }
        if let Some(a) = &self.allowed {
            next.retain(|(m, _)| a.contains(&(n, *m)));
        }
        next
    }

    /// (G1's CHANGES 1 on d3dc5aa46, ironspec Nov 12 15:03: no silent ends) Where a walk cannot go
    /// on from node `n`: a rail exit with no rail at its cell or the cell below, no column entered
    /// through it (its first cell) or left onto this rail (its last), and an open cell (no
    /// collision shape: the cart leaves the track there, a fall or a roll-off nobody declared; a
    /// wall only stops it), that a cart can reach moving toward it (from the rail its other exit
    /// leads to, by a move the walks allow); or a column's last position with no rail to leave
    /// onto (`exit_rails`); each with the cells a declaration may name (the rail, the exit cell
    /// and the cell below it; the column's last cell).
    fn open_ends(&self, n: TailNode) -> Vec<(String, Vec<BlockPos>)> {
        let mut out = Vec::new();
        match n {
            TailNode::Rail(x, y, z) => {
                let c = BlockPos::new(x, y, z);
                // (a switched T's stored shape is one of the shapes a cart takes: its exits count
                // like any rail's, its other arms are rails)
                let exits = rail_exits(self.rails.get(&c).map(|s| s.as_str()).unwrap_or(""));
                for (i, (dx, dy, dz)) in exits.iter().enumerate() {
                    let m = BlockPos::new(x + dx, y + dy, z + dz);
                    let into_column = (0..self.columns.len()).any(|k| {
                        let f = self.col_cell(k, 0);
                        f == m || f == m.below() || (self.col_cell(k, self.col_len[k] - 1) == m && self.exit_rails(k).contains(&c))
                    });
                    if self.rails.contains_key(&m) || self.rails.contains_key(&m.below()) || into_column || self.solid.contains(&m) {
                        continue;
                    }
                    let arrives = {
                        // (a cart reaches the dead end moving toward it: from the rail its other
                        // exit leads to, or from any rail whose move into this one runs along the
                        // exit's direction (a free-side entry into a curve, the S-bend's merge C),
                        // by a move the walks allow)
                        let allowed_in = |r: BlockPos| {
                            self.rails.contains_key(&r) && {
                                let from = TailNode::Rail(r.x, r.y, r.z);
                                match &self.allowed {
                                    Some(a) => a.contains(&(from, n)),
                                    None => self.rail_moves(r).contains(&c),
                                }
                            }
                        };
                        let through = exits.iter().enumerate().filter(|(j, _)| *j != i).any(|(_, (ex, ey, ez))| {
                            let o = BlockPos::new(x + ex, y + ey, z + ez);
                            [o, o.below()].into_iter().any(|r| allowed_in(r))
                        });
                        let along = [(0, -1), (0, 1), (-1, 0), (1, 0)].into_iter().filter(|(ddx, ddz)| -ddx * dx - ddz * dz > 0).any(|(ddx, ddz)| {
                            [0, 1, -1].into_iter().any(|ddy| allowed_in(BlockPos::new(x + ddx, y + ddy, z + ddz)))
                        });
                        // (G1's CHANGES 1 on fa1267b75: a cart arriving from a column: a fall's
                        // landing rail rolls off either way, so every open exit counts, a slope's
                        // high exit aside (a cart dropped onto a slope slides down it: its fall
                        // carries no speed up the slope); a lift's exit rail (level beside its last
                        // cell) is left by an exit away from the column, by a move the walks allow)
                        let from_col = (0..self.columns.len()).any(|k| {
                            let last = TailNode::Col(k, self.col_len[k] - 1);
                            let moves_in = match &self.allowed {
                                Some(a) => a.contains(&(last, n)),
                                None => self.exit_rails(k).contains(&c),
                            };
                            let col = self.columns[k];
                            let cc = self.col_cell(k, self.col_len[k] - 1);
                            moves_in && if col[3] < col[1] { *dy == 0 } else { (m.x - c.x, m.z - c.z) != (cc.x - c.x, cc.z - c.z) }
                        });
                        through || along || from_col
                    };
                    if !arrives {
                        continue;
                    }
                    out.push((format!("the rail {x},{y},{z}'s exit to {},{},{}", m.x, m.y, m.z), vec![c, m, m.below()]));
                }
            }
            TailNode::Col(k, i) => {
                if i + 1 == self.col_len[k] && self.exit_rails(k).is_empty() {
                    let cc = self.col_cell(k, i);
                    let c = self.columns[k];
                    out.push((format!("the column {:.1},{:.1}..{:.1},{:.1}'s end in {},{},{}", c[0], c[1], c[3], c[2], cc.x, cc.y, cc.z), vec![cc]));
                }
            }
        }
        out
    }

    /// The nodes from which a cart reaches one of `seeds` within `max` gt, with that time: the walk
    /// over the moves reversed (a merge through a curve's open side counts: the cart rolling on
    /// a straight into the curve's side follows the curve), a rest cell never left.
    fn walk_rev(&self, seeds: &[TailNode], max: f64) -> HashMap<TailNode, f64> {
        let mut nodes: Vec<TailNode> = self.rails.keys().map(|c| TailNode::Rail(c.x, c.y, c.z)).collect();
        for k in 0..self.columns.len() {
            for i in 0..self.col_len[k] {
                nodes.push(TailNode::Col(k, i));
            }
        }
        let mut rev: HashMap<TailNode, Vec<(TailNode, f64)>> = HashMap::new();
        for n in &nodes {
            if let TailNode::Rail(x, y, z) = n
                && self.rests.contains(&BlockPos::new(*x, *y, *z))
            {
                continue;
            }
            for (m, w) in self.edges(*n) {
                rev.entry(m).or_default().push((*n, w));
            }
        }
        let mut best: HashMap<TailNode, f64> = HashMap::new();
        let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, TailNode)>> = std::collections::BinaryHeap::new();
        for n in seeds {
            if best.insert(*n, 0.0).is_none() {
                heap.push(std::cmp::Reverse((0, *n)));
            }
        }
        while let Some(std::cmp::Reverse((_, n))) = heap.pop() {
            let d = best[&n];
            for (m, w) in rev.get(&n).cloned().unwrap_or_default() {
                let nd = d + w;
                if nd <= max && best.get(&m).is_none_or(|b| nd < *b) {
                    best.insert(m, nd);
                    heap.push(std::cmp::Reverse(((nd * 1000.0) as u64, m)));
                }
            }
        }
        best
    }
}

/// (ironspec Nov 12 08:24, the tools gap: the statics' position set is every position where any
/// scarer makes a rider panic, not only the relied-on points) Every rider position on the rails
/// (`rider_positions`) and in the free-fall or lift `columns` (x, y0, z, y1: every 0.25): where a
/// hostile within its range is in the rider's sight (PANIC: a zombie within 8, a pillager within
/// 15), and (G1 on 6757c71fe, onechunk Nov 12 09:21) its tail: NEAREST_HOSTILE stays until a
/// hostiles scan (every 20 gt) runs on a living scan (every 20 gt) taken after the hostile left
/// sight, up to ~38 gt, and VillagerPanicTrigger keeps running meanwhile, so every position within
/// `tail_gt` of travel along the track from a seen one (`Track`: a ridden cart at most 0.4 blocks a
/// tick on each axis, a straight cell 2.5 gt and a curve 1.25, either way along a rail; in a column
/// x, y_from, z, y_to only from y_from toward y_to, 1 block a tick, entered at its start from the
/// rails around it and left at its end; a `rests` cell, where a cart waits longer than the tail,
/// stops it: the holds) panics too, the flee counted there too (a stale NEAREST_HOSTILE) and a tail
/// position within 6.05 of a zombie a staleness failure; at each,
/// the panic stroll's candidates (VillageBoundRandomStroll
/// 2/2) and each one's reachability from that position by the villager's own pathfinder, a partial
/// path counting as not reached; FAIL at every panicking position with an unreached candidate,
/// printed by class (each unreached candidate cell with the positions that draw it, how many of
/// them in a tail); with `list` every panicking position too. Returns the lines and the verdict's
/// counts (ironspec Nov 12 16:23 / 16:52, G1: the caller fails on these numbers, never on a
/// printed line's text). The walks follow the way carts travel each
/// link (`Track::orient`, by the columns, one-way shapes and `travel`, `reversal` both ways),
/// unless `two_way` (ironspec Nov 12 16:02).
pub fn clear_all(spec: &FarmSpec, home: Option<BlockPos>, panic_boxes: &[([i32; 3], [i32; 3])], claims: &[BlockPos], pills: &[Vec3], columns: &[[f64; 4]], tail_gt: f64, rests: &[BlockPos], list: bool, idle: bool, watch: &[([i32; 3], [i32; 3])], approach: &[(BlockPos, f64)], late_gt: f64, ends_declared: &[(BlockPos, String)], travel: &[(Vec<BlockPos>, String)], reversal: &[(BlockPos, String)], two_way: bool, orient_list: bool) -> Result<(Vec<String>, ClearVerdict), String> {
    let mut pos = rider_positions(spec);
    let mut col_of: Vec<Option<(usize, usize)>> = vec![None; pos.len()];
    let mut track = Track::new(spec, columns, rests);
    // (ironspec Nov 12 16:02: the walks follow the way carts travel each link, by default)
    let travel_lines = if two_way { vec!["travel: --two-way: every rail link walked either way (the old walk)".to_string()] } else { track.orient(travel, reversal, orient_list) };
    let track = track;
    for (k, _) in columns.iter().enumerate() {
        // (a column x, y_from, z, y_to: riders travel from y_from to y_to, a drop downward, a lift
        // upward)
        for i in 0..track.col_len[k] {
            let p = track.col_pos(k, i);
            pos.push((BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32), -1.0, p));
            col_of.push(Some((k, i)));
        }
    }
    let at: Vec<Vec3> = pos.iter().map(|(_, _, p)| *p).collect();
    let ps = check_inner(spec, home, &[], panic_boxes, true, claims, pills, None, &at)?;
    let node_of = |idx: usize| -> TailNode {
        match col_of[idx] {
            Some((k, i)) => TailNode::Col(k, i),
            None => TailNode::Rail(pos[idx].0.x, pos[idx].0.y, pos[idx].0.z),
        }
    };
    // the tail: a shortest-time walk (game ticks) from every seen position along the track
    let seeds: Vec<TailNode> = if tail_gt > 0.0 { ps.iter().enumerate().filter(|(_, pl)| pl.seen > 0).map(|(idx, _)| node_of(idx)).collect() } else { Vec::new() };
    let best = track.walk(&seeds, tail_gt);
    // (G1's CHANGES 1 on d3dc5aa46: every track end a walk reaches with time left, by walk)
    let mut ends: std::collections::BTreeMap<String, (Vec<BlockPos>, [f64; 3])> = std::collections::BTreeMap::new();
    record_ends(&track, &best, tail_gt, 0, &mut ends);
    // (ironspec Nov 12 12:36, tick-exact's ff1ca056a: from inside a bubble column the panic stroll's
    // path starts at the water's surface, one node, partial: a panicking position inside a water
    // column with any PANIC candidate fails the gate, whatever a path from its own cell reads)
    let block_at: HashMap<BlockPos, String> = spec.blocks.iter().map(|(p, b)| (*p, b.to_string())).collect();
    let in_water = |p: &Vec3| block_at.get(&BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)).is_some_and(|s| s.contains("bubble_column") || s.contains("minecraft:water"));
    let (mut panicking, mut seen_n, mut fails, mut stamps) = (0usize, 0usize, 0usize, 0usize);
    let mut column_fails = 0usize;
    let mut fails_by = [0usize; 4];
    let mut listed: Vec<String> = Vec::new();
    let mut stale: Vec<String> = Vec::new();
    let mut unreached_at: std::collections::BTreeMap<(i32, i32, i32), Vec<(usize, usize)>> = std::collections::BTreeMap::new();
    let mut watched: std::collections::BTreeMap<(i32, i32, i32), Vec<String>> = std::collections::BTreeMap::new();
    // (ironspec Nov 12 10:24: every walk-target source the brain can run at each position and
    // activity: panicking, PANIC's stroll and flee; calm, IDLE's stroll and walks to a seen rider
    // within 8; each unreached candidate: the positions drawing it, by rider cell: (t's, the largest
    // share, how many in a tail))
    const SRC: [&str; 4] = [
        "PANIC's stroll (VillageBoundRandomStroll 2/2)",
        "PANIC's flee (SetWalkTargetAwayFrom: a seen hostile within 6)",
        "IDLE's stroll (VillageBoundRandomStroll 10/7)",
        "IDLE's walk to a rider it sees within 8 (InteractWith, SetWalkTargetFromLookTarget)",
    ];
    let mut classes: std::collections::BTreeMap<(usize, i32, i32, i32), std::collections::BTreeMap<(i32, i32, i32), (Vec<f64>, f64, usize)>> = std::collections::BTreeMap::new();
    for (idx, ((c, t, p), pl)) in pos.iter().zip(ps.iter()).enumerate() {
        let tail = pl.seen == 0 && best.contains_key(&node_of(idx));
        let panics = pl.seen > 0 || tail;
        if panics {
            panicking += 1;
            if pl.seen > 0 {
                seen_n += 1;
            }
            if list {
                listed.push(format!(
                    "  PANIC {},{},{} {} rider {:.3},{:.3},{:.3}: {}",
                    c.x,
                    c.y,
                    c.z,
                    if *t < 0.0 { "column".to_string() } else { format!("t {t:.2}") },
                    p.x,
                    p.y,
                    p.z,
                    if tail { format!("tail ({:.1} gt from a seen position)", best[&node_of(idx)]) } else { "seen".to_string() }
                ));
            }
        }
        let srcs: Vec<(usize, &Source)> = if panics {
            let mut v = vec![(0, &pl.panic_stroll)];
            // (G1's CHANGES 2: the flee fires on a stale NEAREST_HOSTILE too, SetWalkTargetAwayFrom
            // checking only its presence and distance < 6, so it counts at tail positions)
            if pl.flee_from > 0 {
                v.push((1, &pl.flee));
            }
            v
        } else if idle {
            vec![(2, &pl.stroll), (3, &pl.others)]
        } else {
            Vec::new()
        };
        // (G1's CHANGES 2: a tail position within 6.05 of a zombie is itself the brief's staleness
        // failure: the remembered hostile within the flee bar after it left sight)
        if tail && pl.near.is_some_and(|(lo, _, _)| lo < FLEE_BAR) {
            stale.push(format!(
                "{},{},{} {} ({:.1} gt from a seen position, a zombie at {:.2})",
                c.x,
                c.y,
                c.z,
                if *t < 0.0 { "column".to_string() } else { format!("t {t:.2}") },
                best[&node_of(idx)],
                pl.near.map(|(lo, _, _)| lo).unwrap_or(0.0)
            ));
        }
        // (the stale-path hazard's inputs: every unreached candidate of a PANIC source at a panicking
        // position; IDLE's partial paths are cleared on the way into a panic: VillagerPanicTrigger.start
        // erases PATH and WALK_TARGET when PANIC was not active, which stops MoveToTargetSink)
        for (k, s) in &srcs {
            if *k >= 2 {
                continue;
            }
            for (u, sh) in s.unreached.iter().filter(|(_, sh)| *sh > 1e-9) {
                let _ = sh;
                unreached_at.entry((u.x, u.y, u.z)).or_default().push((idx, *k));
            }
        }
        // (ironspec Nov 12 11:15: every candidate, reached or not, inside a watched box: a feed or a
        // switched cell no candidate may land in, nor be lifted into)
        for (k, s) in &srcs {
            for (u, _) in s.reached.iter().chain(s.unreached.iter()) {
                if watch.iter().any(|(a, b)| a[0] <= u.x && u.x <= b[0] && a[1] <= u.y && u.y <= b[1] && a[2] <= u.z && u.z <= b[2]) {
                    watched.entry((u.x, u.y, u.z)).or_default().push(format!("{},{},{} {} [{}]", c.x, c.y, c.z, if *t < 0.0 { "column".to_string() } else { format!("t {t:.2}") }, SRC[*k].split(" (").next().unwrap_or("")));
                }
            }
        }
        let mut failed = false;
        let mut stamped = false;
        let water = panics && in_water(p);
        for (k, s) in srcs {
            // (a panicking position in a water column: every PANIC candidate fails, reached or not)
            if water && k < 2 && s.cells > 0 {
                failed = true;
                fails_by[k] += 1;
                column_fails += 1;
                for (u, sh) in s.reached.iter().chain(s.unreached.iter()).filter(|(_, sh)| *sh > 1e-9) {
                    let e = classes.entry((k, u.x, u.y, u.z)).or_default().entry((c.x, c.y, c.z)).or_insert((Vec::new(), 0.0, 0));
                    e.0.push(*t);
                    e.1 = e.1.max(*sh);
                    e.2 += usize::from(tail);
                }
                continue;
            }
            if s.live() == 0 {
                continue;
            }
            // (ironspec Nov 12 10:49: the panic-position static counts PANIC-activity draws only;
            // IDLE's at the calm positions fall under the stamp bar, printed apart)
            if k < 2 {
                failed = true;
            } else {
                stamped = true;
            }
            fails_by[k] += 1;
            for (u, sh) in s.unreached.iter().filter(|(_, sh)| *sh > 1e-9) {
                let e = classes.entry((k, u.x, u.y, u.z)).or_default().entry((c.x, c.y, c.z)).or_insert((Vec::new(), 0.0, 0));
                e.0.push(*t);
                e.1 = e.1.max(*sh);
                e.2 += usize::from(tail);
            }
        }
        if failed {
            fails += 1;
        }
        if stamped {
            stamps += 1;
        }
    }
    // (9fc3155f Nov 12 11:24, onechunk's s2 t 5547 partial: MoveToTargetSink keeps its path while it
    // runs (at most 250 gt for a villager) when the new walk target lies within 2 blocks (distSqr <=
    // 4) of the last, and PathNavigation.createPath returns the current path for the same target, so
    // a PANIC draw reached from here can still follow a partial path set earlier in the same panic
    // (VillagerPanicTrigger.start erases PATH and WALK_TARGET on the way into a panic, so an IDLE
    // draw's path never carries over): a stale-path hazard at every panicking position whose reached
    // PANIC candidate lies within 2 blocks of a PANIC candidate left unreached at a position up to
    // 250 gt of travel before it, through panicking positions only)
    let panicking_nodes: std::collections::HashSet<TailNode> = ps.iter().enumerate().filter(|(idx, pl)| pl.seen > 0 || best.contains_key(&node_of(*idx))).map(|(idx, _)| node_of(idx)).collect();
    let mut hazards: std::collections::BTreeMap<(i32, i32, i32), Vec<String>> = std::collections::BTreeMap::new();
    for ((ux, uy, uz), from) in &unreached_at {
        let seeds: Vec<TailNode> = from.iter().map(|(idx, _)| node_of(*idx)).collect();
        let reach = track.walk_within(&seeds, MOVE_SINK_MAX, Some(&panicking_nodes));
        for (idx, pl) in ps.iter().enumerate() {
            let panics = pl.seen > 0 || best.contains_key(&node_of(idx));
            if !panics {
                continue;
            }
            let Some(dt) = reach.get(&node_of(idx)) else { continue };
            for (x, _) in pl.panic_stroll.reached.iter().chain(if pl.flee_from > 0 { pl.flee.reached.iter() } else { [].iter() }) {
                let d2 = (x.x - ux).pow(2) + (x.y - uy).pow(2) + (x.z - uz).pow(2);
                if d2 <= 4 {
                    let (c, t, _) = &pos[idx];
                    hazards.entry((*ux, *uy, *uz)).or_default().push(format!(
                        "{},{},{} at {},{},{} {} ({:.0} gt after an unreached draw there)",
                        x.x,
                        x.y,
                        x.z,
                        c.x,
                        c.y,
                        c.z,
                        if *t < 0.0 { "column".to_string() } else { format!("t {t:.2}") },
                        dt
                    ));
                }
            }
        }
    }
    let n_haz: usize = hazards.values().map(|v| v.len()).sum();
    // (ironspec Nov 12 12:13 / 12:29, tick-exact's `panic-lift` row: MoveToTargetSink creates a PANIC
    // draw's path only after its remaining cooldown (up to 39 gt, drawn at its last stop and frozen
    // while there is no walk target), so the path starts wherever the cart has rolled by then: each
    // PANIC candidate drawn at a panicking track node, its path computed from every node within
    // `late_gt` of travel after it (the rails either way, a conservative bound; a column only
    // onward) from that node's rider position (a rail cell's middle, a column's position): a late
    // FAIL where any of them leaves it unreached (inside a water column the path is the water's
    // surface, one node: redsim's passenger start rule, the official row's shape))
    let mut late: std::collections::BTreeMap<(i32, i32, i32), Vec<(String, String, f64)>> = std::collections::BTreeMap::new();
    let mut late_positions = 0usize;
    if late_gt > 0.0 {
        let node_pos = |n: &TailNode| -> Option<Vec3> {
            match n {
                TailNode::Rail(x, y, z) => {
                    let c = BlockPos::new(*x, *y, *z);
                    track.rails.get(&c).map(|s| along_rail(c, s, 0.5))
                }
                TailNode::Col(k, i) => Some(track.col_pos(*k, *i)),
            }
        };
        // (each panicking node's PANIC candidates reached from it, over its positions)
        let mut by_node: std::collections::BTreeMap<TailNode, std::collections::BTreeSet<(i32, i32, i32)>> = std::collections::BTreeMap::new();
        for (idx, pl) in ps.iter().enumerate() {
            let nd = node_of(idx);
            if !(pl.seen > 0 || best.contains_key(&nd)) {
                continue;
            }
            let set = by_node.entry(nd).or_default();
            for (u, _) in pl.panic_stroll.reached.iter().chain(if pl.flee_from > 0 { pl.flee.reached.iter() } else { [].iter() }) {
                set.insert((u.x, u.y, u.z));
            }
        }
        let mut pairs: Vec<(Vec3, BlockPos)> = Vec::new();
        let mut pair_ix: HashMap<(TailNode, (i32, i32, i32)), usize> = HashMap::new();
        let mut checks: Vec<(TailNode, (i32, i32, i32), TailNode, f64)> = Vec::new();
        for (nd, xs) in &by_node {
            if xs.is_empty() {
                continue;
            }
            let after = track.walk(&[*nd], late_gt);
            record_ends(&track, &after, late_gt, 1, &mut ends);
            for (q, dt) in &after {
                if q == nd {
                    continue;
                }
                let Some(qp) = node_pos(q) else { continue };
                for x in xs {
                    let key = (*q, *x);
                    if !pair_ix.contains_key(&key) {
                        pair_ix.insert(key, pairs.len());
                        pairs.push((qp, BlockPos::new(x.0, x.1, x.2)));
                    }
                    checks.push((*nd, *x, *q, *dt));
                }
            }
        }
        let ok = reach_pairs(spec, home, claims, &pairs)?;
        let mut failed_nodes: std::collections::BTreeSet<TailNode> = std::collections::BTreeSet::new();
        for (nd, x, q, dt) in checks {
            if ok[pair_ix[&(q, x)]] {
                continue;
            }
            failed_nodes.insert(nd);
            let qs = match q {
                TailNode::Rail(a, b, c) => format!("{a},{b},{c}"),
                TailNode::Col(k, i) => {
                    let p = track.col_pos(k, i);
                    format!("column {:.2},{:.2},{:.2}", p.x, p.y, p.z)
                }
            };
            let ns = match nd {
                TailNode::Rail(a, b, c) => format!("{a},{b},{c}"),
                TailNode::Col(k, i) => {
                    let p = track.col_pos(k, i);
                    format!("column {:.2},{:.2},{:.2}", p.x, p.y, p.z)
                }
            };
            late.entry(x).or_default().push((ns, qs, dt));
        }
        late_positions = failed_nodes.len();
    }
    let panic_classes = classes.keys().filter(|(k, _, _, _)| *k < 2).count();
    let mut lines = vec![format!(
        "every rider position a scarer makes panic (each rail cell every 0.1 along it, a slope's 0.97 too{}): {panicking} of {} positions panic ({seen_n} with a hostile in sight, {} more in a seen position's tail: within {tail_gt} gt of travel along the track, a ridden cart at most {TAIL_RAIL_SPEED} b/gt on the rails and {TAIL_COLUMN_SPEED} in a column); none-or-all at each (the gate, an inherited policy, ironspec Nov 12 05:56; PANIC's draws: the 2/2 stroll and the flee): {}{}",
        if columns.is_empty() { String::new() } else { format!(", {} columns every 0.25", columns.len()) },
        pos.len(),
        panicking - seen_n,
        if fails == 0 {
            "yes (every candidate reached by a complete path)".to_string()
        } else {
            format!(
                "FAIL at {fails} ({}{}), {panic_classes} classes (an unreached candidate each)",
                (0..2).filter(|k| fails_by[*k] > 0).map(|k| format!("{} {}", fails_by[k], SRC[k].split(" (").next().unwrap_or(""))).collect::<Vec<_>>().join(", "),
                if column_fails > 0 { format!("; {column_fails} of them in a water column, where any PANIC candidate fails: the path starts at the water's surface, one node") } else { String::new() }
            )
        },
        if idle {
            format!(
                "; the stamp bar's input, apart (IDLE's sources at the {} calm positions, not this gate): {stamps} positions with an unreached candidate ({}), {} classes",
                pos.len() - panicking,
                (2..4).map(|k| format!("{} {}", fails_by[k], SRC[k].split(" (").next().unwrap_or(""))).collect::<Vec<_>>().join(", "),
                classes.len() - panic_classes
            )
        } else {
            String::new()
        }
    )];
    let mut cls: Vec<_> = classes.into_iter().collect();
    cls.sort_by_key(|((k, _, _, _), from)| (*k, std::cmp::Reverse(from.values().map(|(ts, _, _)| ts.len()).sum::<usize>())));
    for ((k, x, y, z), from) in cls {
        let n: usize = from.values().map(|(ts, _, _)| ts.len()).sum();
        let lst: Vec<String> = from
            .iter()
            .map(|((a, b, c), (ts, s, kk))| {
                let tl = if ts.iter().all(|t| *t < 0.0) { "column".to_string() } else { format!("t {:.2}..{:.2}", ts.iter().cloned().fold(f64::INFINITY, f64::min), ts.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) };
                format!("{a},{b},{c} ({} positions{}, {tl}, up to {:.1}%)", ts.len(), if *kk > 0 { format!(", {kk} in a tail") } else { String::new() }, 100.0 * s)
            })
            .collect();
        lines.push(format!("  {} class [{}] {x},{y},{z} unreached, drawn from {n} positions: {}", if k < 2 { "FAIL" } else { "STAMP" }, SRC[k], lst.join("; ")));
    }
    lines.push(format!(
        "late paths (tick-exact's panic-lift row: a PANIC draw pathed up to {late_gt} gt later, after MoveToTargetSink's cooldown, from where the cart has rolled; a reached candidate that some later node leaves unreached): {}",
        if late_gt <= 0.0 { "not checked".to_string() } else if late.is_empty() { "0".to_string() } else { format!("FAIL at {late_positions} panicking nodes, {} candidate cells", late.len()) }
    ));
    let mut lt: Vec<_> = late.into_iter().collect();
    lt.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for ((x, y, z), v) in lt {
        // (by draw node: its pairs and the earliest later node that leaves the candidate unreached)
        let mut by_draw: std::collections::BTreeMap<String, (usize, f64, String)> = std::collections::BTreeMap::new();
        for (ns, qs, dt) in &v {
            let e = by_draw.entry(ns.clone()).or_insert((0, f64::INFINITY, String::new()));
            e.0 += 1;
            if *dt < e.1 {
                e.1 = *dt;
                e.2 = qs.clone();
            }
        }
        lines.push(format!(
            "  LATE {x},{y},{z}: {} (draw, later node) pairs unreached; by draw node: {}",
            v.len(),
            by_draw.iter().map(|(ns, (n, dt, qs))| format!("drawn at {ns}: {n} pairs, the first pathed {dt:.1} gt later from {qs}")).collect::<Vec<_>>().join("; ")
        ));
    }
    lines.push(format!(
        "staleness (an inherited policy, G1's CHANGES 2: a tail position within {FLEE_BAR} of a zombie, NEAREST_HOSTILE remembered within the flee bar): {}",
        if stale.is_empty() { "0".to_string() } else { format!("FAIL at {}: {}", stale.len(), stale.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    lines.push(format!(
        "stale-path hazards (printed, not enforced; in one panic: a reached PANIC candidate within 2 blocks of a PANIC candidate left unreached up to {MOVE_SINK_MAX} gt of travel before, through panicking positions: MoveToTargetSink keeps that partial path; the 2 blocks (its distSqr > 4 re-path test) and the {MOVE_SINK_MAX} gt run are physical): {}",
        if n_haz == 0 { "0".to_string() } else { format!("{n_haz} at {} upstream unreached cells", hazards.len()) }
    ));
    let mut hz: Vec<_> = hazards.into_iter().collect();
    hz.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for ((x, y, z), v) in hz {
        lines.push(format!("  HAZARD upstream unreached {x},{y},{z} -> {} panic draws within 2 of it: {}", v.len(), v.iter().take(6).cloned().collect::<Vec<_>>().join("; ")));
    }
    if !watch.is_empty() {
        lines.push(format!(
            "candidates inside the watched boxes ({}), reached or not, every source: {}",
            watch.iter().map(|(a, b)| format!("{},{},{}..{},{},{}", a[0], a[1], a[2], b[0], b[1], b[2])).collect::<Vec<_>>().join(" "),
            if watched.is_empty() { "0".to_string() } else { format!("{} cells", watched.len()) }
        ));
        for ((x, y, z), from) in &watched {
            lines.push(format!("  WATCHED {x},{y},{z} drawn from {} positions: {}", from.len(), from.iter().take(8).cloned().collect::<Vec<_>>().join("; ")));
        }
    }
    // (ironspec Nov 12 12:13, onechunk's late paths: MoveToTargetSink's cooldown (up to 39 gt after
    // its last path stops) delays a PANIC draw's path until the cart has moved on, e.g. into the
    // shared lift, where it is computed from the water's surface: every panicking position within
    // `gt` of travel of an approach cell (the lift's entrance), with how it panics (the hostiles it
    // sees, or its tail), its PANIC candidates with the block under each, and its P_null: the
    // input to pricing P_null 1 there (no candidates) or keeping the stretch out of panic)
    if !approach.is_empty() {
        let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
        let mut run = Run::start(spec, &o)?;
        while run.settle_tick() {}
        let w = &run.world;
        let zs = hostiles(spec);
        let seat = zombie_seat();
        for (cell, gt) in approach {
            // (the positions BEFORE the entrance: the walk over the track's moves reversed)
            let reach = track.walk_rev(&[TailNode::Rail(cell.x, cell.y, cell.z)], *gt);
            record_ends(&track, &reach, *gt, 2, &mut ends);
            let mut rows: Vec<(f64, String)> = Vec::new();
            // (the pricing inputs: each candidate cell with the block under it, how many positions
            // draw it and the nearest's time to the entrance; each hostile with the positions it sees)
            let mut by_cand: std::collections::BTreeMap<(i32, i32, i32), (String, usize, f64)> = std::collections::BTreeMap::new();
            let mut by_foe: std::collections::BTreeMap<String, (usize, std::collections::BTreeSet<(i32, i32, i32)>)> = std::collections::BTreeMap::new();
            let mut tails = 0usize;
            for (idx, pl) in ps.iter().enumerate() {
                let Some(dt) = reach.get(&node_of(idx)) else { continue };
                let tail = pl.seen == 0 && best.contains_key(&node_of(idx));
                if pl.seen == 0 && !tail {
                    continue;
                }
                let (c, t, p) = &pos[idx];
                let eye = Vec3::new(p.x, p.y + 1.62, p.z);
                let mut by: Vec<String> = Vec::new();
                for z in &zs {
                    let f = Vec3::new(z.x, z.y + seat, z.z);
                    let d = ((f.x - p.x).powi(2) + (f.y - p.y).powi(2) + (f.z - p.z).powi(2)).sqrt();
                    if d <= 8.0 && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + 1.74, f.z)) {
                        by.push(format!("zombie {:.1},{:.1},{:.1} at {d:.2}", z.x, z.y, z.z));
                        let e = by_foe.entry(format!("zombie {:.1},{:.1},{:.1}", z.x, z.y, z.z)).or_default();
                        e.0 += 1;
                        e.1.insert((c.x, c.y, c.z));
                    }
                }
                for f in pills {
                    let d = ((f.x - p.x).powi(2) + (f.y - p.y).powi(2) + (f.z - p.z).powi(2)).sqrt();
                    if d <= PILLAGER_BAR && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + PILLAGER_EYE, f.z)) {
                        by.push(format!("pillager {:.1},{:.1},{:.1} at {d:.2}", f.x, f.y, f.z));
                        let e = by_foe.entry(format!("pillager {:.1},{:.1},{:.1}", f.x, f.y, f.z)).or_default();
                        e.0 += 1;
                        e.1.insert((c.x, c.y, c.z));
                    }
                }
                if by.is_empty() {
                    tails += 1;
                }
                let s = &pl.panic_stroll;
                for (u, _) in s.reached.iter().chain(s.unreached.iter()) {
                    let e = by_cand.entry((u.x, u.y, u.z)).or_insert((w.get(u.below()).to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string(), 0, f64::INFINITY));
                    e.1 += 1;
                    e.2 = e.2.min(*dt);
                }
                // (G1's note on d3dc5aa46: the flee's candidates too, where the flee runs (a hostile
                // within its range at this position), for the pricing; marked)
                let fl = if pl.flee_from > 0 { Some(&pl.flee) } else { None };
                if let Some(f) = fl {
                    for (u, _) in f.reached.iter().chain(f.unreached.iter()) {
                        let e = by_cand.entry((u.x, u.y, u.z)).or_insert((format!("{} [flee]", w.get(u.below()).to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or("")), 0, f64::INFINITY));
                        e.1 += 1;
                        e.2 = e.2.min(*dt);
                    }
                }
                let flee_txt = fl.map_or(String::new(), |f| {
                    let v: Vec<String> = f.reached.iter().map(|(u, _)| (u, true)).chain(f.unreached.iter().map(|(u, _)| (u, false))).map(|(u, r)| format!("{},{},{}{}", u.x, u.y, u.z, if r { "" } else { " (unreached)" })).collect();
                    format!("; flee candidates {}{}", v.len(), if v.is_empty() { String::new() } else { format!(": {}", v.join(", ")) })
                });
                let s = &pl.panic_stroll;
                let cands: Vec<String> = s.reached.iter().map(|(u, _)| (u, true)).chain(s.unreached.iter().map(|(u, _)| (u, false))).map(|(u, r)| format!("{},{},{} on {}{}", u.x, u.y, u.z, w.get(u.below()).to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or(""), if r { "" } else { " (unreached)" })).collect();
                let share: f64 = s.reached_share + s.unreached.iter().map(|(_, sh)| *sh).sum::<f64>();
                rows.push((
                    *dt,
                    format!(
                        "  APPROACH {},{},{} {} rider {:.2},{:.2},{:.2}: {:.1} gt from the entrance; {}; PANIC candidates {}{}; P_null {:.3}{flee_txt}",
                        c.x,
                        c.y,
                        c.z,
                        if *t < 0.0 { "column".to_string() } else { format!("t {t:.2}") },
                        p.x,
                        p.y,
                        p.z,
                        dt,
                        if by.is_empty() { format!("tail ({:.1} gt after the last sight)", best.get(&node_of(idx)).copied().unwrap_or(0.0)) } else { format!("seen by {}", by.join(", ")) },
                        cands.len(),
                        if cands.is_empty() { String::new() } else { format!(": {}", cands.join(", ")) },
                        (1.0 - share).max(0.0)
                    ),
                ));
            }
            rows.sort_by(|a, b| a.0.total_cmp(&b.0));
            lines.push(format!("the approach to {},{},{} within {gt} gt of travel before it (printed; a bar of 0 with a PANIC candidate, as V15's lift approach, is inherited):", cell.x, cell.y, cell.z) + &format!(" {} panicking positions ({tails} in a tail only), {} with a PANIC candidate", rows.len(), rows.iter().filter(|(_, r)| !r.contains("PANIC candidates 0")).count()));
            lines.push(format!(
                "  candidates there ({} cells; each with the block under it, the positions drawing it, the nearest one's time to the entrance): {}",
                by_cand.len(),
                by_cand.iter().map(|((x, y, z), (b, n, dt))| format!("{x},{y},{z} on {b} ({n} positions, {dt:.1} gt)")).collect::<Vec<_>>().join("; ")
            ));
            lines.push(format!(
                "  hostiles seeing positions there: {}",
                if by_foe.is_empty() { "none (every one in a tail)".to_string() } else { by_foe.iter().map(|(f, (n, cells))| format!("{f}: {n} positions on {} cells ({})", cells.len(), cells.iter().map(|(x, y, z)| format!("{x},{y},{z}")).collect::<Vec<_>>().join(" "))).collect::<Vec<_>>().join("; ") }
            ));
            lines.extend(rows.into_iter().map(|(_, r)| r));
        }
    }
    // (G1's CHANGES 1 on d3dc5aa46, ironspec Nov 12 15:03: no silent ends) every track end a walk
    // reached with time left, each declared (a --track-end or --tail-rest naming one of its cells)
    // or failing the run: an undeclared end truncates the tail, late-path or approach walk there
    let mut undeclared_ends = 0usize;
    let mut end_lines = Vec::new();
    let gt = |v: f64| if v > 0.0 { format!("{v:.1} gt") } else { "-".to_string() };
    for (desc, (cells, left)) in &ends {
        let decl = ends_declared.iter().find(|(p, _)| cells.contains(p)).map(|(_, r)| r.clone()).or_else(|| rests.iter().find(|p| cells.contains(p)).map(|p| format!("the --tail-rest {},{},{}", p.x, p.y, p.z)));
        if decl.is_none() {
            undeclared_ends += 1;
        }
        end_lines.push(format!(
            "  END {desc}: tail {} left, late-path {} left, approach {} left{}",
            gt(left[0]),
            gt(left[1]),
            gt(left[2]),
            match decl {
                Some(r) => format!(" (declared: {r})"),
                None => " UNDECLARED".to_string(),
            }
        ));
    }
    lines.push(format!(
        "track ends the walks reach with time left (a rail exit with no rail or column beyond it, or a column's end with no rail: the walk cannot go on there; 0 undeclared, an inherited policy, ironspec Nov 12 15:03): {}",
        if ends.is_empty() {
            "0".to_string()
        } else if undeclared_ends == 0 {
            format!("{}, every one declared", ends.len())
        } else {
            format!("FAIL: {undeclared_ends} undeclared of {} (each truncates a walk with time left: declare it with --track-end x,y,z=REASON or a --tail-rest, pass the fall or column it leads into, or lay the missing track)", ends.len())
        }
    ));
    lines.extend(end_lines);
    lines.extend(travel_lines);
    lines.extend(listed);
    Ok((lines, ClearVerdict { undeclared_ends, gate_fails: fails, late_fails: late_positions, stale: stale.len(), hazards: n_haz }))
}

/// `clear_all`'s verdict, as numbers (ironspec Nov 12 16:23 / 16:52, G1: the command fails on
/// these, never on a printed line): undeclared track ends a walk reaches with time left, the
/// gate's failing positions (none-or-all), the late-path's failing nodes, the staleness failures,
/// and the stale-path hazards (reported).
#[derive(Clone, Copy, Debug, Default)]
pub struct ClearVerdict {
    pub undeclared_ends: usize,
    pub gate_fails: usize,
    pub late_fails: usize,
    pub stale: usize,
    pub hazards: usize,
}

#[cfg(test)]
mod along_rail_tests {
    use super::along_rail;
    use redsim::pos::BlockPos;

    /// (G1 on ec23a9b35) `along_rail` gives a seated villager's feet (its cart's position) at
    /// tick-exact's official positions (df472311b): R_1's centre, R_1 near its high end, a flat top,
    /// the siding slope's centre; a slope's rise included (the positive control for the seat).
    #[test]
    fn along_rail_official_positions() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-4;
        let slope = "minecraft:rail[shape=ascending_south,waterlogged=false]";
        let r1 = along_rail(BlockPos::new(3, 17, 1), slope, 0.5);
        assert!(close(r1.y, 17.5625) && close(r1.z, 1.5), "R_1 centre: y {} z {}", r1.y, r1.z);
        let mid = along_rail(BlockPos::new(3, 17, 1), slope, 0.9804);
        assert!(close(mid.y, 18.0429) && close(mid.z, 1.9804), "mid-slope: y {} z {}", mid.y, mid.z);
        let top = along_rail(BlockPos::new(3, 22, 6), "minecraft:rail[shape=north_south,waterlogged=false]", 0.5);
        assert!(close(top.y, 22.0625), "flat top: y {}", top.y);
        let siding = along_rail(BlockPos::new(1, 24, 8), "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]", 0.5);
        assert!(close(siding.y, 24.5625), "siding slope centre: y {}", siding.y);
    }
}

#[cfg(test)]
mod orient_tests {
    use super::{TailNode, Track};
    use redsim::pos::BlockPos;
    use std::collections::HashMap;

    /// A fall at x 0 z 0 (y 3 down to 1) landing on (0,0,0); a chain south to a T (0,0,2) stored
    /// north_east, its west arm a straight pointing into it, its east arm a straight on to (2,0,2).
    fn rails() -> HashMap<BlockPos, String> {
        let r = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
        let mut m = HashMap::new();
        for (x, z, s) in [(0, 0, "north_south"), (0, 1, "north_south"), (0, 2, "north_east"), (1, 2, "east_west"), (2, 2, "east_west"), (-1, 2, "east_west"), (-2, 2, "east_west")] {
            m.insert(BlockPos::new(x, 0, z), r(s));
        }
        m
    }

    fn n(x: i32, y: i32, z: i32) -> TailNode {
        TailNode::Rail(x, y, z)
    }

    /// (G1's note 2 on a4cd9b8b0: orient()'s control) A chain from a column, the conservation at
    /// a T (marked INFERRED), a conflict kept both ways, a reversal both ways and not a conflict;
    /// the column entered only from a rail leading into it and left only onto its landing.
    #[test]
    fn orient_chain_junction_conflict_reversal() {
        let cols = [[0.5, 3.0, 0.5, 1.0]];
        let rests: [BlockPos; 0] = [];
        let mut t = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        assert_eq!(t.exit_rails(0), vec![BlockPos::new(0, 0, 0)], "the fall lands on the rail under its last cell only");
        assert!(!t.edges(n(0, 0, 0)).iter().any(|(m, _)| matches!(m, TailNode::Col(..))), "the landing does not enter the fall");
        let lines = t.orient(&[], &[], true);
        let a = t.allowed.clone().expect("oriented");
        assert!(a.contains(&(TailNode::Col(0, 8), n(0, 0, 0))), "{lines:?}");
        assert!(a.contains(&(n(0, 0, 0), n(0, 0, 1))) && !a.contains(&(n(0, 0, 1), n(0, 0, 0))), "the chain from the column: {lines:?}");
        assert!(a.contains(&(n(0, 0, 1), n(0, 0, 2))) && !a.contains(&(n(0, 0, 2), n(0, 0, 1))), "{lines:?}");
        assert!(a.contains(&(n(-1, 0, 2), n(0, 0, 2))) && !a.contains(&(n(0, 0, 2), n(-1, 0, 2))), "one way by shape: {lines:?}");
        assert!(a.contains(&(n(0, 0, 2), n(1, 0, 2))) && !a.contains(&(n(1, 0, 2), n(0, 0, 2))), "the conservation at the T: {lines:?}");
        assert!(lines.iter().any(|l| l.contains("ORIENTED 0,0,2 > 1,0,2 (junction (INFERRED))")), "{lines:?}");
        let mut t = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        let lines = t.orient(&[(vec![BlockPos::new(0, 0, 2), BlockPos::new(1, 0, 2)], "a run".into()), (vec![BlockPos::new(1, 0, 2), BlockPos::new(0, 0, 2)], "another".into())], &[], false);
        let a = t.allowed.clone().expect("oriented");
        assert!(a.contains(&(n(0, 0, 2), n(1, 0, 2))) && a.contains(&(n(1, 0, 2), n(0, 0, 2))), "a conflict stays both ways: {lines:?}");
        assert!(lines.iter().any(|l| l.contains("oriented both ways (a conflict") && l.contains("0,0,2-1,0,2")), "{lines:?}");
        let mut t = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        let lines = t.orient(&[], &[(BlockPos::new(0, 0, 1), "the run shows it".into())], false);
        let a = t.allowed.clone().expect("oriented");
        assert!(a.contains(&(n(0, 0, 1), n(0, 0, 0))) && a.contains(&(n(0, 0, 2), n(0, 0, 1))), "a reversal's links both ways: {lines:?}");
        assert!(!lines.iter().any(|l| l.starts_with("  oriented both ways")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("declared --reversal 0,0,1: the run shows it")), "{lines:?}");
    }

    /// (G1's note 4 on a4cd9b8b0) A T with a signal source within Manhattan 2 moves onto every
    /// arm (the union of its shapes); without one, its stored shape's two only.
    #[test]
    fn switched_t_union() {
        let cols: [[f64; 4]; 0] = [];
        let rests: [BlockPos; 0] = [];
        let plain = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        let moves = |t: &Track| {
            let mut m: Vec<BlockPos> = t.rail_moves(BlockPos::new(0, 0, 2));
            m.sort_by_key(|c| (c.x, c.y, c.z));
            m
        };
        assert_eq!(moves(&plain), vec![BlockPos::new(0, 0, 1), BlockPos::new(1, 0, 2)]);
        let lever = Track::from_parts(rails(), &cols, &rests, vec![BlockPos::new(0, 1, 2)], Default::default());
        assert_eq!(moves(&lever), vec![BlockPos::new(-1, 0, 2), BlockPos::new(0, 0, 1), BlockPos::new(1, 0, 2)]);
    }

    /// (no silent ends, with the strict columns) A rail exit into an open cell that a cart reaches
    /// moving toward it is a track end; into a wall (a collision shape) it is not; and an exit the
    /// oriented walk never moves toward (the low rail's move into it not allowed) is not either.
    #[test]
    fn open_ends_falls_walls_and_direction() {
        let cols = [[0.5, 3.0, 0.5, 1.0]];
        let rests: [BlockPos; 0] = [];
        let east_end = |t: &Track| t.open_ends(n(2, 0, 2)).iter().any(|(d, _)| d.contains("exit to 3,0,2"));
        let mut t = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        assert!(east_end(&t), "an open cell beyond the rail end, two-way");
        t.orient(&[], &[], false);
        assert!(east_end(&t), "an open cell beyond the rail end, reached moving east");
        let west_end = |t: &Track| t.open_ends(n(-2, 0, 2)).iter().any(|(d, _)| d.contains("exit to -3,0,2"));
        assert!(!west_end(&t), "the west end: the oriented walk only leaves it eastward");
        let mut wall = Track::from_parts(rails(), &cols, &rests, Vec::new(), [BlockPos::new(3, 0, 2)].into_iter().collect());
        wall.orient(&[], &[], false);
        assert!(!east_end(&wall), "a wall beyond the rail end stops the cart: no track end");
    }

    /// (G1's CHANGES 1 on fa1267b75) A cart arriving from a column: a fall's landing rail with one
    /// exit open is an end, two-way and oriented (it rolls off either way); a lift's exit rail with
    /// its far exit open is an end, and with a rail beyond it is not.
    #[test]
    fn open_ends_from_columns() {
        let rests: [BlockPos; 0] = [];
        let ew = "minecraft:rail[shape=east_west,waterlogged=false]".to_string();
        // (a fall from feet 5 to feet 1 over (0,1,0), landing on (0,0,0); (-1,0,0) beyond its west exit)
        let fall = [[0.5, 5.0, 0.5, 1.0]];
        let landing: HashMap<BlockPos, String> = [(BlockPos::new(0, 0, 0), ew.clone()), (BlockPos::new(-1, 0, 0), ew.clone())].into_iter().collect();
        let east = |t: &Track| t.open_ends(n(0, 0, 0)).iter().any(|(d, _)| d.contains("exit to 1,0,0"));
        let mut t = Track::from_parts(landing.clone(), &fall, &rests, Vec::new(), Default::default());
        assert!(east(&t), "the landing's open exit, two-way");
        t.orient(&[], &[], false);
        assert!(east(&t), "the landing's open exit, oriented (the column moves into the landing)");
        // (a landing on a slope: the cart slides down it, off its low exit, never up to its high one)
        let slope: HashMap<BlockPos, String> = [(BlockPos::new(0, 0, 0), "minecraft:rail[shape=ascending_east,waterlogged=false]".to_string())].into_iter().collect();
        let mut s = Track::from_parts(slope, &fall, &rests, Vec::new(), Default::default());
        s.orient(&[], &[], false);
        let ends: Vec<String> = s.open_ends(n(0, 0, 0)).into_iter().map(|(d, _)| d).collect();
        assert!(ends.iter().any(|d| d.contains("exit to -1,0,0")) && !ends.iter().any(|d| d.contains("exit to 1,1,0")), "{ends:?}");
        // (a lift from feet 1 to feet 5 in (0,·,0), its exit rail (1,5,0) level beside its top)
        let lift = [[0.5, 1.0, 0.5, 5.0]];
        let exit: HashMap<BlockPos, String> = [(BlockPos::new(1, 5, 0), ew.clone())].into_iter().collect();
        let far = |t: &Track| t.open_ends(n(1, 5, 0)).iter().any(|(d, _)| d.contains("exit to 2,5,0"));
        let near = |t: &Track| t.open_ends(n(1, 5, 0)).iter().any(|(d, _)| d.contains("exit to 0,5,0"));
        let mut l = Track::from_parts(exit.clone(), &lift, &rests, Vec::new(), Default::default());
        assert!(far(&l) && !near(&l), "the lift's exit rail: its far exit an end, two-way; the column side not");
        l.orient(&[], &[], false);
        assert!(far(&l), "the lift's exit rail's far exit, oriented");
        let mut beyond = exit.clone();
        beyond.insert(BlockPos::new(2, 5, 0), ew.clone());
        let mut b = Track::from_parts(beyond, &lift, &rests, Vec::new(), Default::default());
        b.orient(&[], &[], false);
        assert!(!far(&b), "a rail beyond the far exit: no end");
    }
}
